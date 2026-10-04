//! Seasonal ice controls preserve shoreline rules, shared occupant effects and transactional replay.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Replace one base tile without disturbing units, overlays or the saved map stream.
fn tile(world: &mut World, map: ObjectId, x: usize, y: usize, terrain: &str, elevation: u8) {
    let mut state = serde_json::to_value(&world.btech).unwrap();
    let map = &mut state["maps"][map.0.to_string()];
    let width = map["width"].as_u64().unwrap() as usize;
    map["terrain"][y * width + x] = serde_json::to_value(stompymux_rs::BattleHex::new(
        Terrain::from_name(terrain).unwrap(),
        elevation,
    ))
    .unwrap();
    world.btech = serde_json::from_value(state).unwrap();
}

/// A separate operator avoids moving the assigned pilot out of the cockpit.
fn operator(world: &mut World, config: &Config, map: ObjectId) -> ObjectId {
    let id = world.create(config, "Ice operator".into(), Kind::Player);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::Wizard);
    world.objects.get_mut(&id).unwrap().location = Some(map);
    id
}

/// Occupied ice uses precisely the ordinary fracture action on every supported chassis.
#[tokio::test]
async fn melting_matches_combat_fractures_native_lua_and_restart() {
    for source in firing::templates() {
        let (_dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(&source, None, include_str!("../game/mechs/AS7-D.toml"))
                .await;
        let map = world.btech.units()[&unit].map.unwrap();
        let actor = operator(&mut world, &config, map);
        tile(&mut world, map, 0, 11, "ice", 2);
        firing::edit(&mut world, unit, |record| {
            record["ground_elevation"] = serde_json::Value::Null;
            if let Some(flight) = record
                .get_mut("vtol_flight")
                .filter(|value| !value.is_null())
            {
                flight["altitude"] = 0.into();
            }
        });
        world.validate(&config).unwrap();
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let ordinary = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let call = format!("btech.map.remove_ice({},{},100)", actor.0, map.0);
        let before = lua.world().btech.clone();
        assert!(
            lua.eval_callback::<()>(&format!("{call}; error('abort melt')"))
                .is_err()
        );
        assert_eq!(lua.world().btech, before);
        lua.drain_outbox();
        let report: mlua::Table = lua.eval_callback(&format!("return {call}")).unwrap();
        assert_eq!(report.get::<mlua::Table>("changed").unwrap().raw_len(), 1);
        let output = support::run_text(&native, &config, actor, 1, "delice 100");
        assert!(output.contains("1 hexes melted."), "{output}");
        assert_eq!(native.world().btech, lua.world().btech);
        let fracture = break_battle_surface_action(
            &ordinary,
            &config,
            map,
            BattleHexCoordinate { x: 0, y: 11 },
            BattleSurface::Ice,
            BattleFallRules::configured(&config),
        )
        .unwrap();
        assert_eq!(fracture.after.terrain(), Terrain::Water);
        assert_eq!(
            native.world().btech.constructed_units(),
            ordinary.world().btech.constructed_units()
        );
        assert_eq!(
            native.world().btech.vehicles(),
            ordinary.world().btech.vehicles()
        );
        let saved = native.world().clone();
        saved.validate(&config).unwrap();
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
    }
}

/// Growth preserves submerged altitude and a subsequent melt does not drop submerged occupants again.
#[tokio::test]
async fn freezing_water_preserves_submerged_mechs() {
    for source in firing::templates().into_iter().take(2) {
        let (_dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(&source, None, include_str!("../game/mechs/AS7-D.toml"))
                .await;
        let map = world.btech.units()[&unit].map.unwrap();
        let actor = operator(&mut world, &config, map);
        tile(&mut world, map, 0, 11, "water", 2);
        firing::edit(&mut world, unit, |record| {
            record["ground_elevation"] = (-2).into()
        });
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let before = scripts.world().btech.constructed_units()[&unit].clone();
        let report =
            change_battle_map_ice_action(&scripts, &config, actor, map, 100, BattleIceChange::Grow)
                .unwrap();
        assert_eq!(report.changed, vec![BattleHexCoordinate { x: 0, y: 11 }]);
        assert!(report.fractures.is_empty());
        assert_eq!(scripts.world().btech.constructed_units()[&unit], before);
        assert_eq!(
            scripts.world().btech.maps()[&map]
                .base_hex(0, 11)
                .unwrap()
                .terrain(),
            Terrain::Ice
        );
        let report =
            change_battle_map_ice_action(&scripts, &config, actor, map, 100, BattleIceChange::Melt)
                .unwrap();
        assert!(report.fractures[0].falls.is_empty());
        assert_eq!(scripts.world().btech.constructed_units()[&unit], before);
        scripts.world().validate(&config).unwrap();
    }
}

/// Percent thresholds still draw for eligible terrain, while invalid requests are state-free.
#[tokio::test]
async fn ice_thresholds_authority_and_rolls() {
    let (_dir, config, mut world, unit, _, _) = firing::fixture_with_target(
        &firing::templates()[0],
        None,
        include_str!("../game/mechs/AS7-D.toml"),
    )
    .await;
    let map = world.btech.units()[&unit].map.unwrap();
    let actor = operator(&mut world, &config, map);
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    tile(&mut world, map, 0, 5, "water", 3);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for percentage in [i32::MIN, -1, 0] {
        let before = serde_json::to_value(&scripts.world().btech).unwrap();
        let mut expected: BattleDice =
            serde_json::from_value(before["maps"][map.0.to_string()]["fire_dice"].clone()).unwrap();
        expected.die(100).unwrap();
        let report = change_battle_map_ice_action(
            &scripts,
            &config,
            actor,
            map,
            percentage,
            BattleIceChange::Grow,
        )
        .unwrap();
        assert!(report.changed.is_empty());
        let after = serde_json::to_value(&scripts.world().btech).unwrap();
        assert_eq!(
            after["maps"][map.0.to_string()]["fire_dice"],
            serde_json::to_value(expected).unwrap()
        );
        assert_eq!(
            after["maps"][map.0.to_string()]["terrain"],
            before["maps"][map.0.to_string()]["terrain"]
        );
    }
    let before = scripts.world().btech.clone();
    for command in [
        "addice",
        "addice 1 2",
        "delice nope",
        "addice 2147483648",
        "addice/test 100",
    ] {
        support::run_text(&scripts, &config, actor, 1, command);
        assert_eq!(scripts.world().btech, before, "{command}");
    }
    assert!(
        change_battle_map_ice_action(
            &scripts,
            &config,
            ObjectId(2),
            map,
            100,
            BattleIceChange::Grow
        )
        .is_err()
    );
    assert!(
        change_battle_map_ice_action(&scripts, &config, actor, unit, 100, BattleIceChange::Grow)
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    let call = format!("btech.map.add_ice({},{},{})", actor.0, map.0, i32::MAX);
    assert!(
        scripts
            .eval_callback::<()>(&format!("{call}; error('abort freeze')"))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    let output = support::run_text(&scripts, &config, actor, 1, "addice 2147483647");
    assert!(output.contains("1 hexes 'iced'."), "{output}");
}

/// A pass cannot grow inward from newly frozen edges; melted edges immediately affect later cells.
#[tokio::test]
async fn shoreline_passes_have_distinct_growth_and_melt_ordering() {
    let (_dir, config, mut world, unit, _, _) = firing::fixture_with_target(
        &firing::templates()[0],
        None,
        include_str!("../game/mechs/AS7-D.toml"),
    )
    .await;
    let map = world.btech.units()[&unit].map.unwrap();
    let actor = operator(&mut world, &config, map);
    let mut state = serde_json::to_value(&world.btech).unwrap();
    let seed = (0..=255)
        .find(|seed| {
            let mut dice = BattleDice::seeded([*seed; 32]);
            dice.die(100).unwrap();
            dice.d6() == 6
        })
        .unwrap();
    state["maps"][map.0.to_string()]["fire_dice"] =
        serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    state["maps"][map.0.to_string()]["width"] = 3.into();
    state["maps"][map.0.to_string()]["terrain"] = serde_json::json!(vec![
        serde_json::to_value(
            stompymux_rs::BattleHex::new(stompymux_rs::Terrain::Grassland, 0)
        )
        .unwrap();
        36
    ]);
    world.btech = serde_json::from_value(state).unwrap();
    let center = BattleHexCoordinate { x: 1, y: 5 };
    let circle: Vec<_> = std::iter::once(center)
        .chain(center.neighbors().unwrap())
        .collect();
    for hex in &circle {
        tile(&mut world, map, hex.x as usize, hex.y as usize, "water", 2);
    }
    let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    lua.eval_callback::<mlua::Table>(&format!(
        "return btech.map.add_ice({},{},100)",
        actor.0, map.0
    ))
    .unwrap();
    support::run_text(&native, &config, actor, 1, "addice 100");
    assert_eq!(native.world().btech, lua.world().btech);
    assert_eq!(
        native.world().btech.maps()[&map]
            .base_hex(1, 5)
            .unwrap()
            .terrain(),
        Terrain::Water
    );
    assert!(circle.iter().any(|hex| {
        native.world().btech.maps()[&map]
            .base_hex(i64::from(hex.x), i64::from(hex.y))
            .unwrap()
            .terrain()
            == Terrain::Ice
    }));
    for hex in &circle {
        tile(&mut world, map, hex.x as usize, hex.y as usize, "ice", 2);
    }
    let encoded = serde_json::to_value(&world.btech).unwrap();
    let mut expected: BattleDice =
        serde_json::from_value(encoded["maps"][map.0.to_string()]["fire_dice"].clone()).unwrap();
    // The earlier left-edge melts mean the center no longer needs the enclosed-ice die.
    for _ in 0..7 {
        expected.die(100).unwrap();
    }
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let report =
        change_battle_map_ice_action(&scripts, &config, actor, map, 100, BattleIceChange::Melt)
            .unwrap();
    assert_eq!(report.changed.len(), 7);
    assert!(
        report
            .changed
            .windows(2)
            .all(|pair| (pair[0].x, pair[0].y) < (pair[1].x, pair[1].y))
    );
    let encoded = serde_json::to_value(&scripts.world().btech).unwrap();
    assert_eq!(
        encoded["maps"][map.0.to_string()]["fire_dice"],
        serde_json::to_value(expected).unwrap()
    );
    scripts.world().validate(&config).unwrap();
}

/// Publication failure restores both changed terrain and the map's advanced dice.
#[tokio::test]
async fn failed_confirmation_restores_ice_and_dice() {
    let (dir, config, mut world, unit, _, _) = firing::fixture_with_target(
        &firing::templates()[0],
        None,
        include_str!("../game/mechs/AS7-D.toml"),
    )
    .await;
    let map = world.btech.units()[&unit].map.unwrap();
    let actor = operator(&mut world, &config, map);
    tile(&mut world, map, 0, 5, "water", 2);
    let config = limited_output_config(dir.path());
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    let error =
        change_battle_map_ice_action(&scripts, &config, actor, map, 100, BattleIceChange::Grow)
            .unwrap_err();
    assert!(error.to_string().contains("output limit"), "{error:#}");
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
}

/// Occupant publication failure restores the whole seasonal pass, including earlier unoccupied melts.
#[tokio::test]
async fn occupied_melting_commits_or_restores_the_whole_map_pass() {
    let (dir, config, mut world, unit, _, _) = firing::fixture_with_target(
        include_str!("../game/mechs/Demolisher.toml"),
        None,
        include_str!("../game/mechs/AS7-D.toml"),
    )
    .await;
    let map = world.btech.units()[&unit].map.unwrap();
    let actor = operator(&mut world, &config, map);
    release_battle_pilot(&mut world, unit, ObjectId(1)).unwrap();
    world
        .objects
        .get_mut(&unit)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let pilot = ObjectId(2);
    world
        .objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    world.objects.get_mut(&pilot).unwrap().location = Some(unit);
    set_battle_character(
        &mut world,
        pilot,
        BattleCharacter {
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
    support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
    assign_battle_pilot(&mut world, unit, pilot).unwrap();
    support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
    tile(&mut world, map, 0, 5, "ice", 2);
    tile(&mut world, map, 0, 11, "ice", 2);
    firing::edit(&mut world, unit, |state| {
        state["ground_elevation"] = serde_json::Value::Null
    });
    let baseline = world.clone();
    let limited = limited_output_config(dir.path());
    let scripts = Scripts::new(&limited, Rc::new(RefCell::new(world))).unwrap();
    assert!(
        change_battle_map_ice_action(&scripts, &config, actor, map, 100, BattleIceChange::Melt)
            .is_err()
    );
    assert_eq!(scripts.world().btech, baseline.btech);
    assert_eq!(scripts.world().objects[&pilot].location, Some(unit));
    assert!(scripts.drain_outbox().is_empty());
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(baseline))).unwrap();
    let report =
        change_battle_map_ice_action(&scripts, &config, actor, map, 100, BattleIceChange::Melt)
            .unwrap();
    assert_eq!(report.changed.len(), 2);
    assert!(scripts.world().btech.vehicles()[&unit].is_destroyed());
    let destination = scripts.world().objects[&pilot].location.unwrap();
    assert_eq!(destination, unit);
    assert!(!scripts.world().btech.vehicles()[&unit].crew_killed());
    assert_ne!(destination, ObjectId(config.battletech.afterlife_dbref));
    let saved = scripts.world().clone();
    saved.validate(&config).unwrap();
    persistence::save(&config.database(), &saved).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, saved.btech);
    assert_eq!(loaded.objects[&pilot].location, Some(destination));
}

/// Restrict publication to exercise rollback after terrain mutation.
fn limited_output_config(directory: &std::path::Path) -> Config {
    let path = directory.join("stompymux.toml");
    let mut table: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
    table
        .entry("runtime")
        .or_insert(toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .unwrap()
        .insert("output_message_limit".into(), 1.into());
    std::fs::write(&path, toml::to_string(&table).unwrap()).unwrap();
    Config::load(directory).unwrap()
}
