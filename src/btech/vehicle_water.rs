//! Vehicle water entry shares control dice, XP and durable flooding with other movement hazards.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Result, ensure};

/// Water avoidance stops movement; success also restores the previous position.
pub(super) struct WaterEntry {
    pub restore: bool,
    pub movement: super::movement_report::MovementReport,
}

/// Ground vehicles with waterproof equipment and hovercraft do not need water avoidance.
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

/// Attempt a stop before ordinary entry; this does not use auto-fall or skid modifiers.
pub(super) fn enter(
    world: &mut World,
    id: ObjectId,
    speed: f64,
    rules: FallRules,
    character: bool,
) -> Result<WaterEntry> {
    let control = super::terrain_control::check(
        world,
        id,
        super::cliff::modifier(speed, false),
        rules.extended_piloting,
        character,
    )?;
    let success = control.success;
    let mut notices = vec![Notice {
        unit: id,
        text: "You notice a body of water in front of you".into(),
    }];
    let mut pilot_notices = Vec::new();
    control.capture_feedback(id, &mut notices, &mut pilot_notices);
    if success {
        notices.push(Notice {
            unit: id,
            text: "You manage to stop before falling in.".into(),
        });
        notices.extend(super::broadcast::observer_notices(
            world,
            id,
            "stops suddenly to avoid driving into the water!",
        ));
    } else {
        notices.push(flood(world, id, character)?);
    }
    Ok(WaterEntry {
        restore: success,
        movement: super::movement_report::MovementReport {
            notices,
            pilot_notices,
            experience_messages: control.experience_messages,
            ..Default::default()
        },
    })
}
