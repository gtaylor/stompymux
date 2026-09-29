//! Ground-vehicle control checks use crew skills and private dice without applying hazard effects.
use super::BattlePilotingCheck;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Evaluate a vehicle control check; the caller owns damage, XP, notices and the transaction.
pub(super) fn roll(
    world: &mut World,
    id: ObjectId,
    modifier: i32,
    extended: bool,
) -> Result<BattlePilotingCheck> {
    let object = world.objects.get(&id).context("Unit is unavailable")?;
    ensure!(!object.flags.contains(Flag::Going), "Unit is unavailable");
    let vehicle = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle is unavailable")?;
    let skill = super::skills::unit_piloting_target(world, id, extended)?;
    let cockpit = u8::from(
        vehicle
            .definition()
            .has_technology(super::BattleTechnology::SmallCockpit),
    );
    let absent_character_pilot = if object.flags.contains(Flag::InCharacter)
        && vehicle
            .pilot()
            .and_then(|pilot| world.objects.get(&pilot))
            .is_none_or(|pilot| pilot.location != Some(id))
    {
        5
    } else {
        0
    };
    let damage = vehicle.piloting_damage();
    let armor = u8::from(
        vehicle
            .definition()
            .has_technology(super::BattleTechnology::HardenedArmor),
    );
    let target = i32::from(skill)
        .wrapping_add(i32::from(damage))
        .wrapping_add(i32::from(cockpit))
        .wrapping_add(i32::from(armor))
        .wrapping_add(modifier)
        .wrapping_add(i32::from(absent_character_pilot));
    let blocked =
        super::piloting::controls_blocked(world, id, vehicle.power()) || vehicle.is_destroyed();
    let roll = if blocked {
        None
    } else {
        Some(
            world
                .btech
                .vehicles
                .get_mut(&id)
                .unwrap()
                .dice
                .generic_roll(),
        )
    };
    Ok(BattlePilotingCheck {
        skill,
        damage,
        cockpit,
        armor,
        situational: modifier,
        absent_character_pilot,
        target,
        roll,
        success: roll.is_some_and(|value| i32::from(value) >= target),
        experience: None,
    })
}
