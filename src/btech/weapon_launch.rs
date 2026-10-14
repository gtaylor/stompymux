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
    pub fall: FallRules,
    pub character_shooter: bool,
}

/// The committed candidate's launch, expenditure and immediate loader consequences. A feed
/// failure never cancels the shot: ammunition and heat are spent and the attack resolves as
/// rolled before the weapon is disabled (Total Warfare p. 114).
pub(super) struct WeaponLaunch {
    /// Cocoon opening precedes the shot's target consequences.
    pub launch_notices: Vec<super::Notice>,
    pub roll: u8,
    pub launched: bool,
    /// The feed jammed after the shot; `feed_broken` says whether the crew can clear it.
    pub jammed: bool,
    /// The jam is permanent for the rest of the game.
    pub feed_broken: bool,
    /// The mount's slots were destroyed: rapid-fire rounds exploded in the barrel or a caseless
    /// breakdown's critical roll wrecked every slot.
    pub loader_destroyed: bool,
    /// A caseless breakdown's Determining Critical Hits roll against the autocannon.
    pub feed_critical_roll: Option<u8>,
    /// Internal damage from rapid-fire rounds exploding in the barrel.
    pub misload: Option<TacticalImpact>,
    pub expenditure: WeaponUse,
    pub hit: bool,
    pub glancing: bool,
    pub ammunition_warning: Option<String>,
}

/// Destroy the autocannon's uppermost intact slots for one Determining Critical Hits roll
/// (Tactical Operations: AUE p. 165). Returns the number of slots destroyed.
pub(super) fn wreck_feed_slots<L: Copy + Ord>(
    criticals: &[L],
    lost: &mut std::collections::BTreeSet<L>,
    count: u8,
) -> u8 {
    let mut destroyed = 0;
    for location in criticals {
        if destroyed == count {
            break;
        }
        if lost.insert(*location) {
            destroyed += 1;
        }
    }
    destroyed
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
        roll,
        jammed,
        feed_broken,
        exploded,
        caseless_failure,
        launched,
        hit,
        glancing,
    } = super::launch_roll::roll_launch(
        super::launch_roll::LaunchRollRequest {
            weapon,
            ammunition: attacker.ammunition_mode(weapon_index)?,
            fire_mode,
            distance: request.distance,
            target_number,
            streak_confused,
        },
        &mut world.btech.constructed.get_mut(&shooter).unwrap().dice,
    )?;
    let launch_notices = super::orbital_drop_combat::open_for_fire(world, shooter)?;
    // The shot is fired before any feed failure takes effect (Total Warfare p. 114).
    let expenditure = super::readiness::use_weapon(
        world,
        shooter,
        pilot,
        weapon_index,
        launched,
        gatling_damage,
    )?;
    let unit = world.btech.constructed.get_mut(&shooter).unwrap();
    let mut feed_critical_roll = None;
    if caseless_failure {
        // One Determining Critical Hits roll, applied to the autocannon's own slots only.
        let critical = unit.dice.generic_roll();
        feed_critical_roll = Some(critical);
        wreck_feed_slots(
            &mount.criticals,
            &mut unit.lost_criticals,
            super::impact::critical_count(critical),
        );
    }
    if exploded {
        unit.lost_criticals.extend(mount.criticals.iter().copied());
    }
    let intact = unit.weapon_intact(weapon_index)?;
    if !intact {
        // A destroyed mount has nothing left to recycle.
        unit.weapon_recycle.remove(&weapon_index);
    }
    if jammed && intact {
        if feed_broken {
            unit.break_weapon_feed(weapon_index)?;
        } else {
            unit.jam_weapon(weapon_index)?;
        }
    }
    let misload = if exploded {
        let resolve = if character_shooter {
            super::impact::resolve_character_misload_in_candidate
        } else {
            super::impact::resolve_misload_in_candidate
        };
        Some(resolve(
            world,
            shooter,
            mount.criticals[0].section,
            u16::from(weapon.profile().damage),
            request.fall,
        )?)
    } else {
        None
    };
    let ammunition_warning = if launched {
        super::combat_warnings::ammunition_message(&attacker, &expenditure)
    } else {
        None
    };
    Ok(WeaponLaunch {
        launch_notices,
        roll,
        launched,
        jammed: jammed && intact,
        feed_broken: feed_broken && intact,
        loader_destroyed: !intact,
        feed_critical_roll,
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
    weapon: Weapon,
    fall_rules: FallRules,
    character_shooter: bool,
) -> Result<Option<RecoilReport>> {
    let shooter_state = &world.btech.constructed_units()[&shooter];
    if weapon != Weapon::HeavyGaussRifle
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
    Ok(Some(RecoilReport {
        pilot,
        experience_messages,
        check,
        fall,
    }))
}

impl RecoilReport {
    /// Share recoil warning, private roll and fall ordering across unit and coordinate shots.
    pub(super) fn append_feedback(
        &self,
        shooter: ObjectId,
        notices: &mut Vec<super::Notice>,
        private: &mut Vec<super::PilotNotice>,
    ) {
        notices.push(super::Notice {
            unit: shooter,
            text:
                "You realize that moving while firing this weapon may not be a good idea after all."
                    .into(),
        });
        super::piloting::capture_feedback(shooter, self.pilot, &self.check, notices, private);
        if let Some(fall) = &self.fall {
            notices.push(super::Notice {
                unit: shooter,
                text: "The weapon's recoil knocks you to the ground!".into(),
            });
            fall.append_notices(shooter, notices, private);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::wreck_feed_slots;
    use std::collections::BTreeSet;

    /// A caseless breakdown destroys the autocannon's uppermost intact slots, skipping slots
    /// already lost and never more than the critical check allows.
    #[test]
    fn caseless_breakdown_wrecks_uppermost_intact_slots() {
        let criticals = [1u8, 2, 3, 4];
        let mut lost = BTreeSet::from([2u8]);
        assert_eq!(wreck_feed_slots(&criticals, &mut lost, 0), 0);
        assert_eq!(lost, BTreeSet::from([2]));
        assert_eq!(wreck_feed_slots(&criticals, &mut lost, 2), 2);
        assert_eq!(lost, BTreeSet::from([1, 2, 3]));
        assert_eq!(wreck_feed_slots(&criticals, &mut lost, 3), 1);
        assert_eq!(lost, BTreeSet::from([1, 2, 3, 4]));
        assert_eq!(wreck_feed_slots(&criticals, &mut lost, 1), 0);
    }
}
