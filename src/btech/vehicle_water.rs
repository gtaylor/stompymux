//! Vehicles that end up in water they cannot cross flood and are lost.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Result, ensure};

/// Whether water at `height` in `tile` floods this vehicle: anything but a hovercraft or a
/// vehicle with waterproof equipment that finds itself below the surface.
pub(super) fn requires_check(unit: &Vehicle, tile: Hex, height: i32) -> bool {
    unit.definition().movement != VehicleMovement::Hover
        && !unit.definition().has_special("Waterproof_Tech")
        && height < i32::from(tile.water_line())
        && (tile.is_open_water() || tile.has_bridge())
}

/// Apply the common water destruction consequence without fabricating material damage.
pub(super) fn flood(world: &mut World, id: ObjectId, character: bool) -> Result<Notice> {
    ensure!(
        character || !world.objects[&id].flags.contains(Flag::InCharacter),
        "Character vehicle flooding requires a host movement transaction"
    );
    world
        .btech
        .vehicles
        .get_mut(&id)
        .unwrap()
        .destroy_by_flooding();
    Ok(Notice {
        unit: id,
        text: "You drive into the water and your vehicle becomes inoperable.".into(),
    })
}
