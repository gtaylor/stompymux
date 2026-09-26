//! Reserved script movement values are persisted independently of physical movement limits.
use crate::{ObjectId, World};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Administrative base-speed fields have no movement consumer and default to zero.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct BaseMovementFields {
    pub walk: i32,
    pub run: i32,
}

/// Read the same field storage for Mechs and vehicles without altering simulation state.
pub(super) fn read(world: &World, id: ObjectId) -> Result<BaseMovementFields> {
    if let Some(unit) = world.btech.constructed_units().get(&id) {
        return Ok(unit.base_movement_fields);
    }
    Ok(world
        .btech
        .vehicles()
        .get(&id)
        .context("Unit is unavailable")?
        .base_movement_fields)
}

/// Validate before selecting storage; the caller owns authorization and transaction publication.
pub(super) fn set(world: &mut World, id: ObjectId, walking: bool, value: &str) -> Result<()> {
    let value = value
        .trim()
        .parse::<i32>()
        .context("Expected a signed 32-bit integer")?;
    let fields = if let Some(unit) = world.btech.constructed.get_mut(&id) {
        &mut unit.base_movement_fields
    } else {
        &mut world
            .btech
            .vehicles
            .get_mut(&id)
            .context("Unit is unavailable")?
            .base_movement_fields
    };
    if walking {
        fields.walk = value;
    } else {
        fields.run = value;
    }
    Ok(())
}
