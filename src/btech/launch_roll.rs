//! Shared launch outcomes for every unit class; adapters own inventory and anatomy-specific damage.
use super::{
    BattleAmmunitionMode, BattleBeaconLaunch, BattleDice, BattleFireMode, BattleGlancingMode,
    BattleWeapon,
};
use anyhow::{Result, ensure};

/// Current weapon and admitted target facts after gatling preparation and ammunition fallback.
pub(super) struct LaunchRollRequest {
    pub damage: super::BattleWeaponDamageEffects,
    pub weapon: BattleWeapon,
    pub ammunition: BattleAmmunitionMode,
    pub fire_mode: BattleFireMode,
    pub distance: f64,
    pub target_number: Option<i32>,
    pub streak_confused: bool,
    pub glancing: BattleGlancingMode,
}

/// Unit-independent decisions consumed by both launch adapters.
pub(super) struct LaunchRoll {
    pub critical_explosion: bool,
    pub critical_jam: bool,
    pub roll: u8,
    pub propellant_roll: Option<u8>,
    pub loader_destroyed: bool,
    pub jammed: bool,
    pub misload_required: bool,
    pub launched: bool,
    pub hit: bool,
    pub glancing: bool,
}

/// Draw attack and propellant dice, then decide loader failure, Streak locking and glancing once.
pub(super) fn roll_launch(request: LaunchRollRequest, dice: &mut BattleDice) -> Result<LaunchRoll> {
    let LaunchRollRequest {
        damage,
        weapon,
        ammunition,
        fire_mode,
        distance,
        target_number,
        streak_confused,
        glancing,
    } = request;
    ensure!(
        distance.is_finite() && distance >= 0.0,
        "Invalid weapon range"
    );
    ensure!(
        target_number != Some(i32::MIN) || glancing != BattleGlancingMode::BelowTarget,
        "Invalid glancing target number"
    );
    let roll = attack_roll(weapon, distance, dice);
    let propellant_roll =
        (ammunition == BattleAmmunitionMode::Caseless && roll <= 3).then(|| dice.generic_roll());
    let (mut loader_destroyed, mut jammed) = propellant_roll.map_or_else(
        || {
            (
                fire_mode.is_double_shot() && roll == 2,
                fire_mode.jams_on(roll),
            )
        },
        |propellant| (propellant > 7, propellant <= 7),
    );
    let critical_explosion = !loader_destroyed && !jammed && damage.explodes(roll);
    let critical_jam = !loader_destroyed && !jammed && !critical_explosion && damage.jams(roll);
    loader_destroyed |= critical_explosion;
    jammed |= critical_jam;
    let launched = !loader_destroyed
        && !jammed
        && (!weapon.is_streak()
            || streak_confused
            || target_number.is_some_and(|number| i32::from(roll) >= number));
    let pod = weapon.beacon_kind(ammunition).is_some();
    let threshold = target_number.map(|number| {
        if pod {
            number
        } else {
            glancing.threshold(number)
        }
    });
    Ok(LaunchRoll {
        critical_explosion,
        critical_jam,
        roll,
        propellant_roll,
        loader_destroyed,
        jammed,
        launched,
        misload_required: critical_explosion
            || (loader_destroyed
                && (fire_mode == BattleFireMode::Rapid || propellant_roll.is_some())),
        hit: launched && threshold.is_some_and(|number| i32::from(roll) >= number),
        glancing: launched
            && !pod
            && (!weapon.is_streak() || streak_confused)
            && glancing != BattleGlancingMode::Disabled
            && threshold == Some(i32::from(roll)),
    })
}

/// Dead-fire missiles and extended LRMs below minimum range use the lowest two of three dice.
pub(super) fn attack_roll(weapon: BattleWeapon, distance: f64, dice: &mut BattleDice) -> u8 {
    if weapon.is_dead_fire()
        || (matches!(
            weapon,
            BattleWeapon::Elrm5
                | BattleWeapon::Elrm10
                | BattleWeapon::Elrm15
                | BattleWeapon::Elrm20
        ) && distance < f64::from(weapon.profile().minimum_range))
    {
        let first = dice.d6();
        let second = dice.d6();
        let third = dice.d6();
        return first + second + third - first.max(second).max(third);
    }
    dice.generic_roll()
}

#[cfg(test)]
mod tests {
    use super::*;
    use BattleWeapon as W;

    /// Generic attack checks count once; direct three-die attacks do not, and caseless failure adds one check.
    #[test]
    fn launch_roll_accounting_distinguishes_direct_dice_and_generic_checks() {
        let seed = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 2)
            .unwrap();
        for (weapon, ammunition, distance, count) in [
            (
                BattleWeapon::MediumLaser,
                BattleAmmunitionMode::Normal,
                1.0,
                1,
            ),
            (BattleWeapon::LrDfm5, BattleAmmunitionMode::Normal, 1.0, 0),
            (BattleWeapon::Elrm5, BattleAmmunitionMode::Normal, 1.0, 0),
            (BattleWeapon::Elrm5, BattleAmmunitionMode::Normal, 10.0, 1),
            (BattleWeapon::Ac10, BattleAmmunitionMode::Caseless, 1.0, 2),
        ] {
            let mut dice = BattleDice::seeded([seed; 32]);
            let outcome = roll_launch(
                LaunchRollRequest {
                    damage: Default::default(),
                    weapon,
                    ammunition,
                    fire_mode: BattleFireMode::Normal,
                    distance,
                    target_number: Some(2),
                    streak_confused: false,
                    glancing: BattleGlancingMode::Disabled,
                },
                &mut dice,
            )
            .unwrap();
            assert_eq!(dice.generic_roll_statistics().total(), count, "{weapon:?}");
            if count != 0 {
                assert!(dice.generic_roll_statistics().counts()[usize::from(outcome.roll - 2)] > 0);
            }
            assert_eq!(
                outcome.propellant_roll.is_some(),
                ammunition == BattleAmmunitionMode::Caseless
            );
        }
    }

    /// Enhanced failures reuse the attack roll and occur only after ordinary loader failures.
    #[test]
    fn enhanced_failures_share_attack_dice_and_precedence() {
        let seed = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 2)
            .unwrap();
        for (damage, explosion, jam) in [
            (
                super::super::BattleWeaponDamageEffects::default(),
                false,
                false,
            ),
            (
                super::super::BattleWeaponDamageEffects {
                    explosion: 1,
                    ..Default::default()
                },
                true,
                false,
            ),
            (
                super::super::BattleWeaponDamageEffects {
                    jam: 1,
                    ..Default::default()
                },
                false,
                true,
            ),
            (
                super::super::BattleWeaponDamageEffects {
                    explosion: 1,
                    jam: 1,
                    ..Default::default()
                },
                true,
                false,
            ),
        ] {
            let mut dice = BattleDice::seeded([seed; 32]);
            let mut expected = dice.clone();
            assert_eq!(expected.two_d6(), 2);
            let result = roll_launch(
                LaunchRollRequest {
                    damage,
                    weapon: BattleWeapon::Ac10,
                    ammunition: BattleAmmunitionMode::Normal,
                    fire_mode: BattleFireMode::Normal,
                    distance: 1.0,
                    target_number: Some(2),
                    streak_confused: false,
                    glancing: BattleGlancingMode::Disabled,
                },
                &mut dice,
            )
            .unwrap();
            assert_eq!(result.critical_explosion, explosion);
            assert_eq!(result.critical_jam, jam);
            assert_eq!(result.misload_required, explosion);
            assert_eq!(result.launched, !explosion && !jam);
            assert_eq!(dice, expected);
        }
    }

    /// Dead-fire attack dice never revert to ordinary 2d6 at longer ranges.
    #[test]
    fn dead_fire_attack_dice_at_all_ranges() {
        for weapon in [
            W::LrDfm5,
            W::LrDfm10,
            W::LrDfm15,
            W::LrDfm20,
            W::SrDfm2,
            W::SrDfm4,
            W::SrDfm6,
        ] {
            for seed in 0..=255 {
                for distance in [0.0, 4.0, 6.0, 12.0, 18.0, 24.001] {
                    let mut expected = BattleDice::seeded([seed; 32]);
                    let mut values = [expected.d6(), expected.d6(), expected.d6()];
                    values.sort_unstable();
                    let mut actual = BattleDice::seeded([seed; 32]);
                    assert_eq!(
                        attack_roll(weapon, distance, &mut actual),
                        values[0] + values[1]
                    );
                    assert_eq!(actual, expected);
                }
            }
        }
    }

    /// Raw distance controls the extra attack die independently of aim-bracket rounding.
    #[test]
    fn elrm_minimum_range_attack_dice_and_replay() {
        for weapon in [W::Elrm5, W::Elrm10, W::Elrm15, W::Elrm20] {
            for seed in 0..=255 {
                for distance in [0.0, 9.999, 10.0, 10.001, 36.0] {
                    let mut expected = BattleDice::seeded([seed; 32]);
                    let count = if distance < 10.0 { 3 } else { 2 };
                    let mut values: Vec<_> = (0..count).map(|_| expected.d6()).collect();
                    values.sort_unstable();
                    let mut actual = BattleDice::seeded([seed; 32]);
                    assert_eq!(
                        attack_roll(weapon, distance, &mut actual),
                        values[0] + values[1]
                    );
                    assert_eq!(actual, expected);
                    let mut restored: BattleDice =
                        serde_json::from_value(serde_json::to_value(&actual).unwrap()).unwrap();
                    assert_eq!(restored.two_d6(), actual.two_d6());
                }
            }
        }
        let mut ordinary = BattleDice::seeded([1; 32]);
        let mut expected = ordinary.clone();
        assert_eq!(attack_roll(W::Lrm20, 0.0, &mut ordinary), expected.two_d6());
        assert_eq!(ordinary, expected);
    }
}
