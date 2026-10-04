//! Shared loose-stock identity and mass lookup, separate from installed equipment admission.
use super::BattleWeapon;
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::ops::RangeInclusive;

/// Weapon identities, including raw personal and infantry weapons. The block ends where bombs,
/// components and commodities begin.
pub const WEAPON_PART_IDS: RangeInclusive<i32> = 1..=384;

/// A weapon's loose ammunition identity sits this far above the weapon's own, past every
/// bomb, component and commodity identity.
pub const AMMUNITION_PART_OFFSET: i32 = 1024;

/// Every part identity is below this bound.
pub const PART_ID_LIMIT: i32 = 2048;

impl BattleWeapon {
    /// Stock identity of this weapon's loose ammunition.
    pub fn ammunition_part_id(self) -> i32 {
        self.part_id() + AMMUNITION_PART_OFFSET
    }
}

/// Physical stock category; possessing an item does not enable its combat subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattlePartKind {
    Weapon,
    Ammunition,
    Component,
    Commodity,
    Bomb,
}

/// Catalogue description of one stable game-directory inventory identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattlePart {
    pub part_id: i32,
    pub name: String,
    pub kind: BattlePartKind,
    /// Catalogue mass in 1/1024-ton units; loose bombs use four times this mass.
    pub mass: u32,
}

impl BattlePart {
    /// Decode stock without interpreting the Rust weapon enum's declaration order.
    pub fn from_id(id: i32) -> Option<Self> {
        if let Some(weapon) = BattleWeapon::from_part_id(id) {
            return Some(Self {
                part_id: id,
                name: weapon.name().into(),
                kind: BattlePartKind::Weapon,
                mass: weapon.mass(),
            });
        }
        if let Some(weapon) = Self::ammunition_weapon_id(id) {
            let weapon =
                Self::from_id(weapon).filter(|part| part.kind == BattlePartKind::Weapon)?;
            return Some(Self {
                part_id: id,
                name: format!("Ammo_{}", weapon.name),
                kind: BattlePartKind::Ammunition,
                mass: 1024,
            });
        }
        let index = super::parts_catalogue::STOCK
            .binary_search_by_key(&id, |entry| entry.0)
            .ok()?;
        let (_, name, mass, kind) = super::parts_catalogue::STOCK[index];
        Some(Self {
            part_id: id,
            name: name.into(),
            kind,
            mass,
        })
    }

    /// The weapon identity whose loose ammunition `id` names, if it names ammunition.
    pub fn ammunition_weapon_id(id: i32) -> Option<i32> {
        let weapon = id - AMMUNITION_PART_OFFSET;
        WEAPON_PART_IDS.contains(&weapon).then_some(weapon)
    }

    /// Every catalogued part in identity order.
    pub fn all() -> impl Iterator<Item = Self> {
        (1..PART_ID_LIMIT).filter_map(Self::from_id)
    }

    /// Resolve an exact, ASCII case-insensitive stock name.
    pub fn parse(name: &str) -> Result<Self> {
        let mut matches = Self::all().filter(|part| part.name.eq_ignore_ascii_case(name));
        let part = matches
            .next()
            .with_context(|| format!("Unknown inventory part {name}"))?;
        ensure!(
            matches.next().is_none(),
            "Ambiguous inventory part {name}; use its numeric identifier"
        );
        Ok(part)
    }

    /// Effective mass of one loose item, including the distinct bomb inventory multiplier.
    pub fn loose_mass(&self) -> u64 {
        u64::from(self.mass)
            * if self.kind == BattlePartKind::Bomb {
                4
            } else {
                1
            }
    }
}
