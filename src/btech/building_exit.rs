//! Shared building-exit placement and rotorcraft continuation, independent of host callbacks.
use super::{BattlePosition, BattleVtolFlightPhase};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Resolve the first reciprocal exit and reject structures whose entrance remains rubble.
pub fn building_exit_for_unit(world: &World, id: ObjectId) -> Result<BattlePosition> {
    super::map_transfer::validate_transfer(world, id, true)?;
    let position = super::scanner::scanner_unit(world, id)
        .and_then(|unit| unit.position)
        .context("Unit is not placed")?;
    let map = world
        .btech
        .maps()
        .get(&position.map)
        .context("Interior map is unavailable")?;
    ensure!(
        map.building.integrity > 0,
        "The entrance is still filled with rubble!"
    );
    super::building_exit_destination(world, position.map)
}

/// Transfer through the current building return route, preserving crew and requested controls.
/// VTOLs continue airborne one level above the exterior surface with their vertical speed intact.
/// The host must apply teleport policies, callbacks and observer publication.
pub fn exit_building(world: &mut World, id: ObjectId) -> Result<BattlePosition> {
    exit_configured(world, id, super::speed_bonus::SpeedPolicy::STANDARD)
}

/// The host supplies its load configuration when reconciling the exterior speed.
pub(super) fn exit_configured(
    world: &mut World,
    id: ObjectId,
    policy: super::speed_bonus::SpeedPolicy,
) -> Result<BattlePosition> {
    let destination = building_exit_for_unit(world, id)?;
    let vertical_speed = world
        .btech
        .vehicles()
        .get(&id)
        .and_then(|unit| unit.vtol_flight())
        .map(|flight| flight.vertical_speed);
    world.attempt(|world| {
        super::map_transfer::place_transfer(world, id, destination)?;
        let maximum = super::effective_speed::configured(world, id, policy)?;
        let motion = if let Some(unit) = world.btech.vehicles.get_mut(&id) {
            if let Some(flight) = &mut unit.vtol_flight {
                flight.phase = BattleVtolFlightPhase::Airborne;
                flight.altitude += 1.0;
                flight.vertical_speed = vertical_speed.expect("VTOL flight state");
            }
            &mut unit.motion
        } else {
            &mut world.btech.constructed.get_mut(&id).unwrap().motion
        };
        let motion = motion.as_mut().expect("placed motion");
        motion.speed = motion.speed.min(maximum);
        if let Some(target) = world.btech.tows().get(&id).copied() {
            super::towing::synchronize_pair(world, id, target)?;
        }
        world.btech.validate_action(world)?;
        Ok(destination)
    })
}
