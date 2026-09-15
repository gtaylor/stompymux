//! Mixed electronic emitters share field transitions, controls and persistent equipment loss.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
mod support;

/// Equip a vehicle with independent Guardian and Angel slots beside a hostile Mech and vehicle.
async fn fixture() -> (tempfile::TempDir, Config, World, [ObjectId; 3]) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Electronic field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        BattleMapAsset::parse("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for index in 0..3 {
        let id = world.create(&config, "Unit".into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        if index != 1 {
            let mut template =
                BattleVehicleTemplate::parse(include_str!("../game/mechs/Demolisher")).unwrap();
            for (slot, equipment) in [(0, "Ecm"), (1, "AngelEcm")] {
                template
                    .sections
                    .get_mut(&BattleVehicleSection::Front)
                    .unwrap()
                    .criticals
                    .insert(
                        slot,
                        CriticalDefinition {
                            equipment: equipment.into(),
                            data: "-".into(),
                            modes: vec![],
                            brand: None,
                        },
                    );
            }
            create_battle_vehicle(&mut world, id, template).unwrap();
        } else {
            create_battle_unit(
                &mut world,
                id,
                BattleTemplate::parse(include_str!("../game/mechs/AS7-D")).unwrap(),
            )
            .unwrap();
        }
        place_battle_unit(&mut world, id, map, index, 1).unwrap();
        if index != 1 {
            let pilot = ObjectId(if index == 0 { 1 } else { 2 });
            world.objects.get_mut(&pilot).unwrap().location = Some(id);
            assign_battle_pilot(&mut world, id, pilot).unwrap();
            start_battle_unit(&mut world, id, pilot, true).unwrap();
            for _ in 0..5 {
                advance_battle_units(&mut world, 0);
            }
        }
        ids.push(id);
    }
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["constructed"][ids[1].0.to_string()]["sensor_signature"]["team"] = 99.into();
    saved["vehicles"][ids[2].0.to_string()]["sensor_signature"]["team"] = 99.into();
    world.btech = serde_json::from_value(saved).unwrap();
    (dir, config, world, ids.try_into().unwrap())
}

#[tokio::test]
async fn vehicle_emitters_share_fields_countermeasures_and_restart() {
    for suite in [
        BattleElectronicSuite::Guardian,
        BattleElectronicSuite::Angel,
    ] {
        let (_dir, config, mut world, [emitter, mech, vehicle]) = fixture().await;
        toggle_battle_electronics(
            &mut world,
            emitter,
            ObjectId(1),
            suite,
            BattleElectronicMode::Ecm,
        )
        .unwrap();
        let notices = refresh_battle_electronic_fields(&mut world).unwrap();
        let suite_name = if suite == BattleElectronicSuite::Guardian {
            "ECM"
        } else {
            "AngelECM"
        };
        assert!(
            battle_unit_status(&world, emitter, "W")
                .unwrap()
                .contains(&format!("{suite_name}([fg=green bold]ECM[reset])"))
        );
        for receiver in [mech, vehicle] {
            let field = battle_electronic_field(&world, receiver).unwrap();
            assert!(field.blocks_outgoing_guidance());
            assert_eq!(field.angel_disturbed, suite == BattleElectronicSuite::Angel);
            assert!(
                notices
                    .iter()
                    .any(|notice| notice.unit == receiver && notice.text.contains("static"))
            );
        }
        assert!(
            refresh_battle_electronic_fields(&mut world)
                .unwrap()
                .is_empty()
        );
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(world.btech, restored.btech);
        for candidate in [&mut world, &mut restored] {
            toggle_battle_electronics(
                candidate,
                vehicle,
                ObjectId(2),
                BattleElectronicSuite::Angel,
                BattleElectronicMode::Eccm,
            )
            .unwrap();
            refresh_battle_electronic_fields(candidate).unwrap();
            assert!(
                battle_electronic_field(candidate, emitter)
                    .unwrap()
                    .countered
            );
            assert!(
                battle_unit_status(candidate, emitter, "W")
                    .unwrap()
                    .contains(&format!("{suite_name}([fg=red bold]ECM[reset])"))
            );
            for receiver in [mech, vehicle] {
                assert!(
                    !battle_electronic_field(candidate, receiver)
                        .unwrap()
                        .blocks_outgoing_guidance()
                );
            }
            destroy_battle_vehicle_critical(
                candidate,
                emitter,
                VehicleCriticalLocation {
                    section: BattleVehicleSection::Front,
                    slot: if suite == BattleElectronicSuite::Guardian {
                        0
                    } else {
                        1
                    },
                },
            )
            .unwrap();
            let state = candidate.btech.vehicles()[&emitter].electronics();
            assert!(
                battle_unit_status(candidate, emitter, "W")
                    .unwrap()
                    .contains(&format!("{suite_name}([fg=red bold]XX[reset])"))
            );
            assert_eq!(state.guardian, BattleElectronicMode::Off);
            assert_eq!(state.angel, BattleElectronicMode::Off);
            assert!(
                toggle_battle_electronics(
                    candidate,
                    emitter,
                    ObjectId(1),
                    suite,
                    BattleElectronicMode::Ecm
                )
                .is_err()
            );
            refresh_battle_electronic_fields(candidate).unwrap();
            candidate.validate(&config).unwrap();
        }
        assert_eq!(world.btech, restored.btech);
    }
}

#[tokio::test]
async fn vehicle_native_lua_electronics_and_failed_callback_are_identical() {
    let (_dir, config, world, [emitter, _, _]) = fixture().await;
    let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let native = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for (function, command) in [
        ("ecm", "ecm"),
        ("eccm", "eccm"),
        ("angelecm", "angelecm"),
        ("angeleccm", "angeleccm"),
        ("angeleccm", "angeleccm"),
    ] {
        lua.eval_callback::<()>(&format!("btech.unit.{function}({},1)", emitter.0))
            .unwrap();
        support::run_text(&native, &config, ObjectId(1), 1, command);
        assert_eq!(lua.world().btech, native.world().btech);
    }
    let before = lua.world().btech.clone();
    assert!(
        lua.eval_callback::<()>(&format!("btech.unit.ecm({},1); error('abort')", emitter.0))
            .is_err()
    );
    assert_eq!(lua.world().btech, before);
    let value: String = lua
        .eval_callback(&format!(
            "return btech.unit.state({}).electronics.guardian",
            emitter.0
        ))
        .unwrap();
    assert_eq!(value, "eccm");
}

#[tokio::test]
async fn vehicle_shutdown_clears_both_suites_and_rejects_invalid_saved_emissions() {
    let (_dir, config, mut world, [emitter, mech, vehicle]) = fixture().await;
    for suite in [
        BattleElectronicSuite::Guardian,
        BattleElectronicSuite::Angel,
    ] {
        toggle_battle_electronics(
            &mut world,
            emitter,
            ObjectId(1),
            suite,
            BattleElectronicMode::Ecm,
        )
        .unwrap();
    }
    refresh_battle_electronic_fields(&mut world).unwrap();
    stop_battle_unit(
        &mut world,
        emitter,
        ObjectId(1),
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    let state = world.btech.vehicles()[&emitter].electronics();
    assert_eq!(state.guardian, BattleElectronicMode::Off);
    assert_eq!(state.angel, BattleElectronicMode::Off);
    let notices = refresh_battle_electronic_fields(&mut world).unwrap();
    for receiver in [mech, vehicle] {
        assert!(
            !battle_electronic_field(&world, receiver)
                .unwrap()
                .blocks_outgoing_guidance()
        );
        assert!(notices.iter().any(|notice| notice.unit == receiver));
    }
    assert!(!battle_electronic_fields_pending(&world));
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][emitter.0.to_string()]["electronics"]["angel"] = "ecm".into();
    assert!(serde_json::from_value::<BtechState>(saved).is_err());
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}
