//! Operator mine placement shares newest-first traversal, durable identities and atomic output.
use crate::support;
use crate::support::btech_firing as firing;
use sqlx::Connection;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

#[tokio::test]
async fn native_lua_mines_share_clamping_order_and_leave_every_chassis_untouched() {
    for source in firing::templates() {
        let (_dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(&source, None, include_str!("../game/mechs/AS7-D.toml"))
                .await;
        let map = world.btech.units()[&unit].map.unwrap();
        let actor = world.create(&config, "Mine operator".into(), Kind::Player);
        world
            .objects
            .get_mut(&actor)
            .unwrap()
            .flags
            .insert(Flag::Wizard);
        world.objects.get_mut(&actor).unwrap().location = Some(map);
        let before = world.btech.clone();
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        for (kind, strength, extra) in [
            ("Standard", i32::MIN, i32::MIN),
            ("Inferno", i32::MAX, i32::MAX),
            ("Command", 0, 17),
            ("Vibra", 25, 80),
            ("Trigger", 1, 3),
        ] {
            let output = support::run_text(
                &native,
                &config,
                actor,
                1,
                &format!("addmine 0 0 {kind} {strength} {extra}"),
            );
            assert!(
                output.contains(&format!(
                    "{kind} mine added to (0,0) (strength: {strength} / extra: {extra})"
                )),
                "{output}"
            );
            let slot: u32 = lua
                .eval_callback(&format!(
                    "return btech.map.add_mine({},{},0,0,'{}',{strength},{extra})",
                    actor.0,
                    map.0,
                    kind.to_ascii_uppercase()
                ))
                .unwrap();
            let stored = lua.world().btech.maps()[&map].minefields()[&slot];
            assert_eq!(
                stored.strength,
                strength.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
            );
            assert_eq!(stored.extra, extra);
            assert_eq!(stored.owner, actor);
        }
        assert_eq!(native.world().btech, lua.world().btech);
        assert_eq!(
            native.world().btech.maps()[&map]
                .ordered_minefields()
                .map(|(slot, _)| *slot)
                .collect::<Vec<_>>(),
            vec![4, 3, 2, 1, 0]
        );
        assert_eq!(
            native.world().btech.constructed_units(),
            before.constructed_units()
        );
        assert_eq!(native.world().btech.vehicles(), before.vehicles());
        let saved = native.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
    }
}

#[tokio::test]
async fn prepending_preserves_auxiliary_columns_and_order_survives_removal() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Mines".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "mine",
        MapAsset::from_cells("2 2\n.0.0\n.0.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let mine = BattleMinefield {
        coordinate: HexCoordinate { x: 1, y: 1 },
        kind: BattleMineKind::Standard,
        strength: 10,
        extra: 0,
        owner: ObjectId(1),
    };
    for slot in [7, 9] {
        set_minefield(&mut world, map, slot, Some(mine)).unwrap();
    }
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
    sqlx::query("UPDATE btech_map_objects SET note='original-' || ordinal WHERE object_type=3")
        .execute(&mut sql)
        .await
        .unwrap();
    let slot = insert_minefield(
        &mut world,
        map,
        BattleMinefield {
            kind: BattleMineKind::Inferno,
            ..mine
        },
    )
    .unwrap();
    assert_eq!(slot, 0);
    assert_eq!(
        world.btech.maps()[&map]
            .ordered_minefields()
            .map(|(id, _)| *id)
            .collect::<Vec<_>>(),
        vec![0, 7, 9]
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT ordinal,note FROM btech_map_objects WHERE object_type=3 ORDER BY ordinal",
    )
    .fetch_all(&mut sql)
    .await
    .unwrap();
    assert_eq!(
        rows,
        vec![
            (0, "new".into()),
            (7, "original-7".into()),
            (9, "original-9".into())
        ]
    );
    set_minefield(&mut world, map, 7, None).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    assert_eq!(
        loaded.btech.maps()[&map]
            .ordered_minefields()
            .map(|(id, _)| *id)
            .collect::<Vec<_>>(),
        vec![0, 9]
    );
    sqlx::query("UPDATE btech_mine_order SET ordinal=0")
        .execute(&mut sql)
        .await
        .unwrap();
    assert!(persistence::load(&config.database()).await.is_err());
}

#[tokio::test]
async fn rejected_or_aborted_placement_does_not_change_order_or_publish_output() {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Mines".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "mine",
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    for (input, expected) in [
        ("addmine 0 0 bad nope", "Invalid number!"),
        ("addmine 1 0 standard 1", "X,Y out of range!"),
        ("addmine 0 0 sta 1", "Invalid mine type!"),
        ("addmine 0 0 standard", "Invalid arguments!"),
    ] {
        let output = support::run_text(&scripts, &config, ObjectId(1), 1, input);
        assert!(output.contains(expected), "{output}");
        assert_eq!(scripts.world().btech, before);
    }
    for args in [
        "1,0,'standard',1",
        "0,0,'sta',1",
        "0,0,'standard',2147483648",
    ] {
        assert!(
            scripts
                .eval_callback::<()>(&format!("btech.map.add_mine(1,{}, {args})", map.0))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
    }
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.map.add_mine(1,{},0,0,'standard',1);error('abort')",
                map.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    let placement = BattleMinePlacement {
        coordinate: HexCoordinate { x: 0, y: 0 },
        kind: BattleMineKind::Standard,
        strength: 10,
        extra: 0,
    };
    assert!(add_battle_mine_action(&scripts, &config, map, map, placement).is_err());
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
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(scripts.world().clone()))).unwrap();
    assert!(add_battle_mine_action(&scripts, &config, ObjectId(1), map, placement).is_err());
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
}

/// Artillery prepends its deposit once; later shells on the same hex do not stack fields.
#[tokio::test]
async fn artillery_mines_precede_existing_records_without_replacing_them() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Mines".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "mine",
        MapAsset::from_cells("2 2\n.0.0\n.0.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let coordinate = HexCoordinate { x: 0, y: 0 };
    set_minefield(
        &mut world,
        map,
        9,
        Some(BattleMinefield {
            coordinate: HexCoordinate { x: 1, y: 1 },
            kind: BattleMineKind::Command,
            strength: 20,
            extra: 4,
            owner: ObjectId(1),
        }),
    )
    .unwrap();
    set_map_decoration(
        &mut world,
        map,
        coordinate,
        Some(BattleDecoration::new(DecorationKind::Fire, 0, None)),
    )
    .unwrap();
    let rules = BattleFallRules {
        vehicle_impact: BattleVehicleImpactRules::STANDARD,
        stacking: BattleStackingRules::STANDARD,
        stagger: BattleStaggerMode::Retain,
        hit: BattleHitRules {
            inferno_penalty: false,
            exile_stun_mode: 0,
        },
        extended_piloting: true,
        toughness: false,
    };
    for repetition in 0..3 {
        let mut flight = BattleArtilleryFlight::new(
            coordinate,
            coordinate,
            BattleWeapon::Thumper,
            BattleArtilleryMode::Mine,
            true,
        )
        .unwrap();
        let mut arrival = None;
        for _ in 0..10 {
            arrival = advance_artillery_flight(&mut world, map, &mut flight, rules).unwrap();
        }
        assert_eq!(arrival.unwrap().mines.len(), usize::from(repetition == 0));
        assert_eq!(
            world.btech.maps()[&map]
                .ordered_minefields()
                .map(|(id, _)| *id)
                .collect::<Vec<_>>(),
            vec![0, 9]
        );
        assert_eq!(world.btech.maps()[&map].minefields()[&9].owner, ObjectId(1));
        persistence::save(&config.database(), &world).await.unwrap();
        world = persistence::load(&config.database()).await.unwrap();
    }
}
