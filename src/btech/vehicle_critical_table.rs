//! The Total Warfare Ground Combat Vehicle and VTOL Combat Vehicle Critical Hits Tables:
//! column selection, inapplicable results moving down the column, and the admission policy
//! that travels with a damage transaction. Consequences are applied elsewhere.
use super::{Dice, RotorHit, Vehicle, VehicleMovement, VehicleSection};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Admission supplied by the surrounding combat action; the tables themselves never vary.
#[derive(Debug, Clone, Copy)]
pub struct VehicleCriticalRules {
    pub combat_safe: bool,
    /// Use the toughness consciousness rule when applying crew injuries.
    pub toughness: bool,
    /// Pilot-skill selection for aircraft engine-loss emergency landing checks.
    pub extended_piloting: bool,
}

/// One row of a critical hits table (Total Warfare pp. 194 and 196, Tactical Operations:
/// Advanced Rules p. 193 for the VTOL turret column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VehicleCriticalEffect {
    /// Driver Hit: +2 to Driving Skill Rolls; a second hit is Crew Stunned.
    Driver,
    /// VTOL Pilot Hit: +2 to Driving Skill Rolls and an immediate control roll; a second hit is Crew Killed.
    VtolPilot,
    /// VTOL Co-Pilot Hit: +1 to to-hit rolls; a second hit is Crew Killed.
    VtolCopilot,
    /// Commander Hit: Crew Stunned plus a permanent +1 to-hit and +1 driving; later hits are Crew Stunned.
    Commander,
    /// Crew Stunned: cruising speed only and no actions for one turn; with Driver and Commander hits it kills the crew.
    CrewStunned,
    CrewKilled,
    /// Weapon Malfunction: one weapon in the location cannot fire until the crew clears it.
    WeaponJam,
    WeaponDestroyed,
    /// Weapon stabilizers in the location are destroyed, doubling the attacker movement modifier.
    Stabilizer,
    /// Sensors: +1 to-hit per hit; the fourth hit stops all weapon fire.
    Sensors,
    /// Engine Hit: immobile, direct-fire energy weapons dead, turret locked.
    Engine,
    /// Fuel Tank: an internal-combustion vehicle explodes; fusion vehicles take an Engine Hit instead.
    FuelTank,
    /// Ammunition: every bin explodes into the location, or into the rear armor with CASE.
    Ammunition,
    /// Cargo/Infantry Hit: no cargo bay model exists, so this row is always skipped.
    Cargo,
    TurretJam,
    TurretLock,
    /// Turret Blown Off destroys the vehicle.
    TurretBlownOff,
    /// Rotors column outcome, applied by the shared critical transaction.
    Rotor(RotorHit),
}

/// A table result with its actual random draw, still awaiting damage and notification effects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Apply the selected critical consequence within the enclosing damage transaction"]
pub struct VehicleCriticalReport {
    pub section: VehicleSection,
    /// The 2d6 table roll before modifiers; empty when no roll was made.
    pub rolls: Vec<u8>,
    /// Armor-piercing, inferno and hardened-armor adjustment applied to the roll.
    pub modifier: i8,
    pub effect: Option<VehicleCriticalEffect>,
}

/// Select one critical using the victim's saved dice; no equipment or crew damage is applied here.
pub fn roll_vehicle_critical(
    world: &mut World,
    id: ObjectId,
    section: VehicleSection,
    rules: VehicleCriticalRules,
) -> Result<VehicleCriticalReport> {
    roll_vehicle_critical_modified(world, id, section, rules, 0)
}

/// Select one critical with a roll modifier, for armor-piercing, inferno and hardened-armor hits.
pub fn roll_vehicle_critical_modified(
    world: &mut World,
    id: ObjectId,
    section: VehicleSection,
    rules: VehicleCriticalRules,
    modifier: i8,
) -> Result<VehicleCriticalReport> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Vehicle is unavailable"
    );
    let vehicle = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle is unavailable")?;
    ensure!(!vehicle.is_destroyed(), "Vehicle is destroyed");
    ensure!(
        vehicle
            .sections()
            .get(&section)
            .is_some_and(|state| state.internal > 0),
        "Vehicle section is unavailable"
    );
    let mut dice = vehicle.dice.clone();
    let report = select(vehicle, section, rules, modifier, &mut dice);
    world.btech.vehicles.get_mut(&id).unwrap().dice = dice;
    Ok(report)
}

/// Keep early-return rules and die sequencing independent from consequence application.
fn select(
    vehicle: &Vehicle,
    section: VehicleSection,
    rules: VehicleCriticalRules,
    modifier: i8,
    dice: &mut Dice,
) -> VehicleCriticalReport {
    let mut report = VehicleCriticalReport {
        section,
        rolls: Vec::new(),
        modifier,
        effect: None,
    };
    // Fixed ground emplacements and critical-proof scenario props have no critical table;
    // a tethered observation VTOL still uses the VTOL table.
    let vtol = vehicle.definition().is_vtol();
    if rules.combat_safe
        || vehicle.combat_safe
        || vehicle.definition().has_special("CritProof_Tech")
        || (!vtol && vehicle.definition().movement == VehicleMovement::Stationary)
    {
        return report;
    }
    let Some(rows) = column(vtol, section) else {
        return report;
    };
    let roll = dice.generic_roll();
    report.rolls.push(roll);
    let adjusted = i16::from(roll) + i16::from(modifier);
    if adjusted < 6 {
        return report;
    }
    report.effect = applicable(
        vehicle,
        section,
        &rows,
        usize::from(adjusted.min(12) as u8 - 6),
    );
    report
}

/// Move down the column from the rolled row, wrapping from 12 back to 6, until a result applies.
/// A column whose every result has already been taken ignores the critical hit (TW p. 194).
fn applicable(
    vehicle: &Vehicle,
    section: VehicleSection,
    rows: &[VehicleCriticalEffect; 7],
    start: usize,
) -> Option<VehicleCriticalEffect> {
    (0..rows.len())
        .map(|step| substitute(vehicle, section, rows[(start + step) % rows.len()]))
        .find(|effect| applies(vehicle, section, *effect))
}

/// Footnote substitutions: Fuel Tank on a fusion vehicle is an Engine Hit, and Ammunition
/// without ammunition is Weapon Destroyed.
fn substitute(
    vehicle: &Vehicle,
    section: VehicleSection,
    effect: VehicleCriticalEffect,
) -> VehicleCriticalEffect {
    use VehicleCriticalEffect as E;
    match effect {
        E::FuelTank if !vehicle.definition().has_special("ICEEngine_Tech") => E::Engine,
        E::Ammunition if !has_ammunition(vehicle, section) => E::WeaponDestroyed,
        other => other,
    }
}

/// Whether a result can still affect this location: the item exists and has not already taken
/// the one hit it can take.
fn applies(vehicle: &Vehicle, section: VehicleSection, effect: VehicleCriticalEffect) -> bool {
    use VehicleCriticalEffect as E;
    match effect {
        E::Driver
        | E::VtolPilot
        | E::VtolCopilot
        | E::Commander
        | E::CrewStunned
        | E::CrewKilled
        | E::TurretBlownOff
        | E::FuelTank
        | E::Rotor(RotorHit::Damage | RotorHit::Destroy) => true,
        E::Rotor(RotorHit::FlightStabilizer) => !vehicle.tail_rotor_destroyed(),
        E::WeaponJam => vehicle
            .weapon_critical_candidates(section)
            .is_ok_and(|candidates| {
                candidates
                    .iter()
                    .any(|index| !vehicle.weapon_failures().contains_key(index))
            }),
        E::WeaponDestroyed => vehicle
            .weapon_critical_candidates(section)
            .is_ok_and(|candidates| !candidates.is_empty()),
        E::Stabilizer => !vehicle.lost_stabilizers().contains(&section),
        E::Sensors => vehicle.sensor_hits() < super::vehicle_control_damage::SENSOR_HITS_TO_BLIND,
        E::Engine => !vehicle.engine_destroyed(),
        E::Ammunition => has_ammunition(vehicle, section),
        E::Cargo => false,
        E::TurretJam | E::TurretLock => {
            vehicle.turret_heading().is_some() && !vehicle.turret_locked()
        }
    }
}

/// Ammunition results need at least one live, reachable bin anywhere on the vehicle.
fn has_ammunition(vehicle: &Vehicle, section: VehicleSection) -> bool {
    vehicle
        .ammunition_cascade(section)
        .is_ok_and(|cascade| cascade.damage > 0)
}

/// Rows 6 through 12 of the column for the struck location; rolls of 2–5 are no critical hit.
fn column(vtol: bool, section: VehicleSection) -> Option<[VehicleCriticalEffect; 7]> {
    use RotorHit as R;
    use VehicleCriticalEffect as E;
    use VehicleSection as S;
    // Total Warfare p. 194 (ground) and p. 196 (VTOL); the turret column is shared with the
    // VTOL turret column of Tactical Operations: Advanced Rules p. 193.
    let turret = [
        E::Stabilizer,
        E::TurretJam,
        E::WeaponJam,
        E::TurretLock,
        E::WeaponDestroyed,
        E::Ammunition,
        E::TurretBlownOff,
    ];
    if vtol {
        return Some(match section {
            S::Front => [
                E::VtolCopilot,
                E::WeaponJam,
                E::Stabilizer,
                E::Sensors,
                E::VtolPilot,
                E::WeaponDestroyed,
                E::CrewKilled,
            ],
            S::Left | S::Right => [
                E::WeaponJam,
                E::Cargo,
                E::Stabilizer,
                E::WeaponDestroyed,
                E::Engine,
                E::Ammunition,
                E::FuelTank,
            ],
            S::Rear => [
                E::Cargo,
                E::WeaponJam,
                E::Stabilizer,
                E::WeaponDestroyed,
                E::Sensors,
                E::Engine,
                E::FuelTank,
            ],
            S::Rotor => [
                E::Rotor(R::Damage),
                E::Rotor(R::Damage),
                E::Rotor(R::Damage),
                E::Rotor(R::FlightStabilizer),
                E::Rotor(R::FlightStabilizer),
                E::Rotor(R::Destroy),
                E::Rotor(R::Destroy),
            ],
            S::Turret => turret,
        });
    }
    Some(match section {
        S::Front => [
            E::Driver,
            E::WeaponJam,
            E::Stabilizer,
            E::Sensors,
            E::Commander,
            E::WeaponDestroyed,
            E::CrewKilled,
        ],
        S::Left | S::Right => [
            E::Cargo,
            E::WeaponJam,
            E::CrewStunned,
            E::Stabilizer,
            E::WeaponDestroyed,
            E::Engine,
            E::FuelTank,
        ],
        S::Rear => [
            E::WeaponJam,
            E::Cargo,
            E::Stabilizer,
            E::WeaponDestroyed,
            E::Engine,
            E::Ammunition,
            E::FuelTank,
        ],
        S::Turret => turret,
        // Ground vehicles have no rotor location.
        S::Rotor => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turret_column_lists_ammunition_before_turret_blown_off() {
        let rows = column(false, VehicleSection::Turret).unwrap();
        assert_eq!(rows[5], VehicleCriticalEffect::Ammunition);
        assert_eq!(rows[6], VehicleCriticalEffect::TurretBlownOff);
        assert_eq!(column(true, VehicleSection::Turret).unwrap(), rows);
    }

    #[test]
    fn rotor_column_follows_total_warfare() {
        let rows = column(true, VehicleSection::Rotor).unwrap();
        assert_eq!(
            rows,
            [
                VehicleCriticalEffect::Rotor(RotorHit::Damage),
                VehicleCriticalEffect::Rotor(RotorHit::Damage),
                VehicleCriticalEffect::Rotor(RotorHit::Damage),
                VehicleCriticalEffect::Rotor(RotorHit::FlightStabilizer),
                VehicleCriticalEffect::Rotor(RotorHit::FlightStabilizer),
                VehicleCriticalEffect::Rotor(RotorHit::Destroy),
                VehicleCriticalEffect::Rotor(RotorHit::Destroy),
            ]
        );
        assert!(column(false, VehicleSection::Rotor).is_none());
    }
}
