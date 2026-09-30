//! Vehicle inferno outcomes, section fires and crew extinguishing share committed damage transactions.
use super::*;
use crate::{Config, Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Target-owned inferno outcome after missile clustering and interception.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish inferno effects with the enclosing attack"]
pub struct BattleVehicleInfernoHit {
    pub missiles: u16,
    /// Standard mobile-vehicle heat explosion check; advanced and stationary outcomes do not roll it.
    pub explosion_roll: Option<u8>,
    /// Stationary-unit jelly duration added by this exposure.
    pub burn_seconds: u32,
    pub damage: Vec<BattleVehicleArmorDamage>,
    pub explosion: Option<BattleVehicleExplosion>,
    /// Includes visibility-filtered observer notices captured before damage.
    pub notices: Vec<BattleNotice>,
    /// Pilot-only control messages indexed into the ordinary notice stream.
    pub pilot_notices: Vec<BattlePilotNotice>,
    /// Damage broadcasts whose audiences are resolved by the enclosing host action.
    pub broadcasts: Vec<BattleNotice>,
}

impl BattleVehicle {
    /// Destruction cancels all burning and crew fire-suppression events together.
    pub(super) fn clear_fires(&mut self) {
        self.burning_sections.clear();
        self.extinguishing = None;
        self.inferno_remaining = 0;
    }

    /// Seconds until each section's next fire pulse, retained even if that section is destroyed.
    pub fn burning_sections(&self) -> &std::collections::BTreeMap<BattleVehicleSection, u8> {
        &self.burning_sections
    }

    /// Seconds until the crew completes its existing extinguishing attempt.
    pub fn extinguishing(&self) -> Option<u8> {
        self.extinguishing
    }

    /// Remaining stationary-unit inferno duration, independent of passive weapon heat.
    pub fn inferno_remaining(&self) -> u32 {
        self.inferno_remaining
    }
}

/// Apply positive inferno exposure atomically using the recipient's existing damage and explosion paths.
pub fn resolve_vehicle_inferno_hit(
    world: &mut World,
    target: ObjectId,
    missiles: u16,
    rules: BattleVehicleImpactRules,
) -> Result<BattleVehicleInfernoHit> {
    resolve_inferno_from(world, target, missiles, rules, None)
}

/// Initial missile exposure retains its author; scheduled fire pulses do not.
pub(super) fn resolve_inferno_from(
    world: &mut World,
    target: ObjectId,
    missiles: u16,
    rules: BattleVehicleImpactRules,
    attacker: Option<ObjectId>,
) -> Result<BattleVehicleInfernoHit> {
    ensure!(missiles > 0, "Inferno hit requires at least one missile");
    let object = world
        .objects
        .get(&target)
        .context("Vehicle is unavailable")?;
    ensure!(
        !object.flags.contains(Flag::Going),
        "Vehicle is unavailable"
    );
    let unit = world
        .btech
        .vehicles()
        .get(&target)
        .context("Vehicle is unavailable")?;
    ensure!(!unit.is_destroyed(), "Vehicle is destroyed");
    let stationary = unit.definition().movement == BattleVehicleMovement::Stationary;
    let burning = unit.inferno_remaining > 0 || !unit.burning_sections.is_empty();
    let mut report = BattleVehicleInfernoHit {
        missiles,
        explosion_roll: None,
        burn_seconds: 0,
        damage: Vec::new(),
        explosion: None,
        notices: super::inferno_hit::exposure_notices(world, target, burning),
        pilot_notices: Vec::new(),
        broadcasts: Vec::new(),
    };
    world.attempt(|world| {
        if stationary {
            report.burn_seconds = super::inferno_hit::duration(missiles);
            add_jelly(world, target, i64::from(report.burn_seconds))?;
        } else if rules.advanced_fire {
            let effects = ignite_sections(world, target, rules.criticals, attacker)?;
            report.damage = effects.damage;
            super::piloting::append_feedback(
                &mut report.pilot_notices,
                effects.pilot_notices.iter().cloned(),
                report.notices.len(),
            );
            report.notices.extend(effects.notices);
            report.broadcasts.extend(effects.broadcasts);
        } else {
            let effect = heat_explosion(world, target, attacker)?;
            report.explosion_roll = effect.explosion_roll;
            report.explosion = effect.explosion;
            super::piloting::append_feedback(
                &mut report.pilot_notices,
                effect.pilot_notices.iter().cloned(),
                report.notices.len(),
            );
            report.notices.extend(effect.notices);
            report.broadcasts.extend(effect.broadcasts);
        }
        world.btech.validate_action(world)?;
        Ok(report)
    })
}

/// Vehicle response to blast heat, separate from missile inferno exposure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish heat effects with the enclosing damage transaction"]
pub struct BattleVehicleHeatExposure {
    /// Signed duration adjustment; the resulting timer is bounded to at least one second.
    pub burn_seconds: i64,
    pub fire: Option<BattleVehicleFireExposure>,
    pub explosion_roll: Option<u8>,
    pub explosion: Option<BattleVehicleExplosion>,
    pub notices: Vec<BattleNotice>,
    /// Pilot-only control messages indexed into the ordinary notice stream.
    pub pilot_notices: Vec<BattlePilotNotice>,
    pub broadcasts: Vec<BattleNotice>,
}

/// Apply blast heat through existing fire checks, explosions or stationary jelly duration.
/// Mobile units check even zero heat, and blasts can affect surviving wreck sections.
pub fn resolve_vehicle_heat_exposure(
    world: &mut World,
    target: ObjectId,
    heat: i32,
    rules: BattleVehicleImpactRules,
) -> Result<BattleVehicleHeatExposure> {
    let object = world
        .objects
        .get(&target)
        .context("Vehicle is unavailable")?;
    ensure!(
        !object.flags.contains(Flag::Going),
        "Vehicle is unavailable"
    );
    let unit = world
        .btech
        .vehicles()
        .get(&target)
        .context("Vehicle is unavailable")?;
    let stationary = unit.definition().movement == BattleVehicleMovement::Stationary;
    world.attempt(|world| {
        let mut report = BattleVehicleHeatExposure {
            burn_seconds: 0,
            fire: None,
            explosion_roll: None,
            explosion: None,
            notices: Vec::new(),
            pilot_notices: Vec::new(),
            broadcasts: Vec::new(),
        };
        if stationary {
            report.burn_seconds = i64::from(heat) * 6;
            add_jelly(world, target, report.burn_seconds)?;
        } else if rules.advanced_fire {
            let fire = resolve_vehicle_fire_exposure(world, target, rules.criticals)?;
            super::piloting::append_feedback(
                &mut report.pilot_notices,
                fire.effects.pilot_notices.iter().cloned(),
                report.notices.len(),
            );
            report.notices.extend(fire.effects.notices.clone());
            report.broadcasts.extend(fire.effects.broadcasts.clone());
            report.fire = Some(fire);
        } else {
            report = heat_explosion(world, target, None)?;
        }
        world.btech.validate_action(world)?;
        Ok(report)
    })
}

/// Jelly accounting is shared by missile exposure and blast heat.
fn add_jelly(world: &mut World, target: ObjectId, seconds: i64) -> Result<()> {
    let unit = world.btech.vehicles.get_mut(&target).unwrap();
    unit.inferno_remaining = super::inferno::adjusted_duration(unit.inferno_remaining, seconds)?;
    Ok(())
}

/// Standard mobile heat uses the same target-owned explosion check for every source.
fn heat_explosion(
    world: &mut World,
    target: ObjectId,
    attacker: Option<ObjectId>,
) -> Result<BattleVehicleHeatExposure> {
    let roll = world
        .btech
        .vehicles
        .get_mut(&target)
        .unwrap()
        .dice
        .generic_roll();
    let mut report = BattleVehicleHeatExposure {
        burn_seconds: 0,
        fire: None,
        explosion_roll: Some(roll),
        explosion: None,
        notices: Vec::new(),
        pilot_notices: Vec::new(),
        broadcasts: Vec::new(),
    };
    if roll <= 8 {
        return Ok(report);
    }
    report.notices.extend(super::broadcast::observer_notices(
        world,
        target,
        "explodes!",
    ));
    report.notices.push(BattleNotice {
        unit: target,
        text: "The heat's too much for your vehicle! It blows up!".into(),
    });
    world.btech.vehicles.get_mut(&target).unwrap().clear_fires();
    let was_destroyed = world.btech.vehicles()[&target].is_destroyed();
    let explosion = super::vehicle_explosion::explode_followup_in_candidate(world, target, false)?;
    let destroyed = world.btech.vehicles()[&target].is_destroyed();
    super::kill_counters::transition(world, target, attacker, was_destroyed, destroyed)?;
    report.notices.extend(explosion.notices.clone());
    report.broadcasts.extend(explosion.broadcasts.clone());
    report.explosion = Some(explosion);
    Ok(report)
}

/// Fire has a known section; it enters armor damage directly without a hit-location roll.
fn burn_damage(
    world: &mut World,
    id: ObjectId,
    section: BattleVehicleSection,
    amount: u8,
    rules: BattleVehicleCriticalRules,
    attacker: Option<ObjectId>,
) -> Result<BattleVehicleArmorDamage> {
    let report = super::vehicle_armor_damage::resolve_rear_followup_in_candidate(
        world,
        id,
        BattleVehicleArmorHit {
            damage_class: BattleDamageClass::Ordinary,
            section,
            amount: u32::from(amount),
            through_armor_critical: false,
            armor_piercing: None,
        },
        false,
        rules,
        super::vehicle_internal_damage::DamageContext {
            attacker,
            ..Default::default()
        },
    )?;
    Ok(report)
}

/// Scheduled fire feedback retains character injury outcomes until host publication.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[must_use = "Publish fire notices, character injuries and casualties with the enclosing action"]
pub struct BattleVehicleFireTick {
    pub notices: Vec<BattleNotice>,
    /// Pilot-only control messages indexed into the ordinary notice stream.
    pub pilot_notices: Vec<BattlePilotNotice>,
    pub character_injuries: Vec<BattleCharacterPilotInjury>,
}

/// Advance all vehicle fires atomically, including shutdown and unplaced units.
/// Failed nested damage leaves timers and dice untouched for the next server commit attempt.
pub fn advance_vehicle_fires(world: &mut World, config: &Config) -> Result<BattleVehicleFireTick> {
    let ids: Vec<_> = world
        .btech
        .vehicles()
        .iter()
        .filter(|(id, unit)| {
            (unit.inferno_remaining > 0
                || !unit.burning_sections.is_empty()
                || unit.extinguishing.is_some())
                && world
                    .objects
                    .get(id)
                    .is_some_and(|object| !object.flags.contains(Flag::Going))
        })
        .map(|(&id, _)| id)
        .collect();
    let mut candidate = world.clone();
    let mut notices = Vec::new();
    let mut pilot_notices = Vec::new();
    let mut character_injuries = Vec::new();
    for id in ids {
        let unit = candidate.btech.vehicles.get_mut(&id).unwrap();
        if unit.inferno_remaining > 0 {
            unit.inferno_remaining -= 1;
            if unit.inferno_remaining == 0 {
                notices.push(BattleNotice {
                    unit: id,
                    text: "You feel suddenly far cooler as the fires finally die.".into(),
                });
            }
        }
        let due: Vec<_> = unit
            .burning_sections
            .iter_mut()
            .filter_map(|(&section, remaining)| {
                if *remaining == 1 {
                    return Some(section);
                }
                *remaining -= 1;
                None
            })
            .collect();
        let toughness = unit
            .pilot()
            .and_then(|pilot| candidate.btech.character_values().get(&pilot))
            .is_some_and(|values| super::advantages::enabled(values, "Toughness"));
        let rules = BattleVehicleImpactRules::configured(&config.battletech, toughness).criticals;
        for section in due {
            let unit = candidate.btech.vehicles.get_mut(&id).unwrap();
            if unit.burning_sections.remove(&section).is_none() {
                continue;
            }
            // A pending pulse draws its damage even if the section was destroyed before it ran.
            let amount = unit.dice.d6();
            if unit.sections()[&section].internal == 0 {
                continue;
            }
            notices.push(BattleNotice {
                unit: id,
                text: format!(
                    "[fg=red bold]Your {} takes damage from the fire![reset]",
                    section.name().replace('_', " ")
                ),
            });
            let before = candidate.clone();
            let damage = burn_damage(&mut candidate, id, section, amount, rules, None)?;
            super::vehicle_injuries::collect_armor(&damage, &mut character_injuries);
            super::piloting::append_feedback(
                &mut pilot_notices,
                damage.pilot_notices.iter().cloned(),
                notices.len(),
            );
            notices.extend(damage.notices);
            for broadcast in damage.broadcasts {
                notices.extend(super::broadcast::observer_notices(
                    &before,
                    broadcast.unit,
                    &broadcast.text,
                ));
            }
            let unit = candidate.btech.vehicles.get_mut(&id).unwrap();
            if amount > 1 && unit.sections()[&section].internal > 0 {
                unit.burning_sections.insert(section, 60);
                continue;
            }
            if unit.sections()[&section].internal > 0 {
                notices.push(BattleNotice {
                    unit: id,
                    text: format!(
                        "The fire burning on your {} finally goes out.",
                        section.name().replace('_', " ")
                    ),
                });
            }
            if unit.burning_sections.is_empty() {
                notices.extend(super::broadcast::observer_notices(
                    &before,
                    id,
                    "is no longer engulfed in flames.",
                ));
            }
        }
        let unit = candidate.btech.vehicles.get_mut(&id).unwrap();
        if let Some(remaining) = &mut unit.extinguishing {
            *remaining -= 1;
            if *remaining == 0 {
                unit.extinguishing = None;
                if !unit.burning_sections.is_empty() {
                    unit.burning_sections.clear();
                    notices.push(BattleNotice {
                        unit: id,
                        text: "You manage to dowse the fire.".into(),
                    });
                    notices.extend(super::broadcast::observer_notices(
                        &candidate,
                        id,
                        "is no longer engulfed in flames.",
                    ));
                }
            }
        }
    }
    candidate.btech.validate_action(&candidate)?;
    *world = candidate;
    Ok(BattleVehicleFireTick {
        notices,
        pilot_notices,
        character_injuries,
    })
}

/// Begin a two-minute crew attempt while the vehicle is shut down; later startup does not cancel it.
pub fn begin_vehicle_extinguishing(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
) -> Result<BattleNotice> {
    super::vehicle_power::controlled(world, id, pilot)?;
    let unit = &world.btech.vehicles()[&id];
    ensure!(
        unit.power() != BattlePower::Running,
        "Your tank is started! You can not extinguish the flames while your tank is started!"
    );
    ensure!(
        !unit.burning_sections.is_empty(),
        "This unit is not on fire!"
    );
    ensure!(
        unit.extinguishing.is_none(),
        "You're already trying to put out the fire!"
    );
    world.btech.vehicles.get_mut(&id).unwrap().extinguishing = Some(120);
    Ok(BattleNotice {
        unit: id,
        text: "You begin to extinguish the fires!".into(),
    })
}

/// Commit the timer and its notice together for native and scripted cockpit actions.
pub fn begin_vehicle_extinguishing_action(
    scripts: &crate::Scripts,
    id: ObjectId,
    pilot: ObjectId,
) -> Result<()> {
    scripts.atomic(|_| {
        let notice = begin_vehicle_extinguishing(&mut scripts.world.borrow_mut(), id, pilot)?;
        super::notify_unit(scripts, notice)
    })
}

/// Native extinguishing shares the same transaction as Lua.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    _: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let id = ctx
            .scripts
            .world
            .borrow()
            .objects
            .get(&ctx.player)
            .and_then(|player| player.location)
            .context("Enter a unit first")?;
        begin_vehicle_extinguishing_action(ctx.scripts, id, ctx.player)
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

/// Shared section-fire effects independent of whether missiles or terrain caused ignition.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct BattleVehicleFireEffects {
    pub damage: Vec<BattleVehicleArmorDamage>,
    pub notices: Vec<BattleNotice>,
    /// Pilot-only control messages indexed into the ordinary notice stream.
    pub pilot_notices: Vec<BattlePilotNotice>,
    pub broadcasts: Vec<BattleNotice>,
}

/// Ignite only new sections, retaining existing timers and using the common armor resolver.
fn ignite_sections(
    world: &mut World,
    target: ObjectId,
    rules: BattleVehicleCriticalRules,
    attacker: Option<ObjectId>,
) -> Result<BattleVehicleFireEffects> {
    let mut effects = BattleVehicleFireEffects::default();
    effects.notices.push(BattleNotice {
        unit: target,
        text: "You catch on fire!".into(),
    });
    effects.notices.extend(super::broadcast::observer_notices(
        world,
        target,
        "catches on fire!",
    ));
    let sections: Vec<_> = world.btech.vehicles()[&target]
        .sections()
        .keys()
        .copied()
        .collect();
    for section in sections {
        let unit = &world.btech.vehicles()[&target];
        if unit.sections()[&section].internal == 0 || unit.burning_sections.contains_key(&section) {
            continue;
        }
        let amount = world.btech.vehicles.get_mut(&target).unwrap().dice.d6();
        effects.notices.push(BattleNotice {
            unit: target,
            text: format!("Your {} catches on fire!", section.name().replace('_', " ")),
        });
        let damage = burn_damage(world, target, section, amount, rules, attacker)?;
        super::piloting::append_feedback(
            &mut effects.pilot_notices,
            damage.pilot_notices.iter().cloned(),
            effects.notices.len(),
        );
        effects.notices.extend(damage.notices.clone());
        effects.broadcasts.extend(damage.broadcasts.clone());
        effects.damage.push(damage);
        let unit = world.btech.vehicles.get_mut(&target).unwrap();
        unit.burning_sections.insert(section, 60);
    }
    Ok(effects)
}

/// Advanced terrain-fire check; the movement caller decides when a new burning hex is entered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish exposure effects within the enclosing movement transaction"]
pub struct BattleVehicleFireExposure {
    pub roll: u8,
    pub adjusted: u8,
    pub motive_roll: Option<u8>,
    pub effects: BattleVehicleFireEffects,
}

/// Resolve an admitted advanced fire exposure atomically, using the vehicle's saved dice.
pub fn resolve_vehicle_fire_exposure(
    world: &mut World,
    id: ObjectId,
    rules: BattleVehicleCriticalRules,
) -> Result<BattleVehicleFireExposure> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Vehicle is unavailable"
    );
    ensure!(
        world.btech.vehicles().contains_key(&id),
        "Vehicle is unavailable"
    );
    world.attempt(|world| {
        let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
        let roll = vehicle.dice.generic_roll();
        let adjusted =
            roll + super::vehicle_motive_effects::modifier(vehicle.definition().movement);
        let mut report = BattleVehicleFireExposure {
            roll,
            adjusted,
            motive_roll: None,
            effects: BattleVehicleFireEffects::default(),
        };
        if adjusted >= 8 {
            report.effects.notices.push(BattleNotice {
                unit: id,
                text: "[fg=red bold]You drive through a wall of searing flames![reset]".into(),
            });
        }
        match adjusted {
            0..=7 => (),
            8 | 9 => {
                report.effects.notices.push(BattleNotice {
                    unit: id,
                    text: "[fg=red bold]The fire damages your motive system![reset]".into(),
                });
                let roll = vehicle.dice.generic_roll();
                report.motive_roll = Some(roll);
                let (motive, penalty) =
                    super::vehicle_motive_effects::outcome(vehicle.definition().movement, roll);
                let (notices, broadcasts) =
                    super::vehicle_motive_effects::apply(vehicle, id, motive, penalty, Some(roll));
                report.effects.notices.extend(notices);
                report.effects.broadcasts.extend(broadcasts);
            }
            10 | 11 => {
                report.effects.notices.push(BattleNotice {
                    unit: id,
                    text: "[fg=red bold]The fire sweeps across your unit damaging it![reset]"
                        .into(),
                });
                // The reference samples all eight section slots, including absent and destroyed slots.
                let sections = [
                    BattleVehicleSection::Left,
                    BattleVehicleSection::Right,
                    BattleVehicleSection::Front,
                    BattleVehicleSection::Rear,
                    BattleVehicleSection::Turret,
                ];
                for index in 0..8 {
                    let unit = world.btech.vehicles.get_mut(&id).unwrap();
                    let amount = unit.dice.d6();
                    let Some(&section) = sections.get(index) else {
                        continue;
                    };
                    if unit
                        .sections()
                        .get(&section)
                        .is_none_or(|state| state.internal == 0)
                    {
                        continue;
                    }
                    let damage = burn_damage(world, id, section, amount, rules, None)?;
                    super::piloting::append_feedback(
                        &mut report.effects.pilot_notices,
                        damage.pilot_notices.iter().cloned(),
                        report.effects.notices.len(),
                    );
                    report.effects.notices.extend(damage.notices.clone());
                    report.effects.broadcasts.extend(damage.broadcasts.clone());
                    report.effects.damage.push(damage);
                }
            }
            _ => {
                let effects = ignite_sections(world, id, rules, None)?;
                report.effects.damage = effects.damage;
                super::piloting::append_feedback(
                    &mut report.effects.pilot_notices,
                    effects.pilot_notices.iter().cloned(),
                    report.effects.notices.len(),
                );
                report.effects.notices.extend(effects.notices);
                report.effects.broadcasts.extend(effects.broadcasts);
            }
        }
        world.btech.validate_action(world)?;
        Ok(report)
    })
}
