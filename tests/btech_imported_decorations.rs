//! Imported decoration records retain source identity and unscheduled lifetimes; fire and smoke
//! records never change the terrain, and generic decorations restore theirs when deleted.
use crate::support;
use sqlx::Connection;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Restore duplicate records whose saved budget fields must not become live timers.
async fn fixture() -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Stored decorations".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "grid",
        BattleMapAsset::from_cells("2 2\n.1.1\n.1&1\n").unwrap(),
    )
    .unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut sql = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    for (kind, ordinal, terrain) in [(0, 0, 34), (0, 1, 35), (1, 0, 45), (2, 0, 32)] {
        sqlx::query("INSERT INTO btech_map_objects VALUES(?,?,?,1,1,1,?,123,456)")
            .bind(map.0)
            .bind(kind)
            .bind(ordinal)
            .bind(terrain)
            .execute(&mut sql)
            .await
            .unwrap();
    }
    world = persistence::load(&config.database()).await.unwrap();
    (dir, config, world, map)
}

#[tokio::test]
async fn imported_records_preserve_lookup_order_and_share_native_lua_removal() {
    let (_dir, config, mut world, map) = fixture().await;
    let coordinate = BattleHexCoordinate { x: 1, y: 1 };
    assert_eq!(
        world.btech.maps()[&map].hex(1, 1).unwrap().terrain(),
        Terrain::Fire
    );
    assert_eq!(
        world.btech.maps()[&map].base_hex(1, 1).unwrap().terrain(),
        Terrain::Grassland
    );
    assert!(!map_fire_pending(&world));
    assert!(!map_smoke_pending(&world));
    for kind in BattleStaticDecorationKind::ALL {
        for record in world.btech.maps()[&map].static_decorations(kind).values() {
            assert_eq!(record.object, ObjectId(1));
            assert_eq!(record.duration, 123);
            assert_eq!(record.scalar, 456);
        }
    }
    let before = world.btech.clone();
    advance_map_fire(&mut world).unwrap();
    advance_map_smoke(&mut world);
    assert_eq!(world.btech, before);
    // Tile zero deliberately collides numerically with source ordinal zero on another tile.
    set_map_decoration(
        &mut world,
        map,
        BattleHexCoordinate { x: 0, y: 0 },
        Some(BattleDecoration::new(BattleDecorationKind::Fire, 0, None)),
    )
    .unwrap();
    let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let listing = support::run_text(&native, &config, ObjectId(1), 1, "list objs");
    assert!(
        listing.contains("X   Y   Type  obj   dc   ds     di"),
        "{listing}"
    );
    assert!(
        listing.contains("1   1   FIRE  1     34   123    456"),
        "{listing}"
    );
    let before = lua.world().btech.clone();
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.map.delete_objects(1,{},'FIRE');error('abort')",
            map.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, before);
    assert!(lua.drain_outbox().is_empty());
    let count: usize = lua
        .eval_callback(&format!(
            "return btech.map.delete_objects(1,{},'FIRE')",
            map.0
        ))
        .unwrap();
    assert_eq!(count, 4);
    let report = support::run_text(&native, &config, ObjectId(1), 1, "delobj fire");
    assert!(report.contains("4 objects deleted!"), "{report}");
    assert_eq!(native.world().btech, lua.world().btech);
    assert_eq!(
        native.world().btech.maps()[&map]
            .hex(1, 1)
            .unwrap()
            .terrain(),
        Terrain::Grassland
    );
    assert_eq!(
        delete_battle_map_objects_action(
            &native,
            &config,
            ObjectId(1),
            map,
            None,
            Some(coordinate)
        )
        .unwrap(),
        2
    );
    assert_eq!(
        native.world().btech.maps()[&map]
            .hex(1, 1)
            .unwrap()
            .terrain(),
        Terrain::Grassland
    );
    let saved = native.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
}

#[tokio::test]
async fn replacing_imported_effects_preserves_underlying_terrain_and_clears_source_rows() {
    let (_dir, config, mut world, map) = fixture().await;
    set_map_decoration(
        &mut world,
        map,
        BattleHexCoordinate { x: 1, y: 1 },
        Some(BattleDecoration::new(BattleDecorationKind::Smoke, 2, None)),
    )
    .unwrap();
    for kind in BattleStaticDecorationKind::ALL {
        assert!(world.btech.maps()[&map].static_decorations(kind).is_empty());
    }
    assert_eq!(
        world.btech.maps()[&map].hex(1, 1).unwrap().terrain(),
        Terrain::Smoke
    );
    assert_eq!(
        world.btech.maps()[&map].base_hex(1, 1).unwrap().terrain(),
        Terrain::Grassland
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, world.btech);
    advance_map_smoke(&mut restored);
    advance_map_smoke(&mut restored);
    assert_eq!(
        restored.btech.maps()[&map].hex(1, 1).unwrap().terrain(),
        Terrain::Grassland
    );
    let (_dir, config, world, map) = fixture().await;
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    // Writing the terrain under permanent fire leaves the fire burning over it.
    set_battle_map_hex_action(
        &scripts,
        &config,
        ObjectId(1),
        map,
        BattleHexCoordinate { x: 1, y: 1 },
        BattleHex::new(Terrain::HeavyForest, 1),
    )
    .unwrap();
    assert_eq!(
        scripts.world().btech.maps()[&map]
            .hex(1, 1)
            .unwrap()
            .terrain(),
        Terrain::Fire
    );
    assert_eq!(
        scripts.world().btech.maps()[&map]
            .base_hex(1, 1)
            .unwrap()
            .terrain(),
        Terrain::HeavyForest
    );
    // Fire is not terrain, so it cannot be written into the map.
    assert!(
        set_battle_map_hex_action(
            &scripts,
            &config,
            ObjectId(1),
            map,
            BattleHexCoordinate { x: 1, y: 1 },
            BattleHex::new(Terrain::Fire, 1),
        )
        .is_err()
    );
    // Resizing clears fire and smoke with the other map objects.
    resize_battle_map_action(&scripts, &config, ObjectId(1), map, 2, 2).unwrap();
    assert_eq!(
        scripts.world().btech.maps()[&map]
            .hex(1, 1)
            .unwrap()
            .terrain(),
        Terrain::HeavyForest
    );
    for kind in BattleStaticDecorationKind::ALL {
        assert!(
            scripts.world().btech.maps()[&map]
                .static_decorations(kind)
                .is_empty()
        );
    }
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
}
