//! Engine Damage to a flying VTOL (Total Warfare p. 197): a +4 Driving Skill Roll to land in a
//! clear, paved, rough or building hex, and a crash over anything else or on a failed roll.
use super::{PilotingCheck, VehicleMotiveHit, VtolFlight, VtolFlightPhase};
use crate::{ObjectId, World};
use anyhow::{Context, Result};

/// Driving Skill Roll modifier for an engine-damage landing.
pub const ENGINE_LANDING_MODIFIER: i32 = 4;

/// Resolve the emergency landing or start a forced descent inside the critical candidate.
/// Unsuitable terrain skips the roll and the VTOL falls; missing map data remains a
/// transaction error. `engine_intact` distinguishes rotor hits that merely leave no movement
/// (the VTOL becomes immobile) from a true Engine Hit (energy weapons dead, turret locked).
pub(super) fn engine_landing(
    world: &mut World,
    id: ObjectId,
    extended: bool,
    engine_intact: bool,
) -> Result<Option<PilotingCheck>> {
    let unit = &world.btech.vehicles()[&id];
    let position = unit
        .position()
        .context("Emergency landing requires aircraft placement")?;
    let tile = world
        .btech
        .maps()
        .get(&position.map)
        .context("Aircraft emergency landing map is unavailable")?
        .hex(i64::from(position.x), i64::from(position.y))?;
    let height = tile.surface_height();
    let check = if super::vtol_landing::supported_surface(tile) {
        Some(super::vehicle_piloting::roll(
            world,
            id,
            ENGINE_LANDING_MODIFIER,
            extended,
        )?)
    } else {
        None
    };
    let unit = world.btech.vehicles.get_mut(&id).unwrap();
    if engine_intact {
        unit.apply_motive_hit(VehicleMotiveHit::Immobilize);
    } else {
        unit.disable_engine();
    }
    if !check.as_ref().is_some_and(|check| check.success) {
        unit.lose_vtol_lift();
        return Ok(check);
    }

    unit.vtol_flight = Some(VtolFlight {
        fall: None,
        phase: VtolFlightPhase::Landed,
        altitude: f64::from(height),
        vertical_speed: 0.0,
    });
    unit.ground_elevation = None;
    unit.under_bridge = false;
    Ok(check)
}
