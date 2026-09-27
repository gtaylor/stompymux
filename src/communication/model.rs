//! Persistent channel, membership, alias, and history data.

use crate::world::ObjectId;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

/// C grammar/storage limits.
pub const HISTORY_LIMIT: usize = 20;
pub const CHANNEL_NAME_LIMIT: usize = 49;
pub const ALIAS_LIMIT: usize = 5;

/// Never-reused runtime channel identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelId(pub u64);

fn identity() -> ChannelId {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    ChannelId(NEXT.fetch_add(1, Ordering::Relaxed))
}

/// Typed channel property.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelFlag {
    Public,
    Loud,
    Transparent,
}

impl ChannelFlag {
    pub fn name(self) -> &'static str {
        match self {
            Self::Public => "PUBLIC",
            Self::Loud => "LOUD",
            Self::Transparent => "TRANSPARENT",
        }
    }

    pub fn bit(self) -> i64 {
        match self {
            Self::Public => 0x200,
            Self::Loud => 0x100,
            Self::Transparent => 0x400,
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "public" => Ok(Self::Public),
            "loud" => Ok(Self::Loud),
            "transparent" => Ok(Self::Transparent),
            _ => anyhow::bail!("Unknown channel flag."),
        }
    }
}

/// Legacy channel mask, including unknown retained bits.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct ChannelFlags(pub i64);

impl ChannelFlags {
    pub fn has(self, flag: ChannelFlag) -> bool {
        self.0 & flag.bit() != 0
    }

    pub fn set(&mut self, flag: ChannelFlag, enabled: bool) -> bool {
        let before = self.0;
        if enabled {
            self.0 |= flag.bit();
        } else {
            self.0 &= !flag.bit();
        }
        before != self.0
    }
}

/// Independent channel access category.
#[derive(Clone, Copy)]
pub enum Access {
    Join,
    Transmit,
    Receive,
}

impl Access {
    pub fn bit(self) -> i64 {
        match self {
            Self::Join => 1,
            Self::Transmit => 2,
            Self::Receive => 4,
        }
    }

    pub fn lock(self) -> crate::LockType {
        match self {
            Self::Join => crate::LockType::ChannelJoin,
            Self::Transmit => crate::LockType::ChannelTransmit,
            Self::Receive => crate::LockType::ChannelReceive,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Membership {
    pub who: ObjectId,
    pub listening: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelAlias {
    pub alias: String,
    pub channel: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelMessage {
    pub at: i64,
    pub message: String,
}

/// Durable channel state plus runtime handle and online ordering. Equality compares the
/// durable fields only.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Channel {
    pub name: String,
    pub object: Option<ObjectId>,
    pub flags: ChannelFlags,
    pub messages: i64,
    pub users: Vec<Membership>,
    pub history: Vec<ChannelMessage>,
    #[serde(skip, default = "identity")]
    pub id: ChannelId,
    #[serde(skip)]
    pub max_users: usize,
    #[serde(skip)]
    pub online: Vec<ObjectId>,
    #[serde(skip)]
    pub online_initialized: bool,
}

impl PartialEq for Channel {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.object == other.object
            && self.flags == other.flags
            && self.messages == other.messages
            && self.users == other.users
            && self.history == other.history
    }
}

impl Eq for Channel {}

impl Channel {
    pub fn new(name: String) -> Self {
        Self {
            name,
            object: None,
            flags: ChannelFlags(127),
            messages: 0,
            users: Vec::new(),
            history: Vec::new(),
            id: identity(),
            max_users: 0,
            online: Vec::new(),
            online_initialized: false,
        }
    }
}
