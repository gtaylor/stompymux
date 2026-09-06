//! Canonical MUX flags, typed object storage and mutation policy.
use crate::world::{Kind, ObjectId, World};
use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Known object flags accepted by configuration.
pub enum Flag {
    Ansi,
    Audible,
    Auditorium,
    Blind,
    Connected,
    Dark,
    Floating,
    Gagged,
    Going,
    Halted,
    InCharacter,
    Light,
    Monitor,
    NoCommand,
    Safe,
    Suspect,
    Transparent,
    Wizard,
    Zombie,
}
/// Authority required by the catalog for manual mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationPolicy {
    /// Ordinary flag edits require Wizard authority.
    Wizard,
    /// Only GOD may change Wizard status, and never remove its own status.
    God,
    /// Clearing GOING on a non-player is allowed by the flag policy.
    Going,
    /// Only session reconciliation may change this flag.
    Session,
}
impl Flag {
    /// Return the canonical spelling used by world objects.
    pub fn world_name(&self) -> String {
        serde_json::to_value(self)
            .expect("flag serialization")
            .as_str()
            .unwrap()
            .to_ascii_uppercase()
    }
}
/// Catalog order is also the legacy display order.
pub const ALL: [Flag; 19] = [
    Flag::Ansi,
    Flag::Audible,
    Flag::Auditorium,
    Flag::Blind,
    Flag::Connected,
    Flag::Dark,
    Flag::Floating,
    Flag::Gagged,
    Flag::Going,
    Flag::Halted,
    Flag::InCharacter,
    Flag::Light,
    Flag::Monitor,
    Flag::NoCommand,
    Flag::Safe,
    Flag::Suspect,
    Flag::Transparent,
    Flag::Wizard,
    Flag::Zombie,
];
impl Flag {
    /// Mutation authority is catalog metadata shared by all callers.
    pub fn policy(self) -> MutationPolicy {
        match self {
            Self::Connected => MutationPolicy::Session,
            Self::Wizard => MutationPolicy::God,
            Self::Going => MutationPolicy::Going,
            _ => MutationPolicy::Wizard,
        }
    }
    /// Resolve canonical names without accepting arbitrary flag strings.
    pub fn parse(name: &str) -> Result<Self> {
        ALL.into_iter()
            .find(|f| f.world_name().eq_ignore_ascii_case(name))
            .ok_or_else(|| anyhow::anyhow!("unknown flag {name}"))
    }
    /// Resolve configured aliases as well as canonical names.
    pub fn resolve(name: &str, aliases: &BTreeMap<String, String>) -> Result<Self> {
        let target = aliases
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map_or(name, |(_, v)| v);
        Self::parse(target)
    }
    /// Legacy compact display character.
    pub fn letter(self) -> char {
        match self {
            Self::Ansi => 'X',
            Self::Audible => 'a',
            Self::Auditorium => 'b',
            Self::Blind => '(',
            Self::Connected => 'c',
            Self::Dark => 'D',
            Self::Floating => 'F',
            Self::Gagged => 'j',
            Self::Going => 'G',
            Self::Halted => 'h',
            Self::InCharacter => '#',
            Self::Light => 'l',
            Self::Monitor => 'M',
            Self::NoCommand => 'n',
            Self::Safe => 's',
            Self::Suspect => 'u',
            Self::Transparent => 't',
            Self::Wizard => 'W',
            Self::Zombie => 'z',
        }
    }
}
/// Typed flag set with backwards-compatible uppercase JSON name arrays.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlagSet(BTreeSet<Flag>);
impl FlagSet {
    /// Query a typed flag.
    pub fn contains(&self, flag: Flag) -> bool {
        self.0.contains(&flag)
    }
    /// Set a flag; session-owned flags must be changed only by the world owner.
    pub fn insert(&mut self, flag: Flag) -> bool {
        self.0.insert(flag)
    }
    /// Clear a flag.
    pub fn remove(&mut self, flag: Flag) -> bool {
        self.0.remove(&flag)
    }
    /// Canonical names in catalog order, including transient flags for Lua.
    pub fn names(&self) -> Vec<String> {
        ALL.into_iter()
            .filter(|f| self.contains(*f))
            .map(|f| f.world_name())
            .collect()
    }
}
impl FromIterator<Flag> for FlagSet {
    fn from_iter<T: IntoIterator<Item = Flag>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}
impl Serialize for FlagSet {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.names().serialize(s)
    }
}
impl<'de> Deserialize<'de> for FlagSet {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Vec::<String>::deserialize(d)?
            .iter()
            .map(|s| Flag::parse(s).map_err(serde::de::Error::custom))
            .collect()
    }
}
/// Legacy control policy, independent of deferred ownership/building systems.
pub fn controls(world: &World, actor: ObjectId, target: ObjectId) -> bool {
    let Some(t) = world
        .objects
        .get(&target)
        .filter(|o| o.kind != Kind::Garbage)
    else {
        return false;
    };
    let wizard = world
        .objects
        .get(&actor)
        .is_some_and(|o| o.flags.contains(Flag::Wizard));
    actor == ObjectId(1)
        || wizard && (actor == target || target != ObjectId(1) && !t.flags.contains(Flag::Wizard))
}
/// Apply flag-specific policy. Trusted Lua uses GOD as its authority.
pub fn change(
    world: &mut World,
    actor: ObjectId,
    target: ObjectId,
    flag: Flag,
    value: bool,
) -> Result<bool> {
    ensure!(
        flag.policy() != MutationPolicy::Session,
        "CONNECTED is managed by player sessions."
    );
    let god = actor == ObjectId(1);
    let wizard = world
        .objects
        .get(&actor)
        .is_some_and(|o| o.flags.contains(Flag::Wizard));
    let o = world
        .objects
        .get_mut(&target)
        .filter(|o| o.kind != Kind::Garbage)
        .ok_or_else(|| anyhow::anyhow!("object does not exist"))?;
    if flag.policy() == MutationPolicy::God {
        ensure!(god, "Permission denied.");
        ensure!(
            target != ObjectId(1) || value,
            "You cannot make yourself mortal."
        );
    } else if flag.policy() == MutationPolicy::Going {
        ensure!(
            god || !value && o.kind != Kind::Player && o.flags.contains(flag),
            "Permission denied."
        );
    } else if !god && !wizard {
        bail!("Permission denied.");
    }
    Ok(if value {
        o.flags.insert(flag)
    } else {
        o.flags.remove(flag)
    })
}
/// Immutable Lua catalog namespace; aliases are a command/configuration concern.
pub struct LuaFlags;
impl mlua::UserData for LuaFlags {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(mlua::MetaMethod::Index, |lua, _, key: String| {
            let flag = Flag::parse(&key).map_err(mlua::Error::external)?;
            if flag.world_name() != key {
                return Err(mlua::Error::external(
                    "flag constants require canonical uppercase names",
                ));
            }
            lua.create_userdata(flag)
        });
    }
}
impl mlua::UserData for Flag {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(mlua::MetaMethod::ToString, |_, flag, ()| {
            Ok(flag.world_name())
        });
        methods.add_meta_method(mlua::MetaMethod::Eq, |_, flag, other: mlua::AnyUserData| {
            Ok(other.borrow::<Flag>().is_ok_and(|other| *flag == *other))
        });
    }
}
