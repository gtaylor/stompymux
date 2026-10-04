//! Atomic direct tactical shots, glancing hits and immediate damage consequences.
use super::{
    BattleAimModifiers, BattleAimRules, BattleBeaconLaunch, BattleHitRules, BattleWeaponUse,
};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// How the configured glancing rule treats the boundary of a successful shot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleGlancingMode {
    Disabled,
    /// Meeting the ordinary target number produces reduced damage.
    AtTarget,
    /// One below the ordinary target number glances; meeting it hits normally.
    BelowTarget,
}

impl BattleGlancingMode {
    /// The game treats every nonzero setting except two as the ordinary glancing rule.
    pub fn from_setting(setting: i64) -> Self {
        match setting {
            0 => Self::Disabled,
            2 => Self::BelowTarget,
            _ => Self::AtTarget,
        }
    }

    /// Effective minimum roll without changing the displayed aim calculation.
    pub(super) fn threshold(self, target: i32) -> i32 {
        target - i32::from(self == Self::BelowTarget)
    }
}

/// Supported conventional shot rules, supplied explicitly by the enclosing game action.
#[derive(Debug, Clone, Copy)]
pub struct BattleShotRules {
    /// Apply configured energy damage changes at actual attack range.
    pub range_damage: bool,
    /// Shared load configuration used by pre-impact experience calculations.
    pub tsm_tow_bonus: bool,
    pub stacking: super::BattleStackingRules,
    pub stagger: super::BattleStaggerMode,
    pub glancing: BattleGlancingMode,
    pub aim: BattleAimRules,
    pub hit: BattleHitRules,
    /// Vehicle target hit-table and critical policy, independent of the shooter anatomy.
    pub vehicle_impact: super::BattleVehicleImpactRules,
    pub hit_arc_mode: i64,
    pub extended_gunnery: bool,
    pub extended_piloting: bool,
    pub target_toughness: bool,
}

/// A moving Heavy Gauss shooter's control check and any resulting fall.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleRecoilReport {
    /// Pilot captured before recoil consequences can change crew assignment.
    pub pilot: Option<ObjectId>,
    pub experience_messages: Vec<super::BattleChannelMessage>,
    pub check: super::BattlePilotingCheck,
    pub fall: Option<super::BattleFallReport>,
}

/// One resolved shot; ammunition, heat, dice and damage are already in the candidate world.
/// Falls are applied; publishing the report notices remains caller-owned.
///
/// Both chassis report the same fields. `U` is the shooter's weapon expenditure and `M`
/// the damage a misload did to it; [`BattleShotReport`] and
/// [`super::BattleVehicleShotReport`] name the report for each shooter.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[must_use = "Apply pending salvo effects and publish notices before committing the enclosing action"]
pub struct ShotReport<U, M> {
    /// Accepted observer and artillery skill award diagnostics, including missed shots.
    pub experience_messages: Vec<super::BattleChannelMessage>,
    pub shooter: ObjectId,
    pub target: ObjectId,
    /// Coordinate-directed fire, including observer-directed shots; the occupant remains the damage target.
    pub coordinate: Option<super::HexCoordinate>,
    pub weapon_index: usize,
    pub aim: BattleAimModifiers,
    /// None represents a physically out-of-range attempt; ordinary weapons still spend a salvo.
    pub target_number: Option<i32>,
    /// Attack result, including dead-fire and close-range extended-LRM dice rules.
    pub roll: u8,
    /// False for a failed Streak lock or loader failure; loader failures consume only shooter dice.
    pub launched: bool,
    /// Launch classification; a missile near miss can still be rejected by target resolution.
    pub hit: bool,
    /// Recoverable ammunition feed failure; no expenditure or recycle.
    pub jammed: bool,
    /// Permanent mount loss from loader failure or propellant ignition.
    pub loader_destroyed: bool,
    /// Second shooter roll on a caseless feed failure; eight or more ignites the propellant.
    pub propellant_roll: Option<u8>,
    /// Internal damage and immediate consequences from misload or propellant ignition.
    pub misload: Option<M>,
    /// A successful boundary hit with reduced damage or missile count.
    pub glancing: bool,
    pub expenditure: U,
    /// Optional pre-expenditure ammunition threshold warning.
    pub ammunition_warning: Option<String>,
    /// Cocoon opening feedback from the shared launch stage.
    pub launch_notices: Vec<super::BattleNotice>,
    /// Present only on a hit; a miss consumes no target damage dice.
    pub salvo: Option<super::BattleTargetSalvo>,
    /// Heat transferred to the target in place of material damage.
    pub heat_transfer: u8,
    /// Woods feedback and terrain effects preceding an unchanged thermal transfer.
    pub thermal_woods: Option<super::BattleWoodsAbsorption>,
    /// Incidental terrain check from a launched non-missile miss.
    pub missed_terrain: Option<super::BattleWoodlandImpact>,
    /// Coolant reduction applied to stored heat, including temporary negative credit.
    pub cooling: Option<f64>,
    /// Present when Heavy Gauss firing required a moving-shooter control check.
    pub recoil: Option<BattleRecoilReport>,
    /// Defensive activation after the missile reaches its base target number.
    pub ams: Option<super::BattleAmsReport>,
    /// Angel interference disables Streak lock protection and guaranteed cluster hits.
    pub streak_confused: bool,
    /// Normal Narc beacon outcome; explosive rounds use ordinary damage groups.
    pub narc: Option<super::BattleNarcReport<super::BattleUnitSection>>,
}

/// A shot fired by a BattleMech.
pub type BattleShotReport = ShotReport<BattleWeaponUse, super::BattleTacticalImpact>;

impl BattleShotReport {
    /// Glancing feedback precedes damage consequences; a lethal salvo concludes both cockpits' feedback.
    pub fn notices(&self) -> Vec<super::BattleNotice> {
        self.notices_with_feedback(&mut Vec::new())
    }

    /// Retain private damage rolls alongside their ordered cockpit consequences.
    pub(crate) fn notices_with_feedback(
        &self,
        private: &mut Vec<super::BattlePilotNotice>,
    ) -> Vec<super::BattleNotice> {
        if let Some(misload) = &self.misload {
            super::piloting::append_feedback(private, misload.pilot_notices.clone(), 0);
            return misload.notices.clone();
        }
        if self.jammed || self.loader_destroyed {
            return Vec::new();
        }
        if !self.launched {
            let mut notices = self.launch_notices.clone();
            notices.extend(super::launch_feedback::streak_failure(self.shooter));
            return notices;
        }
        let mut notices = self.launch_notices.clone();
        if let Some(text) = &self.ammunition_warning {
            notices.push(super::BattleNotice {
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
                        .and_then(super::BattleTargetSalvo::missiles_before_defense),
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
    rules: BattleShotRules,
) -> Result<BattleShotReport> {
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
    rules: BattleShotRules,
) -> Result<BattleShotReport> {
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
    /// Damage or coolant recipient already selected by the host.
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
    rules: BattleShotRules,
    xp: &crate::config::XpConfig,
) -> Result<BattleShotReport> {
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
    rules: BattleShotRules,
    effects: ShotEffects<'_>,
) -> Result<BattleShotReport> {
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
    let indirect =
        super::spotter::indirect_target_for_source(world, operator.source, weapon_index)?;

    let target = indirect.map_or(target, |link| link.target);
    let coolant = selected_weapon == super::BattleWeapon::CoolantGun;
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
    if attacker.ammunition_mode(weapon_index)?.munition() == super::BattleAmmunitionMode::Stinger {
        ensure!(coordinate.is_none(), "Stinger missiles cannot shoot hexes!");
        ensure!(
            super::stinger::target_airborne(world, target),
            "Stinger missiles can only engage airborne targets!"
        );
    }
    let vehicle_target = world.btech.vehicles().contains_key(&target);
    let self_cooling = coolant && shooter == target;
    ensure!(
        shooter != target || self_cooling,
        "A unit cannot fire on itself"
    );
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
    super::torpedo::check_target(world, selected_weapon, target)?;
    if indirect.is_some() {
        super::spotter::check_indirect_water(world, shooter, target)?;
    }
    let attacker = &world.btech.constructed_units()[&shooter];
    super::fire_target::check_target_safety_for_source(
        world,
        operator.source,
        target,
        selected_weapon,
    )?;
    let loadout = attacker.loadout()?;
    ensure!(
        attacker
            .weapon_readiness_with_loadout(&loadout, weapon_index)?
            .ready,
        "Weapon is not ready"
    );
    let mount = &loadout.weapons[weapon_index];
    let position = attacker.position().unwrap();
    let tile =
        world.btech.maps()[&position.map].base_hex(i64::from(position.x), i64::from(position.y))?;
    // Torpedo launchers are built to fire from a submerged leg.
    ensure!(
        mount.weapon.is_torpedo()
            || !(tile.immerses(attacker.elevation_level(tile))
                && attacker.chassis().is_leg(mount.criticals[0].section)),
        "Submerged weapon firing requires underwater combat rules"
    );
    ensure!(
        self_cooling
            || indirect.is_some()
            || rules.aim.override_weapon_arcs
            || super::weapon_bears_on(world, shooter, target, weapon_index)?,
        "Target is outside weapon arc"
    );
    let mut aim_dice = attacker.dice.clone();
    let prepared = super::gatling::prepare(world, shooter, weapon_index, &mut aim_dice)?;
    let gunnery = super::unit_gunnery_target(world, shooter, weapon_index, rules.extended_gunnery)?;
    let mut aim = super::aim::aim_modifiers_for_source(
        world,
        operator.source,
        target,
        weapon_index,
        gunnery,
        rules.aim,
    )?;
    if !self_cooling {
        super::aim::ensure_perceived(&aim)?;
    }
    super::hit_direction::HitDirection::Direct {
        shooter,
        mode: rules.hit_arc_mode,
    }
    .current(world, target)?;
    drop(loadouts);
    drop(admission);
    let mut candidate = super::shot_transaction::ShotCandidate::new(world);
    let launch_measurement = super::autopilot::diagnostics::combat("launch_defenses");
    let experience_messages = if character && let Some(link) = indirect {
        super::spotter::award_indirect_experience(&mut candidate, shooter, link, &mut aim)?
    } else {
        Vec::new()
    };
    let target_number = aim.subtotal();
    candidate.btech.constructed.get_mut(&shooter).unwrap().dice = aim_dice;
    let weapon = mount.weapon;
    let streak_confused = weapon.is_streak()
        && (super::electronic_field(world, shooter)?.angel_disturbed
            || super::electronic_field(world, target)?.angel_protected);
    let launch_fall = super::BattleFallRules {
        vehicle_impact: rules.vehicle_impact,
        stacking: rules.stacking,
        stagger: rules.stagger,
        hit: rules.hit,
        extended_piloting: rules.extended_piloting,
        toughness: shooter_toughness,
    };
    let super::weapon_launch::WeaponLaunch {
        roll,
        launched,
        jammed,
        loader_destroyed,
        propellant_roll,
        misload,
        expenditure,
        hit,
        glancing,
        ammunition_warning,
        launch_notices,
    } = super::weapon_launch::resolve_prepared_launch(
        &mut candidate,
        super::weapon_launch::WeaponLaunchRequest {
            shooter,
            pilot,
            weapon_index,
            distance: aim.distance,
            target_number,
            streak_confused,
            glancing: rules.glancing,
            fall: launch_fall,
            character_shooter,
        },
        Some(prepared),
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
        rules.glancing,
        streak_confused,
    );
    let mut ams = if resolved.hit
        && weapon.profile().missiles > 0
        && !expenditure.ammunition_mode.bypasses_ams()
    {
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
            ammunition: expenditure.ammunition_mode,
            fire_mode: expenditure.fire_mode,
            hit: resolved.hit,
            launched,
            intercepted: ams.as_ref().is_some_and(|ams| ams.shot_down > 0),
            vehicle_impact: rules.vehicle_impact,
            hit_rules: rules.hit,
            hit_arc_mode: rules.hit_arc_mode,
            woods_damage: rules.aim.woods_damage,
            range_damage: rules.range_damage,
            damage_penalty: expenditure.damage_penalty,
            distance: aim.distance,
            gatling_damage: expenditure.gatling_damage,
            coordinate,
        },
    )?;
    let experience = effects
        .xp
        .map(|config| super::gunnery_experience::GunneryAwardContext {
            config,
            request: super::BattleGunneryAwardRequest {
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
    let salvo = if resolved.hit && launched && expenditure.ammunition_mode.is_swarm() {
        Some(super::BattleTargetSalvo::Swarm(super::swarm::resolve(
            &mut candidate,
            super::swarm::SwarmRequest {
                shooter,
                target,
                weapon: (&expenditure).into(),
                rules,
                first_hit: resolved.hit,
                first_roll: roll,
                target_number: target_number.unwrap_or_default(),
                glancing: resolved.glancing,
                character,
                experience,
            },
        )?))
    } else if resolved.hit && !pod && heat_transfer == 0 && cooling.is_none() && vehicle_target {
        Some(super::target_salvo::resolve_vehicle_target_in_candidate(
            &mut candidate,
            shooter,
            target,
            super::BattleVehicleSalvoRequest {
                range_damage: rules.range_damage,
                damage_penalty: expenditure.damage_penalty,
                weapon,
                ammunition: expenditure.ammunition_mode,
                fire_mode: expenditure.fire_mode,
                gatling_damage: expenditure.gatling_damage,
                distance: aim.distance,
                glancing: resolved.glancing,
                guidance_blocked: false,
                angel_blocked: false,
                intercepted: ams.as_ref().map_or(0, |report| report.shot_down),
            },
            rules.hit_arc_mode,
            rules.vehicle_impact,
            super::vehicle_salvo::SalvoContext {
                submerged,
                woods_damage: rules.aim.woods_damage,
                incoming: None,
                attacker: Some(shooter),
                experience,
                aimed,
            },
        )?)
    } else if resolved.hit && !pod && heat_transfer == 0 && cooling.is_none() {
        Some(super::BattleTargetSalvo::Mech(
            super::salvo::resolve_salvo_in_candidate(
                &mut candidate,
                shooter,
                target,
                &expenditure,
                super::salvo::ShotDamage {
                    submerged,
                    woods_damage: rules.aim.woods_damage,
                    range_damage: rules.range_damage,
                    aimed,
                    incoming: None,
                    rules: super::BattleFallRules {
                        vehicle_impact: rules.vehicle_impact,
                        stacking: rules.stacking,
                        stagger: rules.stagger,
                        hit: rules.hit,
                        extended_piloting: rules.extended_piloting,
                        toughness: rules.target_toughness,
                    },
                    hit_arc_mode: rules.hit_arc_mode,
                    glancing: resolved.glancing,
                    character,
                    intercepted: ams.as_ref().map_or(0, |report| report.shot_down),
                    experience,
                },
            )?,
        ))
    } else {
        None
    };
    if let Some(defense) = &mut ams {
        defense.shot_down = defense.shot_down.min(if let Some(pod) = &narc {
            u8::from(pod.hit)
        } else {
            salvo
                .as_ref()
                .and_then(super::BattleTargetSalvo::missiles_before_defense)
                .unwrap_or(0)
        });
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
        super::shot_counters::hit(roll, target_number, rules.glancing),
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
    Ok(BattleShotReport {
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
        loader_destroyed,
        propellant_roll,
        misload,
        glancing: resolved.glancing,
        expenditure,
        ammunition_warning,
        launch_notices,
        salvo,
        heat_transfer,
        thermal_woods,
        missed_terrain,
        cooling,
        recoil,
        ams,
        streak_confused,
        narc,
    })
}

impl BattleShotRules {
    /// Translate server configuration once for unit and coordinate firing adapters.
    pub(super) fn configured(config: &crate::config::BattleTechConfig, toughness: bool) -> Self {
        Self {
            range_damage: config.moddamagewithrange != 0,
            tsm_tow_bonus: config.tsm_tow_bonus != 0,
            stacking: crate::BattleStackingRules {
                mode: config.stacking,
                damage_percent: config.stackdamage,
                hit_arcs: config.hit_arcs,
            },
            stagger: super::BattleStaggerMode::from_setting(config.newstagger),
            glancing: super::BattleGlancingMode::from_setting(config.glancing_blows),
            aim: super::BattleAimRules::configured(config),
            hit: super::BattleHitRules {
                inferno_penalty: config.inferno_penalty != 0,
                exile_stun_mode: config.exile_stun_code.clamp(0, 2) as u8,
            },
            vehicle_impact: super::BattleVehicleImpactRules::configured(config, toughness),
            hit_arc_mode: config.hit_arcs,
            extended_gunnery: config.extended_gunnery != 0,
            extended_piloting: config.extended_piloting != 0,
            target_toughness: toughness,
        }
    }
}
