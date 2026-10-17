//! Rotorcraft movement limits share one additive budget between both axes (Total Warfare
//! p. 54) and retain damage semantics.
use stompymux_rs::*;

/// Material-only aircraft; the host flight lifecycle remains responsible for world admission.
fn aircraft() -> Vehicle {
    Vehicle::new(
        VehicleTemplate::parse("Kestrel", include_str!("../game/units/Kestrel.toml")).unwrap(),
    )
    .unwrap()
}

#[test]
fn rotorcraft_velocity_budget_is_symmetric_and_tracks_material_damage() {
    let mut unit = aircraft();
    let maximum = unit.maximum_speed();
    for sign in [-1.0, 1.0] {
        // Hexes and levels come from one pool of MP: 60% spent climbing leaves 40% for travel.
        let horizontal = unit.vtol_horizontal_limit(sign * maximum * 0.6).unwrap();
        assert!((horizontal - maximum * 0.4).abs() < 1e-10);
        assert!(
            (unit.vtol_vertical_limit(sign * horizontal).unwrap() - maximum * 0.6).abs() < 1e-10
        );
    }
    // The 12/18 Kestrel flanks forward but reverses no faster than its template's 12
    // cruising MP (Total Warfare p. 53).
    assert_eq!((unit.cruising_speed(), maximum), (129.0, 193.5));
    assert_eq!(unit.vtol_throttle(f64::MAX, 0.0).unwrap(), maximum);
    assert_eq!(unit.vtol_throttle(-f64::MAX, 0.0).unwrap(), -129.0);
    assert_eq!(unit.vtol_throttle(-f64::MAX, 43.0).unwrap(), -86.0);
    assert_eq!(unit.vtol_horizontal_limit(maximum).unwrap(), 0.0);
    assert_eq!(unit.vtol_vertical_limit(maximum * 2.0).unwrap(), 0.0);
    let damage = unit.apply_rotor_hit(RotorHit::Damage, false).unwrap();
    assert_eq!(unit.vtol_horizontal_limit(0.0).unwrap(), damage.speed_after);
    let loss = unit.apply_rotor_hit(RotorHit::Destroy, false).unwrap();
    assert!(loss.lost_rotor);
    assert_eq!(unit.vtol_horizontal_limit(0.0).unwrap(), 0.0);
    assert_eq!(unit.vtol_vertical_limit(0.0).unwrap(), 0.0);
}

#[test]
fn tail_rotor_limits_commands_without_erasing_momentum_or_reverse_motion() {
    let mut saved = serde_json::to_value(aircraft()).unwrap();
    saved["position"] = serde_json::json!({"map":0,"x":0,"y":0});
    saved["map_slot"] = 0.into();
    saved["power"] = serde_json::to_value(Power::Running).unwrap();
    let mut motion = Motion::stationary(HexCoordinate { x: 0, y: 0 }.center());
    motion.speed = 150.0;
    motion.desired_speed = 180.0;
    saved["motion"] = serde_json::to_value(motion).unwrap();
    let mut unit: Vehicle = serde_json::from_value(saved).unwrap();
    let _ = unit
        .apply_rotor_hit(RotorHit::FlightStabilizer, false)
        .unwrap();
    // Climbing spends the cruising budget too (TW p. 54): at 60% of its 193.5 km/h flank, the
    // 12/18 Kestrel has 12.9 km/h of its 129 km/h cruise left for horizontal travel.
    let vertical = unit.maximum_speed() * 0.6;
    let cruise = unit.cruising_speed() - vertical;
    assert!((cruise - 12.9).abs() < 1e-9);
    assert!(unit.vtol_throttle(cruise + 0.2, vertical).is_err());
    assert_eq!(unit.vtol_throttle(cruise, vertical).unwrap(), cruise);
    unit.constrain_vtol_cruise(vertical).unwrap();
    assert_eq!(unit.motion().unwrap().speed, 150.0);
    assert_eq!(unit.motion().unwrap().desired_speed, cruise);
    let before = unit.clone();
    unit.constrain_vtol_cruise(vertical).unwrap();
    assert_eq!(unit, before);
    let mut saved = serde_json::to_value(&unit).unwrap();
    saved["motion"]["desired_speed"] = (-10.0).into();
    let mut reverse: Vehicle = serde_json::from_value(saved).unwrap();
    reverse.constrain_vtol_cruise(vertical).unwrap();
    assert_eq!(reverse.motion().unwrap().desired_speed, -10.0);
    assert_eq!(
        serde_json::from_value::<Vehicle>(serde_json::to_value(&unit).unwrap()).unwrap(),
        unit
    );
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(unit.vtol_throttle(invalid, 0.0).is_err());
        assert!(unit.vtol_vertical_limit(invalid).is_err());
        assert!(unit.constrain_vtol_cruise(invalid).is_err());
        assert_eq!(unit, before);
    }
}

/// A flight stabilizer hit holds the VTOL to cruising speed (Total Warfare p. 197), and climbing
/// spends the same MP as travel (p. 54), so the climb a 12/18 Kestrel may command is what its
/// horizontal throttle leaves of its 129 km/h cruise, not of its 193.5 km/h flank.
#[test]
fn a_stabilizer_hit_holds_the_climb_to_the_cruising_budget() {
    let mut saved = serde_json::to_value(aircraft()).unwrap();
    saved["position"] = serde_json::json!({"map":0,"x":0,"y":0});
    saved["map_slot"] = 0.into();
    saved["power"] = serde_json::to_value(Power::Running).unwrap();
    saved["vtol_flight"]["phase"] = serde_json::json!({"kind": "airborne"});
    saved["vtol_flight"]["altitude"] = 5.0.into();
    let mut motion = Motion::stationary(HexCoordinate { x: 0, y: 0 }.center());
    motion.speed = 43.0;
    motion.desired_speed = 43.0;
    saved["motion"] = serde_json::to_value(motion).unwrap();
    let mut unit: Vehicle = serde_json::from_value(saved).unwrap();
    assert_eq!(unit.vtol_vertical_limit(43.0).unwrap(), 150.5);
    unit.set_vtol_vertical_speed(100.0).unwrap();
    let _ = unit
        .apply_rotor_hit(RotorHit::FlightStabilizer, false)
        .unwrap();
    assert_eq!(unit.vtol_vertical_limit(43.0).unwrap(), 86.0);
    assert_eq!(unit.vtol_horizontal_limit(0.0).unwrap(), 129.0);
    let before = unit.clone();
    assert!(unit.set_vtol_vertical_speed(86.5).is_err());
    assert!(unit.set_vtol_vertical_speed(-86.5).is_err());
    assert_eq!(unit, before);
    unit.set_vtol_vertical_speed(-86.0).unwrap();
    assert_eq!(unit.vtol_flight().unwrap().vertical_speed, -86.0);
}
