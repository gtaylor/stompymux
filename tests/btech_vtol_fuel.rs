//! Replayable aircraft fuel consumption, exhaustion and schema validation.
use stompymux_rs::*;

/// Material-only running aircraft with configurable powerplant and fuel capacity.
fn aircraft(combustion: bool, fuel: u32, speed: f64, seed: u8) -> BattleVehicle {
    let source = format!(
        "{}\nFuel {{ {fuel} }}\n",
        include_str!("../game/mechs/Kestrel.toml")
    );
    let mut template = BattleVehicleTemplate::parse("test",&source).unwrap();
    if !combustion {
        template
            .attributes
            .insert("specials".into(), "CargoTech".into());
    }
    let unit = BattleVehicle::new(template).unwrap();
    let mut saved = serde_json::to_value(unit).unwrap();
    saved["position"] = serde_json::json!({"map":0,"x":0,"y":0});
    saved["map_slot"] = 0.into();
    saved["power"] = serde_json::to_value(BattlePower::Running).unwrap();
    let mut motion = BattleMotion::stationary(BattleHexCoordinate { x: 0, y: 0 }.center());
    motion.speed = speed;
    motion.desired_speed = speed;
    saved["motion"] = serde_json::to_value(motion).unwrap();
    saved["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    serde_json::from_value(saved).unwrap()
}

#[test]
fn fuel_reaches_zero_before_announcing_exhaustion_and_preserves_material() {
    let mut unit = aircraft(true, 2, 21.5, 0);
    let original = unit.clone();
    for remaining in [1, 0] {
        assert_eq!(
            unit.consume_vtol_fuel(21.5, 1, false, false).unwrap(),
            BattleVtolFuelUse::Consumed {
                amount: 1,
                remaining
            }
        );
    }
    assert_eq!(
        unit.consume_vtol_fuel(21.5, 1, false, false).unwrap(),
        BattleVtolFuelUse::Exhausted { newly: true }
    );
    assert_eq!(unit.vtol_fuel().unwrap().remaining(), -1);
    assert!(!unit.motion().unwrap().active());
    assert!(!unit.rotor_destroyed() && !unit.is_destroyed());
    assert_eq!(unit.sections(), original.sections());
    assert_eq!(
        unit.consume_vtol_fuel(21.5, 1, false, false).unwrap(),
        BattleVtolFuelUse::Exhausted { newly: false }
    );
    assert_eq!(
        serde_json::from_value::<BattleVehicle>(serde_json::to_value(&unit).unwrap()).unwrap(),
        unit
    );
}

#[test]
fn low_speed_fuel_checks_and_exemptions_replay_the_same_saved_dice() {
    for skip in [false, true] {
        let seed = (0..=255)
            .find(|seed| (BattleDice::seeded([*seed; 32]).die(2).unwrap() == 1) == skip)
            .unwrap();
        let base = aircraft(true, 10, 0.0, seed);
        let mut unit = base.clone();
        let report = unit.consume_vtol_fuel(0.0, 0, false, true).unwrap();
        assert_eq!(
            report,
            if skip {
                BattleVtolFuelUse::Skipped
            } else {
                BattleVtolFuelUse::Consumed {
                    amount: 1,
                    remaining: 9,
                }
            }
        );
        let mut dice = BattleDice::seeded([seed; 32]);
        dice.die(2).unwrap();
        assert_eq!(
            serde_json::to_value(&unit).unwrap()["dice"],
            serde_json::to_value(dice).unwrap()
        );
        let mut replay: BattleVehicle =
            serde_json::from_value(serde_json::to_value(&base).unwrap()).unwrap();
        assert_eq!(
            replay.consume_vtol_fuel(0.0, 0, false, true).unwrap(),
            report
        );
        assert_eq!(replay, unit);
    }
    let mut fusion = aircraft(false, 10, 0.0, 0);
    let before = fusion.clone();
    assert_eq!(
        fusion.consume_vtol_fuel(0.0, 0, false, true).unwrap(),
        BattleVtolFuelUse::Skipped
    );
    assert_eq!(fusion, before);
    assert_eq!(
        fusion.consume_vtol_fuel(0.0, 0, true, false).unwrap(),
        BattleVtolFuelUse::Skipped
    );
    assert_eq!(fusion, before);
    assert!(fusion.consume_vtol_fuel(f64::NAN, 0, false, false).is_err());
    assert_eq!(fusion, before);
}

#[test]
fn fuel_defaults_and_invalid_snapshots_are_checked_against_the_chassis() {
    let aircraft = BattleVehicle::new(
        BattleVehicleTemplate::parse("Kestrel",include_str!("../game/mechs/Kestrel.toml")).unwrap(),
    )
    .unwrap();
    assert_eq!(aircraft.vtol_fuel().unwrap().capacity(), 4000);
    for remaining in [-2_i64, i64::from(u32::MAX) + 1] {
        let mut saved = serde_json::to_value(&aircraft).unwrap();
        saved["vtol_fuel"]["remaining"] = remaining.into();
        assert!(serde_json::from_value::<BattleVehicle>(saved).is_err());
    }
    let mut saved = serde_json::to_value(&aircraft).unwrap();
    saved["vtol_fuel"]["capacity"] = 4001.into();
    assert!(serde_json::from_value::<BattleVehicle>(saved).is_err());
    let ground = BattleVehicle::new(
        BattleVehicleTemplate::parse("Demolisher",include_str!("../game/mechs/Demolisher.toml")).unwrap(),
    )
    .unwrap();
    assert!(ground.vtol_fuel().is_none());
    let mut saved = serde_json::to_value(&ground).unwrap();
    saved["vtol_fuel"] = serde_json::to_value(aircraft.vtol_fuel()).unwrap();
    assert!(serde_json::from_value::<BattleVehicle>(saved).is_err());
}

#[test]
fn overspeed_fuel_cost_depends_on_altitude_and_shutdown_does_not_draw_dice() {
    let base = aircraft(true, 5, 21.5, 0);
    let mut saved = serde_json::to_value(base).unwrap();
    saved["motive_speed_loss"] = 182.75.into();
    saved["motion"]["desired_speed"] = 10.75.into();
    let base: BattleVehicle = serde_json::from_value(saved).unwrap();
    for (altitude, amount) in [(99, 2), (100, 1)] {
        let mut unit = base.clone();
        assert_eq!(
            unit.consume_vtol_fuel(0.0, altitude, false, false).unwrap(),
            BattleVtolFuelUse::Consumed {
                amount,
                remaining: 5 - amount
            }
        );
        assert_eq!(
            serde_json::to_value(&unit).unwrap()["dice"],
            serde_json::to_value(&base).unwrap()["dice"]
        );
    }
    let mut unit = BattleVehicle::new(
        BattleVehicleTemplate::parse("Kestrel",include_str!("../game/mechs/Kestrel.toml")).unwrap(),
    )
    .unwrap();
    let before = unit.clone();
    assert_eq!(
        unit.consume_vtol_fuel(0.0, 0, false, false).unwrap(),
        BattleVtolFuelUse::Skipped
    );
    assert_eq!(unit, before);
}
