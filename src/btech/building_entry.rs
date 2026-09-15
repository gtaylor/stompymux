//! Shared delayed building admission; the host evaluates locks and commits movement callbacks.
use super::{BattleHexCoordinate, BattlePosition, BattlePosture};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// An entry event saves the selector, not a destination that could become stale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleBuildingEntry {
    pub direction: Option<u8>,
    /// Zero means ready for the host to recheck locks and publish movement.
    pub remaining: u8,
}

impl BattleBuildingEntry {
    /// Bound saved work while allowing a ready event to survive transaction retries.
    pub(super) fn validate(self) -> Result<()> {
        ensure!(self.remaining <= 18, "Invalid building entry countdown");
        Ok(())
    }
}

/// Inspect a pending entry uniformly across the admitted chassis.
pub fn building_entry(world: &World, id: ObjectId) -> Option<BattleBuildingEntry> {
    world.btech.vehicles().get(&id).map_or_else(
        || {
            world
                .btech
                .constructed_units()
                .get(&id)
                .and_then(|unit| unit.building_entry)
        },
        |unit| unit.building_entry,
    )
}

/// Obtain the one saved event slot without duplicating scheduling or admission rules.
fn slot(world: &mut World, id: ObjectId) -> Result<&mut Option<BattleBuildingEntry>> {
    if world.btech.vehicles().contains_key(&id) {
        return Ok(&mut Arc::make_mut(&mut world.btech.vehicles)
            .get_mut(&id)
            .unwrap()
            .building_entry);
    }
    Ok(&mut Arc::make_mut(&mut world.btech.constructed)
        .get_mut(&id)
        .context("Unit is not constructed")?
        .building_entry)
}

/// Resolve current route and movement eligibility; call again when the delay expires.
/// The lock must be evaluated by the host against the returned interior map.
pub fn building_entry_destination_for_unit(
    world: &World,
    id: ObjectId,
    pilot: ObjectId,
    direction: Option<u8>,
) -> Result<BattlePosition> {
    destination_configured(
        world,
        id,
        pilot,
        direction,
        super::speed_bonus::SpeedPolicy::STANDARD,
    )
}

/// Host admission supplies the configured hot-myomer towing discount at every recheck.
pub(super) fn destination_configured(
    world: &World,
    id: ObjectId,
    pilot: ObjectId,
    direction: Option<u8>,
    policy: super::speed_bonus::SpeedPolicy,
) -> Result<BattlePosition> {
    super::targeting::controlled(world, id, pilot)?;
    super::fortification::require_mobile(world, id)?;
    use super::map_transfer::TransferMotionBlockage;
    let motion = super::map_transfer::transfer_motion_blockage(world, id, false)?;
    ensure!(
        motion != Some(TransferMotionBlockage::Jumping),
        "While in mid-jump? No way."
    );
    let (position, speed, crew_recovery) = if let Some(unit) = world.btech.vehicles().get(&id) {
        (unit.position(), unit.motion(), unit.crew_recovery.remaining)
    } else {
        let unit = &world.btech.constructed_units()[&id];
        (unit.position(), unit.motion(), unit.crew_recovery.remaining)
    };
    ensure!(crew_recovery == 0, "You are unconscious");
    if let Some(unit) = world.btech.constructed_units().get(&id) {
        ensure!(
            unit.posture == BattlePosture::Standing && unit.stand_timer.is_none(),
            "Crawl inside? I think not. Stand first."
        );
    }
    ensure!(
        motion != Some(TransferMotionBlockage::Uncontrolled),
        "While in mid-flight? No way."
    );
    if let Some(unit) = world.btech.vehicles().get(&id) {
        ensure!(
            unit.vtol_fuel().is_none_or(|fuel| fuel.remaining() > 0),
            "You lack fuel to maneuver in!"
        );
    }
    ensure!(
        motion != Some(TransferMotionBlockage::AirborneVtol),
        "You need to land before you can enter the hangar."
    );
    let position = position.context("Place the unit on a battlefield first")?;
    let speed = speed.context("Placed unit has no motion")?.speed.abs();
    let maximum = super::effective_speed::configured(world, id, policy)?;
    ensure!(
        maximum.abs() < 10.75 || speed * 5.0 < maximum,
        "You are moving too fast to enter the hangar!"
    );
    super::building_entry_destination(
        world,
        position.map,
        BattleHexCoordinate {
            x: i32::from(position.x),
            y: i32::from(position.y),
        },
        direction,
    )
}

/// A denied lock can be forced only on an unsafe structure below half integrity.
/// Integer division matters for odd maximum integrity and for unconfigured buildings.
pub fn building_entry_lock_allows(
    world: &World,
    interior: ObjectId,
    lock_passed: bool,
) -> Result<bool> {
    let building = world
        .btech
        .maps()
        .get(&interior)
        .context("Building map is unavailable")?
        .building;
    Ok(lock_passed || (!building.is_safe() && building.integrity < building.maximum_integrity / 2))
}

/// Schedule the shared eighteen-second event after a host-provided lock decision.
/// Admission does not move the unit or reserve destination slots.
pub fn begin_building_entry(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    direction: Option<u8>,
    lock_passed: bool,
) -> Result<BattlePosition> {
    begin_configured(
        world,
        id,
        pilot,
        direction,
        lock_passed,
        super::speed_bonus::SpeedPolicy::STANDARD,
    )
}

/// Schedule entry with the same configured admission used by the host's lock rechecks.
pub(super) fn begin_configured(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    direction: Option<u8>,
    lock_passed: bool,
    policy: super::speed_bonus::SpeedPolicy,
) -> Result<BattlePosition> {
    let destination = destination_configured(world, id, pilot, direction, policy)?;
    ensure!(
        building_entry_lock_allows(world, destination.map, lock_passed)?,
        "The hangar is locked."
    );
    ensure!(
        building_entry(world, id).is_none(),
        "You are already entering the hangar!"
    );
    *slot(world, id)? = Some(BattleBuildingEntry {
        direction: direction
            .filter(|value| *value != 0)
            .map(|value| value.to_ascii_lowercase()),
        remaining: 18,
    });
    Ok(destination)
}

/// Advance one committed second. Ready events remain saved until the host consumes them.
/// This is separate from the ordinary power clock so readiness cannot silently move objects.
pub fn advance_building_entry(
    world: &mut World,
    id: ObjectId,
) -> Result<Option<BattleBuildingEntry>> {
    let event = slot(world, id)?;
    if let Some(event) = event {
        event.remaining = event.remaining.saturating_sub(1);
    }
    Ok(event.filter(|event| event.remaining == 0))
}

/// Consume or cancel an event in the host's movement transaction, including failed rechecks.
pub fn clear_building_entry(world: &mut World, id: ObjectId) -> Result<()> {
    *slot(world, id)? = None;
    Ok(())
}
