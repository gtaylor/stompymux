//! Configured coordinate-fire resolution and pre-impact observer feedback.
use super::{HexCoordinate, ShotRules};
use crate::{Config, ObjectId, World};
use anyhow::{Context, Result};

/// Use the common character transaction while keeping terrain-shot feedback coordinate based.
pub(super) fn resolve_in_action(
    world: &mut World,
    config: &Config,
    shooter: ObjectId,
    pilot: ObjectId,
    index: usize,
    coordinate: HexCoordinate,
) -> Result<super::firing::FiringAction> {
    let weapon = crate::btech::with_unit!(
        world
            .btech
            .unit(shooter)
            .context("Shooter is not constructed")?,
        |unit| { unit.weapon_readiness(index)?.weapon }
    );
    let observers = super::broadcast::hex_fire_messages(world, shooter, coordinate, weapon);
    let explosions =
        super::observer_messages(world, shooter, "shudders from an internal explosion!");
    let report = super::hex_shot::resolve_hex_shot_in_action(
        world,
        shooter,
        pilot,
        coordinate,
        index,
        ShotRules::configured(&config.battletech, false),
    )?;
    let feedback: super::launch_feedback::LaunchFeedback = (&report).into();
    if let Some(messages) = super::launch_feedback::failure_messages(&feedback) {
        return Ok(super::firing::FiringAction {
            pilot_notices: Vec::new(),
            report: report.into(),
            messages,
        });
    }
    let name = weapon.name().split_once('.').expect("catalog namespace").1;
    let number = report
        .target_number
        .map_or("out of range".into(), |number| number.to_string());
    let mut messages = vec![(
        shooter,
        format!(
            "You fire {name} at ({},{}) - BTH: {number} Roll: {}.",
            coordinate.x, coordinate.y, report.roll
        ),
    )];
    messages.extend(observers);
    let mut private = Vec::new();
    let notices = report.notices_with_feedback(&mut private);
    let mut pilot_notices = Vec::new();
    super::piloting::append_feedback(&mut pilot_notices, private, messages.len());
    messages.extend(notices.into_iter().map(|notice| (notice.unit, notice.text)));
    let base = messages.len();
    messages.extend(super::launch_feedback::consequence_messages(
        feedback,
        explosions,
        &mut pilot_notices,
        base,
    ));
    Ok(super::firing::FiringAction {
        pilot_notices,
        report: report.into(),
        messages,
    })
}
