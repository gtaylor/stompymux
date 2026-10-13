//! Inferno missiles against vehicles (Total Warfare p. 142): one roll on the critical hits table
//! per missile at −2, plus the stationary-unit jelly timer that infernos and blast heat feed.
use super::*;
use crate::{Config, Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Modifier to each inferno missile's critical roll against a Combat Vehicle (TW p. 142).
pub const INFERNO_CRITICAL_MODIFIER: i8 = -2;

/// One inferno missile's hit location and the critical roll it caused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VehicleInfernoCritical {
    pub location_roll: u8,
    pub section: VehicleSection,
    pub critical: VehicleCriticalResolution,
}

/// Target-owned inferno outcome after missile clustering and interception.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish inferno effects with the enclosing attack"]
pub struct VehicleInfernoHit {
    pub missiles: u16,
    /// Per-missile critical rolls against a mobile vehicle, in missile order.
    pub criticals: Vec<VehicleInfernoCritical>,
    /// Stationary-unit jelly duration added by this exposure.
    pub burn_seconds: u32,
    /// Includes visibility-filtered observer notices captured before damage.
    pub notices: Vec<Notice>,
    /// Pilot-only control messages indexed into the ordinary notice stream.
    pub pilot_notices: Vec<PilotNotice>,
    /// Damage broadcasts whose audiences are resolved by the enclosing host action.
    pub broadcasts: Vec<Notice>,
}

impl Vehicle {
    /// Remaining stationary-unit inferno duration, independent of passive weapon heat.
    pub fn inferno_remaining(&self) -> u32 {
        self.inferno_remaining
    }
}

/// Apply positive inferno exposure atomically. Scripted exposures strike the front.
pub fn resolve_vehicle_inferno_hit(
    world: &mut World,
    target: ObjectId,
    missiles: u16,
    rules: VehicleImpactRules,
) -> Result<VehicleInfernoHit> {
    resolve_inferno_from(world, target, HitArc::Front, missiles, rules, None)
}

/// Initial missile exposure retains its author and attack direction.
pub(super) fn resolve_inferno_from(
    world: &mut World,
    target: ObjectId,
    arc: HitArc,
    missiles: u16,
    rules: VehicleImpactRules,
    attacker: Option<ObjectId>,
) -> Result<VehicleInfernoHit> {
    ensure!(missiles > 0, "Inferno hit requires at least one missile");
    let object = world
        .objects
        .get(&target)
        .context("Vehicle is unavailable")?;
    ensure!(
        !object.flags.contains(Flag::Going),
        "Vehicle is unavailable"
    );
    let unit = world
        .btech
        .vehicles()
        .get(&target)
        .context("Vehicle is unavailable")?;
    ensure!(!unit.is_destroyed(), "Vehicle is destroyed");
    let stationary = unit.definition().movement == VehicleMovement::Stationary;
    let burning = unit.inferno_remaining > 0;
    let mut report = VehicleInfernoHit {
        missiles,
        criticals: Vec::new(),
        burn_seconds: 0,
        notices: super::inferno_hit::exposure_notices(world, target, burning),
        pilot_notices: Vec::new(),
        broadcasts: Vec::new(),
    };
    world.attempt(|world| {
        if stationary {
            report.burn_seconds = super::inferno_hit::duration(missiles);
            add_jelly(world, target, i64::from(report.burn_seconds))?;
            world.btech.validate_action(world)?;
            return Ok(report);
        }
        let safe = rules.criticals.combat_safe
            || world.btech.vehicles()[&target].combat_safe
            || super::combat_safe::protects(world, attacker, target);
        if safe {
            report
                .notices
                .extend(super::combat_safe::notice(attacker, target));
            world.btech.validate_action(world)?;
            return Ok(report);
        }
        for _ in 0..missiles {
            let vehicle = &world.btech.vehicles()[&target];
            if vehicle.is_destroyed() {
                break;
            }
            let mut dice = vehicle.dice.clone();
            let location_roll = dice.generic_roll();
            let section = if vehicle.definition().is_vtol() {
                vehicle.vtol_hit_location(arc, location_roll)?
            } else {
                vehicle.hit_location(arc, location_roll)?
            };
            world.btech.vehicles.get_mut(&target).unwrap().dice = dice;
            report.notices.push(Notice {
                unit: target,
                text: format!(
                    "[fg=red bold]Burning inferno gel splashes across your {}![reset]",
                    section.name().replace('_', " ")
                ),
            });
            let critical = super::vehicle_critical_resolution::resolve_modified_in_candidate(
                world,
                target,
                section,
                rules.criticals,
                super::vehicle_internal_damage::DamageContext {
                    attacker,
                    ..Default::default()
                },
                INFERNO_CRITICAL_MODIFIER,
            )?;
            super::piloting::append_feedback(
                &mut report.pilot_notices,
                critical.pilot_notices.iter().cloned(),
                report.notices.len(),
            );
            report.notices.extend(critical.notices.clone());
            report.broadcasts.extend(critical.broadcasts.clone());
            report.criticals.push(VehicleInfernoCritical {
                location_roll,
                section,
                critical,
            });
        }
        world.btech.validate_action(world)?;
        Ok(report)
    })
}

/// Vehicle response to blast heat. Vehicles have no heat scale, so only the stationary-unit
/// jelly timer changes; mobile vehicles take no effect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish heat effects with the enclosing damage transaction"]
pub struct VehicleHeatExposure {
    /// Signed duration adjustment; the resulting timer is bounded to at least one second.
    pub burn_seconds: i64,
    pub notices: Vec<Notice>,
    /// Pilot-only control messages indexed into the ordinary notice stream.
    pub pilot_notices: Vec<PilotNotice>,
    pub broadcasts: Vec<Notice>,
}

/// Apply blast heat to a vehicle; blasts can reach surviving wreck sections of stationary units.
pub fn resolve_vehicle_heat_exposure(
    world: &mut World,
    target: ObjectId,
    heat: i32,
    _rules: VehicleImpactRules,
) -> Result<VehicleHeatExposure> {
    let object = world
        .objects
        .get(&target)
        .context("Vehicle is unavailable")?;
    ensure!(
        !object.flags.contains(Flag::Going),
        "Vehicle is unavailable"
    );
    let unit = world
        .btech
        .vehicles()
        .get(&target)
        .context("Vehicle is unavailable")?;
    let stationary = unit.definition().movement == VehicleMovement::Stationary;
    world.attempt(|world| {
        let mut report = VehicleHeatExposure {
            burn_seconds: 0,
            notices: Vec::new(),
            pilot_notices: Vec::new(),
            broadcasts: Vec::new(),
        };
        if stationary {
            report.burn_seconds = i64::from(heat) * 6;
            add_jelly(world, target, report.burn_seconds)?;
        }
        world.btech.validate_action(world)?;
        Ok(report)
    })
}

/// Jelly accounting is shared by missile exposure and blast heat.
fn add_jelly(world: &mut World, target: ObjectId, seconds: i64) -> Result<()> {
    let unit = world.btech.vehicles.get_mut(&target).unwrap();
    unit.inferno_remaining = adjusted_duration(unit.inferno_remaining, seconds)?;
    Ok(())
}

/// Signed exposures reschedule an active burn, retaining at least one committed tick.
fn adjusted_duration(remaining: u32, seconds: i64) -> Result<u32> {
    if seconds == 0 {
        return Ok(remaining);
    }
    let duration = (i64::from(remaining) + seconds).max(1);
    ensure!(
        duration <= i64::from(i32::MAX),
        "Inferno duration exceeds its limit"
    );
    Ok(duration as u32)
}

/// Scheduled jelly countdown feedback for host publication.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[must_use = "Publish fire notices with the enclosing action"]
pub struct VehicleFireTick {
    pub notices: Vec<Notice>,
}

/// Advance every stationary unit's inferno jelly by one second, including shutdown units.
pub fn advance_vehicle_fires(world: &mut World, _config: &Config) -> Result<VehicleFireTick> {
    let mut notices = Vec::new();
    for (&id, unit) in world.btech.vehicles.iter_mut() {
        if unit.inferno_remaining == 0
            || world
                .objects
                .get(&id)
                .is_none_or(|object| object.flags.contains(Flag::Going))
        {
            continue;
        }
        unit.inferno_remaining -= 1;
        if unit.inferno_remaining == 0 {
            notices.push(Notice {
                unit: id,
                text: "You feel suddenly far cooler as the fires finally die.".into(),
            });
        }
    }
    Ok(VehicleFireTick { notices })
}
