//! The map's light as a to-hit modifier: darkness penalties, what searchlights offset, how a
//! hot Mech stands out, and the glow of laser heat sinks. The table itself is
//! [`Light::aim_modifier`](super::Light).
use crate::{ObjectId, World};
use anyhow::{Context, Result};

/// The to-hit modifier the light on `target`'s map adds to an attack on it, a physical attack
/// when `physical`. A searchlight on the target, or its own lamp, offsets the night; fire
/// around it takes up to two off; a Mech's heat counts against it, and so do its laser heat
/// sinks when it makes heat (see [`laser_heat_sinks`]).
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
    let illumination = super::searchlight::unit_illumination(world, target);
    let mech = world.btech.constructed_units().get(&target);
    let heat = mech.map(|mech| mech.heat().excess.clamp(0.0, f64::from(u16::MAX)) as u16);
    let darkness = light.aim_modifier(physical, illumination, heat);
    let Some(mech) = mech.filter(|mech| {
        mech.definition()
            .has_technology(super::Technology::LaserHeatSinks)
    }) else {
        return Ok(darkness);
    };
    Ok(laser_heat_sinks(
        darkness,
        physical,
        mech.heat().excess > 0.0,
        made_heat_this_turn(mech, world.btech.simulation_time()),
    ))
}

/// The night or dusk `darkness` modifier against a 'Mech with laser heat sinks (Tactical
/// Operations: Advanced Units & Equipment p. 129). Once its heat scale rises above zero
/// (`hot`) the modifier no longer applies to any attack; otherwise a turn in which it took an
/// action that makes heat takes one off it for weapon attacks, never below zero.
fn laser_heat_sinks(darkness: i16, physical: bool, hot: bool, made_heat: bool) -> i16 {
    if hot {
        return 0;
    }
    if made_heat && !physical {
        return (darkness - 1).max(0);
    }
    darkness
}

/// Whether the 'Mech took an action this turn that makes heat: it is moving, or within the
/// last turn it jumped or fired a weapon. Moving now is the realtime reading of having moved
/// this turn, as for seismic sensors.
fn made_heat_this_turn(mech: &super::Mech, now: i64) -> bool {
    mech.motion().is_some_and(|motion| motion.speed != 0.0)
        || mech.jumped_this_turn(now)
        || mech.fired_this_turn(now)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Heat on the scale cancels the darkness modifier for every attack; a heat-making turn
    /// takes one off a weapon attack's, never below zero (TO:AUE p. 129).
    #[test]
    fn laser_heat_sinks_follow_heat_and_actions() {
        assert_eq!(laser_heat_sinks(3, false, false, false), 3);
        assert_eq!(laser_heat_sinks(3, false, false, true), 2);
        assert_eq!(laser_heat_sinks(1, true, false, true), 1);
        assert_eq!(laser_heat_sinks(0, false, false, true), 0);
        assert_eq!(laser_heat_sinks(4, false, true, false), 0);
        assert_eq!(laser_heat_sinks(2, true, true, true), 0);
    }
}
