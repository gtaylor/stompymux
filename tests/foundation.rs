//! In-process foundation scenarios: relational loading, configuration includes, telnet decoding,
//! Lua resource limits, bootstrap, flags, powers and teleport containment.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::{
    Config, ObjectId, Scripts, StateValue as Scalar, accounts, persistence, server,
    telnet::{Decoder, Input},
};
use support::{copy, stable_world};
fn fixture() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    copy(
        &support::repository_root().join("tests/fixtures/game"),
        d.path(),
    );
    let p = d.path().join("stompymux.toml");
    let text = std::fs::read_to_string(&p)
        .unwrap()
        .replace("password_hash_opslimit = 3", "password_hash_opslimit = 1")
        .replace(
            "password_hash_memlimit = 12582912",
            "password_hash_memlimit = 1048576",
        )
        .replace("login_attempt_burst = 3", "login_attempt_burst = 100")
        .replace("login_hash_limit = 5", "login_hash_limit = 100");
    std::fs::write(p, text).unwrap();
    d
}
async fn populated() -> (tempfile::TempDir, Config) {
    let d = fixture();
    let c = Config::load(d.path()).unwrap();
    persistence::load(&c.database()).await.unwrap();
    (d, c)
}
#[tokio::test(flavor = "current_thread")]
async fn direct_load_is_lossless_for_supported_data_and_read_only() {
    let d = fixture();
    let c = Config::load(d.path()).unwrap();
    let source = d.path().join("data/stompymux.db");
    let before = std::fs::read(&source).unwrap();
    let w = persistence::load(&source).await.unwrap();
    assert_eq!(w.objects.len(), 16);
    assert_eq!(w.accounts.len(), 2);
    assert_eq!(w.channels.len(), 2);
    assert_eq!(
        w.objects[&ObjectId(13)].state["locks.traverse"]["flag/WIZARD"],
        Scalar::Boolean(true)
    );
    persistence::load(&source).await.unwrap();
    let loaded = persistence::load(&c.database()).await.unwrap();
    assert_eq!(
        serde_json::to_value(w).unwrap(),
        serde_json::to_value(loaded).unwrap()
    );
    assert_eq!(before, std::fs::read(&source).unwrap());
}
#[tokio::test(flavor = "current_thread")]
async fn rejects_unsupported_schema_and_bad_references() {
    let d = fixture();
    let source = d.path().join("data/stompymux.db");
    let c = Config::load(d.path()).unwrap();
    let mut sql = <sqlx::SqliteConnection as sqlx::Connection>::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&source)
            .foreign_keys(false),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE snapshot SET schema_version=29")
        .execute(&mut sql)
        .await
        .unwrap();
    assert!(persistence::load(&source).await.is_err());
    assert!(c.database().exists());
    sqlx::query("UPDATE snapshot SET schema_version=32")
        .execute(&mut sql)
        .await
        .unwrap();
    sqlx::query("UPDATE objects SET location=99999 WHERE dbref=1")
        .execute(&mut sql)
        .await
        .unwrap();
    assert!(
        persistence::load(&source)
            .await
            .unwrap()
            .validate(&c)
            .is_err()
    );
    sqlx::Connection::close(sql).await.unwrap();
}
#[tokio::test(flavor = "current_thread")]
async fn load_all_scalar_types_and_argon_hash() {
    let d = fixture();
    let c = Config::load(d.path()).unwrap();
    let source = d.path().join("data/stompymux.db");
    let mut sql = <sqlx::SqliteConnection as sqlx::Connection>::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&source)
            .foreign_keys(false),
    )
    .await
    .unwrap();
    let hash = accounts::hash("known-secret", &c).unwrap();
    sqlx::query("UPDATE player_state SET password_hash=?1 WHERE object_dbref=1")
        .bind(&hash)
        .execute(&mut sql)
        .await
        .unwrap();
    sqlx::query("INSERT INTO object_state VALUES(1,'test','string',1,?1)")
        .bind("hi")
        .execute(&mut sql)
        .await
        .unwrap();
    sqlx::query("INSERT INTO object_state VALUES(1,'test','integer',3,?1)")
        .bind(42_i64)
        .execute(&mut sql)
        .await
        .unwrap();
    sqlx::query("INSERT INTO object_state VALUES(1,'test','number',4,?1)")
        .bind(1.5_f64)
        .execute(&mut sql)
        .await
        .unwrap();
    sqlx::query("INSERT INTO object_state VALUES(1,'test','blob',1,?1)")
        .bind(b"bytes".as_slice())
        .execute(&mut sql)
        .await
        .unwrap();
    sqlx::query("INSERT INTO object_state VALUES(1,'test','boolean',2,?1)")
        .bind(1_i64)
        .execute(&mut sql)
        .await
        .unwrap();
    sqlx::query("INSERT INTO object_state VALUES(1,'test','integer_number',4,?1)")
        .bind(7_i64)
        .execute(&mut sql)
        .await
        .unwrap();
    persistence::load(&source).await.unwrap();
    let w = persistence::load(&c.database()).await.unwrap();
    assert!(accounts::verify(
        "known-secret",
        w.accounts[&ObjectId(1)].hash.as_ref().unwrap()
    ));
    assert!(!accounts::verify("wrong", &hash));
    assert_eq!(
        w.objects[&ObjectId(1)].state["test"]["string"],
        Scalar::String("hi".into())
    );
    assert_eq!(
        w.objects[&ObjectId(1)].state["test"]["blob"],
        Scalar::String("bytes".into())
    );
    assert_eq!(
        w.objects[&ObjectId(1)].state["test"]["boolean"],
        Scalar::Boolean(true)
    );
    assert_eq!(
        w.objects[&ObjectId(1)].state["test"]["number"],
        Scalar::Number(1.5)
    );
    assert_eq!(
        w.objects[&ObjectId(1)].state["test"]["integer_number"],
        Scalar::Number(7.0)
    );

    assert_eq!(
        w.objects[&ObjectId(1)].state["test"]["integer"],
        Scalar::Integer(42)
    );
    sqlx::Connection::close(sql).await.unwrap();
}
#[tokio::test(flavor = "current_thread")]
async fn config_includes_override_and_detect_cycles() {
    let d = fixture();
    let path = d.path().join("stompymux.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("{text}\n[aliases.commands]\nl = 'look'\n")).unwrap();
    let c = Config::load(d.path()).unwrap();
    assert_eq!(c.aliases.commands["l"], "look");
    std::fs::write(d.path().join("aliases.toml"), "include=['stompymux.toml']").unwrap();
    assert!(Config::load(d.path()).is_err());
}
#[tokio::test(flavor = "current_thread")]
async fn telnet_fragmentation_echo_and_utf8() {
    let mut d = Decoder::default();
    let bytes = [255, 251, 31, 255, 250, 31, 0, 120, 0, 40, 255, 240];
    for b in bytes {
        d.feed(&[b]).unwrap();
    }
    assert_eq!((d.width, d.height), (120, 40));
    let mut lines = Vec::new();
    for b in "héllo\r\nnext\n".bytes() {
        for input in d.feed(&[b]).unwrap() {
            if let Input::Line(s) = input {
                lines.push(s);
            }
        }
    }
    assert_eq!(lines, vec!["héllo", "next"]);
    assert!(matches!(
        d.feed(&[0xfe, b'\n']).unwrap()[0],
        Input::InvalidUtf8
    ));
    assert!(d.feed(&vec![b'a'; 8193]).is_err());
    assert!(
        d.echo(true)
            .iter()
            .any(|e| matches!(e, Input::Reply(v) if v == &[255,251,1]))
    );
    assert!(!d.echo(false).iter().any(|e| matches!(e, Input::Reply(_))));
    assert!(
        d.feed(&[255, 253, 1])
            .unwrap()
            .iter()
            .any(|e| matches!(e, Input::Reply(v) if v == &[255,252,1]))
    );
}
#[tokio::test(flavor = "current_thread")]
async fn copied_lua_renders_rooms_locks_and_commands() {
    let (_d, c) = populated().await;
    let w = Rc::new(RefCell::new(
        persistence::load(&c.database()).await.unwrap(),
    ));
    let s = Scripts::new(&c, w).unwrap();
    let appearance = s.appearance(ObjectId(1), ObjectId(4), 1).unwrap();
    assert!(appearance.contains("Starter Room"));
    assert!(s.lock(ObjectId(1), ObjectId(13)).unwrap());
    s.world_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(stompymux_rs::Flag::Wizard);
    assert!(!s.lock(ObjectId(2), ObjectId(13)).unwrap());
    assert!(s.dispatch(ObjectId(1), 1, "global-hello").unwrap());
    assert!(s.outbox()[0].1.contains("Hello, world"));
    assert!(s.dispatch(ObjectId(1), 1, "flow-demo confirm").is_err());
}
#[tokio::test(flavor = "current_thread")]
async fn lua_resource_limits_and_fail_closed() {
    let (_d, c) = populated().await;
    let w = Rc::new(RefCell::new(
        persistence::load(&c.database()).await.unwrap(),
    ));
    let s = Scripts::new(&c, w).unwrap();
    assert!(s.inspect_lua().load("while true do end").exec().is_err());
    let s = s.rebuild_for_inspection(&c).unwrap();
    let lua = s.inspect_lua();
    lua.set_memory_limit(lua.used_memory() + 32768).unwrap();
    assert!(
        s.inspect_lua()
            .load("return string.rep('x',1000000)")
            .eval::<String>()
            .is_err()
    );
    let s = s.rebuild_for_inspection(&c).unwrap();
    assert!(
        s.inspect_lua()
            .load("mux.world.object(1):state('test'):set('too_big',string.rep('x',65537))")
            .exec()
            .is_err()
    );
    s.world_mut()
        .objects
        .get_mut(&ObjectId(13))
        .unwrap()
        .state
        .get_mut("locks.traverse")
        .unwrap()
        .insert("flag/UNKNOWN".into(), Scalar::Boolean(true));
    assert!(s.lock(ObjectId(1), ObjectId(13)).is_err());
}
#[tokio::test(flavor = "current_thread")]
async fn bootstrap_once_and_existing_world_does_not_bootstrap() {
    let d = fixture();
    std::fs::remove_file(d.path().join("data/stompymux.db")).unwrap();
    let c = Config::load(d.path()).unwrap();
    let s = server::prepare(&c).await.unwrap();
    assert_eq!(s.world().objects.len(), 16);
    assert_eq!(s.world().channels.len(), 2);
    let before = stable_world(&c.database()).await;
    drop(s);
    let s = server::prepare(&c).await.unwrap();
    assert_eq!(s.world().objects.len(), 16);
    assert_eq!(before, stable_world(&c.database()).await);
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(d.path().join("bootstrap-credentials.txt"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let (_d, c) = populated().await;
    let s = server::prepare(&c).await.unwrap();
    assert_eq!(s.world().objects.len(), 16);
    assert!(!c.root.join("bootstrap-credentials.txt").exists());
}
#[tokio::test(flavor = "current_thread")]
async fn bounded_output_marks_slow_clients_for_disconnect() {
    use stompymux_rs::sessions::{LoginFlow, Session};
    use tokio::time::Instant;
    let (output, _receiver) = tokio::sync::mpsc::channel(1);
    let now = Instant::now();
    let session = Session {
        retry_remaining: 3,
        output,
        stats: Default::default(),
        palette: Default::default(),
        color_override: Default::default(),
        presets_emitted: Default::default(),
        peer: "127.0.0.1".parse().unwrap(),
        site: Default::default(),
        player: None,
        flow: LoginFlow::Name,
        connected: now,
        active: now,
        decoder: Decoder::default(),
        quota: 1,
        quota_at: now,
        failed: Default::default(),
        output_message_limit: stompymux_rs::config::RuntimeConfig::default().output_message_limit,
    };
    assert!(session.raw(vec![1]));
    assert!(!session.raw(vec![2]));
    assert!(session.failed.get());
}
#[tokio::test(flavor = "current_thread")]
async fn missing_required_parent_prevents_startup() {
    let (d, c) = populated().await;
    std::fs::remove_file(d.path().join("lua/object_logic/default_room.lua")).unwrap();
    let w = Rc::new(RefCell::new(
        persistence::load(&c.database()).await.unwrap(),
    ));
    assert!(Scripts::new(&c, w).is_err());
}

#[tokio::test(flavor = "current_thread")]
async fn player_lookup_accepts_dbrefs_only_for_player_accounts() {
    let (_d, c) = populated().await;
    let mut world = persistence::load(&c.database()).await.unwrap();
    world.accounts.get_mut(&ObjectId(2)).unwrap().alias = Some("Wiz".into());
    for identity in ["#2", "Wizard", "wizard", "WIZ"] {
        assert_eq!(world.find_player(identity), Some(ObjectId(2)));
    }
    for identity in [
        "#0",
        "#4",
        "#999999",
        "#",
        "#-2",
        "#+2",
        "#2oops",
        "#9223372036854775808",
    ] {
        assert_eq!(world.find_player(identity), None, "{identity}");
    }
    world.accounts.remove(&ObjectId(2));
    assert_eq!(world.find_player("#2"), None);
}

/// Typed flags preserve old snapshots while rejecting unknown flag identities.
#[tokio::test(flavor = "current_thread")]
async fn flag_catalog_storage_commands_and_lua_contract() {
    use stompymux_rs::{
        commands,
        flags::{self, Flag},
    };
    let (_d, c) = populated().await;
    let world = Rc::new(RefCell::new(
        persistence::load(&c.database()).await.unwrap(),
    ));
    let s = Scripts::new(&c, world.clone()).unwrap();
    assert_eq!(flags::ALL.len(), 19);
    assert_eq!(
        flags::ALL.iter().map(|f| f.letter()).collect::<String>(),
        "Xab(cDFjGh#lMnsutWz"
    );
    for flag in flags::ALL {
        assert_eq!(
            Flag::parse(&flag.world_name().to_lowercase()).unwrap(),
            flag
        );
        let code = format!(
            "local f=mux.world.flags.{}; assert(tostring(f)=='{}'); assert(f==mux.world.flags.{})",
            flag.world_name(),
            flag.world_name(),
            flag.world_name()
        );
        s.inspect_lua().load(code).exec().unwrap();
    }
    s.inspect_lua()
        .load(
            r#"
      local flags=mux.world.flags
      local object=mux.world.object(4)
      assert(not pcall(function() flags.ANSI=false end))
      assert(not pcall(function() return flags.UNKNOWN end))
      assert(not pcall(function() return flags.ansi end))
      assert(not pcall(function() object:flags():add('DARK') end))
      assert(not pcall(function() object:flags():has('DARK') end))
      assert(not pcall(function() object:flags():add(flags.CONNECTED) end))
      assert(not pcall(function() mux.world.object(1):flags():remove(flags.WIZARD) end))
      assert(object:flags():add(flags.DARK)==true)
      assert(object:flags():add(flags.DARK)==false)
      assert(object:flags():remove(flags.DARK)==true)
      assert(object:flags():remove(flags.DARK)==false)
    "#,
        )
        .exec()
        .unwrap();
    for (input, expected) in [
        ("@flag #4=DA", "DARK set."),
        ("@flag #4=!DARK", "DARK cleared."),
        ("@flag me=!wizard", "cannot make yourself mortal"),
        ("@flag me=connected", "managed by player sessions"),
        ("@list flags", "CONNECTED(c)"),
        ("@ex me", "Flags:"),
        ("@flag #4=not_a_flag", "don't understand"),
        ("@flag #999=dark", "No such object"),
    ] {
        let action = commands::run(&s, &c, ObjectId(1), 1, input).unwrap();
        let text = match action {
            commands::Action::Report(commands::Report::Styled(text))
            | commands::Action::Report(commands::Report::Inspection(text))
            | commands::Action::Report(commands::Report::Literal(text))
            | commands::Action::Report(commands::Report::Reply(text)) => text,
            _ => s.pop_outbox().unwrap().1.source().to_string(),
        };
        assert!(text.contains(expected), "{input}: {text}");
    }
    {
        let mut w = world.borrow_mut();
        for _ in 0..2 {
            let id = w.create(&c, "Twin".into(), stompymux_rs::Kind::Thing);
            w.objects.get_mut(&id).unwrap().location = w.objects[&ObjectId(1)].location;
        }
        let exit = w.create(&c, "Test exit;shortcut".into(), stompymux_rs::Kind::Exit);
        w.objects.get_mut(&exit).unwrap().location = w.objects[&ObjectId(1)].location;
    }
    for (input, expected) in [
        ("@flag Twin=dark", "which object"),
        ("@flag shortcut=dark", "DARK set."),
        ("@flag here=light", "LIGHT set."),
    ] {
        commands::run(&s, &c, ObjectId(1), 1, input).unwrap();
        assert!(s.pop_outbox().unwrap().1.contains(expected));
    }
    let player = world
        .borrow_mut()
        .create(&c, "Ordinary".into(), stompymux_rs::Kind::Player);
    commands::run(&s, &c, player, 1, "@flag me=dark").unwrap();
    assert_eq!(s.pop_outbox().unwrap().1.source(), "Permission denied.");
    assert!(!stompymux_rs::authority::controls(
        &world.borrow(),
        ObjectId(2),
        ObjectId(1)
    ));
    assert!(stompymux_rs::authority::controls(
        &world.borrow(),
        ObjectId(2),
        ObjectId(2)
    ));
    assert!(
        flags::change(
            &mut world.borrow_mut(),
            ObjectId(2),
            ObjectId(4),
            Flag::Wizard,
            true
        )
        .is_err()
    );
    flags::change(
        &mut world.borrow_mut(),
        ObjectId(1),
        ObjectId(4),
        Flag::Going,
        true,
    )
    .unwrap();
    assert!(
        flags::change(
            &mut world.borrow_mut(),
            ObjectId(2),
            ObjectId(4),
            Flag::Going,
            false
        )
        .unwrap()
    );
    assert!(
        flags::change(
            &mut world.borrow_mut(),
            ObjectId(2),
            ObjectId(4),
            Flag::Going,
            true
        )
        .is_err()
    );
    world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let snapshot = world.borrow().clone();
    persistence::save(&c.database(), &snapshot).await.unwrap();
    let mut db = <sqlx::SqliteConnection as sqlx::Connection>::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(c.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    let connected: i64 = sqlx::query_scalar("SELECT has_connected_flag FROM objects WHERE dbref=1")
        .fetch_one(&mut db)
        .await
        .unwrap();
    assert_eq!(connected, 0);
    sqlx::query("UPDATE objects SET has_connected_flag=1 WHERE dbref=1")
        .execute(&mut db)
        .await
        .unwrap();
    assert!(
        !persistence::load(&c.database()).await.unwrap().objects[&ObjectId(1)]
            .flags
            .contains(Flag::Connected)
    );
    sqlx::raw_sql("ALTER TABLE objects ADD COLUMN has_unknown_flag INTEGER NOT NULL DEFAULT 1")
        .execute(&mut db)
        .await
        .unwrap();
    assert!(persistence::load(&c.database()).await.is_ok());
    sqlx::raw_sql(
        "PRAGMA ignore_check_constraints=ON; UPDATE objects SET has_dark_flag=2 WHERE dbref=1",
    )
    .execute(&mut db)
    .await
    .unwrap();
    let error = format!("{:#}", persistence::load(&c.database()).await.unwrap_err());
    assert!(
        error.contains("#1") && error.contains("has_dark_flag"),
        "{error}"
    );
    sqlx::Connection::close(db).await.unwrap();
}

/// The complete catalog, typed Lua contract and relational storage agree.
#[tokio::test(flavor = "current_thread")]
async fn powers_catalog_lua_and_relational_storage() {
    use stompymux_rs::powers::{self, Power, PowerSet};
    let d = fixture();
    let c = Config::load(d.path()).unwrap();
    let mut legacy = <sqlx::SqliteConnection as sqlx::Connection>::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(c.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    // Mutate only the temporary import fixture's schema-32 objects table.
    sqlx::query("UPDATE objects SET has_idle_power=1 WHERE dbref=2")
        .execute(&mut legacy)
        .await
        .unwrap();
    persistence::load(&c.database()).await.unwrap();
    let world = Rc::new(RefCell::new(
        persistence::load(&c.database()).await.unwrap(),
    ));
    assert!(
        world.borrow().objects[&ObjectId(2)]
            .powers
            .contains(Power::Idle)
    );
    let s = Scripts::new(&c, world.clone()).unwrap();
    assert_eq!(powers::ALL, [Power::Idle]);
    assert_eq!(Power::parse("iDlE").unwrap(), Power::Idle);
    assert_eq!(serde_json::to_string(&PowerSet::default()).unwrap(), "[]");
    s.inspect_lua()
        .load(
            r#"
        local p=mux.world.powers
        local o=mux.world.object(2)
        assert(tostring(p.IDLE)=='IDLE' and p.IDLE==p.IDLE)
        assert(o:powers():has(p.IDLE))
        assert(o:powers():remove(p.IDLE))
        assert(not o:powers():remove(p.IDLE))
        assert(o:powers():add(p.IDLE))
        assert(not o:powers():add(p.IDLE))
        assert(not pcall(function() p.IDLE=false end))
        assert(not pcall(function() return p.UNKNOWN end))
        assert(not pcall(function() return p.idle end))
        assert(not pcall(function() return p[false] end))
        assert(not pcall(function() o:powers():has('IDLE') end))
        assert(not pcall(function() o:powers():add(mux.world.flags.ANSI) end))
        assert(not pcall(function() o:powers():remove(mux.world.flags.ANSI) end))
        assert(not pcall(function() o:flags():add(p.IDLE) end))
        assert(not pcall(function() mux.world.object(999999):powers():has(p.IDLE) end))
    "#,
        )
        .exec()
        .unwrap();
    let snapshot = world.borrow().clone();
    persistence::save(&c.database(), &snapshot).await.unwrap();
    let mut db = <sqlx::SqliteConnection as sqlx::Connection>::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(c.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    let idle: i64 = sqlx::query_scalar("SELECT has_idle_power FROM objects WHERE dbref=2")
        .fetch_one(&mut db)
        .await
        .unwrap();
    assert_eq!(idle, 1);
    assert!(
        persistence::load(&c.database()).await.unwrap().objects[&ObjectId(2)]
            .powers
            .contains(Power::Idle)
    );
    sqlx::raw_sql("ALTER TABLE objects ADD COLUMN has_unknown_power INTEGER NOT NULL DEFAULT 1")
        .execute(&mut db)
        .await
        .unwrap();
    assert!(persistence::load(&c.database()).await.is_ok());
    sqlx::raw_sql(
        "PRAGMA ignore_check_constraints=ON; UPDATE objects SET has_idle_power=2 WHERE dbref=2",
    )
    .execute(&mut db)
    .await
    .unwrap();
    let error = format!("{:#}", persistence::load(&c.database()).await.unwrap_err());
    assert!(
        error.contains("#2") && error.contains("has_idle_power"),
        "{error}"
    );
    sqlx::Connection::close(legacy).await.unwrap();
    sqlx::Connection::close(db).await.unwrap();
}

/// Administration reuses flag targeting and control permissions.
#[tokio::test(flavor = "current_thread")]
async fn power_commands_validate_targets_permissions_and_names() {
    use stompymux_rs::{Kind, commands, powers::Power};
    let (d, _) = populated().await;
    let path = d.path().join("aliases.toml");
    let aliases = std::fs::read_to_string(&path)
        .unwrap()
        .replace("[aliases.commands]", "[aliases.commands]\npow='@power'");
    std::fs::write(path, aliases).unwrap();
    let c = Config::load(d.path()).unwrap();
    let world = Rc::new(RefCell::new(
        persistence::load(&c.database()).await.unwrap(),
    ));
    let scripts = Scripts::new(&c, world.clone()).unwrap();
    let ordinary = world
        .borrow_mut()
        .create(&c, "Ordinary".into(), Kind::Player);
    for _ in 0..2 {
        let id = world.borrow_mut().create(&c, "Twin".into(), Kind::Thing);
        let room = world.borrow().objects[&ObjectId(1)].location;
        world.borrow_mut().objects.get_mut(&id).unwrap().location = room;
    }
    for (actor, command, expected) in [
        (ObjectId(1), "pow #4=iDlE", "idle granted."),
        (ObjectId(1), "@power #4=!idle", "idle removed."),
        (ObjectId(1), "@power here=idle", "idle granted."),
        (ObjectId(2), "@power me=idle", "idle granted."),
        (ObjectId(2), "@power #1=idle", "Permission denied."),
        (ordinary, "@power me=idle", "Permission denied."),
        (ordinary, "@list powers", "Permission denied."),
        (ObjectId(1), "@power Twin=idle", "which object"),
        (ObjectId(1), "@power #99999=idle", "No such object"),
        (ObjectId(1), "@power #4=UNKNOWN", "understand that power"),
        (ObjectId(1), "@power #4=idle idle", "understand that power"),
        (ObjectId(1), "@power #4=", "specify a power"),
        (ObjectId(1), "@power #4=!", "specify a power"),
        (ObjectId(1), "@list powers", "Powers: idle"),
        (ObjectId(1), "@examine #2", "Powers: idle"),
    ] {
        let action = commands::run(&scripts, &c, actor, 1, command).unwrap();
        let output = match action {
            commands::Action::Report(commands::Report::Styled(text))
            | commands::Action::Report(commands::Report::Inspection(text))
            | commands::Action::Report(commands::Report::Literal(text))
            | commands::Action::Report(commands::Report::Reply(text)) => text,
            _ => scripts.pop_outbox().unwrap().1.source().to_string(),
        };
        assert!(output.contains(expected), "{command}: {output}");
    }
    assert!(
        !world.borrow().objects[&ordinary]
            .powers
            .contains(Power::Idle)
    );
}

/// Wizard movement handles containers, home, occupied containers and exit relocation.
#[tokio::test(flavor = "current_thread")]
async fn wizard_teleport_and_home_validate_containment() {
    use stompymux_rs::{Kind, commands};
    let (_d, c) = populated().await;
    let mut w = persistence::load(&c.database()).await.unwrap();
    let stored_home = w.objects[&ObjectId(1)].home;
    let cargo = w.create(&c, "Cargo".into(), Kind::Thing);
    let inner = w.create(&c, "Inner".into(), Kind::Thing);
    w.objects.get_mut(&cargo).unwrap().location = Some(ObjectId(4));
    w.objects.get_mut(&inner).unwrap().location = Some(cargo);
    let ordinary = w.create(&c, "Ordinary".into(), Kind::Player);
    w.objects.get_mut(&ordinary).unwrap().location = Some(ObjectId(4));
    let s = Scripts::new(&c, Rc::new(RefCell::new(w))).unwrap();
    for input in ["home", "@teleport #4", "@tel/quiet #4"] {
        commands::run(&s, &c, ordinary, 99, input).unwrap();
        assert_eq!(s.pop_outbox().unwrap().1.source(), "Permission denied.");
    }
    commands::run(&s, &c, ObjectId(1), 1, &format!("@tel #{}", cargo.0)).unwrap();
    assert_eq!(s.world().objects[&ObjectId(1)].location, Some(cargo));
    assert!(s.outbox().iter().any(|(_, t)| t.contains("Cargo")));
    commands::run(&s, &c, ObjectId(1), 1, "look").unwrap();
    assert!(s.pop_outbox().unwrap().1.contains("Inner"));
    commands::run(&s, &c, ObjectId(1), 1, &format!("@teleport #{}", inner.0)).unwrap();
    for input in [
        format!("@teleport #{}=me", cargo.0),
        "@teleport me".into(),
        "@teleport #4=#2".into(),
        "@teleport #13".into(),
        "home extra".into(),
        "@tel/quiet #4".into(),
        "@teleport =#4".into(),
    ] {
        let before = s.world().clone();
        commands::run(&s, &c, ObjectId(1), 1, &input).unwrap();
        assert_eq!(
            serde_json::to_value(before).unwrap(),
            serde_json::to_value(&*s.world()).unwrap(),
            "{input}"
        );
    }
    commands::run(
        &s,
        &c,
        ObjectId(1),
        1,
        &format!("@teleport #2=#{}", cargo.0),
    )
    .unwrap();
    commands::run(
        &s,
        &c,
        ObjectId(1),
        1,
        &format!("@teleport #{}=#4", cargo.0),
    )
    .unwrap();
    assert_eq!(s.world().objects[&ObjectId(2)].location, Some(cargo));
    let linked = s.world().objects[&ObjectId(13)].destination;
    commands::run(
        &s,
        &c,
        ObjectId(1),
        1,
        &format!("@teleport #13=#{}", cargo.0),
    )
    .unwrap();
    assert_eq!(s.world().objects[&ObjectId(13)].destination, linked);
    assert_eq!(s.world().objects[&ObjectId(13)].location, Some(cargo));
    s.drain_outbox();
    commands::run(&s, &c, ObjectId(1), 1, "HoMe").unwrap();
    assert_eq!(s.world().objects[&ObjectId(1)].location, stored_home);
    assert_eq!(
        s.outbox()
            .iter()
            .filter(|(_, t)| t.source() == "There's no place like home...")
            .count(),
        3
    );
    commands::run(&s, &c, ObjectId(1), 1, "@teleport #2").unwrap();
    assert_eq!(s.world().objects[&ObjectId(1)].location, Some(ObjectId(2)));
    for home in [
        None,
        Some(ObjectId(13)),
        Some(ObjectId(999999)),
        Some(ObjectId(1)),
    ] {
        s.world_mut().objects.get_mut(&ObjectId(1)).unwrap().home = home;
        let before = serde_json::to_value(&*s.world()).unwrap();
        commands::run(&s, &c, ObjectId(1), 1, "home").unwrap();
        assert_eq!(before, serde_json::to_value(&*s.world()).unwrap());
    }
    s.world_mut().objects.get_mut(&ObjectId(1)).unwrap().home = stored_home;
    let snapshot = s.world().clone();
    persistence::save(&c.database(), &snapshot).await.unwrap();
    let mut loaded = persistence::load(&c.database()).await.unwrap();
    loaded.validate(&c).unwrap();
    assert_eq!(loaded.objects[&ObjectId(1)].location, Some(ObjectId(2)));
    loaded.objects.get_mut(&cargo).unwrap().location = Some(inner);
    assert!(loaded.validate(&c).is_err());
    loaded.objects.get_mut(&cargo).unwrap().location = Some(ObjectId(13));
    assert!(loaded.validate(&c).is_err());
}

/// Nested locks get distinct subject/enactor identities and callbacks roll back atomically.
#[tokio::test(flavor = "current_thread")]
async fn teleport_locks_context_and_callbacks_are_transactional() {
    use stompymux_rs::{Kind, commands};
    let (d, c) = populated().await;
    std::fs::write(d.path().join("lua/object_logic/movement_test.lua"),r#"
      return {locks={teleport=function(ctx)
        if lock_error then error("injected lock failure") end
        assert(ctx.cause==1 and ctx.subject==1 and ctx.enactor==2)
        assert(ctx.descriptor==nil)
        return not deny_destination
      end,teleport_out=function(ctx)
        assert(ctx.subject==ctx.enactor)
        return not deny_out
      end},events={on_leave=function(ctx)
        assert(ctx.source==ctx.object)
        if fail_exit then mux.world.object(ctx.enactor):set_description('leaked');error('exit failed') end
      end,on_enter=function(ctx)
        assert(ctx.destination==ctx.object)
        mux.world.object(ctx.enactor):state('movement'):set('cause',ctx.cause)
        if fail_enter then mux.world.pemit(ctx.enactor,'LEAKED');error('enter failed') end
      end}}
    "#).unwrap();
    let mut w = persistence::load(&c.database()).await.unwrap();
    let boxid = w.create(&c, "Box".into(), Kind::Thing);
    w.objects.get_mut(&boxid).unwrap().location = Some(ObjectId(4));
    w.objects.get_mut(&ObjectId(4)).unwrap().lua_parent = "movement_test.lua".into();
    w.objects.get_mut(&ObjectId(2)).unwrap().location = Some(boxid);
    w.objects.get_mut(&ObjectId(2)).unwrap().home = Some(ObjectId(4));
    let s = Scripts::new(&c, Rc::new(RefCell::new(w))).unwrap();
    let request = "@teleport #2=#4";
    for globals in [
        "deny_destination=true",
        "deny_destination=false;lock_error=true",
        "lock_error=false;deny_out=true",
        "deny_out=false;fail_enter=true",
    ] {
        s.inspect_lua().load(globals).exec().unwrap();
        let before = serde_json::to_value(&*s.world()).unwrap();
        commands::run(&s, &c, ObjectId(1), 1, request).unwrap();
        assert_eq!(before, serde_json::to_value(&*s.world()).unwrap());
        assert!(!s.outbox().iter().any(|(_, t)| t.source() == "LEAKED"));
    }
    s.inspect_lua()
        .load("fail_enter=false;deny_out=false")
        .exec()
        .unwrap();
    commands::run(&s, &c, ObjectId(1), 1, request).unwrap();
    assert_eq!(
        s.world().objects[&ObjectId(2)].state["movement"]["cause"],
        Scalar::Integer(-1)
    );
    s.inspect_lua().load("fail_exit=true").exec().unwrap();
    let before = serde_json::to_value(&*s.world()).unwrap();
    commands::run(
        &s,
        &c,
        ObjectId(1),
        1,
        &format!("@teleport #2=#{}", boxid.0),
    )
    .unwrap();
    assert_eq!(before, serde_json::to_value(&*s.world()).unwrap());
    // Going home bypasses both destination and nested teleport-out locks.
    s.world_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .location = Some(boxid);
    s.inspect_lua()
        .load("fail_exit=false;deny_destination=true;deny_out=true")
        .exec()
        .unwrap();
    commands::run(&s, &c, ObjectId(2), 2, "home").unwrap();
    assert_eq!(s.world().objects[&ObjectId(2)].location, Some(ObjectId(4)));
    let before = serde_json::to_value(&*s.world()).unwrap();
    commands::run(&s, &c, ObjectId(2), 2, "home").unwrap();
    assert_eq!(before, serde_json::to_value(&*s.world()).unwrap());
}
