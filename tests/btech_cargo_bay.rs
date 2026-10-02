//! Saved cargo transfer locations, selective persistence and coordinate disclosure.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// A decoded map supports both authored transfer locations and ordinary stock.
async fn fixture() -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Cargo bay".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "bay",
        BattleMapAsset::from_cells("3 2\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    (dir, config, world, map)
}

/// Location checks reveal coordinates only under the saved hint policy.
#[tokio::test]
async fn cargo_point_bounds_authority_and_hints_are_enforced() {
    let (_dir, config, mut world, map) = fixture().await;
    check_battle_cargo_transfer_point(&world, map, 0, 0).unwrap();
    for reveal_hint in [false, true] {
        let point = BattleCargoTransferPoint {
            x: 2,
            y: 1,
            reveal_hint,
        };
        set_battle_cargo_transfer_point(&mut world, ObjectId(1), map, Some(point)).unwrap();
        check_battle_cargo_transfer_point(&world, map, 2, 1).unwrap();
        let error = check_battle_cargo_transfer_point(&world, map, 0, 0)
            .unwrap_err()
            .to_string();
        assert_eq!(error.contains("2,1"), reveal_hint, "{error}");
        let before = world.btech.clone();
        for (actor, target, x, y) in [
            (4, map, 1, 0),
            (1, ObjectId(999999), 0, 0),
            (1, map, -1, 0),
            (1, map, 3, 0),
            (1, map, 0, 2),
        ] {
            assert!(
                set_battle_cargo_transfer_point(
                    &mut world,
                    ObjectId(actor),
                    target,
                    Some(BattleCargoTransferPoint { x, y, reveal_hint })
                )
                .is_err()
            );
            assert_eq!(world.btech, before);
        }
        world.validate(&config).unwrap();
    }
    set_battle_cargo_transfer_point(&mut world, ObjectId(1), map, None).unwrap();
    assert_eq!(world.btech.maps()[&map].cargo_transfer_point(), None);
}

/// Native and Lua settings share the same domain operation and whole-callback rollback.
#[tokio::test]
async fn cargo_point_controls_share_native_lua_state_and_rollback() {
    let (_dir, config, world, map) = fixture().await;
    let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for (argument, value, fragment) in [
        ("2 1 reveal", "{x=2,y=1,reveal_hint=true}", "hint revealed"),
        ("1 0 hide", "{x=1,y=0}", "hint hidden"),
        ("clear", "nil", "no cargo transfer point"),
    ] {
        let callback = format!("btech.map.set_cargo_point(1,{},{} )", map.0, value);
        let before = lua.world().btech.clone();
        assert!(
            lua.eval_callback::<()>(&format!("{callback};error('abort')"))
                .is_err()
        );
        assert_eq!(lua.world().btech, before);
        assert!(lua.drain_outbox().is_empty());
        let output = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("@btech cargo-point #{} {argument}", map.0),
        );
        assert!(output.contains(fragment), "{output}");
        lua.eval_callback::<()>(&callback).unwrap();
        assert_eq!(native.world().btech, lua.world().btech);
        if argument == "clear" {
            assert!(
                lua.eval_callback::<bool>(&format!(
                    "return btech.map.cargo_point({}) == nil",
                    map.0
                ))
                .unwrap()
            );
        }
    }
    lua.eval_callback::<()>(&format!(
        "btech.map.set_cargo_point(1,{},{{x=1,y=1,reveal_hint=false}})",
        map.0
    ))
    .unwrap();
    let before = lua.world().btech.clone();
    let point: mlua::Table = lua
        .eval_callback(&format!("return btech.map.cargo_point({})", map.0))
        .unwrap();
    point.set("x", 100).unwrap();
    assert_eq!(lua.world().btech, before);
    let inspected: mlua::Table = lua
        .eval_callback(&format!(
            "return btech.map.inspect({}).cargo_transfer_point",
            map.0
        ))
        .unwrap();
    assert_eq!(inspected.get::<i32>("x").unwrap(), 1);
    inspected.set("reveal_hint", true).unwrap();
    assert_eq!(lua.world().btech, before);
    assert!(
        lua.eval_callback::<()>(&format!("btech.map.set_cargo_point(4,{},nil)", map.0))
            .is_err()
    );
    assert_eq!(lua.world().btech, before);
}

/// Existing table rows round-trip; edits preserve extra columns, reload preserves points and clearing deletes rows.
#[tokio::test]
async fn cargo_points_load_save_reload_and_purge_with_the_map() {
    use sqlx::Connection;
    let (_dir, config, mut world, map) = fixture().await;
    persistence::save(&config.database(), &world).await.unwrap();
    let mut db = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query("ALTER TABLE btech_map_cargo_configuration ADD COLUMN annotation TEXT NOT NULL DEFAULT 'keep'").execute(&mut db).await.unwrap();
    sqlx::query(
        "INSERT INTO btech_map_cargo_configuration(map_dbref,x,y,reveal_hint) VALUES(?,1,1,1)",
    )
    .bind(map.0)
    .execute(&mut db)
    .await
    .unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        world.btech.maps()[&map].cargo_transfer_point(),
        Some(BattleCargoTransferPoint {
            x: 1,
            y: 1,
            reveal_hint: true
        })
    );
    let point = BattleCargoTransferPoint {
        x: 2,
        y: 0,
        reveal_hint: false,
    };
    set_battle_cargo_transfer_point(&mut world, ObjectId(1), map, Some(point)).unwrap();
    reload_battle_map(
        &mut world,
        map,
        "reloaded",
        BattleMapAsset::from_cells("3 2\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    assert_eq!(world.btech.maps()[&map].cargo_transfer_point(), Some(point));
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT annotation FROM btech_map_cargo_configuration WHERE map_dbref=?"
        )
        .bind(map.0)
        .fetch_one(&mut db)
        .await
        .unwrap(),
        "keep"
    );
    set_battle_cargo_transfer_point(&mut world, ObjectId(1), map, None).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM btech_map_cargo_configuration")
            .fetch_one(&mut db)
            .await
            .unwrap(),
        0
    );
    set_battle_cargo_transfer_point(&mut world, ObjectId(1), map, Some(point)).unwrap();
    world
        .objects
        .get_mut(&map)
        .unwrap()
        .flags
        .insert(Flag::Going);
    persistence::save(&config.database(), &world).await.unwrap();
    persistence::repair(&config.database(), config.database.busy_timeout_ms, |raw| {
        dbck::plan(&world, raw, &config)
    })
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM btech_map_cargo_configuration")
            .fetch_one(&mut db)
            .await
            .unwrap(),
        0
    );
}

/// Malformed database points cannot enter runtime state even when the SQLite schema permits them.
#[tokio::test]
async fn malformed_saved_cargo_points_are_rejected() {
    use sqlx::Connection;
    let (_dir, config, world, map) = fixture().await;
    persistence::save(&config.database(), &world).await.unwrap();
    let mut db = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    for (owner, x, y, hint) in [
        (map.0, -1, 0, 0),
        (map.0, 3, 0, 0),
        (map.0, 0, 2, 0),
        (map.0, 0, 0, 2),
        (999999, 0, 0, 0),
    ] {
        sqlx::query(
            "INSERT INTO btech_map_cargo_configuration(map_dbref,x,y,reveal_hint) VALUES(?,?,?,?)",
        )
        .bind(owner)
        .bind(x)
        .bind(y)
        .bind(hint)
        .execute(&mut db)
        .await
        .unwrap();
        assert!(persistence::load(&config.database()).await.is_err());
        sqlx::query("DELETE FROM btech_map_cargo_configuration")
            .execute(&mut db)
            .await
            .unwrap();
    }
}
