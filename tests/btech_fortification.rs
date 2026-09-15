//! Scenario fortification shares administration, movement admission and immobile aiming across chassis.
use stompymux_rs::*;
mod support;
const CHASSIS: [&str; 4] = [
    include_str!("fixtures/btech/mechs/JR7-D"),
    include_str!("../game/mechs/SCP-1N"),
    include_str!("../game/mechs/Demolisher"),
    include_str!("../game/mechs/Kestrel"),
];

/// A stationary unit and its assigned running pilot, ready for shared control admission.
async fn fixture(source: &str) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Emplacement".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "emplacement",
        BattleMapAsset::parse("2 2\n.0.0\n.0.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Unit".into(), Kind::Thing);
    BattleUnitTemplate::parse(source)
        .unwrap()
        .create(&mut world, id)
        .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    (dir, config, world, id)
}

#[tokio::test]
async fn native_lua_admin_inspection_rollback_and_persistence_match_all_chassis() {
    for source in CHASSIS {
        let (_dir, config, world, id) = fixture(source).await;
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let lua = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let output = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("@btech unit-fortified #{}=on", id.0),
        );
        assert!(output.contains("fortification enabled"), "{output}");
        lua.eval_callback::<()>(&format!("btech.unit.fortified({},true)", id.0))
            .unwrap();
        assert_eq!(native.world().btech, lua.world().btech);
        assert!(
            lua.eval_callback::<bool>(&format!(
                "return btech.unit.fortified({}) and btech.unit.state({}).fortified",
                id.0, id.0
            ))
            .unwrap()
        );
        assert!(
            battle_unit_status(&lua.world(), id, "info")
                .unwrap()
                .contains("FORTIFIED")
        );
        assert!(
            support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("@btech inspect #{}", id.0)
            )
            .contains("Fortified: true")
        );
        lua.drain_outbox();
        let before = lua.world().btech.clone();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.fortified({},false); error('abort')",
                id.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, before);
        assert!(lua.drain_outbox().is_empty());
        let snapshot = lua.world().clone();
        persistence::save(&config.database(), &snapshot)
            .await
            .unwrap();
        let replay = persistence::load(&config.database()).await.unwrap();
        assert_eq!(snapshot.btech, replay.btech);
        assert!(battle_unit_fortified(&replay, id).unwrap());
        let other = lua
            .world_mut()
            .create(&config, "Guest".into(), Kind::Player);
        support::run_text(
            &lua,
            &config,
            other,
            2,
            &format!("@btech unit-fortified #{}=off", id.0),
        );
        assert!(battle_unit_fortified(&lua.world(), id).unwrap());
        let before = lua.world().btech.clone();
        support::run_text(
            &lua,
            &config,
            ObjectId(1),
            1,
            &format!("@btech unit-fortified #{}=maybe", id.0),
        );
        assert_eq!(lua.world().btech, before);
        lua.world().validate(&config).unwrap();
    }
}

#[tokio::test]
async fn movement_stance_and_takeoff_controls_share_the_fortification_gate() {
    for (index, source) in CHASSIS.into_iter().enumerate() {
        let (_dir, config, mut world, id) = fixture(source).await;
        set_battle_fortified(&mut world, id, true).unwrap();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let mut commands = vec!["speed 1", "heading 90", "enterbase"];
        match index {
            0 => commands.push("jump 0 1"),
            1 => commands.push("hulldown"),
            2 => commands.push("dig"),
            3 => commands.extend(["takeoff", "vertical 1"]),
            _ => unreachable!(),
        }
        for command in commands {
            let before = scripts.world().btech.clone();
            let output = support::run_text(&scripts, &config, ObjectId(1), 1, command);
            assert!(output.contains("fortified"), "{command}: {output}");
            assert_eq!(scripts.world().btech, before);
        }
        let before = scripts.world().btech.clone();
        assert!(
            scripts
                .eval_callback::<()>(&format!("btech.unit.heading({},1,90)", id.0))
                .unwrap_err()
                .to_string()
                .contains("fortified")
        );
        assert_eq!(scripts.world().btech, before);
        set_battle_fortified(&mut scripts.world_mut(), id, false).unwrap();
        if index != 3 {
            set_battle_heading(&mut scripts.world_mut(), id, ObjectId(1), 90.0).unwrap();
        }
    }
}

#[tokio::test]
async fn immobile_target_bonus_and_shutdown_preservation_are_shared() {
    for source in CHASSIS {
        let (_dir, config, mut world, id) = fixture(source).await;
        let before = battle_unit_target_movement_modifier(&world, id, 3.0, false).unwrap();
        let readiness = if let Some(unit) = world.btech.vehicles().get(&id) {
            unit.weapon_readiness(0).unwrap()
        } else {
            world.btech.constructed_units()[&id]
                .weapon_readiness(0)
                .unwrap()
        };
        set_battle_fortified(&mut world, id, true).unwrap();
        let fortified_readiness = if let Some(unit) = world.btech.vehicles().get(&id) {
            unit.weapon_readiness(0).unwrap()
        } else {
            world.btech.constructed_units()[&id]
                .weapon_readiness(0)
                .unwrap()
        };
        assert_eq!(readiness, fortified_readiness);
        assert_eq!(
            battle_unit_target_movement_modifier(&world, id, 3.0, false).unwrap(),
            before - 4
        );
        stop_battle_unit(
            &mut world,
            id,
            ObjectId(1),
            BattleMovementRules::STANDARD.fall,
        )
        .unwrap();
        assert!(battle_unit_fortified(&world, id).unwrap());
        assert_eq!(
            battle_unit_target_movement_modifier(&world, id, 3.0, false).unwrap(),
            before - 4
        );
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn tow_pairs_reject_either_fortified_endpoint_and_cannot_be_fortified_after_attachment() {
    for source in [CHASSIS[0], CHASSIS[2], CHASSIS[3]] {
        for target_source in [CHASSIS[0], CHASSIS[2], CHASSIS[3]] {
            let (_dir, config, mut world, id) = fixture(source).await;
            let map = world.objects[&id].location.unwrap();
            let target = world.create(&config, "Target".into(), Kind::Thing);
            BattleUnitTemplate::parse(target_source)
                .unwrap()
                .create(&mut world, target)
                .unwrap();
            place_battle_unit(&mut world, target, map, 0, 0).unwrap();
            for endpoint in [id, target] {
                set_battle_fortified(&mut world, endpoint, true).unwrap();
                let before = world.btech.clone();
                assert!(
                    set_battle_tow(&mut world, id, Some(target))
                        .unwrap_err()
                        .to_string()
                        .contains("fortified")
                );
                assert!(
                    battle_pickup_admission(&world, id, ObjectId(1), target)
                        .unwrap_err()
                        .to_string()
                        .contains("fortified")
                );
                assert_eq!(world.btech, before);
                set_battle_fortified(&mut world, endpoint, false).unwrap();
            }
            set_battle_tow(&mut world, id, Some(target)).unwrap();
            for endpoint in [id, target] {
                let before = world.btech.clone();
                assert!(set_battle_fortified(&mut world, endpoint, true).is_err());
                assert_eq!(world.btech, before);
            }
            world.validate(&config).unwrap();
            let mut saved = serde_json::to_value(&world.btech).unwrap();
            let class = if world.btech.vehicles().contains_key(&target) {
                "vehicles"
            } else {
                "constructed"
            };
            saved[class][target.0.to_string()]["fortified"] = serde_json::json!(true);
            world.btech = serde_json::from_value(saved).unwrap();
            assert!(world.validate(&config).is_err());
        }
    }
}

#[tokio::test]
async fn enabling_requires_settled_motion_and_a_landed_aircraft() {
    let (_dir, _, mut world, id) = fixture(CHASSIS[0]).await;
    set_battle_speed(&mut world, id, ObjectId(1), 1.0).unwrap();
    let before = world.btech.clone();
    assert!(set_battle_fortified(&mut world, id, true).is_err());
    assert_eq!(world.btech, before);
    set_battle_speed(&mut world, id, ObjectId(1), 0.0).unwrap();
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["constructed"][id.0.to_string()]["jump_stabilization"] = serde_json::json!(2);
    world.btech = serde_json::from_value(saved).unwrap();
    set_battle_fortified(&mut world, id, true).unwrap();
    for _ in 0..2 {
        advance_battle_jumps(&mut world, BattleMovementRules::STANDARD).unwrap();
    }
    assert_eq!(world.btech.constructed_units()[&id].jump_stabilization(), 0);
    let (_dir, _, mut world, id) = fixture(CHASSIS[3]).await;
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    // Use the material takeoff API to test its fortification gate without a second host implementation.
    set_battle_fortified(&mut world, id, true).unwrap();
    let mut vehicle = world.btech.vehicles()[&id].clone();
    assert!(
        vehicle
            .begin_vtol_takeoff(false, true, 0)
            .unwrap_err()
            .to_string()
            .contains("fortified")
    );
    set_battle_fortified(&mut world, id, false).unwrap();
    let mut vehicle = world.btech.vehicles()[&id].clone();
    vehicle.begin_vtol_takeoff(false, true, 0).unwrap();
    saved["vehicles"][id.0.to_string()] = serde_json::to_value(vehicle).unwrap();
    world.btech = serde_json::from_value(saved).unwrap();
    let before = world.btech.clone();
    assert!(set_battle_fortified(&mut world, id, true).is_err());
    assert_eq!(world.btech, before);
}
