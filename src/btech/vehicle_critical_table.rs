//! Ground-vehicle critical tables select typed consequences without partially applying damage.
use super::{BattleDice, BattleVehicle, BattleVehicleMovement, BattleVehicleSection};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::sync::Arc;

/// Advanced criticals take precedence over the FASA switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleVehicleCriticalTable {
    Standard,
    Fasa,
    Advanced,
}

impl BattleVehicleCriticalTable {
    /// Translate the two independent configuration switches into one selected table.
    pub fn from_settings(advanced: bool, fasa: bool) -> Self {
        if advanced {
            return Self::Advanced;
        }
        if fasa {
            return Self::Fasa;
        }
        Self::Standard
    }
}

/// Admission supplied by the surrounding combat action.
#[derive(Debug, Clone, Copy)]
pub struct BattleVehicleCriticalRules {
    /// Exterior rotor damage divisor; zero disables scaling. Internal damage is unaffected.
    pub rotor_damage_divisor: u32,
    pub table: BattleVehicleCriticalTable,
    /// Host aircraft policy; None applies the explicit table to either vehicle class.
    pub vtol_table: Option<BattleVehicleCriticalTable>,
    pub enabled: bool,
    pub combat_safe: bool,
    /// Use the toughness consciousness rule when applying crew injuries.
    pub toughness: bool,
    /// Pilot-skill selection for aircraft engine-loss emergency landing checks.
    pub extended_piloting: bool,
}

/// Selected consequence; its table and section remain part of the report for application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleVehicleCriticalEffect {
    CrewHit,
    VtolPilot,
    VtolCopilot,
    MainWeaponJam,
    Engine,
    CrewKilled,
    FuelTank,
    PowerPlant,
    MotiveSpeedLoss,
    Immobilize,
    TurretLock,
    Driver,
    WeaponJam,
    Stabilizer,
    Sensors,
    Commander,
    WeaponDestroyed,
    Cargo,
    CrewStunned,
    Ammunition,
    TurretJam,
    TurretBlownOff,
    /// Aircraft-specific rotor outcome, applied by the shared critical transaction.
    Rotor(super::BattleRotorHit),
}

/// A table result with its actual random draws, still awaiting damage and notification effects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Apply the selected critical consequence within the enclosing damage transaction"]
pub struct BattleVehicleCriticalReport {
    pub table: BattleVehicleCriticalTable,
    pub section: BattleVehicleSection,
    /// Ground standard preliminary die then optional d6; VTOL standard/FASA d6; advanced 2d6 total.
    pub rolls: Vec<u8>,
    pub effect: Option<BattleVehicleCriticalEffect>,
}

/// Select one critical using the victim's saved dice; no equipment or crew damage is applied here.
pub fn roll_vehicle_critical(
    world: &mut World,
    id: ObjectId,
    section: BattleVehicleSection,
    rules: BattleVehicleCriticalRules,
) -> Result<BattleVehicleCriticalReport> {
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
    let report = select(vehicle, section, rules, &mut dice);
    Arc::make_mut(&mut world.btech.vehicles)
        .get_mut(&id)
        .unwrap()
        .dice = dice;
    Ok(report)
}

/// Keep early-return rules and die sequencing independent from consequence application.
fn select(
    vehicle: &BattleVehicle,
    section: BattleVehicleSection,
    rules: BattleVehicleCriticalRules,
    dice: &mut BattleDice,
) -> BattleVehicleCriticalReport {
    use BattleVehicleCriticalEffect as E;
    use BattleVehicleCriticalTable as T;
    let table = rules.table_for(vehicle);
    let mut report = BattleVehicleCriticalReport {
        table: if vehicle.definition().is_vtol() && table == T::Fasa {
            T::Standard
        } else {
            table
        },
        section,
        rolls: Vec::new(),
        effect: None,
    };
    if !rules.enabled
        || rules.combat_safe
        || vehicle.combat_safe
        || vehicle.definition().has_special("CritProof_Tech")
    {
        return report;
    }
    let stationary = vehicle.definition().movement == BattleVehicleMovement::Stationary;
    if table == T::Advanced {
        let roll = dice.generic_roll();
        report.rolls.push(roll);
        if vehicle.definition().is_vtol() {
            report.effect = advanced_vtol(section, roll);
        } else if !stationary {
            report.effect = advanced(section, roll);
        }
        return report;
    }
    if vehicle.definition().is_vtol() {
        let roll = dice.d6();
        report.rolls.push(roll);
        report.effect = Some(
            [
                E::CrewKilled,
                E::MainWeaponJam,
                E::Engine,
                E::CrewKilled,
                E::FuelTank,
                E::PowerPlant,
            ][usize::from(roll - 1)],
        );
        return report;
    }
    if stationary {
        return report;
    }
    if table == T::Standard {
        let turret = section == BattleVehicleSection::Turret;
        let roll = dice
            .die(if turret { 3 } else { 10 })
            .expect("nonzero critical die") as u8;
        report.rolls.push(roll);
        if turret && roll == 2 {
            report.effect = (!vehicle.turret_locked()).then_some(E::TurretLock);
            return report;
        }
        if !turret && roll <= 5 {
            if !vehicle.immobilized() {
                report.effect = Some(if roll == 5 {
                    E::Immobilize
                } else {
                    E::MotiveSpeedLoss
                });
            }
            return report;
        }
    }
    let roll = dice.d6();
    report.rolls.push(roll);
    report.effect = Some(
        [
            E::CrewHit,
            E::MainWeaponJam,
            E::Engine,
            E::CrewKilled,
            E::FuelTank,
            E::PowerPlant,
        ][usize::from(roll - 1)],
    );
    report
}

/// Advanced ground-vehicle outcomes depend on the struck face, with rolls below six producing no effect.
fn advanced(section: BattleVehicleSection, roll: u8) -> Option<BattleVehicleCriticalEffect> {
    use BattleVehicleCriticalEffect as E;
    use BattleVehicleSection as S;
    if roll < 6 {
        return None;
    }
    let row = match section {
        // Rotor hits have no entry in the ground-vehicle critical table.
        S::Rotor => return None,
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
        S::Turret => [
            E::Stabilizer,
            E::TurretJam,
            E::WeaponJam,
            E::TurretLock,
            E::WeaponDestroyed,
            E::TurretBlownOff,
            E::Ammunition,
        ],
    };
    Some(row[usize::from(roll - 6)])
}

/// Advanced rotorcraft hull tables share effect application with conventional vehicles.
fn advanced_vtol(section: BattleVehicleSection, roll: u8) -> Option<BattleVehicleCriticalEffect> {
    use BattleVehicleCriticalEffect as E;
    use BattleVehicleSection as S;
    if section == S::Rotor {
        return super::BattleRotorHit::from_critical_roll(roll)
            .expect("2d6 critical roll")
            .map(E::Rotor);
    }
    if roll < 6 {
        return None;
    }
    let row = match section {
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
        S::Turret | S::Rotor => return None,
    };
    Some(row[usize::from(roll - 6)])
}

impl BattleVehicleCriticalRules {
    /// Select per victim at the common hit/critical boundary, retaining policy for nested effects.
    pub(crate) fn table_for(self, vehicle: &BattleVehicle) -> BattleVehicleCriticalTable {
        if vehicle.definition().is_vtol() {
            return self.vtol_table.unwrap_or(self.table);
        }
        self.table
    }
}
