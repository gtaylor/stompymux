//! Shared vehicle descent composes the forced-descent clock and vehicle fall transaction.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::sync::Arc;

/// One committed second of forced vehicle descent, before host consequence publication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[must_use = "Publish crash consequences in the enclosing flight action"]
pub enum BattleVehicleDescentEvent {
    Waiting,
    Descending,
    /// Powered flight has arrested descent without impact.
    Recovered,
    Impact {
        levels: u32,
        fall: Box<BattleVehicleFallReport>,
    },
}

/// Advance the saved forced-descent clock against current terrain and resolve impact atomically.
/// This material transaction leaves live admission, immersion and publication to the host.
/// Errors retain the original cursor, altitude, damage and dice for a subsequent retry.
pub fn advance_vtol_fall(
    world: &mut World,
    id: ObjectId,
    rules: BattleFallRules,
) -> Result<BattleVehicleDescentEvent> {
    ensure!(
        world
            .btech
            .vehicles()
            .get(&id)
            .is_some_and(|unit| unit.vtol_flight().is_some()),
        "Aircraft is unavailable"
    );
    advance_in_candidate(world, id, rules, false, false)
}

/// Advance forced descent for any vehicle using the shared impact and replay boundary.
pub fn advance_vehicle_descent(
    world: &mut World,
    id: ObjectId,
    rules: BattleFallRules,
) -> Result<BattleVehicleDescentEvent> {
    advance_in_candidate(world, id, rules, false, false)
}

/// Character-capable descent inside the host movement transaction.
fn advance_in_candidate(
    world: &mut World,
    id: ObjectId,
    rules: BattleFallRules,
    character: bool,
    free_fusion_fuel: bool,
) -> Result<BattleVehicleDescentEvent> {
    let object = world.objects.get(&id).context("Aircraft is unavailable")?;
    ensure!(
        !object.flags.contains(crate::Flag::Going)
            && (character || !object.flags.contains(crate::Flag::InCharacter)),
        "Aircraft descent requires a live unit and character publication when applicable"
    );
    let unit = world
        .btech
        .vehicles()
        .get(&id)
        .context("Aircraft is unavailable")?;
    let position = unit.position().context("Aircraft is not placed")?;
    let tile = world
        .btech
        .maps()
        .get(&position.map)
        .context("Aircraft descent map is unavailable")?
        .base_hex(i64::from(position.x), i64::from(position.y))?;
    let surface = super::fall_profile::surface(tile, unit.elevation_level(tile));
    let mut candidate = world.clone();
    let unit = Arc::make_mut(&mut candidate.btech.vehicles)
        .get_mut(&id)
        .unwrap();
    let step = if unit.vtol_flight().is_some() {
        unit.advance_vtol_fall(i32::from(surface), free_fusion_fuel)?
    } else {
        let fall = unit.free_fall.as_mut().context("Vehicle is not falling")?;
        fall.advance(i32::from(surface))?
    };
    let event = match step {
        BattleFreeFallStep::Recovered => BattleVehicleDescentEvent::Recovered,
        BattleFreeFallStep::Waiting => BattleVehicleDescentEvent::Waiting,
        BattleFreeFallStep::Descending => BattleVehicleDescentEvent::Descending,
        BattleFreeFallStep::Impact { levels } => BattleVehicleDescentEvent::Impact {
            levels,
            fall: Box::new(if candidate.btech.vehicles()[&id].vtol_flight().is_some() {
                resolve_in_candidate(&mut candidate, id, levels, rules, character)?
            } else {
                let unit = Arc::make_mut(&mut candidate.btech.vehicles)
                    .get_mut(&id)
                    .unwrap();
                unit.free_fall = None;
                unit.ground_elevation = Some(f64::from(surface));
                super::vehicle_fall::resolve_material(&mut candidate, id, levels, rules, character)?
            }),
        },
    };
    *world = candidate;
    Ok(event)
}

/// Resolve tactical crash material at the aircraft's current map position atomically.
/// The enclosing flight action owns contact placement, immersion, publication and
/// final world validation.
/// Character aircraft require the host's character consequence transaction.
/// Nested ice and mine effects retain their own admission checks; a rejected
/// consequence rolls back the entire crash, including dice and descent state.
pub fn resolve_vtol_crash(
    world: &mut World,
    id: ObjectId,
    levels: u32,
    rules: BattleFallRules,
) -> Result<BattleVehicleFallReport> {
    resolve_in_candidate(world, id, levels, rules, false)
}

/// Compose aircraft settlement with the shared character-capable fall material.
pub(super) fn resolve_in_candidate(
    world: &mut World,
    id: ObjectId,
    levels: u32,
    rules: BattleFallRules,
    character: bool,
) -> Result<BattleVehicleFallReport> {
    let levels = i16::try_from(levels).context("Fall severity exceeds pilot-check range")?;
    resolve_signed_in_candidate(world, id, levels, rules, character)
}

/// Horizontal terrain rollback may yield signed fall severity; retain it for the crew check.
pub(super) fn resolve_signed_in_candidate(
    world: &mut World,
    id: ObjectId,
    levels: i16,
    rules: BattleFallRules,
    character: bool,
) -> Result<BattleVehicleFallReport> {
    let unit = world
        .btech
        .vehicles()
        .get(&id)
        .context("Aircraft is unavailable")?;
    let flight = unit.vtol_flight().context("Crash requires a VTOL")?;
    ensure!(
        matches!(
            flight.phase,
            BattleVtolFlightPhase::Airborne | BattleVtolFlightPhase::Falling
        ),
        "Crash requires an airborne aircraft"
    );
    let position = unit.position().context("Aircraft is not placed")?;
    let tile = world
        .btech
        .maps()
        .get(&position.map)
        .context("Aircraft crash map is unavailable")?
        .base_hex(i64::from(position.x), i64::from(position.y))?;
    let height = super::fall_profile::surface(tile, unit.elevation_level(tile));
    let mut candidate = world.clone();
    let unit = Arc::make_mut(&mut candidate.btech.vehicles)
        .get_mut(&id)
        .unwrap();
    // Clear descent before impact criticals can remove lift again.
    unit.vtol_flight = Some(BattleVtolFlight {
        phase: BattleVtolFlightPhase::Landed,
        altitude: f64::from(height),
        vertical_speed: 0.0,
        fall: None,
    });
    let mut report =
        super::vehicle_fall::resolve_material_signed(&mut candidate, id, levels, rules, character)?;
    if !rules.vehicle_impact.criticals.combat_safe && !super::battle_combat_safe(&candidate, id)? {
        Arc::make_mut(&mut candidate.btech.vehicles)
            .get_mut(&id)
            .unwrap()
            .apply_motive_hit(BattleVehicleMotiveHit::Immobilize);
        report.notices.push(BattleNotice {
            unit: id,
            text: "Your rotor has been destroyed!".into(),
        });
    }
    *world = candidate;
    Ok(report)
}

/// Advance falling vehicles once in stable unit order inside the movement candidate.
/// Reports use the same host publication path as ground-vehicle falls.
pub(super) fn advance_all(
    world: &mut World,
    rules: BattleMovementRules,
    character: bool,
) -> Result<super::movement_report::MovementReport> {
    let ids: Vec<_> = world
        .btech
        .vehicles()
        .iter()
        .filter_map(|(&id, unit)| {
            (unit.free_fall().is_some()
                && world
                    .objects
                    .get(&id)
                    .is_some_and(|object| !object.flags.contains(crate::Flag::Going)))
            .then_some(id)
        })
        .collect();
    let mut report = super::movement_report::MovementReport::default();
    for id in ids {
        // Earlier impacts can settle other aircraft through a nested terrain effect.
        let unit = &world.btech.vehicles()[&id];
        if unit.free_fall().is_none() {
            continue;
        }
        let mut fall_rules = rules.fall;
        fall_rules.toughness |= unit
            .pilot()
            .is_some_and(|pilot| super::skills::boolean_advantage(world, pilot, "Toughness"));
        let observers = super::broadcast::observer_notices(world, id, "hits the ground!");
        let event = advance_in_candidate(
            world,
            id,
            fall_rules,
            character,
            rules.free_fusion_vtol_fuel,
        )?;
        if let BattleVehicleDescentEvent::Impact { fall, .. } = event {
            report.notices.push(BattleNotice {
                unit: id,
                text: "You hit the ground!".into(),
            });
            report.notices.extend(observers);
            super::piloting::append_feedback(
                &mut report.pilot_notices,
                fall.pilot_notices.iter().cloned(),
                report.notices.len(),
            );
            report.notices.extend(fall.notices.iter().cloned());
            report.vehicle_falls.push(*fall);
        }
    }
    Ok(report)
}

/// Begin forced descent after an enclosing release or terrain action removes support.
/// The caller supplies no height: descent starts at the vehicle's current retained altitude.
/// Tow ownership must be removed first. Publication and later immersion belong to the host.
pub fn begin_vehicle_descent(world: &mut World, id: ObjectId) -> Result<()> {
    super::towing::require_detached(world, id)?;
    begin_descent(world, id)
}

/// Begin physical descent, retaining any external load attached to the falling carrier.
/// Placement and release callers enforce tow admission before entering this transition.
pub(super) fn begin_descent(world: &mut World, id: ObjectId) -> Result<()> {
    ensure!(
        world.btech.towed_by(id).is_none(),
        "Tow target follows its carrier"
    );
    let unit = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle is unavailable")?;
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
        "Vehicle is unavailable"
    );
    ensure!(unit.free_fall().is_none(), "Vehicle is already falling");
    let position = unit.position().context("Vehicle is not placed")?;
    let tile = world
        .btech
        .maps()
        .get(&position.map)
        .context("Map is unavailable")?
        .base_hex(i64::from(position.x), i64::from(position.y))?;
    let elevation = unit.elevation_level(tile);
    let altitude = unit.altitude(tile);
    ensure!(
        elevation > i32::from(super::fall_profile::surface(tile, elevation)),
        "Vehicle is already on the surface"
    );
    let mut candidate = world.clone();
    let unit = Arc::make_mut(&mut candidate.btech.vehicles)
        .get_mut(&id)
        .unwrap();
    unit.halt();
    unit.dig = super::BattleDigState::default();
    unit.building_entry = None;
    unit.ground_elevation = None;
    unit.orbital_drop = None;
    if let Some(flight) = &mut unit.vtol_flight {
        flight.phase = BattleVtolFlightPhase::Falling;
        flight.vertical_speed = 0.0;
        flight.fall = Some(BattleFreeFall::at_altitude(altitude)?);
    } else {
        unit.free_fall = Some(BattleFreeFall::at_altitude(altitude)?);
    }
    candidate.btech.validate(&candidate)?;
    *world = candidate;
    Ok(())
}
