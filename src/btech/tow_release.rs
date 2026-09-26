//! Shared tow release settles carried material or starts its existing forced-descent clock.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Release a tow atomically after the caller establishes authority over the carrier.
/// The same operation serves operator dropoff and pickup of a target that carries a tow.
/// Returned notices belong to the enclosing publication transaction.
pub fn release_tow(world: &mut World, carrier: ObjectId) -> Result<Vec<BattleNotice>> {
    let target = *world
        .btech
        .tows()
        .get(&carrier)
        .context("You aren't carrying a unit!")?;
    for id in [carrier, target] {
        ensure!(
            world
                .objects
                .get(&id)
                .is_some_and(|object| !object.flags.contains(Flag::Going)),
            "Tow participant is unavailable"
        );
    }
    let position = super::scanner::scanner_unit(world, target)
        .and_then(|unit| unit.position)
        .context("Tow target is not placed")?;
    let tile = world
        .btech
        .maps()
        .get(&position.map)
        .context("Map is unavailable")?
        .base_hex(i64::from(position.x), i64::from(position.y))?;
    let height =
        super::unit_elevation(world, target)?.context("Tow target height is unavailable")?;
    let altitude =
        super::unit_altitude(world, target)?.context("Tow target altitude is unavailable")?;
    let surface = if tile.terrain == Terrain::Ice {
        0
    } else {
        super::fall_profile::surface(tile, height)
    };
    let falling = i64::from(height) > i64::from(surface) + 2;
    let mut candidate = world.clone();
    super::towing::detach(&mut candidate, carrier);
    if candidate.btech.vehicles().contains_key(&target) {
        if falling {
            super::begin_vehicle_descent(&mut candidate, target)?;
        } else {
            let unit = candidate.btech.vehicles.get_mut(&target).unwrap();
            unit.under_bridge = false;
            if let Some(flight) = &mut unit.vtol_flight {
                *flight = BattleVtolFlight {
                    altitude: f64::from(surface),
                    ..Default::default()
                };
                unit.ground_elevation = None;
            } else {
                unit.ground_elevation = Some(f64::from(surface));
            }
        }
    } else {
        let unit = candidate
            .btech
            .constructed
            .get_mut(&target)
            .context("Tow target is unavailable")?;
        unit.ground_elevation = (!falling).then_some(f64::from(surface));
        if falling {
            unit.free_fall = Some(BattleFreeFall::at_altitude(altitude)?);
            unit.jump_stabilization = 0;
            unit.stand_timer = None;
        }
    }
    let mut notices = vec![
        BattleNotice {
            unit: carrier,
            text: "You drop the unit you were carrying.".into(),
        },
        BattleNotice {
            unit: target,
            text: "You have been released from towing.".into(),
        },
    ];
    notices.extend(super::broadcast::interaction_notices(
        world, carrier, target, "drops",
    ));
    if falling {
        notices.push(BattleNotice {
            unit: carrier,
            text: "Maybe you should have done this closer to the ground.".into(),
        });
        notices.push(BattleNotice {
            unit: target,
            text: "You wish they had done that a bit closer to the ground.".into(),
        });
        notices.extend(super::broadcast::observer_notices(
            &candidate,
            target,
            "falls through the sky.",
        ));
    }
    candidate.btech.validate_action(&candidate)?;
    *world = candidate;
    Ok(notices)
}
