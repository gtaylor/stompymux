//! Atomic vehicle launch preparation and attack rolls, before target damage and publication.
use super::{GlancingMode, VehicleWeaponUse};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Target-independent inputs from the enclosing attack's current admission and aim calculation.
#[derive(Debug, Clone, Copy)]
pub struct VehicleLaunchRequest {
    pub shooter: ObjectId,
    pub pilot: ObjectId,
    pub weapon_index: usize,
    pub distance: f64,
    pub target_number: Option<i32>,
    pub streak_confused: bool,
    pub glancing: GlancingMode,
    /// Shooter-local critical policy for a rapid misload or caseless propellant ignition.
    pub critical_rules: super::VehicleCriticalRules,
}

/// A launched cycle or failed Streak lock; the enclosing attack still owns all target effects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Apply target effects and publish notices before committing the enclosing attack"]
pub struct VehicleLaunch {
    /// Pre-expenditure warning, published with the enclosing shot.
    pub ammunition_warning: Option<String>,
    /// Cocoon opening precedes the shot's target consequences.
    pub launch_notices: Vec<super::Notice>,
    pub roll: u8,
    /// Permanent mount loss from an Ultra/rapid loader failure or propellant ignition.
    pub loader_destroyed: bool,
    /// Persistent ammunition-feed jam, distinct from temporary critical failures.
    pub jammed: bool,
    /// Caseless ignition check following an attack roll of two or three.
    pub propellant_roll: Option<u8>,
    /// Internal damage and nested critical consequences on the shooter.
    pub misload: Option<super::VehicleInternalDamage>,
    /// Launch classification; a missile near miss can still be rejected by target resolution.
    pub hit: bool,
    /// Tactical missile shots qualify this boundary against their base target number.
    pub glancing: bool,
    pub expenditure: VehicleWeaponUse,
}

/// Reserve and roll a vehicle firing cycle atomically, consuming only its own saved dice stream.
/// Gatling preparation precedes attack dice; failed Streak locks recycle without ammunition loss.
/// The caller must check target admission and resolve damage in the same enclosing transaction.
/// Ultra loader loss destroys the mount; rotary and hotload failures persist a feed jam.
/// Rapid and caseless misloads resolve shooter-local internal damage before spending surviving supply.
pub fn launch_vehicle_weapon(
    world: &mut World,
    request: VehicleLaunchRequest,
) -> Result<VehicleLaunch> {
    world.attempt(|world| {
        let result = launch(world, request, false, None)?;
        Ok(result)
    })
}

/// Resolve on an unpublished candidate so every error discards mode, inventory and random changes.
pub(super) fn launch_artillery(
    world: &mut World,
    request: VehicleLaunchRequest,
) -> Result<VehicleLaunch> {
    launch(world, request, true, None)
}

/// One reservation and roll implementation for direct weapons and admitted artillery.
fn launch(
    world: &mut World,
    request: VehicleLaunchRequest,
    artillery: bool,
    prepared_gatling: Option<super::gatling::GatlingPreparation>,
) -> Result<VehicleLaunch> {
    use super::FireMode;
    let VehicleLaunchRequest {
        shooter,
        pilot,
        weapon_index,
        distance,
        target_number,
        streak_confused,
        glancing,
        critical_rules,
    } = request;
    super::combat_operator::controlled(world, shooter, pilot)?;
    let unit = world
        .btech
        .vehicles()
        .get(&shooter)
        .context("Vehicle is unavailable")?;
    let readiness = unit.weapon_readiness(weapon_index)?;
    ensure!(readiness.ready, "Weapon is not ready");
    let weapon = readiness.weapon;
    ensure!(!weapon.is_ams(), "That weapon is defensive only!");
    ensure!(
        !weapon.is_artillery() || artillery,
        "Artillery requires artillery launch rules"
    );
    let ammunition = unit.ammunition_mode(weapon_index)?;
    let attacker = unit.clone();
    let requested = unit.fire_mode(weapon_index)?;
    let effective = unit.effective_fire_mode(weapon_index)?;
    // Gatling reserves its supply-limited D6 before the weapon's attack roll.
    let prepared = if effective == FireMode::Gatling {
        Some(super::vehicle_readiness::reserve_prepared_weapon(
            world,
            shooter,
            pilot,
            weapon_index,
            true,
            prepared_gatling,
        )?)
    } else {
        None
    };
    // Short supply changes the saved selection even when caseless ignition prevents launch.
    if effective != requested {
        world
            .btech
            .vehicles
            .get_mut(&shooter)
            .unwrap()
            .fire_modes
            .remove(&weapon_index);
    }
    let super::launch_roll::LaunchRoll {
        critical_explosion: _,
        critical_jam: _,
        roll,
        propellant_roll,
        loader_destroyed,
        jammed,
        misload_required,
        launched,
        hit,
        glancing,
    } = super::launch_roll::roll_launch(
        super::launch_roll::LaunchRollRequest {
            damage: Default::default(),
            weapon,
            ammunition,
            fire_mode: effective,
            distance,
            target_number,
            streak_confused,
            glancing,
        },
        &mut world.btech.vehicles.get_mut(&shooter).unwrap().dice,
    )?;
    let launch_notices = if jammed || loader_destroyed {
        Vec::new()
    } else {
        super::orbital_drop_combat::open_for_fire(world, shooter)?
    };
    let mut expenditure = if let Some(prepared) = prepared {
        prepared
    } else if loader_destroyed || jammed {
        let vehicle = world.btech.vehicles.get_mut(&shooter).unwrap();
        if loader_destroyed {
            let criticals = vehicle.loadout()?.weapons[weapon_index].criticals.clone();
            for location in criticals {
                vehicle.destroy_critical(location)?;
            }
        } else {
            vehicle.jam_weapon(weapon_index)?;
        }
        VehicleWeaponUse {
            weapon,
            ammunition: Vec::new(),
            ammunition_mode: ammunition,
            fire_mode: effective,
            gatling_damage: None,
            launched: false,
            heat: 0,
        }
    } else {
        super::reserve_vehicle_weapon(world, shooter, pilot, weapon_index, launched)?
    };
    let misload = if misload_required {
        let draws = attacker.ammunition_feed(weapon_index, effective.rounds_per_cycle())?;
        let section = attacker.loadout()?.weapons[weapon_index].criticals[0].section;
        let damage = super::vehicle_internal_damage::resolve_in_candidate(
            world,
            shooter,
            section,
            u32::from(weapon.profile().damage),
            critical_rules,
            super::vehicle_internal_damage::DamageContext::default(),
        )?;
        let vehicle = world.btech.vehicles.get_mut(&shooter).unwrap();
        expenditure
            .ammunition
            .extend(vehicle.spend_surviving_draws(draws));
        Some(damage)
    } else {
        None
    };
    let ammunition_warning = if launched {
        super::combat_warnings::vehicle_ammunition_message(&attacker, &expenditure)
    } else {
        None
    };
    Ok(VehicleLaunch {
        ammunition_warning,
        launch_notices,
        roll,
        loader_destroyed,
        jammed,
        propellant_roll,
        misload,
        hit,
        glancing,
        expenditure,
    })
}

/// Direct shots have already prepared gatling dice before their sensor aim calculation.
pub(super) fn launch_prepared(
    world: &mut World,
    request: VehicleLaunchRequest,
    prepared: super::gatling::GatlingPreparation,
) -> Result<VehicleLaunch> {
    launch(world, request, false, Some(prepared))
}
