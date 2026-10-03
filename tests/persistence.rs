//! Direct schema-32 preservation, field deltas and legacy relationship regressions.
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
use std::collections::BTreeSet;
use stompymux_rs::{
    Channel, Config, Flag, Kind, Login, ObjectId, StateValue as Scalar, World, persistence,
};

/// Use an isolated copy of the populated relational fixture.
async fn fixture() -> (tempfile::TempDir, std::path::PathBuf, World) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("stompymux.db");
    std::fs::copy(
        crate::support::repository_root().join("tests/fixtures/game/data/stompymux.db"),
        &path,
    )
    .unwrap();
    let world = persistence::load(&path).await.unwrap();
    persistence::save(&path, &world).await.unwrap();
    (dir, path, world)
}
/// Open only an already-existing temporary SQLite file.
async fn connect(path: &std::path::Path) -> SqliteConnection {
    SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .foreign_keys(false),
    )
    .await
    .unwrap()
}
/// Read all schema objects to detect unwanted DDL, including indexes/triggers.
async fn schema(db: &mut SqliteConnection) -> Vec<(String, String)> {
    sqlx::query_as("SELECT name,sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY name")
        .fetch_all(db)
        .await
        .unwrap()
}
#[tokio::test(flavor = "current_thread")]
async fn preserves_unknown_data_and_unchanged_storage_classes() {
    let (_dir, path, _) = fixture().await;
    let mut db = connect(&path).await;
    sqlx::raw_sql("ALTER TABLE objects ADD COLUMN opaque BLOB; ALTER TABLE objects ADD COLUMN has_future_flag INTEGER NOT NULL DEFAULT 1; ALTER TABLE objects ADD COLUMN has_future_power INTEGER NOT NULL DEFAULT 1; UPDATE objects SET opaque=X'00FF42'; ALTER TABLE object_state ADD COLUMN opaque TEXT DEFAULT 'state extension'; ALTER TABLE player_state ADD COLUMN opaque TEXT DEFAULT 'account extension'; CREATE TABLE future_system(id INTEGER PRIMARY KEY, payload BLOB); INSERT INTO future_system VALUES(1,X'00FEFF'); CREATE INDEX future_index ON future_system(payload); CREATE TABLE writes_seen(value INTEGER); INSERT INTO writes_seen VALUES(0); CREATE TRIGGER name_only AFTER UPDATE OF name ON objects BEGIN UPDATE writes_seen SET value=value+1; END; INSERT INTO macro_sets VALUES(0,1,0,'preserved'); INSERT INTO commac_entries VALUES(1,2,0,0,0,0,0); UPDATE snapshot SET min_size=123,dump_type=7; INSERT INTO btech_unit_configuration(object_dbref,preferred_id) VALUES(1,'keep-btech');").execute(&mut db).await.unwrap();
    for (key, value, tag) in [
        ("blob", b"hello".as_slice(), 1),
        ("remove", b"bye".as_slice(), 1),
    ] {
        sqlx::query("INSERT INTO object_state(object_dbref,namespace,key,value_type,value) VALUES(1,'preserve',?1,?2,?3)").bind(key).bind(tag).bind(value).execute(&mut db).await.unwrap();
    }
    sqlx::raw_sql("INSERT INTO object_state(object_dbref,namespace,key,value_type,value) VALUES(1,'preserve','text',1,'plain'),(1,'preserve','number',4,7)").execute(&mut db).await.unwrap();
    let ddl = schema(&mut db).await;
    db.close().await.unwrap();
    let mut world = persistence::load(&path).await.unwrap();
    let bytes = std::fs::read(&path).unwrap();
    persistence::save(&path, &world).await.unwrap();
    assert_eq!(
        bytes,
        std::fs::read(&path).unwrap(),
        "no-op save wrote database bytes"
    );
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(Flag::Dark);
    world.accounts.get_mut(&ObjectId(1)).unwrap().alias = Some("newalias".into());
    let state = world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .state
        .get_mut("preserve")
        .unwrap();
    state.remove("remove");
    state.insert("changed".into(), Scalar::Integer(42));
    persistence::save(&path, &world).await.unwrap();
    let mut db = connect(&path).await;
    assert_eq!(schema(&mut db).await, ddl);
    let opaque: Vec<(String, i64, i64)> =
        sqlx::query_as("SELECT hex(opaque),has_future_flag,has_future_power FROM objects")
            .fetch_all(&mut db)
            .await
            .unwrap();
    assert!(opaque.iter().all(|r| r == &("00FF42".into(), 1, 1)));
    let kinds: Vec<(String, String)> = sqlx::query_as(
        "SELECT key,typeof(value) FROM object_state WHERE namespace='preserve' ORDER BY key",
    )
    .fetch_all(&mut db)
    .await
    .unwrap();
    assert_eq!(
        kinds,
        [
            ("blob".into(), "blob".into()),
            ("changed".into(), "integer".into()),
            ("number".into(), "integer".into()),
            ("text".into(), "text".into())
        ]
    );
    let extras: Vec<String> =
        sqlx::query_scalar("SELECT opaque FROM object_state WHERE namespace='preserve'")
            .fetch_all(&mut db)
            .await
            .unwrap();
    assert!(extras.iter().all(|s| s == "state extension"));
    let account: String =
        sqlx::query_scalar("SELECT opaque FROM player_state WHERE object_dbref=1")
            .fetch_one(&mut db)
            .await
            .unwrap();
    assert_eq!(account, "account extension");
    let future: String = sqlx::query_scalar("SELECT hex(payload) FROM future_system")
        .fetch_one(&mut db)
        .await
        .unwrap();
    assert_eq!(future, "00FEFF");
    let meta: (i64, i64) = sqlx::query_as("SELECT min_size,dump_type FROM snapshot")
        .fetch_one(&mut db)
        .await
        .unwrap();
    assert_eq!(meta, (123, 7));
    let macro_state: (i64, i64) =
        sqlx::query_as("SELECT curmac,macro_slot_4 FROM commac_entries WHERE who=1")
            .fetch_one(&mut db)
            .await
            .unwrap();
    assert_eq!(macro_state, (2, 0));
    let btech: String = sqlx::query_scalar(
        "SELECT preferred_id FROM btech_unit_configuration WHERE object_dbref=1",
    )
    .fetch_one(&mut db)
    .await
    .unwrap();
    assert_eq!(btech, "keep-btech");
    let writes: i64 = sqlx::query_scalar("SELECT value FROM writes_seen")
        .fetch_one(&mut db)
        .await
        .unwrap();
    assert_eq!(writes, 0, "unmodified name was updated");
    db.close().await.unwrap();
}
/// Verify legacy lists agree with semantic location relationships after a restart.
async fn assert_lists(path: &std::path::Path) {
    let w = persistence::load(path).await.unwrap();
    let mut db = connect(path).await;
    for o in w
        .objects
        .values()
        .filter(|o| matches!(o.kind, Kind::Room | Kind::Thing | Kind::Player))
    {
        for (column, exits) in [("contents", false), ("exits", true)] {
            let mut next: i64 = sqlx::query_scalar(if exits {
                "SELECT exits FROM objects WHERE dbref=?1"
            } else {
                "SELECT contents FROM objects WHERE dbref=?1"
            })
            .bind(o.id.0)
            .fetch_one(&mut db)
            .await
            .unwrap();
            let mut found = BTreeSet::new();
            while next >= 0 {
                assert!(found.insert(ObjectId(next)), "cycle");
                next = sqlx::query_scalar("SELECT next FROM objects WHERE dbref=?1")
                    .bind(next)
                    .fetch_one(&mut db)
                    .await
                    .unwrap();
            }
            let expected: BTreeSet<_> = w
                .objects
                .values()
                .filter(|m| m.location == Some(o.id) && (m.kind == Kind::Exit) == exits)
                .map(|o| o.id)
                .collect();
            assert_eq!(found, expected, "container #{} {column}", o.id.0);
        }
    }
    db.close().await.unwrap();
}
#[tokio::test(flavor = "current_thread")]
async fn movement_creation_and_exit_lists_remain_compatible() {
    let (dir, path, mut w) = fixture().await;
    std::fs::write(dir.path().join("stompymux.toml"), "").unwrap();
    let c = Config::load(dir.path()).unwrap();
    let container = w.create(&c, "Container".into(), Kind::Thing);
    w.objects.get_mut(&container).unwrap().location = Some(ObjectId(4));
    let child = w.create(&c, "Child".into(), Kind::Thing);
    w.objects.get_mut(&child).unwrap().location = Some(container);
    persistence::save(&path, &w).await.unwrap();
    assert_lists(&path).await;
    w.objects.get_mut(&container).unwrap().location = Some(ObjectId(5));
    w.objects.get_mut(&ObjectId(2)).unwrap().location = Some(child);
    let exit = *w
        .objects
        .iter()
        .find(|(_, o)| o.kind == Kind::Exit)
        .unwrap()
        .0;
    let destination = w.objects[&exit].destination;
    w.objects.get_mut(&exit).unwrap().location = Some(container);
    persistence::save(&path, &w).await.unwrap();
    assert_lists(&path).await;
    let loaded = persistence::load(&path).await.unwrap();
    assert_eq!(loaded.objects[&exit].destination, destination);
    assert_eq!(loaded.objects[&child].location, Some(container));
    let mut db = connect(&path).await;
    let foreign = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&mut db)
        .await
        .unwrap();
    assert!(foreign.is_empty());
    sqlx::query("UPDATE objects SET contents=4 WHERE dbref=4")
        .execute(&mut db)
        .await
        .unwrap();
    db.close().await.unwrap();
    w.objects.get_mut(&container).unwrap().location = Some(ObjectId(4));
    let bytes = std::fs::read(&path).unwrap();
    assert!(persistence::save(&path, &w).await.is_err());
    assert_eq!(bytes, std::fs::read(&path).unwrap());
}
#[tokio::test(flavor = "current_thread")]
async fn history_capacity_order_and_extra_required_columns() {
    let (_dir, path, mut w) = fixture().await;
    let account = w.accounts.get_mut(&ObjectId(1)).unwrap();
    account.history = (0..20)
        .map(|i| Login {
            success: i % 2 == 0,
            at: i,
            host: format!("host{i}"),
        })
        .collect();
    persistence::trim_history(&mut account.history, 32);
    assert_eq!(account.history.len(), 7);
    persistence::save(&path, &w).await.unwrap();
    let mut db = connect(&path).await;
    let entries:Vec<(i64,i64,i64)>=sqlx::query_as("SELECT outcome,position,occurred_at FROM player_login_history WHERE player_dbref=1 ORDER BY outcome,position").fetch_all(&mut db).await.unwrap();
    assert_eq!(
        entries,
        [
            (0, 0, 18),
            (0, 1, 16),
            (0, 2, 14),
            (0, 3, 12),
            (1, 0, 19),
            (1, 1, 17),
            (1, 2, 15)
        ]
    );
    let mut loaded = persistence::load(&path).await.unwrap();
    persistence::trim_history(
        &mut loaded.accounts.get_mut(&ObjectId(1)).unwrap().history,
        2,
    );
    assert_eq!(
        loaded.accounts[&ObjectId(1)]
            .history
            .iter()
            .map(|entry| entry.host.as_str())
            .collect::<Vec<_>>(),
        ["host18", "host19"]
    );
    loaded.accounts.get_mut(&ObjectId(1)).unwrap().history = vec![
        Login {
            success: true,
            at: 10,
            host: "success-old".into(),
        },
        Login {
            success: false,
            at: 20,
            host: "failure".into(),
        },
        Login {
            success: true,
            at: 5,
            host: "success-new-clock-backward".into(),
        },
    ];
    persistence::save(&path, &loaded).await.unwrap();
    let reloaded = persistence::load(&path).await.unwrap();
    assert_eq!(
        reloaded.accounts[&ObjectId(1)]
            .history
            .iter()
            .filter(|entry| entry.success)
            .map(|entry| entry.host.as_str())
            .collect::<Vec<_>>(),
        ["success-old", "success-new-clock-backward"]
    );
    // Retain existing channels as an unknown archive, and introduce an unsupported required column.
    sqlx::raw_sql("ALTER TABLE comsys_channels RENAME TO archived_channels; CREATE TABLE comsys_channels(name TEXT PRIMARY KEY,type INTEGER NOT NULL,num_messages INTEGER NOT NULL,chan_obj INTEGER NOT NULL,extension TEXT NOT NULL)").execute(&mut db).await.unwrap();
    db.close().await.unwrap();
    let mut w = persistence::load(&path).await.unwrap();
    w.objects.get_mut(&ObjectId(2)).unwrap().name = "Must roll back".into();
    w.channels.insert(
        "new".into(),
        Channel {
            name: "new".into(),
            object: None,
            flags: stompymux_rs::communication::ChannelFlags(0),
            messages: 0,
            ..Channel::new("new".into())
        },
    );
    let before = std::fs::read(&path).unwrap();
    let error = format!("{:#}", persistence::save(&path, &w).await.unwrap_err());
    assert!(error.contains("extension"), "{error}");
    assert_eq!(before, std::fs::read(&path).unwrap());
}
