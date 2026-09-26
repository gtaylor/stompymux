//! Shared conventional weapon launch and recoil, independent of the eventual damage recipient.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Target-independent inputs supplied after the enclosing attack has checked geometry and authority.
pub(super) struct WeaponLaunchRequest {
    pub shooter: ObjectId,
    pub pilot: ObjectId,
    pub weapon_index: usize,
    pub distance: f64,
    pub target_number: Option<i32>,
    pub streak_confused: bool,
    pub glancing: BattleGlancingMode,
    pub fall: BattleFallRules,
    pub character_shooter: bool,
}

/// The committed candidate's launch, expenditure and immediate loader consequences.
pub(super) struct WeaponLaunch {
    /// Cocoon opening precedes the shot's target consequences.
    pub launch_notices: Vec<super::BattleNotice>,
    pub roll: u8,
    pub launched: bool,
    pub jammed: bool,
    pub loader_destroyed: bool,
    pub propellant_roll: Option<u8>,
    pub misload: Option<BattleTacticalImpact>,
    pub expenditure: BattleWeaponUse,
    pub hit: bool,
    pub glancing: bool,
    pub ammunition_warning: Option<String>,
}

/// Resolve on an unpublished world candidate; later attack failures must discard that candidate.
pub(super) fn resolve_launch(
    world: &mut World,
    request: WeaponLaunchRequest,
) -> Result<WeaponLaunch> {
    resolve_prepared_launch(world, request, None)
}

/// Retain a pre-aim gatling roll when the direct-shot preparation already consumed it.
pub(super) fn resolve_prepared_launch(
    world: &mut World,
    request: WeaponLaunchRequest,
    prepared: Option<super::gatling::GatlingPreparation>,
) -> Result<WeaponLaunch> {
    let WeaponLaunchRequest {
        shooter,
        pilot,
        weapon_index,
        target_number,
        streak_confused,
        character_shooter,
        ..
    } = request;
    super::combat_operator::controlled_mech(world, shooter, pilot)?;
    let attacker = world.btech.constructed_units()[&shooter].clone();
    let loadout = attacker.loadout()?;
    let mount = loadout
        .weapons
        .get(weapon_index)
        .context("Weapon index out of bounds")?;
    let weapon = mount.weapon;
    attacker.check_spotter_fire(shooter, weapon_index)?;
    ensure!(
        attacker.weapon_readiness(weapon_index)?.ready,
        "Weapon is not ready"
    );
    ensure!(!weapon.is_ams(), "That weapon is defensive only!");
    let gatling_damage = if let Some(prepared) = prepared {
        prepared.damage()
    } else {
        let mut dice = attacker.dice.clone();
        let prepared = super::gatling::prepare(world, shooter, weapon_index, &mut dice)?;
        world.btech.constructed.get_mut(&shooter).unwrap().dice = dice;
        prepared.damage()
    };
    let fire_mode = attacker.effective_fire_mode(weapon_index)?;
    if fire_mode != attacker.fire_mode(weapon_index)? {
        super::weapon_controls::set_fire_mode(world, shooter, weapon_index, fire_mode);
    }
    let super::launch_roll::LaunchRoll {
        critical_explosion,
        critical_jam,
        roll,
        propellant_roll,
        loader_destroyed,
        jammed,
        misload_required,
        launched,
        hit,
        glancing,
    } = super::launch_roll::roll_launch(
        super::launch_roll::LaunchRollRequest {
            damage: attacker.weapon_damage_effects(weapon_index)?,
            weapon,
            ammunition: attacker.ammunition_mode(weapon_index)?,
            fire_mode,
            distance: request.distance,
            target_number,
            streak_confused,
            glancing: request.glancing,
        },
        &mut world.btech.constructed.get_mut(&shooter).unwrap().dice,
    )?;
    let launch_notices = if jammed || loader_destroyed {
        Vec::new()
    } else {
        super::orbital_drop_combat::open_for_fire(world, shooter)?
    };
    let mut misload = None;
    let mut expenditure = if jammed || loader_destroyed {
        if loader_destroyed {
            world
                .btech
                .constructed
                .get_mut(&shooter)
                .unwrap()
                .weapon_damage_jams
                .remove(&weapon_index);
            world
                .btech
                .constructed
                .get_mut(&shooter)
                .unwrap()
                .lost_criticals
                .extend(mount.criticals.iter().copied());
        } else if critical_jam {
            world
                .btech
                .constructed
                .get_mut(&shooter)
                .unwrap()
                .weapon_damage_jams
                .insert(weapon_index);
        } else {
            world
                .btech
                .constructed
                .get_mut(&shooter)
                .unwrap()
                .jam_weapon(weapon_index)?;
        }
        super::BattleWeaponUse {
            weapon,
            ammunition: Vec::new(),
            fire_mode,
            heat: 0,
            critical_failure: if critical_jam {
                Some(super::BattleWeaponDamageKind::Barrel)
            } else if critical_explosion {
                Some(if weapon.gunnery_skill(true) == "Gunnery-Laser" {
                    super::BattleWeaponDamageKind::Crystal
                } else {
                    super::BattleWeaponDamageKind::Feed
                })
            } else {
                None
            },
            damage_penalty: attacker.weapon_damage_effects(weapon_index)?.damage,
            gatling_damage: None,
            ammunition_mode: attacker.ammunition_mode(weapon_index)?,
        }
    } else {
        super::readiness::use_weapon(
            world,
            shooter,
            pilot,
            weapon_index,
            launched,
            gatling_damage,
        )?
    };
    if misload_required {
        if critical_explosion && mount.one_shot {
            world
                .btech
                .constructed
                .get_mut(&shooter)
                .unwrap()
                .spent_launchers
                .insert(weapon_index);
        }
        let draws =
            if critical_explosion && (mount.one_shot || weapon.profile().ammunition_per_ton == 0) {
                Vec::new()
            } else {
                attacker.ammunition_feed(weapon_index, fire_mode.rounds_per_cycle())?
            };
        let resolve = if character_shooter {
            super::impact::resolve_character_misload_in_candidate
        } else {
            super::impact::resolve_misload_in_candidate
        };
        misload = Some(resolve(
            world,
            shooter,
            mount.criticals[0].section,
            u16::from(weapon.profile().damage),
            request.fall,
        )?);
        // Damage may have destroyed a supply bin. Decrement only its surviving inventory.
        let unit = world.btech.constructed.get_mut(&shooter).unwrap();
        expenditure
            .ammunition
            .extend(super::ammunition_feed::spend_surviving_draws(
                &mut unit.ammunition,
                draws,
            ));
        if !expenditure.ammunition.is_empty() {
            unit.live_mass.invalidate();
        }
    }
    let ammunition_warning = if launched {
        super::combat_warnings::ammunition_message(&attacker, &expenditure)
    } else {
        None
    };
    Ok(WeaponLaunch {
        launch_notices,
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
    })
}

/// Apply post-impact Heavy Gauss recoil on the same candidate, preserving the current shooter condition.
pub(super) fn resolve_recoil(
    world: &mut World,
    shooter: ObjectId,
    weapon: BattleWeapon,
    fall_rules: BattleFallRules,
    character_shooter: bool,
) -> Result<Option<BattleRecoilReport>> {
    let shooter_state = &world.btech.constructed_units()[&shooter];
    if weapon != BattleWeapon::HeavyGaussRifle
        || !shooter_state
            .motion()
            .is_some_and(|motion| motion.speed.abs() > 0.0)
        || shooter_state.is_destroyed()
    {
        return Ok(None);
    }
    let pilot = shooter_state.pilot();
    let modifier = match shooter_state.definition().tons {
        ..=35 => 2,
        36..=55 => 1,
        56..=75 => 0,
        _ => -1,
    };
    let mut check = super::roll_piloting(world, shooter, modifier, fall_rules.extended_piloting)?;
    let mut experience_messages = Vec::new();
    if character_shooter {
        experience_messages.extend(super::piloting::award_control_check(
            world,
            shooter,
            &mut check,
            fall_rules.extended_piloting,
        )?);
    }
    let fall = if check.success {
        None
    } else {
        let resolve = if character_shooter {
            super::fall::resolve_character_fall
        } else {
            super::resolve_fall
        };
        Some(resolve(world, shooter, 1, fall_rules)?)
    };
    Ok(Some(BattleRecoilReport {
        pilot,
        experience_messages,
        check,
        fall,
    }))
}

impl BattleRecoilReport {
    /// Share recoil warning, private roll and fall ordering across unit and coordinate shots.
    pub(super) fn append_feedback(
        &self,
        shooter: ObjectId,
        notices: &mut Vec<super::BattleNotice>,
        private: &mut Vec<super::BattlePilotNotice>,
    ) {
        notices.push(super::BattleNotice {
            unit: shooter,
            text:
                "You realize that moving while firing this weapon may not be a good idea after all."
                    .into(),
        });
        super::piloting::capture_feedback(shooter, self.pilot, &self.check, notices, private);
        if let Some(fall) = &self.fall {
            notices.push(super::BattleNotice {
                unit: shooter,
                text: "The weapon's recoil knocks you to the ground!".into(),
            });
            fall.append_notices(shooter, notices, private);
        }
    }
}
