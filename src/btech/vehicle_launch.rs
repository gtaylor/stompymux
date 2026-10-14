//! Atomic vehicle launch preparation and attack rolls, before target damage and publication.
use super::VehicleWeaponUse;
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
    /// Shooter-local critical policy for rapid-fire rounds exploding in the barrel.
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
    /// The mount was destroyed after the shot: rapid-fire rounds exploded in the barrel or a
    /// caseless feed broke down (vehicles treat that as a Weapon Destroyed critical hit).
    pub loader_destroyed: bool,
    /// Persistent ammunition-feed jam after the shot, distinct from temporary critical failures.
    pub jammed: bool,
    /// The jam is permanent for the rest of the game.
    pub feed_broken: bool,
    /// Always absent: a vehicle's caseless breakdown destroys the weapon without a critical roll.
    pub feed_critical_roll: Option<u8>,
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
/// Every feed failure fires the shot first: an Ultra roll of 2 breaks the feed for good, rotary
/// and rapid-fire jams persist a clearable feed jam, rapid-fire rounds exploding in the barrel
/// destroy the mount and damage its section, and a caseless breakdown destroys the weapon.
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
    // Short supply changes the saved selection even when the shot then fails its feed.
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
        roll,
        jammed,
        feed_broken,
        exploded,
        caseless_failure,
        launched,
        hit,
        glancing,
    } = super::launch_roll::roll_launch(
        super::launch_roll::LaunchRollRequest {
            weapon,
            ammunition,
            fire_mode: effective,
            distance,
            target_number,
            streak_confused,
        },
        &mut world.btech.vehicles.get_mut(&shooter).unwrap().dice,
    )?;
    let launch_notices = super::orbital_drop_combat::open_for_fire(world, shooter)?;
    // The shot is fired before any feed failure takes effect (Total Warfare p. 114).
    let expenditure = if let Some(prepared) = prepared {
        prepared
    } else {
        super::reserve_vehicle_weapon(world, shooter, pilot, weapon_index, launched)?
    };
    let criticals = attacker.loadout()?.weapons[weapon_index].criticals.clone();
    let vehicle = world.btech.vehicles.get_mut(&shooter).unwrap();
    // Vehicles treat a caseless breakdown as a Weapon Destroyed critical hit (TacOps: AUE p. 165).
    let destroyed = exploded || caseless_failure;
    if destroyed {
        for location in &criticals {
            vehicle.destroy_critical(*location)?;
        }
    } else if jammed {
        if feed_broken {
            vehicle.break_weapon_feed(weapon_index)?;
        } else {
            vehicle.jam_weapon(weapon_index)?;
        }
    }
    let misload = if exploded {
        Some(super::vehicle_internal_damage::resolve_in_candidate(
            world,
            shooter,
            criticals[0].section,
            u32::from(weapon.profile().damage),
            critical_rules,
            super::vehicle_internal_damage::DamageContext::default(),
        )?)
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
        loader_destroyed: destroyed,
        jammed: jammed && !destroyed,
        feed_broken: feed_broken && !destroyed,
        feed_critical_roll: None,
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
