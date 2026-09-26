//! SETCOND and Lua environmental updates drive existing live rules without copying unit state.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Native and Lua transitions retain unrelated map fields and exact unit state across all supported chassis.
#[tokio::test]
async fn environment_controls_share_flags_rollback_and_restart() {
    for source in firing::templates() {
        let (_dir, config, mut world, unit, _, _) = firing::fixture_with_target(
            &source,
            Some(BattleWeapon::MediumLaser),
            include_str!("../game/mechs/AS7-D"),
        )
        .await;
        let map = world.btech.units()[&unit].map.unwrap();
        let operator = world.create(&config, "Environment operator".into(), Kind::Player);
        world
            .objects
            .get_mut(&operator)
            .unwrap()
            .flags
            .insert(Flag::Wizard);
        world.objects.get_mut(&operator).unwrap().location = Some(map);
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        for (arguments, gravity, temperature, vacuum, underground, special) in [
            ("50 -40 1 1", 50, -40, true, true, true),
            ("100 20", 100, 20, false, true, false),
            ("255 127 0 0", 255, 127, false, true, true),
            ("100 -30", 100, -30, false, true, false),
            ("100 50", 100, 50, false, true, false),
            ("100 51", 100, 51, false, true, true),
            ("-0 +0 -0 +0", 0, 0, false, true, true),
            ("100 20 0 0 ignored arguments", 100, 20, false, true, false),
        ] {
            let before = lua.world().btech.clone();
            let call = format!(
                "btech.map.environment({},{},{{gravity={gravity},temperature={temperature},vacuum={},underground={}}})",
                operator.0,
                map.0,
                vacuum,
                arguments.ends_with("1 1")
            );
            assert!(
                lua.eval_callback::<()>(&format!("{call}; error('abort environment')"))
                    .is_err()
            );
            assert_eq!(lua.world().btech, before);
            lua.drain_outbox();
            let actual: mlua::Table = lua.eval_callback(&format!("return {call}")).unwrap();
            assert_eq!(actual.get::<bool>("underground").unwrap(), underground);
            let output = support::run_text(
                &native,
                &config,
                operator,
                1,
                &format!("setcond {arguments}"),
            );
            assert!(output.contains("Conditions set!"), "{output}");
            assert_eq!(native.world().btech, lua.world().btech);
            let world = lua.world();
            let record = &world.btech.maps()[&map];
            assert_eq!(
                record.environment(),
                BattleMapEnvironment {
                    gravity,
                    temperature,
                    vacuum,
                    underground
                }
            );
            assert_eq!(record.uses_special_rules(), special);
            assert_eq!(world.btech.constructed_units(), before.constructed_units());
            assert_eq!(world.btech.vehicles(), before.vehicles());
            world.validate(&config).unwrap();
        }
        let saved = lua.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
    }
}

/// All chassis read gravity live; Mech cooling reads temperature and underground blocks VTOL takeoff.
#[tokio::test]
async fn changed_environment_reaches_live_movement_heat_and_flight() {
    for source in firing::templates() {
        let (_dir, config, mut world, unit, _, _) = firing::fixture_with_target(
            &source,
            Some(BattleWeapon::MediumLaser),
            include_str!("../game/mechs/AS7-D"),
        )
        .await;
        let map = world.btech.units()[&unit].map.unwrap();
        let conditions = |gravity, temperature, underground| BattleMapEnvironment {
            gravity,
            temperature,
            vacuum: false,
            underground,
        };
        let original_speed = battle_effective_maximum_speed(&world, unit, false).unwrap();
        let cooling = world
            .btech
            .constructed_units()
            .get(&unit)
            .map(|unit| unit.heat_rates(&world).dissipation);
        set_battle_map_environment(&mut world, ObjectId(1), map, conditions(50, -40, false))
            .unwrap();
        if let Some(cooling) = cooling {
            assert_eq!(
                world.btech.constructed_units()[&unit]
                    .heat_rates(&world)
                    .dissipation,
                cooling + 1.0
            );
            assert!(
                (battle_effective_maximum_speed(&world, unit, false).unwrap()
                    - original_speed * 2.0)
                    .abs()
                    < 0.001
            );
            firing::edit(&mut world, unit, |state| {
                state["heat"] = serde_json::json!({"stored":20.0,"excess":0.0})
            });
            let mut hot = world.clone();
            set_battle_map_environment(&mut hot, ObjectId(1), map, conditions(100, 60, false))
                .unwrap();
            advance_battle_heat(&mut world);
            advance_battle_heat(&mut hot);
            assert!(
                world.btech.constructed_units()[&unit].heat().stored
                    < hot.btech.constructed_units()[&unit].heat().stored
            );
        } else {
            assert_eq!(
                battle_effective_maximum_speed(&world, unit, false).unwrap(),
                original_speed * 2.0
            );
        }
        set_battle_map_environment(&mut world, ObjectId(1), map, conditions(100, 20, true))
            .unwrap();
        if world
            .btech
            .vehicles()
            .get(&unit)
            .is_some_and(|unit| unit.definition().is_vtol())
        {
            let before = world.btech.clone();
            assert!(begin_battle_vtol_takeoff(&mut world, unit, ObjectId(1), 5, false).is_err());
            assert_eq!(world.btech, before);
        }
        world.validate(&config).unwrap();
    }
}

/// Invalid native/Lua arguments and unauthorized writers cannot partially change the shared map.
#[tokio::test]
async fn environment_validation_and_authority_are_atomic() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Weather map".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "weather",
        BattleMapAsset::parse("1 1\n.0\n272: 100 20\n").unwrap(),
    )
    .unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
    let ordinary = world.create(&config, "Ordinary".into(), Kind::Player);
    let before = world.btech.clone();
    assert!(
        set_battle_map_environment(
            &mut world,
            ordinary,
            map,
            BattleMapEnvironment {
                gravity: 10,
                temperature: 30,
                vacuum: true,
                underground: false
            }
        )
        .is_err()
    );
    assert_eq!(world.btech, before);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for (args, expected) in [
        ("", "(At least) 2 options required (gravity + temperature)"),
        (
            "100",
            "(At least) 2 options required (gravity + temperature)",
        ),
        (
            "256 20",
            "Invalid gravity (must be integer in range of 0 to 255)",
        ),
        (
            "2147483648 20",
            "Invalid gravity (must be integer in range of 0 to 255)",
        ),
        (
            "100.5 20",
            "Invalid gravity (must be integer in range of 0 to 255)",
        ),
        (
            "100 -129",
            "Invalid temperature (must be integer in range of -128 to 127",
        ),
        (
            "100 nope",
            "Invalid temperature (must be integer in range of -128 to 127",
        ),
        ("100 20 2", "Invalid vacuum flag (must be integer, 0 or 1)"),
        (
            "100 20 nope",
            "Invalid vacuum flag (must be integer, 0 or 1)",
        ),
        (
            "100 20 0 2",
            "Invalid underground flag (must be integer, 0 or 1)",
        ),
        (
            "100 20 0 nope",
            "Invalid underground flag (must be integer, 0 or 1)",
        ),
        (
            "bad bad bad bad",
            "Invalid gravity (must be integer in range of 0 to 255)",
        ),
    ] {
        let reply = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("setcond {args}"),
        );
        assert_eq!(reply, expected, "{args}");
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
    }
    for fields in [
        "gravity=256,temperature=20",
        "gravity=100,temperature=-129",
        "gravity=100,temperature=20,vacuum=1",
        "gravity=100,temperature=20,cloud_base=100",
    ] {
        assert!(
            scripts
                .eval_callback::<mlua::Table>(&format!(
                    "return btech.map.environment(1,{},{{{fields}}})",
                    map.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
    }
    let output = support::run_text(&scripts, &config, ObjectId(1), 1, "setcond/invalid 100 20");
    assert!(output.contains("takes no switches"), "{output}");
    assert_eq!(scripts.world().btech, before);
    support::run_text(&scripts, &config, ObjectId(1), 1, "setcond 100 20 1 0");
    assert_eq!(scripts.world().btech.maps()[&map].flags, 272 | 2 | 4);
}

/// An airborne flight keeps its route, samples changed gravity on the next tick and replays after restart.
#[tokio::test]
async fn in_flight_gravity_changes_resume_without_resetting_the_route() {
    let (_dir, config, mut world, unit, _, _) = firing::fixture_with_target(
        include_str!("../game/mechs/JR7-D"),
        Some(BattleWeapon::MediumLaser),
        include_str!("../game/mechs/AS7-D"),
    )
    .await;
    let map = world.btech.units()[&unit].map.unwrap();
    launch_battle_jump(&mut world, unit, ObjectId(1), 0, 2.0).unwrap();
    let flight = world.btech.constructed_units()[&unit].flight().unwrap();
    let mut heavy = world.clone();
    for (candidate, gravity) in [(&mut world, 50), (&mut heavy, 200)] {
        set_battle_map_environment(
            candidate,
            ObjectId(1),
            map,
            BattleMapEnvironment {
                gravity,
                temperature: 20,
                vacuum: false,
                underground: false,
            },
        )
        .unwrap();
        assert_eq!(
            candidate.btech.constructed_units()[&unit].flight().unwrap(),
            flight
        );
        candidate.validate(&config).unwrap();
    }
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    for candidate in [&mut world, &mut heavy, &mut restored] {
        advance_battle_jumps(candidate, BattleMovementRules::STANDARD).unwrap();
        candidate.validate(&config).unwrap();
    }
    assert_eq!(world.btech, restored.btech);
    assert!(
        world.btech.constructed_units()[&unit]
            .flight()
            .unwrap()
            .travelled()
            > heavy.btech.constructed_units()[&unit]
                .flight()
                .unwrap()
                .travelled()
    );
}

/// A failure to publish confirmation restores the map in both native and Lua paths.
#[tokio::test]
async fn environment_confirmation_failure_restores_shared_state() {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Environment rollback".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "rollback",
        BattleMapAsset::parse("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
    let path = dir.path().join("stompymux.toml");
    let mut table: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
    table
        .entry("runtime")
        .or_insert(toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .unwrap()
        .insert("output_message_limit".into(), 1.into());
    std::fs::write(&path, toml::to_string(&table).unwrap()).unwrap();
    let config = Config::load(dir.path()).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    let action = commands::run(&scripts, &config, ObjectId(1), 1, "setcond 50 -40 1 1").unwrap();
    assert!(
        matches!(action, CommandAction::Report(CommandReport::Reply(message)) if message.contains("output limit"))
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    assert!(scripts.eval_callback::<mlua::Table>(&format!("return btech.map.environment(1,{},{{gravity=50,temperature=-40,vacuum=true,underground=true}})", map.0)).is_err());
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
}
