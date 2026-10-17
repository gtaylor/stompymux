//! The VTOL Combat Vehicle Critical Hits Table: rotor and hull columns, pilot and co-pilot
//! escalations, engine-damage landings and fuel-tank explosions.
use crate::support;
use stompymux_rs::*;

fn rules() -> VehicleCriticalRules {
    VehicleCriticalRules {
        extended_piloting: false,
        combat_safe: false,
        toughness: false,
    }
}

/// A seed whose first 2d6 is `roll`.
fn rolling(roll: u8) -> u8 {
    (0..=255)
        .find(|value| Dice::seeded([*value; 32]).two_d6() == roll)
        .unwrap()
}

/// Install an unplaced or airborne Kestrel directly for the critical transaction.
fn install(world: &mut World, id: ObjectId, unit: &Vehicle, seed: u8, airborne: Option<ObjectId>) {
    let mut saved = serde_json::to_value(unit).unwrap();
    saved["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
    if let Some(map) = airborne {
        saved["position"] = serde_json::json!({"map":map.0,"x":0,"y":0});
        saved["map_slot"] = 0.into();
        saved["power"] = serde_json::to_value(Power::Running).unwrap();
        saved["motion"] =
            serde_json::to_value(Motion::stationary(HexCoordinate { x: 0, y: 0 }.center()))
                .unwrap();
        saved["vtol_flight"] = serde_json::to_value(VtolFlight {
            fall: None,
            phase: VtolFlightPhase::Airborne,
            vertical_speed: 60.0,
            altitude: 10.0,
        })
        .unwrap();
    }
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["vehicles"][id.0.to_string()] = saved;
    world.btech = serde_json::from_value(state).unwrap();
}

/// Run a falling aircraft's descent clock to its impact, which must come without recovery.
fn crash(world: &mut World, id: ObjectId) {
    for _ in 0..100 {
        match advance_battle_vtol_fall(world, id, MovementRules::STANDARD.fall).unwrap() {
            VehicleDescentEvent::Impact { .. } => return,
            VehicleDescentEvent::Recovered => panic!("an aircraft that cannot fly recovered"),
            VehicleDescentEvent::Waiting | VehicleDescentEvent::Descending => {}
        }
    }
    panic!("the descent never reached the ground");
}

/// A VTOL wrecked by its crash: on the ground, with its rotor smashed (Total Warfare p. 68).
fn assert_wrecked(world: &World, id: ObjectId) {
    let unit = &world.btech.vehicles()[&id];
    assert_eq!(unit.vtol_flight().unwrap().phase, VtolFlightPhase::Landed);
    assert!(unit.rotor_destroyed());
    assert!(unit.is_destroyed());
}

fn kestrel() -> Vehicle {
    Vehicle::new(
        VehicleTemplate::parse("Kestrel", include_str!("../game/units/Kestrel.toml")).unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn rotor_criticals_commit_saved_dice_and_material_effects_together() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Rotor field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "rotor",
        MapAsset::from_cells("1 1\n.3\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Rotor critical fixture".into(), Kind::Thing);
    let aircraft = kestrel();
    for roll in 2..=12 {
        install(&mut world, id, &aircraft, rolling(roll), Some(map));
        let mut replay = world.clone();
        let report =
            resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Rotor, rules())
                .unwrap();
        assert_eq!(report.selection.rolls, [roll]);
        let expected = match roll {
            2..=5 => None,
            6..=8 => Some(RotorHit::Damage),
            9 | 10 => Some(RotorHit::FlightStabilizer),
            _ => Some(RotorHit::Destroy),
        };
        assert_eq!(
            report.selection.effect,
            expected.map(VehicleCriticalEffect::Rotor)
        );
        assert_eq!(
            resolve_battle_vehicle_critical(&mut replay, id, VehicleSection::Rotor, rules())
                .unwrap(),
            report
        );
        assert_eq!(world.btech, replay.btech);
        let unit = &world.btech.vehicles()[&id];
        assert_eq!(
            unit.maximum_speed(),
            match roll {
                11 | 12 => 0.0,
                6..=8 => 182.75,
                _ => 193.5,
            }
        );
        // Flight Stabilizer Hit: cruising speed only, +3 driving, +1 to-hit (TW p. 198).
        assert_eq!(unit.tail_rotor_destroyed(), matches!(roll, 9 | 10));
        assert_eq!(
            unit.piloting_damage(),
            if matches!(roll, 9 | 10) { 3 } else { 0 }
        );
        assert_eq!(unit.gunnery_damage(), u8::from(matches!(roll, 9 | 10)));
        assert_eq!(unit.rotor_destroyed(), roll >= 11);
        // Rotors Destroyed plays out as a crash: the VTOL falls and dies on impact.
        assert!(!unit.is_destroyed() && !unit.crew_killed());
        assert_eq!(unit.vtol_flight().unwrap().altitude, 10.0);
        assert_eq!(
            unit.vtol_flight().unwrap().phase,
            if roll >= 11 {
                VtolFlightPhase::Falling
            } else {
                VtolFlightPhase::Airborne
            }
        );
        assert_eq!(
            serde_json::from_value::<Vehicle>(serde_json::to_value(unit).unwrap()).unwrap(),
            *unit
        );
        if roll >= 11 {
            let before = world.btech.clone();
            assert!(
                resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Rotor, rules())
                    .is_err()
            );
            assert_eq!(world.btech, before);
        }
    }
    // A second Flight Stabilizer Hit moves down the column to Rotors Destroyed.
    install(&mut world, id, &aircraft, rolling(9), Some(map));
    let _ =
        resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Rotor, rules()).unwrap();
    world
        .btech
        .set_unit_dice(id, Dice::seeded([rolling(9); 32]))
        .unwrap();
    let report =
        resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Rotor, rules()).unwrap();
    assert_eq!(
        report.selection.effect,
        Some(VehicleCriticalEffect::Rotor(RotorHit::Destroy))
    );
}

#[tokio::test]
async fn rotor_damage_that_leaves_no_movement_forces_an_engine_damage_landing() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Landing field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "land",
        MapAsset::from_cells("1 1\n.3\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Slow rotor".into(), Kind::Thing);
    let slow = Vehicle::new(
        VehicleTemplate::parse(
            "Kestrel",
            &crate::support::templates::derived_engine_rating(include_str!(
                "../game/units/Kestrel.toml"
            ))
            .replace("walk_mp = 12", "walk_mp = 1"),
        )
        .unwrap(),
    )
    .unwrap();
    install(&mut world, id, &slow, 0, Some(map));
    let target = battle_unit_piloting_target(&world, id, false).unwrap() + 4;
    for success in [false, true] {
        let seed = (0..=255)
            .find(|seed| {
                let mut dice = Dice::seeded([*seed; 32]);
                dice.two_d6() == 7 && (i16::from(dice.two_d6()) >= target) == success
            })
            .unwrap();
        install(&mut world, id, &slow, seed, Some(map));
        world
            .btech
            .rewrite_unit_record(id, |record| {
                record["vtol_flight"]["vertical_speed"] = 0.0.into();
            })
            .unwrap();
        let report =
            resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Rotor, rules())
                .unwrap();
        assert_eq!(
            report.selection.effect,
            Some(VehicleCriticalEffect::Rotor(RotorHit::Damage))
        );
        let check = report.emergency_landing.as_ref().unwrap();
        assert_eq!(check.situational, 4, "engine-damage landings roll at +4");
        assert_eq!(check.success, success);
        let unit = &world.btech.vehicles()[&id];
        assert_eq!(unit.maximum_speed(), 0.0);
        assert!(!unit.rotor_destroyed());
        assert!(!unit.engine_destroyed(), "the engine itself keeps working");
        assert_eq!(
            unit.vtol_flight().unwrap().phase,
            if success {
                VtolFlightPhase::Landed
            } else {
                VtolFlightPhase::Falling
            }
        );
        // Treated as Engine Damage (TW p. 196), a failed landing destroys the VTOL: with no
        // movement left it cannot brake its fall even though the engine still runs.
        if !success {
            crash(&mut world, id);
            assert_wrecked(&world, id);
        }
    }
}

#[tokio::test]
async fn suppressed_rotor_criticals_do_not_draw_dice_or_change_material() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Protected rotor".into(), Kind::Thing);
    for suppression in ["safe", "proof"] {
        let source = if suppression == "proof" {
            include_str!("../game/units/Kestrel.toml").replace(
                "walk_mp = 12\n",
                "walk_mp = 12\nspecials = [\"CritProof_Tech\"]\n",
            )
        } else {
            include_str!("../game/units/Kestrel.toml").into()
        };
        let aircraft = Vehicle::new(VehicleTemplate::parse("Kestrel", &source).unwrap()).unwrap();
        install(&mut world, id, &aircraft, 0, None);
        let before = world.btech.clone();
        let report = resolve_battle_vehicle_critical(
            &mut world,
            id,
            VehicleSection::Rotor,
            VehicleCriticalRules {
                combat_safe: suppression == "safe",
                ..rules()
            },
        )
        .unwrap();
        assert!(report.selection.rolls.is_empty());
        assert!(report.selection.effect.is_none());
        assert_eq!(world.btech, before);
    }
}

#[tokio::test]
async fn stationary_observation_aircraft_still_use_the_rotor_critical_table() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Observation rotor".into(), Kind::Thing);
    let unit = Vehicle::new(
        VehicleTemplate::parse(
            "ObservationVTOL",
            include_str!("../game/units/ObservationVTOL.toml"),
        )
        .unwrap(),
    )
    .unwrap();
    install(&mut world, id, &unit, rolling(9), None);
    let report =
        resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Rotor, rules()).unwrap();
    assert_eq!(
        report.selection.effect,
        Some(VehicleCriticalEffect::Rotor(RotorHit::FlightStabilizer))
    );
    assert!(world.btech.vehicles()[&id].tail_rotor_destroyed());
}

#[tokio::test]
async fn hull_columns_move_down_and_apply_pilot_and_copilot_effects() {
    use VehicleCriticalEffect as E;
    use VehicleSection as S;
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Hull critical fixture".into(), Kind::Thing);
    // A pristine Kestrel: machine guns and ammunition in the front only, ICE engine.
    let rows = [
        (
            S::Front,
            [
                E::VtolCopilot,
                E::WeaponJam,
                E::Stabilizer,
                E::Sensors,
                E::VtolPilot,
                E::WeaponDestroyed,
                E::CrewKilled,
            ],
        ),
        (
            S::Left,
            [
                E::Stabilizer,
                E::Stabilizer,
                E::Stabilizer,
                E::Engine,
                E::Engine,
                E::Ammunition,
                E::FuelTank,
            ],
        ),
        (
            S::Right,
            [
                E::Stabilizer,
                E::Stabilizer,
                E::Stabilizer,
                E::Engine,
                E::Engine,
                E::Ammunition,
                E::FuelTank,
            ],
        ),
        (
            S::Rear,
            [
                E::Stabilizer,
                E::Stabilizer,
                E::Stabilizer,
                E::Sensors,
                E::Sensors,
                E::Engine,
                E::FuelTank,
            ],
        ),
    ];
    let unit = kestrel();
    for (section, row) in rows {
        for roll in 2..=12 {
            install(&mut world, id, &unit, rolling(roll), None);
            let base = world.clone();
            let report = roll_battle_vehicle_critical(&mut world, id, section, rules()).unwrap();
            assert_eq!(report.rolls, [roll]);
            assert_eq!(
                report.effect,
                if roll < 6 {
                    None
                } else {
                    Some(row[usize::from(roll - 6)])
                },
                "{section:?}/{roll}"
            );
            if matches!(report.effect, Some(E::VtolPilot | E::VtolCopilot)) {
                let mut resolved = base.clone();
                let mut replay = base;
                let result =
                    resolve_battle_vehicle_critical(&mut resolved, id, section, rules()).unwrap();
                assert_eq!(result.selection, report);
                assert_eq!(
                    resolve_battle_vehicle_critical(&mut replay, id, section, rules()).unwrap(),
                    result
                );
                assert_eq!(replay.btech, resolved.btech);
                let changed = &resolved.btech.vehicles()[&id];
                assert_eq!(changed.piloting_damage(), if roll == 10 { 2 } else { 0 });
                assert_eq!(changed.gunnery_damage(), if roll == 6 { 1 } else { 0 });
                assert_eq!(changed.pilot_injuries(), 0);
                assert_eq!(changed.crew_stun_remaining(), 0);
                // Landed aircraft make no control roll for a Pilot Hit.
                assert!(result.control_check.is_none());
                // A second hit on the same crew member kills the crew (TW p. 197).
                resolved
                    .btech
                    .set_unit_dice(id, Dice::seeded([rolling(roll); 32]))
                    .unwrap();
                let repeat =
                    resolve_battle_vehicle_critical(&mut resolved, id, section, rules()).unwrap();
                assert_eq!(repeat.selection.effect, report.effect);
                assert!(repeat.unit_destroyed);
                assert!(resolved.btech.vehicles()[&id].crew_killed());
            }
        }
    }
}

#[tokio::test]
async fn airborne_pilot_hits_roll_for_control_and_drop_a_level_on_failure() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Pilot field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "pilot",
        MapAsset::from_cells("1 1\n.3\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Pilot hit".into(), Kind::Thing);
    let unit = kestrel();
    for success in [false, true] {
        let seed = (0..=255)
            .find(|seed| {
                let mut dice = Dice::seeded([*seed; 32]);
                dice.two_d6() == 10 && (dice.two_d6() >= 6) == success
            })
            .unwrap();
        install(&mut world, id, &unit, seed, Some(map));
        let report =
            resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Front, rules())
                .unwrap();
        assert_eq!(
            report.selection.effect,
            Some(VehicleCriticalEffect::VtolPilot)
        );
        let check = report.control_check.as_ref().unwrap();
        assert_eq!(check.situational, 0);
        assert_eq!(check.success, success);
        let changed = &world.btech.vehicles()[&id];
        assert_eq!(changed.piloting_damage(), 2);
        assert_eq!(
            changed.vtol_flight().unwrap().altitude,
            if success { 10.0 } else { 9.0 }
        );
        assert_eq!(
            changed.vtol_flight().unwrap().phase,
            VtolFlightPhase::Airborne
        );
    }
    // One level above the ground, a failed roll is a crash.
    let seed = (0..=255)
        .find(|seed| {
            let mut dice = Dice::seeded([*seed; 32]);
            dice.two_d6() == 10 && dice.two_d6() < 6
        })
        .unwrap();
    install(&mut world, id, &unit, seed, Some(map));
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["vtol_flight"]["altitude"] = 4.0.into();
        })
        .unwrap();
    let _ =
        resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Front, rules()).unwrap();
    assert_eq!(
        world.btech.vehicles()[&id].vtol_flight().unwrap().phase,
        VtolFlightPhase::Falling
    );
    // The drop puts the VTOL on the ground (TW p. 197): it crashes rather than recovering in
    // the air. A hovering, still-flyable VTOL over clear ground has then landed (TW p. 68).
    crash(&mut world, id);
    let unit = &world.btech.vehicles()[&id];
    assert_eq!(unit.vtol_flight().unwrap().phase, VtolFlightPhase::Landed);
    assert_eq!(unit.vtol_flight().unwrap().altitude, 3.0);
    assert!(!unit.is_destroyed());
}

#[tokio::test]
async fn airborne_engine_critical_without_a_map_leaves_no_partial_changes() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Airborne engine".into(), Kind::Thing);
    install(
        &mut world,
        id,
        &kestrel(),
        rolling(10),
        Some(ObjectId(999_999)),
    );
    let before = world.btech.clone();
    assert!(
        resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Left, rules()).is_err()
    );
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn fuel_tank_explosions_settle_at_the_surface_and_destroy_the_vtol() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Explosion field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "explosion",
        MapAsset::from_cells("1 1\n.3\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let id = world.create(&config, "Exploding aircraft".into(), Kind::Thing);
    for case in [false, true] {
        let source = if case {
            // CASE takes the half ton the troop compartment would otherwise hold.
            include_str!("../game/units/Kestrel.toml")
                .replace("[transport_bays]\ninfantry_compartment = 3.5\n\n", "")
                .replace(
                    "[sections.aft_side]\n",
                    "[sections.aft_side]\nslots = [{ at = 1, item = \"CASE\" }]\n",
                )
        } else {
            include_str!("../game/units/Kestrel.toml").into()
        };
        let unit = Vehicle::new(VehicleTemplate::parse("Kestrel", &source).unwrap()).unwrap();
        assert_eq!(unit.has_powerplant_containment(), case);
        install(&mut world, id, &unit, rolling(12), Some(map));
        let mut replay = world.clone();
        let report =
            resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Left, rules()).unwrap();
        assert_eq!(
            report.selection.effect,
            Some(VehicleCriticalEffect::FuelTank)
        );
        // CASE does not contain a fuel-tank explosion (TW p. 197).
        assert!(!report.explosion.as_ref().unwrap().contained);
        let unit = &world.btech.vehicles()[&id];
        assert!(unit.is_destroyed());
        assert_eq!(unit.vtol_flight().unwrap().altitude, 3.0);
        assert_eq!(unit.vtol_flight().unwrap().vertical_speed, 0.0);
        assert_eq!(
            resolve_battle_vehicle_critical(&mut replay, id, VehicleSection::Left, rules())
                .unwrap(),
            report
        );
        assert_eq!(replay.btech, world.btech);
        assert_eq!(
            serde_json::from_value::<Vehicle>(serde_json::to_value(unit).unwrap()).unwrap(),
            *unit
        );
    }
    install(
        &mut world,
        id,
        &kestrel(),
        rolling(12),
        Some(ObjectId(999_999)),
    );
    let before = world.btech.clone();
    assert!(
        resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Left, rules()).is_err()
    );
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn engine_emergency_landings_roll_at_plus_four_and_commit_failed_attempts_as_falls() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Emergency field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "emergency",
        MapAsset::from_cells("1 1\n.3\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let id = world.create(&config, "Emergency aircraft".into(), Kind::Thing);
    let unit = kestrel();
    for success in [false, true] {
        let seed = (0..=255)
            .find(|seed| {
                let mut dice = Dice::seeded([*seed; 32]);
                dice.two_d6() == 10 && (dice.two_d6() >= 10) == success
            })
            .unwrap();
        install(&mut world, id, &unit, seed, Some(map));
        world
            .btech
            .rewrite_unit_record(id, |record| {
                record["vtol_flight"]["altitude"] = 5.0.into();
            })
            .unwrap();
        let before = world.btech.clone();
        let mut replay = world.clone();
        let rules = VehicleCriticalRules {
            extended_piloting: true,
            ..rules()
        };
        let report =
            resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Left, rules).unwrap();
        assert_eq!(report.selection.effect, Some(VehicleCriticalEffect::Engine));
        let check = report.emergency_landing.as_ref().unwrap();
        assert_eq!(check.success, success);
        assert_eq!(check.skill, 6);
        assert_eq!(check.situational, 4);
        assert_eq!(check.target, 10);
        assert_ne!(world.btech, before);
        assert_eq!(
            resolve_battle_vehicle_critical(&mut replay, id, VehicleSection::Left, rules).unwrap(),
            report
        );
        assert_eq!(replay.btech, world.btech);
        let changed = &world.btech.vehicles()[&id];
        assert_eq!(changed.maximum_speed(), 0.0);
        assert!(changed.engine_destroyed());
        if !success {
            assert_eq!(
                changed.vtol_flight().unwrap().phase,
                VtolFlightPhase::Falling
            );
            assert_eq!(changed.vtol_flight().unwrap().altitude, 5.0);
            assert!(changed.vtol_flight().unwrap().fall.is_some());
            // A failed landing roll destroys the VTOL (TW p. 197): its dead engine gives no
            // lift, so the fall runs unbraked into a wrecking crash.
            let mut crashing = world.clone();
            crash(&mut crashing, id);
            assert_wrecked(&crashing, id);
            // A second Engine Hit no longer applies; the column moves on to Ammunition.
            world
                .btech
                .set_unit_dice(id, Dice::seeded([seed; 32]))
                .unwrap();
            let repeated =
                resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Left, rules)
                    .unwrap();
            assert_eq!(
                repeated.selection.effect,
                Some(VehicleCriticalEffect::Ammunition)
            );
            assert!(repeated.emergency_landing.is_none());
            continue;
        }
        assert_eq!(
            changed.vtol_flight().unwrap(),
            VtolFlight {
                fall: None,
                phase: VtolFlightPhase::Landed,
                altitude: 3.0,
                vertical_speed: 0.0
            }
        );
        assert_eq!(changed.sections(), unit.sections());
        assert!(!changed.is_destroyed());
        assert_eq!(
            serde_json::from_value::<Vehicle>(serde_json::to_value(changed).unwrap()).unwrap(),
            *changed
        );
    }
}

#[tokio::test]
async fn engine_loss_over_water_starts_falling_without_a_landing_roll() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Water emergency".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "water",
        MapAsset::from_cells("1 1\n~3\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let id = world.create(&config, "Aircraft".into(), Kind::Thing);
    install(&mut world, id, &kestrel(), rolling(10), Some(map));
    let report =
        resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Left, rules()).unwrap();
    assert_eq!(report.selection.effect, Some(VehicleCriticalEffect::Engine));
    assert!(report.emergency_landing.is_none());
    let unit = &world.btech.vehicles()[&id];
    assert_eq!(unit.vtol_flight().unwrap().phase, VtolFlightPhase::Falling);
    let mut dice = Dice::seeded([rolling(10); 32]);
    dice.two_d6();
    assert_eq!(
        serde_json::to_value(unit).unwrap()["dice"],
        serde_json::to_value(dice).unwrap()
    );
    // Engine damage over terrain it cannot land on destroys the VTOL (TW p. 197).
    crash(&mut world, id);
    assert_wrecked(&world, id);
}

/// Emergency landing rolls reach only the assigned pilot before the landing outcome.
#[tokio::test]
async fn emergency_landing_feedback_is_private_and_replayable() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Emergency landing".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let id = world.create(&config, "Aircraft".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        VehicleTemplate::parse("Kestrel", include_str!("../game/units/Kestrel.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    let passenger = world.create(&config, "Passenger".into(), Kind::Player);
    world.objects.get_mut(&passenger).unwrap().location = Some(id);
    world
        .objects
        .get_mut(&passenger)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    set_battle_character(
        &mut world,
        ObjectId(1),
        Character {
            build: 5,
            reflexes: 5,
            intuition: 5,
            learn: 5,
            charisma: 5,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Piloting-Aerospace",
        CharacterValue {
            value: 2,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    let target = battle_unit_piloting_target(&world, id, true).unwrap() + 4;
    for success in [false, true] {
        let seed = (0..=255)
            .find(|seed| {
                let mut dice = Dice::seeded([*seed; 32]);
                dice.two_d6() == 10 && (i16::from(dice.two_d6()) >= target) == success
            })
            .unwrap();
        let mut candidate = world.clone();
        candidate
            .btech
            .rewrite_unit_record(id, |record| {
                let aircraft = record;
                aircraft["power"] = serde_json::to_value(Power::Running).unwrap();
                aircraft["vtol_flight"] = serde_json::to_value(VtolFlight {
                    fall: None,
                    phase: VtolFlightPhase::Airborne,
                    altitude: 2.0,
                    vertical_speed: 0.0,
                })
                .unwrap();
                aircraft["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
            })
            .unwrap();
        persistence::save(&config.database(), &candidate)
            .await
            .unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        for who in [ObjectId(1), passenger] {
            restored
                .objects
                .get_mut(&who)
                .unwrap()
                .flags
                .insert(Flag::Connected);
        }
        let scripts = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(candidate)),
        )
        .unwrap();
        let replay =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
        let rules = VehicleCriticalRules {
            extended_piloting: true,
            ..rules()
        };
        let report = resolve_battle_vehicle_critical_action(
            &scripts,
            &config,
            id,
            VehicleSection::Left,
            rules,
        )
        .unwrap();
        assert_eq!(report.emergency_landing.as_ref().unwrap().success, success);
        assert_eq!(report.pilot_notices.len(), 2);
        let output = scripts.drain_outbox();
        let messages: Vec<_> = output
            .iter()
            .filter(|(who, _)| *who == ObjectId(1))
            .map(|(_, message)| message.source())
            .collect();
        let index = messages
            .iter()
            .position(|message| *message == "You make a piloting skill roll!")
            .unwrap();
        assert_eq!(messages[index - 1], "Your engine takes a direct hit!");
        assert!(messages[index + 1].starts_with("Modified Pilot Skill:"));
        assert_eq!(
            messages[index + 2],
            if success {
                "You land safely!"
            } else {
                "You lose lift and start to fall!"
            }
        );
        assert!(output.iter().any(|(who, _)| *who == passenger));
        assert!(!output.iter().any(|(who, message)| *who == passenger
            && (message.source().starts_with("Modified Pilot Skill:")
                || message.source() == "You make a piloting skill roll!")));
        let repeated = resolve_battle_vehicle_critical_action(
            &replay,
            &config,
            id,
            VehicleSection::Left,
            rules,
        )
        .unwrap();
        assert_eq!(report, repeated);
        assert_eq!(scripts.world().btech, replay.world().btech);
        assert_eq!(output, replay.drain_outbox());
    }
}
