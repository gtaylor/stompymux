//! Terrain dictionary creation, explicit recovery, corruption handling and transactional delivery.
use crate::support;
use sqlx::{Connection, SqliteConnection};
use std::{cell::Cell, rc::Rc};
use stompymux_rs::{
    BattleMapAsset, Config, Flag, Kind, ObjectId, Scripts, ShutdownRequest, Terrain, World,
    create_battle_map, dbck, persistence, reload_battle_map,
};

const SOURCE: &str = "3 2\n.0~2'1\n#0-3^9\n32: 75 -12\n";
const RELOAD: &str = "3 2\n.0~2'1\n#0-3%4\n64: 80 15\n";

/// An isolated schema-8 game, two equal-size assets, and an unregistered map container.
async fn fixture() -> (tempfile::TempDir, Config, World, ObjectId, SqliteConnection) {
    let (dir, config, mut world) = support::isolated_world().await;
    world.accounts.get_mut(&ObjectId(1)).unwrap().hash =
        Some(stompymux_rs::accounts::hash("secret", &config).unwrap());
    std::fs::create_dir_all(dir.path().join("maps")).unwrap();
    std::fs::write(dir.path().join("maps/asymmetric.map"), SOURCE).unwrap();
    std::fs::write(dir.path().join("maps/reload.map"), RELOAD).unwrap();
    let id = world.create(&config, "Terrain lab".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().location = Some(ObjectId(config.start()));
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    persistence::save(&config.database(), &world).await.unwrap();
    let sql = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(config.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    (dir, config, world, id, sql)
}

/// Count the stored terrain dictionary rows, headers and codes together.
async fn terrain_rows(sql: &mut SqliteConnection) -> i64 {
    sqlx::query_scalar("SELECT (SELECT count(*) FROM btech_map_terrain) + (SELECT count(*) FROM btech_map_terrain_codes)").fetch_one(sql).await.unwrap()
}

/// Persist the asymmetric map through the same domain operation used by native commands and Lua.
async fn create(config: &Config, world: &mut World, id: ObjectId) {
    create_battle_map(
        world,
        id,
        "asymmetric.map",
        BattleMapAsset::parse(SOURCE).unwrap(),
    )
    .unwrap();
    persistence::save(&config.database(), world).await.unwrap();
}

#[tokio::test]
async fn dictionary_round_trip_is_per_map_and_preserves_unowned_columns() {
    let (_dir, config, mut world, id, mut sql) = fixture().await;
    assert_eq!(terrain_rows(&mut sql).await, 0);
    create(&config, &mut world, id).await;
    assert!(terrain_rows(&mut sql).await > 0);
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    let map = &loaded.btech.maps()[&id];
    assert!(map.terrain_ready());
    assert_eq!(map.hex(1, 0).unwrap().surface_height(), -2);
    assert_eq!(map.hex(1, 1).unwrap().surface_height(), -3);
    assert_eq!(map.hex(2, 1).unwrap().terrain, Terrain::Mountains);
    assert_eq!(
        (
            map.width,
            map.height,
            map.gravity,
            map.temperature,
            map.flags
        ),
        (3, 2, 75, -12, 32)
    );
    assert!(map.hex(-1, 0).is_err());
    assert!(map.hex(i64::MAX, 0).is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT encoding_version FROM btech_map_terrain WHERE map_dbref=?"
        )
        .bind(id.0)
        .fetch_one(&mut sql)
        .await
        .unwrap(),
        1
    );
    sqlx::raw_sql("ALTER TABLE btech_map_hexes ADD COLUMN opaque BLOB DEFAULT x'00ff42'; ALTER TABLE btech_map_terrain_codes ADD COLUMN opaque TEXT DEFAULT 'dictionary extension'; ALTER TABLE btech_maps ADD COLUMN opaque TEXT DEFAULT 'map extension'; CREATE TRIGGER prohibit_hex_delete BEFORE DELETE ON btech_map_hexes BEGIN SELECT RAISE(ABORT,'grid rows must retain extension data'); END;").execute(&mut sql).await.unwrap();
    let code: i64 = sqlx::query_scalar(
        "SELECT code FROM btech_map_terrain_codes WHERE map_dbref=? AND terrain='~'",
    )
    .bind(id.0)
    .fetch_one(&mut sql)
    .await
    .unwrap();
    let before = loaded.clone();
    reload_battle_map(
        &mut loaded,
        id,
        "reload.map",
        BattleMapAsset::parse(RELOAD).unwrap(),
    )
    .unwrap();
    assert_eq!(
        before.btech.maps()[&id].hex(2, 1).unwrap().terrain,
        Terrain::Mountains
    );
    persistence::save(&config.database(), &loaded)
        .await
        .unwrap();
    let map = persistence::load(&config.database())
        .await
        .unwrap()
        .btech
        .maps()[&id]
        .clone();
    assert_eq!(map.hex(2, 1).unwrap().terrain, Terrain::Rough);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT code FROM btech_map_terrain_codes WHERE map_dbref=? AND terrain='~'"
        )
        .bind(id.0)
        .fetch_one(&mut sql)
        .await
        .unwrap(),
        code
    );
    assert_eq!(
        sqlx::query_scalar::<_, Vec<u8>>(
            "SELECT opaque FROM btech_map_hexes WHERE map_dbref=? AND x=2 AND y=1"
        )
        .bind(id.0)
        .fetch_one(&mut sql)
        .await
        .unwrap(),
        [0, 255, 66]
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT opaque FROM btech_map_terrain_codes WHERE map_dbref=? AND code=?"
        )
        .bind(id.0)
        .bind(code)
        .fetch_one(&mut sql)
        .await
        .unwrap(),
        "dictionary extension"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT opaque FROM btech_maps WHERE dbref=?")
            .bind(id.0)
            .fetch_one(&mut sql)
            .await
            .unwrap(),
        "map extension"
    );
    sqlx::query("DROP TRIGGER prohibit_hex_delete")
        .execute(&mut sql)
        .await
        .unwrap();
    // Dictionary namespaces do not depend on the order that other maps are loaded.
    let second = loaded.create(&config, "Second map".into(), Kind::Room);
    create_battle_map(
        &mut loaded,
        second,
        "water.map",
        BattleMapAsset::parse("1 1\n~9\n").unwrap(),
    )
    .unwrap();
    persistence::save(&config.database(), &loaded)
        .await
        .unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        loaded.btech.maps()[&second]
            .hex(0, 0)
            .unwrap()
            .surface_height(),
        -9
    );
    assert_eq!(
        loaded.btech.maps()[&id].hex(1, 0).unwrap().surface_height(),
        -2
    );
    sql.close().await.unwrap();
}

#[tokio::test]
async fn ambiguous_maps_require_explicit_reload_and_purge_removes_dictionaries() {
    let (_dir, config, mut world, id, mut sql) = fixture().await;
    create(&config, &mut world, id).await;
    // A map without a dictionary header, as a reference import leaves it, stays ambiguous.
    sqlx::raw_sql("DELETE FROM btech_map_terrain_codes; DELETE FROM btech_map_terrain; UPDATE btech_map_hexes SET value=231;").execute(&mut sql).await.unwrap();
    let mut world = persistence::load(&config.database()).await.unwrap();
    assert!(!world.btech.maps()[&id].terrain_ready());
    assert!(
        world.btech.maps()[&id]
            .hex(0, 0)
            .unwrap_err()
            .to_string()
            .contains("ambiguous")
    );
    world.objects.get_mut(&id).unwrap().description = Some("unrelated MUX change".into());
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(terrain_rows(&mut sql).await, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT min(value) FROM btech_map_hexes")
            .fetch_one(&mut sql)
            .await
            .unwrap(),
        231
    );
    let scripts = Scripts::new(&config, Rc::new(std::cell::RefCell::new(world))).unwrap();
    let text = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        &format!("@btech map-reload #{}=asymmetric.map", id.0),
    );
    assert!(text.contains("terrain reloaded"), "{text}");
    let candidate = scripts.world().clone();
    persistence::save(&config.database(), &candidate)
        .await
        .unwrap();
    let mut world = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        world.btech.maps()[&id].hex(1, 0).unwrap().terrain,
        Terrain::Water
    );
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::Going);
    persistence::repair(&config.database(), 5000, |raw| {
        dbck::plan(&world, raw, &config)
    })
    .await
    .unwrap();
    assert!(
        !persistence::load(&config.database())
            .await
            .unwrap()
            .btech
            .maps()
            .contains_key(&id)
    );
    for table in ["btech_map_terrain", "btech_map_terrain_codes"] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(format!(
                "SELECT count(*) FROM {table}"
            )))
            .fetch_one(&mut sql)
            .await
            .unwrap(),
            0
        );
    }
    sql.close().await.unwrap();
}

#[tokio::test]
async fn corrupt_dictionary_backed_maps_never_fall_back_to_guessed_or_asset_terrain() {
    let (_dir, config, mut world, id, mut sql) = fixture().await;
    create(&config, &mut world, id).await;
    sqlx::raw_sql(
        "PRAGMA ignore_check_constraints=ON; UPDATE btech_map_terrain SET encoding_version=2;",
    )
    .execute(&mut sql)
    .await
    .unwrap();
    assert!(
        persistence::load(&config.database())
            .await
            .unwrap_err()
            .to_string()
            .contains("Unsupported terrain encoding")
    );
    sqlx::query("UPDATE btech_map_terrain SET encoding_version=1")
        .execute(&mut sql)
        .await
        .unwrap();
    let code: i64 =
        sqlx::query_scalar("SELECT value FROM btech_map_hexes WHERE map_dbref=? AND x=0 AND y=0")
            .bind(id.0)
            .fetch_one(&mut sql)
            .await
            .unwrap();
    sqlx::query("UPDATE btech_map_hexes SET value=255 WHERE map_dbref=? AND x=0 AND y=0")
        .bind(id.0)
        .execute(&mut sql)
        .await
        .unwrap();
    assert!(
        persistence::load(&config.database())
            .await
            .unwrap_err()
            .to_string()
            .contains("Unknown terrain code")
    );
    sqlx::query("DELETE FROM btech_map_hexes WHERE map_dbref=? AND x=0 AND y=0")
        .bind(id.0)
        .execute(&mut sql)
        .await
        .unwrap();
    assert!(
        persistence::load(&config.database())
            .await
            .unwrap_err()
            .to_string()
            .contains("Incomplete terrain grid")
    );
    sqlx::query("INSERT INTO btech_map_hexes VALUES(?,0,0,?)")
        .bind(id.0)
        .bind(code)
        .execute(&mut sql)
        .await
        .unwrap();
    persistence::load(&config.database()).await.unwrap();
    sqlx::query("DROP TABLE btech_map_terrain_codes")
        .execute(&mut sql)
        .await
        .unwrap();
    assert!(
        persistence::load(&config.database())
            .await
            .unwrap_err()
            .to_string()
            .contains("lacks the tables btech_map_terrain_codes")
    );
    sql.close().await.unwrap();
}

#[tokio::test]
async fn lua_map_operations_participate_in_callback_rollback_and_checking_guards() {
    let (_dir, config, world, id, sql) = fixture().await;
    sql.close().await.unwrap();
    let scripts = Scripts::new(&config, Rc::new(std::cell::RefCell::new(world))).unwrap();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.map.create({},'asymmetric.map'); error('abort map')",
                id.0
            ))
            .is_err()
    );
    assert!(!scripts.world().btech.maps().contains_key(&id));
    scripts
        .eval_callback::<()>(&format!(
            "assert(btech.map.create({},'asymmetric.map'))",
            id.0
        ))
        .unwrap();
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.map.reload({},'reload.map'); error('abort reload')",
                id.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    let (terrain,elevation,ready,has_tiles):(String,i64,bool,bool)=scripts.eval_callback(&format!("local m=btech.map.inspect({0}); local h=btech.map.hex({0},1,0); return h.terrain,h.elevation,m.terrain_ready,m.terrain~=nil",id.0)).unwrap();
    assert_eq!(
        (terrain.as_str(), elevation, ready, has_tiles),
        ("water", 2, true, false)
    );
    let checking = scripts
        .from_sources_for_inspection(
            &config,
            stompymux_rs::help::HelpIndex::load(&config).unwrap(),
            std::sync::Arc::new(stompymux_rs::LuaSources::read(&config).unwrap()),
            stompymux_rs::RuntimeMode::Checking,
        )
        .unwrap();
    let code: String = checking
        .inspect_lua()
        .load(format!(
            "local ok,e=pcall(btech.map.reload,{},'reload.map'); assert(not ok); return e.code",
            id.0
        ))
        .eval()
        .unwrap();
    assert_eq!(code, "mux.unavailable.checking");
    let mut world = scripts.world().clone();
    assert!(
        reload_battle_map(
            &mut world,
            id,
            "different.map",
            BattleMapAsset::parse("1 1\n.0\n").unwrap()
        )
        .is_err()
    );
    assert_eq!(world.btech, before);
}

#[tokio::test(flavor = "current_thread")]
async fn server_rolls_back_schema_and_output_on_failed_creation_then_recovers() {
    tokio::task::LocalSet::new().run_until(async {
        let (_dir,config,_world,id,mut sql)=fixture().await;
        sqlx::raw_sql("CREATE TRIGGER reject_tiles BEFORE INSERT ON btech_map_hexes BEGIN SELECT RAISE(ABORT,'tile failure'); END;").execute(&mut sql).await.unwrap();
        let (address,shutdown,task,_lua)=support::start(&config,Rc::new(Cell::new(0))).await;
        let mut client=support::Client::connect(address,1).await;
        client.send(&format!("@btech map-create #{}=asymmetric.map",id.0)).await;
        let text=client.until("Unable to save your changes.").await;
        assert!(!text.contains("created from"));
        assert_eq!(terrain_rows(&mut sql).await,0);
        assert!(!persistence::load(&config.database()).await.unwrap().btech.maps().contains_key(&id));
        client.send(&format!("@btech inspect #{}",id.0)).await;
        client.until("No saved BattleTech identity").await;
        sqlx::query("DROP TRIGGER reject_tiles").execute(&mut sql).await.unwrap();
        client.send(&format!("@btech map-create #{}=asymmetric.map",id.0)).await;
        client.until(&format!("Map #{} created from asymmetric.map.",id.0)).await;
        assert!(persistence::load(&config.database()).await.unwrap().btech.maps()[&id].terrain_ready());
        sqlx::raw_sql("CREATE TRIGGER reject_reload BEFORE UPDATE OF value ON btech_map_hexes BEGIN SELECT RAISE(ABORT,'reload failure'); END;").execute(&mut sql).await.unwrap();
        client.send(&format!("@btech map-reload #{}=reload.map",id.0)).await;
        let text=client.until("Unable to save your changes.").await;
        assert!(!text.contains("terrain reloaded"));
        client.send(&format!("@btech inspect #{}",id.0)).await;
        client.until("temperature -12").await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech.maps()[&id].hex(2,1).unwrap().terrain,Terrain::Mountains);
        sqlx::query("DROP TRIGGER reject_reload").execute(&mut sql).await.unwrap();
        sqlx::query("CREATE TRIGGER reject_conditions BEFORE UPDATE OF light ON btech_maps BEGIN SELECT RAISE(ABORT,'condition failure'); END").execute(&mut sql).await.unwrap();
        client.send(&format!("@btech map-conditions #{}=night,10", id.0)).await;
        let text = client.until("Unable to save your changes.").await;
        assert!(!text.contains("conditions saved"));
        client.send(&format!("@btech inspect #{}", id.0)).await;
        client.until("Light 2; visibility 30; maximum visibility 60 hexes").await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech.maps()[&id].light, 2);
        sqlx::query("DROP TRIGGER reject_conditions").execute(&mut sql).await.unwrap();
        client.send(&format!("@btech map-conditions #{}=night,10", id.0)).await;
        client.until("conditions saved: night, visibility 10 hexes.").await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech.maps()[&id].light, 0);

        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
        sql.close().await.unwrap();
    }).await;
}

/// Deferred overlays must survive rejected terrain edits; only derived LOS is invalidated.
#[tokio::test]
async fn reload_rejects_unowned_objects_and_preserves_owned_lookup_rows() {
    let (_dir, config, mut world, id, mut sql) = fixture().await;
    create(&config, &mut world, id).await;
    sqlx::query("INSERT INTO btech_map_los VALUES(?,0,1,123)")
        .bind(id.0)
        .execute(&mut sql)
        .await
        .unwrap();
    sqlx::query("INSERT INTO btech_map_bits VALUES(?,0,0,1)")
        .bind(id.0)
        .execute(&mut sql)
        .await
        .unwrap();
    sqlx::query("INSERT INTO btech_map_objects VALUES(?,10,0,0,0,-1,0,0,0)")
        .bind(id.0)
        .execute(&mut sql)
        .await
        .unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    let before = world.btech.clone();
    reload_battle_map(
        &mut world,
        id,
        "reload.map",
        BattleMapAsset::parse(RELOAD).unwrap(),
    )
    .unwrap();
    let error = persistence::save(&config.database(), &world)
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("has map objects"));
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        before
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT flags FROM btech_map_los WHERE map_dbref=?")
            .bind(id.0)
            .fetch_one(&mut sql)
            .await
            .unwrap(),
        123
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT value FROM btech_map_bits WHERE map_dbref=?")
            .bind(id.0)
            .fetch_one(&mut sql)
            .await
            .unwrap(),
        1
    );
    sqlx::query("DELETE FROM btech_map_objects WHERE map_dbref=?")
        .bind(id.0)
        .execute(&mut sql)
        .await
        .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM btech_map_los WHERE map_dbref=?")
            .bind(id.0)
            .fetch_one(&mut sql)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT value FROM btech_map_bits WHERE map_dbref=?")
            .bind(id.0)
            .fetch_one(&mut sql)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

#[tokio::test]
async fn map_condition_commands_and_lua_validate_and_rollback() {
    let (_dir, config, mut world, id, sql) = fixture().await;
    create(&config, &mut world, id).await;
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    let scripts = Scripts::new(&config, Rc::new(std::cell::RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    support::run_text(
        &scripts,
        &config,
        ObjectId(2),
        2,
        &format!("@btech map-conditions #{}=night,10", id.0),
    );
    assert_eq!(scripts.world().btech, before);
    let run = |args: &str| {
        support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("@btech map-conditions #{}={args}", id.0),
        )
    };
    for invalid in ["noon,10", "night,-1", "day,61", "night,10,20", "night"] {
        run(invalid);
        assert_eq!(scripts.world().btech, before, "{invalid}");
    }
    assert!(run("NiGhT, 10").contains("conditions saved"));
    assert_eq!(scripts.world().btech.maps()[&id].light, 0);
    assert_eq!(scripts.world().btech.maps()[&id].maximum_visibility, 30);
    let changed = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.map.conditions({}, btech.map.light_levels.DAY, 60); error('abort')",
                id.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, changed);
    // Plain strings and constants from other catalogs are rejected like out-of-range visibility.
    for invalid in [
        "btech.map.light_levels.NIGHT, -1",
        "btech.map.light_levels.NIGHT, 256",
        "btech.map.light_levels.NIGHT, 61",
        "'night', 10",
        "btech.unit.types.MECH, 10",
    ] {
        assert!(
            scripts
                .eval_callback::<()>(&format!("btech.map.conditions({}, {invalid})", id.0))
                .is_err()
        );
        assert_eq!(scripts.world().btech, changed);
    }
    let light: i64 = scripts
        .eval_callback(&format!(
            "btech.map.conditions({}, btech.map.light_levels.TWILIGHT, 20); return btech.map.inspect({}).light",
            id.0, id.0
        ))
        .unwrap();
    assert_eq!(light, 1);
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
    sql.close().await.unwrap();
}

#[tokio::test]
async fn decoration_rows_preserve_extensions_and_reject_corrupt_lifetimes() {
    use stompymux_rs::{
        BattleDecoration, BattleDecorationKind, BattleHexCoordinate, set_map_decoration,
    };
    let (_dir, config, mut world, id, mut sql) = fixture().await;
    create(&config, &mut world, id).await;
    let coordinate = BattleHexCoordinate { x: 0, y: 0 };
    let smoke = BattleDecoration::new(BattleDecorationKind::Smoke, 100, None);
    set_map_decoration(&mut world, id, coordinate, Some(smoke)).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    sqlx::raw_sql("ALTER TABLE btech_map_decorations ADD COLUMN opaque TEXT DEFAULT 'retained'")
        .execute(&mut sql)
        .await
        .unwrap();
    set_map_decoration(
        &mut world,
        id,
        coordinate,
        Some(BattleDecoration {
            remaining: 99,
            ..smoke
        }),
    )
    .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let value: String =
        sqlx::query_scalar("SELECT opaque FROM btech_map_decorations WHERE map_dbref=?")
            .bind(id.0)
            .fetch_one(&mut sql)
            .await
            .unwrap();
    assert_eq!(value, "retained");
    let before = world.btech.clone();
    sqlx::raw_sql("CREATE TRIGGER reject_decoration_update BEFORE UPDATE ON btech_map_decorations BEGIN SELECT RAISE(ABORT,'decoration failure'); END;").execute(&mut sql).await.unwrap();
    set_map_decoration(
        &mut world,
        id,
        coordinate,
        Some(BattleDecoration {
            remaining: 98,
            ..smoke
        }),
    )
    .unwrap();
    assert!(persistence::save(&config.database(), &world).await.is_err());
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        before
    );
    sqlx::raw_sql("DROP TRIGGER reject_decoration_update; PRAGMA ignore_check_constraints=ON; UPDATE btech_map_decorations SET remaining=-1, expires_at=NULL;").execute(&mut sql).await.unwrap();
    assert!(persistence::load(&config.database()).await.is_err());
}

#[tokio::test]
async fn wind_and_fire_randomness_survive_reload_and_reject_missing_streams() {
    use stompymux_rs::{
        BattleDecoration, BattleDecorationKind, BattleHexCoordinate, set_map_decoration,
        set_map_wind,
    };
    let (_dir, config, mut world, id, mut sql) = fixture().await;
    create(&config, &mut world, id).await;
    let original = world.btech.clone();
    for (direction, speed) in [(-1, 0), (360, 0), (0, -1), (0, 32768)] {
        assert!(set_map_wind(&mut world, id, direction, speed).is_err());
        assert_eq!(world.btech, original);
    }
    for (speed, interval) in [(0, 60), (1, 59), (39, 21), (40, 20), (60, 20), (32767, 20)] {
        set_map_wind(&mut world, id, 359, speed).unwrap();
        assert_eq!(world.btech.maps()[&id].fire_spread_interval(), interval);
    }
    set_map_wind(&mut world, id, 240, 35).unwrap();
    let coordinate = BattleHexCoordinate { x: 0, y: 0 };
    set_map_decoration(
        &mut world,
        id,
        coordinate,
        Some(BattleDecoration::new(
            BattleDecorationKind::Fire,
            120,
            Some(60),
        )),
    )
    .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    let stream_query =
        "SELECT dice_seed,dice_stream,dice_block,dice_word FROM btech_map_random WHERE map_dbref=?";
    let encoded: (Vec<u8>, i64, i64, i64) = sqlx::query_as(stream_query)
        .bind(id.0)
        .fetch_one(&mut sql)
        .await
        .unwrap();
    assert_eq!(encoded.0.len(), 32);
    reload_battle_map(
        &mut world,
        id,
        "reload.map",
        BattleMapAsset::parse(RELOAD).unwrap(),
    )
    .unwrap();
    assert_eq!(
        (
            world.btech.maps()[&id].wind_direction,
            world.btech.maps()[&id].wind_speed
        ),
        (240, 35)
    );
    assert!(
        world.btech.maps()[&id]
            .decoration(coordinate)
            .unwrap()
            .is_none()
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let after: (Vec<u8>, i64, i64, i64) = sqlx::query_as(stream_query)
        .bind(id.0)
        .fetch_one(&mut sql)
        .await
        .unwrap();
    assert_eq!(encoded, after);
    set_map_decoration(
        &mut world,
        id,
        coordinate,
        Some(BattleDecoration::new(
            BattleDecorationKind::Fire,
            60,
            Some(60),
        )),
    )
    .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let reloaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(reloaded.btech, world.btech);
    sqlx::query("DELETE FROM btech_map_random WHERE map_dbref=?")
        .bind(id.0)
        .execute(&mut sql)
        .await
        .unwrap();
    assert!(persistence::load(&config.database()).await.is_err());
}

/// Permanent markers neither expire nor spread, including alongside active clocks and after restart.
#[tokio::test]
async fn permanent_decorations_survive_idle_and_mixed_timer_service() {
    use stompymux_rs::{
        BattleDecoration, BattleDecorationKind, BattleHexCoordinate, advance_map_fire,
        advance_map_smoke, map_fire_pending, map_smoke_pending, set_map_decoration,
    };
    let (_dir, config, mut world, id, _sql) = fixture().await;
    create(&config, &mut world, id).await;
    let fire = BattleHexCoordinate { x: 0, y: 0 };
    let smoke = BattleHexCoordinate { x: 1, y: 0 };
    for (coordinate, kind) in [
        (fire, BattleDecorationKind::Fire),
        (smoke, BattleDecorationKind::Smoke),
    ] {
        set_map_decoration(
            &mut world,
            id,
            coordinate,
            Some(BattleDecoration::new(kind, 0, None)),
        )
        .unwrap();
    }
    assert!(!map_fire_pending(&world));
    assert!(!map_smoke_pending(&world));
    let permanent = world.btech.clone();
    for _ in 0..180 {
        advance_map_fire(&mut world).unwrap();
        advance_map_smoke(&mut world);
    }
    assert_eq!(world.btech, permanent);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut replay = persistence::load(&config.database()).await.unwrap();
    assert_eq!(replay.btech, permanent);
    for candidate in [&mut world, &mut replay] {
        for (coordinate, kind) in [
            (
                BattleHexCoordinate { x: 0, y: 1 },
                BattleDecorationKind::Fire,
            ),
            (
                BattleHexCoordinate { x: 1, y: 1 },
                BattleDecorationKind::Smoke,
            ),
        ] {
            set_map_decoration(
                candidate,
                id,
                coordinate,
                Some(BattleDecoration::new(kind, 2, None)),
            )
            .unwrap();
        }
        assert!(map_fire_pending(candidate));
        assert!(map_smoke_pending(candidate));
        for _ in 0..241 {
            advance_map_fire(candidate).unwrap();
            advance_map_smoke(candidate);
        }
        assert!(!map_fire_pending(candidate));
        assert!(!map_smoke_pending(candidate));
        for coordinate in [fire, smoke] {
            assert_eq!(
                candidate.btech.maps()[&id].decoration(coordinate).unwrap(),
                permanent.maps()[&id].decoration(coordinate).unwrap()
            );
        }
        assert_eq!(
            candidate.btech.maps()[&id].hex(0, 0).unwrap().terrain,
            Terrain::Fire
        );
        assert_eq!(
            candidate.btech.maps()[&id].hex(1, 0).unwrap().terrain,
            Terrain::Smoke
        );
    }
    let before = world.btech.clone();
    assert!(
        set_map_decoration(
            &mut world,
            id,
            fire,
            Some(BattleDecoration::new(
                BattleDecorationKind::Fire,
                0,
                Some(1)
            ))
        )
        .is_err()
    );
    assert_eq!(world.btech, before);
    // Replacing a permanent marker installs an ordinary expiry; removing one reveals its source.
    set_map_decoration(
        &mut world,
        id,
        fire,
        Some(BattleDecoration::new(BattleDecorationKind::Smoke, 1, None)),
    )
    .unwrap();
    advance_map_smoke(&mut world);
    set_map_decoration(&mut world, id, smoke, None).unwrap();
    assert_eq!(
        world.btech.maps()[&id].hex(0, 0).unwrap().terrain,
        Terrain::Grassland
    );
    assert_eq!(
        world.btech.maps()[&id].hex(1, 0).unwrap().terrain,
        Terrain::Water
    );
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

/// Definition edits update lookup lanes atomically, retaining other maps and padding.
#[tokio::test]
async fn mine_and_building_lookup_updates_are_selective_and_transactional() {
    use stompymux_rs::{
        BattleBuildingEntrance, BattleHexCoordinate, BattleMineKind, BattleMinefield,
        set_building_entrance, set_minefield,
    };
    let (_dir, config, mut world, id, mut sql) = fixture().await;
    create(&config, &mut world, id).await;
    let other = world.create(&config, "Other map".into(), Kind::Room);
    create_battle_map(
        &mut world,
        other,
        "other",
        BattleMapAsset::parse(SOURCE).unwrap(),
    )
    .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    for (map, value) in [(id, 255), (other, 1)] {
        sqlx::query("INSERT INTO btech_map_bits VALUES(?,0,0,?)")
            .bind(map.0)
            .bind(value)
            .execute(&mut sql)
            .await
            .unwrap();
    }
    world = persistence::load(&config.database()).await.unwrap();
    // An unrelated world change preserves owned lookup rows.
    world.objects.get_mut(&id).unwrap().name = "Renamed map".into();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM btech_map_bits")
            .fetch_one(&mut sql)
            .await
            .unwrap(),
        2
    );
    let before = world.btech.clone();
    let coordinate = BattleHexCoordinate { x: 0, y: 0 };
    set_minefield(
        &mut world,
        id,
        0,
        Some(BattleMinefield {
            coordinate,
            kind: BattleMineKind::Standard,
            strength: 10,
            extra: 0,
            owner: ObjectId(1),
        }),
    )
    .unwrap();
    sqlx::query("CREATE TRIGGER reject_cache_update BEFORE UPDATE ON btech_map_bits BEGIN SELECT RAISE(ABORT,'cache failure'); END").execute(&mut sql).await.unwrap();
    assert!(persistence::save(&config.database(), &world).await.is_err());
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        before
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM btech_map_bits")
            .fetch_one(&mut sql)
            .await
            .unwrap(),
        2
    );
    sqlx::query("DROP TRIGGER reject_cache_update")
        .execute(&mut sql)
        .await
        .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT value FROM btech_map_bits WHERE map_dbref=?")
            .bind(id.0)
            .fetch_one(&mut sql)
            .await
            .unwrap(),
        235
    );
    set_building_entrance(
        &mut world,
        other,
        0,
        Some(BattleBuildingEntrance {
            coordinate,
            interior: id,
            data_char: 0,
            data_short: 0,
            data_int: 0,
        }),
    )
    .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM btech_map_bits")
            .fetch_one(&mut sql)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT value FROM btech_map_bits WHERE map_dbref=?")
            .bind(other.0)
            .fetch_one(&mut sql)
            .await
            .unwrap(),
        3
    );
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    assert_eq!(loaded.btech.maps()[&id].minefields().len(), 1);
    assert_eq!(
        loaded.btech.maps()[&other]
            .building_at(coordinate)
            .unwrap()
            .unwrap()
            .interior,
        id
    );
}
