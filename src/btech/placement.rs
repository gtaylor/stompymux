//! Administrative battlefield placement, coordinated with world containment.
use super::BattlePosition;
use crate::{Flag, Kind, ObjectId, World};
use anyhow::{Context, Result, ensure};
use std::sync::Arc;

/// Place a constructed unit on decoded terrain, preserving the unit's existing condition.
pub fn place_unit(world: &mut World, id: ObjectId, map: ObjectId, x: i64, y: i64) -> Result<()> {
    super::map_transfer::place(
        world,
        id,
        BattlePosition {
            map,
            x: u16::try_from(x).context("Map coordinates out of bounds")?,
            y: u16::try_from(y).context("Map coordinates out of bounds")?,
        },
        super::map_transfer::PlacementMode::Administrative,
    )
}

/// Remove a unit from battlefield coordinates and move it into an ordinary container.
pub fn remove_unit(world: &mut World, id: ObjectId, destination: ObjectId) -> Result<()> {
    super::towing::require_detached(world, id)?;
    if world.btech.vehicles().contains_key(&id) {
        return super::vehicle_placement::remove_vehicle(world, id, destination);
    }
    unit_target(world, id)?;
    ensure!(
        world.btech.constructed_units()[&id].power() == super::BattlePower::Off,
        "Shut down the unit before administrative placement"
    );
    ensure!(
        !world.btech.maps().contains_key(&destination),
        "Use unit-place to enter a battlefield"
    );
    ensure!(
        world
            .objects
            .get(&destination)
            .is_some_and(|object| matches!(object.kind, Kind::Room | Kind::Thing)
                && !object.flags.contains(Flag::Going)),
        "Destination must be a live room or thing"
    );
    world.validate_move(id, destination)?;
    detach_membership(world, id)?;
    world.objects.get_mut(&id).unwrap().location = Some(destination);
    Ok(())
}

/// Clear tactical membership without moving the unit's enclosing game object.
/// Callers own shutdown and tow release; ordinary removal and bulk clearing share cleanup.
pub(super) fn detach_membership(world: &mut World, id: ObjectId) -> Result<()> {
    super::towing::require_detached(world, id)?;
    ensure!(
        super::scanner::scanner_unit(world, id)
            .is_some_and(|unit| unit.power == super::BattlePower::Off),
        "Shut down the unit before administrative placement"
    );
    super::map_slots::depart(&mut world.btech, id);
    super::contacts::forget_unit(world, id);
    let identity = if let Some(unit) = Arc::make_mut(&mut world.btech.vehicles).get_mut(&id) {
        unit.pilot = None;
        unit.building_entry = None;
        unit.set_placement(None);
        unit.identity()
    } else {
        let unit = Arc::make_mut(&mut world.btech.constructed)
            .get_mut(&id)
            .context("Unit construction state is unavailable")?;
        unit.pilot = None;
        unit.c3i_network = None;
        unit.c3_network = None;
        unit.detached = false;
        unit.position = None;
        unit.map_slot = None;
        unit.battlefield_label = None;
        unit.hex_sync_pending = false;
        unit.ground_elevation = None;
        unit.flight = None;
        unit.free_fall = None;
        unit.jump_stabilization = 0;
        unit.stagger = Default::default();
        unit.stand_timer = None;
        unit.hull_down = Default::default();
        unit.building_entry = None;
        unit.motion = None;
        unit.identity()
    };
    Arc::make_mut(&mut world.btech.units).insert(id, identity);
    Ok(())
}

/// Check supported live unit state before any placement mutation.
fn unit_target(world: &World, id: ObjectId) -> Result<()> {
    ensure!(world.objects.get(&id).is_some_and(|object| object.kind == Kind::Thing && !object.flags.contains(Flag::Going)), "Unit target must be a live thing");
    ensure!(
        world.btech.constructed_units().contains_key(&id),
        "Unit construction state is unavailable"
    );
    Ok(())
}
