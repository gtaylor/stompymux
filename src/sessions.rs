//! Connection state, bounded output and session-owned search cursors.
use crate::{telnet::Decoder, world::ObjectId};
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
}
pub struct Session {
    pub output: mpsc::Sender<Output>,
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
    pub fn raw(&self, data: Vec<u8>) -> bool {
        let ok = data.len() <= self.output_message_limit
            && self.output.try_send(Output::Bytes(data)).is_ok();
        if !ok {
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
