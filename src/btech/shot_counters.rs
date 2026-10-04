//! Unit-owned shot statistics, shared by Mech and vehicle firing transactions.
use crate::{ObjectId, World};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Reference integer fields remain independent: administrative writes need not balance totals.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ShotCounters {
    pub fired: i32,
    pub hit: i32,
    pub missed: i32,
}

/// Inspect the common counters without acquiring contacts or advancing simulation state.
pub(super) fn read(world: &World, id: ObjectId) -> Result<ShotCounters> {
    let unit = world.btech.unit(id).context("Unit is unavailable")?;
    Ok(unit.shot_counters())
}

/// Borrow the owning storage only after the caller has validated a complete update.
fn storage(world: &mut World, id: ObjectId) -> Result<&mut ShotCounters> {
    if let Some(unit) = world.btech.constructed.get_mut(&id) {
        return Ok(&mut unit.shot_counters);
    }
    Ok(&mut world
        .btech
        .vehicles
        .get_mut(&id)
        .context("Unit is unavailable")?
        .shot_counters)
}

/// Broadcast classification includes the configured near-miss band even for beacon weapons.
pub(super) fn hit(roll: u8, target: Option<i32>, mode: super::BattleGlancingMode) -> bool {
    target.is_some_and(|number| {
        i64::from(roll)
            >= i64::from(number) - i64::from(mode == super::BattleGlancingMode::BelowTarget)
    })
}

/// Count a launched direct-unit attack once, before missile grouping or interception.
/// Coordinate fire and failed launch attempts never enter the reference shot counters.
pub(super) fn record(
    world: &mut World,
    id: ObjectId,
    launched: bool,
    coordinate: bool,
    hit: bool,
) -> Result<()> {
    if !launched || coordinate {
        return Ok(());
    }
    let mut counters = read(world, id)?;
    counters.fired = counters
        .fired
        .checked_add(1)
        .context("Shot counter overflow")?;
    let result = if hit {
        &mut counters.hit
    } else {
        &mut counters.missed
    };
    *result = result.checked_add(1).context("Shot counter overflow")?;
    *storage(world, id)? = counters;
    Ok(())
}

/// Administrative values use signed 32-bit storage and preserve the other counters.
pub(super) fn set(world: &mut World, id: ObjectId, field: &str, value: &str) -> Result<()> {
    let value = value
        .trim()
        .parse::<i32>()
        .context("Expected a signed 32-bit integer")?;
    let counters = storage(world, id)?;
    match field {
        "shots_fired" => counters.fired = value,
        "shots_hit" => counters.hit = value,
        "shots_missed" => counters.missed = value,
        _ => unreachable!("validated shot counter field"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The broadcast's hit band is independent of the missile/beacon damage admission band.
    #[test]
    fn broadcast_result_keeps_near_misses_and_range_separate() {
        use super::super::BattleGlancingMode::{AtTarget, BelowTarget, Disabled};
        for mode in [Disabled, AtTarget, BelowTarget] {
            assert!(!hit(12, None, mode));
            assert!(!hit(5, Some(7), mode));
            assert_eq!(hit(6, Some(7), mode), mode == BelowTarget);
            assert!(hit(7, Some(7), mode));
        }
    }
}
