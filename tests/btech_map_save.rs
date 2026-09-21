//! Map saves stage file effects through callback rollback and publish only after world commit.
use crate::support;
use sqlx::Connection;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use stompymux_rs::*;

/// An operator on a map with stale fire ensures saving also changes durable terrain.
async fn fixture() -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    world.accounts.get_mut(&ObjectId(1)).unwrap().hash =
        Some(accounts::hash("secret", &config).unwrap());
    let map = world.create(&config, "Save field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "save",
        BattleMapAsset::parse("2 1\n&2#1\n0: 100 20\n").unwrap(),
    )
    .unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
    std::fs::create_dir_all(config.path(&config.database.map_database)).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    (dir, config, world, map)
}

/// Authored fire remains permanent through both load paths, save preparation and restart.
#[tokio::test]
async fn authored_eternal_fire_survives_load_save_and_restart() {
    let (_dir, config, world, map) = fixture().await;
    let root = config.path(&config.database.map_database);
    std::fs::write(root.join("fire.map"), "2 1\n&2#1\n").unwrap();
    let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let output = support::run_text(&native, &config, ObjectId(1), 1, "loadmap fire.map");
    assert!(output.contains("Loading fire.map"), "{output}");
    assert!(
        lua.eval_callback::<bool>(&format!(
            "return btech.map.load_as(1, {}, 'fire.map')",
            map.0
        ))
        .unwrap()
    );
    assert_eq!(native.world().btech, lua.world().btech);
    assert_eq!(native.world().btech.maps()[&map].flags & 8, 8);
    let source = "2 1\n&2#1\n8: 100 20\n";
    for scripts in [&native, &lua] {
        let before = scripts.world().btech.clone();
        let exported = scripts.world().btech.maps()[&map].export_asset().unwrap();
        assert_eq!(exported.source, source);
        assert!(exported.stale_effects.is_empty());
        assert!(
            scripts
                .eval_callback::<bool>(&format!(
                    "return btech.map.save(1, {}, 'saved-fire.map')",
                    map.0
                ))
                .unwrap()
        );
        assert_eq!(scripts.world().btech, before);
    }
    let saved = native.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, saved.btech);
    assert_eq!(
        restored.btech.maps()[&map].export_asset().unwrap().source,
        source
    );
    MapAssetWrite::new(&config, ObjectId(1), "saved-fire.map", source.into())
        .unwrap()
        .publish(&config)
        .unwrap();
    let decoded = read_battle_map(&root, "saved-fire.map").unwrap();
    assert_eq!(decoded.flags, 8);
    assert_eq!(decoded.hex(0, 0).unwrap().terrain, Terrain::Fire);
}

/// Native and Lua prepare identical bytes, but neither publishes an uncommitted action.
#[tokio::test]
async fn map_save_staging_callbacks_and_paths() {
    let (_dir, config, world, map) = fixture().await;
    let root = config.path(&config.database.map_database);
    let path = root.join("export.map");
    std::fs::write(&path, "original").unwrap();
    let before = world.btech.clone();
    let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let call = format!("btech.map.save(1,{},'export.map')", map.0);
    assert!(
        lua.eval_callback::<()>(&format!("{call}; error('abort save')"))
            .is_err()
    );
    assert_eq!(lua.world().btech, before);
    assert!(lua.drain_outbox().is_empty());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
    assert!(
        lua.eval_callback::<bool>(&format!("return {call}"))
            .unwrap()
    );
    let output = support::run_text(&native, &config, ObjectId(1), 1, "savemap export.map");
    assert!(output.contains("Saving export.map") && !output.contains("Saving complete"));
    assert_eq!(native.world().btech, lua.world().btech);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
    let request = MapAssetWrite::new(
        &config,
        ObjectId(1),
        "export.map",
        before.maps()[&map].export_asset().unwrap().source,
    )
    .unwrap();
    let saved = native.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    request.publish(&config).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "2 1\n.2#1\n");
    for name in ["", "../escape", "/tmp/escape", "missing/asset", "bad\0name"] {
        assert!(MapAssetWrite::new(&config, ObjectId(1), name, "data".into()).is_err());
    }
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("target"), "outside").unwrap();
    std::os::unix::fs::symlink(outside.path(), root.join("escape")).unwrap();
    std::os::unix::fs::symlink(outside.path().join("target"), root.join("link")).unwrap();
    for name in ["escape/target", "link"] {
        assert!(MapAssetWrite::new(&config, ObjectId(1), name, "data".into()).is_err());
    }
    let request = MapAssetWrite::new(&config, ObjectId(1), "late", "new".into()).unwrap();
    std::os::unix::fs::symlink(outside.path().join("target"), root.join("late")).unwrap();
    assert!(request.publish(&config).is_err());
    assert_eq!(
        std::fs::read_to_string(outside.path().join("target")).unwrap(),
        "outside"
    );
}

/// A real server publishes a saved asset only after successful database persistence.
#[tokio::test(flavor = "current_thread")]
async fn server_map_save_waits_for_commit_and_reports_completion() {
    tokio::task::LocalSet::new().run_until(async {
        let (_dir, config, _, map) = fixture().await;
        let path = config.path(&config.database.map_database).join("server.map");
        std::fs::write(&path, "original").unwrap();
        let (addr, shutdown, task, _) = support::start(&config, Rc::new(Cell::new(1))).await;
        let mut client = support::Client { socket: tokio::net::TcpStream::connect(addr).await.unwrap(), pending: Vec::new() };
        client.until("Who are you? ").await;
        client.send("#1").await;
        client.until("Password: ").await;
        client.send("secret").await;
        client.until("Save field").await;
        let mut sql = sqlx::SqliteConnection::connect(&format!("sqlite:{}", config.database().display())).await.unwrap();
        sqlx::query("CREATE TRIGGER deny_map_export BEFORE UPDATE ON btech_map_hexes BEGIN SELECT RAISE(ABORT,'map save failure'); END")
            .execute(&mut sql).await.unwrap();
        client.send("savemap server.map").await;
        let response = client.until("Unable to save your changes.").await;
        assert!(!response.contains("Saving complete"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech.maps()[&map].base_hex(0,0).unwrap().terrain, Terrain::Fire);
        sqlx::query("DROP TRIGGER deny_map_export").execute(&mut sql).await.unwrap();
        client.send("savemap server.map").await;
        client.until("Saving complete!").await;
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "2 1\n.2#1\n");
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

/// Map writes share effect budgets, nested checkpoints and runtime inheritance without premature IO.
#[tokio::test]
async fn map_write_effects_restore_limits_and_inherit() {
    let (_dir, config, _, _) = fixture().await;
    let effects = Effects::new(&config, &Rc::new(RefCell::new(Vec::new())));
    let empty = effects.checkpoint();
    let request =
        MapAssetWrite::new(&config, ObjectId(1), "budget.map", "x".repeat(600_000)).unwrap();
    effects.stage_map_write(request.clone()).unwrap();
    assert!(effects.stage_map_write(request).is_err());
    assert_eq!(effects.checkpoint().map_writes.len(), 1);
    effects.restore(empty);
    assert!(effects.drain_map_writes().is_empty());
    let old = Effects::new(&config, &Rc::new(RefCell::new(Vec::new())));
    old.stage_map_write(MapAssetWrite::new(&config, ObjectId(1), "old.map", "old".into()).unwrap())
        .unwrap();
    effects
        .stage_map_write(MapAssetWrite::new(&config, ObjectId(1), "new.map", "new".into()).unwrap())
        .unwrap();
    effects.inherit(&old);
    let writes = effects.drain_map_writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].name, "new.map");
    assert!(
        !config
            .path(&config.database.map_database)
            .join("new.map")
            .exists()
    );
}
