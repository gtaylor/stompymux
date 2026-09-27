//! Shared object-owned parts quantities, independent of unit anatomy and installed equipment.
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Stable game-directory part and manufacturer identifiers with a positive stored quantity.
/// These identify loose inventory; they never create or modify an installed critical slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleInventoryEntry {
    pub part_id: i32,
    pub brand_id: u8,
    pub quantity: i32,
}

impl BattleInventoryEntry {
    /// Stable ordering keeps snapshots and persisted manifests deterministic.
    pub(crate) fn key(self) -> (i32, u8) {
        (self.part_id, self.brand_id)
    }

    /// Validate storage independently of the eventual name and mass catalogue.
    pub(crate) fn validate(self) -> Result<()> {
        ensure!(self.part_id >= 0, "Invalid inventory part identifier");
        ensure!(
            self.brand_id <= 5,
            "Invalid inventory manufacturer identifier"
        );
        ensure!(self.quantity > 0, "Inventory quantity must be positive");
        Ok(())
    }
}

/// Inspect one object's detached-compatible ordered inventory without requiring a unit chassis.
pub fn inventory(world: &World, object: ObjectId) -> Result<&[BattleInventoryEntry]> {
    ensure!(
        world
            .objects
            .get(&object)
            .is_some_and(|owner| owner.kind != crate::Kind::Garbage),
        "Inventory requires a live object"
    );
    Ok(world
        .btech
        .inventories
        .get(&object)
        .map_or(&[], Vec::as_slice))
}

/// Wizard stock correction; zero removes a row. Cargo loading uses separate gameplay admission.
pub fn set_inventory_quantity(
    world: &mut World,
    actor: ObjectId,
    object: ObjectId,
    part_id: i32,
    brand_id: u8,
    quantity: i32,
) -> Result<()> {
    ensure!(
        crate::authority::is_wizard(world, actor),
        "Permission denied."
    );
    edit_quantity(world, object, part_id, brand_id, quantity)
}

/// Set a stock row after the caller has established authority; zero removes it.
pub fn set_part_store_quantity(
    world: &mut World,
    object: ObjectId,
    part_id: i32,
    brand_id: u8,
    quantity: i32,
) -> Result<()> {
    edit_quantity(world, object, part_id, brand_id, quantity)
}

pub fn part_cost(world: &World, part_id: i32) -> Result<u64> {
    ensure!(
        super::BattlePart::from_id(part_id).is_some(),
        "Unknown inventory part"
    );
    Ok(world.btech.part_costs.get(&part_id).copied().unwrap_or(0))
}

pub fn set_part_cost(world: &mut World, part_id: i32, cost: u64) -> Result<()> {
    ensure!(
        super::BattlePart::from_id(part_id).is_some(),
        "Unknown inventory part"
    );
    let costs = Arc::make_mut(&mut world.btech.part_costs);
    if cost == 0 {
        costs.remove(&part_id);
    } else {
        costs.insert(part_id, cost);
    }
    Ok(())
}

/// Wizard correction with immediate load reconciliation and transactional economy publication.
pub fn set_inventory_quantity_action(
    scripts: &crate::Scripts,
    config: &crate::Config,
    actor: ObjectId,
    object: ObjectId,
    part_id: i32,
    brand_id: u8,
    quantity: i32,
) -> Result<()> {
    scripts.atomic(|before| {
        set_inventory_quantity(
            &mut scripts.world_mut(),
            actor,
            object,
            part_id,
            brand_id,
            quantity,
        )?;
        let previous = inventory(before, object)?
            .iter()
            .find(|row| row.key() == (part_id, brand_id))
            .map_or(0, |row| row.quantity);
        let change = quantity - previous;
        if change == 0 {
            return Ok(());
        }
        if before.btech.constructed_units().contains_key(&object)
            || before.btech.vehicles().contains_key(&object)
        {
            super::load::reconcile(
                &mut scripts.world_mut(),
                object,
                config.battletech.tsm_tow_bonus != 0,
            )?;
        }
        let name = super::stock_selection::name(&BattleInventoryEntry {
            part_id,
            brand_id,
            quantity: 1,
        });
        let message = super::channels::stock_message(actor, object, &name, change);
        super::channels::publish(scripts, config, &[message])?;
        scripts.effects.validate()?;
        Ok(())
    })
}

/// Prepared gameplay candidates share the same stock invariants as Wizard corrections.
pub(super) fn edit_quantity(
    world: &mut World,
    object: ObjectId,
    part_id: i32,
    brand_id: u8,
    quantity: i32,
) -> Result<()> {
    inventory(world, object)?;
    ensure!(quantity >= 0, "Inventory quantity cannot be negative");
    let entry = BattleInventoryEntry {
        part_id,
        brand_id,
        quantity: quantity.max(1),
    };
    entry.validate()?;
    let mut rows = inventory(world, object)?.to_vec();
    match rows.binary_search_by_key(&entry.key(), |row| row.key()) {
        Ok(index) if quantity == 0 => {
            rows.remove(index);
        }
        Ok(index) => rows[index].quantity = quantity,
        Err(index) if quantity > 0 => rows.insert(index, entry),
        Err(_) => {}
    }
    if world.btech.constructed_units().contains_key(&object)
        || world.btech.vehicles().contains_key(&object)
    {
        super::parts::entries_mass(&rows)?;
    }
    let inventories = &mut world.btech.inventories;
    if rows.is_empty() {
        inventories.remove(&object);
    } else {
        inventories.insert(object, rows);
    }
    Ok(())
}

/// Economy deltas share zero-floor removal and generic actuator-stock accounting.
pub(super) fn change_quantity(
    world: &mut World,
    object: ObjectId,
    entry: BattleInventoryEntry,
    change: i32,
) -> Result<()> {
    let old = inventory(world, object)?
        .iter()
        .find(|row| row.key() == entry.key())
        .map_or(0, |row| row.quantity);
    let next = old
        .checked_add(change)
        .context("Inventory quantity overflow")?
        .max(0);
    if !(394..=397).contains(&entry.part_id) || next == 0 {
        return edit_quantity(world, object, entry.part_id, entry.brand_id, next);
    }
    let generic = inventory(world, object)?
        .iter()
        .find(|row| row.key() == (548, entry.brand_id))
        .map_or(0, |row| row.quantity);
    edit_quantity(
        world,
        object,
        548,
        entry.brand_id,
        generic
            .checked_add(next)
            .context("Inventory quantity overflow")?,
    )
}

/// Reject malformed snapshots before saving or using inventory in gameplay.
pub(super) fn validate(world: &World) -> Result<()> {
    for (&object, entries) in world.btech.inventories.iter() {
        inventory(world, object)?;
        if world.btech.constructed_units().contains_key(&object)
            || world.btech.vehicles().contains_key(&object)
        {
            super::parts::entries_mass(entries)?;
        }
        ensure!(!entries.is_empty(), "Empty inventory must not be stored");
        for entry in entries {
            entry.validate()?;
        }
        ensure!(
            entries.windows(2).all(|pair| pair[0].key() < pair[1].key()),
            "Inventory entries must be unique and ordered"
        );
    }
    Ok(())
}
