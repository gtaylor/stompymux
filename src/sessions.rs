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
    /// Pending object search, discarded when this session disconnects.
    pub find_cursor: Option<crate::find::FindCursor>,
    pub flow: LoginFlow,
    pub connected: Instant,
    pub active: Instant,
    pub decoder: Decoder,
    pub quota: usize,
    pub quota_at: Instant,
    pub failed: Cell<bool>,
    pub output_message_limit: usize,
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
    pub fn text(&self, s: &str, ansi: bool) -> bool {
        self.raw(crate::telnet::encode(&if ansi && self.decoder.ansi {
            s.into()
        } else {
            crate::text::plain(s)
        }))
    }
    pub fn close(&self) {
        let _ = self.output.try_send(Output::Close);
    }
}
