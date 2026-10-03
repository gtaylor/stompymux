//! Generic decoration ownership preserves restoration metadata and unrelated saved fields.
use crate::support;
use sqlx::Connection;
use stompymux_rs::*;

#[tokio::test]
async fn generic_decoration_records_survive_reload_and_clear_on_resize() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Generic decorations".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "grid",
        BattleMapAsset::from_cells("2 2\n#1#1\n#1#1\n").unwrap(),
    )
    .unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut sql = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query("INSERT INTO btech_map_objects VALUES(?,2,5,1,1,1,126,123,456)")
        .bind(map.0)
        .execute(&mut sql)
        .await
        .unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    let definition =
        world.btech.maps()[&map].static_decorations(BattleStaticDecorationKind::Decoration)[&5];
    assert_eq!(definition.restored_terrain, Some(Terrain::Water));
    assert_eq!(definition.object, ObjectId(1));
    assert_eq!(definition.duration, 123);
    assert_eq!(definition.scalar, 456);
    assert_eq!(
        world.btech.maps()[&map].hex(1, 1).unwrap().terrain(),
        Terrain::Road
    );
    assert!(!map_fire_pending(&world));
    assert!(!map_smoke_pending(&world));
    let before = world.btech.clone();
    assert!(
        set_battle_static_decoration(
            &mut world,
            map,
            BattleStaticDecorationKind::Decoration,
            5,
            Some(BattleStaticDecoration {
                coordinate: BattleHexCoordinate { x: -1, y: 1 },
                ..definition
            })
        )
        .is_err()
    );
    assert_eq!(world.btech, before);
    set_battle_static_decoration(
        &mut world,
        map,
        BattleStaticDecorationKind::Decoration,
        5,
        Some(BattleStaticDecoration {
            coordinate: BattleHexCoordinate { x: 0, y: 1 },
            restored_terrain: Some(Terrain::Grassland),
            ..definition
        }),
    )
    .unwrap();
    set_battle_static_decoration(
        &mut world,
        map,
        BattleStaticDecorationKind::Decoration,
        9,
        Some(definition),
    )
    .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let payload: (i64,i64,i64,i64,i64,i64) = sqlx::query_as("SELECT x,y,object_dbref,data_char,data_short,data_int FROM btech_map_objects WHERE map_dbref=? AND object_type=2 AND ordinal=5").bind(map.0).fetch_one(&mut sql).await.unwrap();
    assert_eq!(payload, (0, 1, 1, 32, 123, 456));
    let copied: (i64, i64, i64) = sqlx::query_as("SELECT object_dbref,data_short,data_int FROM btech_map_objects WHERE map_dbref=? AND object_type=2 AND ordinal=9")
        .bind(map.0).fetch_one(&mut sql).await.unwrap();
    assert_eq!(copied, (1, 123, 456));
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    reload_battle_map(
        &mut world,
        map,
        "ice",
        BattleMapAsset::from_cells("2 2\n-3-3\n-3-3\n").unwrap(),
    )
    .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        world.btech.maps()[&map]
            .static_decorations(BattleStaticDecorationKind::Decoration)
            .len(),
        2
    );
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let response = support::run_text(&scripts, &config, ObjectId(1), 1, "setmapsize 2 2");
    let saved = scripts.world().clone();
    assert!(
        saved.btech.maps()[&map]
            .static_decorations(BattleStaticDecorationKind::Decoration)
            .is_empty(),
        "{response}"
    );
    assert_eq!(
        saved.btech.maps()[&map].hex(0, 1).unwrap().terrain(),
        Terrain::Ice
    );
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM btech_map_objects WHERE map_dbref=? AND object_type=2",
    )
    .bind(map.0)
    .fetch_one(&mut sql)
    .await
    .unwrap();
    assert_eq!(count, 0);
    for (x, y, terrain) in [(-1, 0, 32), (2, 0, 32), (0, 0, 0), (0, 0, 256)] {
        sqlx::query("DELETE FROM btech_map_objects WHERE map_dbref=? AND object_type=2")
            .bind(map.0)
            .execute(&mut sql)
            .await
            .unwrap();
        sqlx::query("INSERT INTO btech_map_objects VALUES(?,2,0,?,?,-1,?,0,0)")
            .bind(map.0)
            .bind(x)
            .bind(y)
            .bind(terrain)
            .execute(&mut sql)
            .await
            .unwrap();
        assert!(persistence::load(&config.database()).await.is_err());
    }
}
