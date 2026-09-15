//! Administrative actual-motion edits preserve requested controls and use shared saved motion.
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use std::sync::Arc;

/// Edit actual motion without advancing time, position or dice; the caller validates and rolls back.
pub(super) fn set(world: &mut World, id: ObjectId, field: &str, value: &str) -> Result<()> {
    let value = if field == "heading" {
        let heading = value
            .trim()
            .parse::<i16>()
            .context("Expected an integer heading")?;
        ensure!(
            (0..360).contains(&heading),
            "Heading must be between 0 and 359"
        );
        f64::from(heading)
    } else {
        let speed = value
            .trim()
            .parse::<f32>()
            .context("Expected a finite speed")?;
        ensure!(speed.is_finite(), "Expected a finite speed");
        f64::from(speed)
    };
    let motion = if let Some(unit) = Arc::make_mut(&mut world.btech.constructed).get_mut(&id) {
        &mut unit.motion
    } else {
        &mut Arc::make_mut(&mut world.btech.vehicles)
            .get_mut(&id)
            .context("Unit is unavailable")?
            .motion
    };
    let motion = motion
        .as_mut()
        .context("Motion edits require a placed unit")?;
    if field == "heading" {
        motion.heading = value;
    } else {
        motion.speed = value;
    }
    Ok(())
}
