//! Cockpit assignment coordinated with ordinary player entry, departure and destruction.
use crate::{Flag, Kind, ObjectId, World};
use anyhow::{Context, Result, ensure};
use std::sync::Arc;

/// Assign a physically present player to an unoccupied cockpit. Authority belongs to the adapter.
pub fn assign_pilot(world: &mut World, unit: ObjectId, pilot: ObjectId) -> Result<()> {
    ensure!(!world.btech.unconscious(pilot), "You are unconscious");
    let player = world.objects.get(&pilot).context("Pilot not found")?;
    ensure!(
        player.kind == Kind::Player && !player.flags.contains(Flag::Going),
        "Pilot must be a live player"
    );
    ensure!(
        player.location == Some(unit),
        "Enter the unit before taking the cockpit"
    );
    ensure!(world.objects.get(&unit).is_some_and(|object| object.kind == Kind::Thing && !object.flags.contains(Flag::Going)), "Unit must be a live thing");
    let assigned = if let Some(vehicle) = world.btech.vehicles().get(&unit) {
        ensure!(!vehicle.is_destroyed(), "Vehicle is destroyed");
        vehicle.pilot()
    } else {
        world
            .btech
            .constructed_units()
            .get(&unit)
            .context("Unit construction state is unavailable")?
            .pilot()
    };
    ensure!(
        assigned.is_none() || assigned == Some(pilot),
        "Cockpit is occupied"
    );
    ensure!(
        !world
            .btech
            .constructed_units()
            .iter()
            .map(|(&id, unit)| (id, unit.pilot()))
            .chain(
                world
                    .btech
                    .vehicles()
                    .iter()
                    .map(|(&id, unit)| (id, unit.pilot()))
            )
            .any(|(id, assigned)| id != unit && assigned == Some(pilot)),
        "Player already pilots another unit"
    );
    super::prepare_recovery(world, pilot)?;
    super::crew_recovery::assign(world, unit, pilot);
    if let Some(vehicle) = Arc::make_mut(&mut world.btech.vehicles).get_mut(&unit) {
        vehicle.pilot = Some(pilot);
    } else {
        Arc::make_mut(&mut world.btech.constructed)
            .get_mut(&unit)
            .unwrap()
            .pilot = Some(pilot);
    }
    Ok(())
}

/// Release only the requested player's cockpit assignment.
pub fn release_pilot(world: &mut World, unit: ObjectId, pilot: ObjectId) -> Result<()> {
    if let Some(vehicle) = world.btech.vehicles().get(&unit) {
        ensure!(
            vehicle.pilot() == Some(pilot),
            "You are not piloting this unit"
        );
        Arc::make_mut(&mut world.btech.vehicles)
            .get_mut(&unit)
            .unwrap()
            .pilot = None;
        return Ok(());
    }
    let record = world
        .btech
        .constructed_units()
        .get(&unit)
        .context("Unit construction state is unavailable")?;
    ensure!(
        record.pilot == Some(pilot),
        "You are not piloting this unit"
    );
    Arc::make_mut(&mut world.btech.constructed)
        .get_mut(&unit)
        .unwrap()
        .pilot = None;
    Ok(())
}

/// Reconcile a moved player's cockpit before arrival callbacks observe the new world state.
pub(crate) fn player_moved(world: &mut World, player: ObjectId) {
    let location = world
        .objects
        .get(&player)
        .and_then(|object| object.location);
    for (&id, vehicle) in Arc::make_mut(&mut world.btech.vehicles).iter_mut() {
        if vehicle.pilot == Some(player) && location != Some(id) {
            vehicle.pilot = None;
        }
    }
    let unit = world
        .btech
        .constructed_units()
        .iter()
        .find_map(|(&id, unit)| (unit.pilot == Some(player) && location != Some(id)).then_some(id));
    let Some(unit) = unit else {
        return;
    };
    Arc::make_mut(&mut world.btech.constructed)
        .get_mut(&unit)
        .unwrap()
        .pilot = None;
}

/// Consciousness includes empty cockpit crew recovery as well as an assigned character.
pub(super) fn unit_unconscious(world: &World, id: ObjectId) -> bool {
    let (pilot, remaining) = if let Some(unit) = world.btech.vehicles().get(&id) {
        (unit.pilot(), unit.crew_recovery().remaining)
    } else {
        let unit = &world.btech.constructed_units()[&id];
        (unit.pilot(), unit.crew_recovery().remaining)
    };
    remaining > 0 || pilot.is_some_and(|pilot| world.btech.unconscious(pilot))
}

/// Replace a cockpit assignment inside an authorized caller's rollback boundary.
/// Assignment retains the ordinary presence, health and recovery rules.
pub(super) fn set_administrative_pilot(
    world: &mut World,
    unit: ObjectId,
    pilot: Option<ObjectId>,
) -> Result<()> {
    let current = if let Some(vehicle) = world.btech.vehicles().get(&unit) {
        vehicle.pilot()
    } else {
        world
            .btech
            .constructed_units()
            .get(&unit)
            .context("Unit is unavailable")?
            .pilot()
    };
    if current == pilot {
        return Ok(());
    }
    if let Some(current) = current {
        release_pilot(world, unit, current)?;
    }
    if let Some(pilot) = pilot {
        assign_pilot(world, unit, pilot)?;
    }
    Ok(())
}
