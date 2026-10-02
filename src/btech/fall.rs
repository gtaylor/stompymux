//! Durable biped posture and atomic ground/water tactical fall resolution.
use super::{
    BattleHitArc, BattleHitRules, BattlePilotInjury, BattlePilotingCheck, BattleSalvoGroup,
};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Ground posture changes eye height, movement and combat geometry.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattlePosture {
    #[default]
    Standing,
    Prone,
}

impl super::BattleUnit {
    /// Current persisted biped posture.
    pub fn posture(&self) -> BattlePosture {
        self.posture
    }
}

/// Change the common ground posture without imposing movement or fall-damage policy.
pub(super) fn set_prone(unit: &mut super::BattleUnit) {
    unit.hull_down = Default::default();
    unit.posture = super::BattlePosture::Prone;
    unit.facing = Default::default();
}

/// Rule choices for pilot protection and grouped fall hits.
#[derive(Debug, Clone, Copy)]
pub struct BattleFallRules {
    /// Vehicle damage policy for mines reached by this consequence chain.
    pub vehicle_impact: super::BattleVehicleImpactRules,
    /// Crowding policy for airborne critical-damage falls inside the damage transaction.
    pub stacking: super::BattleStackingRules,
    pub stagger: super::BattleStaggerMode,
    pub hit: BattleHitRules,
    pub extended_piloting: bool,
    pub toughness: bool,
}

/// Completed tactical fall; callers stage notices and consume any remaining impact effects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Stage fall and injury notices and consume remaining impact effects before committing"]
pub struct BattleFallReport {
    /// Combat-safe units skip the personal-injury check.
    pub avoidance: Option<BattlePilotingCheck>,
    /// Pilot captured before fall injuries can clear the assignment.
    pub pilot: Option<ObjectId>,
    /// Accepted protection-check diagnostics captured before fall damage.
    pub experience_messages: Vec<super::BattleChannelMessage>,
    pub pilot_injury: Option<BattlePilotInjury>,
    pub character_injury: Option<super::BattleCharacterPilotInjury>,
    pub direction_roll: u8,
    pub arc: BattleHitArc,
    pub damage: u32,
    pub groups: Vec<BattleSalvoGroup>,
    pub flooding: Vec<super::BattleSectionExposureReport>,
    pub inferno_notices: Vec<super::BattleNotice>,
    /// Mine activation after the fall damage sequence.
    pub mines: super::BattleMineEventReport,
    /// Ice fracture can cause nested water falls for this unit and its neighbors.
    pub ice_break: Option<Box<super::BattleSurfaceBreak>>,
}

/// Fall by the supplied damage multiplier (one for an ordinary same-level fall).
/// Water applies immersion and flooding; intact ice can fracture into nested water falls. Bridge falls select the deck or lower surface from the starting altitude.
pub fn resolve_fall(
    world: &mut World,
    id: ObjectId,
    levels: u8,
    rules: BattleFallRules,
) -> Result<BattleFallReport> {
    ensure!(levels > 0, "Fall multiplier must be positive");
    resolve_fall_inner(world, id, i32::from(levels), rules, false)
}

/// Zero-multiplier falls still change posture, roll pilot protection and apply immersion.
pub(super) fn resolve_zero_fall(
    world: &mut World,
    id: ObjectId,
    rules: BattleFallRules,
) -> Result<BattleFallReport> {
    resolve_fall_inner(world, id, 0, rules, false)
}

/// Reverse uphill crashes can have a negative protection modifier and no structural damage.
pub(super) fn resolve_signed_fall(
    world: &mut World,
    id: ObjectId,
    levels: i16,
    rules: BattleFallRules,
) -> Result<BattleFallReport> {
    resolve_fall_inner(world, id, i32::from(levels), rules, false)
}

/// Character-enabled fall for an action that owns casualty publication and rollback.
pub(super) fn resolve_character_fall(
    world: &mut World,
    id: ObjectId,
    levels: u8,
    rules: BattleFallRules,
) -> Result<BattleFallReport> {
    resolve_fall_inner(world, id, i32::from(levels), rules, true)
}

/// Character-enabled signed severity used by accelerated free-fall impacts.
pub(super) fn resolve_character_signed_fall(
    world: &mut World,
    id: ObjectId,
    levels: i16,
    rules: BattleFallRules,
) -> Result<BattleFallReport> {
    resolve_fall_inner(world, id, i32::from(levels), rules, true)
}

pub(super) fn resolve_contract_fall(
    world: &mut World,
    id: ObjectId,
    levels: i32,
    rules: BattleFallRules,
    character: bool,
    tons: u32,
) -> Result<BattleFallReport> {
    resolve_fall_inner_with_tonnage(world, id, levels, rules, character, Some(tons))
}

/// Shared posture, injury, flooding and direction resolution for both fall causes.
fn resolve_fall_inner(
    world: &mut World,
    id: ObjectId,
    levels: i32,
    rules: BattleFallRules,
    character: bool,
) -> Result<BattleFallReport> {
    resolve_fall_inner_with_tonnage(world, id, levels, rules, character, None)
}

fn resolve_fall_inner_with_tonnage(
    world: &mut World,
    id: ObjectId,
    levels: i32,
    rules: BattleFallRules,
    character: bool,
    tonnage: Option<u32>,
) -> Result<BattleFallReport> {
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit construction state is unavailable")?;
    ensure!(
        !unit.is_destroyed() || unit.free_fall().is_some(),
        "Unit is destroyed"
    );
    resolve_material_with_tonnage(world, id, levels, rules, character, tonnage)
}

/// Resolve an admitted environmental fall even if an earlier casualty destroyed this unit.
/// The enclosing action owns admission, casualty publication and rollback.
pub(super) fn resolve_material(
    world: &mut World,
    id: ObjectId,
    levels: i32,
    rules: BattleFallRules,
    character: bool,
) -> Result<BattleFallReport> {
    resolve_material_with_tonnage(world, id, levels, rules, character, None)
}

fn resolve_material_with_tonnage(
    world: &mut World,
    id: ObjectId,
    levels: i32,
    rules: BattleFallRules,
    character: bool,
    tonnage: Option<u32>,
) -> Result<BattleFallReport> {
    let object = world.objects.get(&id).context("Unit is unavailable")?;
    ensure!(
        !object.flags.contains(Flag::Going)
            && (character || !object.flags.contains(Flag::InCharacter)),
        "Fall requires a live tactical unit"
    );
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit construction state is unavailable")?;
    unit.validate()?;
    let position = unit.position();
    let (tile, gravity) = if let Some(position) = position {
        let map = world
            .btech
            .maps()
            .get(&position.map)
            .context("Map not found")?;
        (
            Some(map.base_hex(i64::from(position.x), i64::from(position.y))?),
            map.uses_special_rules().then_some(map.gravity),
        )
    } else {
        (None, None)
    };
    let below_bridge = tile.is_some_and(|tile| {
        tile.deck_height()
            .is_some_and(|deck| unit.elevation_level(tile) < i32::from(deck) - 2)
    });
    let below_ice = tile.is_some_and(|tile| {
        tile.terrain() == super::Terrain::Ice && unit.elevation_level(tile) < 0
    });
    let tons = tonnage.unwrap_or(u32::from(unit.definition().tons));
    let pilot = unit.pilot();
    let has_pilot = pilot.is_some();
    let safe = unit.combat_safe;
    world.attempt(|world| {
        let mut avoidance = if safe {
            None
        } else {
            Some(super::piloting::roll_piloting_i32(
                world,
                id,
                levels,
                rules.extended_piloting,
            )?)
        };
        let experience_messages = if character && let Some(check) = &mut avoidance {
            super::piloting::award_control_check(world, id, check, rules.extended_piloting)?
                .into_iter()
                .collect()
        } else {
            Vec::new()
        };
        let character_injury =
            if character && avoidance.is_some_and(|check| !check.success) && has_pilot {
                Some(super::injure_character_pilot(
                    world,
                    id,
                    1,
                    rules.toughness,
                )?)
            } else {
                None
            };
        let pilot_injury =
            if !character && avoidance.is_some_and(|check| !check.success) && has_pilot {
                Some(super::injure_tactical_pilot(world, id, 1, rules.toughness)?)
            } else {
                None
            };
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        if matches!(
            unit.stand_timer,
            Some(super::BattleStandTimer::Rising { .. })
        ) {
            unit.stand_timer = None;
        }
        unit.stagger.clear_damage();
        unit.ground_elevation = if below_bridge {
            Some(-1.0)
        } else if below_ice {
            Some(f64::from(
                tile.expect("ice fall has a terrain tile").surface_height(),
            ))
        } else {
            None
        };
        let airborne = unit.airborne();
        if let Some(event) = &mut unit.free_fall {
            event.land();
        }
        unit.flight = None;
        unit.orbital_drop = None;
        if airborne && !unit.is_destroyed() {
            unit.jump_stabilization = 12;
        }
        set_prone(unit);
        if let Some(motion) = &mut unit.motion {
            motion.speed = 0.0;
            motion.desired_speed = 0.0;
            motion.desired_heading = motion.heading;
        }
        let ice_break = if position.is_some() {
            if character {
                super::surface_break::check_ice_landing_in_action(world, id, rules)?
            } else {
                super::surface_break::check_ice_landing(world, id, rules)?
            }
            .map(Box::new)
        } else {
            None
        };
        let wet = if let Some(position) = position {
            let tile = world.btech.maps()[&position.map]
                .base_hex(i64::from(position.x), i64::from(position.y))?;
            tile.immerses(world.btech.constructed_units()[&id].elevation_level(tile))
        } else {
            false
        };
        let damage = super::fall_profile::damage(tons, levels, wet, gravity)?;
        let flooding = if position.is_none() {
            Vec::new()
        } else if character {
            super::flooding::flood_unit_in_action(world, id, rules)?
        } else {
            super::flood_unit(world, id, rules)?
        };
        let inferno_notices = if position.is_some() {
            super::extinguish_inferno_in_water(world, id)?
        } else {
            Vec::new()
        };
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        let direction_roll = unit.dice.d6();
        let (arc, offset) = super::fall_profile::direction(direction_roll)?;
        if let Some(motion) = &mut unit.motion {
            motion.speed = 0.0;
            motion.desired_speed = 0.0;
            motion.heading = (motion.heading + f64::from(offset)).rem_euclid(360.0);
            motion.desired_heading = motion.heading;
        }
        let mut groups = Vec::new();
        let mut remaining = if safe { 0 } else { damage };
        while remaining > 0 {
            let amount = remaining.min(5);
            remaining -= amount;
            let amount = amount as u16;
            let unit = &world.btech.constructed_units()[&id];
            let mut dice = unit.dice.clone();
            let roll = dice.generic_roll();
            let hit = rules.hit.resolve(unit, arc, roll, &mut dice)?;
            world.btech.constructed.get_mut(&id).unwrap().dice = dice;
            // Native falling continues rolling each five-point group after a lethal packet;
            // its damage entry point then ignores the already destroyed unit.
            if world.btech.constructed_units()[&id].is_destroyed() {
                continue;
            }
            let outcome = if character {
                super::impact::resolve_character_impact_with_rules(
                    world,
                    id,
                    hit,
                    amount,
                    Some(rules),
                )?
            } else {
                super::pilot_injury::resolve_tactical_impact_in_candidate(
                    world, id, hit, amount, rules, None,
                )?
            };
            groups.push(BattleSalvoGroup {
                damage: amount,
                hit,
                impact: outcome.impact,
                pilot_injuries: outcome.pilot_injuries,
                pilot_notices: outcome.pilot_notices,
                notices: outcome.notices,
                balance: outcome.balance,
                flooding: outcome.flooding,
            });
        }
        let mines = if position.is_some() {
            super::mine_event::resolve(
                world,
                id,
                super::BattleMineTriggerReason::Fall,
                rules,
                character,
            )?
        } else {
            super::BattleMineEventReport {
                unit: id,
                reason: super::BattleMineTriggerReason::Fall,
                blasts: Vec::new(),
                triggers: 0,
                notices: Vec::new(),
                pilot_notices: Vec::new(),
            }
        };
        Ok(BattleFallReport {
            experience_messages,
            avoidance,
            pilot,
            pilot_injury,
            character_injury,
            direction_roll,
            arc,
            damage,
            groups,
            flooding,
            inferno_notices,
            mines,
            ice_break,
        })
    })
}

impl BattleFallReport {
    /// Notices for applied pilot injury, initial flooding and subsequent grouped damage.
    pub fn notices(&self, unit: ObjectId) -> Vec<super::BattleNotice> {
        let mut notices = Vec::new();
        self.append_notices(unit, &mut notices, &mut Vec::new());
        notices
    }

    /// Append protection and damage feedback without losing private audience or ordering.
    pub fn append_notices(
        &self,
        unit: ObjectId,
        notices: &mut Vec<super::BattleNotice>,
        private: &mut Vec<super::BattlePilotNotice>,
    ) {
        if let Some(check) = &self.avoidance {
            super::piloting::capture_feedback(unit, self.pilot, check, notices, private);
        }
        if let Some(notice) = self
            .pilot_injury
            .as_ref()
            .and_then(|injury| injury.notice(unit))
        {
            notices.push(notice);
        }
        notices.extend(self.inferno_notices.clone());
        if let Some(fracture) = &self.ice_break {
            super::piloting::append_feedback(
                private,
                fracture.pilot_notices.iter().cloned(),
                notices.len(),
            );
            notices.extend(fracture.notices.iter().cloned());
        }
        for report in &self.flooding {
            super::piloting::append_feedback(
                private,
                report.pilot_notices.iter().cloned(),
                notices.len(),
            );
            notices.extend(report.notices.iter().cloned());
        }
        for group in &self.groups {
            super::piloting::append_feedback(
                private,
                group.pilot_notices.iter().cloned(),
                notices.len(),
            );
            notices.extend(group.notices.iter().cloned());
        }
        super::piloting::append_feedback(
            private,
            self.mines.pilot_notices.iter().cloned(),
            notices.len(),
        );
        notices.extend(self.mines.notices.iter().cloned());
    }
}

impl BattleFallRules {
    /// Standard host fall policy shared by shutdown, landing and towing actions.
    pub fn configured(settings: &crate::Config) -> Self {
        let config = &settings.battletech;
        Self {
            vehicle_impact: crate::BattleVehicleImpactRules::configured(config, false),
            stacking: super::BattleStackingRules {
                mode: config.stacking,
                damage_percent: config.stackdamage,
                hit_arcs: config.hit_arcs,
            },
            hit: super::BattleHitRules {
                inferno_penalty: config.inferno_penalty != 0,
                exile_stun_mode: config.exile_stun_code.clamp(0, 2) as u8,
            },
            stagger: super::BattleStaggerMode::from_setting(config.newstagger),
            extended_piloting: config.extended_piloting != 0,
            toughness: false,
        }
    }
}
