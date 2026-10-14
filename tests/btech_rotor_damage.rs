//! Saved rotor material state: one cruising MP lost per hit with flank recomputed (Total
//! Warfare p. 197), the flight stabilizer, rotor destruction and the realtime rule that a
//! grounded VTOL with no rotor is a wreck.
use crate::support;
use stompymux_rs::*;

/// A rotorcraft whose equipment uses the ordinary vehicle loadout and material model.
fn aircraft() -> Vehicle {
    Vehicle::new(
        VehicleTemplate::parse("Kestrel", include_str!("../game/units/Kestrel.toml")).unwrap(),
    )
    .unwrap()
}

#[test]
fn rotor_damage_lowers_speed_one_mp_at_a_time_and_the_flight_stabilizer_hits_once() {
    let mut unit = aircraft();
    let original = unit.clone();
    let stalled = unit.apply_rotor_hit(RotorHit::Damage, true).unwrap();
    assert_eq!(stalled.speed_before, stalled.speed_after);
    assert_eq!(unit, original);
    let stabilizer = unit
        .apply_rotor_hit(RotorHit::FlightStabilizer, false)
        .unwrap();
    assert!(!stabilizer.lost_rotor && !stabilizer.grounded);
    assert!(unit.tail_rotor_destroyed());
    assert_eq!(unit.piloting_damage(), 3);
    assert_eq!(unit.gunnery_damage(), 1);
    assert_eq!(unit.maximum_speed(), original.maximum_speed());
    let _ = unit
        .apply_rotor_hit(RotorHit::FlightStabilizer, false)
        .unwrap();
    assert_eq!(
        unit.piloting_damage(),
        3,
        "a second stabilizer hit adds nothing"
    );
    // A 12/18 Kestrel loses one cruising MP per hit and recomputes flank as 1.5 times the
    // rest, rounded up (TW p. 197): 17, 15, 14, 12, 11, 9, 8, 6, 5, 3, 2 and then nothing.
    for flank in [17, 15, 14, 12, 11, 9, 8, 6, 5, 3, 2, 0] {
        let before = unit.clone();
        let report = unit.apply_rotor_hit(RotorHit::Damage, false).unwrap();
        assert_eq!(report.speed_after, f64::from(flank) * 10.75);
        assert!(!report.lost_rotor);
        assert!(!unit.rotor_destroyed());
        assert!(!unit.is_destroyed());
        assert!(!unit.crew_killed());
        for section in [
            VehicleSection::Front,
            VehicleSection::Rear,
            VehicleSection::Left,
            VehicleSection::Right,
            VehicleSection::Rotor,
        ] {
            assert_eq!(unit.sections()[&section], original.sections()[&section]);
        }
        let restored: Vehicle =
            serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
        assert_eq!(restored, unit);
        let mut replay = before;
        assert_eq!(
            replay.apply_rotor_hit(RotorHit::Damage, false).unwrap(),
            report
        );
        assert_eq!(replay, unit);
    }
    // Rotors Destroyed on a landed VTOL is destruction (TW p. 197).
    let destroyed = unit.apply_rotor_hit(RotorHit::Destroy, false).unwrap();
    assert!(destroyed.lost_rotor);
    assert!(unit.rotor_destroyed() && unit.is_destroyed());
    let wreck = unit.clone();
    let repeated = unit.apply_rotor_hit(RotorHit::Destroy, false).unwrap();
    assert!(!repeated.lost_rotor);
    assert_eq!(unit, wreck);
}

#[test]
fn direct_rotor_section_loss_uses_shared_damage_and_ground_models_reject_rotor_state() {
    let mut unit = aircraft();
    let report = unit
        .damage_phase(VehicleSection::Rotor, u16::MAX, DamagePhase::Internal)
        .unwrap();
    assert!(
        report.unit_destroyed,
        "a grounded VTOL without a rotor is a wreck"
    );
    assert_eq!(report.destroyed_sections, [VehicleSection::Rotor]);
    assert!(unit.rotor_destroyed());
    assert_eq!(unit.maximum_speed(), 0.0);
    assert_eq!(unit.sections()[&VehicleSection::Rotor].armor, 0);
    let mut corrupt = serde_json::to_value(&unit).unwrap();
    corrupt["position"] = serde_json::json!({"map": 0, "x": 0, "y": 0});
    corrupt["map_slot"] = 0.into();
    corrupt["power"] = serde_json::to_value(Power::Running).unwrap();
    let mut motion = Motion::stationary(HexCoordinate { x: 0, y: 0 }.center());
    motion.speed = 1.0;
    motion.desired_speed = 1.0;
    corrupt["motion"] = serde_json::to_value(motion).unwrap();
    assert!(serde_json::from_value::<Vehicle>(corrupt).is_err());
    let ground = Vehicle::new(
        VehicleTemplate::parse("Demolisher", include_str!("../game/units/Demolisher.toml"))
            .unwrap(),
    )
    .unwrap();
    let mut changed = ground.clone();
    assert!(changed.apply_rotor_hit(RotorHit::Damage, false).is_err());
    assert_eq!(changed, ground);
    let mut corrupt = serde_json::to_value(&ground).unwrap();
    corrupt["tail_rotor_destroyed"] = true.into();
    assert!(serde_json::from_value::<Vehicle>(corrupt).is_err());
}

#[test]
fn an_airborne_rotor_loss_is_a_crash_in_progress_rather_than_a_wreck() {
    let mut saved = serde_json::to_value(aircraft()).unwrap();
    saved["position"] = serde_json::json!({"map": 0, "x": 0, "y": 0});
    saved["map_slot"] = 0.into();
    saved["power"] = serde_json::to_value(Power::Running).unwrap();
    saved["motion"] =
        serde_json::to_value(Motion::stationary(HexCoordinate { x: 0, y: 0 }.center())).unwrap();
    saved["vtol_flight"] = serde_json::to_value(VtolFlight {
        fall: None,
        phase: VtolFlightPhase::Airborne,
        altitude: 6.0,
        vertical_speed: 0.0,
    })
    .unwrap();
    let mut unit: Vehicle = serde_json::from_value(saved).unwrap();
    let report = unit.apply_rotor_hit(RotorHit::Destroy, false).unwrap();
    assert!(report.lost_rotor);
    assert!(!unit.is_destroyed());
    assert_eq!(unit.vtol_flight().unwrap().phase, VtolFlightPhase::Falling);
}

#[tokio::test]
async fn aircraft_construction_registers_valid_landed_flight_state() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Aircraft".into(), Kind::Thing);
    let template = aircraft().definition().clone();
    create_battle_vehicle(&mut world, id, template).unwrap();
    world.validate(&config).unwrap();
    let unit = &world.btech.vehicles()[&id];
    assert_eq!(unit.vtol_flight().unwrap().phase, VtolFlightPhase::Landed);
    assert!(unit.position().is_none());
}
