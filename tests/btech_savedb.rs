//! Explicit checkpoints use serialized persistence and publish success only after it completes.
use sqlx::Connection;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use stompymux_rs::*;
mod support;

#[tokio::test]
async fn save_requests_follow_effect_savepoints_and_lua_rollback() {
    let (_dir, config, world) = support::isolated_world().await;
    mlua::Lua::new()
        .load(include_str!("../game/lua/types/btech.d.lua"))
        .into_function()
        .unwrap();
    let outbox: Outbox = Default::default();
    let effects = Effects::new(&config, &outbox);
    let initial = effects.checkpoint();
    assert!(!effects.save_requested());
    effects.request_save();
    let pending = effects.checkpoint();
    effects.request_save();
    assert!(effects.save_requested());
    effects.restore(initial);
    assert!(!effects.save_requested());
    effects.restore(pending);
    assert!(effects.save_requested());
    effects.commit();
    assert!(!effects.save_requested());
    effects.request_save();
    effects.rollback();
    assert!(!effects.save_requested());

    let before = world.btech.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    assert!(
        scripts
            .eval_callback::<()>("btech.database.save(1);error('abort')")
            .is_err()
    );
    assert!(scripts.drain_outbox().is_empty());
    assert!(
        scripts
            .eval_callback::<()>("btech.database.save(4)")
            .is_err()
    );
    assert!(scripts.drain_outbox().is_empty());
    assert!(
        scripts
            .eval_callback::<bool>("return btech.database.save(1)")
            .unwrap()
    );
    let messages = scripts.drain_outbox();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].1.source(), "SQLite checkpoint complete.");
    assert_eq!(scripts.world().btech, before);
    let denied = support::run_text(&scripts, &config, ObjectId(4), 4, "savedb");
    assert!(denied.contains("Permission denied"), "{denied}");
}

#[tokio::test(flavor = "current_thread")]
async fn server_forces_unchanged_checkpoint_and_retries_after_database_lock() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (dir, _, mut world) = support::isolated_world().await;
            let path = dir.path().join("stompymux.toml");
            let mut table: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
            table
                .entry("database")
                .or_insert(toml::Value::Table(toml::Table::new()))
                .as_table_mut()
                .unwrap()
                .insert("busy_timeout_ms".into(), 20.into());
            std::fs::write(path, toml::to_string(&table).unwrap()).unwrap();
            let config = Config::load(dir.path()).unwrap();
            world.accounts.get_mut(&ObjectId(1)).unwrap().hash =
                Some(accounts::hash("secret", &config).unwrap());
            persistence::save(&config.database(), &world).await.unwrap();
            let (address, shutdown, task, _) = support::start(&config, Rc::new(Cell::new(1))).await;
            let mut client = support::Client::connect(address, 1).await;
            client.send("savedb ignored").await;
            client.until("SQLite checkpoint complete.").await;
            let before = persistence::load(&config.database()).await.unwrap();
            let mut lock = sqlx::SqliteConnection::connect_with(
                &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
            )
            .await
            .unwrap();
            sqlx::query("BEGIN IMMEDIATE")
                .execute(&mut lock)
                .await
                .unwrap();
            client.send("savedb").await;
            let failure = client
                .until("Unable to save your changes. Please try again.")
                .await;
            assert!(!failure.contains("SQLite checkpoint complete."));
            sqlx::query("ROLLBACK").execute(&mut lock).await.unwrap();
            lock.close().await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                before.btech
            );
            client.send("savedb").await;
            client.until("SQLite checkpoint complete.").await;
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                before.btech
            );
            shutdown.send(ShutdownRequest::Sigterm).unwrap();
            task.await.unwrap().unwrap();
        })
        .await;
}
