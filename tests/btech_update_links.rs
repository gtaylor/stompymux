//! Authored link persistence and bounded, atomic rebuilding of shared building routes.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
mod support;

fn add_map(
    world: &mut World,
    config: &Config,
    name: &str,
    width: usize,
    height: usize,
) -> ObjectId {
    let id = world.create(config, name.into(), Kind::Room);
    let source = format!(
        "{width} {height}\n{}",
        format!("{}\n", ".0".repeat(width)).repeat(height)
    );
    create_battle_map(world, id, name, BattleMapAsset::parse(&source).unwrap()).unwrap();
    id
}

fn link(parent: ObjectId, entries: [BattleMapEntrance; 4]) -> BattleMapLink {
    BattleMapLink {
        parent,
        coordinate: BattleHexCoordinate { x: 0, y: 0 },
        entrances: entries,
    }
}

#[tokio::test]
async fn rebuild_order_native_lua_rollback_and_restart() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let root = add_map(&mut world, &config, "root", 5, 5);
    let a = add_map(&mut world, &config, "a", 5, 3);
    let b = add_map(&mut world, &config, "b", 3, 5);
    let c = add_map(&mut world, &config, "c", 1, 1);
    let stale = add_map(&mut world, &config, "stale", 1, 1);
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(root);
    set_battle_map_link(
        &mut world,
        a,
        Some(link(
            root,
            [
                BattleMapEntrance::Offset { distance: 1 },
                BattleMapEntrance::Exact {
                    coordinate: BattleHexCoordinate { x: 4, y: 2 },
                },
                BattleMapEntrance::None,
                BattleMapEntrance::Offset { distance: i32::MAX },
            ],
        )),
    )
    .unwrap();
    set_battle_map_link(&mut world, b, Some(link(root, Default::default()))).unwrap();
    set_battle_map_link(
        &mut world,
        c,
        Some(link(
            a,
            [BattleMapEntrance::Offset { distance: i32::MAX }; 4],
        )),
    )
    .unwrap();
    set_building_entrance(
        &mut world,
        root,
        9,
        Some(BattleBuildingEntrance {
            coordinate: BattleHexCoordinate { x: 1, y: 1 },
            interior: stale,
            data_char: 0,
            data_short: 0,
            data_int: 0,
        }),
    )
    .unwrap();
    set_battle_building_exit(&mut world, stale, 0, Some(root)).unwrap();
    set_battle_building_entry_point(
        &mut world,
        root,
        7,
        Some(BattleBuildingEntryPoint {
            coordinate: BattleHexCoordinate { x: 2, y: 2 },
            direction: b'n',
            object: ObjectId(-1),
            data_short: 0,
            data_int: 0,
        }),
    )
    .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    let lookup = serde_json::to_value(&world.btech).unwrap();
    assert_eq!(
        lookup["maps"][root.0.to_string()]["lookup_bits"],
        serde_json::json!({"1":[8,0]})
    );
    let before = world.btech.clone();
    let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let call = format!("btech.map.update_links_as(1,{})", root.0);
    assert!(
        lua.eval_callback::<()>(&format!("{call};error('abort')"))
            .is_err()
    );
    assert_eq!(lua.world().btech, before);
    assert!(lua.drain_outbox().is_empty());
    let stats: (usize, usize, usize, usize) = lua
        .eval_callback(&format!(
            "local r={call}; return r.buildings,r.leaves,r.entrances,r.skipped"
        ))
        .unwrap();
    assert_eq!(stats, (3, 3, 7, 0));
    let text = support::run_text(&native, &config, ObjectId(1), 1, "updatelinks ignored");
    assert!(
        text.contains(
            "Updated 3 BUILD objs, 3 LEAVE objs, 7 ENTRANCE objs; skipped 0 link descents."
        ),
        "{text}"
    );
    assert_eq!(native.world().btech, lua.world().btech);
    let saved = native.world().clone();
    // Rebuilding marks authored entrances at zero and clears the stale hangar at (1,1).
    // The cleared row stays allocated and survives persistence below.
    let lookup = serde_json::to_value(&saved.btech).unwrap();
    assert_eq!(
        lookup["maps"][root.0.to_string()]["lookup_bits"],
        serde_json::json!({"0":[2,0],"1":[0,0]})
    );

    assert_eq!(
        saved.btech.maps()[&root].building_entrances()[&0].interior,
        b
    );
    assert_eq!(
        saved.btech.maps()[&root].building_entrances()[&1].interior,
        a
    );
    assert!(saved.btech.maps()[&stale].building_exits().is_empty());
    assert_eq!(
        saved.btech.maps()[&root].building_entry_points(),
        before.maps()[&root].building_entry_points()
    );
    let points = saved.btech.maps()[&a].building_entry_points();
    assert_eq!(
        points.values().map(|p| p.direction).collect::<Vec<_>>(),
        vec![b'w', b'e', b'n']
    );
    assert_eq!(points[&0].coordinate, BattleHexCoordinate { x: 4, y: 1 });
    assert_eq!(points[&2].coordinate, BattleHexCoordinate { x: 2, y: 1 });
    assert_eq!(saved.btech.maps()[&c].building_entry_points().len(), 4);
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
    update_battle_map_links_action(&native, &config, ObjectId(1), root).unwrap();
    assert_eq!(native.world().btech, saved.btech);
}

#[tokio::test]
async fn configured_cycles_and_depth_limit_terminate_without_recursion() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let a = add_map(&mut world, &config, "a", 1, 1);
    let b = add_map(&mut world, &config, "b", 1, 1);
    set_battle_map_link(&mut world, a, Some(link(b, Default::default()))).unwrap();
    set_battle_map_link(&mut world, b, Some(link(a, Default::default()))).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let report = update_battle_map_links_action(&scripts, &config, ObjectId(1), a).unwrap();
    assert_eq!((report.buildings, report.leaves, report.skipped), (2, 1, 1));
    let (_dir, config, mut world) = support::isolated_world().await;
    let mut maps = Vec::new();
    for index in 0..1026 {
        let id = add_map(&mut world, &config, &format!("depth {index}"), 1, 1);
        if let Some(&parent) = maps.last() {
            set_battle_map_link(&mut world, id, Some(link(parent, Default::default()))).unwrap();
        }
        maps.push(id);
    }
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let report = update_battle_map_links_action(&scripts, &config, ObjectId(1), maps[0]).unwrap();
    assert_eq!(
        (report.buildings, report.leaves, report.skipped),
        (1024, 1023, 1)
    );
    assert!(
        scripts.world().btech.maps()[&maps[1024]]
            .building_exits()
            .is_empty()
    );
}

#[tokio::test]
async fn configuration_lua_preserves_inactive_columns_and_missing_default_rows() {
    use sqlx::Connection;
    let (_dir, config, mut world) = support::isolated_world().await;
    let parent = add_map(&mut world, &config, "parent", 3, 3);
    let child = add_map(&mut world, &config, "child", 3, 3);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let call = format!(
        "btech.map.set_authored_link({},{{parent={},coordinate={{x=1,y=1}}}})",
        child.0, parent.0
    );
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!("{call};error('abort')"))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    scripts.eval_callback::<()>(&call).unwrap();
    assert_eq!(
        scripts
            .eval_callback::<i64>(&format!(
                "return btech.map.authored_link({}).parent",
                child.0
            ))
            .unwrap(),
        parent.0
    );
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let mut sql = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE btech_map_entrances SET x=91,y=92,offset=93 WHERE child_dbref=? AND direction=0",
    )
    .bind(child.0)
    .execute(&mut sql)
    .await
    .unwrap();
    sqlx::query("DELETE FROM btech_map_entrances WHERE child_dbref=? AND direction=1")
        .bind(child.0)
        .execute(&mut sql)
        .await
        .unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    let mut definition = loaded.btech.maps()[&child].authored_link().unwrap();
    definition.entrances[0] = BattleMapEntrance::Offset { distance: 1 };
    definition.entrances[1] = BattleMapEntrance::Exact {
        coordinate: BattleHexCoordinate { x: 2, y: 1 },
    };
    set_battle_map_link(&mut loaded, child, Some(definition)).unwrap();
    persistence::save(&config.database(), &loaded)
        .await
        .unwrap();
    let row: (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT mode,x,y,offset FROM btech_map_entrances WHERE child_dbref=? AND direction=0",
    )
    .bind(child.0)
    .fetch_one(&mut sql)
    .await
    .unwrap();
    assert_eq!(row, (1, 91, 92, 1));
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        loaded.btech
    );
    let before = loaded.btech.clone();
    for bad in [
        BattleMapEntrance::Offset { distance: -1 },
        BattleMapEntrance::Exact {
            coordinate: BattleHexCoordinate { x: 3, y: 0 },
        },
    ] {
        let mut invalid = definition;
        invalid.entrances[0] = bad;
        assert!(set_battle_map_link(&mut loaded, child, Some(invalid)).is_err());
        assert_eq!(loaded.btech, before);
    }
    set_battle_map_link(&mut loaded, child, None).unwrap();
    persistence::save(&config.database(), &loaded)
        .await
        .unwrap();
    assert!(
        persistence::load(&config.database())
            .await
            .unwrap()
            .btech
            .maps()[&child]
            .authored_link()
            .is_none()
    );
}

/// Resizing retains authored intent while rebuilds skip placements outside current dimensions.
#[tokio::test]
async fn rebuild_resolves_offsets_and_skips_cropped_coordinates() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let root = add_map(&mut world, &config, "root", 3, 3);
    let child = add_map(&mut world, &config, "child", 3, 3);
    let cropped = add_map(&mut world, &config, "cropped", 1, 1);
    let definition = link(
        root,
        [
            BattleMapEntrance::Exact {
                coordinate: BattleHexCoordinate { x: 2, y: 2 },
            },
            BattleMapEntrance::Offset { distance: i32::MAX },
            BattleMapEntrance::None,
            BattleMapEntrance::None,
        ],
    );
    set_battle_map_link(&mut world, child, Some(definition)).unwrap();
    let mut outside = link(root, Default::default());
    outside.coordinate = BattleHexCoordinate { x: 2, y: 2 };
    set_battle_map_link(&mut world, cropped, Some(outside)).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    resize_battle_map_action(&scripts, &config, ObjectId(1), root, 2, 2).unwrap();
    resize_battle_map_action(&scripts, &config, ObjectId(1), child, 1, 1).unwrap();
    let report = update_battle_map_links_action(&scripts, &config, ObjectId(1), root).unwrap();
    assert_eq!(
        (
            report.buildings,
            report.leaves,
            report.entrances,
            report.skipped
        ),
        (1, 1, 1, 0)
    );
    let saved = scripts.world().clone();
    assert_eq!(saved.btech.maps()[&child].authored_link(), Some(definition));
    assert_eq!(saved.btech.maps()[&cropped].authored_link(), Some(outside));
    let points = saved.btech.maps()[&child].building_entry_points();
    assert_eq!(points.len(), 1);
    assert_eq!(
        points[&0],
        BattleBuildingEntryPoint {
            coordinate: BattleHexCoordinate { x: 0, y: 0 },
            direction: b'e',
            object: ObjectId(-1),
            data_short: 0,
            data_int: 0,
        }
    );
    assert!(saved.btech.maps()[&cropped].building_exits().is_empty());
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
}

/// Authority rejection and failed final output must not leave partially rebuilt routes.
#[tokio::test]
async fn rebuild_failure_restores_routes_and_configuration() {
    let (dir, config, mut world) = support::isolated_world().await;
    let root = add_map(&mut world, &config, "root", 2, 2);
    let child = add_map(&mut world, &config, "child", 2, 2);
    let other = add_map(&mut world, &config, "other", 2, 2);
    set_battle_map_link(
        &mut world,
        child,
        Some(link(root, [BattleMapEntrance::Offset { distance: 0 }; 4])),
    )
    .unwrap();
    set_building_entrance(
        &mut world,
        root,
        7,
        Some(BattleBuildingEntrance {
            coordinate: BattleHexCoordinate { x: 1, y: 1 },
            interior: other,
            data_char: 0,
            data_short: 0,
            data_int: 0,
        }),
    )
    .unwrap();
    set_battle_building_exit(&mut world, other, 5, Some(root)).unwrap();
    let before = world.btech.clone();
    let path = dir.path().join("stompymux.toml");
    let mut table: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
    table
        .entry("runtime")
        .or_insert(toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .unwrap()
        .insert("output_message_limit".into(), 1.into());
    std::fs::write(path, toml::to_string(&table).unwrap()).unwrap();
    let config = Config::load(dir.path()).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let denied = update_battle_map_links_action(&scripts, &config, child, root).unwrap_err();
    assert!(denied.to_string().contains("Permission denied"));
    assert_eq!(scripts.world().btech, before);
    assert!(update_battle_map_links_action(&scripts, &config, ObjectId(1), root).is_err());
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
}

#[tokio::test]
async fn parent_metadata_is_owned_independently_of_return_routes() {
    use sqlx::Connection;
    let (_dir, config, mut world) = support::isolated_world().await;
    let root = add_map(&mut world, &config, "root", 2, 2);
    let child = add_map(&mut world, &config, "child", 2, 2);
    let other = add_map(&mut world, &config, "other", 2, 2);
    set_battle_map_link(&mut world, child, Some(link(root, Default::default()))).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE btech_maps SET on_map=123456 WHERE dbref=?")
        .bind(child.0)
        .execute(&mut connection)
        .await
        .unwrap();
    connection.close().await.unwrap();
    let mut world = persistence::load(&config.database()).await.unwrap();
    assert_eq!(world.btech.maps()[&child].building_parent, 123456);
    set_battle_building_exit(&mut world, child, 0, Some(other)).unwrap();
    assert_eq!(world.btech.maps()[&child].building_parent, 123456);
    persistence::save(&config.database(), &world).await.unwrap();
    let world = persistence::load(&config.database()).await.unwrap();
    assert_eq!(world.btech.maps()[&child].building_parent, 123456);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    update_battle_map_links_action(&scripts, &config, ObjectId(1), root).unwrap();
    assert_eq!(scripts.world().btech.maps()[&child].building_parent, root.0);
    assert_eq!(scripts.world().btech.maps()[&root].building_parent, 0);
    let mut world = scripts.world().clone();
    set_battle_building_exit(&mut world, child, 0, Some(other)).unwrap();
    assert_eq!(world.btech.maps()[&child].building_parent, root.0);
    set_building_entrance(&mut world, root, 999, None).unwrap();
    assert_eq!(world.btech.maps()[&child].building_parent, root.0);
    reload_battle_map(
        &mut world,
        child,
        "renamed",
        BattleMapAsset::parse("2 2\n.0.0\n.0.0\n").unwrap(),
    )
    .unwrap();
    assert_eq!(world.btech.maps()[&child].building_parent, root.0);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut world = persistence::load(&config.database()).await.unwrap();
    assert_eq!(world.btech.maps()[&child].building_parent, root.0);
    set_building_entrance(&mut world, root, 0, None).unwrap();
    assert_eq!(world.btech.maps()[&child].building_parent, 0);
    assert!(world.btech.maps()[&child].building_exits().is_empty());
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}
