//! Saved rotor material state preserves hull and crew while making new lift loss explicit.
use crate::support;
use stompymux_rs::*;

/// A rotorcraft whose equipment uses the ordinary vehicle loadout and material model.
fn aircraft() -> BattleVehicle {
    BattleVehicle::new(BattleVehicleTemplate::parse(include_str!("../game/mechs/Kestrel")).unwrap())
        .unwrap()
}

#[test]
fn repeated_rotor_damage_reuses_speed_loss_and_preserves_hull_and_crew() {
    let mut unit = aircraft();
    let original = unit.clone();
    let stalled = unit.apply_rotor_hit(BattleRotorHit::Damage, true).unwrap();
    assert_eq!(stalled.speed_before, stalled.speed_after);
    assert_eq!(unit, original);
    let tail = unit
        .apply_rotor_hit(BattleRotorHit::TailRotor, false)
        .unwrap();
    assert!(!tail.repeated && !tail.lost_rotor);
    assert!(unit.tail_rotor_destroyed());
    assert_eq!(unit.maximum_speed(), original.maximum_speed());
    assert!(
        unit.apply_rotor_hit(BattleRotorHit::TailRotor, false)
            .unwrap()
            .repeated
    );
    for step in 1..=18 {
        let before = unit.clone();
        let report = unit.apply_rotor_hit(BattleRotorHit::Damage, false).unwrap();
        assert_eq!(report.speed_after, (18 - step) as f64 * 10.75);
        assert_eq!(report.lost_rotor, step == 18);
        assert_eq!(unit.rotor_destroyed(), step == 18);
        assert!(!unit.is_destroyed());
        assert!(!unit.crew_killed());
        assert_eq!(unit.pilot_injuries(), 0);
        for section in [
            BattleVehicleSection::Front,
            BattleVehicleSection::Rear,
            BattleVehicleSection::Left,
            BattleVehicleSection::Right,
        ] {
            assert_eq!(unit.sections()[&section], original.sections()[&section]);
        }
        let restored: BattleVehicle =
            serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
        assert_eq!(restored, unit);
        let mut replay = before;
        assert_eq!(
            replay
                .apply_rotor_hit(BattleRotorHit::Damage, false)
                .unwrap(),
            report
        );
        assert_eq!(replay, unit);
    }
    let wreck = unit.clone();
    let repeated = unit
        .apply_rotor_hit(BattleRotorHit::Destroy, false)
        .unwrap();
    assert!(repeated.repeated && !repeated.lost_rotor);
    assert_eq!(unit, wreck);
}

#[test]
fn direct_rotor_section_loss_uses_shared_damage_and_ground_models_reject_rotor_state() {
    let mut unit = aircraft();
    let report = unit
        .damage_phase(
            BattleVehicleSection::Rotor,
            u16::MAX,
            BattleDamagePhase::Internal,
        )
        .unwrap();
    assert!(!report.unit_destroyed);
    assert_eq!(report.destroyed_sections, [BattleVehicleSection::Rotor]);
    assert!(unit.rotor_destroyed());
    assert_eq!(unit.maximum_speed(), 0.0);
    assert_eq!(unit.sections()[&BattleVehicleSection::Rotor].armor, 0);
    let mut corrupt = serde_json::to_value(&unit).unwrap();
    corrupt["position"] = serde_json::json!({"map": 0, "x": 0, "y": 0});
    corrupt["map_slot"] = 0.into();
    corrupt["power"] = serde_json::to_value(BattlePower::Running).unwrap();
    let mut motion = BattleMotion::stationary(BattleHexCoordinate { x: 0, y: 0 }.center());
    motion.speed = 1.0;
    motion.desired_speed = 1.0;
    corrupt["motion"] = serde_json::to_value(motion).unwrap();
    let error = serde_json::from_value::<BattleVehicle>(corrupt).unwrap_err();
    assert!(error.to_string().contains("Rotorless aircraft"), "{error}");
    let ground = BattleVehicle::new(
        BattleVehicleTemplate::parse(include_str!("../game/mechs/Demolisher")).unwrap(),
    )
    .unwrap();
    let mut changed = ground.clone();
    assert!(
        changed
            .apply_rotor_hit(BattleRotorHit::Damage, false)
            .is_err()
    );
    assert_eq!(changed, ground);
    let mut corrupt = serde_json::to_value(&ground).unwrap();
    corrupt["tail_rotor_destroyed"] = true.into();
    assert!(serde_json::from_value::<BattleVehicle>(corrupt).is_err());
}

#[tokio::test]
async fn aircraft_construction_registers_valid_landed_flight_state() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Aircraft".into(), Kind::Thing);
    let template = aircraft().definition().clone();
    create_battle_vehicle(&mut world, id, template).unwrap();
    world.validate(&config).unwrap();
    let unit = &world.btech.vehicles()[&id];
    assert_eq!(
        unit.vtol_flight().unwrap().phase,
        BattleVtolFlightPhase::Landed
    );
    assert!(unit.position().is_none());
}
