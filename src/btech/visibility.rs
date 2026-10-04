//! Operator visibility overrides, distinct from physical terrain and ordinary sensor acquisition.
use crate::{Flag, Kind, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Independent scenario privileges shared by every supported chassis.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Visibility {
    /// Ordinary sensors cannot acquire this unit.
    pub invisible: bool,
    /// Visibility queries bypass sensor acquisition and terrain obstruction.
    pub clairvoyant: bool,
}

/// Inspect operator visibility state without requiring a running cockpit.
pub fn visibility(world: &World, id: ObjectId) -> Result<Visibility> {
    let unit = world.btech.unit(id).context("Unit is not constructed")?;
    Ok(unit.visibility())
}

/// Replace the scenario's visibility flags; the caller owns authority and transaction publication.
pub fn set_battle_visibility(
    world: &mut World,
    id: ObjectId,
    visibility: Visibility,
) -> Result<()> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|o| o.kind == Kind::Thing && !o.flags.contains(Flag::Going)),
        "Unit must be a live thing"
    );
    super::with_unit_mut!(
        world
            .btech
            .unit_mut(id)
            .context("Unit is not constructed")?,
        |unit| {
            unit.visibility = visibility;
            Ok(())
        }
    )
}

/// Unblocked unit visibility honors clairvoyance while preserving physical geometry reports.
pub(super) fn unit_unblocked(world: &World, observer: ObjectId, target: ObjectId) -> Result<bool> {
    let terrain = super::unit_terrain_los(world, observer, target)?;
    Ok(visibility(world, observer)?.clairvoyant || !terrain.blocked)
}

/// Coordinate visibility bypasses obstruction only after validating the map and coordinate.
pub(super) fn hex_unblocked(
    world: &World,
    observer: ObjectId,
    target: super::HexCoordinate,
) -> Result<bool> {
    let (terrain, _) = super::los::unit_hex_los(world, observer, target)?;
    Ok(visibility(world, observer)?.clairvoyant || !terrain.blocked)
}
