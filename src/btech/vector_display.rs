//! Combined cockpit range, compass bearing and optional elevation-angle measurements.
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// A read-only vector; vertical bearing is signed and rounded away from zero.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VectorReport {
    pub horizontal: f64,
    pub spatial: f64,
    pub bearing: u16,
    pub vertical_bearing: i16,
    pub text: String,
}

/// Resolve default targets, x/y pairs, or x/y/z triples using live signed elevations.
/// Unlike range reports, vector reports retain terrain heights on dark maps.
pub fn vector_display(
    world: &World,
    unit: ObjectId,
    viewer: ObjectId,
    arguments: &str,
) -> Result<VectorReport> {
    let words: Vec<_> = arguments.split_whitespace().collect();
    ensure!(
        matches!(words.len(), 0 | 2 | 3 | 4 | 6),
        "Invalid number of attributes to Vector function!"
    );
    let values: Vec<i32> = words
        .iter()
        .map(|word| word.parse().context("Invalid map coordinates!"))
        .collect::<Result<_>>()?;
    let (coordinates, origin_height, destination_height, prefix) = match values.as_slice() {
        [x, y, z] => (
            format!("{x} {y}"),
            None,
            Some(f64::from(*z)),
            Some(format!("Vector to  {x},{y},{z} is: ")),
        ),
        [x0, y0, z0, x1, y1, z1] => (
            format!("{x0} {y0} {x1} {y1}"),
            Some(f64::from(*z0)),
            Some(f64::from(*z1)),
            Some(format!("Vector to {x1},{y1},{z1} from {x0},{y0},{z0} is: ")),
        ),
        _ => (arguments.to_owned(), None, None, None),
    };
    let segment =
        super::navigation_measurement::resolve(world, unit, viewer, &coordinates, "Vector")?;
    let origin_height = match origin_height {
        Some(height) => height,
        None => super::navigation_measurement::elevation(world, segment.map, &segment.origin)?,
    };
    let destination_height = match destination_height {
        Some(height) => height,
        None => super::navigation_measurement::elevation(world, segment.map, &segment.destination)?,
    };
    let horizontal = segment.origin.point.range(segment.destination.point)?;
    let difference = (destination_height - origin_height) / 5.0;
    let spatial = horizontal.hypot(difference);
    let bearing = segment
        .origin
        .point
        .bearing(segment.destination.point)?
        .unwrap_or(180.0)
        .round()
        .rem_euclid(360.0) as u16;
    let vertical = difference.abs().atan2(horizontal).to_degrees().ceil() as i16;
    let vertical_bearing = if difference < 0.0 {
        -vertical
    } else {
        vertical
    };
    let mark = if matches!(values.len(), 0 | 3 | 6) {
        let sign = if difference > 0.0 {
            '+'
        } else if difference < 0.0 {
            '-'
        } else {
            ' '
        };
        format!(" mark {sign}{vertical}")
    } else {
        String::new()
    };
    let prefix = prefix.unwrap_or(segment.prefix);
    let distance = super::navigation_measurement::range_text(spatial, horizontal);
    Ok(VectorReport {
        horizontal,
        spatial,
        bearing,
        vertical_bearing,
        text: format!("{prefix}{distance} and {bearing} degrees{mark}."),
    })
}

/// Native vectors are private replies to the invoking cockpit occupant.
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
        vector_display(&world, unit, ctx.player, &input.args)
    })();
    Ok(crate::CommandAction::Report(crate::CommandReport::Reply(
        match result {
            Ok(report) => report.text,
            Err(error) => format!("{error:#}"),
        },
    )))
}
