//! C retry and creation-zone policies against isolated SQLite worlds and real sockets.
use crate::support;
use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::Rc,
    time::Duration,
};
use stompymux_rs::{
    Config, CreationContext, Flag, Kind, ObjectId, Scripts, accounts, commands, persistence,
    server::{self, ShutdownRequest},
};
use support::{Client, copy, start};
use tokio::{io::AsyncReadExt, net::TcpStream};
async fn fixture(retries: i64, zone: i64) -> (tempfile::TempDir, Config) {
    let d = tempfile::tempdir().unwrap();
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"),
        d.path(),
    );
    let path = d.path().join("stompymux.toml");
    let mut doc: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    for (section, key, value) in [
        ("mux", "retry_limit", retries),
        ("mux", "player_zone", zone),
        ("security", "password_hash_opslimit", 1),
        ("security", "password_hash_memlimit", 1048576),
        ("security", "login_attempt_burst", 100),
        ("security", "login_hash_limit", 100),
    ] {
        doc.as_table_mut()
            .unwrap()
            .entry(section)
            .or_insert(toml::Value::Table(Default::default()))
            .as_table_mut()
            .unwrap()
            .insert(key.into(), toml::Value::Integer(value));
    }
    std::fs::write(path, toml::to_string(&doc).unwrap()).unwrap();
    let c = Config::load(d.path()).unwrap();
    credentials(&c).await;
    (d, c)
}
/// Seed known credentials and a non-Wizard exclusively in the temporary database.
async fn credentials(c: &Config) {
    let mut world = persistence::load(&c.database()).await.unwrap();
    for id in [ObjectId(1), ObjectId(2)] {
        world.accounts.get_mut(&id).unwrap().hash = Some(accounts::hash("secret", c).unwrap());
    }
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    persistence::save(&c.database(), &world).await.unwrap();
}

/// A wrong-password allowance belongs to the connection, including across account-name changes.
#[tokio::test(flavor = "current_thread")]
async fn tcp_retry_limits_and_reconnect() {
    tokio::task::LocalSet::new()
        .run_until(async {
            for limit in [1, 3, 0, -1] {
                let (_d, c) = fixture(limit, 0).await;
                let (addr, shutdown, task, _) = start(&c, Rc::new(Cell::new(0))).await;
                let mut client = Client {
                    socket: TcpStream::connect(addr).await.unwrap(),
                    pending: Vec::new(),
                };
                for attempt in 0..limit.max(1) {
                    client.until("Who are you? ").await;
                    client
                        .send(if attempt % 2 == 0 { "#1" } else { "GOD" })
                        .await;
                    client.until("Password: ").await;
                    if attempt + 1 == limit.max(1) {
                        client.send("incorrect\r\n#1").await;
                    } else {
                        client.send("incorrect").await;
                    }
                    client.until("different password.").await;
                }
                let mut tail = client.pending;
                tokio::time::timeout(Duration::from_secs(5), client.socket.read_to_end(&mut tail))
                    .await
                    .unwrap()
                    .unwrap();
                assert!(!String::from_utf8_lossy(&tail).contains("Who are you?"));
                assert!(tail.windows(3).any(|b| b == [255, 252, 1]) || !tail.contains(&255));
                let mut reconnected = Client::connect(addr, 1).await;
                reconnected.send("look").await;
                reconnected.until("Staff Nexus").await;
                shutdown.send(ShutdownRequest::Sigterm).unwrap();
                task.await.unwrap().unwrap();
            }
        })
        .await;
}

/// Live limits affect new sockets; positive defaults are validated before publication.
#[tokio::test(flavor = "current_thread")]
async fn live_edits_and_creation_zones() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (_d, c) = fixture(3, 0).await;
            let (addr, shutdown, task, _) = start(&c, Rc::new(Cell::new(0))).await;
            let mut old = Client {
                socket: TcpStream::connect(addr).await.unwrap(),
                pending: Vec::new(),
            };
            old.until("Who are you? ").await;
            let mut god = Client::connect(addr, 1).await;
            god.send("@admin retry_limit=1").await;
            god.until("Set.").await;
            old.send("#1").await;
            old.until("Password: ").await;
            old.send("bad").await;
            old.until("Who are you? ").await;
            god.send("@admin player_zone=999999").await;
            god.until("Invalid or unavailable zone").await;
            god.send("@admin player_zone=4").await;
            god.until("Set.").await;
            god.send("@pcreate ZonePlayer=secret").await;
            god.until("created").await;
            god.send("@list options").await;
            god.until("Player zone...4").await;
            let w = persistence::load(&c.database()).await.unwrap();
            let p = w.find_player("ZonePlayer").unwrap();
            assert_eq!(w.objects[&p].zone, Some(ObjectId(4)));
            assert_eq!(w.objects[&ObjectId(1)].zone, None);
            shutdown.send(ShutdownRequest::Sigterm).unwrap();
            task.await.unwrap().unwrap();
        })
        .await;
}

/// Builder commands and trusted Lua select a creator independently of the cloned source.
#[tokio::test(flavor = "current_thread")]
async fn builder_lua_and_player_defaults() {
    let (_d, c) = fixture(3, 4).await;
    let mut w = persistence::load(&c.database()).await.unwrap();
    let source_zone = w.create(&c, "Other Zone".into(), Kind::Room);
    w.objects.get_mut(&ObjectId(1)).unwrap().zone = Some(ObjectId(4));
    let s = Scripts::new(&c, Rc::new(RefCell::new(w))).unwrap();
    for command in [
        "@create Parcel",
        "@dig Chamber=out,back",
        "@open passage=#4",
        "@clone Parcel=Copy",
    ] {
        let before = s.world().next_id;
        let _action = commands::run(&s, &c, ObjectId(1), 1, command).unwrap();
        assert!(s.world().next_id > before, "{command}");
        for o in s.world().objects.values().filter(|o| o.id.0 >= before) {
            assert_eq!(o.zone, Some(ObjectId(4)), "{command}");
        }
        s.drain_outbox();
        if command == "@create Parcel" {
            s.world_mut()
                .objects
                .get_mut(&ObjectId(before))
                .unwrap()
                .zone = Some(source_zone);
        }
    }
    s.eval_callback::<()>(&format!("local a=mux.world.create_object{{type=mux.world.types.ROOM,name='Inherited'}};assert(a:zone():dbref()==4);local b=mux.world.create_object{{type=mux.world.types.ROOM,name='Explicit',zone={}}};assert(b:zone():dbref()=={})",source_zone.0,source_zone.0)).unwrap();
    let stored = s.world().clone();
    persistence::save(&c.database(), &stored).await.unwrap();
    let loaded = persistence::load(&c.database()).await.unwrap();
    for o in stored.objects.values() {
        assert_eq!(loaded.objects[&o.id].zone, o.zone);
    }
    let mut w = s.world_mut();
    let player = w
        .create_with(
            &c,
            "PlayerDefault".into(),
            Kind::Player,
            CreationContext::Player,
        )
        .unwrap();
    assert_eq!(w.objects[&player].zone, Some(ObjectId(4)));
    w.objects.get_mut(&player).unwrap().location = Some(source_zone);
    w.objects
        .get_mut(&ObjectId(4))
        .unwrap()
        .flags
        .remove(Flag::NoCommand);
    assert!(
        commands::sources::sources(&w, player)
            .iter()
            .any(|source| source.object == ObjectId(4))
    );
    w.objects
        .get_mut(&ObjectId(4))
        .unwrap()
        .flags
        .insert(Flag::Going);
    let before = w.next_id;
    assert!(
        w.create_with(&c, "Invalid".into(), Kind::Player, CreationContext::Player)
            .is_err()
    );
    assert_eq!(before, w.next_id);
}

/// Policy catalog remains complete without inventing consumers for dormant C settings.
#[test]
fn catalog_coverage_and_password_failure_classification() {
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/config/mux-policy.json")).unwrap();
    let legacy: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/config/legacy-catalog.json")).unwrap();
    for row in legacy
        .iter()
        .filter(|r| r["path"].as_str().unwrap().starts_with("mux."))
    {
        let matches = rows
            .iter()
            .filter(|r| r["path"] == row["path"])
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0]["default"], row["default"]);
        assert!(
            [
                "implemented",
                "covered here",
                "intentionally deferred",
                "no active C consumer"
            ]
            .contains(&matches[0]["classification"].as_str().unwrap())
        );
    }
    assert!(accounts::verify_checked("secret", "malformed hash").is_err());
}

/// Bootstrap may resolve defaults forward, while restarting never assigns defaults retroactively.
#[tokio::test(flavor = "current_thread")]
async fn bootstrap_zone_forward_reference_and_restart() {
    let (_d, c) = fixture(3, 4).await;
    std::fs::remove_file(c.database()).unwrap();
    let s = server::prepare(&c).await.unwrap();
    assert!(s.world().objects.contains_key(&ObjectId(4)));
    assert_eq!(s.world().objects[&ObjectId(1)].zone, None);
    let before = std::fs::read(c.database()).unwrap();
    drop(s);
    server::prepare(&c).await.unwrap();
    assert_eq!(before, std::fs::read(c.database()).unwrap());
    let (_d, c) = fixture(3, 99999).await;
    let before = std::fs::read(c.database()).unwrap();
    assert!(server::prepare(&c).await.is_err());
    assert_eq!(before, std::fs::read(c.database()).unwrap());
}

/// Nonpositive player defaults and absent creator zones mean no zone, not dbref zero.
#[tokio::test(flavor = "current_thread")]
async fn zone_sentinels_and_callback_rollback() {
    for zone in [0, -1] {
        let (_d, c) = fixture(3, zone).await;
        let mut world = persistence::load(&c.database()).await.unwrap();
        let player = world
            .create_with(&c, "NoZone".into(), Kind::Player, CreationContext::Player)
            .unwrap();
        assert_eq!(world.objects[&player].zone, None);
        let thing = world
            .create_with(
                &c,
                "NoInheritedZone".into(),
                Kind::Thing,
                CreationContext::Object {
                    creator: ObjectId(1),
                    zone: None,
                },
            )
            .unwrap();
        assert_eq!(world.objects[&thing].zone, None);
        let s = Scripts::new(&c, Rc::new(RefCell::new(world))).unwrap();
        let before = s.world().next_id;
        assert!(s.eval_callback::<()>("mux.world.create_object{type=mux.world.types.ROOM,name='RolledBack',zone=4};error('cancel creation')").is_err());
        assert_eq!(before, s.world().next_id);
    }
}
