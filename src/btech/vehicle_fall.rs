//! Vehicle falls reuse pilot checks, shared fall arithmetic, packet impacts and mine activation.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// A completed vehicle fall, before any caller-specific drowning or crash consequence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish fall consequences with the enclosing environmental transaction"]
pub struct BattleVehicleFallReport {
    /// Ice fracture may cause nested falls before this fall applies its own damage.
    pub ice_break: Option<Box<BattleSurfaceBreak>>,
    pub avoidance: Option<BattlePilotingCheck>,
    /// Accepted protection-check XP diagnostics, published by the host action.
    pub experience_messages: Vec<BattleChannelMessage>,
    pub pilot_injury: Option<BattlePilotInjury>,
    /// Personal injury to an assigned character pilot, independent of tactical crew health.
    pub character_injury: Option<BattleCharacterPilotInjury>,
    pub direction_roll: u8,
    pub arc: BattleHitArc,
    pub damage: u32,
    pub impacts: Vec<BattleVehicleImpact>,
    pub mines: BattleMineEventReport,
    /// Private protection and neighboring fall checks ordered among notices.
    pub pilot_notices: Vec<BattlePilotNotice>,
    pub notices: Vec<BattleNotice>,
}

/// Resolve a tactical vehicle fall atomically; the environmental caller owns immersion eligibility.
/// Zero severity still checks personal injury, changes heading and activates fall-sensitive mines.
pub fn resolve_vehicle_fall(
    world: &mut World,
    id: ObjectId,
    levels: u8,
    rules: BattleFallRules,
) -> Result<BattleVehicleFallReport> {
    resolve_in_candidate(world, id, levels, rules, false)
}

/// The host owns publication and evacuation when character consequences are admitted.
pub(super) fn resolve_in_candidate(
    world: &mut World,
    id: ObjectId,
    levels: u8,
    rules: BattleFallRules,
    character: bool,
) -> Result<BattleVehicleFallReport> {
    ensure!(
        !world
            .btech
            .vehicles()
            .get(&id)
            .context("Vehicle is unavailable")?
            .is_destroyed(),
        "Vehicle is destroyed"
    );
    world.attempt(|world| {
        let report = resolve_material(world, id, u32::from(levels), rules, character)?;
        world.btech.validate_action(world)?;
        Ok(report)
    })
}

/// Compose fall effects in an enclosing transaction before final world validation.
/// The caller owns admission, immersion, character publication and validation.
/// Aircraft wrecks retain fall packets; ordinary fall entry points require a surviving unit.
pub(super) fn resolve_material(
    world: &mut World,
    id: ObjectId,
    levels: u32,
    rules: BattleFallRules,
    character: bool,
) -> Result<BattleVehicleFallReport> {
    let levels = i32::try_from(levels).context("Fall severity exceeds pilot-check range")?;
    resolve_material_signed(world, id, levels, rules, character)
}

/// Signed terrain-derived severity still modifies the crew roll when structural damage is zero.
pub(super) fn resolve_material_signed(
    world: &mut World,
    id: ObjectId,
    levels: i32,
    rules: BattleFallRules,
    character: bool,
) -> Result<BattleVehicleFallReport> {
    resolve_material_signed_with_tonnage(world, id, levels, rules, character, None)
}

pub(super) fn resolve_material_signed_with_tonnage(
    world: &mut World,
    id: ObjectId,
    levels: i32,
    rules: BattleFallRules,
    character: bool,
    tonnage: Option<u32>,
) -> Result<BattleVehicleFallReport> {
    let object = world.objects.get(&id).context("Vehicle is unavailable")?;
    ensure!(
        !object.flags.contains(Flag::Going)
            && (character || !object.flags.contains(Flag::InCharacter)),
        "Vehicle fall requires a live unit and character publication when applicable"
    );
    let unit = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle is unavailable")?;
    let position = unit.position();
    let tile = position
        .map(|position| {
            world
                .btech
                .maps()
                .get(&position.map)
                .context("Map is unavailable")?
                .base_hex(i64::from(position.x), i64::from(position.y))
        })
        .transpose()?;
    let below_ice =
        tile.is_some_and(|tile| tile.terrain == Terrain::Ice && unit.elevation_level(tile) < 0);
    let tons = tonnage.unwrap_or(u32::from(unit.definition().tons));
    let gravity = position.and_then(|position| {
        let map = &world.btech.maps()[&position.map];
        map.uses_special_rules().then_some(map.gravity)
    });
    let pilot = unit.pilot();
    let has_pilot = pilot.is_some();
    let safe = rules.vehicle_impact.criticals.combat_safe || unit.combat_safe;
    world.attempt(|world| {
        let mut notices = Vec::new();
        let mut pilot_notices = Vec::new();
        let mut avoidance = if safe {
            None
        } else {
            notices.push(BattleNotice {
                unit: id,
                text: "You try to avoid taking personal damage.".into(),
            });
            Some(super::vehicle_piloting::roll(
                world,
                id,
                levels,
                rules.extended_piloting,
            )?)
        };
        if let Some(check) = &avoidance {
            super::piloting::capture_feedback(id, pilot, check, &mut notices, &mut pilot_notices);
        }
        let experience_messages = if character && let Some(check) = &mut avoidance {
            super::piloting::award_control_check(world, id, check, rules.extended_piloting)?
                .into_iter()
                .collect()
        } else {
            Vec::new()
        };
        let mut character_injury = None;
        let pilot_injury = if avoidance.as_ref().is_some_and(|check| !check.success) && has_pilot {
            notices.push(BattleNotice {
                unit: id,
                text: "You take personal injury!".into(),
            });
            match super::pilot_injury::injure_in_candidate(world, id, 1, rules.toughness)? {
                super::pilot_injury::PilotInjury::Tactical(injury) => {
                    notices.extend(injury.notice(id));
                    Some(injury)
                }
                super::pilot_injury::PilotInjury::Character(injury) => {
                    character_injury = Some(injury);
                    None
                }
            }
        } else {
            None
        };
        let unit = world.btech.vehicles.get_mut(&id).unwrap();
        unit.orbital_drop = None;
        unit.ground_elevation = (below_ice && !unit.definition().is_vtol())
            .then(|| tile.map(|tile| f64::from(tile.surface_height())))
            .flatten();
        unit.under_bridge = false;
        unit.halt();
        let ice_break = if position.is_none() {
            None
        } else {
            let check_ice = if character {
                super::surface_break::check_ice_landing_in_action
            } else {
                super::surface_break::check_ice_landing
            };
            check_ice(world, id, rules)?.map(Box::new)
        };
        if let Some(fracture) = &ice_break {
            super::piloting::append_feedback(
                &mut pilot_notices,
                fracture.pilot_notices.iter().cloned(),
                notices.len(),
            );
            notices.extend(fracture.notices.iter().cloned());
        }
        // C measures fall-damage wetness from the live hex after ice breakage turns
        // it to water (mech_ice.c break_sub precedes mech_fall's damage roll).
        let tile = position
            .map(|position| {
                world
                    .btech
                    .maps()
                    .get(&position.map)
                    .context("Map is unavailable")?
                    .base_hex(i64::from(position.x), i64::from(position.y))
            })
            .transpose()?;
        let unit = world.btech.vehicles.get_mut(&id).unwrap();
        let wet = tile.is_some_and(|tile| {
            (matches!(
                tile.terrain,
                Terrain::Water | Terrain::Ice | Terrain::Bridge
            ) && unit.elevation_level(tile) < 0)
                || tile.terrain == Terrain::HighWater
        });
        let damage = super::fall_profile::damage(tons, levels, wet, gravity)?;
        let direction_roll = unit.dice.d6();
        let (arc, offset) = super::fall_profile::direction(direction_roll)?;
        unit.detached_heading = (unit.heading() + f64::from(offset)).rem_euclid(360.0);
        if let Some(motion) = unit.motion.as_mut() {
            motion.heading = unit.detached_heading;
            motion.desired_heading = motion.heading;
        }
        let mut impacts = Vec::new();
        let mut remaining = if safe { 0 } else { damage };
        while remaining > 0 {
            let amount = remaining.min(5);
            remaining -= amount;
            let impact = super::vehicle_impact::resolve_followup_in_candidate(
                world,
                id,
                arc,
                super::vehicle_impact::ImpactRequest {
                    amount,
                    armor_piercing: None,
                    rear: arc == BattleHitArc::Rear,
                    attacker: None,
                },
                rules.vehicle_impact,
            )?;
            super::piloting::append_feedback(
                &mut pilot_notices,
                impact.pilot_notices.iter().cloned(),
                notices.len(),
            );
            notices.extend(impact.notices.iter().chain(&impact.broadcasts).cloned());
            impacts.push(impact);
            if wet {
                world.btech.vehicles.get_mut(&id).unwrap().inferno_remaining = 0;
            }
        }
        let mines = if position.is_some() {
            super::mine_event::resolve(world, id, BattleMineTriggerReason::Fall, rules, character)?
        } else {
            super::BattleMineEventReport {
                unit: id,
                reason: BattleMineTriggerReason::Fall,
                blasts: Vec::new(),
                triggers: 0,
                notices: Vec::new(),
                pilot_notices: Vec::new(),
            }
        };
        super::piloting::append_feedback(
            &mut pilot_notices,
            mines.pilot_notices.iter().cloned(),
            notices.len(),
        );
        notices.extend(mines.notices.iter().cloned());
        Ok(BattleVehicleFallReport {
            ice_break,
            avoidance,
            experience_messages,
            pilot_injury,
            character_injury,
            direction_roll,
            arc,
            damage,
            impacts,
            mines,
            pilot_notices,
            notices,
        })
    })
}
