//! Typed world objects, relationships and persistent account state.
use crate::config::Config;
use anyhow::Context;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ObjectId(pub i64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Room,
    Thing,
    Exit,
    Player,
    Garbage,
}
impl Kind {
    pub fn code(self) -> i64 {
        match self {
            Self::Room => 0,
            Self::Thing => 1,
            Self::Exit => 2,
            Self::Player => 3,
            Self::Garbage => 5,
        }
    }
    pub fn from_code(v: i64) -> Result<Self> {
        Ok(match v {
            0 => Self::Room,
            1 => Self::Thing,
            2 => Self::Exit,
            3 => Self::Player,
            5 => Self::Garbage,
            _ => anyhow::bail!("invalid object kind {v}"),
        })
    }
    pub fn parent(self) -> &'static str {
        match self {
            Self::Room => "room",
            Self::Exit => "exit",
            Self::Player => "player",
            _ => "thing",
        }
    }
}
pub use crate::state::Value as Scalar;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Object {
    /// Runtime incarnation retained across rollback but never persisted.
    #[serde(skip)]
    pub generation: crate::state::Generation,
    pub id: ObjectId,
    pub name: String,
    pub kind: Kind,
    pub location: Option<ObjectId>,
    pub zone: Option<ObjectId>,
    pub home: Option<ObjectId>,
    pub affiliation: Option<ObjectId>,
    pub destination: Option<ObjectId>,
    /// Legacy room location slot, separate from physical containment.
    pub dropto: Option<ObjectId>,
    pub description: Option<String>,
    pub internal_description: Option<String>,
    pub lua_parent: String,
    pub flags: crate::flags::FlagSet,
    pub powers: crate::powers::PowerSet,
    pub state: crate::state::State,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Account {
    pub hash: Option<String>,
    pub alias: Option<String>,
    pub last_login: Option<i64>,
    pub last_site: Option<String>,
    pub successes: i64,
    pub failures: i64,
    pub unreported_failures: i64,
    pub history: Vec<Login>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Login {
    pub success: bool,
    pub at: i64,
    pub host: String,
}
pub use crate::communication::Channel;
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct World {
    /// Runtime-only palette, excluded from all storage formats.
    #[serde(skip)]
    pub palette: std::sync::Arc<crate::text::Palette>,
    pub objects: BTreeMap<ObjectId, Object>,
    pub accounts: BTreeMap<ObjectId, Account>,
    pub channels: BTreeMap<String, Channel>,
    pub channel_aliases: BTreeMap<ObjectId, Vec<crate::communication::ChannelAlias>>,
    pub last_pages: BTreeMap<ObjectId, Vec<ObjectId>>,
    pub next_id: i64,
    pub record_players: usize,
    pub initialized: bool,
}
impl World {
    pub fn create(&mut self, c: &Config, name: String, kind: Kind) -> ObjectId {
        let id = ObjectId(self.next_id);
        self.next_id += 1;
        let (flags, parent) = match kind {
            Kind::Player => (
                &c.mux.default_player_flags,
                &c.mux.default_player_lua_parent,
            ),
            Kind::Room => (&c.mux.default_room_flags, &c.mux.default_room_lua_parent),
            Kind::Exit => (&c.mux.default_exit_flags, &c.mux.default_exit_lua_parent),
            _ => (&c.mux.default_thing_flags, &c.mux.default_thing_lua_parent),
        };
        let flags = flags
            .iter()
            .copied()
            .filter(|f| *f != crate::flags::Flag::Connected)
            .collect();
        self.objects.insert(
            id,
            Object {
                generation: Default::default(),
                id,
                name,
                kind,
                location: None,
                zone: None,
                home: None,
                affiliation: None,
                destination: None,
                dropto: None,
                description: None,
                internal_description: None,
                lua_parent: parent.clone(),
                flags,
                powers: Default::default(),
                state: BTreeMap::new(),
            },
        );
        id
    }
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
        self.accounts.iter().find_map(|(id, a)| {
            self.objects
                .get(id)
                .filter(|o| {
                    crate::text::plain_with(&self.palette, &o.name).eq_ignore_ascii_case(name)
                        || a.alias
                            .as_ref()
                            .is_some_and(|s| s.eq_ignore_ascii_case(name))
                })
                .map(|_| *id)
        })
    }
    /// Walk enclosing containers, rejecting invalid types, references and cycles.
    pub fn containment_chain(&self, start: Option<ObjectId>) -> Result<Vec<ObjectId>> {
        let mut chain = Vec::new();
        let mut seen = BTreeSet::new();
        let mut current = start;
        while let Some(id) = current {
            ensure!(seen.insert(id), "Containment cycle at #{}.", id.0);
            let o = self
                .objects
                .get(&id)
                .ok_or_else(|| anyhow::anyhow!("Missing container #{}.", id.0))?;
            ensure!(
                matches!(o.kind, Kind::Room | Kind::Player | Kind::Thing),
                "Object #{} cannot contain objects.",
                id.0
            );
            chain.push(id);
            if o.kind == Kind::Room {
                ensure!(
                    o.location.is_none(),
                    "Room #{} has an invalid location.",
                    id.0
                );
                break;
            }
            current = o.location;
        }
        Ok(chain)
    }
    /// Check the complete destination chain before any location mutation.
    pub fn validate_move(&self, object: ObjectId, destination: ObjectId) -> Result<()> {
        let o = self
            .objects
            .get(&object)
            .ok_or_else(|| anyhow::anyhow!("No such object."))?;
        ensure!(
            matches!(o.kind, Kind::Player | Kind::Thing | Kind::Exit),
            "You can't teleport that."
        );
        let chain = self.containment_chain(Some(destination))?;
        ensure!(
            !chain.contains(&object),
            "Cannot move an object into itself or its descendants."
        );
        self.containment_chain(o.location)?;
        Ok(())
    }
    /// Account identities cannot be guessed or repaired by semantic maintenance.
    pub fn validate_accounts(&self) -> Result<()> {
        let mut names = BTreeSet::new();
        for (id, a) in &self.accounts {
            let o = self
                .objects
                .get(id)
                .ok_or_else(|| anyhow::anyhow!("account object missing"))?;
            ensure!(o.kind == Kind::Player, "account is not a player");
            ensure!(
                names.insert(crate::text::plain_with(&self.palette, &o.name).to_ascii_lowercase()),
                "duplicate player name"
            );
            if let Some(alias) = &a.alias
                && !alias.is_empty()
                && !alias.eq_ignore_ascii_case(&crate::text::plain_with(&self.palette, &o.name))
            {
                ensure!(
                    names.insert(alias.to_ascii_lowercase()),
                    "duplicate player alias"
                );
            }
        }
        Ok(())
    }
    pub fn validate(&self, c: &Config) -> Result<()> {
        for id in [c.start(), c.home()] {
            ensure!(
                self.objects
                    .get(&ObjectId(id))
                    .is_some_and(|o| o.kind == Kind::Room),
                "starting room/home #{id} missing or not a room"
            );
        }
        self.validate_accounts()?;
        for o in self.objects.values().filter(|o| o.kind != Kind::Garbage) {
            crate::state::validate(&o.state, c)
                .with_context(|| format!("object #{} state", o.id.0))?;
            let chain = self.containment_chain(o.location)?;
            ensure!(!chain.contains(&o.id), "Containment cycle at #{}.", o.id.0);
            ensure!(
                o.kind != Kind::Room || o.location.is_none(),
                "Room #{} has an invalid location.",
                o.id.0
            );
            for id in [
                o.location,
                o.home,
                o.destination,
                o.zone,
                o.affiliation,
                o.dropto,
            ]
            .into_iter()
            .flatten()
            {
                ensure!(
                    self.objects
                        .get(&id)
                        .is_some_and(|target| target.kind != Kind::Garbage),
                    "{} references missing #{}",
                    o.name,
                    id.0
                );
            }
        }
        let mut channel_names = BTreeSet::new();
        for channel in self.channels.values() {
            ensure!(
                channel_names.insert(channel.name.to_ascii_lowercase()),
                "duplicate channel {}",
                channel.name
            );
            ensure!(
                channel.messages >= 0,
                "channel {} has a negative message count",
                channel.name
            );
            let mut members = BTreeSet::new();
            for member in &channel.users {
                ensure!(
                    members.insert(member.who),
                    "channel {} has duplicate member #{}",
                    channel.name,
                    member.who.0
                );
                ensure!(
                    self.objects
                        .get(&member.who)
                        .is_some_and(|o| o.kind != Kind::Garbage),
                    "channel {} has invalid member #{}",
                    channel.name,
                    member.who.0
                );
            }
            if let Some(id) = channel.object {
                ensure!(
                    self.objects
                        .get(&id)
                        .is_some_and(|o| o.kind != Kind::Garbage),
                    "channel {} references invalid #{}",
                    channel.name,
                    id.0
                );
            }
        }
        for (who, aliases) in &self.channel_aliases {
            ensure!(
                self.objects
                    .get(who)
                    .is_some_and(|o| o.kind != Kind::Garbage),
                "channel aliases have invalid owner #{}",
                who.0
            );
            let mut names = BTreeSet::new();
            for alias in aliases {
                ensure!(
                    names.insert(alias.alias.to_ascii_lowercase()),
                    "duplicate channel alias {} for #{}",
                    alias.alias,
                    who.0
                );
            }
        }
        for (who, recipients) in &self.last_pages {
            ensure!(
                self.accounts.contains_key(who),
                "last-page owner #{} is not a player",
                who.0
            );
            for recipient in recipients {
                ensure!(
                    self.accounts.contains_key(recipient),
                    "last-page recipient #{} is not a player",
                    recipient.0
                );
            }
        }
        Ok(())
    }
    pub fn visible(&self, o: &Object, viewer: ObjectId) -> bool {
        !o.flags.contains(crate::flags::Flag::Dark)
            || o.id == viewer
            || self
                .objects
                .get(&viewer)
                .is_some_and(|p| p.flags.contains(crate::flags::Flag::Wizard))
    }
}
