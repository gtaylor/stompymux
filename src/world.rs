use crate::config::Config;
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Scalar {
    Boolean(bool),
    Integer(i64),
    Number(f64),
    String(String),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Object {
    pub id: ObjectId,
    pub name: String,
    pub kind: Kind,
    pub location: Option<ObjectId>,
    pub zone: Option<ObjectId>,
    pub home: Option<ObjectId>,
    pub affiliation: Option<ObjectId>,
    pub destination: Option<ObjectId>,
    pub description: Option<String>,
    pub internal_description: Option<String>,
    pub lua_parent: String,
    pub flags: BTreeSet<String>,
    pub powers: BTreeSet<String>,
    pub state: BTreeMap<String, BTreeMap<String, Scalar>>,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Login {
    pub success: bool,
    pub at: i64,
    pub host: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub name: String,
    pub object: Option<ObjectId>,
    pub flags: i64,
    pub messages: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct World {
    pub objects: BTreeMap<ObjectId, Object>,
    pub accounts: BTreeMap<ObjectId, Account>,
    pub channels: BTreeMap<String, Channel>,
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
        let flags = flags.iter().map(|flag| flag.world_name()).collect();
        self.objects.insert(
            id,
            Object {
                id,
                name,
                kind,
                location: None,
                zone: None,
                home: None,
                affiliation: None,
                destination: None,
                description: None,
                internal_description: None,
                lua_parent: parent.clone(),
                flags,
                powers: BTreeSet::new(),
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
                    o.name.eq_ignore_ascii_case(name)
                        || a.alias
                            .as_ref()
                            .is_some_and(|s| s.eq_ignore_ascii_case(name))
                })
                .map(|_| *id)
        })
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
        let mut names = BTreeSet::new();
        for (id, a) in &self.accounts {
            let o = self
                .objects
                .get(id)
                .ok_or_else(|| anyhow::anyhow!("account object missing"))?;
            ensure!(o.kind == Kind::Player, "account is not a player");
            ensure!(
                names.insert(o.name.to_ascii_lowercase()),
                "duplicate player name"
            );
            if let Some(alias) = &a.alias
                && !alias.is_empty()
                && !alias.eq_ignore_ascii_case(&o.name)
            {
                ensure!(
                    names.insert(alias.to_ascii_lowercase()),
                    "duplicate player alias"
                );
            }
        }
        for o in self.objects.values().filter(|o| o.kind != Kind::Garbage) {
            for id in [o.location, o.home, o.destination, o.zone, o.affiliation]
                .into_iter()
                .flatten()
            {
                ensure!(
                    self.objects.contains_key(&id),
                    "{} references missing #{}",
                    o.name,
                    id.0
                );
            }
        }
        Ok(())
    }
    pub fn visible(&self, o: &Object, viewer: ObjectId) -> bool {
        !o.flags.contains("DARK")
            || o.id == viewer
            || self
                .objects
                .get(&viewer)
                .is_some_and(|p| p.flags.contains("WIZARD"))
    }
}
