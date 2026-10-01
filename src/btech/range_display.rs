//! Cockpit spatial range reports with dark-map terrain-height masking.
use super::navigation_measurement::EndpointSource;
use crate::{ObjectId, World};
use anyhow::{Context, Result};
use serde::Serialize;

/// Detached spatial and horizontal measurements in hex units.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleRangeReport {
    pub horizontal: f64,
    pub spatial: f64,
    pub text: String,
}

/// Measure the default target, a destination hex, or two explicit hex centers.
/// Dark maps suppress terrain height differences but preserve live unit target height.
pub fn range_display(
    world: &World,
    unit: ObjectId,
    viewer: ObjectId,
    arguments: &str,
) -> Result<BattleRangeReport> {
    let segment = super::navigation_measurement::resolve(world, unit, viewer, arguments, "Range")?;
    let horizontal = segment.origin.point.range(segment.destination.point)?;
    let dark = world.btech.maps()[&segment.map].has_flag(super::BattleMapFlag::Dark);
    let (origin_height, destination_height) = if dark
        && matches!(segment.origin.source, EndpointSource::Hex(_))
    {
        (0.0, 0.0)
    } else {
        let origin = super::navigation_measurement::elevation(world, segment.map, &segment.origin)?;
        let destination = if dark && matches!(segment.destination.source, EndpointSource::Hex(_)) {
            f64::from(
                super::unit_elevation(world, segment.observer)?.context("Unit has no elevation")?,
            )
        } else {
            super::navigation_measurement::elevation(world, segment.map, &segment.destination)?
        };
        (origin, destination)
    };
    let spatial = horizontal.hypot((destination_height - origin_height) / 5.0);
    let suffix = super::navigation_measurement::range_text(spatial, horizontal);
    Ok(BattleRangeReport {
        horizontal,
        spatial,
        text: format!("{}{suffix}.", segment.prefix),
    })
}

/// Native range reports are private replies to the invoking occupant.
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
        range_display(&world, unit, ctx.player, &input.args)
    })();
    Ok(crate::CommandAction::Report(crate::CommandReport::Reply(
        match result {
            Ok(report) => report.text,
            Err(error) => format!("{error:#}"),
        },
    )))
}
