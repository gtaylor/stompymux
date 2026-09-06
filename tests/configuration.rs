//! Configuration compatibility, runtime limits and isolated CLI regressions.
use std::{
    cell::RefCell,
    collections::BTreeSet,
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};
use stompymux_rs::{
    accounts,
    config::{Config, Flag, catalog::KEYS},
    persistence,
    scripting::Scripts,
    server,
    telnet::{Decoder, Input},
    world::World,
};
fn config(text: &str) -> (tempfile::TempDir, Config) {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("stompymux.toml"), text).unwrap();
    let c = Config::load(d.path()).unwrap();
    (d, c)
}
fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game")
}
fn copy(source: &Path, dest: &Path) {
    std::fs::create_dir_all(dest).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        if entry.path().is_dir() {
            copy(&entry.path(), &dest.join(entry.file_name()));
        } else {
            std::fs::copy(entry.path(), dest.join(entry.file_name())).unwrap();
        }
    }
}
fn game() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    copy(&fixture_path(), d.path());
    d
}
fn append(dir: &Path, text: &str) {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(dir.join("stompymux.toml"))
        .unwrap();
    file.write_all(text.as_bytes()).unwrap();
}
#[tokio::test(flavor = "current_thread")]
async fn complete_legacy_catalog_and_compiled_defaults() {
    let (_d, c) = config("");
    let inventory: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/config/legacy-catalog.json")).unwrap();
    assert_eq!(inventory.len(), 182);
    let expected: BTreeSet<_> = inventory
        .iter()
        .map(|v| v["path"].as_str().unwrap())
        .collect();
    let actual: BTreeSet<_> = KEYS
        .iter()
        .filter(|s| !s.legacy.is_empty())
        .map(|s| s.path)
        .collect();
    assert_eq!(expected, actual);
    for row in inventory {
        let path = row["path"].as_str().unwrap();
        let actual = serde_json::to_value(c.effective_value(path).unwrap()).unwrap();
        let expected = if path == "database.game_database" {
            serde_json::json!("data/stompymux-rs.db")
        } else {
            row["default"].clone()
        };
        assert_eq!(actual, expected, "default for {path}");
        assert_eq!(
            c.effective_value(path),
            c.effective_value(row["legacy_name"].as_str().unwrap()),
            "Lua name mapping for {path}"
        );
    }
    assert_eq!(c.server.port, 6250);
    assert_eq!(c.server.listen_address.to_string(), "127.0.0.1");
    assert_eq!(c.security.login_hash_concurrency, 5);
}
#[tokio::test(flavor = "current_thread")]
async fn complete_fixture_parses_every_shape() {
    let (_d, c) = config(include_str!("fixtures/config/complete.toml"));
    assert_eq!(c.colors["brand-blue"], [32, 96, 192]);
    assert_eq!(c.access.commands["@foo"].0, ["wizard", "need_player"]);
    assert_eq!(c.sites.forbid.len(), 2);
    assert_eq!(c.sites.forbid[0].address.to_string(), "192.0.2.0");
    assert_eq!(c.server.listen_address.to_string(), "::1");
    assert!(c.validate_for_serve().is_err());
    assert!(!c.warnings.iter().any(|w| w.contains("unknown")));
}
#[tokio::test(flavor = "current_thread")]
async fn includes_merge_maps_replace_arrays_and_keep_parent_precedence() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir(d.path().join("parts")).unwrap();
    std::fs::write(d.path().join("parts/first.toml"),"server.port=1000\nnames.bad=['first']\naliases.commands.a='look'\naliases.commands.b='say'\n").unwrap();
    std::fs::write(d.path().join("second.toml"),"include=['parts/first.toml']\nserver.port=2000\nnames.bad=['second']\naliases.commands.a='WHO'\n").unwrap();
    std::fs::write(
        d.path().join("stompymux.toml"),
        "include=['parts/first.toml','second.toml']\nserver.port=3000\naliases.commands.c='quit'\n",
    )
    .unwrap();
    let c = Config::load(d.path()).unwrap();
    assert_eq!(c.server.port, 3000);
    assert_eq!(c.names.bad, ["second"]);
    assert_eq!(c.aliases.commands["a"], "WHO");
    assert_eq!(c.aliases.commands["b"], "say");
    assert_eq!(c.aliases.commands["c"], "quit");
}
#[tokio::test(flavor = "current_thread")]
async fn bootstrap_map_replaces_included_map() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("included.toml"),
        "database.bootstrap.objects.99={type='room',name='Extra'}",
    )
    .unwrap();
    let objects = toml::Value::try_from(stompymux_rs::config::BootstrapConfig::default()).unwrap();
    // Serialize a proper document rather than an inline Value display.
    let mut table = toml::Table::new();
    table.insert(
        "include".into(),
        toml::Value::Array(vec!["included.toml".into()]),
    );
    let mut db = toml::Table::new();
    db.insert("bootstrap".into(), objects);
    table.insert("database".into(), db.into());
    std::fs::write(
        d.path().join("stompymux.toml"),
        toml::to_string(&table).unwrap(),
    )
    .unwrap();
    let c = Config::load(d.path()).unwrap();
    assert_eq!(c.database.bootstrap.objects.len(), 6);
    assert!(
        !c.database
            .bootstrap
            .objects
            .contains_key(&stompymux_rs::config::BootstrapId(99))
    );
}
#[tokio::test(flavor = "current_thread")]
async fn unknown_keys_warn_but_known_type_errors_report_the_included_file() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("child.toml"), "server.port='oops'").unwrap();
    std::fs::write(
        d.path().join("stompymux.toml"),
        "include=['child.toml']\nserver.port=5555",
    )
    .unwrap();
    let error = format!("{:#}", Config::load(d.path()).unwrap_err());
    assert!(error.contains("child.toml") && error.contains("server.port"));
    std::fs::write(d.path().join("child.toml"),"server.new_setting=123\nnew_feature.enabled=true\nsites.forbid=[{address='192.0.2.1',mask='255.255.255.255',typo=true}]").unwrap();
    let c = Config::load(d.path()).unwrap();
    assert!(c.warnings.iter().any(|w| w.contains("server.new_setting")));
    assert!(
        c.warnings
            .iter()
            .any(|w| w.contains("sites.forbid[0].typo"))
    );
    assert!(c.effective_value("server.new_setting").is_none());
}
#[tokio::test(flavor = "current_thread")]
async fn malformed_known_values_and_cycles_are_rejected() {
    for text in [
        "lua.memory_limit=-1",
        "server.port=65536",
        "battletech.techtime_multiplier=nan",
        "battletech.techtime_multiplier=11.0",
        "colors.bad=[0,256,0]",
        "access.commands.foo=12",
        "sites.forbid=[{address='nope',mask='255.255.255.0'}]",
        "sites.forbid=[{address='::1',mask='255.255.255.0'}]",
        "database.bootstrap.objects.1={type='room',name='GOD',wizard=true}",
        "runtime.event_queue_capacity=0",
        "include='x'",
        "include=[123]",
    ] {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("stompymux.toml"), text).unwrap();
        assert!(Config::load(d.path()).is_err(), "accepted {text}");
    }
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("stompymux.toml"), "include=['loop.toml']").unwrap();
    std::fs::write(d.path().join("loop.toml"), "include=['stompymux.toml']").unwrap();
    assert!(format!("{:#}", Config::load(d.path()).unwrap_err()).contains("cycle"));
}
#[tokio::test(flavor = "current_thread")]
async fn aliases_resolve_default_flags_after_includes() {
    let (_d, c) = config("aliases.flags.wi='WIZARD'\nmux.default_player_flags=['wi','ANSI']");
    assert_eq!(c.mux.default_player_flags, vec![Flag::Wizard, Flag::Ansi]);
}
#[tokio::test(flavor = "current_thread")]
async fn listener_precedence_and_effective_lua_values() {
    let (_d, c) = config("server.port=4321\nserver.listen_address='::1'");
    assert_eq!(c.listener().to_string(), "[::1]:4321");
    let c = c
        .with_listener_overrides(Some("127.0.0.2".parse().unwrap()), Some(0))
        .unwrap();
    assert_eq!(c.listener().to_string(), "127.0.0.2:0");
    assert_eq!(c.effective_value("port").unwrap().as_integer(), Some(0));
    assert_eq!(
        c.effective_value("btech_xp_usePilotBVMod")
            .unwrap()
            .as_integer(),
        Some(1)
    );
}
#[tokio::test(flavor = "current_thread")]
async fn live_and_legacy_paths_are_distinct_and_legacy_live_storage_is_diagnosed() {
    let d = game();
    let path = d.path().join("stompymux.toml");
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replace("data/stompymux-rs.db", "state/live.db")
        .replace("data/stompymux.db", "archive/source.db");
    std::fs::write(path, text).unwrap();
    std::fs::create_dir(d.path().join("archive")).unwrap();
    std::fs::rename(
        d.path().join("data/stompymux.db"),
        d.path().join("archive/source.db"),
    )
    .unwrap();
    let c = Config::load(d.path()).unwrap();
    persistence::import(&c.legacy_database(), &c).await.unwrap();
    assert!(d.path().join("state/live.db").exists());
    assert!(!d.path().join("data/stompymux-rs.db").exists());
    assert!(
        format!(
            "{:#}",
            persistence::load(&c.legacy_database()).await.unwrap_err()
        )
        .contains("import-legacy")
    );
}
#[tokio::test(flavor = "current_thread")]
async fn acl_parsing_and_check_succeed_but_serve_has_no_side_effects() {
    let d = game();
    append(d.path(), "\n[access.lists]\noptions='wizard'\n");
    let c = Config::load(d.path()).unwrap();
    assert!(server::prepare(&c).await.is_err());
    assert!(!c.database().exists());
    assert!(!c.path(&c.database.bootstrap.credentials_file).exists());
    let before = std::fs::read(c.legacy_database()).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_stompymux-rs"))
        .args(["check", "--game-dir", d.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(before, std::fs::read(c.legacy_database()).unwrap());
}
#[tokio::test(flavor = "current_thread")]
async fn lua_sees_defaults_overrides_and_legacy_aliases() {
    let d = game();
    let c = Config::load(d.path())
        .unwrap()
        .with_listener_overrides(None, Some(8765))
        .unwrap();
    let w = persistence::read_legacy(&c.legacy_database(), &c)
        .await
        .unwrap();
    let scripts = Scripts::new(&c, Rc::new(RefCell::new(w))).unwrap();
    assert!(scripts.lua.load("return mux.config.get('port')==8765 and mux.config.get('server.port')==8765 and mux.config.get('btech_xp_usePilotBVMod')==1 and mux.config.get('runtime.input_line_limit')==8192").eval::<bool>().unwrap());
}
#[tokio::test(flavor = "current_thread")]
async fn configured_decoding_hashing_and_sqlite_timeouts_take_effect() {
    let (_d, c) = config(
        "runtime.input_line_limit=4\nruntime.telnet_subnegotiation_limit=2\nsecurity.password_hash_opslimit=1\nsecurity.password_hash_memlimit=1048576\ndatabase.busy_timeout_ms=1",
    );
    let mut decoder = Decoder::new(&c.runtime);
    assert!(matches!(&decoder.feed(b"test\n").unwrap()[0],Input::Line(s) if s=="test"));
    assert!(decoder.feed(b"12345").is_err());
    let mut decoder = Decoder::new(&c.runtime);
    assert!(decoder.feed(&[255, 250, 24, 0, 65, 66]).is_err());
    let hash = accounts::hash("secret", &c).unwrap();
    assert!(hash.contains("m=1024,t=1,p=1"));
    let world = World::default();
    persistence::initialize(&c.database(), &world)
        .await
        .unwrap();
    let mut conn = <sqlx::SqliteConnection as sqlx::Connection>::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(c.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    sqlx::raw_sql("BEGIN EXCLUSIVE")
        .execute(&mut conn)
        .await
        .unwrap();
    let now = Instant::now();
    assert!(
        persistence::save_with_timeout(&c.database(), &world, c.database.busy_timeout_ms)
            .await
            .is_err()
    );
    assert!(now.elapsed() < Duration::from_secs(1));
    sqlx::raw_sql("ROLLBACK").execute(&mut conn).await.unwrap();
    sqlx::Connection::close(conn).await.unwrap();
}
#[tokio::test(flavor = "current_thread")]
async fn configured_lua_limits_take_effect() {
    let d = game();
    append(d.path(), "\n[runtime]\noutput_message_limit=8\n");
    let c = Config::load(d.path()).unwrap();
    let w = persistence::read_legacy(&c.legacy_database(), &c)
        .await
        .unwrap();
    let s = Scripts::new(&c, Rc::new(RefCell::new(w))).unwrap();
    assert!(s.lua.load("mux.world.pemit(1,'123456789')").exec().is_err());
}

fn put(dir: &Path, key: &str, value: toml::Value) {
    let path = dir.join("stompymux.toml");
    let mut document: toml::Value =
        toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut table = document.as_table_mut().unwrap();
    let parts: Vec<_> = key.split('.').collect();
    for part in &parts[..parts.len() - 1] {
        table = table
            .entry((*part).to_owned())
            .or_insert_with(|| toml::Value::Table(Default::default()))
            .as_table_mut()
            .unwrap();
    }
    table.insert(parts[parts.len() - 1].into(), value);
    std::fs::write(path, toml::to_string(&document).unwrap()).unwrap();
}
#[tokio::test(flavor = "current_thread")]
async fn cli_uses_toml_listener_paths_and_optional_overrides() {
    use tokio::{
        io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
        net::TcpStream,
        process::Command,
    };
    let d = game();
    put(d.path(), "server.port", 0.into());
    put(d.path(), "server.listen_address", "127.0.0.2".into());
    put(
        d.path(),
        "mux.connect_file",
        "text/custom-connect.txt".into(),
    );
    put(d.path(), "mux.quit_file", "text/custom-quit.txt".into());
    put(d.path(), "runtime.input_line_limit", 128.into());
    put(d.path(), "security.login_history_limit", 1.into());
    std::fs::write(
        d.path().join("text/custom-connect.txt"),
        "Configured banner\r\n",
    )
    .unwrap();
    std::fs::write(
        d.path().join("text/custom-quit.txt"),
        "Configured goodbye\r\n",
    )
    .unwrap();
    let c = Config::load(d.path()).unwrap();
    persistence::import(&c.legacy_database(), &c).await.unwrap();
    let mut w = persistence::load(&c.database()).await.unwrap();
    w.accounts
        .get_mut(&stompymux_rs::world::ObjectId(2))
        .unwrap()
        .hash = Some(accounts::hash("secret", &c).unwrap());
    w.accounts
        .get_mut(&stompymux_rs::world::ObjectId(2))
        .unwrap()
        .history = (0..10)
        .map(|_| stompymux_rs::world::Login {
            success: true,
            at: 0,
            host: "old".into(),
        })
        .collect();
    persistence::save(&c.database(), &w).await.unwrap();
    async fn until(stream: &mut TcpStream, needle: &str) {
        tokio::time::timeout(Duration::from_secs(5), async {
            let mut collected = Vec::new();
            loop {
                let mut b = [0; 4096];
                let n = stream.read(&mut b).await.unwrap();
                assert!(n > 0, "closed waiting for {needle}");
                collected.extend_from_slice(&b[..n]);
                if String::from_utf8_lossy(&collected).contains(needle) {
                    break;
                }
            }
        })
        .await
        .unwrap();
    }
    for args in [vec![], vec!["--listen-address", "127.0.0.1", "--port", "0"]] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_stompymux-rs"))
            .args(["serve", "--game-dir", d.path().to_str().unwrap()])
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut reader = BufReader::new(child.stdout.take().unwrap());
        let mut ready = String::new();
        tokio::time::timeout(Duration::from_secs(10), reader.read_line(&mut ready))
            .await
            .unwrap()
            .unwrap();
        let address = ready
            .trim()
            .strip_prefix("Listening on ")
            .expect("server ready");
        assert!(address.starts_with(if args.is_empty() {
            "127.0.0.2:"
        } else {
            "127.0.0.1:"
        }));
        let mut stream = TcpStream::connect(address).await.unwrap();
        until(&mut stream, "Configured banner").await;
        stream.write_all(b"#2\r\n").await.unwrap();
        until(&mut stream, "Password: ").await;
        stream.write_all(b"secret\r\n").await.unwrap();
        until(&mut stream, "Staff Nexus").await;
        let persisted = persistence::load(&c.database()).await.unwrap();
        assert_eq!(
            persisted.accounts[&stompymux_rs::world::ObjectId(2)]
                .history
                .len(),
            1
        );
        stream.write_all(b"quit\r\n").await.unwrap();
        until(&mut stream, "Configured goodbye").await;
        child.kill().await.unwrap();
        child.wait().await.unwrap();
    }
}
#[tokio::test(flavor = "current_thread")]
async fn configured_bootstrap_objects_and_credentials_path_are_used() {
    let d = game();
    std::fs::remove_file(d.path().join("data/stompymux.db")).unwrap();
    put(
        d.path(),
        "database.bootstrap.credentials_file",
        "private/initial.txt".into(),
    );
    let mut objects = stompymux_rs::config::BootstrapConfig::default().objects;
    objects
        .get_mut(&stompymux_rs::config::BootstrapId(2))
        .unwrap()
        .wizard = false;
    objects.insert(
        stompymux_rs::config::BootstrapId(42),
        stompymux_rs::config::BootstrapObject {
            r#type: stompymux_rs::config::BootstrapKind::Room,
            name: "Configured room".into(),
            wizard: false,
        },
    );
    put(
        d.path(),
        "database.bootstrap.objects",
        toml::Value::try_from(objects).unwrap(),
    );
    let c = Config::load(d.path()).unwrap();
    let scripts = server::prepare(&c).await.unwrap();
    assert_eq!(
        scripts.world.borrow().objects[&stompymux_rs::world::ObjectId(42)].name,
        "Configured room"
    );
    assert!(
        !scripts.world.borrow().objects[&stompymux_rs::world::ObjectId(2)]
            .flags
            .contains(stompymux_rs::flags::Flag::Wizard)
    );
    assert!(d.path().join("private/initial.txt").exists());
    assert!(!d.path().join("bootstrap-credentials.txt").exists());
}
#[tokio::test(flavor = "current_thread")]
async fn instruction_and_output_entry_budgets_are_configurable() {
    let d = game();
    put(d.path(), "lua.instruction_limit", 10000.into());
    put(d.path(), "lua.output_entry_limit", 1.into());
    let c = Config::load(d.path()).unwrap();
    let w = persistence::read_legacy(&c.legacy_database(), &c)
        .await
        .unwrap();
    let scripts = Scripts::new(&c, Rc::new(RefCell::new(w))).unwrap();
    scripts
        .lua
        .load("mux.world.pemit(1,'first')")
        .exec()
        .unwrap();
    assert!(
        scripts
            .lua
            .load("mux.world.pemit(1,'second')")
            .exec()
            .is_err()
    );
    assert!(
        scripts
            .lua
            .load("for i=1,10000000 do local x=i*i end")
            .exec()
            .is_err()
    );
}

/// Parse the operator-supplied files without opening any live game data.
#[tokio::test(flavor = "current_thread")]
async fn supplied_configuration_parses_without_unknown_keys() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("stompymux.toml"),
        include_str!("../game/stompymux.toml"),
    )
    .unwrap();
    std::fs::write(
        d.path().join("aliases.toml"),
        include_str!("../game/aliases.toml"),
    )
    .unwrap();
    let c = Config::load(d.path()).unwrap();
    assert_eq!(KEYS.len(), 202);
    assert_eq!(c.server.port, 5555);
    assert!(
        !c.warnings.iter().any(|w| w.contains("unknown")),
        "{:?}",
        c.warnings
    );
}
