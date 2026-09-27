//! Landing-block commands share circular suitability and persist signed radii without touching units.
use crate::support;
use crate::support::btech_map_objects as map_objects;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Newly authored blocks prepend without moving imported rows or their extension columns.
#[tokio::test]
async fn landing_order_preserves_identity_replays_and_rejects_corruption() {
    use sqlx::Connection;
    let (_dir, config, mut world, map, _) = map_objects::fixture().await;
    let slots = |world: &World| {
        world.btech.maps()[&map]
            .ordered_landing_exclusions()
            .map(|(slot, _)| *slot)
            .collect::<Vec<_>>()
    };
    assert_eq!(slots(&world), vec![0, 4]);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut sql = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query("ALTER TABLE btech_map_objects ADD COLUMN note TEXT NOT NULL DEFAULT 'new'")
        .execute(&mut sql)
        .await
        .unwrap();
    sqlx::query("UPDATE btech_map_objects SET note='imported-' || ordinal WHERE object_type=9")
        .execute(&mut sql)
        .await
        .unwrap();
    // Reference imports have ordinal traversal and no Rust order rows.
    sqlx::query("DELETE FROM btech_landing_order")
        .execute(&mut sql)
        .await
        .unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    assert_eq!(slots(&world), vec![0, 4]);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let call = format!("btech.map.add_block(1,{},0,2,12,7)", map.0);
    assert!(
        scripts
            .eval_callback::<()>(&format!("{call}; error('abort')"))
            .is_err()
    );
    assert_eq!(scripts.world().btech, world.btech);
    assert!(scripts.drain_outbox().is_empty());
    assert_eq!(
        scripts
            .eval_callback::<u32>(&format!("return {call}"))
            .unwrap(),
        1
    );
    support::run_text(&scripts, &config, ObjectId(1), 1, "addblock 2 0 13");
    assert_eq!(slots(&scripts.world()), vec![2, 1, 0, 4]);
    let report = support::run_text(&scripts, &config, ObjectId(1), 1, "list objs");
    let coordinates: Vec<_> = report
        .lines()
        .filter(|line| line.contains("BLZ"))
        .map(|line| line.split_whitespace().take(2).collect::<Vec<_>>())
        .collect();
    assert_eq!(
        coordinates,
        vec![
            vec!["2", "0"],
            vec!["0", "2"],
            vec!["1", "1"],
            vec!["1", "1"]
        ]
    );
    world = scripts.world().clone();
    let mut edited = world.btech.maps()[&map].landing_exclusions()[&2];
    edited.radius = 99;
    set_battle_landing_exclusion(&mut world, map, 2, Some(edited)).unwrap();
    assert_eq!(slots(&world), vec![2, 1, 0, 4]);
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    let notes: Vec<(i64, String)> = sqlx::query_as(
        "SELECT ordinal,note FROM btech_map_objects WHERE object_type=9 ORDER BY ordinal",
    )
    .fetch_all(&mut sql)
    .await
    .unwrap();
    assert_eq!(
        notes,
        vec![
            (0, "imported-0".into()),
            (1, "new".into()),
            (2, "new".into()),
            (4, "imported-4".into())
        ]
    );
    set_battle_landing_exclusion(&mut world, map, 1, None).unwrap();
    assert_eq!(slots(&world), vec![2, 0, 4]);
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    for malformed in [&[2, 2, 4][..], &[2, 0], &[2, 0, 99], &[2, 0, 4]] {
        store_landing_order(&mut sql, map, malformed).await;
        if malformed != [2, 0, 4] {
            assert!(persistence::load(&config.database()).await.is_err());
        }
    }
    world
        .objects
        .get_mut(&map)
        .unwrap()
        .flags
        .insert(Flag::Going);
    persistence::repair(&config.database(), 5000, |raw| {
        dbck::plan(&world, raw, &config)
    })
    .await
    .unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert!(!restored.btech.maps().contains_key(&map));
    for query in [
        "SELECT count(*) FROM btech_landing_order WHERE map_dbref=?",
        "SELECT count(*) FROM btech_mine_order WHERE map_dbref=?",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(query)
                .bind(map.0)
                .fetch_one(&mut sql)
                .await
                .unwrap(),
            0
        );
    }
}

/// Four bounded fields share numeric failures; trailing fields are ignored before validation.
#[tokio::test]
async fn addblock_native_argument_boundaries_and_replies() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Landing field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "grid",
        BattleMapAsset::parse("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    for (arguments, expected) in [
        ("", "Invalid arguments!"),
        ("1 1", "Invalid arguments!"),
        ("1\u{a0}1 2", "Invalid arguments!"),
        ("bad 1 2", "Invalid number!"),
        ("1 bad 2", "Invalid number!"),
        ("1 1 bad", "Invalid number!"),
        ("1 1 2 bad", "Invalid number!"),
        ("2147483648 1 2", "Invalid number!"),
        ("1 -2147483649 2", "Invalid number!"),
        ("1 1 2147483648", "Invalid number!"),
        ("1 1 2 2147483648", "Invalid number!"),
        ("1 1 2\u{a0} 0", "Invalid number!"),
        ("-1 1 2 bad", "Invalid number!"),
        ("-1 1 2", "X,Y out of range!"),
        ("1 -1 2", "X,Y out of range!"),
        ("3 1 2", "X,Y out of range!"),
        ("1 3 2", "X,Y out of range!"),
    ] {
        let output = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("addblock {arguments}"),
        );
        assert_eq!(text::plain(&output).trim(), expected, "{arguments:?}");
        assert_eq!(scripts.world().btech, world.btech);
    }
    for arguments in ["+1 1 -0", "\t1\t1\t0\t+0", "1 1 0 0 bad ignored"] {
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let output = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("addblock {arguments}"),
        );
        assert_eq!(
            text::plain(&output).trim(),
            "Landingzone-block added to 1,1 (distance: 0)"
        );
        lua.eval_callback::<u32>(&format!("return btech.map.add_block(1,{},1,1,0,0)", map.0))
            .unwrap();
        assert_eq!(native.world().btech, lua.world().btech);
    }
}

/// Signed radius boundaries and optional team exemptions agree across native and Lua entry points.
#[tokio::test]
async fn addblock_radius_team_and_restart() {
    for radius in [i32::MIN, -1, 0, 1, i32::MAX] {
        for team in [0, 7, 256, -1] {
            let (_dir, config, mut world) = support::isolated_world().await;
            let map = world.create(&config, "Landing field".into(), Kind::Room);
            create_battle_map(
                &mut world,
                map,
                "grid",
                BattleMapAsset::parse("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
            )
            .unwrap();
            world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
            persistence::save(&config.database(), &world).await.unwrap();
            let before = world.btech.clone();
            let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let call = format!("btech.map.add_block(1,{},1,1,{radius},{team})", map.0);
            assert!(
                lua.eval_callback::<()>(&format!("{call}; error('abort block')"))
                    .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            let slot: u32 = lua.eval_callback(&format!("return {call}")).unwrap();
            assert_eq!(slot, 0);
            let output = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("addblock 1 1 {radius} {team}"),
            );
            assert!(output.contains("Landingzone-block added"), "{output}");
            assert_eq!(native.world().btech, lua.world().btech);
            let saved = native.world().clone();
            let zone = saved.btech.maps()[&map].landing_exclusions()[&slot];
            assert_eq!(
                (zone.radius, zone.exempt_team, zone.owner),
                (i64::from(radius), team, ObjectId(1))
            );
            let expected = if radius >= 0 && team != 7 {
                BattleLandingSuitability::Blocked
            } else {
                BattleLandingSuitability::Ready
            };
            assert_eq!(
                saved.btech.maps()[&map]
                    .landing_suitability(BattleHexCoordinate { x: 1, y: 1 }, 7)
                    .unwrap(),
                expected
            );
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                saved.btech
            );
            let second = add_battle_landing_exclusion_action(
                &native,
                &config,
                ObjectId(1),
                map,
                BattleHexCoordinate { x: 1, y: 1 },
                radius,
                team,
            )
            .unwrap();
            assert_eq!(second, 1);
        }
    }
}

/// Imported scalar radii retain their full signed width; malformed commands never add a restriction.
#[tokio::test]
async fn addblock_validation_and_full_width_saved_radii() {
    use sqlx::Connection;
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Landing field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "grid",
        BattleMapAsset::parse("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for command in [
        "addblock",
        "addblock -1 1 2",
        "addblock 3 1 2",
        "addblock 1 1 2147483648",
        "addblock/no 1 1 2",
    ] {
        support::run_text(&scripts, &config, ObjectId(1), 1, command);
        assert_eq!(scripts.world().btech, before);
    }
    assert!(
        add_battle_landing_exclusion_action(
            &scripts,
            &config,
            ObjectId(2),
            map,
            BattleHexCoordinate { x: 1, y: 1 },
            1,
            0
        )
        .is_err()
    );
    support::run_text(&scripts, &config, ObjectId(1), 1, "addblock 1 1 1");
    let saved = scripts.world().clone();
    assert_eq!(
        saved.btech.maps()[&map].landing_exclusions()[&0].exempt_team,
        0
    );
    persistence::save(&config.database(), &saved).await.unwrap();
    let mut sql =
        sqlx::SqliteConnection::connect(&format!("sqlite:{}", config.database().display()))
            .await
            .unwrap();
    for radius in [i64::MIN, i64::from(u32::MAX), i64::MAX] {
        sqlx::query("UPDATE btech_map_objects SET data_int=? WHERE map_dbref=? AND object_type=9")
            .bind(radius)
            .bind(map.0)
            .execute(&mut sql)
            .await
            .unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            loaded.btech.maps()[&map].landing_exclusions()[&0].radius,
            radius
        );
        let expected = if radius < 0 {
            BattleLandingSuitability::Ready
        } else {
            BattleLandingSuitability::Blocked
        };
        assert_eq!(
            loaded.btech.maps()[&map]
                .landing_suitability(BattleHexCoordinate { x: 1, y: 1 }, 0)
                .unwrap(),
            expected
        );
    }
}

/// Replace a map's stored landing traversal with `order`, bypassing the game's validation.
async fn store_landing_order(sql: &mut sqlx::SqliteConnection, map: ObjectId, order: &[u32]) {
    sqlx::query("DELETE FROM btech_landing_order WHERE map_dbref=?")
        .bind(map.0)
        .execute(&mut *sql)
        .await
        .unwrap();
    for (position, ordinal) in order.iter().enumerate() {
        sqlx::query("INSERT INTO btech_landing_order(map_dbref,position,ordinal) VALUES(?,?,?)")
            .bind(map.0)
            .bind(position as i64)
            .bind(i64::from(*ordinal))
            .execute(&mut *sql)
            .await
            .unwrap();
    }
}
