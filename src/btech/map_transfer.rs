//! Shared battlefield placement commits map identity, membership and chassis-specific position state.
use super::*;
use crate::{Flag, Kind, ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Administrative relocation resets motion; a building transfer preserves running controls.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PlacementMode {
    Administrative,
    Transfer,
}

/// Move a running, grounded unit between battlefields without consuming dice or changing its crew.
/// The host owns route selection, locks, movement callbacks and publication.
/// Invalid placement or resulting world state leaves the original world unchanged.
pub fn transfer_unit(world: &mut World, id: ObjectId, position: BattlePosition) -> Result<()> {
    validate_transfer(world, id, false)?;
    world.attempt(|world| {
        place_transfer(world, id, position)?;
        world.btech.validate(world)?;
        Ok(())
    })
}

/// Move every member before checking relationship invariants; caller owns rollback.
pub(super) fn place_transfer(
    world: &mut World,
    id: ObjectId,
    position: BattlePosition,
) -> Result<()> {
    ensure!(
        world.btech.towed_by(id).is_none(),
        "A towed unit cannot transfer independently"
    );
    let target = world.btech.tows().get(&id).copied();
    place(world, id, position, PlacementMode::Transfer)?;
    if let Some(target) = target {
        place(world, target, position, PlacementMode::Transfer)?;
        super::towing::synchronize_pair(world, id, target)?;
    }
    Ok(())
}

/// Shared movement admission; exits also accept stable VTOL flight.
pub(super) fn validate_transfer(world: &World, id: ObjectId, airborne_vtol: bool) -> Result<()> {
    require_transfer_motion(world, id, airborne_vtol)?;
    if let Some(unit) = world.btech.vehicles().get(&id) {
        ensure!(
            unit.power() == BattlePower::Running && !unit.is_destroyed(),
            "Transfer requires a running unit"
        );
        ensure!(
            unit.position().is_some(),
            "Transfer requires battlefield placement"
        );
    } else {
        let unit = world
            .btech
            .constructed_units()
            .get(&id)
            .context("Unit is not constructed")?;
        ensure!(
            unit.power() == BattlePower::Running && !unit.is_destroyed(),
            "Transfer requires a running unit"
        );
        ensure!(
            unit.position().is_some(),
            "Transfer requires battlefield placement"
        );
        ensure!(
            unit.stand_timer.is_none(),
            "Finish standing before transferring between maps"
        );
    }
    Ok(())
}

/// Reject airborne transitions that placement would otherwise silently cancel.
/// Building exits may retain stable rotorcraft flight; all forced descents must finish first.
pub(super) fn require_transfer_motion(
    world: &World,
    id: ObjectId,
    airborne_vtol: bool,
) -> Result<()> {
    ensure!(
        transfer_motion_blockage(world, id, airborne_vtol)?.is_none(),
        "Finish airborne movement before transferring between maps"
    );
    Ok(())
}

/// One movement prohibition, shared by generic transfers and cockpit-specific refusals.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TransferMotionBlockage {
    Jumping,
    Uncontrolled,
    AirborneVtol,
}

/// Classify the same airborne state without choosing a caller's user-facing message.
pub(super) fn transfer_motion_blockage(
    world: &World,
    id: ObjectId,
    airborne_vtol: bool,
) -> Result<Option<TransferMotionBlockage>> {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        if unit.free_fall().is_some() || unit.orbital_drop().is_some() {
            return Ok(Some(TransferMotionBlockage::Uncontrolled));
        }
        return Ok(unit
            .vtol_flight()
            .filter(|flight| {
                flight.phase != BattleVtolFlightPhase::Landed
                    && !(airborne_vtol && flight.phase == BattleVtolFlightPhase::Airborne)
            })
            .map(|_| TransferMotionBlockage::AirborneVtol));
    }
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit is not constructed")?;
    Ok(if unit.flight().is_some() {
        Some(TransferMotionBlockage::Jumping)
    } else if unit.free_fall().is_some() || unit.orbital_drop().is_some() {
        Some(TransferMotionBlockage::Uncontrolled)
    } else {
        None
    })
}

/// Preflight containment and destination before mutating the common placement state.
pub(super) fn place(
    world: &mut World,
    id: ObjectId,
    position: BattlePosition,
    mode: PlacementMode,
) -> Result<()> {
    ensure!(world.objects.get(&id).is_some_and(|object| object.kind == Kind::Thing && !object.flags.contains(Flag::Going)), "Unit target must be a live thing");
    let vehicle = world.btech.vehicles().get(&id);
    let mech = world.btech.constructed_units().get(&id);
    let power = vehicle
        .map(BattleVehicle::power)
        .or_else(|| mech.map(BattleUnit::power))
        .context("Unit construction state is unavailable")?;
    if mode == PlacementMode::Administrative {
        super::towing::require_detached(world, id)?;
        ensure!(
            power == BattlePower::Off,
            "Shut down the unit before administrative placement"
        );
    }
    ensure!(
        world
            .objects
            .get(&position.map)
            .is_some_and(|object| matches!(object.kind, Kind::Room | Kind::Thing)
                && !object.flags.contains(Flag::Going)),
        "Map target must be a live room or thing"
    );
    let tile = world
        .btech
        .maps()
        .get(&position.map)
        .context("Map not found")?
        .hex(i64::from(position.x), i64::from(position.y))?;
    world.validate_move(id, position.map)?;
    let slot = super::map_slots::placement_slot(world, id, position.map)?;
    let label = super::map_slots::placement_label(world, id, position.map, slot)?;
    let point = HexCoordinate {
        x: i32::from(position.x),
        y: i32::from(position.y),
    }
    .center();
    let mut motion = if mode == PlacementMode::Transfer {
        vehicle
            .and_then(BattleVehicle::motion)
            .or_else(|| mech.and_then(BattleUnit::motion))
            .context("Placed unit has no motion")?
    } else {
        BattleMotion::stationary(point)
    };
    motion.point = point;
    super::map_slots::arrive(&mut world.btech, id, position.map, slot);
    super::contacts::forget_unit(world, id);
    let identity = if let Some(vehicle) = world.btech.vehicles.get_mut(&id) {
        if mode == PlacementMode::Administrative {
            vehicle.pilot = None;
        }
        vehicle.set_placement(Some((position, slot)));
        vehicle.battlefield_label = label;
        vehicle.motion = Some(motion);
        if let Some(flight) = &mut vehicle.vtol_flight {
            flight.altitude = f64::from(tile.surface_height());
        }
        vehicle.identity()
    } else {
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        if unit.position.is_none_or(|old| old.map != position.map) {
            unit.c3i_network = None;
            unit.c3_network = None;
        }
        unit.detached = false;
        unit.position = Some(position);
        unit.map_slot = Some(slot);
        unit.battlefield_label = label;
        unit.hex_sync_pending = false;
        unit.ground_elevation = None;
        unit.flight = None;
        unit.free_fall = None;
        unit.orbital_drop = None;
        if mode == PlacementMode::Administrative {
            unit.jump_stabilization = 0;
            unit.stagger = Default::default();
            unit.stand_timer = None;
            unit.hull_down = Default::default();
        }
        unit.motion = Some(motion);
        unit.charge.target = None;
        unit.identity()
    };
    world.btech.units.insert(id, identity);
    world.objects.get_mut(&id).unwrap().location = Some(position.map);
    // A new battlefield may be darker or brighter than the one the unit left.
    super::searchlight::reconcile(world, id);
    Ok(())
}
