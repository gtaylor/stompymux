//! Shared channel and paging services, independent of command spelling and Lua userdata.
mod commands;
mod delivery;
mod membership;
mod page;
use crate::{
    config::Config,
    flags::Flag,
    lua::{Outbox, SharedWorld},
    world::{Kind, ObjectId, World},
};
use anyhow::{Result, ensure};
pub use commands::{addcom, admin, alias, allcom, clearcom, comlist, delcom, page};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

/// C grammar/storage limits, not new operator tunables.
pub const HISTORY_LIMIT: usize = 20;
/// Maximum bytes before the legacy terminating NUL.
pub const CHANNEL_NAME_LIMIT: usize = 49;
/// Per-player shorthand bytes before the legacy terminating NUL.
pub const ALIAS_LIMIT: usize = 5;

/// Never reuse a channel identity, including after rollback of a provisional creation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelId(pub u64);

/// Allocate identities outside world snapshots so stale Lua handles cannot revive.
fn identity() -> ChannelId {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    ChannelId(NEXT.fetch_add(1, Ordering::Relaxed))
}

/// Typed channel properties; permission bits and unknown stored bits remain independent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelFlag {
    Public,
    Loud,
    Transparent,
}

impl ChannelFlag {
    /// Canonical spelling used by typed Lua constants.
    pub fn name(self) -> &'static str {
        match self {
            Self::Public => "PUBLIC",
            Self::Loud => "LOUD",
            Self::Transparent => "TRANSPARENT",
        }
    }

    /// Stable schema-32 mask; unknown bits remain untouched.
    pub fn bit(self) -> i64 {
        match self {
            Self::Public => 0x200,
            Self::Loud => 0x100,
            Self::Transparent => 0x400,
        }
    }

    /// Resolve case-insensitive administrative flag spelling.
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "public" => Ok(Self::Public),
            "loud" => Ok(Self::Loud),
            "transparent" => Ok(Self::Transparent),
            _ => anyhow::bail!("Unknown channel flag."),
        }
    }
}

/// Legacy channel mask retains future/deferred bits during known flag changes.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct ChannelFlags(pub i64);

impl ChannelFlags {
    /// Read a single known property without masking deferred bits.
    pub fn has(self, flag: ChannelFlag) -> bool {
        self.0 & flag.bit() != 0
    }

    /// Change one property and report whether it changed.
    pub fn set(&mut self, flag: ChannelFlag, enabled: bool) -> bool {
        let before = self.0;
        if enabled {
            self.0 |= flag.bit()
        } else {
            self.0 &= !flag.bit()
        }
        before != self.0
    }
}

/// Separate player/object access bits in the low byte of the legacy channel mask.
#[derive(Clone, Copy)]
pub enum Access {
    Join,
    Transmit,
    Receive,
}

impl Access {
    /// Stable schema-32 mask; unknown bits remain untouched.
    pub fn bit(self) -> i64 {
        match self {
            Self::Join => 1,
            Self::Transmit => 2,
            Self::Receive => 4,
        }
    }

    /// Canonical object-module lock key.
    pub fn lock(self) -> &'static str {
        match self {
            Self::Join => "channel_join",
            Self::Transmit => "channel_transmit",
            Self::Receive => "channel_receive",
        }
    }
}

/// Persistent listening preference; actual online state is derived from sessions.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Membership {
    /// Member object identity; online status is derived independently.
    pub who: ObjectId,
    /// Persisted on/off preference.
    pub listening: bool,
}

/// One player's case-insensitive channel shorthand, in legacy position order.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelAlias {
    /// Player-local, case-insensitive command token.
    pub alias: String,
    /// Canonical channel name, or a retained stale legacy alias target.
    pub channel: String,
}

/// Stored rendered-source history, ordered oldest to newest.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelMessage {
    /// Unix timestamp in seconds.
    pub at: i64,
    /// Styled source text retained for history rendering.
    pub message: String,
}

/// Durable channel metadata plus runtime-only handle identity/capacity information.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Channel {
    /// Canonical persistent channel name.
    pub name: String,
    /// Optional object supplying locks and description.
    pub object: Option<ObjectId>,
    /// Known access/property bits plus preserved unknown bits.
    pub flags: ChannelFlags,
    /// Lifetime message count, independent of history retention.
    pub messages: i64,
    /// Memberships in legacy slot order.
    pub users: Vec<Membership>,
    /// Oldest-to-newest bounded message history.
    pub history: Vec<ChannelMessage>,
    #[serde(skip, default = "identity")]
    /// Runtime identity, never stored in the legacy schema.
    pub id: ChannelId,
    #[serde(skip)]
    /// C-compatible membership capacity; runtime-only.
    pub max_users: usize,
}

impl Channel {
    /// C creates private channels with player/object permissions in mask 127.
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
        }
    }
}

/// Shared transaction dependencies; all closures run on the existing world thread.
pub struct Service<'a> {
    /// World owned serially by the server and shared Lua VM.
    pub world: &'a SharedWorld,
    /// Output staged in the current transaction.
    pub outbox: &'a Outbox,
    /// Effective communication and output-limit configuration.
    pub config: &'a Config,
    /// Current sandbox VM; callbacks share its instruction budget.
    pub lua: &'a mlua::Lua,
}

/// C communication IC policy: traverse player containers and honor OOC override/GAGGED.
pub fn in_character(w: &World, config: &Config, player: ObjectId) -> bool {
    let Some(o) = w.objects.get(&player) else {
        return true;
    };
    if o.flags.contains(Flag::Gagged) {
        return true;
    }
    if config.battletech.ooc_comsys != 0 {
        return false;
    }
    let mut location = o.location;
    for _ in 0..100 {
        let Some(o) = location.and_then(|id| w.objects.get(&id)) else {
            return false;
        };
        if o.kind != Kind::Player {
            return o.flags.contains(Flag::InCharacter);
        }
        if location == o.location {
            break;
        }
        location = o.location;
    }
    location
        .and_then(|id| w.objects.get(&id))
        .is_some_and(|o| o.flags.contains(Flag::InCharacter))
}

/// Existing Wizard/GOD authority used by all communication operations.
pub fn wizard(w: &World, id: ObjectId) -> bool {
    id.0 == 1
        || w.objects
            .get(&id)
            .is_some_and(|o| o.flags.contains(Flag::Wizard))
}

impl Service<'_> {
    /// Resolve the canonical channel name without creating it.
    pub fn name(&self, name: &str) -> Result<String> {
        self.world
            .borrow()
            .channels
            .keys()
            .find(|n| n.eq_ignore_ascii_case(name))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Unknown channel {name}."))
    }

    /// Resolve a generation-sensitive handle; removed identities never revive.
    pub fn by_id(&self, id: ChannelId) -> Result<String> {
        self.world
            .borrow()
            .channels
            .values()
            .find(|c| c.id == id)
            .map(|c| c.name.clone())
            .ok_or_else(|| anyhow::anyhow!("channel handle is stale"))
    }

    /// Validate the legacy name grammar and allocate a private channel.
    pub fn create(&self, name: &str) -> Result<ChannelId> {
        ensure!(!name.is_empty(), "You must specify a channel to create.");
        ensure!(
            name.len() <= CHANNEL_NAME_LIMIT && name.bytes().all(|b| (33..=126).contains(&b)),
            "Channel names must be printable ASCII without spaces."
        );
        ensure!(self.name(name).is_err(), "Channel already exists.");
        let channel = Channel::new(name.into());
        let id = channel.id;
        self.world
            .borrow_mut()
            .channels
            .insert(name.into(), channel);
        Ok(id)
    }

    /// Remove owned channel state while retaining unrelated communication macros.
    pub fn destroy(&self, name: &str) -> Result<()> {
        let name = self.name(name)?;
        let mut w = self.world.borrow_mut();
        w.channels.remove(&name);
        for aliases in w.channel_aliases.values_mut() {
            aliases.retain(|a| !a.channel.eq_ignore_ascii_case(&name));
        }
        Ok(())
    }

    /// Stage bounded styled output for delivery only after transaction commit.
    pub fn notify(&self, player: ObjectId, message: impl Into<String>) -> Result<()> {
        let message = message.into();
        let mut out = self.outbox.borrow_mut();
        ensure!(
            message.len() <= self.config.runtime.output_message_limit
                && out.len() < self.config.lua.output_entry_limit
                && out.iter().map(|(_, d)| d.len()).sum::<usize>() + message.len()
                    <= self.config.lua.output_byte_limit,
            "Communication output limit exceeded"
        );
        out.push((player, message.into()));
        Ok(())
    }

    /// Apply Wizard bypass, Lua grant locks and independent legacy access bits.
    pub fn allowed(&self, who: ObjectId, channel: &str, access: Access) -> Result<bool> {
        let (bits, object, player) = {
            let w = self.world.borrow();
            if wizard(&w, who) {
                return Ok(true);
            };
            let o = w
                .objects
                .get(&who)
                .ok_or_else(|| anyhow::anyhow!("object does not exist"))?;
            let c = w
                .channels
                .get(channel)
                .ok_or_else(|| anyhow::anyhow!("channel removed by callback"))?;
            (c.flags.0, c.object, o.kind == Kind::Player)
        };
        // Locks grant access independently of flags, including C's missing-lock default.
        if let Some(object) = object.filter(|id| id.0 != 0) {
            let f:mlua::Function=self.lua.load("return function(o,p,k) return mux.world.lock_passes({object=mux.world.object(o),enactor=p,subject=p,cause=p,lock=k}) end").eval().map_err(|e|anyhow::anyhow!(e.to_string()))?;
            let before = self.world.borrow().clone();
            let pending = self.outbox.borrow().len();
            match f.call::<bool>((object.0, who.0, access.lock())) {
                Ok(true) => return Ok(true),
                Ok(false) => {}
                Err(e) => {
                    *self.world.borrow_mut() = before;
                    self.outbox.borrow_mut().truncate(pending);
                    eprintln!("Channel lock {}: {e}", access.lock());
                }
            }
        }
        Ok(bits & (access.bit() * if player { 1 } else { 16 }) != 0)
    }
}
