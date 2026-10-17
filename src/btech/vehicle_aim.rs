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
    let indirect = super::spotter::indirect_aim(world, source, target, weapon_index)?;
    let distance = super::unit_range(world, shooter, target)?.hexes();
    let ammunition = attacker.ammunition_mode(weapon_index)?;
    let target_terms =
        super::aim::target_modifiers(world, shooter, target, mount.weapon, ammunition, distance)?;
    let mut aim = weapon_modifiers(attacker, weapon_index, distance, gunnery)?;
    aim.attacker_movement += super::recent_fire::skid_modifiers(world, shooter).0;
    let indirect = indirect.map(|aim| super::IndirectAim {
        tag_guided: target_terms.tag_guided,
        ..aim
    });
    aim.indirect = indirect;
    super::aimed_target::apply_aim(
        world,
        shooter,
        target,
        super::aimed_target::AimedAttack {
            weapon: mount.weapon,
            fire_mode: attacker.fire_mode(weapon_index)?,
            ammunition,
            indirect: indirect.is_some(),
            volley: attacker.array_volley(weapon_index)?.is_some(),
        },
        &mut aim,
    )?;
    aim.spotting = u8::from(super::spotter::spotting_for_others(world, shooter));
    aim.ammunition_accuracy += target_terms.ammunition_accuracy;
    // A hyper-assault Gauss rifle's flak attack gets no targeting computer help (Total
    // Warfare p. 136).
    if target_terms.flak && mount.weapon.is_hag() {
        aim.targeting_computer = 0;
    }
    aim.target_movement = target_terms.movement;
    aim.bogged_down = target_terms.bogged_down;
    aim.orbital_drop = target_terms.orbital_drop;
    aim.light = target_terms.light;
    aim.environment = super::planetary_conditions::aim_modifier(world, shooter, mount.weapon);
    aim.interference = super::planetary_conditions::interference_aim_modifier(world, shooter);
    aim.beacon_accuracy += target_terms.beacon_accuracy;
    aim.target_lock = if indirect.is_some() {
        0
    } else {
        super::aim::lock_modifier(world, source, target, rules.override_weapon_arcs)?
    };
    aim.perception = super::aim::indirect_perception(world, shooter, target, indirect)?;
    let submerged = super::weapon_geometry::apply_water_range(
        world,
        shooter,
        weapon_index,
        mount.weapon,
        &mut aim,
    )?;
    super::network_range::apply(
        world,
        shooter,
        super::network_range::NetworkTarget::Unit(target),
        (mount.weapon, ammunition),
        submerged,
        &mut aim,
    )?;
    aim.range = aim
        .range
        .map(|range| range.against_stealth(target_terms.concealed));
    Ok(aim)
}

/// Vehicle movement and equipment contributions used for both unit and coordinate targets.
pub(super) fn weapon_modifiers(
    attacker: &super::Vehicle,
    weapon_index: usize,
    distance: f64,
    gunnery: i16,
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
        attacker.fire_mode(weapon_index)?,
        ammunition,
    )?;
    aim.attacker_movement = attacker.weapon_movement_modifier(weapon_index)?;
    aim.control_damage = attacker.gunnery_damage();
    aim.beacon_accuracy = i8::from(attacker.has_beacon(super::BeaconKind::Haywire));
    // A surviving MRM Apollo system negates the MRM's +1 to hit (Tactical Operations: Advanced
    // Units & Equipment p. 143).
    if weapon.is_mrm() && attacker.apollo_operational(weapon_index)? {
        aim.weapon_accuracy -= 1;
    }
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
