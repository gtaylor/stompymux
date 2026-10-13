//! Located vehicle impacts commit hit-table effects, armor damage and critical cascades together.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Admission that varies per combat action; the Total Warfare tables themselves are fixed.
#[derive(Debug, Clone, Copy)]
pub struct VehicleImpactRules {
    pub criticals: VehicleCriticalRules,
}

/// One damage group's table selection and all staged effects, before visibility publication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish impact effects only with the enclosing attack transaction"]
pub struct VehicleImpact {
    /// The hit-location roll; combat-safe routing still draws it.
    pub rolls: Vec<u8>,
    /// Combat-safe routing has no hit location or motive effects.
    pub hit: Option<VehicleHit>,
    pub damage: Option<VehicleArmorDamage>,
    pub notices: Vec<Notice>,
    /// Pilot-only control feedback indexed into the damage notice stream.
    pub pilot_notices: Vec<PilotNotice>,
    pub broadcasts: Vec<Notice>,
}

/// Resolve one already successful weapon attack group from its incoming arc, preserving
/// all-or-nothing dice and damage. Weapon targeting and publication still belong to the attack.
pub fn resolve_vehicle_impact(
    world: &mut World,
    id: ObjectId,
    arc: HitArc,
    amount: u32,
    armor_piercing: Option<Weapon>,
    rules: VehicleImpactRules,
) -> Result<VehicleImpact> {
    let vehicle = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle is unavailable")?;
    ensure!(!vehicle.is_destroyed(), "Vehicle is destroyed");
    world.attempt(|world| {
        let result = resolve_impact(
            world,
            id,
            arc,
            ImpactRequest {
                amount,
                armor_piercing,
                rear: false,
                attacker: None,
                class: DamageClass::Ordinary,
            },
            rules,
            None,
            true,
            false,
        )?;
        Ok(result)
    })
}

/// One admitted vehicle damage entry, independent of the attacker's chassis.
#[derive(Debug, Clone, Copy)]
pub(super) struct ImpactRequest {
    pub amount: u32,
    pub armor_piercing: Option<Weapon>,
    pub rear: bool,
    pub attacker: Option<ObjectId>,
    pub class: DamageClass,
}

/// Continue a previously admitted non-weapon impact (falls, blasts), retaining location and
/// damage rolls after hull loss. Damage without a weapon Damage Value is not reduced on rotors.
pub(super) fn resolve_followup_in_candidate(
    world: &mut World,
    id: ObjectId,
    arc: HitArc,
    request: ImpactRequest,
    rules: VehicleImpactRules,
) -> Result<VehicleImpact> {
    resolve_impact(world, id, arc, request, rules, None, false, false)
}

/// A weapon damage group, optionally aimed at a known section, bypassing random location
/// effects. A glancing blow lowers every critical roll of the group by two (TO:AR p. 79).
pub(super) fn resolve_directed_followup(
    world: &mut World,
    id: ObjectId,
    arc: HitArc,
    request: ImpactRequest,
    rules: VehicleImpactRules,
    glancing: bool,
    forced: Option<VehicleHit>,
) -> Result<VehicleImpact> {
    resolve_impact(world, id, arc, request, rules, forced, true, glancing)
}

/// Critical-roll modifier of a glancing blow (TO:AR p. 79).
pub const GLANCING_CRITICAL_MODIFIER: i8 = -2;

/// Shared material path: location, rotor scaling, armor, criticals and internal damage.
fn resolve_impact(
    world: &mut World,
    id: ObjectId,
    arc: HitArc,
    request: ImpactRequest,
    rules: VehicleImpactRules,
    forced: Option<VehicleHit>,
    weapon: bool,
    glancing: bool,
) -> Result<VehicleImpact> {
    let ImpactRequest {
        mut amount,
        armor_piercing,
        rear,
        attacker,
        class,
    } = request;
    ensure!(amount > 0, "Impact damage must be positive");
    if let Some(weapon) = armor_piercing {
        ensure!(
            AmmunitionMode::ArmorPiercing.supports(weapon),
            "Weapon cannot fire armor-piercing ammunition"
        );
    }
    let mut result = if let Some(hit) = forced {
        VehicleImpact {
            rolls: Vec::new(),
            hit: Some(hit),
            damage: None,
            notices: Vec::new(),
            pilot_notices: Vec::new(),
            broadcasts: Vec::new(),
        }
    } else {
        resolve_location_followup(world, id, arc, rules)?
    };
    let Some(hit) = result.hit else {
        result
            .notices
            .extend(super::combat_safe::notice(attacker, id));
        // Safe material impacts still enter the damage stage and consume its diagnostic roll.
        world
            .btech
            .vehicles
            .get_mut(&id)
            .unwrap()
            .dice
            .generic_roll();
        return Ok(result);
    };
    // Rotor hits take one point per ten points (or fraction) of a weapon's damage (TW p. 197).
    if weapon
        && hit.section == VehicleSection::Rotor
        && world.btech.vehicles()[&id].definition().is_vtol()
    {
        amount = amount.div_ceil(ROTOR_DAMAGE_DIVISOR);
    }
    let damage = super::vehicle_armor_damage::resolve_rear_followup_in_candidate(
        world,
        id,
        VehicleArmorHit {
            section: hit.section,
            amount,
            through_armor_critical: hit.through_armor_critical,
            armor_piercing,
            damage_class: class,
        },
        rear,
        rules.criticals,
        super::vehicle_internal_damage::DamageContext {
            attacker,
            depth: 0,
            critical_modifier: if glancing {
                GLANCING_CRITICAL_MODIFIER
            } else {
                0
            },
        },
    )?;
    super::piloting::append_feedback(
        &mut result.pilot_notices,
        damage.pilot_notices.iter().cloned(),
        result.notices.len(),
    );
    result.notices.extend(damage.notices.clone());
    result.broadcasts.extend(damage.broadcasts.clone());
    result.damage = Some(damage);
    Ok(result)
}

/// Weapon damage against rotors counts one point per this many points, rounding up.
pub const ROTOR_DAMAGE_DIVISOR: u32 = 10;

/// Resolve table routing and its direct effects inside an unpublished attack candidate.
/// Pod hits stop here; ordinary impacts continue into armor and internal damage.
pub(super) fn resolve_location(
    world: &mut World,
    id: ObjectId,
    arc: HitArc,
    rules: VehicleImpactRules,
) -> Result<VehicleImpact> {
    let vehicle = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle is unavailable")?;
    ensure!(!vehicle.is_destroyed(), "Vehicle is destroyed");
    resolve_location_followup(world, id, arc, rules)
}

/// Table routing also applies to later packets against a wreck's remaining sections.
fn resolve_location_followup(
    world: &mut World,
    id: ObjectId,
    arc: HitArc,
    rules: VehicleImpactRules,
) -> Result<VehicleImpact> {
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
    let mut dice = vehicle.dice.clone();
    let roll = dice.generic_roll();
    let mut result = VehicleImpact {
        rolls: vec![roll],
        hit: None,
        damage: None,
        notices: Vec::new(),
        pilot_notices: Vec::new(),
        broadcasts: Vec::new(),
    };
    if rules.criticals.combat_safe || vehicle.combat_safe {
        world.btech.vehicles.get_mut(&id).unwrap().dice = dice;
        return Ok(result);
    }
    if vehicle.definition().is_vtol() {
        let selected = vehicle.vtol_hit(arc, roll)?;
        world.btech.vehicles.get_mut(&id).unwrap().dice = dice;
        if selected.rotor {
            let rotor = super::rotor_damage::apply_in_world(
                world,
                id,
                RotorHit::Damage,
                rules.criticals.extended_piloting,
            )?;
            result.notices.push(Notice {
                unit: id,
                text: rotor.message().into(),
            });
            append_landing(
                world,
                id,
                rotor,
                &mut result.notices,
                &mut result.pilot_notices,
            );
        }
        result.hit = Some(selected.hit);
        return Ok(result);
    }

    let hit = vehicle.hit(arc, roll, &mut dice)?;
    world.btech.vehicles.get_mut(&id).unwrap().dice = dice;
    if let Some(motive) = hit.motive {
        let (notices, broadcasts) =
            super::vehicle_motive_effects::apply_located(world, id, motive)?;
        result.notices.extend(notices);
        result.broadcasts.extend(broadcasts);
    }
    result.hit = Some(hit);
    Ok(result)
}

/// Report the engine-damage landing a rotor hit forced, with pilot-only roll diagnostics.
pub(super) fn append_landing(
    world: &World,
    id: ObjectId,
    rotor: super::RotorDamage,
    notices: &mut Vec<Notice>,
    pilot_notices: &mut Vec<PilotNotice>,
) {
    let Some(check) = &rotor.landing else {
        let falling = world.btech.vehicles()[&id]
            .vtol_flight()
            .is_some_and(|flight| flight.phase == VtolFlightPhase::Falling);
        if rotor.grounded && falling {
            notices.push(Notice {
                unit: id,
                text: "There is nowhere to set down: you lose lift and start to fall!".into(),
            });
        }
        return;
    };
    super::piloting::capture_feedback(
        id,
        world.btech.vehicles()[&id].pilot(),
        check,
        notices,
        pilot_notices,
    );
    notices.push(Notice {
        unit: id,
        text: if check.success {
            "You manage to set down before the rotor fails completely!"
        } else {
            "You lose lift and start to fall!"
        }
        .into(),
    });
}

impl VehicleImpactRules {
    /// Standard tactical vehicle policy: no combat-safe shortcut, no toughness, basic piloting.
    pub const STANDARD: Self = Self {
        criticals: VehicleCriticalRules {
            extended_piloting: false,
            combat_safe: false,
            toughness: false,
        },
    };

    /// The Total Warfare vehicle policy for either shooter class with the server's skill settings.
    pub(crate) fn configured(config: &crate::config::BattleTechConfig, toughness: bool) -> Self {
        Self {
            criticals: VehicleCriticalRules {
                extended_piloting: config.extended_piloting != 0,
                combat_safe: false,
                toughness,
            },
        }
    }
}
