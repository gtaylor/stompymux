//! Vehicle firing composes shared admission, launch, defenses and target damage in one candidate.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result};

/// Host policies for a tactical vehicle shot and each side's anatomy-specific consequences.
#[derive(Debug, Clone, Copy)]
pub struct VehicleShotRules {
    pub shot: ShotRules,
    pub shooter_criticals: VehicleCriticalRules,
}

/// A shot fired by a vehicle: a complete tactical state transition whose feedback the
/// host must still publish in its transaction.
pub type VehicleShotReport =
    super::ShotReport<super::VehicleWeaponUse, super::VehicleInternalDamage>;

/// Resolve an admitted tactical shot against a Mech or vehicle without publishing partial state.
/// Shared aim dice precede launch and AMS dice; target-owned cluster and impact dice follow.
/// Beacon effects use each target's hit routing; direct heat effects share unit storage adapters.
pub fn fire_vehicle_shot(
    world: &mut World,
    shooter: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    weapon_index: usize,
    rules: VehicleShotRules,
) -> Result<VehicleShotReport> {
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
    rules: VehicleShotRules,
) -> Result<VehicleShotReport> {
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
    rules: VehicleShotRules,
    xp: &crate::config::XpConfig,
) -> Result<VehicleShotReport> {
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
    rules: VehicleShotRules,
    xp: Option<&crate::config::XpConfig>,
) -> Result<VehicleShotReport> {
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
    let observed =
        super::spotter::observed_target_for_source(world, operator.source, weapon_index)?;
    let indirect =
        super::spotter::indirect_target_for_source(world, operator.source, weapon_index)?;
    let requested = target;
    let target = observed.map_or(requested.unit, |link| link.target);
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
    let (aim, prepared) = super::vehicle_shot::check_with_dice(
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
    // A surviving Apollo system takes one from the launcher's Cluster Hits roll (Tactical
    // Operations: Advanced Units & Equipment p. 143).
    let apollo = weapon.is_mrm() && attacker.apollo_operational(weapon_index)?;
    // Firing a gun its machine gun array links fires every ready linked gun as one attack
    // (Total Warfare p. 137).
    let volley = attacker.array_volley(weapon_index)?;
    let submerged = super::weapon_geometry::submerged(world, shooter, weapon_index)?;
    drop(loadouts);
    drop(admission);
    let mut candidate = super::shot_transaction::ShotCandidate::new(world);
    let launch_measurement = super::autopilot::diagnostics::combat("launch_defenses");
    let experience_messages = if character && let Some(link) = indirect {
        super::spotter::award_indirect_experience(&mut candidate, shooter, link)?
    } else {
        Vec::new()
    };
    candidate.btech.vehicles.get_mut(&shooter).unwrap().dice = aim_dice;
    let mut launch = super::vehicle_launch::launch_prepared(
        &mut candidate,
        VehicleLaunchRequest {
            shooter,
            pilot,
            weapon_index,
            distance: aim.distance,
            target_number: aim.subtotal(),
            streak_confused: angel_blocked,
            critical_rules: rules.shooter_criticals,
        },
        prepared,
    )?;
    // The array's other guns spend their ammunition and recycle with the one fired.
    let array = super::machine_gun_array::fire_linked_guns(
        &mut candidate,
        shooter,
        pilot,
        weapon_index,
        volley.as_deref(),
        launch.expenditure.launched,
        &mut launch.launch_notices,
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
        angel_blocked,
    );
    launch.glancing = resolved.glancing;
    let mut ams = if resolved.hit && weapon.profile().missiles > 0 {
        super::ams::intercept(&mut candidate, shooter, target, weapon)?
    } else {
        None
    };
    // A single missile the defense destroyed never reaches the target; a cluster engagement
    // shifts the Cluster Hits roll instead.
    let missile_destroyed = ams
        .as_ref()
        .is_some_and(super::AmsReport::destroyed_missile);
    let ams_engaged = ams.is_some() && !missile_destroyed;
    let indirect_fire = aim.indirect.is_some();
    let super::direct_effects::DirectEffects {
        narc,
        heat_transfer,
        missed_terrain,
        missed_building,
    } = super::direct_effects::resolve(
        &mut candidate,
        super::direct_effects::DirectEffectRequest {
            shooter,
            target,
            weapon,
            ammunition,
            fire_mode: launch.expenditure.fire_mode,
            hit: resolved.hit,
            launched: launch.expenditure.launched,
            intercepted: missile_destroyed,
            vehicle_impact: rules.shot.vehicle_impact,
            distance: aim.distance,
            gatling_damage: launch.expenditure.gatling_damage,
            coordinate,
        },
    )?;
    let experience = xp.map(|config| super::gunnery_experience::GunneryAwardContext {
        config,
        request: GunneryAwardRequest {
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
    // A Swarm flight that misses its first target flies on to others (Tactical Operations:
    // Advanced Units & Equipment p. 183).
    let salvo = if launch.expenditure.launched && ammunition.is_swarm() {
        let target_number = aim.subtotal().unwrap_or_default();
        Some(super::TargetSalvo::Swarm(super::swarm::resolve(
            &mut candidate,
            super::swarm::SwarmRequest {
                shooter,
                target,
                weapon: (&launch.expenditure).into(),
                rules: rules.shot,
                first_hit: resolved.hit,
                first_roll: launch.roll,
                first_glancing: resolved.glancing,
                target_number,
                base_target_number: super::swarm::base_target_number(&aim, target_number),
                character,
                experience,
                ams_engaged,
            },
        )?))
    } else if missile_destroyed || !resolved.hit || narc.is_some() || heat_transfer.is_some() {
        None
    } else if candidate.btech.vehicles().contains_key(&target) {
        Some(super::target_salvo::resolve_vehicle_target_in_candidate(
            &mut candidate,
            shooter,
            target,
            VehicleSalvoRequest {
                weapon,
                ammunition,
                fire_mode: launch.expenditure.fire_mode,
                gatling_damage: launch.expenditure.gatling_damage,
                distance: aim.distance,
                glancing: resolved.glancing,
                guidance_blocked: source_field.blocks_outgoing_guidance()
                    || target_field.blocks_incoming_guidance(),
                angel_blocked,
                ams_engaged,
                indirect: indirect_fire,
                apollo,
                array,
            },
            rules.shot.vehicle_impact,
            super::vehicle_salvo::SalvoContext {
                incoming: None,
                attacker: Some(shooter),
                experience,
                aimed,
            },
        )?)
    } else {
        Some(TargetSalvo::Mech(super::salvo::resolve_salvo_in_candidate(
            &mut candidate,
            shooter,
            target,
            &launch.expenditure,
            super::salvo::ShotDamage {
                aimed,
                incoming: None,
                rules: FallRules {
                    vehicle_impact: rules.shot.vehicle_impact,
                    extended_piloting: rules.shot.extended_piloting,
                    toughness: rules.shot.target_toughness,
                },
                glancing: resolved.glancing,
                character,
                ams_engaged,
                indirect: indirect_fire,
                apollo,
                array,
                submerged,
                experience,
            },
        )?))
    };
    if let Some(defense) = &mut ams {
        defense.shot_down = if missile_destroyed {
            1
        } else {
            salvo
                .as_ref()
                .and_then(TargetSalvo::missiles_intercepted)
                .unwrap_or(0)
        };
    }

    super::shot_counters::record(
        &mut candidate,
        shooter,
        launch.expenditure.launched,
        coordinate.is_some(),
        super::shot_counters::hit(launch.roll, aim.subtotal()),
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
    let VehicleLaunch {
        ammunition_warning,
        launch_notices,
        roll,
        loader_destroyed,
        jammed,
        feed_broken,
        feed_critical_roll,
        misload,
        hit,
        glancing,
        expenditure,
    } = launch;
    Ok(VehicleShotReport {
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
        feed_broken,
        feed_critical_roll,
        misload,
        glancing,
        expenditure,
        ammunition_warning,
        launch_notices,
        recoil: None,
        streak_confused: weapon.is_streak() && angel_blocked,
        ams,
        narc,
        heat_transfer,
        missed_terrain,
        missed_building,
        salvo,
        experience_messages,
    })
}

impl VehicleShotReport {
    /// Collect cockpit consequences in launch, defense, damage and final-outcome order.
    pub fn notices(&self) -> Vec<Notice> {
        self.notices_with_feedback(&mut Vec::new())
    }

    /// Retain private damage rolls alongside their ordered cockpit consequences.
    pub(crate) fn notices_with_feedback(
        &self,
        private: &mut Vec<super::PilotNotice>,
    ) -> Vec<Notice> {
        if !self.expenditure.launched {
            let mut notices = self.launch_notices.clone();
            notices.extend(super::launch_feedback::streak_failure(self.shooter));
            return notices;
        }
        let mut notices = self.launch_notices.clone();
        if let Some(text) = &self.ammunition_warning {
            notices.push(Notice {
                unit: self.shooter,
                text: text.clone(),
            });
        }
        if let Some(terrain) = &self.missed_terrain {
            notices.extend(terrain.notices.iter().cloned());
        }
        if let Some(stray) = &self.missed_building {
            notices.extend(stray.notices.iter().cloned());
            if let Some(collapse) = &stray.collapse {
                super::piloting::append_feedback(
                    private,
                    collapse.pilot_notices.iter().cloned(),
                    notices.len(),
                );
                notices.extend(collapse.notices.iter().cloned());
            }
        }
        notices.extend(super::fire_feedback::glancing_notices(
            self.target,
            self.expenditure.weapon,
            self.glancing,
        ));
        notices.extend(super::direct_effects::thermal_notices(
            self.shooter,
            self.target,
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
                        .and_then(TargetSalvo::missiles_before_defense),
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
