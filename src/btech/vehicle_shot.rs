//! Tactical vehicle shot admission shares geometry and aim without spending ammunition or dice.
use super::{AimModifiers, ShotRules};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Host-aware admission shares all geometry and control checks with raw tactical queries.
pub(super) struct VehicleShotAdmission {
    pub rules: ShotRules,
    pub character: bool,
}

/// Check a direct tactical vehicle shot against current state and return its aim breakdown.
/// The result is not a durable authorization: an enclosing firing action must recheck before
/// committing expenditure and damage. Out-of-range attempts retain a missing subtotal, matching
/// conventional shots that still expend ammunition. Character consequences remain owned by the enclosing action. This query never
/// mutates world state or publishes notices.
pub fn check_vehicle_shot(
    world: &World,
    shooter: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    weapon_index: usize,
    rules: ShotRules,
) -> Result<AimModifiers> {
    let mut dice = world
        .btech
        .vehicles()
        .get(&shooter)
        .context("Vehicle is unavailable")?
        .dice
        .clone();
    check_with_dice(
        world,
        shooter,
        pilot,
        target,
        weapon_index,
        VehicleShotAdmission {
            rules,
            character: false,
        },
        &mut dice,
    )
    .map(|(aim, _)| aim)
}

/// Use the same admission for firing while retaining sensor rolls on the caller's candidate stream.
pub(super) fn check_with_dice(
    world: &World,
    shooter: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    weapon_index: usize,
    admission: VehicleShotAdmission,
    dice: &mut super::Dice,
) -> Result<(AimModifiers, super::gatling::GatlingPreparation)> {
    let operator = super::combat_operator::controlled(world, shooter, pilot)?;
    let rules = admission.rules;
    let attacker = world
        .btech
        .vehicles()
        .get(&shooter)
        .context("Vehicle is unavailable")?;
    ensure!(
        attacker.pod_removal().is_none(),
        "You are too busy removing iNARC pods!"
    );
    let loadout = attacker.loadout()?;
    let mount = loadout
        .weapons
        .get(weapon_index)
        .context("Weapon index out of bounds")?;
    attacker.check_spotter_fire(shooter, weapon_index)?;
    let indirect =
        super::spotter::indirect_target_for_source(world, operator.source, weapon_index)?;
    ensure!(
        shooter != target,
        "You cannot target yourself with this weapon"
    );
    ensure!(!mount.weapon.is_ams(), "That weapon is defensive only!");
    ensure!(
        !mount.weapon.is_artillery(),
        "Artillery requires artillery firing rules"
    );
    for id in [shooter, target] {
        let object = world.objects.get(&id).context("Unit is unavailable")?;
        ensure!(!object.flags.contains(Flag::Going), "Unit is unavailable");
        ensure!(
            admission.character || !object.flags.contains(Flag::InCharacter),
            "Direct tactical vehicle shots require non-character units"
        );
        let unit = super::scanner::scanner_unit(world, id)
            .context("Unit construction state is unavailable")?;
        ensure!(!unit.destroyed, "Unit is destroyed");
        super::unit_elevation(world, id)?.context("Unit is not placed")?;
    }
    let submerged = super::weapon_geometry::submerged(world, shooter, weapon_index)?;
    super::weapon_geometry::check_water(world, shooter, mount.weapon, submerged)?;
    super::planetary_conditions::check_wind(world, shooter, mount.weapon)?;
    super::torpedo::check_target(world, mount.weapon, shooter, target, submerged)?;
    if indirect.is_some() {
        super::spotter::check_indirect_water(world, shooter, target)?;
    }
    let defender = super::scanner::scanner_unit(world, target).context("Target is unavailable")?;
    let map = attacker.position().context("Shooter is not placed")?.map;
    ensure!(
        defender
            .position
            .is_some_and(|position| position.map == map),
        "Units must share a battlefield"
    );
    super::fire_target::check_target_safety_for_source(world, operator.source, target)?;
    super::fire_target::check_unit_munition(world, shooter, weapon_index)?;
    ensure!(
        attacker.weapon_readiness(weapon_index)?.ready,
        "Weapon is not ready"
    );
    let range = super::unit_range(world, shooter, target)?;
    ensure!(
        indirect.is_some()
            || rules.aim.override_weapon_arcs
            || attacker.weapon_bears_on(weapon_index, range.bearing.unwrap_or(180.0))?,
        "Target is outside weapon arc"
    );
    let prepared = super::gatling::prepare(world, shooter, weapon_index, dice)?;
    let gunnery = super::unit_gunnery_target(world, shooter, weapon_index, rules.extended_gunnery)?;
    let aim = super::aim::aim_modifiers_for_source(
        world,
        operator.source,
        target,
        weapon_index,
        gunnery,
        rules.aim,
    )?;
    super::aim::ensure_perceived(&aim)?;
    super::hit_direction::HitDirection::Direct { shooter }.current(world, target)?;
    Ok((aim, prepared))
}
