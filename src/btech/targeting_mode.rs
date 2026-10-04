//! Scenario-selected tracking modes compose with shared weapon aim for all unit types.
use super::*;
use crate::{ObjectId, World};
use anyhow::Result;

/// Read the saved tracking mode independently of installed computer equipment.
pub(super) fn mode(world: &World, id: ObjectId) -> u8 {
    world.btech.constructed_units().get(&id).map_or_else(
        || {
            world
                .btech
                .vehicles()
                .get(&id)
                .map_or(0, |unit| unit.hardware.targeting_mode)
        },
        |unit| unit.hardware.targeting_mode,
    )
}

/// Apply physical-distance and selected-target terms after network range adjustments.
pub(super) fn apply(
    world: &World,
    source: super::fire_target::TargetSource,
    target: Option<ObjectId>,
    weapon: Weapon,
    ammunition: AmmunitionMode,
    aim: &mut AimModifiers,
) -> Result<()> {
    aim.targeting_mode = match mode(world, source.unit) {
        selected @ (1 | 2) => range_bias(
            selected,
            aim.distance,
            weapon.profile_for_ammunition(ammunition).medium_range,
        ),
        3 => i8::from(super::aim::selected_front(world, source)? == Some(false)),
        4 if target.is_some() => {
            let target = target.unwrap();
            let airborne = super::orbital_drop_state::current(world, target).is_some()
                || world
                    .btech
                    .constructed_units()
                    .get(&target)
                    .is_some_and(|unit| unit.flight().is_some())
                || world
                    .btech
                    .vehicles()
                    .get(&target)
                    .and_then(Vehicle::vtol_flight)
                    .is_some_and(|flight| {
                        matches!(
                            flight.phase,
                            VtolFlightPhase::Airborne | VtolFlightPhase::Falling
                        )
                    });
            if airborne {
                if super::scanner::scanner_unit(world, source.unit).is_some_and(|unit| unit.radar) {
                    -3
                } else {
                    -2
                }
            } else {
                1
            }
        }
        _ => 0,
    };
    Ok(())
}

/// Range bias uses physical distance and the same fractional boundary as conventional aiming.
fn range_bias(mode: u8, distance: f64, medium: u8) -> i8 {
    let long = (distance + 0.95).trunc() > f64::from(medium);
    if long == (mode == 2) { -1 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_bias_changes_at_the_fractional_medium_boundary() {
        for (distance, short) in [(0.0, -1), (6.049, -1), (6.051, 1), (9.0, 1)] {
            assert_eq!(range_bias(1, distance, 6), short);
            assert_eq!(range_bias(2, distance, 6), -short);
        }
    }
}
