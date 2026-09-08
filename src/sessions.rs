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
    /// Remaining credential failures, captured at acceptance and never rolled back.
    pub retry_remaining: i64,
    pub output: mpsc::Sender<Output>,
    /// Shared with the socket task, containing counters only, never world state.
    pub stats: Arc<Stats>,
    pub peer: IpAddr,
    /// Immutable site classification captured at socket acceptance.
    pub site: crate::sites::Classification,
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
    /// Nonpositive configured limits still permit the first attempt, as in C.
    pub fn failed_login(&mut self) -> bool {
        self.retry_remaining = self.retry_remaining.saturating_sub(1);
        self.retry_remaining <= 0
    }

    /// Queue ordered Telnet output; metadata events are already reflected in the decoder.
    pub fn protocol(&self, events: Vec<crate::telnet::Input>, config: &crate::config::Config) {
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
                crate::telnet::Input::Problem(secondary, message) => config.log(
                    &[crate::logging::Category::Problems],
                    "TELNET",
                    secondary,
                    message,
                ),
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

    /// Render a styled report using the recipient's negotiated capabilities.
    pub async fn styled_report(
        &self,
        text: &str,
        ansi: bool,
        config: &crate::config::Config,
    ) -> anyhow::Result<()> {
        let options = self.render_options(ansi);
        let spans = crate::text::Document::Styled(text.into()).spans(&self.palette, &options);
        let spans = bounded_styled_spans(
            spans,
            &self.palette,
            &options,
            self.output_message_limit,
            config.lua.output_byte_limit,
        )?;
        self.deliver_spans(spans, ansi, config).await
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

/// Reserve rendered controls and a visible omission notice when styled reports exceed their budget.
fn bounded_styled_spans(
    spans: Vec<crate::text::Span>,
    palette: &crate::text::Palette,
    options: &crate::text::RenderOptions,
    message_limit: usize,
    total_limit: usize,
) -> anyhow::Result<Vec<crate::text::Span>> {
    use unicode_segmentation::UnicodeSegmentation;
    let fits = |spans: &[crate::text::Span]| {
        let mut check = spans.to_vec();
        check.push(crate::text::Span {
            text: "\n".into(),
            ..Default::default()
        });
        crate::text::telnet_chunks(&check, palette, options, message_limit, total_limit).is_ok()
    };
    if fits(&spans) {
        return Ok(spans);
    }
    let visible: String = spans.iter().map(|s| s.text.as_str()).collect();
    let boundaries: Vec<_> = visible
        .grapheme_indices(true)
        .map(|(i, _)| i)
        .chain(std::iter::once(visible.len()))
        .collect();
    let prefix = |end: usize| {
        let mut left = end;
        let mut result = Vec::new();
        for span in &spans {
            if left == 0 {
                break;
            }
            let mut span = span.clone();
            let take = left.min(span.text.len());
            span.text.truncate(take);
            left -= take;
            result.push(span);
        }
        result.push(crate::text::Span {
            text: "\n***Report truncated: additional results omitted***".into(),
            ..Default::default()
        });
        result
    };
    anyhow::ensure!(fits(&prefix(0)), "Report output limit too small.");
    let (mut low, mut high) = (0, boundaries.len());
    while low + 1 < high {
        let mid = (low + high) / 2;
        if fits(&prefix(boundaries[mid])) {
            low = mid;
        } else {
            high = mid;
        }
    }
    Ok(prefix(boundaries[low]))
}

#[cfg(test)]
mod report_tests {
    use super::*;
    /// Rendered limits include style controls and preserve Unicode graphemes before notices.
    #[test]
    fn styled_reports_mark_aggregate_truncation() {
        let palette = crate::text::Palette::default();
        let options = crate::text::RenderOptions::default();
        let spans = crate::text::Document::Styled(format!("[bold]{}[/]", "é👩‍🚀".repeat(100)))
            .spans(&palette, &options);
        let bounded = bounded_styled_spans(spans, &palette, &options, 80, 200).unwrap();
        let visible: String = bounded.iter().map(|s| s.text.as_str()).collect();
        assert!(visible.contains("Report truncated"));
        let prefix = visible.split('\n').next().unwrap();
        assert!(matches!(prefix.replace("é👩‍🚀", "").as_str(), "" | "é"));
        let chunks = crate::text::telnet_chunks(&bounded, &palette, &options, 80, 200).unwrap();
        assert!(chunks.iter().all(|c| c.len() <= 80));
        assert!(chunks.iter().map(Vec::len).sum::<usize>() <= 200);
    }
}
