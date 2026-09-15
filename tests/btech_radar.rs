//! Radar altitude, acquisition, obscurant and aiming boundaries independent of world mutation.
use stompymux_rs::*;

/// Low altitude has a strict squared-range boundary, while high altitude keeps the hardware ceiling.
#[test]
fn radar_altitude_range_and_acquisition() {
    for (elevation, clearance, distance, eligible, acquisition) in [
        (2, 10, 0.0, false, 0),
        (3, 1, 0.0, false, 0),
        (3, 2, 8.999, true, 90),
        (3, 2, 9.0, false, 0),
        (9, 2, 80.999, true, 90),
        (9, 2, 81.0, false, 0),
        (10, 2, 90.0, true, 90),
        (10, 2, 90.01, true, 89),
        (10, 2, 150.0, true, 30),
        (10, 2, 170.0, true, 10),
        (10, 2, 180.0, true, 10),
        (10, 2, 180.001, false, 0),
    ] {
        let report = BattleRadarTarget {
            flying_type: false,
            elevation,
            height_above_surface: clearance,
        }
        .evaluate(Default::default(), distance, false)
        .unwrap();
        assert_eq!(
            report.eligible, eligible,
            "{elevation}/{clearance}/{distance}"
        );
        assert_eq!(report.acquisition_factor, acquisition);
    }
}

/// Only terrain blockage and the disable switch reject an otherwise elevated target.
#[test]
fn radar_obscurants_signed_aim_and_invalid_inputs() {
    let target = BattleRadarTarget {
        flying_type: false,
        elevation: 10,
        height_above_surface: 5,
    };
    let mut terrain = BattleTerrainLos {
        woods: 8,
        target_woods: 2,
        water: 7,
        smoke: true,
        fire: true,
        mountain: true,
        partial_cover: true,
        ..Default::default()
    };
    let report = target.evaluate(terrain, 5.0, false).unwrap();
    assert!(report.eligible);
    assert_eq!(report.aim_modifier, 9);
    assert_eq!(
        target
            .evaluate(Default::default(), 5.0, false)
            .unwrap()
            .aim_modifier,
        -3
    );
    assert!(!target.evaluate(terrain, 5.0, true).unwrap().eligible);
    terrain.blocked = true;
    assert!(!target.evaluate(terrain, 5.0, false).unwrap().eligible);
    for distance in [-0.01, f64::NAN, f64::INFINITY] {
        assert!(
            target
                .evaluate(Default::default(), distance, false)
                .is_err()
        );
    }
    terrain.woods = 16;
    assert!(target.evaluate(terrain, 0.0, false).is_err());
}

/// Flying type changes accuracy, not the low-altitude clearance and detection gates.
#[test]
fn radar_flying_type_bonus_applies_below_ten_without_relaxing_detection() {
    for elevation in [0, 2, 3, 9, 10] {
        let ground = BattleRadarTarget {
            flying_type: false,
            elevation,
            height_above_surface: i64::from(elevation),
        };
        let flying = BattleRadarTarget {
            flying_type: true,
            ..ground
        };
        let a = ground.evaluate(Default::default(), 2.0, false).unwrap();
        let b = flying.evaluate(Default::default(), 2.0, false).unwrap();
        assert_eq!(b.eligible, a.eligible);
        assert_eq!(b.acquisition_factor, a.acquisition_factor);
        assert_eq!(b.aim_modifier, -3);
        assert_eq!(a.aim_modifier, if elevation >= 10 { -3 } else { 0 });
    }
}

#[allow(dead_code)]
#[path = "support/btech_firing.rs"]
mod firing;
mod support;

/// Live target sampling carries rotorcraft identity into the common radar evaluation.
#[tokio::test]
async fn live_low_altitude_vtol_receives_radar_bonus_after_restart() {
    let (_dir, config, mut world, observer, target, _) = firing::fixture_with_target(
        include_str!("../game/mechs/RadioTower"),
        None,
        include_str!("../game/mechs/Kestrel"),
    )
    .await;
    firing::edit(&mut world, target, |state| {
        state["vtol_flight"]["phase"] = serde_json::json!({"kind":"airborne"});
        state["vtol_flight"]["altitude"] = serde_json::json!(3.0);
    });
    let before = world.btech.clone();
    let report = battle_radar_contact(&world, observer, target).unwrap();
    assert!(report.eligible);
    assert_eq!(report.aim_modifier, -3);
    assert_eq!(world.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_radar_contact(&restored, observer, target).unwrap(),
        report
    );
}
