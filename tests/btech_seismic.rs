//! Seismic physical eligibility, signal bands and explicit random aiming adjustments.
use stompymux_rs::*;

/// A moving, running biped presents a normal ground signature.
fn target() -> BattleSeismicTarget {
    BattleSeismicTarget {
        running: true,
        jumping: false,
        speed: 21.5,
        mass: 50 * 1024,
    }
}

/// Signal variation extends the four-hex guaranteed band in discrete steps up to eight hexes.
#[test]
fn seismic_signal_range_bands() {
    for (signal, reach) in [
        (0, 4),
        (19, 4),
        (20, 5),
        (39, 5),
        (40, 6),
        (59, 6),
        (60, 7),
        (79, 7),
        (80, 8),
        (100, 8),
    ] {
        let rules = BattleSeismicRules {
            detect_stopped: false,
            signal_strength: signal,
            aim_adjustment: 0,
        };
        let report = rules
            .evaluate(f64::from(reach), false, target(), false, false)
            .unwrap();
        assert!(report.eligible);
        assert_eq!(report.acquisition_factor, 50 - reach * 4);
        assert!(
            !rules
                .evaluate(f64::from(reach) + 0.001, false, target(), false, false)
                .unwrap()
                .eligible
        );
    }
}

/// Motion eligibility is strict at one MP, while its aiming bonus is inclusive and applies in reverse.
#[test]
fn seismic_motion_power_and_jump_rules() {
    let rules = BattleSeismicRules {
        detect_stopped: false,
        signal_strength: 80,
        aim_adjustment: 0,
    };
    for speed in [-10.751, -10.75, 0.0, 10.75, 10.751] {
        let target = BattleSeismicTarget { speed, ..target() };
        let report = rules.evaluate(1.0, false, target, false, false).unwrap();
        assert_eq!(report.eligible, speed.abs() > 10.75);
        assert_eq!(
            report.aim_modifier,
            if speed.abs() >= 10.75 { 1 } else { 2 }
        );
        assert!(
            BattleSeismicRules {
                detect_stopped: true,
                ..rules
            }
            .evaluate(1.0, false, target, false, false)
            .unwrap()
            .eligible
        );
    }
    for (jumping, target, disabled) in [
        (true, target(), false),
        (
            false,
            BattleSeismicTarget {
                running: false,
                ..target()
            },
            false,
        ),
        (
            false,
            BattleSeismicTarget {
                jumping: true,
                ..target()
            },
            false,
        ),
        (false, target(), true),
    ] {
        let report = rules
            .evaluate(1.0, jumping, target, false, disabled)
            .unwrap();
        assert!(!report.eligible);
        assert_eq!(report.acquisition_factor, 0);
    }
}

/// Current mass is truncated to whole tons, and cover/random contributions are independent.
#[test]
fn seismic_mass_cover_and_random_aim() {
    let rules = BattleSeismicRules {
        detect_stopped: true,
        signal_strength: 80,
        aim_adjustment: 0,
    };
    for (mass, modifier) in [
        (35 * 1024, 2),
        (36 * 1024 - 1, 2),
        (36 * 1024, 1),
        (65 * 1024, 1),
        (66 * 1024 - 1, 1),
        (66 * 1024, 0),
    ] {
        let target = BattleSeismicTarget { mass, ..target() };
        assert_eq!(
            rules
                .evaluate(1.0, false, target, false, false)
                .unwrap()
                .aim_modifier,
            modifier
        );
        assert_eq!(
            BattleSeismicRules {
                aim_adjustment: 1,
                ..rules
            }
            .evaluate(1.0, false, target, true, false)
            .unwrap()
            .aim_modifier,
            modifier + 4
        );
    }
    for rules in [
        BattleSeismicRules {
            signal_strength: 101,
            ..rules
        },
        BattleSeismicRules {
            aim_adjustment: 2,
            ..rules
        },
    ] {
        assert!(rules.evaluate(1.0, false, target(), false, false).is_err());
    }
    assert!(
        rules
            .evaluate(f64::NAN, false, target(), false, false)
            .is_err()
    );
    assert!(
        rules
            .evaluate(
                1.0,
                false,
                BattleSeismicTarget {
                    speed: f64::INFINITY,
                    ..target()
                },
                false,
                false
            )
            .is_err()
    );
}
