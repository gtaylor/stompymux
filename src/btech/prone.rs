//! Deliberate drops to the ground (Total Warfare p. 49): no roll, no damage and no extra heat,
//! the unit keeps its facing, and the flooding and mine events of touching the ground follow.
use super::{MineEventReport, Notice, SectionExposureReport};
use crate::{Config, ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Seconds after launching a jump during which a Mech may not drop to the ground: the
/// book forbids dropping in a turn the Mech jumped.
const JUMP_LOCKOUT_SECONDS: i64 = 30;

/// Completed controlled drop and the ground-contact events it triggered.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProneReport {
    /// New breaches flooded after the final prone posture is established.
    pub flooding: Vec<SectionExposureReport>,
    /// The stepping event of coming to rest on the ground.
    pub mines: MineEventReport,
    /// Ordered cockpit and observer messages already published by the host action.
    pub notices: Vec<Notice>,
    /// Private feedback from flooding and mine consequences.
    pub pilot_notices: Vec<super::PilotNotice>,
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
            &report.notices,
            &report.pilot_notices,
            None,
            true,
        )?;
        for flood in &report.flooding {
            super::evacuation::publish_section_exposure_consequences(scripts, config, flood)?;
        }
        super::evacuation::publish_mine_consequences(scripts, config, &report.mines)?;
        super::evacuation::publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(report)
    })
}

/// Validate, then drop: a deliberate drop costs 1 MP in the book and nothing else, so here
/// it only stops the unit's travel and lays it down with the facing it had.
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
    ensure!(
        unit.last_jump_launch
            .is_none_or(|launched| world.btech.simulation_time() - launched >= JUMP_LOCKOUT_SECONDS),
        "You can't drop to the ground so soon after jumping!"
    );
    unit.position().context("Unit is not on a battlefield")?;
    unit.motion().context("Unit has no motion state")?;
    let mut rules = super::FallRules::configured(config);
    rules.toughness = world
        .btech
        .character_values()
        .get(&pilot)
        .is_some_and(|values| super::advantages::enabled(values, "Toughness"));
    let mut notices = vec![Notice {
        unit: id,
        text: "You drop to the ground prone!".into(),
    }];
    notices.extend(super::broadcast::observer_notices(
        world,
        id,
        "drops to the ground!",
    ));
    let unit = world.btech.constructed.get_mut(&id).unwrap();
    super::fall::set_prone(unit);
    unit.motion.as_mut().unwrap().stop_translation();
    let mut pilot_notices = Vec::new();
    let flooding = super::flooding::flood_unit_in_action(world, id, rules)?;
    for flood in &flooding {
        super::piloting::append_feedback(
            &mut pilot_notices,
            flood.pilot_notices.iter().cloned(),
            notices.len(),
        );
        notices.extend(flood.notices.clone());
    }
    let mines = super::mine_event::resolve(world, id, super::MineTriggerReason::Step, rules, true)?;
    super::piloting::append_feedback(
        &mut pilot_notices,
        mines.pilot_notices.iter().cloned(),
        notices.len(),
    );
    notices.extend(mines.notices.clone());
    Ok(ProneReport {
        flooding,
        mines,
        notices,
        pilot_notices,
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
