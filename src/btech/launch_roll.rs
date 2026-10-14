//! Shared launch outcomes for every unit class; adapters own inventory and anatomy-specific damage.
use super::{AmmunitionMode, BeaconLaunch, Dice, FireMode, Weapon};
use anyhow::{Result, ensure};

/// Current weapon and admitted target facts after gatling preparation and ammunition fallback.
pub(super) struct LaunchRollRequest {
    pub weapon: Weapon,
    pub ammunition: AmmunitionMode,
    pub fire_mode: FireMode,
    pub distance: f64,
    pub target_number: Option<i32>,
    pub streak_confused: bool,
}

/// Unit-independent decisions consumed by both launch adapters. Every feed failure fires the
/// shot first (Total Warfare p. 114): the adapters spend ammunition and heat, resolve the attack
/// as rolled, and only then disable the weapon.
pub(super) struct LaunchRoll {
    pub roll: u8,
    /// The feed jammed; the weapon cannot fire again until it is cleared or repaired.
    pub jammed: bool,
    /// The jam cannot be cleared in the field: an Ultra autocannon's double shot on a 2 or a
    /// caseless feed breakdown leaves the weapon useless for the rest of the game.
    pub feed_broken: bool,
    /// Rapid-fire mode rounds exploded in the barrel (Tactical Operations p. 98): the mount's
    /// slots are destroyed and its location takes one shot's damage.
    pub exploded: bool,
    /// A caseless feed breakdown (Tactical Operations: AUE p. 165): the shot misses outright and
    /// the autocannon takes one Determining Critical Hits roll.
    pub caseless_failure: bool,
    pub launched: bool,
    pub hit: bool,
    pub glancing: bool,
}

/// Draw the attack dice, then decide feed failure, Streak locking and glancing once.
pub(super) fn roll_launch(request: LaunchRollRequest, dice: &mut Dice) -> Result<LaunchRoll> {
    let LaunchRollRequest {
        weapon,
        ammunition,
        fire_mode,
        distance,
        target_number,
        streak_confused,
    } = request;
    ensure!(
        distance.is_finite() && distance >= 0.0,
        "Invalid weapon range"
    );
    let roll = dice.generic_roll();
    let caseless_failure = ammunition == AmmunitionMode::Caseless && roll == 2;
    let exploded = fire_mode.explodes_on(roll);
    let feed_broken = caseless_failure || fire_mode.breaks_feed_on(roll);
    let jammed = feed_broken || fire_mode.jams_on(roll);
    // A Streak launcher fires only on a successful roll, even when Angel ECM has confused its
    // lock (Tactical Operations: AUE p. 91).
    let launched =
        !weapon.is_streak() || target_number.is_some_and(|number| i32::from(roll) >= number);
    let pod = weapon.beacon_kind(ammunition).is_some();
    let hit = launched
        && !caseless_failure
        && target_number.is_some_and(|number| i32::from(roll) >= number);
    Ok(LaunchRoll {
        roll,
        jammed,
        feed_broken,
        exploded,
        caseless_failure,
        launched,
        hit,
        // Tactical Operations glancing blows: a roll exactly equal to the target number halves damage.
        glancing: hit
            && !pod
            && (!weapon.is_streak() || streak_confused)
            && target_number == Some(i32::from(roll)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use Weapon as W;

    /// A seed whose first 2d6 draw is the given total.
    fn seed_rolling(total: u8) -> u8 {
        (0..=255)
            .find(|seed| Dice::seeded([*seed; 32]).two_d6() == total)
            .unwrap()
    }

    fn launch(
        weapon: Weapon,
        ammunition: AmmunitionMode,
        fire_mode: FireMode,
        target_number: Option<i32>,
        streak_confused: bool,
        seed: u8,
    ) -> LaunchRoll {
        roll_launch(
            LaunchRollRequest {
                weapon,
                ammunition,
                fire_mode,
                distance: 1.0,
                target_number,
                streak_confused,
            },
            &mut Dice::seeded([seed; 32]),
        )
        .unwrap()
    }

    /// Every attack is one generic 2d6 check, including extended LRMs inside their minimum
    /// range and caseless autocannons.
    #[test]
    fn every_launch_counts_one_generic_attack_check() {
        let seed = seed_rolling(2);
        for (weapon, ammunition) in [
            (W::MediumLaser, AmmunitionMode::Normal),
            (W::Elrm5, AmmunitionMode::Normal),
            (W::Ac10, AmmunitionMode::Caseless),
        ] {
            let mut dice = Dice::seeded([seed; 32]);
            let outcome = roll_launch(
                LaunchRollRequest {
                    weapon,
                    ammunition,
                    fire_mode: FireMode::Normal,
                    distance: 1.0,
                    target_number: Some(2),
                    streak_confused: false,
                },
                &mut dice,
            )
            .unwrap();
            assert_eq!(dice.generic_roll_statistics().total(), 1, "{weapon:?}");
            assert!(dice.generic_roll_statistics().counts()[usize::from(outcome.roll - 2)] > 0);
        }
    }

    /// Every feed failure still fires the shot; only its disabling consequence differs.
    #[test]
    fn feed_failures_fire_first() {
        let two = seed_rolling(2);
        let three = seed_rolling(3);
        let ultra = launch(
            W::UltraAc5,
            AmmunitionMode::Normal,
            FireMode::Ultra,
            Some(8),
            false,
            two,
        );
        assert!(ultra.launched && ultra.jammed && ultra.feed_broken && !ultra.exploded);
        assert!(!ultra.hit && !ultra.caseless_failure);
        let rapid = launch(
            W::Ac5,
            AmmunitionMode::Normal,
            FireMode::Rapid,
            Some(8),
            false,
            two,
        );
        assert!(rapid.launched && rapid.exploded && !rapid.jammed && !rapid.feed_broken);
        let rapid_jam = launch(
            W::Ac5,
            AmmunitionMode::Normal,
            FireMode::Rapid,
            Some(3),
            false,
            three,
        );
        assert!(rapid_jam.launched && rapid_jam.jammed && !rapid_jam.feed_broken && rapid_jam.hit);
        let rotary = launch(
            W::RotaryAc5,
            AmmunitionMode::Normal,
            FireMode::Rotary6,
            Some(3),
            false,
            three,
        );
        assert!(rotary.launched && rotary.jammed && !rotary.feed_broken && rotary.hit);
        let hotload = launch(
            W::Lrm20,
            AmmunitionMode::Normal,
            FireMode::Hotload,
            Some(3),
            false,
            two,
        );
        assert!(hotload.launched && !hotload.jammed && !hotload.feed_broken && !hotload.hit);
        // A caseless breakdown spends the shot but misses automatically, even against an easy target.
        let caseless = launch(
            W::Ac10,
            AmmunitionMode::Caseless,
            FireMode::Normal,
            Some(2),
            false,
            two,
        );
        assert!(caseless.launched && caseless.caseless_failure && caseless.feed_broken);
        assert!(caseless.jammed && !caseless.hit && !caseless.glancing && !caseless.exploded);
        let caseless_three = launch(
            W::Ac10,
            AmmunitionMode::Caseless,
            FireMode::Normal,
            Some(2),
            false,
            three,
        );
        assert!(!caseless_three.caseless_failure && !caseless_three.jammed && caseless_three.hit);
    }

    /// A confused Streak launcher still refuses to launch on a failed roll; a successful roll
    /// fires and may glance.
    #[test]
    fn confused_streak_launches_only_on_success() {
        let seven = seed_rolling(7);
        for confused in [false, true] {
            let miss = launch(
                W::StreakSrm6,
                AmmunitionMode::Normal,
                FireMode::Normal,
                Some(8),
                confused,
                seven,
            );
            assert!(!miss.launched && !miss.hit && !miss.glancing);
            let boundary = launch(
                W::StreakSrm6,
                AmmunitionMode::Normal,
                FireMode::Normal,
                Some(7),
                confused,
                seven,
            );
            assert!(boundary.launched && boundary.hit);
            assert_eq!(boundary.glancing, confused);
        }
    }
}
