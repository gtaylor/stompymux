//! Shared vehicle descent composes the forced-descent clock with the vehicle fall transaction
//! and the Total Warfare VTOL crash: speed-based crash damage (p. 68), an explosion when the
//! crash reaches internal structure (p. 198), and the p. 68 outcome that a VTOL still able to
//! fly which crashed where it could have landed has landed, while any other crash destroys it.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// One committed second of forced vehicle descent, before host consequence publication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[must_use = "Publish crash consequences in the enclosing flight action"]
pub enum VehicleDescentEvent {
    Waiting,
    Descending,
    /// Powered flight has arrested descent without impact.
    Recovered,
    Impact {
        levels: u32,
        fall: Box<VehicleFallReport>,
    },
}

/// Advance the saved forced-descent clock against current terrain and resolve impact atomically.
/// This material transaction leaves live admission, immersion and publication to the host.
/// Errors retain the original cursor, altitude, damage and dice for a subsequent retry.
pub fn advance_vtol_fall(
    world: &mut World,
    id: ObjectId,
    rules: FallRules,
) -> Result<VehicleDescentEvent> {
    ensure!(
        world
            .btech
            .vehicles()
            .get(&id)
            .is_some_and(|unit| unit.vtol_flight().is_some()),
        "Aircraft is unavailable"
    );
    advance_in_candidate(world, id, rules, false)
}

/// Advance forced descent for any vehicle using the shared impact and replay boundary.
pub fn advance_vehicle_descent(
    world: &mut World,
    id: ObjectId,
    rules: FallRules,
) -> Result<VehicleDescentEvent> {
    advance_in_candidate(world, id, rules, false)
}

/// Character-capable descent inside the host movement transaction.
fn advance_in_candidate(
    world: &mut World,
    id: ObjectId,
    rules: FallRules,
    character: bool,
) -> Result<VehicleDescentEvent> {
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
    world.attempt(|world| {
        let unit = world.btech.vehicles.get_mut(&id).unwrap();
        let step = if unit.vtol_flight().is_some() {
            unit.advance_vtol_fall(i32::from(surface))?
        } else {
            let fall = unit.free_fall.as_mut().context("Vehicle is not falling")?;
            fall.advance(i32::from(surface))?
        };
        let event = match step {
            FreeFallStep::Recovered => VehicleDescentEvent::Recovered,
            FreeFallStep::Waiting => VehicleDescentEvent::Waiting,
            FreeFallStep::Descending => VehicleDescentEvent::Descending,
            FreeFallStep::Impact { levels } => VehicleDescentEvent::Impact {
                levels,
                fall: Box::new(if world.btech.vehicles()[&id].vtol_flight().is_some() {
                    resolve_in_candidate(world, id, levels, rules, character)?
                } else {
                    let unit = world.btech.vehicles.get_mut(&id).unwrap();
                    unit.free_fall = None;
                    unit.ground_elevation = Some(f64::from(surface));
                    super::vehicle_fall::resolve_material(world, id, levels, rules, character)?
                }),
            },
        };
        Ok(event)
    })
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
    rules: FallRules,
) -> Result<VehicleFallReport> {
    resolve_in_candidate(world, id, levels, rules, false)
}

/// Compose aircraft settlement with the shared character-capable fall material.
pub(super) fn resolve_in_candidate(
    world: &mut World,
    id: ObjectId,
    levels: u32,
    rules: FallRules,
    character: bool,
) -> Result<VehicleFallReport> {
    let levels = i16::try_from(levels).context("Fall severity exceeds pilot-check range")?;
    resolve_signed_in_candidate(world, id, levels, rules, character)
}

/// Resolve the crash itself. Crash damage follows TW p. 68: the hexes the VTOL was covering
/// in a turn (its speed in MP) times its tonnage divided by ten, in five-point groups on the
/// VTOL hit location table. Height does not add damage, so a VTOL hovering when it lost lift
/// takes none. Any crash damage that reaches internal structure explodes the wreck (TW
/// p. 198). A VTOL that is still functional afterwards, with a working rotor and movement,
/// and that could normally land in the hex it crashed in, has landed there and may fly on;
/// otherwise the crash destroys it (TW p. 68), which this engine records as a wrecked rotor.
pub(super) fn resolve_signed_in_candidate(
    world: &mut World,
    id: ObjectId,
    levels: i16,
    rules: FallRules,
    character: bool,
) -> Result<VehicleFallReport> {
    let unit = world
        .btech
        .vehicles()
        .get(&id)
        .context("Aircraft is unavailable")?;
    let flight = unit.vtol_flight().context("Crash requires a VTOL")?;
    ensure!(
        matches!(
            flight.phase,
            VtolFlightPhase::Airborne | VtolFlightPhase::Falling
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
    let hexes = unit
        .motion()
        .map_or(0, |motion| (motion.speed.abs() / 10.75).ceil() as u32);
    world.attempt(|world| {
        let unit = world.btech.vehicles.get_mut(&id).unwrap();
        let was_destroyed = unit.is_destroyed();
        // Clear descent before impact criticals can remove lift again.
        unit.vtol_flight = Some(VtolFlight {
            phase: VtolFlightPhase::Landed,
            altitude: f64::from(height),
            vertical_speed: 0.0,
            fall: None,
        });
        if !was_destroyed && unit.is_destroyed() {
            unit.finish_destruction();
        }
        let mut report = super::vehicle_fall::resolve_material_profile(
            world,
            id,
            i32::from(levels),
            rules,
            character,
            None,
            super::vehicle_fall::ImpactProfile::VtolCrash { hexes },
        )?;
        if rules.vehicle_impact.criticals.combat_safe || super::combat_safe(world, id)? {
            return Ok(report);
        }
        let reached_structure = report.groups.iter().any(|group| {
            group
                .damage
                .as_ref()
                .is_some_and(|damage| damage.overflow > 0)
        });
        if reached_structure && !world.btech.vehicles()[&id].is_destroyed() {
            report.feedback.notices.push(Notice {
                unit: id,
                text: "[fg=red bold]The crash tears the VTOL's frame apart and it explodes![reset]"
                    .into(),
            });
            report
                .feedback
                .notices
                .extend(super::broadcast::observer_notices(
                    world,
                    id,
                    "explodes on impact!",
                ));
            let explosion =
                super::vehicle_explosion::explode_followup_in_candidate(world, id, false)?;
            report.feedback.notices.extend(explosion.notices);
        }
        // The crash may have broken ice or collapsed a roof: judge the landing on the live hex.
        let landable = world
            .btech
            .maps()
            .get(&position.map)
            .context("Aircraft crash map is unavailable")?
            .base_hex(i64::from(position.x), i64::from(position.y))
            .is_ok_and(super::vtol_landing::supported_surface);
        let unit = world.btech.vehicles.get_mut(&id).unwrap();
        let functional =
            !unit.is_destroyed() && !unit.rotor_destroyed() && unit.maximum_speed() > 0.0;
        if functional && landable {
            report.feedback.notices.push(Notice {
                unit: id,
                text: "[fg=yellow]You come down hard, but the VTOL is still flyable.[reset]".into(),
            });
            return Ok(report);
        }
        let rotor_lost = !unit.rotor_destroyed();
        unit.damage_phase(VehicleSection::Rotor, u16::MAX, DamagePhase::Internal)?;
        unit.apply_motive_hit(VehicleMotiveHit::Immobilize);
        if rotor_lost {
            report.feedback.notices.push(Notice {
                unit: id,
                text: "Your rotor is smashed to pieces on impact!".into(),
            });
        }
        report.feedback.notices.push(Notice {
            unit: id,
            text: "[fg=red bold]The VTOL is wrecked.[reset]".into(),
        });
        Ok(report)
    })
}

/// Advance falling vehicles once in stable unit order inside the movement candidate.
/// Reports use the same host publication path as ground-vehicle falls.
pub(super) fn advance_all(
    world: &mut World,
    rules: MovementRules,
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
        let event = advance_in_candidate(world, id, fall_rules, character)?;
        if let VehicleDescentEvent::Impact { fall, .. } = event {
            report.notices.push(Notice {
                unit: id,
                text: "You hit the ground!".into(),
            });
            report.notices.extend(observers);
            super::piloting::append_feedback(
                &mut report.pilot_notices,
                fall.feedback.pilot_notices.iter().cloned(),
                report.notices.len(),
            );
            report.notices.extend(fall.feedback.notices.iter().cloned());
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
    world.attempt(|world| {
        let unit = world.btech.vehicles.get_mut(&id).unwrap();
        unit.halt();
        unit.building_entry = None;
        unit.ground_elevation = None;
        unit.orbital_drop = None;
        if let Some(flight) = &mut unit.vtol_flight {
            flight.phase = VtolFlightPhase::Falling;
            flight.vertical_speed = 0.0;
            flight.fall = Some(FreeFall::at_altitude(altitude)?);
        } else {
            unit.free_fall = Some(FreeFall::at_altitude(altitude)?);
        }
        world.btech.validate_action(world)?;
        Ok(())
    })
}
