//! Shared pickup composition: admission, prior-tow release, preparation, attachment and terrain.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Material effects retained until the host publishes nested injuries and casualties.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[must_use = "Publish notices and nested consequences in the enclosing pickup transaction"]
pub struct BattlePickupReport {
    /// Private terrain-induced checks ordered among pickup notices.
    pub pilot_notices: Vec<BattlePilotNotice>,
    pub notices: Vec<BattleNotice>,
    pub flooding: Vec<BattleSectionExposureReport>,
    pub ice: Option<BattleSurfaceBreak>,
}

/// Tactical pickup; character participants or terrain casualties require the host action.
pub fn pickup_unit(
    world: &mut World,
    carrier: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    rules: BattleFallRules,
    tsm_tow_bonus: bool,
) -> Result<BattlePickupReport> {
    pickup(world, carrier, pilot, target, rules, tsm_tow_bonus, false)
}

/// Compose on an unpublished candidate; every error discards the previous tow release as well.
pub(super) fn pickup(
    world: &mut World,
    carrier: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    rules: BattleFallRules,
    tsm_tow_bonus: bool,
    character: bool,
) -> Result<BattlePickupReport> {
    super::pickup_admission(world, carrier, pilot, target)?;
    ensure!(
        character
            || [carrier, target]
                .iter()
                .all(|id| !world.objects[id].flags.contains(Flag::InCharacter)),
        "Character pickup requires a host action"
    );
    let position = super::scanner::scanner_unit(world, carrier)
        .and_then(|unit| unit.position)
        .context("Carrier is not placed")?;
    let tile =
        world.btech.maps()[&position.map].base_hex(i64::from(position.x), i64::from(position.y))?;
    let source_height =
        super::unit_elevation(world, carrier)?.context("Carrier height unavailable")?;
    let target_height =
        super::unit_elevation(world, target)?.context("Target height unavailable")?;
    let through_ice = tile.terrain() == Terrain::Ice && source_height >= 0 && target_height < 0;
    let hover = world
        .btech
        .vehicles()
        .get(&carrier)
        .is_some_and(|unit| unit.definition().movement == BattleVehicleMovement::Hover);
    let mut candidate = world.clone();
    let mut pilot_notices = Vec::new();
    let mut notices = vec![
        BattleNotice {
            unit: carrier,
            text: "You attach your tow lines to the target.".into(),
        },
        BattleNotice {
            unit: target,
            text: "Tow lines are attached to you.".into(),
        },
    ];
    if candidate.btech.tows().contains_key(&target) {
        notices.extend(super::release_tow(&mut candidate, target)?);
    }
    notices.extend(super::pickup::prepare_target(
        &mut candidate,
        target,
        rules,
    )?);
    notices.extend(super::broadcast::interaction_notices(
        world, carrier, target, "picks up",
    ));
    super::set_tow(&mut candidate, carrier, Some(target))?;
    super::towing::synchronize_pair(&mut candidate, carrier, target)?;
    let mut flooding = Vec::new();
    if let Some(unit) = candidate.btech.vehicles().get(&target) {
        if !unit.is_destroyed() && super::vehicle_water::requires_check(unit, tile, source_height) {
            notices.push(super::vehicle_water::flood(
                &mut candidate,
                target,
                character,
            )?);
        }
    } else {
        flooding = if character {
            super::flooding::flood_unit_in_action(&mut candidate, target, rules)?
        } else {
            super::flood_unit(&mut candidate, target, rules)?
        };
        for flood in &flooding {
            super::piloting::append_feedback(
                &mut pilot_notices,
                flood.pilot_notices.iter().cloned(),
                notices.len(),
            );
            notices.extend(flood.notices.iter().cloned());
        }
    }
    let ice = if through_ice {
        let coordinate = BattleHexCoordinate {
            x: i32::from(position.x),
            y: i32::from(position.y),
        };
        let report = match (source_height == 0 && !hover, character) {
            (true, true) => super::surface_break::break_ice_in_action(
                &mut candidate,
                position.map,
                coordinate,
                Some(carrier),
                rules,
            )?,
            (true, false) => super::break_ice(
                &mut candidate,
                position.map,
                coordinate,
                Some(carrier),
                rules,
            )?,
            (false, true) => super::surface_break::break_ice_upward_in_action(
                &mut candidate,
                position.map,
                coordinate,
                carrier,
                rules,
            )?,
            (false, false) => super::surface_break::break_ice_upward(
                &mut candidate,
                position.map,
                coordinate,
                carrier,
                rules,
            )?,
        };
        super::piloting::append_feedback(
            &mut pilot_notices,
            report.pilot_notices.iter().cloned(),
            notices.len(),
        );
        notices.extend(report.notices.iter().cloned());
        Some(report)
    } else {
        None
    };
    let maximum = super::throttle_maximum(&candidate, carrier, tsm_tow_bonus)?;
    let motion = if let Some(unit) = candidate.btech.vehicles.get_mut(&carrier) {
        unit.motion.as_mut()
    } else {
        candidate
            .btech
            .constructed
            .get_mut(&carrier)
            .and_then(|unit| unit.motion.as_mut())
    };
    if let Some(motion) = motion {
        motion.limit_load(maximum, maximum);
    }
    super::towing::synchronize_pair(&mut candidate, carrier, target)?;
    candidate.btech.validate_action(&candidate)?;
    *world = candidate;
    Ok(BattlePickupReport {
        pilot_notices,
        notices,
        flooding,
        ice,
    })
}
