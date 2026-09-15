//! Coordinate attacks normalize existing chassis launch handlers into one terrain/artillery outcome.
use super::*;
use crate::World;
use anyhow::Result;
use serde::Serialize;

/// Shooter-local launch damage with anatomy-specific casualty consequences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "report", rename_all = "snake_case")]
pub enum BattleLaunchMisload {
    Mech(BattleTacticalImpact),
    Vehicle(BattleVehicleInternalDamage),
}

impl BattleLaunchMisload {
    /// Private checks retain positions within the corresponding immediate notice stream.
    pub(super) fn pilot_notices(&self) -> &[BattlePilotNotice] {
        match self {
            Self::Mech(hit) => &hit.pilot_notices,
            Self::Vehicle(hit) => &hit.pilot_notices,
        }
    }

    /// Immediate notices are already ordered by the shared launch handler.
    pub(super) fn notices(&self) -> Vec<BattleNotice> {
        match self {
            Self::Mech(hit) => hit.notices.clone(),
            Self::Vehicle(hit) => hit.notices.iter().chain(&hit.broadcasts).cloned().collect(),
        }
    }
}

/// Target-independent launch facts consumed by both coordinate attack pipelines.
pub(super) struct CoordinateLaunch {
    pub roll: u8,
    pub hit: bool,
    pub launched: bool,
    pub jammed: bool,
    pub loader_destroyed: bool,
    pub propellant_roll: Option<u8>,
    pub expenditure: BattleWeaponUse,
    pub misload: Option<BattleLaunchMisload>,
    pub ammunition_warning: Option<String>,
    /// Cocoon opening feedback from the shared launch stage.
    pub launch_notices: Vec<BattleNotice>,
}

/// Delegate reservation, jams and attack dice to the established chassis launch handler.
pub(super) fn resolve(
    world: &mut World,
    request: super::weapon_launch::WeaponLaunchRequest,
    artillery: bool,
) -> Result<CoordinateLaunch> {
    if world.btech.vehicles().contains_key(&request.shooter) {
        let request = BattleVehicleLaunchRequest {
            shooter: request.shooter,
            pilot: request.pilot,
            weapon_index: request.weapon_index,
            distance: request.distance,
            target_number: request.target_number,
            streak_confused: request.streak_confused,
            glancing: request.glancing,
            critical_rules: BattleVehicleCriticalRules {
                toughness: request.fall.toughness,
                ..request.fall.vehicle_impact.criticals
            },
        };
        let launch = if artillery {
            super::vehicle_launch::launch_artillery(world, request)?
        } else {
            super::launch_vehicle_weapon(world, request)?
        };
        return Ok(CoordinateLaunch {
            roll: launch.roll,
            hit: launch.hit,
            launched: launch.expenditure.launched,
            jammed: launch.jammed,
            loader_destroyed: launch.loader_destroyed,
            propellant_roll: launch.propellant_roll,
            expenditure: launch.expenditure.into(),
            misload: launch.misload.map(BattleLaunchMisload::Vehicle),
            ammunition_warning: launch.ammunition_warning,
            launch_notices: launch.launch_notices,
        });
    }
    let launch = super::weapon_launch::resolve_launch(world, request)?;
    Ok(CoordinateLaunch {
        roll: launch.roll,
        hit: launch.hit,
        launched: launch.launched,
        jammed: launch.jammed,
        loader_destroyed: launch.loader_destroyed,
        propellant_roll: launch.propellant_roll,
        expenditure: launch.expenditure,
        misload: launch.misload.map(BattleLaunchMisload::Mech),
        ammunition_warning: launch.ammunition_warning,
        launch_notices: launch.launch_notices,
    })
}
