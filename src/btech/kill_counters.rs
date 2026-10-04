//! Shared kill attribution at the material or crew event that first destroys a unit.
use crate::{ObjectId, World};
use anyhow::{Context, Result};

/// Inspect the unit-owned signed total without changing combat state.
pub(super) fn read(world: &World, id: ObjectId) -> Result<i32> {
    let unit = world.btech.unit(id).context("Unit is unavailable")?;
    Ok(unit.units_killed())
}

/// Select the owning chassis store, without keeping a second attribution ledger.
fn storage(world: &mut World, id: ObjectId) -> Result<&mut i32> {
    if world.btech.constructed_units().contains_key(&id) {
        return Ok(&mut world.btech.constructed.get_mut(&id).unwrap().units_killed);
    }
    Ok(&mut world
        .btech
        .vehicles
        .get_mut(&id)
        .context("Unit is unavailable")?
        .units_killed)
}

/// Record an immediate alive-to-destroyed transition before resolving secondary effects.
/// Nested owners sample their own transition; a dead target cannot award another kill.
/// The enclosing candidate transaction must discard material changes on overflow.
pub(super) fn transition(
    world: &mut World,
    target: ObjectId,
    attacker: Option<ObjectId>,
    was_destroyed: bool,
    destroyed: bool,
) -> Result<()> {
    let Some(attacker) = attacker.filter(|attacker| *attacker != target) else {
        return Ok(());
    };
    if was_destroyed || !destroyed {
        return Ok(());
    }
    let next = read(world, attacker)?
        .checked_add(1)
        .context("Kill counter overflow")?;
    *storage(world, attacker)? = next;
    Ok(())
}

/// Signed administrative edits do not create or reverse destruction events.
pub(super) fn set(world: &mut World, id: ObjectId, value: &str) -> Result<()> {
    let value = value
        .trim()
        .parse::<i32>()
        .context("Expected a signed 32-bit integer")?;
    *storage(world, id)? = value;
    Ok(())
}
