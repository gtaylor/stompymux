//! Shared non-BattleTech lock identities and invocation contracts.
use crate::world::ObjectId;

/// Catalog of policies understood by both native operations and Lua modules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockType {
    Match,
    Traverse,
    Take,
    Use,
    Drop,
    Give,
    Receive,
    Enter,
    Leave,
    Teleport,
    TeleportOut,
    Link,
    SetHome,
    Speak,
    ChannelJoin,
    ChannelTransmit,
    ChannelReceive,
}

/// Canonical registration order from the object-lock catalog.
pub const LOCKS: [LockType; 17] = [
    LockType::Match,
    LockType::Traverse,
    LockType::Take,
    LockType::Use,
    LockType::Drop,
    LockType::Give,
    LockType::Receive,
    LockType::Enter,
    LockType::Leave,
    LockType::Teleport,
    LockType::TeleportOut,
    LockType::Link,
    LockType::SetHome,
    LockType::Speak,
    LockType::ChannelJoin,
    LockType::ChannelTransmit,
    LockType::ChannelReceive,
];

impl LockType {
    pub fn key(self) -> &'static str {
        match self {
            Self::Match => "match",
            Self::Traverse => "traverse",
            Self::Take => "take",
            Self::Use => "use",
            Self::Drop => "drop",
            Self::Give => "give",
            Self::Receive => "receive",
            Self::Enter => "enter",
            Self::Leave => "leave",
            Self::Teleport => "teleport",
            Self::TeleportOut => "teleport_out",
            Self::Link => "link",
            Self::SetHome => "set_home",
            Self::Speak => "speak",
            Self::ChannelJoin => "channel_join",
            Self::ChannelTransmit => "channel_transmit",
            Self::ChannelReceive => "channel_receive",
        }
    }
    pub fn name(self) -> String {
        self.key().to_ascii_uppercase()
    }
    pub fn from_key(key: &str) -> Option<Self> {
        LOCKS.into_iter().find(|lock| lock.key() == key)
    }
}

/// Typed native identities; optional descriptor must identify the applicable player session.
#[derive(Clone, Copy, Debug)]
pub struct LockInvocation {
    pub kind: LockType,
    pub object: ObjectId,
    pub enactor: ObjectId,
    pub cause: ObjectId,
    pub subject: ObjectId,
    pub descriptor: Option<u64>,
    pub silent: bool,
}

/// A checked result; absent messages request the command's defaults.
#[derive(Clone, Debug)]
pub struct LockOutcome {
    pub passes: bool,
    pub enactor_message: Option<String>,
    pub other_message: Option<String>,
}
