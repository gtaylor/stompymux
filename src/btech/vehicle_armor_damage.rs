//! Armor damage and penetration share one candidate, preserving critical ordering and dice ownership.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// One already located hit, after weapon-specific damage adjustments by the enclosing attack.
#[derive(Debug, Clone, Copy)]
pub struct VehicleArmorHit {
    pub section: VehicleSection,
    pub amount: u32,
    pub through_armor_critical: bool,
    /// AP ammunition's weapon family, when applicable; ordinary hits use None.
    pub armor_piercing: Option<Weapon>,
    /// Energy and area-effect hits interact with reflective armor.
    pub damage_class: DamageClass,
}

/// Ordered protection changes and critical effects; visibility and attack publication remain external.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish damage notices with the enclosing attack transaction"]
pub struct VehicleArmorDamage {
    pub section: VehicleSection,
    pub incoming: u32,
    pub armor_damage: u32,
    pub absorbed: u16,
    pub overflow: u32,
    /// Damage-entry roll, optional rear-hit diagnostic, then any through-armor critical roll.
    pub rolls: Vec<u8>,
    pub criticals: Vec<VehicleCriticalResolution>,
    pub internal: Option<VehicleInternalDamage>,
    pub unit_destroyed: bool,
    pub notices: Vec<Notice>,
    /// Pilot-only control feedback indexed into the damage notice stream.
    pub pilot_notices: Vec<PilotNotice>,
    pub broadcasts: Vec<Notice>,
}

/// Apply armor, through-armor criticals and internal overflow atomically.
/// Hit-table motive effects and attack-level feedback belong to the caller.
pub fn resolve_vehicle_armor_damage(
    world: &mut World,
    id: ObjectId,
    hit: VehicleArmorHit,
    rules: VehicleCriticalRules,
) -> Result<VehicleArmorDamage> {
    world.attempt(|world| {
        let result = resolve_in_candidate(world, id, hit, rules)?;
        Ok(result)
    })
}

/// Resolve the material stages inside the enclosing action's isolated state.
pub(super) fn resolve_in_candidate(
    world: &mut World,
    id: ObjectId,
    hit: VehicleArmorHit,
    rules: VehicleCriticalRules,
) -> Result<VehicleArmorDamage> {
    let vehicle = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle is unavailable")?;
    ensure!(!vehicle.is_destroyed(), "Vehicle is destroyed");
    ensure!(
        vehicle
            .sections()
            .get(&hit.section)
            .is_some_and(|section| section.internal > 0),
        "Vehicle section is unavailable"
    );
    resolve_followup_in_candidate(world, id, hit, rules)
}

/// Continue an admitted attack against remaining material or a section already lost to earlier packets.
/// Lost sections consume the damage-entry roll and discard the packet without transferring it.
pub(super) fn resolve_followup_in_candidate(
    world: &mut World,
    id: ObjectId,
    hit: VehicleArmorHit,
    rules: VehicleCriticalRules,
) -> Result<VehicleArmorDamage> {
    resolve_rear_followup_in_candidate(world, id, hit, false, rules, Default::default())
}

/// Blast rear selection redirects front material hits and retains its pre-damage diagnostic roll.
pub(super) fn resolve_rear_followup_in_candidate(
    world: &mut World,
    id: ObjectId,
    mut hit: VehicleArmorHit,
    rear: bool,
    rules: VehicleCriticalRules,
    context: super::vehicle_internal_damage::DamageContext,
) -> Result<VehicleArmorDamage> {
    if rear && hit.section == VehicleSection::Front {
        hit.section = VehicleSection::Rear;
    }
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
    // A directed shot may name an anatomical slot absent from this particular hull.
    // Like a section lost to an earlier packet, it consumes entry dice and discards damage.
    let section_lost = vehicle
        .sections()
        .get(&hit.section)
        .is_none_or(|section| section.internal == 0);
    if let Some(weapon) = hit.armor_piercing {
        ensure!(
            AmmunitionMode::ArmorPiercing.supports(weapon),
            "Weapon cannot fire armor-piercing ammunition"
        );
    }
    let original = vehicle
        .definition()
        .sections
        .get(&hit.section)
        .map_or(0, |section| section.armor);
    let incoming = hit.amount;
    if hit.amount > 0
        && hit.section == VehicleSection::Rotor
        && vehicle.definition().is_vtol()
        && rules.rotor_damage_divisor > 0
    {
        hit.amount = (hit.amount / rules.rotor_damage_divisor).max(1);
    }
    let hardened = vehicle
        .definition()
        .has_technology(super::Technology::HardenedArmor);
    let reflective = !hardened
        && vehicle
            .definition()
            .has_technology(super::Technology::LaserReflectiveArmor)
        && hit.damage_class != DamageClass::Ordinary;
    // Hardened armor records the armor it could remove: half the hit, rounding up.
    // Reflective armor halves energy hits, rounding down, and doubles area-effect hits.
    let amount = if hardened {
        hit.amount.div_ceil(2)
    } else if reflective {
        super::Technology::reflective_hit(hit.damage_class, hit.amount, u16::MAX)
            .map_or(hit.amount, |(removed, _)| u32::from(removed))
    } else {
        hit.amount
    };
    let mut result = VehicleArmorDamage {
        section: hit.section,
        incoming,
        armor_damage: amount,
        absorbed: 0,
        overflow: 0,
        rolls: Vec::new(),
        criticals: Vec::new(),
        internal: None,
        unit_destroyed: vehicle.is_destroyed(),
        notices: Vec::new(),
        pilot_notices: Vec::new(),
        broadcasts: Vec::new(),
    };
    if amount == 0 {
        return Ok(result);
    }
    let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
    result.rolls.push(vehicle.dice.generic_roll());
    if rules.combat_safe || super::combat_safe::protects(world, context.attacker, id) {
        result
            .notices
            .extend(super::combat_safe::notice(context.attacker, id));
        return Ok(result);
    }
    result.notices.extend(super::weapons_hold::damage_notice(
        world,
        context.attacker,
        id,
    ));
    let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
    if rear && !vehicle.definition().has_special("SalvageTech") {
        result.rolls.push(vehicle.dice.generic_roll());
    }
    if section_lost {
        result.overflow = hit.amount;
        return Ok(result);
    }
    if let Some(notices) =
        super::orbital_drop_combat::intercept(world, id, context.attacker, hit.amount)?
    {
        result.notices.extend(notices);
        return Ok(result);
    }
    super::damage_counters::record(world, id, context.attacker, hit.amount)?;
    result.notices.extend(super::hiding::damage(world, id));
    if !rear
        && hit.section == VehicleSection::Front
        && !world.btech.vehicles()[&id].definition().is_vtol()
        && let Some((notices, broadcasts)) = super::searchlight::strike(world, id)
    {
        result.notices.extend(notices);
        result.broadcasts.extend(broadcasts);
    }

    let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
    result.notices.push(Notice {
        unit: id,
        text: format!(
            "[fg=yellow bold]You have been hit for {} points of damage in the {} [reset]",
            hit.amount,
            hit.section.name().replace('_', " ")
        ),
    });
    let previous_warning =
        super::combat_warnings::armor_severity(original, vehicle.sections()[&hit.section].armor);
    // Each hardened armor point stops two damage; overflow passes at full value.
    let (armor_damage, hardened_overflow) = if hardened {
        let (removed, overflow) =
            super::Technology::hardened_hit(hit.amount, vehicle.sections()[&hit.section].armor);
        (u32::from(removed), Some(overflow))
    } else if let Some((removed, overflow)) = reflective
        .then(|| {
            super::Technology::reflective_hit(
                hit.damage_class,
                hit.amount,
                vehicle.sections()[&hit.section].armor,
            )
        })
        .flatten()
    {
        (u32::from(removed), Some(overflow))
    } else {
        (amount, None)
    };
    result.absorbed = vehicle
        .damage_phase(
            hit.section,
            armor_damage.min(u32::from(u16::MAX)) as u16,
            DamagePhase::Armor { rear: false },
        )?
        .absorbed;
    result.overflow = hardened_overflow.unwrap_or(amount - u32::from(result.absorbed));
    let remaining = vehicle.sections()[&hit.section].armor;
    let warning = super::combat_warnings::armor_severity(original, remaining);
    // Hardened armor negates armor-piercing critical chances.
    let ap = !hit.through_armor_critical
        && !hardened
        && result.overflow == 0
        && hit.armor_piercing.is_some()
        && (original == 0 || u32::from(remaining) * 100 / u32::from(original) < 50);
    let mut count = 0;
    if hit.through_armor_critical || ap {
        let roll = vehicle.dice.generic_roll();
        result.rolls.push(roll);
        let adjusted = roll.saturating_sub(if ap {
            hit.armor_piercing.unwrap().armor_piercing_penalty()
        } else {
            0
        });
        count = match adjusted {
            8 | 9 => 1,
            10 | 11 => 2,
            12 => 3,
            _ => 0,
        };
    }
    for _ in 0..count {
        let vehicle = &world.btech.vehicles()[&id];
        if vehicle.is_destroyed() || vehicle.sections()[&hit.section].internal == 0 {
            break;
        }
        let critical = super::vehicle_critical_resolution::resolve_in_candidate(
            world,
            id,
            hit.section,
            rules,
            context,
        )?;
        super::piloting::append_feedback(
            &mut result.pilot_notices,
            critical.pilot_notices.iter().cloned(),
            result.notices.len(),
        );
        result.notices.extend(critical.notices.clone());
        result.broadcasts.extend(critical.broadcasts.clone());
        result.criticals.push(critical);
    }
    let vehicle = &world.btech.vehicles()[&id];
    if warning > previous_warning && vehicle.armor_warning() {
        result.notices.push(Notice {
            unit: id,
            text: super::combat_warnings::vehicle_armor_message(hit.section, warning),
        });
    }
    if result.overflow > 0 && vehicle.sections()[&hit.section].internal > 0 {
        let internal = super::vehicle_internal_damage::resolve_penetration(
            world,
            id,
            hit.section,
            result.overflow,
            rules,
            count > 0,
            context,
        )?;
        super::piloting::append_feedback(
            &mut result.pilot_notices,
            internal.pilot_notices.iter().cloned(),
            result.notices.len(),
        );
        result.notices.extend(internal.notices.clone());
        result.broadcasts.extend(internal.broadcasts.clone());
        result.internal = Some(internal);
    }
    if result.overflow == 0 {
        let (roll, notice) = super::vacuum::check_vehicle(world, id, hit.section, false)?;
        result.rolls.extend(roll);
        result.notices.extend(notice);
    }
    result.unit_destroyed = world.btech.vehicles()[&id].is_destroyed();
    Ok(result)
}
