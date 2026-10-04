//! Internal vehicle damage resolves secondary criticals before consuming structure in one candidate.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// An internal explosion's ordered rolls, critical consequences and final protection change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish damage notices and visibility-filtered broadcasts with the enclosing attack"]
pub struct VehicleInternalDamage {
    pub section: VehicleSection,
    pub incoming: u32,
    pub structural_damage: u32,
    /// Internal critical roll, preceded by a damage-entry roll for standalone explosions.
    pub rolls: Vec<u8>,
    pub absorbed: u16,
    /// Vehicle-local internal explosions do not transfer excess to another section.
    pub discarded: u32,
    pub destroyed_sections: Vec<VehicleSection>,
    pub unit_destroyed: bool,
    pub criticals: Vec<VehicleCriticalResolution>,
    /// Includes nested critical notices in execution order.
    pub notices: Vec<Notice>,
    /// Pilot-only control feedback indexed into the damage notice stream.
    pub pilot_notices: Vec<PilotNotice>,
    pub broadcasts: Vec<Notice>,
}

/// Resolve a vehicle-local internal explosion atomically, including nested critical effects.
/// Unsupported nested outcomes leave both damage and all random streams unchanged.
pub fn resolve_vehicle_internal_damage(
    world: &mut World,
    id: ObjectId,
    section: VehicleSection,
    amount: u32,
    rules: VehicleCriticalRules,
) -> Result<VehicleInternalDamage> {
    world.attempt(|world| {
        let result =
            resolve_in_candidate(world, id, section, amount, rules, DamageContext::default())?;
        Ok(result)
    })
}

/// Source attribution and recursion budget travel together through vehicle critical cascades.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct DamageContext {
    pub attacker: Option<ObjectId>,
    pub depth: u8,
}

impl DamageContext {
    /// Preserve the incoming attacker when a critical creates another damage entry.
    pub fn nested(self) -> Self {
        Self {
            depth: self.depth + 1,
            ..self
        }
    }
}

/// Nested resolution uses the parent's private candidate and bounded cascade depth.
pub(super) fn resolve_in_candidate(
    world: &mut World,
    id: ObjectId,
    section: VehicleSection,
    amount: u32,
    rules: VehicleCriticalRules,
    context: DamageContext,
) -> Result<VehicleInternalDamage> {
    let vehicle = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle is unavailable")?;
    ensure!(!vehicle.is_destroyed(), "Vehicle is destroyed");
    resolve_damage(world, id, section, amount, rules, context, None)
}

/// Armor overflow stays in the same damage event: no second entry roll or duplicate hit notice.
pub(super) fn resolve_penetration(
    world: &mut World,
    id: ObjectId,
    section: VehicleSection,
    amount: u32,
    rules: VehicleCriticalRules,
    armor_criticals: bool,
    context: DamageContext,
) -> Result<VehicleInternalDamage> {
    resolve_damage(
        world,
        id,
        section,
        amount,
        rules,
        context,
        Some(armor_criticals),
    )
}

/// Apply internal structure damage with either explosion entry or preceding armor-stage context.
fn resolve_damage(
    world: &mut World,
    id: ObjectId,
    section: VehicleSection,
    amount: u32,
    rules: VehicleCriticalRules,
    context: DamageContext,
    armor_criticals: Option<bool>,
) -> Result<VehicleInternalDamage> {
    ensure!(
        context.depth < 64,
        "Vehicle critical cascade limit exceeded"
    );
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
    ensure!(
        vehicle
            .sections()
            .get(&section)
            .is_some_and(|state| state.internal > 0),
        "Vehicle section is unavailable"
    );
    let structural_damage = if vehicle
        .definition()
        .has_technology(super::Technology::ReinforcedStructure)
    {
        amount.div_ceil(2)
    } else if vehicle
        .definition()
        .has_technology(super::Technology::CompositeStructure)
    {
        amount
            .checked_mul(2)
            .context("Vehicle internal damage exceeds limit")?
    } else {
        amount
    };
    let mut result = VehicleInternalDamage {
        section,
        incoming: amount,
        structural_damage,
        rolls: Vec::new(),
        absorbed: 0,
        discarded: structural_damage,
        destroyed_sections: Vec::new(),
        unit_destroyed: vehicle.is_destroyed(),
        criticals: Vec::new(),
        notices: Vec::new(),
        pilot_notices: Vec::new(),
        broadcasts: Vec::new(),
    };
    if amount == 0 {
        return Ok(result);
    }
    let safe = rules.combat_safe || super::combat_safe::protects(world, context.attacker, id);
    let warning = super::weapons_hold::damage_notice(world, context.attacker, id);
    let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
    if armor_criticals.is_none() {
        result.rolls.push(vehicle.dice.generic_roll());
        if safe {
            result
                .notices
                .extend(super::combat_safe::notice(context.attacker, id));
            return Ok(result);
        }
        result.notices.extend(warning);
        if let Some(notices) =
            super::orbital_drop_combat::intercept(world, id, context.attacker, amount)?
        {
            result.notices.extend(notices);
            return Ok(result);
        }
        super::damage_counters::record(world, id, context.attacker, amount)?;
        result.notices.push(Notice {
            unit: id,
            text: format!(
                "[fg=yellow bold]You have been hit for {amount} points of damage in the {} [reset]",
                section.name().replace('_', " ")
            ),
        });
    }
    let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
    let roll = vehicle.dice.generic_roll();
    result.rolls.push(roll);
    // Damage that penetrated hardened armor rolls criticals two lower.
    let penalty = if armor_criticals.is_some()
        && vehicle
            .definition()
            .has_technology(super::Technology::HardenedArmor)
    {
        super::Technology::HARDENED_CRITICAL_PENALTY
    } else {
        0
    };
    let count = match roll.saturating_sub(penalty) {
        _ if armor_criticals == Some(true) => 0,
        8 | 9 => 1,
        10 | 11 => 2,
        12 => 3,
        _ => 0,
    };
    for _ in 0..count {
        let vehicle = &world.btech.vehicles()[&id];
        if vehicle.is_destroyed() || vehicle.sections()[&section].internal == 0 {
            break;
        }
        let critical = super::vehicle_critical_resolution::resolve_in_candidate(
            world,
            id,
            section,
            rules,
            context.nested(),
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
    let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
    let was_destroyed = vehicle.is_destroyed();
    let damage = vehicle.damage_phase(
        section,
        structural_damage.min(u32::from(u16::MAX)) as u16,
        DamagePhase::Internal,
    )?;
    let destroyed = vehicle.is_destroyed();
    super::kill_counters::transition(world, id, context.attacker, was_destroyed, destroyed)?;
    let vehicle = &world.btech.vehicles()[&id];
    result.absorbed = damage.absorbed;
    result.discarded = structural_damage - u32::from(damage.absorbed);
    for lost in &damage.destroyed_sections {
        let name = lost.name().replace('_', " ");
        result.notices.push(Notice {
            unit: id,
            text: format!("Your {name} has been destroyed!"),
        });
        result.broadcasts.push(Notice {
            unit: id,
            text: format!("'s {name} has been destroyed!"),
        });
    }
    result.destroyed_sections = damage.destroyed_sections;
    if vehicle.sections()[&section].internal > 0 {
        let penetrating = armor_criticals.is_some();
        if !penetrating || result.discarded == 0 {
            let (roll, notice) = super::vacuum::check_vehicle(world, id, section, penetrating)?;
            result.rolls.extend(roll);
            result.notices.extend(notice);
        }
    }
    result.unit_destroyed = world.btech.vehicles()[&id].is_destroyed();
    Ok(result)
}
