//! Connection state, bounded output and session-owned search cursors.
use crate::telnet::transport::Stats;
use crate::{telnet::Decoder, world::ObjectId};
use std::sync::{Arc, atomic::Ordering::Relaxed};
use std::{cell::Cell, net::IpAddr, time::Instant};
use tokio::sync::mpsc;
use zeroize::Zeroizing;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SessionId(pub u64);
pub enum LoginFlow {
    Name,
    Password(String),
    ConfirmCreate(String),
    NewPassword(String),
    ConfirmPassword(String, Zeroizing<String>),
    Pending,
}
pub enum Output {
    Bytes(Vec<u8>),
    Close,
    StartCompression,
}
pub struct Session {
    pub output: mpsc::Sender<Output>,
    /// Shared with the socket task, containing counters only, never world state.
    pub stats: Arc<Stats>,
    pub peer: IpAddr,
    pub player: Option<ObjectId>,
    /// Interactive login state.
    pub flow: LoginFlow,
    pub connected: Instant,
    pub active: Instant,
    pub decoder: Decoder,
    pub quota: usize,
    pub quota_at: Instant,
    pub failed: Cell<bool>,
    pub output_message_limit: usize,
    /// Rendering catalog and transient connection preferences.
    pub palette: Arc<crate::text::Palette>,
    pub color_override: Cell<Option<crate::text::ColorDepth>>,
    pub presets_emitted: Cell<bool>,
}
impl Session {
    /// Queue ordered Telnet output; metadata events are already reflected in the decoder.
    pub fn protocol(&self, events: Vec<crate::telnet::Input>) {
        for event in events {
            match event {
                crate::telnet::Input::Reply(bytes) => {
                    self.raw(bytes);
                }
                crate::telnet::Input::StartCompression => {
                    if self
                        .stats
                        .compression
                        .compare_exchange(0, 1, Relaxed, Relaxed)
                        .is_ok()
                        && self.output.try_send(Output::StartCompression).is_err()
                    {
                        self.failed.set(true);
                        self.stats.compression.store(0, Relaxed);
                    }
                }
                crate::telnet::Input::Diagnostic(message) => eprintln!("Telnet: {message}"),
                _ => {}
            }
        }
    }
    pub fn raw(&self, data: Vec<u8>) -> bool {
        if data.is_empty() {
            return true;
        }
        let n = data.len() as u64;
        self.stats.output_total.fetch_add(n, Relaxed);
        self.stats.output_pending.fetch_add(n, Relaxed);
        let ok = data.len() <= self.output_message_limit
            && self.output.try_send(Output::Bytes(data)).is_ok();
        if !ok {
            self.stats.output_pending.fetch_sub(n, Relaxed);
            self.stats.output_lost.fetch_add(n, Relaxed);
            self.failed.set(true);
        }
        ok
    }

    /// Resolve capabilities independently for each receiving session.
    pub fn render_options(&self, ansi: bool) -> crate::text::RenderOptions {
        use crate::text::ColorDepth;
        let color = if !ansi {
            ColorDepth::None
        } else if let Some(c) = self.color_override.get() {
            c
        } else if self.decoder.screen_reader {
            ColorDepth::None
        } else {
            ColorDepth::advertised(self.decoder.color_depth)
        };
        let capabilities = self
            .decoder
            .environment
            .0
            .iter()
            .filter_map(|((kind, name), value)| {
                if *kind != crate::telnet::environment::Kind::UserVar || value != b"1" {
                    return None;
                }
                let name = std::str::from_utf8(name).ok()?;
                if name == "OSC_HYPERLINKS" {
                    Some(String::new())
                } else {
                    name.strip_prefix("OSC_HYPERLINKS_")
                        .filter(|suffix| crate::text::OSC_CAPABILITIES.contains(suffix))
                        .map(str::to_string)
                }
            })
            .collect();
        crate::text::RenderOptions {
            color,
            capabilities,
            width: usize::from(self.decoder.width).max(1),
        }
    }

    /// Render game text before Telnet encoding, queue accounting and compression.
    pub fn document(&self, document: &crate::text::Document, ansi: bool, newline: bool) -> bool {
        let options = self.render_options(ansi);
        if options.has("PRESETS") && !self.presets_emitted.replace(true) {
            for (name, config) in &self.palette.presets {
                let bytes = crate::telnet::encode(&crate::text::preset(name, config, &options));
                if !self.raw(bytes) {
                    return false;
                }
            }
        }
        let limit = self
            .output_message_limit
            .saturating_sub(if newline { 2 } else { 0 });
        let mut bytes = document.telnet(&self.palette, &options, limit);
        if newline && self.output_message_limit >= 2 {
            bytes.extend_from_slice(b"\r\n");
        }
        self.raw(bytes)
    }

    /// Deliver a complete help response through the same transport and accounting boundaries.
    pub async fn help(
        &self,
        response: &crate::help::HelpResponse,
        ansi: bool,
        config: &crate::config::Config,
    ) -> anyhow::Result<()> {
        let options = self.render_options(ansi);
        self.deliver_spans(response.spans(&options), ansi, config)
            .await
    }

    /// Display source without interpreting markup or wrapping indentation.
    pub async fn literal_report(
        &self,
        text: &str,
        config: &crate::config::Config,
    ) -> anyhow::Result<()> {
        let options = self.render_options(false);
        let spans = crate::text::Document::Literal(text.into()).spans(&self.palette, &options);
        self.deliver_spans(spans, false, config).await
    }

    /// Reserve bounded queue slots under one deadline for a complete logical report.
    async fn deliver_spans(
        &self,
        mut spans: Vec<crate::text::Span>,
        ansi: bool,
        config: &crate::config::Config,
    ) -> anyhow::Result<()> {
        let options = self.render_options(ansi);
        spans.push(crate::text::Span {
            text: "\n".into(),
            ..Default::default()
        });
        let chunks = crate::text::telnet_chunks(
            &spans,
            &self.palette,
            &options,
            self.output_message_limit,
            config.lua.output_byte_limit,
        )?;
        let deadline = tokio::time::Instant::now()
            + std::time::Duration::from_millis(config.runtime.write_timeout_ms);
        for chunk in chunks {
            let n = chunk.len() as u64;
            self.stats.output_total.fetch_add(n, Relaxed);
            let permit = tokio::time::timeout_at(deadline, self.output.reserve()).await;
            match permit {
                Ok(Ok(permit)) => {
                    self.stats.output_pending.fetch_add(n, Relaxed);
                    permit.send(Output::Bytes(chunk));
                }
                _ => {
                    self.stats.output_lost.fetch_add(n, Relaxed);
                    self.failed.set(true);
                    anyhow::bail!("help output queue unavailable");
                }
            }
        }
        Ok(())
    }

    pub fn text(&self, s: &str, ansi: bool) -> bool {
        self.document(&crate::text::Document::Styled(s.into()), ansi, false)
    }

    pub fn close(&self) {
        let _ = self.output.try_send(Output::Close);
    }
}
