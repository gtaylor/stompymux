//! Engine-loss emergency landing uses shared pilot checks and vehicle propulsion damage.
use super::{BattlePilotingCheck, BattleVtolFlight, BattleVtolFlightPhase};
use crate::{ObjectId, World};
use anyhow::{Context, Result};

/// Resolve emergency landing or start a forced descent inside the critical candidate.
/// Unsuitable terrain skips the pilot check; missing map data remains a transaction error.
pub(super) fn engine_landing(
    world: &mut World,
    id: ObjectId,
    extended: bool,
    advanced: bool,
) -> Result<Option<BattlePilotingCheck>> {
    let unit = &world.btech.vehicles()[&id];
    let position = unit
        .position()
        .context("Emergency landing requires aircraft placement")?;
    let tile = world
        .btech
        .maps()
        .get(&position.map)
        .context("Aircraft emergency landing map is unavailable")?
        .base_hex(i64::from(position.x), i64::from(position.y))?;
    let height = tile.surface_height();
    let check = if super::vtol_landing::supported_surface(tile) {
        let modifier = i16::try_from(unit.elevation_level(tile) - i32::from(height))
            .context("Emergency landing height exceeds pilot-check range")?;
        Some(super::vehicle_piloting::roll(
            world,
            id,
            i32::from(modifier),
            extended,
        )?)
    } else {
        None
    };
    let unit = world.btech.vehicles.get_mut(&id).unwrap();
    unit.disable_engine(advanced);
    if !check.as_ref().is_some_and(|check| check.success) {
        unit.lose_vtol_lift();
        return Ok(check);
    }

    unit.vtol_flight = Some(BattleVtolFlight {
        fall: None,
        phase: BattleVtolFlightPhase::Landed,
        altitude: f64::from(height),
        vertical_speed: 0.0,
    });
    unit.ground_elevation = None;
    unit.under_bridge = false;
    Ok(check)
}
