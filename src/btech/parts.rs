//! Loose stock carried in inventories: physical mass and named stock corrections.
use super::{BattleInventoryEntry, BattlePart};
use crate::{ObjectId, World};
use anyhow::{Context, Result};

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
    quantity: i32,
) -> Result<()> {
    let part = BattlePart::parse(name)?;
    super::set_inventory_quantity(world, actor, object, part.part_id, quantity)
}
