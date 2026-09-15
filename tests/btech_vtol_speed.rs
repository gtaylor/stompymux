//! Rotorcraft velocity limits use one budget for both axes and retain damage semantics.
use stompymux_rs::*;

/// Material-only aircraft; the host flight lifecycle remains responsible for world admission.
fn aircraft() -> BattleVehicle {
    BattleVehicle::new(BattleVehicleTemplate::parse(include_str!("../game/mechs/Kestrel")).unwrap())
        .unwrap()
}

#[test]
fn rotorcraft_velocity_budget_is_symmetric_and_tracks_material_damage() {
    let mut unit = aircraft();
    let maximum = unit.maximum_speed();
    for sign in [-1.0, 1.0] {
        let horizontal = unit.vtol_horizontal_limit(sign * maximum * 0.6).unwrap();
        assert!((horizontal - maximum * 0.8).abs() < 1e-10);
        assert!(
            (unit.vtol_vertical_limit(sign * horizontal).unwrap() - maximum * 0.6).abs() < 1e-10
        );
    }
    assert_eq!(unit.vtol_throttle(f64::MAX, 0.0).unwrap(), maximum);
    assert_eq!(
        unit.vtol_throttle(-f64::MAX, 0.0).unwrap(),
        -maximum * 2.0 / 3.0
    );
    assert_eq!(unit.vtol_horizontal_limit(maximum).unwrap(), 0.0);
    assert_eq!(unit.vtol_vertical_limit(maximum * 2.0).unwrap(), 0.0);
    let damage = unit.apply_rotor_hit(BattleRotorHit::Damage, false).unwrap();
    assert_eq!(unit.vtol_horizontal_limit(0.0).unwrap(), damage.speed_after);
    let loss = unit
        .apply_rotor_hit(BattleRotorHit::Destroy, false)
        .unwrap();
    assert!(loss.lost_rotor);
    assert_eq!(unit.vtol_horizontal_limit(0.0).unwrap(), 0.0);
    assert_eq!(unit.vtol_vertical_limit(0.0).unwrap(), 0.0);
}

#[test]
fn tail_rotor_limits_commands_without_erasing_momentum_or_reverse_motion() {
    let mut saved = serde_json::to_value(aircraft()).unwrap();
    saved["position"] = serde_json::json!({"map":0,"x":0,"y":0});
    saved["map_slot"] = 0.into();
    saved["power"] = serde_json::to_value(BattlePower::Running).unwrap();
    let mut motion = BattleMotion::stationary(BattleHexCoordinate { x: 0, y: 0 }.center());
    motion.speed = 150.0;
    motion.desired_speed = 180.0;
    saved["motion"] = serde_json::to_value(motion).unwrap();
    let mut unit: BattleVehicle = serde_json::from_value(saved).unwrap();
    let damage = unit
        .apply_rotor_hit(BattleRotorHit::TailRotor, false)
        .unwrap();
    assert!(!damage.repeated);
    let vertical = unit.maximum_speed() * 0.6;
    let cruise = unit.vtol_horizontal_limit(vertical).unwrap() * 2.0 / 3.0;
    assert!(unit.vtol_throttle(cruise + 0.2, vertical).is_err());
    assert_eq!(unit.vtol_throttle(cruise, vertical).unwrap(), cruise);
    unit.constrain_vtol_cruise(vertical).unwrap();
    assert_eq!(unit.motion().unwrap().speed, 150.0);
    assert!((unit.motion().unwrap().desired_speed - (cruise - 0.1)).abs() < 1e-10);
    let before = unit.clone();
    unit.constrain_vtol_cruise(vertical).unwrap();
    assert_eq!(unit, before);
    let mut saved = serde_json::to_value(&unit).unwrap();
    saved["motion"]["desired_speed"] = (-10.0).into();
    let mut reverse: BattleVehicle = serde_json::from_value(saved).unwrap();
    reverse.constrain_vtol_cruise(vertical).unwrap();
    assert_eq!(reverse.motion().unwrap().desired_speed, -10.0);
    assert_eq!(
        serde_json::from_value::<BattleVehicle>(serde_json::to_value(&unit).unwrap()).unwrap(),
        unit
    );
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(unit.vtol_throttle(invalid, 0.0).is_err());
        assert!(unit.vtol_vertical_limit(invalid).is_err());
        assert!(unit.constrain_vtol_cruise(invalid).is_err());
        assert_eq!(unit, before);
    }
}
