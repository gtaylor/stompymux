//! Shared loose-stock identity and mass lookup, separate from installed equipment admission.
use super::{BattleInventoryEntry, BattleWeapon};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

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
        if (193..385).contains(&id) {
            let weapon = Self::from_id(id - 192)?;
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

    /// Resolve an exact, ASCII case-insensitive stock name; manufacturer selection is separate.
    pub fn parse(name: &str) -> Result<Self> {
        let mut matches = (1..=super::parts_catalogue::STOCK
            .last()
            .map_or(0, |entry| entry.0))
            .filter_map(Self::from_id)
            .filter(|part| part.name.eq_ignore_ascii_case(name));
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
    fn loose_mass(&self) -> u64 {
        u64::from(self.mass)
            * if self.kind == BattlePartKind::Bomb {
                4
            } else {
                1
            }
    }
}

/// Sum quantities with checked arithmetic, retaining integral mass until unit-load adjustment.
pub(super) fn entries_mass(entries: &[BattleInventoryEntry]) -> Result<u64> {
    entries.iter().try_fold(0_u64, |total, entry| {
        entry.validate()?;
        let part = BattlePart::from_id(entry.part_id)
            .with_context(|| format!("Unknown inventory part {}", entry.part_id))?;
        total
            .checked_add(
                part.loose_mass()
                    .checked_mul(entry.quantity as u64)
                    .context("Inventory mass overflow")?,
            )
            .context("Inventory mass overflow")
    })
}

/// Physical stock mass shared by every supported carrying chassis, before cargo-technology bonuses.
pub fn inventory_mass(world: &World, object: ObjectId) -> Result<u64> {
    entries_mass(super::inventory(world, object)?)
}

/// Wizard stock correction by catalogue name; the numeric setter owns validation and transactions.
pub fn set_inventory_named(
    world: &mut World,
    actor: ObjectId,
    object: ObjectId,
    name: &str,
    brand: u8,
    quantity: i32,
) -> Result<()> {
    let part = BattlePart::parse(name)?;
    super::set_inventory_quantity(world, actor, object, part.part_id, brand, quantity)
}
