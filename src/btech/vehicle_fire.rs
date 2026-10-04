//! Vehicle firing composes shared admission, launch, defenses and target damage in one candidate.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result};

/// Host policies for a tactical vehicle shot and each side's anatomy-specific consequences.
#[derive(Debug, Clone, Copy)]
pub struct BattleVehicleShotRules {
    pub shot: BattleShotRules,
    pub shooter_criticals: BattleVehicleCriticalRules,
}

/// A shot fired by a vehicle: a complete tactical state transition whose feedback the
/// host must still publish in its transaction.
pub type BattleVehicleShotReport =
    super::ShotReport<super::BattleVehicleWeaponUse, super::BattleVehicleInternalDamage>;

/// Resolve an admitted tactical shot against a Mech or vehicle without publishing partial state.
/// Shared aim dice precede launch and AMS dice; target-owned cluster and impact dice follow.
/// Beacon effects use each target's hit routing; direct heat effects share unit storage adapters.
pub fn fire_vehicle_shot(
    world: &mut World,
    shooter: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    weapon_index: usize,
    rules: BattleVehicleShotRules,
) -> Result<BattleVehicleShotReport> {
    anyhow::ensure!(
        !(pilot == shooter && world.btech.controllers().contains_key(&shooter)),
        "Autopilot actor token is internal"
    );
    let report = fire_shot(
        world,
        shooter,
        pilot,
        super::shot::ShotTarget {
            unit: target,
            coordinate: None,
        },
        weapon_index,
        rules,
        None,
    )?;
    let _ = super::autopilot::manual_takeover(world, shooter);
    Ok(report)
}

/// Fire one ordinary direct weapon for an attached autopilot.  The shooter ID
/// is used as the internal actor token; it is never treated as a player.
pub(crate) fn fire_vehicle_shot_autopilot(
    world: &mut World,
    shooter: ObjectId,
    target: ObjectId,
    weapon_index: usize,
    rules: BattleVehicleShotRules,
) -> Result<BattleVehicleShotReport> {
    fire_shot(
        world,
        shooter,
        shooter,
        super::shot::ShotTarget {
            unit: target,
            coordinate: None,
        },
        weapon_index,
        rules,
        None,
    )
}

/// The host owns character feedback and evacuation for both participants.
pub(super) fn fire_vehicle_shot_in_action(
    world: &mut World,
    shooter: ObjectId,
    pilot: ObjectId,
    target: super::shot::ShotTarget,
    weapon_index: usize,
    rules: BattleVehicleShotRules,
    xp: &crate::config::XpConfig,
) -> Result<BattleVehicleShotReport> {
    anyhow::ensure!(
        !(pilot == shooter && world.btech.controllers().contains_key(&shooter)),
        "Autopilot actor token is internal"
    );
    let report = fire_shot(world, shooter, pilot, target, weapon_index, rules, Some(xp))?;
    if pilot != shooter {
        let _ = super::autopilot::manual_takeover(world, shooter);
    }
    Ok(report)
}

/// Share launch, defenses and target damage regardless of publication policy.
fn fire_shot(
    world: &mut World,
    shooter: ObjectId,
    pilot: ObjectId,
    target: super::shot::ShotTarget,
    weapon_index: usize,
    rules: BattleVehicleShotRules,
    xp: Option<&crate::config::XpConfig>,
) -> Result<BattleVehicleShotReport> {
    let mut attempt = super::autopilot::diagnostics::Attempt::begin();
    let admission = super::autopilot::diagnostics::combat("admission_aim");
    let loadouts =
        super::loadout_context::LoadoutScope::participants(&world.btech, [shooter, target.unit]);
    let operator = super::combat_operator::controlled(world, shooter, pilot)?;
    let character = xp.is_some();
    let attacker = world
        .btech
        .vehicles()
        .get(&shooter)
        .context("Vehicle is unavailable")?;
    attacker.check_spotter_fire(shooter, weapon_index)?;
    let indirect =
        super::spotter::indirect_target_for_source(world, operator.source, weapon_index)?;
    let requested = target;
    let target = indirect.map_or(requested.unit, |link| link.target);
    let coordinate = if indirect.is_some() {
        let position = super::scanner::scanner_unit(world, target)
            .and_then(|unit| unit.position)
            .context("Target is not placed")?;
        Some(HexCoordinate {
            x: i32::from(position.x),
            y: i32::from(position.y),
        })
    } else {
        requested.coordinate
    };
    let mut aim_dice = attacker.dice.clone();
    let (mut aim, prepared) = super::vehicle_shot::check_with_dice(
        world,
        shooter,
        pilot,
        target,
        weapon_index,
        super::vehicle_shot::VehicleShotAdmission {
            rules: rules.shot,
            character,
        },
        &mut aim_dice,
    )?;
    let attacker = &world.btech.vehicles()[&shooter];
    let weapon = attacker.weapon_readiness(weapon_index)?.weapon;
    let ammunition = attacker.ammunition_mode(weapon_index)?;

    let source_field = electronic_field(world, shooter)?;
    let target_field = electronic_field(world, target)?;
    let angel_blocked = source_field.angel_disturbed || target_field.angel_protected;
    let submerged = super::weapon_geometry::submerged(world, shooter, weapon_index)?;
    drop(loadouts);
    drop(admission);
    let mut candidate = super::shot_transaction::ShotCandidate::new(world);
    let launch_measurement = super::autopilot::diagnostics::combat("launch_defenses");
    let experience_messages = if character && let Some(link) = indirect {
        super::spotter::award_indirect_experience(&mut candidate, shooter, link, &mut aim)?
    } else {
        Vec::new()
    };
    candidate.btech.vehicles.get_mut(&shooter).unwrap().dice = aim_dice;
    let mut launch = super::vehicle_launch::launch_prepared(
        &mut candidate,
        BattleVehicleLaunchRequest {
            shooter,
            pilot,
            weapon_index,
            distance: aim.distance,
            target_number: aim.subtotal(),
            streak_confused: angel_blocked,
            glancing: rules.shot.glancing,
            critical_rules: rules.shooter_criticals,
        },
        prepared,
    )?;
    let aimed = super::aimed_hit::prepare(
        &mut candidate,
        super::aimed_hit::AimedLaunch {
            shooter,
            target,
            index: weapon_index,
            weapon,
            fire_mode: launch.expenditure.fire_mode,
            ammunition,
            launched: launch.expenditure.launched,
            hit: launch.hit,
            in_range: aim.range.is_some(),
        },
    )?;
    let resolved = super::target_hit::TargetHit {
        hit: launch.hit,
        glancing: launch.glancing,
    }
    .for_weapon(
        weapon,
        ammunition,
        launch.roll,
        aim.subtotal(),
        rules.shot.glancing,
        angel_blocked,
    );
    launch.glancing = resolved.glancing;
    let mut ams = if resolved.hit && weapon.profile().missiles > 0 && !ammunition.bypasses_ams() {
        super::ams::intercept(&mut candidate, shooter, target, weapon.profile().missiles)?
    } else {
        None
    };
    let super::direct_effects::DirectEffects {
        narc,
        cooling,
        heat_transfer,
        woods: thermal_woods,
        missed_terrain,
    } = super::direct_effects::resolve(
        &mut candidate,
        super::direct_effects::DirectEffectRequest {
            submerged,
            shooter,
            target,
            weapon,
            ammunition,
            fire_mode: launch.expenditure.fire_mode,
            hit: resolved.hit,
            launched: launch.expenditure.launched,
            intercepted: ams.as_ref().is_some_and(|ams| ams.shot_down > 0),
            vehicle_impact: rules.shot.vehicle_impact,
            hit_rules: rules.shot.hit,
            hit_arc_mode: rules.shot.hit_arc_mode,
            woods_damage: rules.shot.aim.woods_damage,
            range_damage: rules.shot.range_damage,
            damage_penalty: 0,
            distance: aim.distance,
            gatling_damage: launch.expenditure.gatling_damage,
            coordinate,
        },
    )?;
    let experience = xp.map(|config| super::gunnery_experience::GunneryAwardContext {
        config,
        request: BattleGunneryAwardRequest {
            tsm_tow_bonus: rules.shot.tsm_tow_bonus,
            attacker: shooter,
            pilot,
            target,
            weapon,
            damage: 0,
            base_to_hit: aim.subtotal().unwrap_or_default(),
            extended_gunnery: rules.shot.extended_gunnery,
            extended_piloting: rules.shot.extended_piloting,
            use_unit_modifier: config.perunit_xpmod != 0,
            now: crate::clock::wall_time(),
        },
    });
    #[cfg(test)]
    super::shot_transaction::checkpoint(super::shot_transaction::FailurePoint::Expenditure)?;
    drop(launch_measurement);
    let damage_measurement = super::autopilot::diagnostics::combat("damage_recoil");
    let salvo = if resolved.hit && launch.expenditure.launched && ammunition.is_swarm() {
        Some(super::BattleTargetSalvo::Swarm(super::swarm::resolve(
            &mut candidate,
            super::swarm::SwarmRequest {
                shooter,
                target,
                weapon: (&launch.expenditure).into(),
                rules: rules.shot,
                first_hit: resolved.hit,
                first_roll: launch.roll,
                target_number: aim.subtotal().unwrap_or_default(),
                glancing: resolved.glancing,
                character,
                experience,
            },
        )?))
    } else if !resolved.hit || narc.is_some() || cooling.is_some() || heat_transfer > 0 {
        None
    } else if candidate.btech.vehicles().contains_key(&target) {
        Some(super::target_salvo::resolve_vehicle_target_in_candidate(
            &mut candidate,
            shooter,
            target,
            BattleVehicleSalvoRequest {
                range_damage: rules.shot.range_damage,
                damage_penalty: 0,
                weapon,
                ammunition,
                fire_mode: launch.expenditure.fire_mode,
                gatling_damage: launch.expenditure.gatling_damage,
                distance: aim.distance,
                glancing: resolved.glancing,
                guidance_blocked: source_field.blocks_outgoing_guidance()
                    || target_field.blocks_incoming_guidance(),
                angel_blocked,
                intercepted: ams.as_ref().map_or(0, |report| report.shot_down),
            },
            rules.shot.hit_arc_mode,
            rules.shot.vehicle_impact,
            super::vehicle_salvo::SalvoContext {
                submerged,
                woods_damage: rules.shot.aim.woods_damage,
                incoming: None,
                attacker: Some(shooter),
                experience,
                aimed,
            },
        )?)
    } else {
        Some(BattleTargetSalvo::Mech(
            super::salvo::resolve_salvo_in_candidate(
                &mut candidate,
                shooter,
                target,
                &launch.expenditure,
                super::salvo::ShotDamage {
                    submerged,
                    woods_damage: rules.shot.aim.woods_damage,
                    range_damage: rules.shot.range_damage,
                    aimed,
                    incoming: None,
                    rules: BattleFallRules {
                        vehicle_impact: rules.shot.vehicle_impact,
                        stacking: rules.shot.stacking,
                        stagger: rules.shot.stagger,
                        hit: rules.shot.hit,
                        extended_piloting: rules.shot.extended_piloting,
                        toughness: rules.shot.target_toughness,
                    },
                    hit_arc_mode: rules.shot.hit_arc_mode,
                    glancing: resolved.glancing,
                    character,
                    intercepted: ams.as_ref().map_or(0, |report| report.shot_down),
                    experience,
                },
            )?,
        ))
    };
    if let Some(defense) = &mut ams {
        defense.shot_down = defense.shot_down.min(if let Some(pod) = &narc {
            u8::from(pod.hit)
        } else {
            salvo
                .as_ref()
                .and_then(BattleTargetSalvo::missiles_before_defense)
                .unwrap_or(0)
        });
    }

    super::shot_counters::record(
        &mut candidate,
        shooter,
        launch.expenditure.launched,
        coordinate.is_some(),
        super::shot_counters::hit(launch.roll, aim.subtotal(), rules.shot.glancing),
    )?;
    #[cfg(test)]
    super::shot_transaction::checkpoint(super::shot_transaction::FailurePoint::Damage)?;
    drop(damage_measurement);
    #[cfg(test)]
    super::shot_transaction::checkpoint(super::shot_transaction::FailurePoint::Validation)?;
    candidate.btech.validate_action(&candidate)?;
    let _publication = super::autopilot::diagnostics::combat("publication");
    attempt.succeed();
    candidate.commit(world);
    let BattleVehicleLaunch {
        ammunition_warning,
        launch_notices,
        roll,
        loader_destroyed,
        jammed,
        propellant_roll,
        misload,
        hit,
        glancing,
        expenditure,
    } = launch;
    Ok(BattleVehicleShotReport {
        shooter,
        target,
        weapon_index,
        target_number: aim.subtotal(),
        aim,
        coordinate,
        roll,
        launched: expenditure.launched,
        hit,
        jammed,
        loader_destroyed,
        propellant_roll,
        misload,
        glancing,
        expenditure,
        ammunition_warning,
        launch_notices,
        recoil: None,
        streak_confused: weapon.is_streak() && angel_blocked,
        ams,
        narc,
        cooling,
        heat_transfer,
        thermal_woods,
        missed_terrain,
        salvo,
        experience_messages,
    })
}

impl BattleVehicleShotReport {
    /// Collect cockpit consequences in launch, defense, damage and final-outcome order.
    pub fn notices(&self) -> Vec<BattleNotice> {
        self.notices_with_feedback(&mut Vec::new())
    }

    /// Retain private damage rolls alongside their ordered cockpit consequences.
    pub(crate) fn notices_with_feedback(
        &self,
        private: &mut Vec<super::BattlePilotNotice>,
    ) -> Vec<BattleNotice> {
        if let Some(misload) = &self.misload {
            super::piloting::append_feedback(private, misload.pilot_notices.iter().cloned(), 0);
            return misload.notices.clone();
        }
        if self.jammed || self.loader_destroyed {
            return Vec::new();
        }
        if !self.expenditure.launched {
            let mut notices = self.launch_notices.clone();
            notices.extend(super::launch_feedback::streak_failure(self.shooter));
            return notices;
        }
        let mut notices = self.launch_notices.clone();
        if let Some(text) = &self.ammunition_warning {
            notices.push(BattleNotice {
                unit: self.shooter,
                text: text.clone(),
            });
        }
        if let Some(terrain) = &self.missed_terrain {
            notices.extend(terrain.notices.iter().cloned());
        }
        if let Some(woods) = &self.thermal_woods {
            notices.extend(woods.notices.iter().cloned());
        }
        notices.extend(super::fire_feedback::glancing_notices(
            self.target,
            self.expenditure.weapon,
            self.glancing,
        ));
        notices.extend(super::direct_effects::thermal_notices(
            self.shooter,
            self.target,
            self.cooling,
            self.heat_transfer,
        ));
        notices.extend(super::fire_feedback::streak_notices(
            self.shooter,
            self.streak_confused,
        ));
        if let Some(pod) = &self.narc {
            notices.extend(pod.notices.iter().cloned());
            notices.extend(pod.notices(self.shooter, self.target));
        }
        if let Some(ams) = &self.ams
            && self.narc.as_ref().is_none_or(|pod| !pod.hit)
        {
            notices.extend(
                ams.notices(
                    self.shooter,
                    self.target,
                    self.salvo
                        .as_ref()
                        .and_then(BattleTargetSalvo::missiles_before_defense),
                ),
            );
        }
        if let Some(salvo) = &self.salvo {
            let mut nested = Vec::new();
            let damage = salvo.notices_with_feedback(self.shooter, self.target, &mut nested);
            super::piloting::append_feedback(private, nested, notices.len());
            notices.extend(damage);
        }
        notices
    }
}
