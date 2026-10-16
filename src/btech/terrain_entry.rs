//! Terrain hazards a 'Mech meets as it steps into a hex, inside a movement transaction: the
//! piloting rolls rubble and ice take to cross, soft ground that can bog it down and magma
//! crust that may break beneath it. Water has its own entry rules in `water_movement.rs`;
//! magma heat is charged on leaving the hex (`motion.rs`) and while standing in it (`heat.rs`).
use super::{FallRules, Notice, Posture};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result};

/// Entry messages and experience; the caller checks whether a fall stopped the unit.
#[derive(Default)]
pub(super) struct TerrainEntryReport {
    pub pilot_notices: Vec<super::PilotNotice>,
    pub notices: Vec<Notice>,
    pub experience_messages: Vec<super::DiagnosticMessage>,
}

/// Resolve the terrain of the hex the 'Mech `id` has just been placed in. `falls` collects
/// nested falls when the host action publishes character consequences.
pub(super) fn enter(
    world: &mut World,
    id: ObjectId,
    rules: FallRules,
    falls: Option<&mut Vec<super::MechFallReport>>,
) -> Result<TerrainEntryReport> {
    let unit = &world.btech.constructed_units()[&id];
    if unit.is_destroyed() {
        return Ok(TerrainEntryReport::default());
    }
    let position = unit
        .position()
        .context("Terrain entry requires placement")?;
    let tile =
        world.btech.maps()[&position.map].base_hex(i64::from(position.x), i64::from(position.y))?;
    let level = unit.elevation_level(tile);
    let falls_pending = falls.is_some();
    let mut report = TerrainEntryReport::default();
    if let Some(modifier) = tile.entry_piloting_modifier(super::GroundMovement::Legged)
        && (tile.touches_ground(level) || (tile.is_ice() && level >= i32::from(tile.water_line())))
        && unit.posture() != Posture::Prone
    {
        let ice = tile.is_icy();
        report.notices.push(Notice {
            unit: id,
            text: if ice {
                "You step carefully onto the ice.".into()
            } else {
                "You pick your way across the rubble.".into()
            },
        });
        // Rubble takes a plain roll; ice takes a roll without the bonus it gives rolls forced
        // on units already standing on it.
        let control = if ice {
            super::terrain_control::check_entry(
                world,
                id,
                modifier,
                rules.extended_piloting,
                falls.is_some(),
            )?
        } else {
            super::terrain_control::check(
                world,
                id,
                modifier,
                rules.extended_piloting,
                falls.is_some(),
            )?
        };
        control.capture_feedback(id, &mut report.notices, &mut report.pilot_notices);
        report
            .experience_messages
            .extend(control.experience_messages);
        if !control.success {
            report.notices.push(Notice {
                unit: id,
                text: if ice {
                    "Your feet go out from under you on the ice!".into()
                } else {
                    "You lose your footing in the rubble and fall!".into()
                },
            });
            report.notices.extend(super::broadcast::observer_notices(
                world,
                id,
                if ice {
                    "slips on the ice and falls down!"
                } else {
                    "stumbles in the rubble and falls down!"
                },
            ));
            let fall = if falls.is_some() && world.objects[&id].flags.contains(Flag::InCharacter) {
                super::fall::resolve_character_fall(world, id, 1, rules)?
            } else {
                super::resolve_fall(world, id, 1, rules)?
            };
            fall.append_notices(id, &mut report.notices, &mut report.pilot_notices);
            if let Some(falls) = falls {
                falls.push(fall);
            }
        }
    }
    // Soft ground can hold a 'Mech that is still on its feet after the step.
    let unit = &world.btech.constructed_units()[&id];
    if !unit.is_destroyed()
        && unit.posture() != Posture::Prone
        && let Some(bog) =
            super::bog_down::enter(world, id, rules.extended_piloting, falls_pending, false)?
    {
        super::piloting::append_feedback(
            &mut report.pilot_notices,
            bog.pilot_notices,
            report.notices.len(),
        );
        report.notices.extend(bog.notices);
        report.experience_messages.extend(bog.experience_messages);
    }
    report.notices.extend(super::magma::crack_crust(
        world,
        id,
        super::magma::MagmaEntry::Ground,
    )?);
    Ok(report)
}
