//! Controlled Mech drops reuse piloting, fall damage, flooding and physical mine events.
use super::{MechFallReport, MineEventReport, Notice, PilotingCheck, SectionExposureReport};
use crate::{Config, ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Completed controlled drop; an optional failed control check precedes ordinary fall consequences.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProneReport {
    /// Slow drops succeed without consuming control dice.
    pub check: Option<PilotingCheck>,
    /// A failed fast-drop check delegates damage to the ordinary fall resolver.
    pub fall: Option<MechFallReport>,
    /// New breaches flooded after the final prone posture is established.
    pub flooding: Vec<SectionExposureReport>,
    /// The final stepping event follows any earlier fall-triggered mines.
    pub mines: MineEventReport,
    /// Ordered cockpit and observer messages already published by the host action.
    pub notices: Vec<Notice>,
    /// Private control feedback follows the drop warnings and precedes impact feedback.
    pub pilot_notices: Vec<super::PilotNotice>,
    /// Control-check experience diagnostics committed with the action.
    pub experience_messages: Vec<super::DiagnosticMessage>,
}

/// Execute the native and Lua controlled-drop contract within one host rollback boundary.
pub fn prone_action(
    scripts: &Scripts,
    config: &Config,
    id: ObjectId,
    pilot: ObjectId,
) -> Result<ProneReport> {
    scripts.atomic(|before| {
        let report = resolve(&mut scripts.world.borrow_mut(), config, id, pilot)?;
        super::piloting::publish_maneuver_feedback(
            scripts,
            config,
            &report.notices,
            &report.pilot_notices,
            report.check.as_ref(),
            true,
        )?;
        super::diagnostics::publish(scripts, config, &report.experience_messages)?;
        if let Some(fall) = &report.fall {
            super::evacuation::publish_fall_consequences(scripts, config, fall)?;
        }
        for flood in &report.flooding {
            super::evacuation::publish_section_exposure_consequences(scripts, config, flood)?;
        }
        super::evacuation::publish_mine_consequences(scripts, config, &report.mines)?;
        super::evacuation::publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(report)
    })
}

/// Validate before dice or posture changes, then resolve the complete physical sequence.
fn resolve(
    world: &mut World,
    config: &Config,
    id: ObjectId,
    pilot: ObjectId,
) -> Result<ProneReport> {
    ensure!(
        !world.btech.vehicles().contains_key(&id),
        "You can't prone in this!"
    );
    super::power::controlled_unit(world, id, pilot)?;
    let unit = &world.btech.constructed_units()[&id];
    ensure!(
        unit.power() == super::Power::Running,
        "Start the unit first"
    );
    ensure!(
        unit.posture() != super::Posture::Prone,
        "You are already prone."
    );
    ensure!(!unit.airborne(), "You can't prone in the air!");
    ensure!(
        unit.stand_timer().is_none(),
        "You can't drop while trying to stand up!"
    );
    unit.position().context("Unit is not on a battlefield")?;
    let speed = unit
        .motion()
        .context("Unit has no motion state")?
        .speed
        .abs();
    let threshold = f64::from(
        super::effective_speed::configured(
            world,
            id,
            super::speed_bonus::SpeedPolicy::configured(config),
        )? as f32
            / 3.0,
    );
    let speed_levels = if speed > threshold * 2.0 {
        2
    } else {
        u8::from(speed > threshold)
    };
    let stagger_level = unit.stagger().action_level();
    let levels = speed_levels.max(u8::from(stagger_level > 0));
    let mut rules = super::FallRules::configured(config);
    rules.toughness = world
        .btech
        .character_values()
        .get(&pilot)
        .is_some_and(|values| super::advantages::enabled(values, "Toughness"));
    let mut notices = Vec::new();
    let mut pilot_notices = Vec::new();
    let mut experience_messages = Vec::new();
    if speed_levels > 0 {
        notices.push(Notice {
            unit: id,
            text: if speed_levels == 2 {
                "You attempt a controlled drop while running."
            } else {
                "You attempt a controlled drop from your fast walk."
            }
            .into(),
        });
    }
    if stagger_level > 0 {
        notices.push(Notice {
            unit: id,
            text: "Still staggering, you try not to fall on your face.".into(),
        });
    }
    let check = if levels > 0 {
        // Large restored values still guarantee failure within the shared control-roll range.
        let modifier =
            (stagger_level + if speed_levels == 2 { 2 } else { 0 }).min(i32::from(i16::MAX)) as i16;
        let mut check = super::roll_piloting(world, id, modifier, rules.extended_piloting)?;
        super::piloting::capture_feedback(
            id,
            Some(pilot),
            &check,
            &mut notices,
            &mut pilot_notices,
        );
        experience_messages.extend(super::piloting::award_control_check(
            world,
            id,
            &mut check,
            rules.extended_piloting,
        )?);
        Some(check)
    } else {
        None
    };
    let failed = check.as_ref().is_some_and(|check| !check.success);
    notices.push(Notice {
        unit: id,
        text: match check.as_ref() {
            None => "You drop to the ground prone!",
            Some(check) if check.success => "You hit the ground with minimal damage",
            Some(_) => "You fall to the ground hard",
        }
        .into(),
    });
    notices.extend(super::broadcast::observer_notices(
        world,
        id,
        if failed {
            "falls hard to the ground!"
        } else {
            "drops to the ground!"
        },
    ));
    let fall = if failed {
        let fall = if world.objects[&id].flags.contains(crate::Flag::InCharacter) {
            super::fall::resolve_character_fall(world, id, levels, rules)?
        } else {
            super::resolve_fall(world, id, levels, rules)?
        };
        fall.append_notices(id, &mut notices, &mut pilot_notices);
        Some(fall)
    } else {
        None
    };
    let unit = world.btech.constructed.get_mut(&id).unwrap();
    super::fall::set_prone(unit);
    unit.motion.as_mut().unwrap().stop_translation();
    let flooding = super::flooding::flood_unit_in_action(world, id, rules)?;
    for flood in &flooding {
        super::piloting::append_feedback(
            &mut pilot_notices,
            flood.pilot_notices.iter().cloned(),
            notices.len(),
        );
        notices.extend(flood.notices.clone());
    }
    notices.extend(super::extinguish_inferno_in_water(world, id)?);
    if rules.stagger != super::StaggerMode::Traditional {
        world
            .btech
            .constructed
            .get_mut(&id)
            .unwrap()
            .stagger
            .clear_damage();
    }
    let mines = super::mine_event::resolve(world, id, super::MineTriggerReason::Step, rules, true)?;
    super::piloting::append_feedback(
        &mut pilot_notices,
        mines.pilot_notices.iter().cloned(),
        notices.len(),
    );
    notices.extend(mines.notices.clone());
    Ok(ProneReport {
        check,
        fall,
        flooding,
        mines,
        notices,
        pilot_notices,
        experience_messages,
    })
}

/// Cockpit adapter; the reference command ignores trailing positional text.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    _: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let id = ctx
            .scripts
            .world
            .borrow()
            .objects
            .get(&ctx.player)
            .and_then(|player| player.location)
            .context("Enter a unit first")?;
        prone_action(ctx.scripts, ctx.config, id, ctx.player)
    })();
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
