//! Horizontal travel estimates from current speed, independent of terrain routing or sensors.
use crate::{ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// A detached estimate; no minutes means the unit is effectively stationary.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EtaReport {
    pub coordinate: super::HexCoordinate,
    pub range: f64,
    pub minutes: Option<u32>,
    pub text: String,
}

/// Measure a supplied coordinate or an ordinary selected hex; does not change simulation state.
/// Reverse speed uses its magnitude. Explicit coordinates may lie outside the current map.
pub fn eta(world: &World, unit: ObjectId, viewer: ObjectId, arguments: &str) -> Result<EtaReport> {
    let source = super::brief::display_source(world, unit, viewer)?;
    let record = super::scanner::scanner_unit(world, source.unit).context("Unit is unavailable")?;
    ensure!(
        record.power == super::Power::Running && !record.destroyed,
        "Start the unit first"
    );
    record.position.context("Unit is not on a battlefield")?;
    let words: Vec<_> = arguments.split_whitespace().collect();
    let coordinate = match words.as_slice() {
        [] => match source.selection(world) {
            Some(super::TargetSelection::Hex(lock)) if lock.mode == super::HexTargetMode::Hex => {
                lock.hex
            }
            _ => anyhow::bail!("You have invalid default target for ETA!"),
        },
        [x, y] => super::HexCoordinate {
            x: x.parse().context("Invalid coordinates!")?,
            y: y.parse().context("Invalid coordinates!")?,
        },
        [_] => anyhow::bail!("Invalid number of arguments!"),
        _ => anyhow::bail!("Invalid arguments!"),
    };
    let range = record
        .point
        .context("Unit has no motion state")?
        .range(coordinate.center())?;
    let minutes = (record.speed.abs() >= 0.1).then(|| {
        (range / (record.speed.abs() / 10.75))
            .min(f64::from(i32::MAX))
            .trunc() as u32
    });
    let estimate = minutes.map_or_else(
        || "Never, mech not moving".to_owned(),
        |minutes| format!("{:02}:{:02}", minutes / 60, minutes % 60),
    );
    Ok(EtaReport {
        coordinate,
        range,
        minutes,
        text: format!(
            "Range to hex ({},{}) is {:.1}.  ETA: {estimate}.",
            coordinate.x, coordinate.y, range
        ),
    })
}

/// Publish an estimate to cockpit occupants with rollback on notification failure.
pub fn eta_action(
    scripts: &Scripts,
    unit: ObjectId,
    viewer: ObjectId,
    arguments: &str,
) -> Result<EtaReport> {
    scripts.atomic(|_| {
        let report = eta(&scripts.world(), unit, viewer, arguments)?;
        let source = super::brief::display_source(&scripts.world(), unit, viewer)?;
        super::notify_unit_text(scripts, source.unit, &report.text)?;
        Ok(report)
    })
}

/// Native estimate resolves the invoking occupant's cockpit.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let unit = ctx
            .scripts
            .world()
            .objects
            .get(&ctx.player)
            .and_then(|player| player.location)
            .context("Enter a unit first")?;
        eta_action(ctx.scripts, unit, ctx.player, &input.args)
    })();
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
