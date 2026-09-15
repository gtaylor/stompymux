//! Lookup-object allocation, row persistence and explicit rebuilds agree across map callers.
use sqlx::Connection;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
mod support;

/// A five-wide map exercises partially occupied packed bytes as well as independent rows.
async fn fixture() -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Lookup field".into(), Kind::Room);
    let interior = world.create(&config, "Workshop".into(), Kind::Room);
    for id in [map, interior] {
        create_battle_map(
            &mut world,
            id,
            "lookup",
            BattleMapAsset::parse("5 3\n.0.0.0.0.0\n.0.0.0.0.0\n.0.0.0.0.0\n").unwrap(),
        )
        .unwrap();
    }
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
    (dir, config, world, map, interior)
}

/// Common authored mine used to distinguish definition state from committed coverage.
fn mine(kind: BattleMineKind) -> BattleMinefield {
    BattleMinefield {
        coordinate: BattleHexCoordinate { x: 4, y: 2 },
        kind,
        strength: 10,
        extra: 0,
        owner: ObjectId(1),
    }
}

/// One independently marked building coordinate, separate from the mine's row.
fn entrance(interior: ObjectId) -> BattleBuildingEntrance {
    BattleBuildingEntrance {
        coordinate: BattleHexCoordinate { x: 3, y: 0 },
        interior,
        data_char: 0,
        data_short: 0,
        data_int: 0,
    }
}

/// Inspect serialized owned rows without exposing a mutable public cache API.
fn bits(world: &World, map: ObjectId) -> serde_json::Value {
    serde_json::to_value(&world.btech).unwrap()["maps"][map.0.to_string()]["lookup_bits"].clone()
}

/// Listing and resize refusal observe allocation; deletion preserves the underlying definitions.
#[tokio::test]
async fn lookup_lifecycle_native_lua_and_resize_precedence() {
    let (_dir, config, mut world, map, interior) = fixture().await;
    let field = mine(BattleMineKind::Standard);
    set_minefield(&mut world, map, 0, Some(field)).unwrap();
    set_building_entrance(&mut world, map, 0, Some(entrance(interior))).unwrap();
    assert_eq!(
        bits(&world, map),
        serde_json::json!({"0":[128,0],"2":[0,1]})
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let listing = support::run_text(&scripts, &config, ObjectId(1), 1, "list objs");
    assert_eq!(
        listing
            .matches("--- MAP/HANGAR INFORMATION OBJECT ---")
            .count(),
        1
    );
    assert_eq!(scripts.world().btech, world.btech);
    for command in [
        "setmapsize",
        "setmapsize nonsense",
        "setmapsize 0 0",
        "setmapsize 5 3",
    ] {
        let output = support::run_text(&scripts, &config, ObjectId(1), 1, command);
        assert_eq!(
            text::plain(&output).trim(),
            "Invalid map for size change, sorry."
        );
        assert_eq!(scripts.world().btech, world.btech);
    }
    let error = scripts
        .eval_callback::<()>(&format!("btech.map.resize(1,{},0,0)", map.0))
        .unwrap_err();
    assert!(format!("{error:#}").contains("Invalid map for size change, sorry."));
    let delete = format!("btech.map.delete_objects(1,{},'TBITS')", map.0);
    assert!(
        scripts
            .eval_callback::<()>(&format!("{delete}; error('abort')"))
            .is_err()
    );
    assert_eq!(scripts.world().btech, world.btech);
    assert!(scripts.drain_outbox().is_empty());
    let output = support::run_text(&scripts, &config, ObjectId(1), 1, "delobj TBITS");
    assert_eq!(text::plain(&output).trim(), "1 objects deleted!");
    world = scripts.world().clone();
    assert!(!world.btech.maps()[&map].has_lookup_object());
    assert!(
        !world.btech.maps()[&map]
            .mine_coverage(field.coordinate)
            .unwrap()
    );
    assert_eq!(world.btech.maps()[&map].minefields()[&0], field);
    assert_eq!(
        world.btech.maps()[&map]
            .building_at(entrance(interior).coordinate)
            .unwrap(),
        Some(entrance(interior))
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, world.btech);
    // An explicit mine edit rebuilds its lane without inventing hangar coverage.
    set_minefield(&mut world, map, 0, Some(field)).unwrap();
    assert_eq!(bits(&world, map), serde_json::json!({"2":[0,1]}));
    set_minefield(&mut world, map, 0, None).unwrap();
    assert_eq!(bits(&world, map), serde_json::json!({"2":[0,0]}));
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

/// Detonating an uncached command mine allocates an empty object, which has no saved row.
#[tokio::test]
async fn empty_allocation_blocks_resize_but_has_no_persistent_row() {
    let (_dir, config, mut world, map, _) = fixture().await;
    set_minefield(&mut world, map, 0, Some(mine(BattleMineKind::Command))).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    delete_battle_map_objects_action(
        &scripts,
        &config,
        ObjectId(1),
        map,
        Some(BattleMapObjectKind::Bits),
        None,
    )
    .unwrap();
    world = scripts.world().clone();
    let blast = resolve_mine_blast(&mut world, map, 0, BattleMovementRules::STANDARD.fall).unwrap();
    assert_eq!(blast.removed, vec![0]);
    assert_eq!(bits(&world, map), serde_json::json!({}));
    assert!(world.btech.maps()[&map].has_lookup_object());
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    assert!(resize_battle_map_action(&scripts, &config, ObjectId(1), map, 5, 3).is_err());
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert!(!restored.btech.maps()[&map].has_lookup_object());
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(restored))).unwrap();
    resize_battle_map_action(&scripts, &config, ObjectId(1), map, 5, 3).unwrap();
}

/// Inspection preserves saved bytes; startup rebuilds only the mine lane and commits it.
#[tokio::test]
async fn lookup_import_validation_and_startup_reconciliation() {
    let (_dir, config, mut world, map, interior) = fixture().await;
    let field = mine(BattleMineKind::Standard);
    set_minefield(&mut world, map, 0, Some(field)).unwrap();
    set_building_entrance(&mut world, map, 0, Some(entrance(interior))).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut sql = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE btech_map_bits SET value=0 WHERE map_dbref=? AND y=2")
        .bind(map.0)
        .execute(&mut sql)
        .await
        .unwrap();
    for (index, value) in [(0, 1), (1, 252)] {
        sqlx::query("INSERT INTO btech_map_bits VALUES(?,1,?,?)")
            .bind(map.0)
            .bind(index)
            .bind(value)
            .execute(&mut sql)
            .await
            .unwrap();
    }
    let inspected = persistence::load(&config.database()).await.unwrap();
    assert!(
        !inspected.btech.maps()[&map]
            .mine_coverage(field.coordinate)
            .unwrap()
    );
    assert!(
        inspected.btech.maps()[&map]
            .mine_coverage(BattleHexCoordinate { x: 0, y: 1 })
            .unwrap()
    );
    assert_eq!(bits(&inspected, map)["1"], serde_json::json!([1, 252]));
    let prepared = server::prepare(&config).await.unwrap();
    assert!(
        prepared.world().btech.maps()[&map]
            .mine_coverage(field.coordinate)
            .unwrap()
    );
    assert!(
        !prepared.world().btech.maps()[&map]
            .mine_coverage(BattleHexCoordinate { x: 0, y: 1 })
            .unwrap()
    );
    assert_eq!(
        bits(&prepared.world(), map)["0"],
        serde_json::json!([128, 0])
    );
    assert_eq!(
        bits(&prepared.world(), map)["1"],
        serde_json::json!([0, 252])
    );
    assert_eq!(
        bits(&persistence::load(&config.database()).await.unwrap(), map),
        bits(&prepared.world(), map)
    );
    sqlx::query("PRAGMA ignore_check_constraints=ON")
        .execute(&mut sql)
        .await
        .unwrap();
    for (y, index, value) in [(0, 0, 1), (0, 1, 1), (3, 0, 1), (0, 2, 1), (0, 0, 256)] {
        sqlx::query("DELETE FROM btech_map_bits WHERE map_dbref=?")
            .bind(map.0)
            .execute(&mut sql)
            .await
            .unwrap();
        sqlx::query("INSERT INTO btech_map_bits VALUES(?,?,?,?)")
            .bind(map.0)
            .bind(y)
            .bind(index)
            .bind(value)
            .execute(&mut sql)
            .await
            .unwrap();
        assert!(
            persistence::load(&config.database()).await.is_err(),
            "{y},{index},{value}"
        );
    }
}

/// Unmarked artillery deposits allow duplicates until an explicit lookup rebuild activates them.
#[tokio::test]
async fn artillery_deposition_defers_coverage_and_cache_controls_duplicates() {
    let (_dir, config, mut world, map, _) = fixture().await;
    let center = BattleHexCoordinate { x: 2, y: 1 };
    for expected in [0, 1] {
        let mut flight = BattleArtilleryFlight::new(
            center,
            center,
            BattleWeapon::LongTom,
            BattleArtilleryMode::Mine,
            true,
        )
        .unwrap();
        let mut result = None;
        for _ in 0..10 {
            result = advance_artillery_flight(
                &mut world,
                map,
                &mut flight,
                BattleMovementRules::STANDARD.fall,
            )
            .unwrap();
        }
        assert_eq!(result.unwrap().mines, vec![expected]);
        assert!(!world.btech.maps()[&map].mine_coverage(center).unwrap());
        assert_eq!(
            world.btech.maps()[&map].minefields()[&expected].owner,
            ObjectId(0)
        );
        persistence::save(&config.database(), &world).await.unwrap();
        world = persistence::load(&config.database()).await.unwrap();
    }
    let original = world.btech.maps()[&map].minefields()[&0];
    set_minefield(&mut world, map, 0, Some(original)).unwrap();
    assert!(world.btech.maps()[&map].mine_coverage(center).unwrap());
    let mut flight = BattleArtilleryFlight::new(
        center,
        center,
        BattleWeapon::LongTom,
        BattleArtilleryMode::Mine,
        true,
    )
    .unwrap();
    let mut result = None;
    for _ in 0..10 {
        result = advance_artillery_flight(
            &mut world,
            map,
            &mut flight,
            BattleMovementRules::STANDARD.fall,
        )
        .unwrap();
    }
    assert!(result.unwrap().mines.is_empty());
    assert_eq!(world.btech.maps()[&map].minefields().len(), 2);
    world.validate(&config).unwrap();
}

/// Coordinate-wide deletion removes TBITS before the final rebuild of surviving mines.
#[tokio::test]
async fn coordinate_deletion_rebuilds_surviving_mines_after_removing_lookup_object() {
    let (_dir, config, mut world, map, _) = fixture().await;
    let surviving = mine(BattleMineKind::Standard);
    set_minefield(&mut world, map, 0, Some(surviving)).unwrap();
    let removed = BattleMinefield {
        coordinate: BattleHexCoordinate { x: 0, y: 0 },
        ..surviving
    };
    set_minefield(&mut world, map, 1, Some(removed)).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let call = format!("btech.map.delete_objects(1,{},nil,0,0)", map.0);
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!("{call};error('abort')"))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    assert_eq!(
        scripts
            .eval_callback::<usize>(&format!("return {call}"))
            .unwrap(),
        2
    );
    let saved = scripts.world().clone();
    assert_eq!(saved.btech.maps()[&map].minefields().len(), 1);
    assert_eq!(bits(&saved, map), serde_json::json!({"2":[0,1]}));
    assert!(
        saved.btech.maps()[&map]
            .mine_coverage(surviving.coordinate)
            .unwrap()
    );
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
}

#[allow(dead_code)]
#[path = "support/btech_firing.rs"]
mod firing;

/// All supported scanner anatomies honor deletion before selecting fields or spending scan dice.
#[tokio::test]
async fn deleted_lookup_disables_mine_selection_and_scan_until_explicit_rebuild() {
    for source in firing::templates() {
        let (_dir, config, mut world, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let map = if let Some(unit) = world.btech.vehicles().get(&id) {
            unit.position().unwrap().map
        } else {
            world.btech.constructed_units()[&id].position().unwrap().map
        };
        let coordinate = BattleHexCoordinate { x: 0, y: 11 };
        let field = BattleMinefield {
            coordinate,
            ..mine(BattleMineKind::Standard)
        };
        set_minefield(&mut world, map, 0, Some(field)).unwrap();
        world
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        world
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        set_battle_character(
            &mut world,
            ObjectId(1),
            BattleCharacter {
                bruise: 0,
                lethal: 0,
                build: 5,
                reflexes: 5,
                intuition: 5,
                learn: 5,
                charisma: 5,
            },
        )
        .unwrap();
        firing::edit(&mut world, id, |state| {
            state["scanner_perception"] = (-10).into()
        });
        assert_eq!(
            mine_activations(&world, id, BattleMineTriggerReason::Step)
                .unwrap()
                .len(),
            1
        );
        let mut detected = world.clone();
        assert!(
            scan_battle_mines(&mut detected, id, ObjectId(1), coordinate, 1000)
                .unwrap()
                .found
        );
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        delete_battle_map_objects_action(
            &scripts,
            &config,
            ObjectId(1),
            map,
            Some(BattleMapObjectKind::Bits),
            None,
        )
        .unwrap();
        let mut world = scripts.world().clone();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        restored
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        for candidate in [&mut world, &mut restored] {
            let before = candidate.btech.clone();
            assert!(
                mine_activations(candidate, id, BattleMineTriggerReason::Step)
                    .unwrap()
                    .is_empty()
            );
            assert!(
                !scan_battle_mines(candidate, id, ObjectId(1), coordinate, 1000)
                    .unwrap()
                    .found
            );
            assert_eq!(candidate.btech, before);
            assert_eq!(candidate.btech.maps()[&map].minefields().len(), 1);
            set_minefield(candidate, map, 0, Some(field)).unwrap();
            assert_eq!(
                mine_activations(candidate, id, BattleMineTriggerReason::Step)
                    .unwrap()
                    .len(),
                1
            );
            assert!(
                scan_battle_mines(candidate, id, ObjectId(1), coordinate, 1000)
                    .unwrap()
                    .found
            );
        }
        assert_eq!(world.btech, restored.btech);
    }
}
