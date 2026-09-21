//! Shared building route selection and selective map-object persistence.
use crate::support;
use stompymux_rs::*;

/// Copying authored records preserves their complete payload, including signed scalar bounds.
#[tokio::test]
async fn building_payloads_survive_copy_retarget_and_restart() {
    let (_dir, config, mut world, exterior, interior) = fixture().await;
    let entrance = BattleBuildingEntrance {
        data_char: u8::MAX,
        data_short: i16::MIN,
        data_int: i64::MIN,
        ..world.btech.maps()[&exterior].building_entrances()[&3]
    };
    let point = BattleBuildingEntryPoint {
        object: ObjectId(77),
        data_short: i16::MAX,
        data_int: i64::MAX,
        ..world.btech.maps()[&interior].building_entry_points()[&1]
    };
    let exit = BattleBuildingExit {
        data_char: 87,
        data_short: -23,
        data_int: 91,
        ..world.btech.maps()[&interior].building_exits()[&4]
    };
    set_building_entrance(&mut world, exterior, 3, Some(entrance)).unwrap();
    set_battle_building_entry_point(&mut world, interior, 1, Some(point)).unwrap();
    set_battle_building_return_link(&mut world, interior, 4, Some(exit)).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    let copied_entrance = world.btech.maps()[&exterior].building_entrances()[&3];
    let copied_point = world.btech.maps()[&interior].building_entry_points()[&1];
    let copied_exit = world.btech.maps()[&interior].building_exits()[&4];
    assert_eq!(
        (copied_entrance, copied_point, copied_exit),
        (entrance, point, exit)
    );
    set_building_entrance(&mut world, exterior, 9, Some(copied_entrance)).unwrap();
    set_battle_building_entry_point(&mut world, interior, 9, Some(copied_point)).unwrap();
    set_battle_building_return_link(&mut world, interior, 9, Some(copied_exit)).unwrap();
    set_battle_building_exit(&mut world, interior, 9, Some(exterior)).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, world.btech);
    assert_eq!(
        restored.btech.maps()[&exterior].building_entrances()[&9],
        entrance
    );
    assert_eq!(
        restored.btech.maps()[&interior].building_entry_points()[&9],
        point
    );
    assert_eq!(restored.btech.maps()[&interior].building_exits()[&9], exit);
}

/// Build reciprocal maps with deliberately unordered and duplicate direction slots.
async fn fixture() -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let exterior = world.create(&config, "Exterior".into(), Kind::Room);
    let interior = world.create(&config, "Interior".into(), Kind::Room);
    for id in [exterior, interior] {
        create_battle_map(
            &mut world,
            id,
            "routes",
            BattleMapAsset::parse("3 2\n.0.0.0\n.0.0.0\n").unwrap(),
        )
        .unwrap();
    }
    for (ordinal, x) in [(5, 1), (3, 2)] {
        set_building_entrance(
            &mut world,
            exterior,
            ordinal,
            Some(BattleBuildingEntrance {
                coordinate: BattleHexCoordinate { x, y: 0 },
                interior,
                data_char: 0,
                data_short: 0,
                data_int: 0,
            }),
        )
        .unwrap();
    }
    for (ordinal, direction, x, y) in [(5, b'n', 1, 1), (1, b'n', 0, 0), (3, b'e', 2, 1)] {
        set_battle_building_entry_point(
            &mut world,
            interior,
            ordinal,
            Some(BattleBuildingEntryPoint {
                coordinate: BattleHexCoordinate { x, y },
                direction,
                object: ObjectId(-1),
                data_short: 0,
                data_int: 0,
            }),
        )
        .unwrap();
    }
    set_battle_building_exit(&mut world, interior, 4, Some(exterior)).unwrap();
    (dir, config, world, exterior, interior)
}

#[tokio::test]
async fn building_routes_select_first_slots_validate_and_replay() {
    let (_dir, config, mut world, exterior, interior) = fixture().await;
    let coordinate = BattleHexCoordinate { x: 1, y: 0 };
    for direction in [None, Some(0), Some(b'n'), Some(b'N')] {
        assert_eq!(
            battle_building_entry_destination(&world, exterior, coordinate, direction).unwrap(),
            BattlePosition {
                map: interior,
                x: 0,
                y: 0
            }
        );
    }
    assert_eq!(
        battle_building_entry_destination(&world, exterior, coordinate, Some(b'e')).unwrap(),
        BattlePosition {
            map: interior,
            x: 2,
            y: 1
        }
    );
    assert!(battle_building_entry_destination(&world, exterior, coordinate, Some(b'w')).is_err());
    assert_eq!(
        battle_building_exit_destination(&world, interior).unwrap(),
        BattlePosition {
            map: exterior,
            x: 2,
            y: 0
        }
    );
    let before = world.btech.clone();
    assert!(set_battle_building_exit(&mut world, interior, 0, Some(interior)).is_err());
    assert!(
        set_battle_building_entry_point(
            &mut world,
            interior,
            0,
            Some(BattleBuildingEntryPoint {
                coordinate: BattleHexCoordinate { x: 3, y: 0 },
                direction: b's',
                object: ObjectId(-1),
                data_short: 0,
                data_int: 0,
            })
        )
        .is_err()
    );
    assert_eq!(world.btech, before);
    let mut corrupt = serde_json::to_value(&world.btech).unwrap();
    corrupt["maps"][interior.0.to_string()]["building_exits"]["4"]["destination"] =
        interior.0.into();
    let mut invalid = world.clone();
    invalid.btech = serde_json::from_value(corrupt).unwrap();
    assert!(invalid.validate(&config).is_err());
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    assert_eq!(
        battle_building_exit_destination(&loaded, interior).unwrap(),
        BattlePosition {
            map: exterior,
            x: 2,
            y: 0
        }
    );
    set_building_entrance(&mut world, exterior, 3, None).unwrap();
    assert!(battle_building_exit_destination(&world, interior).is_err());
    assert!(world.btech.maps()[&interior].building_exits().is_empty());
    // Another exterior entrance survives, but it does not recreate deleted return links.
    assert!(
        world.btech.maps()[&exterior]
            .building_entrances()
            .contains_key(&5)
    );
    set_building_entrance(&mut world, exterior, 5, None).unwrap();
    assert!(battle_building_exit_destination(&world, interior).is_err());
}

#[tokio::test]
async fn route_updates_preserve_authored_payloads_and_terrain_reload_keeps_routes() {
    use sqlx::Connection;
    let (_dir, config, mut world, exterior, interior) = fixture().await;
    persistence::save(&config.database(), &world).await.unwrap();
    let mut connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE btech_map_objects SET object_dbref=77,data_short=78,data_int=79 WHERE map_dbref=? AND object_type=6 AND ordinal=1").bind(interior.0).execute(&mut connection).await.unwrap();
    sqlx::query("UPDATE btech_map_objects SET x=19,y=20,data_char=21,data_short=22,data_int=23 WHERE map_dbref=? AND object_type=5").bind(interior.0).execute(&mut connection).await.unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    let retained = world.btech.maps()[&interior].building_entry_points()[&1];
    assert_eq!(
        (retained.object.0, retained.data_short, retained.data_int),
        (77, 78, 79)
    );
    set_battle_building_entry_point(
        &mut world,
        interior,
        1,
        Some(BattleBuildingEntryPoint {
            coordinate: BattleHexCoordinate { x: 1, y: 0 },
            direction: b's',
            ..retained
        }),
    )
    .unwrap();
    reload_battle_map(
        &mut world,
        interior,
        "new terrain",
        BattleMapAsset::parse("3 2\n.1.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let point: (i64,i64,i64) = sqlx::query_as("SELECT object_dbref,data_short,data_int FROM btech_map_objects WHERE map_dbref=? AND object_type=6 AND ordinal=1").bind(interior.0).fetch_one(&mut connection).await.unwrap();
    assert_eq!(point, (77, 78, 79));
    let exit: (i64,i64,i64,i64,i64) = sqlx::query_as("SELECT x,y,data_char,data_short,data_int FROM btech_map_objects WHERE map_dbref=? AND object_type=5").bind(interior.0).fetch_one(&mut connection).await.unwrap();
    assert_eq!(exit, (19, 20, 21, 22, 23));
    assert_eq!(
        world.btech.maps()[&interior].building_exits()[&4].coordinate,
        BattleHexCoordinate { x: 19, y: 20 }
    );
    let coordinate = BattleHexCoordinate { x: -5, y: 100 };
    let retained_exit = world.btech.maps()[&interior].building_exits()[&4];
    set_battle_building_return_link(
        &mut world,
        interior,
        4,
        Some(BattleBuildingExit {
            coordinate,
            destination: exterior,
            ..retained_exit
        }),
    )
    .unwrap();
    // Destination-only edits retain metadata coordinates rather than resetting them to the origin.
    set_battle_building_exit(&mut world, interior, 4, Some(exterior)).unwrap();
    assert_eq!(
        world.btech.maps()[&interior].building_exits()[&4].coordinate,
        coordinate
    );
    let before = world.btech.clone();
    for destination in [interior, ObjectId(i64::MAX)] {
        assert!(
            set_battle_building_return_link(
                &mut world,
                interior,
                4,
                Some(BattleBuildingExit {
                    coordinate,
                    destination,
                    data_char: 0,
                    data_short: 0,
                    data_int: 0,
                })
            )
            .is_err()
        );
        assert_eq!(world.btech, before);
    }
    assert_eq!(
        battle_building_exit_destination(&world, interior).unwrap(),
        BattlePosition {
            map: exterior,
            x: 2,
            y: 0
        }
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let exit: (i64,i64,i64,i64,i64) = sqlx::query_as("SELECT x,y,data_char,data_short,data_int FROM btech_map_objects WHERE map_dbref=? AND object_type=5").bind(interior.0).fetch_one(&mut connection).await.unwrap();
    assert_eq!(exit, (-5, 100, 21, 22, 23));
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let inspected: (i32, i32, i64) = scripts.eval_callback(&format!("local r=btech.map.inspect({}).building_exits[4]; return r.coordinate.x,r.coordinate.y,r.destination", interior.0)).unwrap();
    assert_eq!(inspected, (-5, 100, exterior.0));

    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    assert_eq!(
        battle_building_exit_destination(&loaded, interior)
            .unwrap()
            .map,
        exterior
    );
    set_battle_building_entry_point(&mut world, interior, 1, None).unwrap();
    set_battle_building_exit(&mut world, interior, 4, None).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert!(loaded.btech.maps()[&interior].building_exits().is_empty());
    assert_eq!(
        loaded.btech.maps()[&interior].building_entry_points().len(),
        2
    );
}

/// Individual removal clears every interior return link, preserves arrivals, and persists both maps.
#[tokio::test]
async fn entrance_removal_clears_interior_returns_and_replays() {
    let (_dir, config, mut world, exterior, interior) = fixture().await;
    set_battle_building_exit(&mut world, interior, 9, Some(exterior)).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let before = world.btech.clone();
    set_building_entrance(&mut world, exterior, 999, None).unwrap();
    assert_eq!(world.btech, before);
    set_building_entrance(&mut world, exterior, 3, None).unwrap();
    assert!(world.btech.maps()[&interior].building_exits().is_empty());
    assert_eq!(
        world.btech.maps()[&interior].building_entry_points(),
        before.maps()[&interior].building_entry_points()
    );
    assert_eq!(world.btech.maps()[&exterior].building_entrances().len(), 1);
    assert!(battle_building_exit_destination(&world, interior).is_err());
    assert!(
        battle_building_entry_destination(
            &world,
            exterior,
            BattleHexCoordinate { x: 1, y: 0 },
            None
        )
        .is_ok()
    );
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    set_building_entrance(&mut loaded, exterior, 3, None).unwrap();
    assert_eq!(loaded.btech, world.btech);
}
