//! Infrared hardware limits, obscurants and signed thermal accuracy thresholds.
use stompymux_rs::*;

/// Infrared ignores smoke, light, water and weather visibility; fire, terrain and six woods block it.
#[test]
fn infrared_visibility_and_range() {
    let conditions = BattleSensorConditions {
        light: BattleLight::Night,
        visibility: 0,
        disabled: false,
        target_lit: true,
    };
    let terrain = BattleTerrainLos {
        smoke: true,
        water: 7,
        woods: 5,
        target_woods: 2,
        partial_cover: true,
        ..Default::default()
    };
    let report = BattleSensorMode::Infrared
        .evaluate(terrain, 15.0, true, conditions)
        .unwrap();
    assert!(report.eligible);
    assert_eq!(report.acquisition_factor, 65);
    assert_eq!(report.aim_modifier, 7 * 4 / 3 + 3 + 2);
    assert!(
        !BattleSensorMode::Infrared
            .evaluate(terrain, 15.001, true, conditions)
            .unwrap()
            .eligible
    );
    for terrain in [
        BattleTerrainLos {
            fire: true,
            ..terrain
        },
        BattleTerrainLos {
            blocked: true,
            ..terrain
        },
        BattleTerrainLos {
            woods: 6,
            ..terrain
        },
    ] {
        assert!(
            !BattleSensorMode::Infrared
                .evaluate(terrain, 1.0, false, conditions)
                .unwrap()
                .eligible
        );
    }
    assert!(
        !BattleSensorMode::Infrared
            .evaluate(
                Default::default(),
                1.0,
                false,
                BattleSensorConditions {
                    disabled: true,
                    ..conditions
                }
            )
            .unwrap()
            .eligible
    );
}

/// Threshold boundaries are strict, and hot targets may grant a negative aim contribution.
#[test]
fn infrared_heat_accuracy_thresholds() {
    for (signature, modifier) in [
        (0.0, 2),
        (0.1, 1),
        (20.0, 1),
        (20.1, 0),
        (35.0, 0),
        (35.1, -1),
        (50.0, -1),
        (50.1, -2),
    ] {
        assert_eq!(
            battle_infrared_heat_modifier(BattleHeatRates {
                production: signature,
                dissipation: signature
            })
            .unwrap(),
            modifier
        );
    }
    assert_eq!(
        battle_infrared_heat_modifier(BattleHeatRates {
            production: 20.0,
            dissipation: 5.0
        })
        .unwrap(),
        0
    );
    assert!(
        battle_infrared_heat_modifier(BattleHeatRates {
            production: f64::NAN,
            dissipation: 0.0
        })
        .is_err()
    );
    assert_eq!(
        "I".parse::<BattleSensorMode>().unwrap(),
        BattleSensorMode::Infrared
    );
}
