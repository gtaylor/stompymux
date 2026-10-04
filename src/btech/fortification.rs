//! Shared scenario fortification state and admission for every supported unit chassis.
use crate::{Flag, Kind, ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Inspect the scenario flag independently of cockpit authority and power state.
pub fn unit_fortified(world: &World, id: ObjectId) -> Result<bool> {
    let unit = world
        .btech
        .unit(id)
        .context("Unit construction is unavailable")?;
    Ok(unit.fortified())
}

/// Trusted scenario edit; enable only on a settled, detached unit so no queued travel escapes the gate.
pub fn set_fortified(world: &mut World, id: ObjectId, enabled: bool) -> Result<()> {
    ensure!(world.objects.get(&id).is_some_and(|object| object.kind == Kind::Thing && !object.flags.contains(Flag::Going)), "Unit is unavailable");
    unit_fortified(world, id)?;
    if enabled {
        super::towing::require_detached(world, id)?;
        let motion = world.btech.vehicles().get(&id).map_or_else(
            || world.btech.constructed_units()[&id].motion(),
            |unit| unit.motion(),
        );
        ensure!(
            motion.is_none_or(|motion| motion.speed == 0.0
                && motion.desired_speed == 0.0
                && motion.heading == motion.desired_heading),
            "Stop the unit and finish turning before fortifying it"
        );
        if let Some(unit) = world.btech.vehicles().get(&id) {
            ensure!(
                unit.free_fall().is_none()
                    && unit
                        .vtol_flight()
                        .is_none_or(|flight| flight.phase == super::BattleVtolFlightPhase::Landed),
                "Land the unit before fortifying it"
            );
            ensure!(
                unit.building_entry.is_none(),
                "Cancel building entry before fortifying the unit"
            );
        } else {
            let unit = &world.btech.constructed_units()[&id];
            ensure!(
                !unit.airborne() && unit.free_fall().is_none(),
                "Land the unit before fortifying it"
            );
            ensure!(
                unit.building_entry.is_none(),
                "Cancel building entry before fortifying the unit"
            );
        }
    }
    if let Some(unit) = world.btech.vehicles.get_mut(&id) {
        unit.fortified = enabled;
    } else {
        world.btech.constructed.get_mut(&id).unwrap().fortified = enabled;
    }
    Ok(())
}

/// Player movement, stance changes and pickup use a single chassis-independent restriction.
pub(super) fn require_mobile(world: &World, id: ObjectId) -> Result<()> {
    require_unfortified(unit_fortified(world, id)?)
}

/// Material-only vehicle controls share the same restriction as world actions.
pub(super) fn require_unfortified(fortified: bool) -> Result<()> {
    ensure!(
        !fortified,
        "Your fortified state prevents movement or towing"
    );
    Ok(())
}
