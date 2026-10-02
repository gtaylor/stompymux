//! Resizing keeps overlapping tiles and valid unit positions with durable grid cropping.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Keep the operator outside the assigned cockpit.
fn operator(world: &mut World, config: &Config, map: ObjectId) -> ObjectId {
    let actor = world.create(config, "Resize operator".into(), Kind::Player);
    world
        .objects
        .get_mut(&actor)
        .unwrap()
        .flags
        .insert(Flag::Wizard);
    world.objects.get_mut(&actor).unwrap().location = Some(map);
    actor
}

/// Expand and crop an already saved battlefield through both interfaces for every chassis.
#[tokio::test]
async fn resizing_preserves_units_and_replays_native_lua() {
    for source in firing::templates() {
        let (_dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(&source, None, include_str!("../game/mechs/AS7-D.toml"))
                .await;
        let map = world.btech.units()[&unit].map.unwrap();
        let actor = operator(&mut world, &config, map);
        persistence::save(&config.database(), &world).await.unwrap();
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        for (width, height) in [(3, 14), (2, 12), (1, 12)] {
            let before = lua.world().btech.clone();
            let call = format!("btech.map.resize({},{},{width},{height})", actor.0, map.0);
            assert!(
                lua.eval_callback::<()>(&format!("{call}; error('abort resize')"))
                    .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            assert!(
                lua.eval_callback::<bool>(&format!("return {call}"))
                    .unwrap()
            );
            lua.drain_outbox();
            let output = support::run_text(
                &native,
                &config,
                actor,
                1,
                &format!("setmapsize {width} {height}"),
            );
            assert!(output.contains("Size set."), "{output}");
            let saved = native.world().clone();
            assert_eq!(saved.btech, lua.world().btech);
            assert_eq!(saved.btech.constructed_units(), before.constructed_units());
            assert_eq!(saved.btech.vehicles(), before.vehicles());
            assert_eq!(
                (
                    saved.btech.maps()[&map].width,
                    saved.btech.maps()[&map].height
                ),
                (width, height)
            );
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                saved.btech
            );
        }
        let before = native.world().btech.clone();
        for (width, height) in [(0, 12), (1001, 12), (1, 10), (-1, 12)] {
            assert!(resize_battle_map_action(&native, &config, actor, map, width, height).is_err());
            assert_eq!(native.world().btech, before);
            assert!(native.drain_outbox().is_empty());
        }
    }
}

/// Equal-area reshaping still rewrites coordinates, and object cleanup applies even at equal size.
#[tokio::test]
async fn reshape_persists_grid_and_removes_map_objects() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Resize field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "grid",
        BattleMapAsset::parse("1 4\n.0\n.0\n.0\n.0\n").unwrap(),
    )
    .unwrap();
    let actor = operator(&mut world, &config, map);
    set_battle_landing_exclusion(
        &mut world,
        map,
        7,
        Some(BattleLandingExclusion {
            coordinate: BattleHexCoordinate { x: 0, y: 3 },
            radius: 2,
            exempt_team: 1,
            owner: actor,
            data_short: 0,
        }),
    )
    .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    use sqlx::Connection;
    let mut sql =
        sqlx::SqliteConnection::connect(&format!("sqlite:{}", config.database().display()))
            .await
            .unwrap();
    sqlx::query("ALTER TABLE btech_map_hexes ADD COLUMN resize_marker TEXT NOT NULL DEFAULT 'new'")
        .execute(&mut sql)
        .await
        .unwrap();
    sqlx::query("UPDATE btech_map_hexes SET resize_marker='keep' WHERE map_dbref=?")
        .bind(map.0)
        .execute(&mut sql)
        .await
        .unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    resize_battle_map_action(&scripts, &config, actor, map, 2, 2).unwrap();
    let saved = scripts.world().clone();
    assert!(saved.btech.maps()[&map].landing_exclusions().is_empty());
    assert_eq!(
        saved.btech.maps()[&map].base_hex(1, 1).unwrap(),
        BattleHex {
            terrain: Terrain::Grassland,
            elevation: 0
        }
    );
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
    let rows: Vec<(i64, i64, String)> = sqlx::query_as(
        "SELECT x,y,resize_marker FROM btech_map_hexes WHERE map_dbref=? ORDER BY x,y",
    )
    .bind(map.0)
    .fetch_all(&mut sql)
    .await
    .unwrap();
    assert_eq!(
        rows,
        vec![
            (0, 0, "keep".into()),
            (0, 1, "keep".into()),
            (1, 0, "new".into()),
            (1, 1, "new".into())
        ]
    );
}

/// Resizing preserves visible fire/smoke tiles while removing timers and reciprocal building exits.
#[tokio::test]
async fn resize_clears_effects_and_building_routes() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Exterior".into(), Kind::Room);
    let interior = world.create(&config, "Interior".into(), Kind::Room);
    for id in [map, interior] {
        create_battle_map(
            &mut world,
            id,
            "grid",
            BattleMapAsset::parse("2 2\n`2#1\n.0.0\n").unwrap(),
        )
        .unwrap();
    }
    let actor = operator(&mut world, &config, map);
    set_building_entrance(
        &mut world,
        map,
        4,
        Some(BattleBuildingEntrance {
            coordinate: BattleHexCoordinate { x: 1, y: 1 },
            interior,
            data_char: 0,
            data_short: 0,
            data_int: 0,
        }),
    )
    .unwrap();
    set_battle_building_exit(&mut world, interior, 5, Some(map)).unwrap();
    for (x, kind) in [
        (0, BattleDecorationKind::Fire),
        (1, BattleDecorationKind::Smoke),
    ] {
        set_map_decoration(
            &mut world,
            map,
            BattleHexCoordinate { x, y: 0 },
            Some(BattleDecoration::new(kind, 30, None)),
        )
        .unwrap();
    }
    persistence::save(&config.database(), &world).await.unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    resize_battle_map_action(&scripts, &config, actor, map, 2, 2).unwrap();
    let saved = scripts.world().clone();
    let field = &saved.btech.maps()[&map];
    assert!(field.building_entrances().is_empty());
    assert!(saved.btech.maps()[&interior].building_exits().is_empty());
    for (x, terrain, elevation) in [(0, Terrain::Fire, 2), (1, Terrain::Smoke, 1)] {
        assert_eq!(
            field.base_hex(x, 0).unwrap(),
            BattleHex { terrain, elevation }
        );
        assert!(
            field
                .decoration(BattleHexCoordinate { x: x as i32, y: 0 })
                .unwrap()
                .is_none()
        );
    }
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
}
