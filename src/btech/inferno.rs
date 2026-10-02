//! Persisted inferno duration, extended by hits and aged only by committed simulation ticks.
use super::{BattleNotice, BattleUnit};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};

impl BattleUnit {
    /// Remaining inferno seconds; positive duration suppresses six points of heat dissipation.
    pub fn inferno_remaining(&self) -> u32 {
        self.inferno_remaining
    }
}

/// Apply an admitted burn without consuming dice or immediately changing stored heat.
/// The caller owns attack authorization and the enclosing damage/publication checkpoint.
pub fn apply_inferno_burn(world: &mut World, id: ObjectId, seconds: u32) -> Result<()> {
    ensure!(seconds > 0, "Inferno duration must be positive");
    adjust_burn(world, id, i64::from(seconds))
}

/// Signed exposures reschedule an active burn, retaining at least one committed tick.
pub(super) fn adjusted_duration(remaining: u32, seconds: i64) -> Result<u32> {
    if seconds == 0 {
        return Ok(remaining);
    }
    let duration = (i64::from(remaining) + seconds).max(1);
    ensure!(
        duration <= i64::from(i32::MAX),
        "Inferno duration exceeds its limit"
    );
    Ok(duration as u32)
}

/// Apply signed blast heat using the same timer accounting as ordinary inferno hits.
pub(super) fn adjust_burn(world: &mut World, id: ObjectId, seconds: i64) -> Result<()> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Unit is unavailable"
    );
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit is not constructed")?;
    let remaining = adjusted_duration(unit.inferno_remaining, seconds)?;
    world
        .btech
        .constructed
        .get_mut(&id)
        .unwrap()
        .inferno_remaining = remaining;
    Ok(())
}

/// Advance after the thermal sample so every saved burn second supplies its cooling penalty.
pub fn advance_inferno_burns(world: &mut World) -> Vec<BattleNotice> {
    let ids: Vec<_> = world
        .btech
        .constructed_units()
        .iter()
        .filter(|(id, unit)| {
            unit.inferno_remaining > 0
                && world
                    .objects
                    .get(id)
                    .is_some_and(|object| !object.flags.contains(Flag::Going))
        })
        .map(|(&id, _)| id)
        .collect();
    let mut notices = Vec::new();
    for id in ids {
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        unit.inferno_remaining -= 1;
        if unit.inferno_remaining == 0 {
            notices.push(BattleNotice {
                unit: id,
                text: "You feel suddenly far cooler as the fires finally die.".into(),
            });
        }
    }
    notices
}

/// Extinguish during an immersion event, replacing surface decoration with two minutes of steam.
/// A standing unit in depth-one water remains burning; the movement/fall caller owns event admission.
pub fn extinguish_inferno_in_water(world: &mut World, id: ObjectId) -> Result<Vec<BattleNotice>> {
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit is not constructed")?;
    if unit.inferno_remaining == 0 {
        return Ok(Vec::new());
    }
    let Some(position) = unit.position() else {
        return Ok(Vec::new());
    };
    let map = world
        .btech
        .maps()
        .get(&position.map)
        .context("Map is unavailable")?;
    let tile = map.base_hex(i64::from(position.x), i64::from(position.y))?;
    if !tile.is_open_water()
        || (unit.elevation_level(tile) == i32::from(tile.water_line()) - 1
            && unit.posture() != super::BattlePosture::Prone)
    {
        return Ok(Vec::new());
    }
    let mut notices = vec![BattleNotice {
        unit: id,
        text: "The flames extinguish in a roar of steam!".into(),
    }];
    notices.extend(super::broadcast::observer_notices(
        world,
        id,
        "is surrounded by a plume of steam as the flames extinguish.",
    ));
    world.attempt(|world| {
        world
            .btech
            .constructed
            .get_mut(&id)
            .unwrap()
            .inferno_remaining = 0;
        super::set_map_decoration(
            world,
            position.map,
            super::BattleHexCoordinate {
                x: i32::from(position.x),
                y: i32::from(position.y),
            },
            Some(super::BattleDecoration::new(
                super::BattleDecorationKind::Smoke,
                120,
                None,
            )),
        )?;
        world.btech.validate_action(world)?;
        Ok(notices)
    })
}
