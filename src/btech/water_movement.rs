//! Water hex-entry control checks and immersion effects inside a movement transaction.
use super::{BattleFallRules, BattleNotice, BattlePosture, Terrain};
use crate::{ObjectId, World};
use anyhow::{Context, Result};

/// Water-entry messages and whether immersion interrupted further travel.
#[derive(Default)]
pub(super) struct WaterEntryReport {
    pub pilot_notices: Vec<super::BattlePilotNotice>,
    pub notices: Vec<BattleNotice>,
    pub experience_messages: Vec<super::BattleChannelMessage>,
    pub stopped: bool,
}

/// Resolve entry at the already committed candidate position. Reverse steps can skip the water check.
pub(super) fn enter_water(
    world: &mut World,
    id: ObjectId,
    check: bool,
    rules: BattleFallRules,
    mut falls: Option<&mut Vec<super::BattleFallReport>>,
) -> Result<WaterEntryReport> {
    let unit = &world.btech.constructed_units()[&id];
    let position = unit.position().context("Water entry requires placement")?;
    let tile =
        world.btech.maps()[&position.map].base_hex(i64::from(position.x), i64::from(position.y))?;
    let below_bridge = tile.terrain == Terrain::Bridge && unit.elevation_level(tile) < 0;
    let below_ice = tile.terrain == Terrain::Ice && unit.elevation_level(tile) < 0;
    let high_water = tile.terrain == Terrain::HighWater;
    if tile.terrain != Terrain::Water && !below_bridge && !below_ice && !high_water {
        return Ok(WaterEntryReport::default());
    }
    let depth = if below_bridge { 1 } else { tile.elevation };
    let pilot = unit.pilot();
    let mut notices = Vec::new();
    let mut pilot_notices = Vec::new();
    let mut experience_messages = Vec::new();
    if check && (depth > 0 || high_water) {
        let walking = unit.movement_maximum_speed() * 2.0 / 3.0;
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        let motion = unit
            .motion
            .as_mut()
            .context("Water entry requires motion")?;
        motion.desired_speed = motion.desired_speed.min(walking);
        let running = motion.speed > walking + 0.1;
        notices.push(BattleNotice {
            unit: id,
            text: (if running {
                "You struggle to keep control as you run into the water!"
            } else {
                "You use your piloting skill to maneuver through the water."
            })
            .to_owned(),
        });
        let modifier = if high_water {
            -2
        } else if depth > 3 {
            1
        } else {
            i16::from(depth) - 2
        } + if running { 2 } else { 0 };
        let mut control = super::roll_piloting(world, id, modifier, rules.extended_piloting)?;
        super::piloting::capture_feedback(id, pilot, &control, &mut notices, &mut pilot_notices);
        if falls.is_some() {
            experience_messages.extend(super::piloting::award_control_check(
                world,
                id,
                &mut control,
                rules.extended_piloting,
            )?);
        }
        if !control.success {
            notices.push(BattleNotice {
                unit: id,
                text: "You slip in the water and fall down".to_owned(),
            });
            let report =
                if falls.is_some() && world.objects[&id].flags.contains(crate::Flag::InCharacter) {
                    super::fall::resolve_character_fall(world, id, 1, rules)?
                } else {
                    super::resolve_fall(world, id, 1, rules)?
                };
            report.append_notices(id, &mut notices, &mut pilot_notices);
            if let Some(falls) = falls.as_deref_mut() {
                falls.push(report);
            }
        }
    }
    let reports = if falls.is_some() {
        super::flooding::flood_unit_in_action(world, id, rules)?
    } else {
        super::flood_unit(world, id, rules)?
    };
    for report in reports {
        super::piloting::append_feedback(&mut pilot_notices, report.pilot_notices, notices.len());
        notices.extend(report.notices);
        if let Some(fall) = report.fall
            && let Some(falls) = falls.as_deref_mut()
        {
            falls.push(fall);
        }
    }
    notices.extend(super::extinguish_inferno_in_water(world, id)?);
    let unit = &world.btech.constructed_units()[&id];
    Ok(WaterEntryReport {
        notices,
        pilot_notices,
        experience_messages,
        stopped: unit.posture() == BattlePosture::Prone
            || unit.is_destroyed()
            || unit.movement_maximum_speed() == 0.0,
    })
}
