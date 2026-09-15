//! Shared external tow relationships, independent of chassis and container cargo.
use crate::{Flag, Kind, ObjectId, World};
use anyhow::{Context, Result, ensure};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

impl super::BtechState {
    /// One target per carrier; reverse ownership is derived rather than stored twice.
    pub fn tows(&self) -> &BTreeMap<ObjectId, ObjectId> {
        &self.tows
    }

    /// Find the unique carrier of a unit without a second mutable index.
    pub fn towed_by(&self, target: ObjectId) -> Option<ObjectId> {
        self.tows
            .iter()
            .find_map(|(&carrier, &unit)| (unit == target).then_some(carrier))
    }
}

/// Locate a constructed participant; tombstones remain valid until database cleanup.
fn position(world: &World, id: ObjectId) -> Result<super::BattlePosition> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| object.kind == Kind::Thing),
        "Tow participant is unavailable"
    );
    super::scanner::scanner_unit(world, id)
        .and_then(|unit| unit.position)
        .context("Tow participant must be placed on a battlefield")
}

/// Set a prepared tow pair or detach it; pickup authorization and target preparation belong to the host.
/// The target must already be powered down, and attachment requires the same hex.
/// This establishes a relationship only; it does not move units or apply pickup consequences.
pub fn set_tow(world: &mut World, carrier: ObjectId, target: Option<ObjectId>) -> Result<()> {
    let source = position(world, carrier)?;
    let Some(target) = target else {
        let mut candidate = world.clone();
        detach(&mut candidate, carrier);
        candidate.btech.validate(&candidate)?;
        *world = candidate;
        return Ok(());
    };
    for id in [carrier, target] {
        ensure!(
            world
                .objects
                .get(&id)
                .is_some_and(|object| !object.flags.contains(Flag::Going)),
            "Tow participant is unavailable"
        );
    }
    ensure!(carrier != target, "A unit cannot tow itself");
    ensure!(
        position(world, target)? == source,
        "Tow participants must be in the same hex"
    );
    ensure!(
        !world.btech.tows.contains_key(&carrier),
        "Unit already carries a tow"
    );
    ensure!(
        !world.btech.tows.contains_key(&target)
            && world.btech.towed_by(carrier).is_none()
            && world.btech.towed_by(target).is_none(),
        "Tow pairs cannot overlap or form chains"
    );
    ensure!(
        super::scanner::scanner_unit(world, target).unwrap().power == super::BattlePower::Off,
        "Power down the tow target first"
    );
    if let Some(unit) = world.btech.constructed_units().get(&target) {
        ensure!(
            !unit.airborne() && (unit.free_fall().is_none() && unit.orbital_drop().is_none()),
            "Land the tow target first"
        );
    }
    if let Some(unit) = world.btech.vehicles().get(&target) {
        ensure!(
            (unit.free_fall().is_none() && unit.orbital_drop().is_none())
                && unit
                    .vtol_flight()
                    .is_none_or(|flight| flight.phase == super::BattleVtolFlightPhase::Landed),
            "Land the tow target first"
        );
    }
    super::fortification::require_mobile(world, carrier)?;
    super::fortification::require_mobile(world, target)?;
    require_uncovered_target(world, target)?;
    Arc::make_mut(&mut world.btech.tows).insert(carrier, target);
    Ok(())
}

/// Saved relationships form disjoint pairs on one battlefield; each target is powered down.
/// Coordinate following is a movement operation, not a second persisted relationship.
pub(super) fn validate(world: &World) -> Result<()> {
    ensure!(
        world.btech.tows.len() <= 1_000_000,
        "Too many tow relationships"
    );
    let mut participants = BTreeSet::new();
    for (&carrier, &target) in world.btech.tows.iter() {
        super::fortification::require_mobile(world, carrier)?;
        super::fortification::require_mobile(world, target)?;
        require_uncovered_target(world, target)?;
        ensure!(
            participants.insert(carrier) && participants.insert(target),
            "Tow pairs cannot overlap or form chains"
        );
        ensure!(
            position(world, carrier)?.map == position(world, target)?.map,
            "Tow participants must share a battlefield"
        );
        ensure!(
            super::scanner::scanner_unit(world, target).unwrap().power == super::BattlePower::Off,
            "Tow target must be powered down"
        );
    }
    Ok(())
}

/// Administrative movement requires releasing either end of an existing tow first.
pub(super) fn require_detached(world: &World, id: ObjectId) -> Result<()> {
    ensure!(
        !world.btech.tows.contains_key(&id) && world.btech.towed_by(id).is_none(),
        "Detach tow cables before administrative placement"
    );
    Ok(())
}

/// Mirror carried position and facing after movement without changing slots or engine controls.
/// The caller owns the world candidate; any failed projection must discard that candidate.
/// Actual speed follows the carrier while desired speed remains zero.
pub(super) fn synchronize(world: &mut World) -> Result<()> {
    let pairs: Vec<_> = world.btech.tows.iter().map(|(&a, &b)| (a, b)).collect();
    for (carrier, target) in pairs {
        synchronize_pair(world, carrier, target)?;
    }
    Ok(())
}

/// Mirror one relationship after a paired transfer or ordinary movement.
pub(super) fn synchronize_pair(
    world: &mut World,
    carrier: ObjectId,
    target: ObjectId,
) -> Result<()> {
    // An interrupted Mech transition has not committed its tactical hex yet.
    if world
        .btech
        .constructed_units()
        .get(&carrier)
        .is_some_and(|unit| unit.hex_sync_pending())
    {
        return Ok(());
    }
    let position = position(world, carrier)?;
    ensure!(
        self::position(world, target)?.map == position.map,
        "Tow participants must share a battlefield"
    );
    let tile = world
        .btech
        .maps()
        .get(&position.map)
        .context("Tow battlefield is unavailable")?
        .hex(i64::from(position.x), i64::from(position.y))?;
    let (motion, height) = if let Some(unit) = world.btech.vehicles().get(&carrier) {
        (
            unit.motion().context("Carrier motion is unavailable")?,
            unit.altitude(tile),
        )
    } else {
        let unit = &world.btech.constructed_units()[&carrier];
        (
            unit.motion().context("Carrier motion is unavailable")?,
            unit.altitude(tile),
        )
    };
    let carried = super::BattleMotion {
        point: motion.point,
        heading: motion.heading,
        desired_heading: motion.heading,
        speed: motion.speed,
        desired_speed: 0.0,
    };
    let identity = if let Some(unit) = Arc::make_mut(&mut world.btech.vehicles).get_mut(&target) {
        unit.update_motion(carried, position, false);
        if let Some(flight) = &mut unit.vtol_flight {
            flight.altitude = height;
            unit.ground_elevation = None;
        } else {
            unit.ground_elevation = Some(height);
        }
        unit.identity()
    } else {
        let unit = Arc::make_mut(&mut world.btech.constructed)
            .get_mut(&target)
            .context("Tow target is unavailable")?;
        unit.motion = Some(carried);
        unit.position = Some(position);
        unit.ground_elevation = Some(height);
        unit.hex_sync_pending = false;
        unit.identity()
    };
    Arc::make_mut(&mut world.btech.units).insert(target, identity);
    Ok(())
}

/// Remove ownership and external translation on an unpublished release candidate.
pub(super) fn detach(world: &mut World, carrier: ObjectId) -> Option<ObjectId> {
    let target = Arc::make_mut(&mut world.btech.tows).remove(&carrier)?;
    let motion = if let Some(unit) = Arc::make_mut(&mut world.btech.vehicles).get_mut(&target) {
        unit.motion.as_mut()
    } else {
        Arc::make_mut(&mut world.btech.constructed)
            .get_mut(&target)
            .and_then(|unit| unit.motion.as_mut())
    };
    if let Some(motion) = motion {
        motion.stop_translation();
        motion.desired_heading = motion.heading;
    }
    Some(target)
}

/// Raw attachment and saved relationships require pickup to have cleared stationary cover.
fn require_uncovered_target(world: &World, id: ObjectId) -> Result<()> {
    let uncovered = world
        .btech
        .constructed_units()
        .get(&id)
        .is_none_or(|unit| unit.hull_down() == super::BattleHullDownState::default())
        && world
            .btech
            .vehicles()
            .get(&id)
            .is_none_or(|unit| unit.dig_state().exposed());
    ensure!(
        uncovered,
        "Prepare the target for pickup before attaching tow cables"
    );
    Ok(())
}
