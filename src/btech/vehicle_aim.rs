//! Vehicle direct-weapon aim composes shared range, target, perception and lock rules with vehicle controls.
use super::{AimModifiers, AimRules, StealthRange, System};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Calculate a vehicle subtotal without authorizing firing or committing the caller's candidate dice.
pub(super) fn modifiers(
    world: &World,
    source: super::fire_target::TargetSource,
    target: ObjectId,
    weapon_index: usize,
    gunnery: i16,
    rules: AimRules,
) -> Result<AimModifiers> {
    let shooter = source.unit;
    for id in [shooter, target] {
        ensure!(
            world
                .objects
                .get(&id)
                .is_some_and(|object| !object.flags.contains(Flag::Going)),
            "Unit is unavailable"
        );
    }
    let attacker = world
        .btech
        .vehicles()
        .get(&shooter)
        .context("Vehicle construction is unavailable")?;
    let loadout = attacker.loadout()?;
    let mount = loadout
        .weapons
        .get(weapon_index)
        .context("Weapon index out of bounds")?;
    let indirect =
        super::spotter::indirect_aim(world, source, target, weapon_index, rules.fasa_turning)?;
    let distance = super::unit_range(world, shooter, target)?.spatial;
    let ammunition = attacker.ammunition_mode(weapon_index)?;
    let target_terms = super::aim::target_modifiers(
        world,
        shooter,
        target,
        mount.weapon,
        ammunition,
        distance,
        rules,
    )?;
    let mut aim = weapon_modifiers(attacker, weapon_index, distance, gunnery, rules)?;
    super::aimed_target::apply_aim(world, shooter, target, mount.weapon, &mut aim)?;
    aim.attacker_water = super::aim::water_modifier(world, shooter)?;
    aim.self_target = shooter == target && mount.weapon == super::Weapon::CoolantGun;
    aim.indirect = indirect;
    aim.ammunition_accuracy += target_terms.ammunition_accuracy;
    aim.target_movement = target_terms.movement;
    aim.dug_in = target_terms.dug_in;
    aim.woods_cover = target_terms.woods_cover;
    aim.orbital_drop = target_terms.orbital_drop;
    aim.light = target_terms.light;
    aim.beacon_accuracy += target_terms.beacon_accuracy;
    aim.target_lock = if indirect.is_some() {
        0
    } else {
        super::aim::lock_modifier(world, source, target, rules.override_weapon_arcs)?
    };
    aim.perception = super::aim::perception_aim(
        world,
        indirect.map_or(shooter, |aim| aim.spotter),
        target,
        indirect.is_some_and(|aim| super::spotter::coordinate_target(world, aim.spotter)),
    )?;
    let submerged = super::weapon_geometry::apply_water_range(
        world,
        shooter,
        weapon_index,
        mount.weapon,
        rules.extended_ranges,
        &mut aim,
    )?;
    super::network_range::apply(
        world,
        shooter,
        super::network_range::NetworkTarget::Unit(target),
        mount.weapon,
        submerged,
        &mut aim,
    )?;
    aim.range = aim
        .range
        .map(|range| range.against_stealth(target_terms.concealed));
    super::targeting_mode::apply(
        world,
        source,
        Some(target),
        mount.weapon,
        ammunition,
        &mut aim,
    )?;
    Ok(aim)
}

/// Vehicle movement and equipment contributions used for both unit and coordinate targets.
pub(super) fn weapon_modifiers(
    attacker: &super::Vehicle,
    weapon_index: usize,
    distance: f64,
    gunnery: i16,
    rules: AimRules,
) -> Result<AimModifiers> {
    let loadout = attacker.loadout()?;
    let weapon = loadout
        .weapons
        .get(weapon_index)
        .context("Weapon index out of bounds")?
        .weapon;
    let ammunition = attacker.ammunition_mode(weapon_index)?;
    let mut aim = super::aim::weapon_base(
        weapon,
        distance,
        gunnery,
        rules,
        attacker.fire_mode(weapon_index)?,
        ammunition,
    )?;
    aim.attacker_movement = attacker.weapon_movement_modifier(weapon_index, rules.fasa_turning)?;
    aim.control_damage = attacker.gunnery_damage();
    aim.beacon_accuracy = i8::from(attacker.has_beacon(super::BeaconKind::Haywire));
    if loadout.weapons[weapon_index].computer_assists(
        ammunition,
        loadout
            .systems
            .iter()
            .filter(|part| part.system == System::TargetingComputer)
            .map(|part| !attacker.critical_unavailable(part.location)),
    ) {
        aim.targeting_computer = -1;
    }
    Ok(aim)
}
