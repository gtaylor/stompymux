//! Radar altitude, range, obscurant and aiming boundaries, and live tracking of airborne VTOLs.
use stompymux_rs::*;

/// Low altitude has a strict squared-range boundary, while high altitude keeps the hardware ceiling.
#[test]
fn radar_altitude_and_range_boundaries() {
    for (elevation, clearance, distance, maximum, reaches) in [
        (2, 10, 0.0, RADAR_RANGE, false),
        (3, 1, 0.0, RADAR_RANGE, false),
        (3, 2, 8.999, RADAR_RANGE, true),
        (3, 2, 9.0, RADAR_RANGE, false),
        (9, 2, 80.999, RADAR_RANGE, true),
        (9, 2, 81.0, RADAR_RANGE, false),
        (10, 2, 90.0, RADAR_RANGE, true),
        (10, 2, 180.0, RADAR_RANGE, true),
        (10, 2, 180.001, RADAR_RANGE, false),
        (10, 2, 100.0, 100, true),
        (10, 2, 100.001, 100, false),
    ] {
        let aim = BattleRadarTarget {
            flying_type: false,
            elevation,
            height_above_surface: clearance,
        }
        .evaluate(Default::default(), distance, maximum)
        .unwrap();
        assert_eq!(
            aim.is_some(),
            reaches,
            "{elevation}/{clearance}/{distance}/{maximum}"
        );
    }
}

/// Only terrain blockage rejects an otherwise elevated target; smoke, fire and water do not.
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
        partial_cover: true,
        ..Default::default()
    };
    assert_eq!(target.evaluate(terrain, 5.0, RADAR_RANGE).unwrap(), Some(9));
    assert_eq!(
        target
            .evaluate(Default::default(), 5.0, RADAR_RANGE)
            .unwrap(),
        Some(-3)
    );
    terrain.blocked = true;
    assert_eq!(target.evaluate(terrain, 5.0, RADAR_RANGE).unwrap(), None);
    for distance in [-0.01, f64::NAN, f64::INFINITY] {
        assert!(
            target
                .evaluate(Default::default(), distance, RADAR_RANGE)
                .is_err()
        );
    }
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
        let a = ground
            .evaluate(Default::default(), 2.0, RADAR_RANGE)
            .unwrap();
        let b = flying
            .evaluate(Default::default(), 2.0, RADAR_RANGE)
            .unwrap();
        assert_eq!(b.is_some(), a.is_some());
        if elevation <= 2 {
            assert_eq!(a, None);
            continue;
        }
        assert_eq!(b, Some(-3));
        assert_eq!(a, Some(if elevation >= 10 { -3 } else { 0 }));
    }
}

use crate::support::btech_firing as firing;

/// Live tracking gives rotorcraft the radar bonus, ignores clouds and obeys the map switch.
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
    let map = world.btech.units()[&observer].map.unwrap();
    let before = world.btech.clone();
    let radar = battle_perceive(&world, observer, target).unwrap().unwrap();
    assert_eq!(radar.channel, BattleDetectionChannel::Radar);
    assert_eq!(radar.aim_modifier, -3);
    assert!(radar.identified);
    assert_eq!(world.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_perceive(&restored, observer, target).unwrap(),
        Some(radar)
    );
    // A cloud layer between the tower and the aircraft hides it from sensors, not radar.
    set_battle_map_cloud_base(&mut restored, ObjectId(1), map, 2).unwrap();
    assert_eq!(
        battle_perceive(&restored, observer, target).unwrap(),
        Some(radar)
    );
    set_battle_map_perception(&mut restored, map, BattleMapPerceptionFlag::Radar, false).unwrap();
    assert_eq!(battle_perceive(&restored, observer, target).unwrap(), None);
    set_battle_map_cloud_base(&mut restored, ObjectId(1), map, 0).unwrap();
    let sensors = battle_perceive(&restored, observer, target)
        .unwrap()
        .unwrap();
    assert_eq!(
        (sensors.channel, sensors.aim_modifier),
        (BattleDetectionChannel::Sensors, 0)
    );
    assert_eq!(
        battle_perception_profile(&restored, observer)
            .unwrap()
            .radar
            .unwrap()
            .status,
        BattlePerceptionStatus::Disabled
    );
}
