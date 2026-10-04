//! Incoming damage accounting shared by unit anatomy and packet attribution.
use crate::{ObjectId, World};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Signed reference fields retain independently editable totals, not remaining-material deltas.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct DamageCounters {
    pub taken: i32,
    pub inflicted: i32,
}

/// Read statistics without changing material, contacts or randomness.
pub(super) fn read(world: &World, id: ObjectId) -> Result<DamageCounters> {
    let unit = world.btech.unit(id).context("Unit is unavailable")?;
    Ok(unit.damage_counters())
}

/// Select only the owning construction store for a validated update.
fn storage(world: &mut World, id: ObjectId) -> Result<&mut DamageCounters> {
    if world.btech.constructed_units().contains_key(&id) {
        return Ok(&mut world
            .btech
            .constructed
            .get_mut(&id)
            .unwrap()
            .damage_counters);
    }
    Ok(&mut world
        .btech
        .vehicles
        .get_mut(&id)
        .context("Unit is unavailable")?
        .damage_counters)
}

/// Count one admitted initial packet; transfers and interception are filtered by damage owners.
/// Prepare both totals before mutating either participant, including signed-counter overflow.
pub(super) fn record(
    world: &mut World,
    target: ObjectId,
    attacker: Option<ObjectId>,
    amount: u32,
) -> Result<()> {
    let amount = i32::try_from(amount).context("Damage counter overflow")?;
    let mut received = read(world, target)?;
    received.taken = received
        .taken
        .checked_add(amount)
        .context("Damage counter overflow")?;
    let inflicted = attacker
        .filter(|id| *id != target)
        .map(|id| {
            let mut counters = read(world, id)?;
            counters.inflicted = counters
                .inflicted
                .checked_add(amount)
                .context("Damage counter overflow")?;
            Ok::<_, anyhow::Error>((id, counters))
        })
        .transpose()?;
    *storage(world, target)? = received;
    if let Some((id, counters)) = inflicted {
        *storage(world, id)? = counters;
    }
    Ok(())
}

/// Administrative edits retain the signed field contract without applying combat damage.
pub(super) fn set(world: &mut World, id: ObjectId, field: &str, value: &str) -> Result<()> {
    let value = value
        .trim()
        .parse::<i32>()
        .context("Expected a signed 32-bit integer")?;
    let counters = storage(world, id)?;
    match field {
        "damage_taken" => counters.taken = value,
        "damage_inflicted" => counters.inflicted = value,
        _ => unreachable!("validated damage counter field"),
    }
    Ok(())
}
