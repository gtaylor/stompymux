//! Durable world objects and the aggregate world state.

use super::containment::Links;
use super::shared_map::SharedMap;
use crate::{accounts::Account, communication::Channel};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Stable database identity for a world object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ObjectId(pub i64);

/// Persistent world object category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Room,
    Thing,
    Exit,
    Player,
    Garbage,
}

impl Kind {
    /// Encode the schema-32 object type.
    pub fn code(self) -> i64 {
        match self {
            Self::Room => 0,
            Self::Thing => 1,
            Self::Exit => 2,
            Self::Player => 3,
            Self::Garbage => 5,
        }
    }

    /// Decode a schema-32 object type.
    pub fn from_code(value: i64) -> Result<Self> {
        Ok(match value {
            0 => Self::Room,
            1 => Self::Thing,
            2 => Self::Exit,
            3 => Self::Player,
            5 => Self::Garbage,
            _ => anyhow::bail!("invalid object kind {value}"),
        })
    }

    /// Default Lua parent category.
    pub fn parent(self) -> &'static str {
        match self {
            Self::Room => "room",
            Self::Exit => "exit",
            Self::Player => "player",
            _ => "thing",
        }
    }
}

/// Persistent and transactional state for one world object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Object {
    #[serde(skip)]
    pub generation: crate::state::Generation,
    #[serde(skip)]
    pub pending_destroyer: Option<ObjectId>,
    pub id: ObjectId,
    pub name: String,
    pub kind: Kind,
    pub location: Option<ObjectId>,
    pub zone: Option<ObjectId>,
    pub home: Option<ObjectId>,
    pub affiliation: Option<ObjectId>,
    pub destination: Option<ObjectId>,
    pub dropto: Option<ObjectId>,
    pub description: Option<String>,
    pub internal_description: Option<String>,
    pub lua_parent: String,
    pub flags: crate::flags::FlagSet,
    pub powers: crate::powers::PowerSet,
    pub state: crate::state::State,
}

/// Complete transactional world state.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct World {
    #[serde(skip)]
    pub links: std::sync::Arc<Links>,
    #[serde(skip)]
    pub palette: std::sync::Arc<crate::text::Palette>,
    pub objects: SharedMap<ObjectId, Object>,
    pub accounts: SharedMap<ObjectId, Account>,
    pub channels: SharedMap<String, Channel>,
    pub channel_aliases: BTreeMap<ObjectId, Vec<crate::communication::ChannelAlias>>,
    pub macros: crate::macros::PlayerMacros,
    /// Saved BattleTech identities shared by transaction checkpoints.
    #[serde(default)]
    pub btech: crate::btech::BtechState,
    /// Process-local generic rolls retained after their unit or map stream is retired.
    #[serde(skip)]
    pub btech_retired_rolls: crate::BattleRollStatistics,
    pub last_pages: BTreeMap<ObjectId, Vec<ObjectId>>,
    pub next_id: i64,
    pub record_players: usize,
    pub initialized: bool,
}

impl World {
    /// Apply `change` in place, restoring the world exactly as it was if it fails.
    ///
    /// The snapshot is cheap: every large collection is copy-on-write, so only the
    /// entries `change` actually touches are ever copied.
    pub fn attempt<T>(&mut self, change: impl FnOnce(&mut World) -> Result<T>) -> Result<T> {
        let before = self.clone();
        let result = change(self);
        if result.is_err() {
            *self = before;
        }
        result
    }

    /// True when nothing the database stores differs from `other`.
    ///
    /// Shared collections compare entry by entry with a pointer check first, so this is
    /// cheap when little changed. It may report a difference in fields the database does
    /// not store, but never misses one it does. Runtime-only fields (links, palette and
    /// retired roll statistics) are ignored.
    pub fn saved_state_eq(&self, other: &World) -> bool {
        self.next_id == other.next_id
            && self.record_players == other.record_players
            && self.initialized == other.initialized
            && self.objects == other.objects
            && self.accounts == other.accounts
            && self.channel_aliases == other.channel_aliases
            && self.last_pages == other.last_pages
            && self.btech == other.btech
            && self.channels == other.channels
            && self.macros == other.macros
    }

    /// Resolve a player by dbref, display name, or account alias.
    pub fn find_player(&self, name: &str) -> Option<ObjectId> {
        if let Some(number) = name.strip_prefix('#') {
            if !number.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            let id = ObjectId(number.parse().ok()?);
            return self
                .objects
                .get(&id)
                .filter(|object| object.kind == Kind::Player && self.accounts.contains_key(&id))
                .map(|_| id);
        }
        self.accounts.iter().find_map(|(id, account)| {
            self.objects
                .get(id)
                .filter(|object| {
                    crate::text::plain_with(&self.palette, &object.name).eq_ignore_ascii_case(name)
                        || account
                            .alias
                            .as_ref()
                            .is_some_and(|alias| alias.eq_ignore_ascii_case(name))
                })
                .map(|_| *id)
        })
    }

    /// Decide whether an object is visible to a viewer.
    pub fn visible(&self, object: &Object, viewer: ObjectId) -> bool {
        !object.flags.contains(crate::flags::Flag::Dark)
            || object.id == viewer
            || self
                .objects
                .get(&viewer)
                .is_some_and(|player| player.flags.contains(crate::flags::Flag::Wizard))
    }
}
