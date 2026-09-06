//! Typed legacy powers, persisted as metadata without gameplay effects.
use crate::world::{Kind, ObjectId, World};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The complete powers catalog in this fork of btmux-khi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Power {
    /// Reserved idle-timeout exemption; currently metadata only.
    Idle,
}
/// Catalog order is also the legacy display order.
pub const ALL: [Power; 1] = [Power::Idle];
impl Power {
    /// Canonical name used by storage and Lua constants.
    pub fn name(self) -> &'static str {
        match self {
            Self::Idle => "IDLE",
        }
    }
    /// Legacy player-facing spelling.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
        }
    }
    /// Resolve a case-insensitive catalog name without accepting unknown powers.
    pub fn parse(name: &str) -> Result<Self> {
        ALL.into_iter()
            .find(|p| p.name().eq_ignore_ascii_case(name))
            .ok_or_else(|| anyhow::anyhow!("unknown power {name}"))
    }
}
/// Durable typed powers with compatible uppercase JSON name arrays.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PowerSet(BTreeSet<Power>);
impl PowerSet {
    /// Query membership without applying a gameplay effect.
    pub fn contains(&self, power: Power) -> bool {
        self.0.contains(&power)
    }
    /// Grant a power and report whether it changed.
    pub fn insert(&mut self, power: Power) -> bool {
        self.0.insert(power)
    }
    /// Remove a power and report whether it changed.
    pub fn remove(&mut self, power: Power) -> bool {
        self.0.remove(&power)
    }
    /// Describe granted powers in legacy catalog order.
    pub fn description(&self) -> String {
        ALL.into_iter()
            .filter(|p| self.contains(*p))
            .map(Power::display_name)
            .collect::<Vec<_>>()
            .join(" ")
    }
}
impl Serialize for PowerSet {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        ALL.into_iter()
            .filter(|p| self.contains(*p))
            .map(Power::name)
            .collect::<Vec<_>>()
            .serialize(s)
    }
}
impl<'de> Deserialize<'de> for PowerSet {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let mut powers = Self::default();
        for name in Vec::<String>::deserialize(d)? {
            powers.insert(Power::parse(&name).map_err(serde::de::Error::custom)?);
        }
        Ok(powers)
    }
}
/// Mutate a live object after checking command authority; trusted Lua acts as GOD.
pub fn change(
    world: &mut World,
    actor: ObjectId,
    target: ObjectId,
    power: Power,
    value: bool,
) -> Result<bool> {
    ensure!(
        crate::flags::controls(world, actor, target),
        "Permission denied."
    );
    let object = world
        .objects
        .get_mut(&target)
        .filter(|o| o.kind != Kind::Garbage)
        .ok_or_else(|| anyhow::anyhow!("object does not exist"))?;
    Ok(if value {
        object.powers.insert(power)
    } else {
        object.powers.remove(power)
    })
}
