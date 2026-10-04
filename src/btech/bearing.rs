//! Read-only cockpit bearings from live positions or explicit map coordinates.
use crate::{ObjectId, World};
use anyhow::{Context, Result};
use serde::Serialize;

/// Detached compass measurement with the same text used by the native command.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleBearingReport {
    pub origin: super::Point,
    pub destination: super::Point,
    pub bearing: u16,
    pub text: String,
}

/// Resolve zero, two or four integer arguments; unit defaults must remain acquired and visible.
pub fn bearing(
    world: &World,
    unit: ObjectId,
    viewer: ObjectId,
    arguments: &str,
) -> Result<BattleBearingReport> {
    let segment =
        super::navigation_measurement::resolve(world, unit, viewer, arguments, "Bearing")?;
    let origin = segment.origin.point;
    let destination = segment.destination.point;
    let prefix = segment.prefix;
    let bearing = origin
        .bearing(destination)?
        .unwrap_or(180.0)
        .round()
        .rem_euclid(360.0) as u16;
    Ok(BattleBearingReport {
        origin,
        destination,
        bearing,
        text: format!("{prefix}{bearing} degrees."),
    })
}

/// Native reply is private to the invoking cockpit occupant.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let world = ctx.scripts.world();
        let unit = world
            .objects
            .get(&ctx.player)
            .and_then(|player| player.location)
            .context("Enter a unit first")?;
        bearing(&world, unit, ctx.player, &input.args)
    })();
    Ok(crate::CommandAction::Report(crate::CommandReport::Reply(
        match result {
            Ok(report) => report.text,
            Err(error) => format!("{error:#}"),
        },
    )))
}
