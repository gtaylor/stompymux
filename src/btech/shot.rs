//! Atomic direct tactical shots, glancing hits and immediate damage consequences.
use super::{AimModifiers, AimRules, BeaconLaunch, WeaponUse};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Supported conventional shot rules, supplied explicitly by the enclosing game action.
#[derive(Debug, Clone, Copy)]
pub struct ShotRules {
    /// Shared load configuration used by pre-impact experience calculations.
    pub tsm_tow_bonus: bool,
    pub aim: AimRules,
    /// Vehicle target hit-table and critical policy, independent of the shooter anatomy.
    pub vehicle_impact: super::VehicleImpactRules,
    pub extended_gunnery: bool,
    pub extended_piloting: bool,
    pub target_toughness: bool,
}

/// A moving Heavy Gauss shooter's control check and any resulting fall.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecoilReport {
    /// Pilot captured before recoil consequences can change crew assignment.
    pub pilot: Option<ObjectId>,
    pub experience_messages: Vec<super::DiagnosticMessage>,
    pub check: super::PilotingCheck,
    pub fall: Option<super::MechFallReport>,
}

/// One resolved shot; ammunition, heat, dice and damage are already in the candidate world.
/// Falls are applied; publishing the report notices remains caller-owned.
///
/// Both chassis report the same fields. `U` is the shooter's weapon expenditure and `M`
/// the damage a misload did to it; [`MechShotReport`] and
/// [`super::VehicleShotReport`] name the report for each shooter.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[must_use = "Apply pending salvo effects and publish notices before committing the enclosing action"]
pub struct ShotReport<U, M> {
    /// Accepted observer and artillery skill award diagnostics, including missed shots.
    pub experience_messages: Vec<super::DiagnosticMessage>,
    pub shooter: ObjectId,
    pub target: ObjectId,
    /// Coordinate-directed fire, including observer-directed shots; the occupant remains the damage target.
    pub coordinate: Option<super::HexCoordinate>,
    pub weapon_index: usize,
    pub aim: AimModifiers,
    /// None represents a physically out-of-range attempt; ordinary weapons still spend a salvo.
    pub target_number: Option<i32>,
    /// The 2d6 attack roll.
    pub roll: u8,
    /// False only for a failed Streak lock; a feed failure still fires the shot.
    pub launched: bool,
    /// Launch classification; a missile near miss can still be rejected by target resolution.
    pub hit: bool,
    /// The ammunition feed jammed after the shot; firing is blocked until it is cleared.
    pub jammed: bool,
    /// The jam is permanent for the rest of the game: `unjam` cannot clear it.
    pub feed_broken: bool,
    /// The mount's slots were destroyed after the shot.
    pub loader_destroyed: bool,
    /// A caseless feed breakdown's Determining Critical Hits roll against the autocannon.
    pub feed_critical_roll: Option<u8>,
    /// Internal damage and immediate consequences from rapid-fire rounds exploding in the barrel.
    pub misload: Option<M>,
    /// A successful boundary hit with reduced damage or missile count.
    pub glancing: bool,
    pub expenditure: U,
    /// Optional pre-expenditure ammunition threshold warning.
    pub ammunition_warning: Option<String>,
    /// Cocoon opening feedback from the shared launch stage.
    pub launch_notices: Vec<super::Notice>,
    /// Present only on a hit; a miss consumes no target damage dice.
    pub salvo: Option<super::TargetSalvo>,
    /// Heat transferred to a Mech target in place of material damage: zero when the turn's
    /// outside-heat cap was already full, `None` unless a heat-mode shot hit.
    pub heat_transfer: Option<u8>,
    /// Incidental terrain check from a launched non-missile miss.
    pub missed_terrain: Option<super::WoodlandImpact>,
    /// A miss from the next hex striking the building around a target inside one.
    pub missed_building: Option<super::StrayHit>,
    /// Present when Heavy Gauss firing required a moving-shooter control check.
    pub recoil: Option<RecoilReport>,
    /// Defensive activation after the missile reaches its base target number.
    pub ams: Option<super::AmsReport>,
    /// Angel interference disables Streak lock protection and guaranteed cluster hits.
    pub streak_confused: bool,
    /// Normal Narc beacon outcome; explosive rounds use ordinary damage groups.
    pub narc: Option<super::NarcReport<super::UnitSection>>,
}

/// A shot fired by a BattleMech.
pub type MechShotReport = ShotReport<WeaponUse, super::TacticalImpact>;

impl MechShotReport {
    /// Glancing feedback precedes damage consequences; a lethal salvo concludes both cockpits' feedback.
    pub fn notices(&self) -> Vec<super::Notice> {
        self.notices_with_feedback(&mut Vec::new())
    }

    /// Retain private damage rolls alongside their ordered cockpit consequences.
    pub(crate) fn notices_with_feedback(
        &self,
        private: &mut Vec<super::PilotNotice>,
    ) -> Vec<super::Notice> {
        if !self.launched {
            let mut notices = self.launch_notices.clone();
            notices.extend(super::launch_feedback::streak_failure(self.shooter));
            return notices;
        }
        let mut notices = self.launch_notices.clone();
        if let Some(text) = &self.ammunition_warning {
            notices.push(super::Notice {
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
                        .and_then(super::TargetSalvo::missiles_before_defense),
                ),
            );
        }
        if let Some(salvo) = &self.salvo {
            let mut nested = Vec::new();
            let damage = salvo.notices_with_feedback(self.shooter, self.target, &mut nested);
            super::piloting::append_feedback(private, nested, notices.len());
            notices.extend(damage);
        }
        if let Some(recoil) = &self.recoil {
            recoil.append_feedback(self.shooter, &mut notices, private);
        }
        notices
    }
}

/// Resolve a conventional direct shot with standing or supported prone posture in a private candidate.
/// Firing guards precede RNG/expenditure; later damage errors also discard all changes. No notifications are sent here.
pub fn resolve_shot(
    world: &mut World,
    shooter: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    weapon_index: usize,
    rules: ShotRules,
) -> Result<MechShotReport> {
    ensure!(
        !(pilot == shooter && world.btech.controllers().contains_key(&shooter)),
        "Autopilot actor token is internal"
    );
    let report = resolve_shot_inner(
        world,
        shooter,
        pilot,
        target,
        weapon_index,
        rules,
        ShotEffects::default(),
    )?;
    let _ = super::autopilot::manual_takeover(world, shooter);
    Ok(report)
}

/// Resolve a direct shot for an attached autopilot.  The unit remains the
/// actor token so all nested launch/readiness admission paths recognize the
/// request without inventing a player object.
pub(crate) fn resolve_shot_autopilot(
    world: &mut World,
    shooter: ObjectId,
    target: ObjectId,
    weapon_index: usize,
    rules: ShotRules,
) -> Result<MechShotReport> {
    resolve_shot_inner(
        world,
        shooter,
        shooter,
        target,
        weapon_index,
        rules,
        ShotEffects::default(),
    )
}

/// The damage recipient and optional coordinate-directed presentation of an admitted attack.
#[derive(Clone, Copy)]
pub(super) struct ShotTarget {
    /// Damage recipient already selected by the host.
    pub unit: ObjectId,
    pub coordinate: Option<super::HexCoordinate>,
}

/// Resolve a complete shot inside a host action that publishes character consequences.
pub(super) fn resolve_shot_in_action(
    world: &mut World,
    shooter: ObjectId,
    pilot: ObjectId,
    target: ShotTarget,
    weapon_index: usize,
    rules: ShotRules,
    xp: &crate::config::XpConfig,
) -> Result<MechShotReport> {
    ensure!(
        !(pilot == shooter && world.btech.controllers().contains_key(&shooter)),
        "Autopilot actor token is internal"
    );
    let report = resolve_shot_inner(
        world,
        shooter,
        pilot,
        target.unit,
        weapon_index,
        rules,
        ShotEffects {
            character: true,
            xp: Some(xp),
            coordinate: target.coordinate,
        },
    )?;
    // Autopilot actions use the shooter as their internal actor token.  A
    // successful player action carries a distinct cockpit occupant and pauses
    // the controller after the enclosing candidate has succeeded.
    if pilot != shooter {
        let _ = super::autopilot::manual_takeover(world, shooter);
    }
    Ok(report)
}

/// Publication capability and configured award policy for a complete shot.
#[derive(Default)]
struct ShotEffects<'a> {
    character: bool,
    coordinate: Option<super::HexCoordinate>,
    xp: Option<&'a crate::config::XpConfig>,
}

/// Shared shot admission, expenditure, damage and recoil for either publication mode.
fn resolve_shot_inner(
    world: &mut World,
    shooter: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    weapon_index: usize,
    rules: ShotRules,
    effects: ShotEffects<'_>,
) -> Result<MechShotReport> {
    let mut attempt = super::autopilot::diagnostics::Attempt::begin();
    let admission = super::autopilot::diagnostics::combat("admission_aim");
    let loadouts =
        super::loadout_context::LoadoutScope::participants(&world.btech, [shooter, target]);
    let character = effects.character;
    let operator = super::combat_operator::controlled_mech(world, shooter, pilot)?;
    let character_shooter = character && world.objects[&shooter].flags.contains(Flag::InCharacter);
    let shooter_toughness = if character_shooter {
        world.btech.constructed_units()[&shooter]
            .pilot()
            .and_then(|pilot| world.btech.character_values().get(&pilot))
            .is_some_and(|values| super::advantages::enabled(values, "Toughness"))
    } else {
        rules.target_toughness
    };
    let attacker = &world.btech.constructed_units()[&shooter];
    let selected_weapon = attacker
        .loadout()?
        .weapons
        .get(weapon_index)
        .context("Weapon index out of bounds")?
        .weapon;
    attacker.check_spotter_fire(shooter, weapon_index)?;
    let observed =
        super::spotter::observed_target_for_source(world, operator.source, weapon_index)?;
    let indirect =
        super::spotter::indirect_target_for_source(world, operator.source, weapon_index)?;

    let target = observed.map_or(target, |link| link.target);
    let coordinate = if indirect.is_some() {
        let position = super::scanner::scanner_unit(world, target)
            .and_then(|unit| unit.position)
            .context("Target is not placed")?;
        Some(super::HexCoordinate {
            x: i32::from(position.x),
            y: i32::from(position.y),
        })
    } else {
        effects.coordinate
    };
    let vehicle_target = world.btech.vehicles().contains_key(&target);
    ensure!(shooter != target, "A unit cannot fire on itself");
    for id in [shooter, target] {
        let object = world.objects.get(&id).context("Unit is unavailable")?;
        ensure!(!object.flags.contains(Flag::Going), "Unit is unavailable");
        ensure!(
            character || !object.flags.contains(Flag::InCharacter),
            "Direct tactical shots require non-character units"
        );
        let state = super::scanner::scanner_unit(world, id)
            .context("Unit construction state is unavailable")?;
        ensure!(!state.destroyed, "Unit is destroyed");
        super::unit_elevation(world, id)?.context("Unit is not on a battlefield")?;
        if let Some(unit) = world.btech.constructed_units().get(&id) {
            super::validation_context::unit(id, unit)?;
        }
    }
    let submerged = super::weapon_geometry::submerged(world, shooter, weapon_index)?;
    super::weapon_geometry::check_water(selected_weapon, submerged)?;
    super::planetary_conditions::check_wind(world, shooter, selected_weapon)?;
    super::torpedo::check_target(world, selected_weapon, target, submerged)?;
    if indirect.is_some() {
        super::spotter::check_indirect_water(world, shooter, target)?;
    }
    let attacker = &world.btech.constructed_units()[&shooter];
    super::fire_target::check_target_safety_for_source(world, operator.source, target)?;
    super::fire_target::check_unit_munition(world, shooter, weapon_index)?;
    let loadout = attacker.loadout()?;
    ensure!(
        attacker
            .weapon_readiness_with_loadout(&loadout, weapon_index)?
            .ready,
        "Weapon is not ready"
    );
    let mount = &loadout.weapons[weapon_index];
    ensure!(
        indirect.is_some()
            || rules.aim.override_weapon_arcs
            || super::weapon_bears_on(world, shooter, target, weapon_index)?,
        "Target is outside weapon arc"
    );
    let mut aim_dice = attacker.dice.clone();
    let prepared = super::gatling::prepare(world, shooter, weapon_index, &mut aim_dice)?;
    let gunnery = super::unit_gunnery_target(world, shooter, weapon_index, rules.extended_gunnery)?;
    let aim = super::aim::aim_modifiers_for_source(
        world,
        operator.source,
        target,
        weapon_index,
        gunnery,
        rules.aim,
    )?;
    super::aim::ensure_perceived(&aim)?;
    super::hit_direction::HitDirection::Direct { shooter }.current(world, target)?;
    drop(loadouts);
    drop(admission);
    let mut candidate = super::shot_transaction::ShotCandidate::new(world);
    let launch_measurement = super::autopilot::diagnostics::combat("launch_defenses");
    let experience_messages = if character && let Some(link) = indirect {
        super::spotter::award_indirect_experience(&mut candidate, shooter, link)?
    } else {
        Vec::new()
    };
    let target_number = aim.subtotal();
    candidate.btech.constructed.get_mut(&shooter).unwrap().dice = aim_dice;
    let weapon = mount.weapon;
    // A surviving Apollo system takes one from the launcher's Cluster Hits roll (Tactical
    // Operations: Advanced Units & Equipment p. 143).
    let apollo = weapon.is_mrm()
        && world.btech.constructed_units()[&shooter].apollo_operational(weapon_index)?;
    // Firing a gun its machine gun array links fires every ready linked gun as one attack
    // (Total Warfare p. 137).
    let volley = world.btech.constructed_units()[&shooter].array_volley(weapon_index)?;
    let streak_confused = weapon.is_streak()
        && (super::electronic_field(world, shooter)?.angel_disturbed
            || super::electronic_field(world, target)?.angel_protected);
    let launch_fall = super::FallRules {
        vehicle_impact: rules.vehicle_impact,
        extended_piloting: rules.extended_piloting,
        toughness: shooter_toughness,
    };
    let super::weapon_launch::WeaponLaunch {
        roll,
        launched,
        jammed,
        feed_broken,
        loader_destroyed,
        feed_critical_roll,
        misload,
        expenditure,
        hit,
        glancing,
        ammunition_warning,
        mut launch_notices,
    } = super::weapon_launch::resolve_prepared_launch(
        &mut candidate,
        super::weapon_launch::WeaponLaunchRequest {
            shooter,
            pilot,
            weapon_index,
            distance: aim.distance,
            target_number,
            streak_confused,
            fall: launch_fall,
            character_shooter,
        },
        Some(prepared),
    )?;
    // The array's other guns spend their ammunition and recycle with the one fired.
    let array = super::machine_gun_array::fire_linked_guns(
        &mut candidate,
        shooter,
        pilot,
        weapon_index,
        volley.as_deref(),
        launched,
        &mut launch_notices,
    )?;
    let beacon = weapon.beacon_kind(expenditure.ammunition_mode);
    let pod = beacon.is_some();
    let aimed = super::aimed_hit::prepare(
        &mut candidate,
        super::aimed_hit::AimedLaunch {
            shooter,
            target,
            index: weapon_index,
            weapon,
            fire_mode: expenditure.fire_mode,
            ammunition: expenditure.ammunition_mode,
            launched,
            hit,
            in_range: aim.range.is_some(),
        },
    )?;
    let resolved = super::target_hit::TargetHit { hit, glancing }.for_weapon(
        weapon,
        expenditure.ammunition_mode,
        roll,
        target_number,
        streak_confused,
    );
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
            ammunition: expenditure.ammunition_mode,
            fire_mode: expenditure.fire_mode,
            hit: resolved.hit,
            launched,
            intercepted: missile_destroyed,
            vehicle_impact: rules.vehicle_impact,
            distance: aim.distance,
            gatling_damage: expenditure.gatling_damage,
            coordinate,
        },
    )?;
    let experience = effects
        .xp
        .map(|config| super::gunnery_experience::GunneryAwardContext {
            config,
            request: super::GunneryAwardRequest {
                tsm_tow_bonus: rules.tsm_tow_bonus,
                attacker: shooter,
                pilot,
                target,
                weapon,
                damage: 0,
                base_to_hit: target_number.unwrap_or_default(),
                extended_gunnery: rules.extended_gunnery,
                extended_piloting: rules.extended_piloting,
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
    let salvo = if launched && expenditure.ammunition_mode.is_swarm() {
        let target_number = target_number.unwrap_or_default();
        Some(super::TargetSalvo::Swarm(super::swarm::resolve(
            &mut candidate,
            super::swarm::SwarmRequest {
                shooter,
                target,
                weapon: (&expenditure).into(),
                rules,
                first_hit: resolved.hit,
                first_roll: roll,
                first_glancing: resolved.glancing,
                target_number,
                base_target_number: super::swarm::base_target_number(&aim, target_number),
                character,
                experience,
                ams_engaged,
            },
        )?))
    } else if missile_destroyed || !resolved.hit || pod || heat_transfer.is_some() {
        None
    } else if vehicle_target {
        Some(super::target_salvo::resolve_vehicle_target_in_candidate(
            &mut candidate,
            shooter,
            target,
            super::VehicleSalvoRequest {
                weapon,
                ammunition: expenditure.ammunition_mode,
                fire_mode: expenditure.fire_mode,
                gatling_damage: expenditure.gatling_damage,
                distance: aim.distance,
                glancing: resolved.glancing,
                guidance_blocked: false,
                angel_blocked: false,
                ams_engaged,
                indirect: indirect_fire,
                apollo,
                array,
            },
            rules.vehicle_impact,
            super::vehicle_salvo::SalvoContext {
                incoming: None,
                attacker: Some(shooter),
                experience,
                aimed,
            },
        )?)
    } else {
        Some(super::TargetSalvo::Mech(
            super::salvo::resolve_salvo_in_candidate(
                &mut candidate,
                shooter,
                target,
                &expenditure,
                super::salvo::ShotDamage {
                    aimed,
                    incoming: None,
                    rules: super::FallRules {
                        vehicle_impact: rules.vehicle_impact,
                        extended_piloting: rules.extended_piloting,
                        toughness: rules.target_toughness,
                    },
                    glancing: resolved.glancing,
                    character,
                    ams_engaged,
                    indirect: indirect_fire,
                    apollo,
                    array,
                    experience,
                },
            )?,
        ))
    };
    if let Some(defense) = &mut ams {
        defense.shot_down = if missile_destroyed {
            1
        } else {
            salvo
                .as_ref()
                .and_then(super::TargetSalvo::missiles_intercepted)
                .unwrap_or(0)
        };
    }
    let recoil = super::weapon_launch::resolve_recoil(
        &mut candidate,
        shooter,
        weapon,
        launch_fall,
        character_shooter,
    )?;
    super::shot_counters::record(
        &mut candidate,
        shooter,
        launched,
        coordinate.is_some(),
        super::shot_counters::hit(roll, target_number),
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
    Ok(MechShotReport {
        experience_messages,
        shooter,
        target,
        coordinate,
        weapon_index,
        aim,
        target_number,
        roll,
        launched,
        hit,
        jammed,
        feed_broken,
        loader_destroyed,
        feed_critical_roll,
        misload,
        glancing: resolved.glancing,
        expenditure,
        ammunition_warning,
        launch_notices,
        salvo,
        heat_transfer,
        missed_terrain,
        missed_building,
        recoil,
        ams,
        streak_confused,
        narc,
    })
}

impl ShotRules {
    /// Translate server configuration once for unit and coordinate firing adapters.
    pub(super) fn configured(config: &crate::config::BattleTechConfig, toughness: bool) -> Self {
        Self {
            tsm_tow_bonus: config.tsm_tow_bonus != 0,
            aim: super::AimRules::configured(config),
            vehicle_impact: super::VehicleImpactRules::configured(config, toughness),
            extended_gunnery: config.extended_gunnery != 0,
            extended_piloting: config.extended_piloting != 0,
            target_toughness: toughness,
        }
    }
}
