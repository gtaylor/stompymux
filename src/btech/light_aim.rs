//! The map's light as a to-hit modifier: darkness penalties, what searchlights offset, and how
//! a hot Mech stands out. The table itself is [`Light::aim_modifier`](super::Light).
use crate::{ObjectId, World};
use anyhow::{Context, Result};

/// The to-hit modifier the light on `target`'s map adds to an attack on it, a physical attack
/// when `physical`. At night a target that is lit, or has its own searchlight on, is easier to
/// see; a Mech's heat counts against it.
pub(super) fn modifier(world: &World, target: ObjectId, physical: bool) -> Result<i16> {
    let unit = super::scanner::scanner_unit(world, target).context("Target is unavailable")?;
    let Some(position) = unit.position else {
        return Ok(0);
    };
    let light = world
        .btech
        .maps()
        .get(&position.map)
        .context("Map not found")?
        .light_level()?;
    if light == super::Light::Day {
        return Ok(0);
    }
    let lit = light.is_night() && super::unit_illuminated(world, target);
    let heat = world
        .btech
        .constructed_units()
        .get(&target)
        .map(|mech| mech.heat().excess.clamp(0.0, f64::from(u16::MAX)) as u16);
    Ok(light.aim_modifier(physical, lit, heat))
}
