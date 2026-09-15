//! Electromagnetic range fluctuation, obscurants and signed aiming contributions.
use stompymux_rs::*;

/// A stationary nominal fifty-ton target has no weight or motion contribution.
fn target() -> BattleElectromagneticTarget {
    BattleElectromagneticTarget {
        tons: 50,
        speed: 0.0,
        fired_recently: false,
    }
}

/// Range bands use the shared signal but EM's own sixteen-to-twenty-four-hex hardware profile.
#[test]
fn electromagnetic_range_signal_and_acquisition() {
    for (signal, reach) in [
        (0, 16),
        (11, 16),
        (12, 17),
        (22, 17),
        (23, 18),
        (34, 19),
        (45, 20),
        (56, 21),
        (67, 22),
        (78, 23),
        (89, 24),
        (100, 24),
    ] {
        let rules = BattleElectromagneticRules {
            signal_strength: signal,
            aim_adjustment: 0,
        };
        let report = rules
            .evaluate(Default::default(), f64::from(reach), target(), false, false)
            .unwrap();
        assert!(report.eligible);
        assert_eq!(report.acquisition_factor, 30 - reach);
        assert!(
            !rules
                .evaluate(
                    Default::default(),
                    f64::from(reach) + 0.001,
                    target(),
                    false,
                    false
                )
                .unwrap()
                .eligible
        );
    }
}

/// Smoke, fire and water remain eligible, while terrain, eight woods and ECM disturbance block contact.
#[test]
fn electromagnetic_obstacles_and_interference() {
    let rules = BattleElectromagneticRules {
        signal_strength: 100,
        aim_adjustment: 0,
    };
    let terrain = BattleTerrainLos {
        fire: true,
        smoke: true,
        water: 7,
        woods: 7,
        target_woods: 2,
        ..Default::default()
    };
    assert!(
        rules
            .evaluate(terrain, 1.0, target(), false, false)
            .unwrap()
            .eligible
    );
    for (terrain, disturbed, disabled) in [
        (terrain, true, false),
        (terrain, false, true),
        (
            BattleTerrainLos {
                mountain: true,
                ..terrain
            },
            false,
            false,
        ),
        (
            BattleTerrainLos {
                blocked: true,
                ..terrain
            },
            false,
            false,
        ),
        (
            BattleTerrainLos {
                woods: 8,
                ..terrain
            },
            false,
            false,
        ),
    ] {
        let report = rules
            .evaluate(terrain, 1.0, target(), disturbed, disabled)
            .unwrap();
        assert!(!report.eligible);
        assert_eq!(report.acquisition_factor, 0);
    }
}

/// EM favors heavy and recently firing targets, but adds a penalty for movement in either direction.
#[test]
fn electromagnetic_aim_weight_motion_fire_and_jitter() {
    let rules = BattleElectromagneticRules {
        signal_strength: 100,
        aim_adjustment: 0,
    };
    for (tons, weight) in [(35, 1), (36, 0), (65, 0), (66, -1)] {
        for (speed, movement) in [(-10.75, 1), (-10.749, 0), (0.0, 0), (10.749, 0), (10.75, 1)] {
            let target = BattleElectromagneticTarget {
                tons,
                speed,
                fired_recently: true,
            };
            assert_eq!(
                rules
                    .evaluate(Default::default(), 1.0, target, false, false)
                    .unwrap()
                    .aim_modifier,
                weight + movement - 1
            );
            let terrain = BattleTerrainLos {
                woods: 4,
                target_woods: 1,
                partial_cover: true,
                ..Default::default()
            };
            assert_eq!(
                BattleElectromagneticRules {
                    aim_adjustment: 1,
                    ..rules
                }
                .evaluate(terrain, 1.0, target, false, false)
                .unwrap()
                .aim_modifier,
                weight + movement + 6
            );
        }
    }
    assert!(
        BattleElectromagneticRules {
            signal_strength: 101,
            ..rules
        }
        .evaluate(Default::default(), 1.0, target(), false, false)
        .is_err()
    );
    assert!(
        BattleElectromagneticRules {
            aim_adjustment: 2,
            ..rules
        }
        .evaluate(Default::default(), 1.0, target(), false, false)
        .is_err()
    );
    assert!(
        rules
            .evaluate(Default::default(), f64::NAN, target(), false, false)
            .is_err()
    );
}
