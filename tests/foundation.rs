use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};
use stompymux_rs::{
    accounts,
    config::Config,
    lua::Scripts,
    persistence, server,
    telnet::{Decoder, Input},
    world::{ObjectId, Scalar},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    process::{Child, Command},
};
fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dest = to.join(e.file_name());
        if e.path().is_dir() {
            copy(&e.path(), &dest)
        } else {
            std::fs::copy(e.path(), dest).unwrap();
        }
    }
}
fn fixture() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    copy(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"),
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
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(stompymux_rs::flags::Flag::Wizard);
    assert!(!s.lock(ObjectId(2), ObjectId(13)).unwrap());
    assert!(s.dispatch(ObjectId(1), 1, "global-hello").unwrap());
    assert!(s.outbox.borrow()[0].1.contains("Hello, world"));
    assert!(s.dispatch(ObjectId(1), 1, "flow-demo confirm").is_err());
}
#[tokio::test(flavor = "current_thread")]
async fn lua_resource_limits_and_fail_closed() {
    let (_d, c) = populated().await;
    let w = Rc::new(RefCell::new(
        persistence::load(&c.database()).await.unwrap(),
    ));
    let s = Scripts::new(&c, w).unwrap();
    assert!(s.lua.load("while true do end").exec().is_err());
    let s = Scripts::new(&c, s.world.clone()).unwrap();
    s.lua.set_memory_limit(s.lua.used_memory() + 32768).unwrap();
    assert!(
        s.lua
            .load("return string.rep('x',1000000)")
            .eval::<String>()
            .is_err()
    );
    let s = Scripts::new(&c, s.world.clone()).unwrap();
    assert!(
        s.lua
            .load("mux.world.object(1):state('test'):set('too_big',string.rep('x',65537))")
            .exec()
            .is_err()
    );
    s.world
        .borrow_mut()
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
    assert_eq!(s.world.borrow().objects.len(), 16);
    assert_eq!(s.world.borrow().channels.len(), 2);
    let before = std::fs::read(c.database()).unwrap();
    drop(s);
    let s = server::prepare(&c).await.unwrap();
    assert_eq!(s.world.borrow().objects.len(), 16);
    assert_eq!(before, std::fs::read(c.database()).unwrap());
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
    assert_eq!(s.world.borrow().objects.len(), 16);
    assert!(!c.root.join("bootstrap-credentials.txt").exists());
}
struct Running {
    child: Child,
    address: String,
}
impl Running {
    async fn start(c: &Config) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_stompymux-rs"))
            .args([
                "serve",
                "--game-dir",
                c.root.to_str().unwrap(),
                "--port",
                "0",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let line = tokio::time::timeout(Duration::from_secs(15), lines.next_line())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        Self {
            child,
            address: line.strip_prefix("Listening on ").unwrap().into(),
        }
    }
    async fn stop(mut self) {
        self.child.kill().await.unwrap();
        self.child.wait().await.unwrap();
    }
}
struct Client {
    socket: TcpStream,
    pending: Vec<u8>,
}
impl Client {
    async fn connect(server: &Running) -> Self {
        let mut c = Self {
            socket: TcpStream::connect(&server.address).await.unwrap(),
            pending: Vec::new(),
        };
        c.until("Who are you? ").await;
        c
    }
    async fn send(&mut self, s: &str) {
        self.socket
            .write_all(format!("{s}\r\n").as_bytes())
            .await
            .unwrap();
    }
    async fn until(&mut self, needle: &str) -> String {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let text = String::from_utf8_lossy(&self.pending).into_owned();
                if let Some(pos) = text.find(needle) {
                    let result = text[..pos + needle.len()].to_string();
                    self.pending.clear();
                    return result;
                }
                let mut b = [0u8; 4096];
                let n = self.socket.read(&mut b).await.unwrap();
                assert_ne!(n, 0, "closed waiting for {needle}: {text}");
                self.pending.extend_from_slice(&b[..n]);
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "timeout waiting for {needle}; received {:?}",
                String::from_utf8_lossy(&self.pending)
            )
        })
    }
    async fn register(&mut self, name: &str) {
        self.send(name).await;
        self.until("[Y/n] ").await;
        self.send("y").await;
        self.until("Choose a password: ").await;
        self.send("secret").await;
        self.until("Retype password: ").await;
        self.send("secret").await;
        self.until("Starter Room").await;
    }
    async fn login(&mut self, name: &str) {
        self.send(name).await;
        self.until("Password: ").await;
        self.send("secret").await;
        self.until("Starter Room").await;
    }
}
#[tokio::test(flavor = "current_thread")]
async fn tcp_register_social_world_and_restart() {
    let (_d, c) = populated().await;
    let running = Running::start(&c).await;
    let mut alice = Client::connect(&running).await;
    alice.register("Alice").await;
    let mut bob = Client::connect(&running).await;
    bob.register("Bob").await;
    alice.send("WHO").await;
    let who = alice.until("maximum.").await;
    assert!(who.contains("Alice") && who.contains("Bob"));
    alice.send("say Hello Bob").await;
    bob.until("Alice says \"Hello Bob\"").await;
    alice.send("out").await;
    alice.until("You cannot go that way.").await;
    alice.send("global-hello").await;
    alice
        .until("Hello, world, from a global Lua command!")
        .await;
    let mut second = Client::connect(&running).await;
    second.login("Alice").await;
    alice.send("quit").await;
    drop(alice);
    second.send("WHO").await;
    assert!(second.until("maximum.").await.contains("Alice"));
    running.stop().await;
    let running = Running::start(&c).await;
    let mut alice = Client::connect(&running).await;
    alice.login("Alice").await;
    let w = persistence::load(&c.database()).await.unwrap();
    assert_eq!(w.objects.len(), 18);
    assert_eq!(
        w.objects[&w.find_player("alice").unwrap()].location,
        Some(ObjectId(4))
    );
    running.stop().await;
}
#[tokio::test(flavor = "current_thread")]
async fn tcp_mismatch_bad_password_and_duplicate_registration() {
    let (_d, c) = populated().await;
    let running = Running::start(&c).await;
    let mut a = Client::connect(&running).await;
    let mut b = Client::connect(&running).await;
    for client in [&mut a, &mut b] {
        client.send("Racer").await;
        client.until("[Y/n] ").await;
        client.send("y").await;
        client.until("Choose a password: ").await;
        client.send("secret").await;
        client.until("Retype password: ").await;
    }
    a.send("wrong").await;
    a.until("Passwords did not match.").await;
    a.send("secret").await;
    a.until("Retype password: ").await;
    a.send("secret").await;
    a.until("Starter Room").await;
    b.send("secret").await;
    b.until("That name has just been registered.").await;
    b.send("Racer").await;
    b.until("Password: ").await;
    b.send("wrong").await;
    b.until("different password.").await;
    let w = persistence::load(&c.database()).await.unwrap();
    assert_eq!(w.accounts.len(), 3);
    running.stop().await;
}

#[tokio::test(flavor = "current_thread")]
async fn movement_persists_and_failed_registration_rolls_back() {
    let (_d, c) = populated().await;
    let mut w = persistence::load(&c.database()).await.unwrap();
    w.accounts.get_mut(&ObjectId(2)).unwrap().hash = Some(accounts::hash("secret", &c).unwrap());
    persistence::save(&c.database(), &w).await.unwrap();
    let running = Running::start(&c).await;
    let mut wizard = Client::connect(&running).await;
    wizard.send("Wizard").await;
    wizard.until("Password: ").await;
    wizard.send("secret").await;
    wizard.until("Staff Nexus").await;
    wizard.send("np").await;
    wizard.until("Starter Room").await;
    assert_eq!(
        persistence::load(&c.database()).await.unwrap().objects[&ObjectId(2)].location,
        Some(ObjectId(4))
    );
    let mut db = <sqlx::SqliteConnection as sqlx::Connection>::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(c.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TRIGGER reject_write BEFORE UPDATE ON snapshot BEGIN SELECT RAISE(ABORT,'injected persistence failure'); END;").execute(&mut db).await.unwrap();
    let mut newcomer = Client::connect(&running).await;
    newcomer.send("Unsaved").await;
    newcomer.until("[Y/n] ").await;
    newcomer.send("y").await;
    newcomer.until("Choose a password: ").await;
    newcomer.send("secret").await;
    newcomer.until("Retype password: ").await;
    newcomer.send("secret").await;
    newcomer.until("Unable to save login.").await;
    assert!(
        persistence::load(&c.database())
            .await
            .unwrap()
            .find_player("Unsaved")
            .is_none()
    );
    wizard.send("out").await;
    wizard.until("Unable to save your changes.").await;
    assert_eq!(
        persistence::load(&c.database()).await.unwrap().objects[&ObjectId(2)].location,
        Some(ObjectId(4))
    );
    sqlx::raw_sql("DROP TRIGGER reject_write")
        .execute(&mut db)
        .await
        .unwrap();
    newcomer.register("Unsaved").await;
    running.stop().await;
    sqlx::Connection::close(db).await.unwrap();
}
#[tokio::test(flavor = "current_thread")]
async fn login_throttle_utf8_and_echo_over_tcp() {
    let (d, _) = populated().await;
    let path = d.path().join("stompymux.toml");
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replace("login_attempt_burst = 100", "login_attempt_burst = 1");
    std::fs::write(path, text).unwrap();
    let c = Config::load(d.path()).unwrap();
    let running = Running::start(&c).await;
    let mut a = Client::connect(&running).await;
    a.socket.write_all(&[0xfe, b'\n']).await.unwrap();
    a.until("Invalid UTF-8").await;
    a.send("GOD").await;
    let response = a.until("Password: ").await;
    assert!(response.contains("Password:"));
    a.send("wrong").await;
    a.until("Who are you? ").await;
    a.send("GOD").await;
    a.until("Password: ").await;
    a.send("wrong").await;
    a.until("Too many login attempts.").await;
    running.stop().await;
}
#[tokio::test(flavor = "current_thread")]
async fn bounded_output_marks_slow_clients_for_disconnect() {
    use std::time::Instant;
    use stompymux_rs::sessions::{LoginFlow, Session};
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
async fn connection_hooks_only_disconnect_last_session_and_shutdown_cleanly() {
    let (d, c) = populated().await;
    let mut w = persistence::load(&c.database()).await.unwrap();
    w.accounts.get_mut(&ObjectId(1)).unwrap().hash = Some(accounts::hash("secret", &c).unwrap());
    persistence::save(&c.database(), &w).await.unwrap();
    std::fs::write(d.path().join("lua/global_logic/session_test.lua"),r#"return {events={
 on_player_connect=function(ctx)
  assert(mux.world.object(ctx.enactor):flags():has(mux.world.flags.CONNECTED))
  assert(ctx.scope=='global' and ctx.object==nil and ctx.cause==ctx.enactor)
  local s=mux.world.object(ctx.enactor):state('connections')
  s:set('connects',s:get('connects',0)+1);s:set('reconnect',ctx.reconnect)
 end,
 on_player_disconnect=function(ctx)
  assert(not mux.world.object(ctx.enactor):flags():has(mux.world.flags.CONNECTED))
  assert(type(ctx.reason)=='string')
  local s=mux.world.object(ctx.enactor):state('connections');s:set('disconnects',s:get('disconnects',0)+1)
 end}}
"#).unwrap();
    let mut running = Running::start(&c).await;
    let mut first = Client::connect(&running).await;
    first.send("GOD").await;
    first.until("Password: ").await;
    first.send("secret").await;
    first.until("Staff Nexus").await;
    let mut second = Client::connect(&running).await;
    second.send("GOD").await;
    second.until("Password: ").await;
    second.send("secret").await;
    second.until("Staff Nexus").await;
    let w = persistence::load(&c.database()).await.unwrap();
    assert_eq!(
        w.objects[&ObjectId(1)].state["connections"]["connects"],
        Scalar::Integer(2)
    );
    assert_eq!(
        w.objects[&ObjectId(1)].state["connections"]["reconnect"],
        Scalar::Boolean(true)
    );
    first.send("quit").await;
    drop(first);
    second.send("global-hello").await;
    second
        .until("Hello, world, from a global Lua command!")
        .await;
    assert!(
        !persistence::load(&c.database()).await.unwrap().objects[&ObjectId(1)].state["connections"]
            .contains_key("disconnects")
    );
    let pid = running.child.id().unwrap().to_string();
    assert!(
        Command::new("kill")
            .args(["-TERM", &pid])
            .status()
            .await
            .unwrap()
            .success()
    );
    let status = tokio::time::timeout(Duration::from_secs(10), running.child.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(status.success());
    let w = persistence::load(&c.database()).await.unwrap();
    assert_eq!(
        w.objects[&ObjectId(1)].state["connections"]["disconnects"],
        Scalar::Integer(1)
    );
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

#[tokio::test(flavor = "current_thread")]
async fn tcp_login_by_dbref_authenticates_existing_player() {
    let (_d, c) = populated().await;
    let mut world = persistence::load(&c.database()).await.unwrap();
    world.accounts.get_mut(&ObjectId(2)).unwrap().hash =
        Some(accounts::hash("secret", &c).unwrap());
    persistence::save(&c.database(), &world).await.unwrap();
    let running = Running::start(&c).await;
    let mut client = Client::connect(&running).await;
    client.send("#2").await;
    client.until("Password: ").await;
    client.send("wrong").await;
    client.until("Who are you? ").await;
    client.send("#2").await;
    client.until("Password: ").await;
    client.send("secret").await;
    client.until("Staff Nexus").await;
    client.send("WHO").await;
    assert!(client.until("maximum.").await.contains("Wizard"));
    let world = persistence::load(&c.database()).await.unwrap();
    assert_eq!(world.accounts.len(), 2);
    assert_eq!(world.objects.len(), 16);
    running.stop().await;
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
        s.lua.load(code).exec().unwrap();
    }
    s.lua
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
            commands::Action::StyledReport(text)
            | commands::Action::Report(text)
            | commands::Action::LiteralReport(text)
            | commands::Action::Reply(text) => text,
            _ => s.outbox.borrow_mut().pop().unwrap().1.source().to_string(),
        };
        assert!(text.contains(expected), "{input}: {text}");
    }
    {
        let mut w = world.borrow_mut();
        for _ in 0..2 {
            let id = w.create(&c, "Twin".into(), stompymux_rs::world::Kind::Thing);
            w.objects.get_mut(&id).unwrap().location = w.objects[&ObjectId(1)].location;
        }
        let exit = w.create(
            &c,
            "Test exit;shortcut".into(),
            stompymux_rs::world::Kind::Exit,
        );
        w.objects.get_mut(&exit).unwrap().location = w.objects[&ObjectId(1)].location;
    }
    for (input, expected) in [
        ("@flag Twin=dark", "which object"),
        ("@flag shortcut=dark", "DARK set."),
        ("@flag here=light", "LIGHT set."),
    ] {
        commands::run(&s, &c, ObjectId(1), 1, input).unwrap();
        assert!(s.outbox.borrow_mut().pop().unwrap().1.contains(expected));
    }
    let player =
        world
            .borrow_mut()
            .create(&c, "Ordinary".into(), stompymux_rs::world::Kind::Player);
    commands::run(&s, &c, player, 1, "@flag me=dark").unwrap();
    assert_eq!(
        s.outbox.borrow_mut().pop().unwrap().1.source(),
        "Permission denied."
    );
    assert!(!flags::controls(&world.borrow(), ObjectId(2), ObjectId(1)));
    assert!(flags::controls(&world.borrow(), ObjectId(2), ObjectId(2)));
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

/// Runtime flag state is observable over TCP, but never stored as durable truth.
#[tokio::test(flavor = "current_thread")]
async fn tcp_flags_follow_registration_and_multiple_sessions() {
    let (_d, c) = populated().await;
    let mut world = persistence::load(&c.database()).await.unwrap();
    world.accounts.get_mut(&ObjectId(1)).unwrap().hash =
        Some(accounts::hash("secret", &c).unwrap());
    persistence::save(&c.database(), &world).await.unwrap();
    let running = Running::start(&c).await;
    let mut admin = Client::connect(&running).await;
    admin.send("#1").await;
    admin.until("Password: ").await;
    admin.send("secret").await;
    admin.until("Staff Nexus").await;
    let mut first = Client::connect(&running).await;
    first.register("FlagTester").await;
    let id = persistence::load(&c.database())
        .await
        .unwrap()
        .find_player("FlagTester")
        .unwrap();
    admin.send(&format!("@examine #{}", id.0)).await;
    admin.until("CONNECTED").await;
    let mut second = Client::connect(&running).await;
    second.login(&format!("#{}", id.0)).await;
    first.send("quit").await;
    drop(first);
    admin.send(&format!("@flag #{}=DARK", id.0)).await;
    admin.until("DARK set.").await;
    admin.send(&format!("@examine #{}", id.0)).await;
    admin.until("CONNECTED").await;
    let mut db = <sqlx::SqliteConnection as sqlx::Connection>::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(c.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TRIGGER fail_flags BEFORE UPDATE ON snapshot BEGIN SELECT RAISE(FAIL,'injected'); END;").execute(&mut db).await.unwrap();
    admin.send(&format!("@flag #{}=!DARK", id.0)).await;
    admin.until("Unable to save your changes").await;
    admin.send(&format!("@examine #{}", id.0)).await;
    admin.until("DARK").await;
    sqlx::raw_sql("DROP TRIGGER fail_flags")
        .execute(&mut db)
        .await
        .unwrap();
    drop(second);
    // Poll the observable condition, allowing the independent socket-close event to arrive.
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            admin.send(&format!("@examine #{}", id.0)).await;
            let output = admin.until("IN_CHARACTER").await;
            if !output.contains("CONNECTED") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
    running.stop().await;
    let loaded = persistence::load(&c.database()).await.unwrap();
    assert!(
        loaded.objects[&id]
            .flags
            .contains(stompymux_rs::flags::Flag::Dark)
    );
    assert!(
        !loaded.objects[&id]
            .flags
            .contains(stompymux_rs::flags::Flag::Connected)
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
    s.lua
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
    use stompymux_rs::{commands, powers::Power, world::Kind};
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
            commands::Action::StyledReport(text)
            | commands::Action::Report(text)
            | commands::Action::LiteralReport(text)
            | commands::Action::Reply(text) => text,
            _ => scripts
                .outbox
                .borrow_mut()
                .pop()
                .unwrap()
                .1
                .source()
                .to_string(),
        };
        assert!(output.contains(expected), "{command}: {output}");
    }
    assert!(
        !world.borrow().objects[&ordinary]
            .powers
            .contains(Power::Idle)
    );
}

/// TCP changes survive restart; failed writes and callbacks cannot grant powers.
#[tokio::test(flavor = "current_thread")]
async fn tcp_powers_are_transactional_and_durable() {
    use stompymux_rs::powers::Power;
    let (d, c) = populated().await;
    let mut w = persistence::load(&c.database()).await.unwrap();
    w.accounts.get_mut(&ObjectId(1)).unwrap().hash = Some(accounts::hash("secret", &c).unwrap());
    w.objects
        .get_mut(&ObjectId(4))
        .unwrap()
        .powers
        .remove(Power::Idle);
    persistence::save(&c.database(), &w).await.unwrap();
    std::fs::write(
        d.path().join("lua/global_logic/power_failure.lua"),
        r#"return {commands={{name='power-failure',permission='everyone',pattern='^power%-failure$',handler=function(ctx)
        mux.world.object(4):powers():add(mux.world.powers.IDLE)
        mux.world.pemit(ctx.enactor,'SHOULD NOT APPEAR')
        error('injected power callback failure')
    end},{name='power-status',permission='everyone',pattern='^power%-status$',handler=function(ctx)
        mux.world.pemit(ctx.enactor,'Power status: '..tostring(mux.world.object(4):powers():has(mux.world.powers.IDLE)))
        return true
    end}}}"#,
    )
    .unwrap();
    let running = Running::start(&c).await;
    let mut admin = Client::connect(&running).await;
    admin.send("#1").await;
    admin.until("Password: ").await;
    admin.send("secret").await;
    admin.until("Staff Nexus").await;
    admin.send("power-failure").await;
    let output = admin.until("injected power callback failure").await;
    assert!(!output.contains("SHOULD NOT APPEAR"));
    assert!(
        !persistence::load(&c.database()).await.unwrap().objects[&ObjectId(4)]
            .powers
            .contains(Power::Idle)
    );
    admin.send("power-status").await;
    admin.until("Power status: false").await;
    let mut db = <sqlx::SqliteConnection as sqlx::Connection>::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(c.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TRIGGER fail_power BEFORE UPDATE ON snapshot BEGIN SELECT RAISE(FAIL,'injected'); END;").execute(&mut db).await.unwrap();
    admin.send("@power #4=idle").await;
    let output = admin.until("Unable to save your changes").await;
    assert!(!output.contains("granted"));
    assert!(
        !persistence::load(&c.database()).await.unwrap().objects[&ObjectId(4)]
            .powers
            .contains(Power::Idle)
    );
    admin.send("power-status").await;
    admin.until("Power status: false").await;
    sqlx::raw_sql("DROP TRIGGER fail_power")
        .execute(&mut db)
        .await
        .unwrap();
    admin.send("@power #4=IDLE").await;
    admin.until("idle granted.").await;
    running.stop().await;
    let restarted = Running::start(&c).await;
    assert!(
        persistence::load(&c.database()).await.unwrap().objects[&ObjectId(4)]
            .powers
            .contains(Power::Idle)
    );
    restarted.stop().await;
    sqlx::Connection::close(db).await.unwrap();
}

/// IDLE exempts ordinary authenticated accounts from inactivity timeout.
#[tokio::test(flavor = "current_thread")]
async fn idle_power_exempts_authenticated_player() {
    let (d, _) = populated().await;
    let path = d.path().join("stompymux.toml");
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replace("idle_interval = 99999", "idle_interval = 1")
        .replace("idle_timeout = 99999999", "idle_timeout = 1");
    std::fs::write(path, text).unwrap();
    let c = Config::load(d.path()).unwrap();
    let mut w = persistence::load(&c.database()).await.unwrap();
    w.accounts.get_mut(&ObjectId(2)).unwrap().hash = Some(accounts::hash("secret", &c).unwrap());
    w.objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .powers
        .insert(stompymux_rs::powers::Power::Idle);
    w.objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(stompymux_rs::flags::Flag::Wizard);
    w.accounts.get_mut(&ObjectId(1)).unwrap().hash = Some(accounts::hash("secret", &c).unwrap());
    persistence::save(&c.database(), &w).await.unwrap();
    let running = Running::start(&c).await;
    let mut client = Client::connect(&running).await;
    client.send("#2").await;
    client.until("Password: ").await;
    client.send("secret").await;
    client.until("Staff Nexus").await;
    tokio::time::sleep(Duration::from_secs(3)).await;
    client.send("look").await;
    client.until("Staff Nexus").await;
    let mut god = Client::connect(&running).await;
    god.send("#1").await;
    god.until("Password: ").await;
    god.send("secret").await;
    god.until("Staff Nexus").await;
    god.send("@power #2=!idle").await;
    god.until("removed.").await;
    client.until("*** Inactivity Timeout ***").await;
    running.stop().await;
}

/// Wizard movement handles containers, home, occupied containers and exit relocation.
#[tokio::test(flavor = "current_thread")]
async fn wizard_teleport_and_home_validate_containment() {
    use stompymux_rs::{commands, world::Kind};
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
        assert_eq!(
            s.outbox.borrow_mut().pop().unwrap().1.source(),
            "Permission denied."
        );
    }
    commands::run(&s, &c, ObjectId(1), 1, &format!("@tel #{}", cargo.0)).unwrap();
    assert_eq!(s.world.borrow().objects[&ObjectId(1)].location, Some(cargo));
    assert!(s.outbox.borrow().iter().any(|(_, t)| t.contains("Cargo")));
    commands::run(&s, &c, ObjectId(1), 1, "look").unwrap();
    assert!(s.outbox.borrow_mut().pop().unwrap().1.contains("Inner"));
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
        let before = s.world.borrow().clone();
        commands::run(&s, &c, ObjectId(1), 1, &input).unwrap();
        assert_eq!(
            serde_json::to_value(before).unwrap(),
            serde_json::to_value(&*s.world.borrow()).unwrap(),
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
    assert_eq!(s.world.borrow().objects[&ObjectId(2)].location, Some(cargo));
    let linked = s.world.borrow().objects[&ObjectId(13)].destination;
    commands::run(
        &s,
        &c,
        ObjectId(1),
        1,
        &format!("@teleport #13=#{}", cargo.0),
    )
    .unwrap();
    assert_eq!(s.world.borrow().objects[&ObjectId(13)].destination, linked);
    assert_eq!(
        s.world.borrow().objects[&ObjectId(13)].location,
        Some(cargo)
    );
    s.outbox.borrow_mut().clear();
    commands::run(&s, &c, ObjectId(1), 1, "HoMe").unwrap();
    assert_eq!(s.world.borrow().objects[&ObjectId(1)].location, stored_home);
    assert_eq!(
        s.outbox
            .borrow()
            .iter()
            .filter(|(_, t)| t.source() == "There's no place like home...")
            .count(),
        3
    );
    commands::run(&s, &c, ObjectId(1), 1, "@teleport #2").unwrap();
    assert_eq!(
        s.world.borrow().objects[&ObjectId(1)].location,
        Some(ObjectId(2))
    );
    for home in [
        None,
        Some(ObjectId(13)),
        Some(ObjectId(999999)),
        Some(ObjectId(1)),
    ] {
        s.world
            .borrow_mut()
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .home = home;
        let before = serde_json::to_value(&*s.world.borrow()).unwrap();
        commands::run(&s, &c, ObjectId(1), 1, "home").unwrap();
        assert_eq!(before, serde_json::to_value(&*s.world.borrow()).unwrap());
    }
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .home = stored_home;
    let snapshot = s.world.borrow().clone();
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
    use stompymux_rs::{commands, world::Kind};
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
      end},events={on_exit=function(ctx)
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
        s.lua.load(globals).exec().unwrap();
        let before = serde_json::to_value(&*s.world.borrow()).unwrap();
        commands::run(&s, &c, ObjectId(1), 1, request).unwrap();
        assert_eq!(before, serde_json::to_value(&*s.world.borrow()).unwrap());
        assert!(
            !s.outbox
                .borrow()
                .iter()
                .any(|(_, t)| t.source() == "LEAKED")
        );
    }
    s.lua
        .load("fail_enter=false;deny_out=false")
        .exec()
        .unwrap();
    commands::run(&s, &c, ObjectId(1), 1, request).unwrap();
    assert_eq!(
        s.world.borrow().objects[&ObjectId(2)].state["movement"]["cause"],
        Scalar::Integer(1)
    );
    s.lua.load("fail_exit=true").exec().unwrap();
    let before = serde_json::to_value(&*s.world.borrow()).unwrap();
    commands::run(
        &s,
        &c,
        ObjectId(1),
        1,
        &format!("@teleport #2=#{}", boxid.0),
    )
    .unwrap();
    assert_eq!(before, serde_json::to_value(&*s.world.borrow()).unwrap());
    // Going home bypasses both destination and nested teleport-out locks.
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .location = Some(boxid);
    s.lua
        .load("fail_exit=false;deny_destination=true;deny_out=true")
        .exec()
        .unwrap();
    commands::run(&s, &c, ObjectId(2), 2, "home").unwrap();
    assert_eq!(
        s.world.borrow().objects[&ObjectId(2)].location,
        Some(ObjectId(4))
    );
    let before = serde_json::to_value(&*s.world.borrow()).unwrap();
    commands::run(&s, &c, ObjectId(2), 2, "home").unwrap();
    assert_eq!(before, serde_json::to_value(&*s.world.borrow()).unwrap());
}

/// Connected targets see container appearance on every session; failed writes preserve location.
#[tokio::test(flavor = "current_thread")]
async fn tcp_teleport_containers_and_home_persist() {
    use stompymux_rs::world::Kind;
    let (_d, c) = populated().await;
    let mut w = persistence::load(&c.database()).await.unwrap();
    for id in [1, 2] {
        w.accounts.get_mut(&ObjectId(id)).unwrap().hash =
            Some(accounts::hash("secret", &c).unwrap());
    }
    let cargo = w.create(&c, "Teleport Cargo".into(), Kind::Thing);
    w.objects.get_mut(&cargo).unwrap().location = Some(ObjectId(4));
    persistence::save(&c.database(), &w).await.unwrap();
    let running = Running::start(&c).await;
    let mut admin = Client::connect(&running).await;
    admin.send("#1").await;
    admin.until("Password: ").await;
    admin.send("secret").await;
    admin.until("Staff Nexus").await;
    let mut first = Client::connect(&running).await;
    first.send("#2").await;
    first.until("Password: ").await;
    first.send("secret").await;
    first.until("Staff Nexus").await;
    let mut second = Client::connect(&running).await;
    second.send("#2").await;
    second.until("Password: ").await;
    second.send("secret").await;
    second.until("Staff Nexus").await;
    admin.send(&format!("@tel #2=#{}", cargo.0)).await;
    admin.until("Teleported.").await;
    first.until("Teleport Cargo").await;
    second.until("Teleport Cargo").await;
    let mut db = <sqlx::SqliteConnection as sqlx::Connection>::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(c.database()),
    )
    .await
    .unwrap();
    sqlx::raw_sql(
        "CREATE TRIGGER fail_move BEFORE UPDATE ON snapshot BEGIN SELECT RAISE(FAIL,'injected'); END;",
    )
    .execute(&mut db)
    .await
    .unwrap();
    admin.send("@teleport #2=#4").await;
    admin.until("Unable to save your changes").await;
    assert_eq!(
        persistence::load(&c.database()).await.unwrap().objects[&ObjectId(2)].location,
        Some(cargo)
    );
    sqlx::raw_sql("DROP TRIGGER fail_move")
        .execute(&mut db)
        .await
        .unwrap();
    first.send("home").await;
    first.until("Staff Nexus").await;
    second.until("Staff Nexus").await;
    admin.send(&format!("@teleport #{}", cargo.0)).await;
    admin.until("Teleport Cargo").await;
    running.stop().await;
    sqlx::Connection::close(db).await.unwrap();
    assert_eq!(
        persistence::load(&c.database()).await.unwrap().objects[&ObjectId(1)].location,
        Some(cargo)
    );
    let restarted = Running::start(&c).await;
    restarted.stop().await;
}

/// Automatic search reports stay private, reject continuations and perform no writes.
#[tokio::test(flavor = "current_thread")]
async fn tcp_search_reports_are_private_and_read_only() {
    let (d, _) = populated().await;
    let path = d.path().join("stompymux.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        format!("{text}\n[runtime]\noutput_message_limit=256\n"),
    )
    .unwrap();
    let c = Config::load(d.path()).unwrap();
    let mut w = persistence::load(&c.database()).await.unwrap();
    for id in [1, 2] {
        w.accounts.get_mut(&ObjectId(id)).unwrap().hash =
            Some(accounts::hash("secret", &c).unwrap());
    }
    for i in 0..80 {
        w.create(
            &c,
            format!("SearchRoom{i:03}"),
            stompymux_rs::world::Kind::Room,
        );
    }
    persistence::save(&c.database(), &w).await.unwrap();
    let running = Running::start(&c).await;
    let mut first = Client::connect(&running).await;
    let mut second = Client::connect(&running).await;
    for client in [&mut first, &mut second] {
        client.send("#2").await;
        client.until("Password: ").await;
        client.send("secret").await;
        client.until("Staff Nexus").await;
    }
    let mut ordinary = Client::connect(&running).await;
    ordinary.register("Finder").await;
    let before = std::fs::read(c.database()).unwrap();
    for command in ["@find", "@find/next", "@search", "@stats", "@list commands"] {
        ordinary.send(command).await;
        ordinary.until("Permission denied.").await;
    }
    first.send("@FI SearchRoom").await;
    let text = first.until("***End of List***").await;
    assert!(text.contains("SearchRoom000") && text.contains("SearchRoom079"));
    second.send("@fin/next").await;
    let text = second.until("Unsupported @find switch.").await;
    assert!(!text.contains("SearchRoom"));
    for command in ["@find/next extra", "@find/unknown"] {
        first.send(command).await;
        first.until("Unsupported @find switch.").await;
    }
    first.send("@search rooms=SearchRoom").await;
    let text = first.until("Garbage...0").await;
    assert!(text.contains("Rooms...80"));
    first.send("@stats").await;
    first.until("garbage)").await;
    first.send("@list commands").await;
    first.until("Global commands (global Lua):").await;
    // A later command synchronizes with the entire preceding listing.
    first.send("@find missing").await;
    first.until("***End of List***").await;
    assert_eq!(before, std::fs::read(c.database()).unwrap());
    running.stop().await;
}

/// The shared registry applies native/Lua aliases and observes changed authority over TCP.
#[tokio::test(flavor = "current_thread")]
async fn tcp_registry_permissions_and_lua_aliases() {
    let (d, c) = populated().await;
    let mut w = persistence::load(&c.database()).await.unwrap();
    for id in [1, 2] {
        w.accounts.get_mut(&ObjectId(id)).unwrap().hash =
            Some(accounts::hash("secret", &c).unwrap());
    }
    persistence::save(&c.database(), &w).await.unwrap();
    let path = d.path().join("stompymux.toml");
    let original = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        format!("{original}\n[aliases.commands]\nrp='registry-probe'\nre='@examine'\n"),
    )
    .unwrap();
    std::fs::write(d.path().join("lua/global_logic/registry.lua"),r#"return {commands={
      {name='registry-probe',permission='god',pattern='^registry%-probe%s+(.*)$',handler=function(ctx,value) mux.world.pemit(ctx.enactor,'GOD probe: '..value); return true end},
      {name='registry-probe',permission='wizard',pattern='^registry%-probe%s+(.*)$',handler=function(ctx,value) mux.world.pemit(ctx.enactor,'Wizard probe: '..value); return true end},
      {name='registry-probe',permission='everyone',pattern='^registry%-probe%s+(.*)$',handler=function(ctx,value) mux.world.pemit(ctx.enactor,'Everyone probe: '..value); return true end}
    }}"#).unwrap();
    let c = Config::load(d.path()).unwrap();
    let running = Running::start(&c).await;
    let mut god = Client::connect(&running).await;
    let mut wizard = Client::connect(&running).await;
    for (client, name) in [(&mut god, "#1"), (&mut wizard, "#2")] {
        client.send(name).await;
        client.until("Password: ").await;
        client.send("secret").await;
        client.until("Staff Nexus").await;
    }
    god.send("RP MiXeD words").await;
    god.until("GOD probe: MiXeD words").await;
    wizard.send("rp MiXeD words").await;
    wizard.until("Wizard probe: MiXeD words").await;
    wizard.send("RE #2").await;
    wizard.until("Powers:").await;
    god.send("@flag #2=!wizard").await;
    god.until("cleared.").await;
    wizard.send("rp after change").await;
    wizard.until("Everyone probe: after change").await;
    wizard.send("re/unknown #2").await;
    wizard.until("Permission denied.").await;
    god.send("@flag #2=wizard").await;
    god.until("set.").await;
    wizard.send("re/unknown #2").await;
    wizard.until("Unsupported command switch.").await;
    wizard.send("rp restored").await;
    wizard.until("Wizard probe: restored").await;
    running.stop().await;
}

/// Every shutdown origin uses the same last-session hooks, persistence and output drain.
#[tokio::test(flavor = "current_thread")]
async fn shutdown_origins_share_cleanup_and_stop_pipelined_commands() {
    for origin in ["-INT", "-TERM", "command"] {
        let (d, c) = populated().await;
        let mut w = persistence::load(&c.database()).await.unwrap();
        w.accounts.get_mut(&ObjectId(2)).unwrap().hash =
            Some(accounts::hash("secret", &c).unwrap());
        w.objects.get_mut(&ObjectId(2)).unwrap().location = Some(ObjectId(c.start()));
        persistence::save(&c.database(), &w).await.unwrap();
        std::fs::write(
            d.path().join("lua/global_logic/shutdown_test.lua"),
            r#"return {events={on_player_disconnect=function(ctx)
          local o=mux.world.object(ctx.enactor)
          assert(not o:flags():has(mux.world.flags.CONNECTED))
          local s=o:state('shutdown');s:set('disconnects',s:get('disconnects',0)+1)
        end}}"#,
        )
        .unwrap();
        let mut running = Running::start(&c).await;
        let mut first = Client::connect(&running).await;
        first.login("#2").await;
        let mut second = Client::connect(&running).await;
        second.login("#2").await;
        let mut pending = Client::connect(&running).await;
        pending.send("Unfinished").await;
        pending.until("[Y/n]").await;
        if origin == "command" {
            first.send("@shutdown\r\n@flag me=DARK\r\n@shutdown").await;
        } else {
            assert!(
                Command::new("kill")
                    .args([origin, &running.child.id().unwrap().to_string()])
                    .status()
                    .await
                    .unwrap()
                    .success()
            );
        }
        let status = tokio::time::timeout(Duration::from_secs(10), running.child.wait())
            .await
            .unwrap()
            .unwrap();
        assert!(status.success(), "{origin}");
        assert!(TcpStream::connect(&running.address).await.is_err());
        for client in [&mut first, &mut second, &mut pending] {
            let mut rest = Vec::new();
            tokio::time::timeout(Duration::from_secs(2), client.socket.read_to_end(&mut rest))
                .await
                .unwrap()
                .unwrap();
            if origin == "command" {
                assert!(String::from_utf8_lossy(&rest).contains("Game: Shutdown by Wizard"));
            }
        }
        let loaded = persistence::load(&c.database()).await.unwrap();
        assert_eq!(
            loaded.objects[&ObjectId(2)].state["shutdown"]["disconnects"],
            Scalar::Integer(1)
        );
        assert!(
            !loaded.objects[&ObjectId(2)]
                .flags
                .contains(stompymux_rs::flags::Flag::Dark)
        );
        assert!(
            !loaded.objects[&ObjectId(2)]
                .flags
                .contains(stompymux_rs::flags::Flag::Connected)
        );
        assert!(loaded.find_player("Unfinished").is_none());
    }
}

/// Command shutdown cancels on initial write failure; both signal origins still terminate unsuccessfully.
#[tokio::test(flavor = "current_thread")]
async fn shutdown_write_failures_cancel_commands_but_fail_signal_exit_status() {
    for origin in ["command", "-INT", "-TERM"] {
        let (_d, c) = populated().await;
        let mut w = persistence::load(&c.database()).await.unwrap();
        w.accounts.get_mut(&ObjectId(2)).unwrap().hash =
            Some(accounts::hash("secret", &c).unwrap());
        w.objects.get_mut(&ObjectId(2)).unwrap().location = Some(ObjectId(c.start()));
        persistence::save(&c.database(), &w).await.unwrap();
        let mut running = Running::start(&c).await;
        let mut client = Client::connect(&running).await;
        client.login("#2").await;
        let mut sql = <sqlx::SqliteConnection as sqlx::Connection>::connect_with(
            &sqlx::sqlite::SqliteConnectOptions::new().filename(c.database()),
        )
        .await
        .unwrap();
        sqlx::raw_sql("UPDATE objects SET has_connected_flag=1 WHERE dbref=2; CREATE TRIGGER shutdown_fail BEFORE UPDATE ON objects BEGIN SELECT RAISE(FAIL,'shutdown write failure'); END;").execute(&mut sql).await.unwrap();
        if origin == "command" {
            client.send("@shutdown").await;
            client.until("Shutdown cancelled").await;
            client.send("look").await;
            client.until("Starter Room").await;
            assert!(running.child.try_wait().unwrap().is_none());
            sqlx::query("DROP TRIGGER shutdown_fail")
                .execute(&mut sql)
                .await
                .unwrap();
            client.send("@shutdown").await;
        } else {
            Command::new("kill")
                .args([origin, &running.child.id().unwrap().to_string()])
                .status()
                .await
                .unwrap();
        }
        let status = tokio::time::timeout(Duration::from_secs(10), running.child.wait())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(status.success(), origin == "command");
        assert!(TcpStream::connect(&running.address).await.is_err());
        sqlx::Connection::close(sql).await.unwrap();
    }
}

/// Maintenance reserves command names, persists purges before disconnecting and rolls back failed purges.
#[tokio::test(flavor = "current_thread")]
async fn tcp_dbck_permissions_aliases_purges_and_rollback() {
    let (d, _c) = populated().await;
    let aliases = d.path().join("aliases.toml");
    std::fs::write(
        &aliases,
        std::fs::read_to_string(&aliases).unwrap().replace(
            "[aliases.commands]",
            "[aliases.commands]\nrepair='@dbck'\nstop='@shutdown'",
        ),
    )
    .unwrap();
    let c = Config::load(d.path()).unwrap();
    let mut w = persistence::load(&c.database()).await.unwrap();
    w.accounts.get_mut(&ObjectId(1)).unwrap().hash = Some(accounts::hash("secret", &c).unwrap());
    w.objects.get_mut(&ObjectId(1)).unwrap().location = Some(ObjectId(c.start()));
    persistence::save(&c.database(), &w).await.unwrap();
    let mut running = Running::start(&c).await;
    let mut wizard = Client::connect(&running).await;
    wizard.login("#1").await;
    let mut player = Client::connect(&running).await;
    player.register("Doomed").await;
    let mut other = Client::connect(&running).await;
    other.login("Doomed").await;
    for command in ["@dbck", "repair/nope", "stop", "@shutdown/reason"] {
        player.send(command).await;
        player.until("Permission denied.").await;
    }
    for command in ["@dbck/nope", "@shutdown/nope"] {
        wizard.send(command).await;
        wizard.until("Unsupported command switch.").await;
    }
    for command in ["@dbck bad", "@shutdown reasons"] {
        wizard.send(command).await;
        wizard.until("Usage:").await;
    }
    wizard.send("@flag Doomed=GOING").await;
    wizard.until("set.").await;
    let mut sql = <sqlx::SqliteConnection as sqlx::Connection>::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(c.database()),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TRIGGER fail_dbck BEFORE UPDATE ON objects WHEN NEW.type=5 BEGIN SELECT RAISE(FAIL,'purge blocked'); END").execute(&mut sql).await.unwrap();
    wizard.send("repair").await;
    wizard.until("no repairs committed").await;
    player.send("look").await;
    player.until("Starter Room").await;
    assert!(
        persistence::load(&c.database())
            .await
            .unwrap()
            .find_player("Doomed")
            .is_some()
    );
    sqlx::query("DROP TRIGGER fail_dbck")
        .execute(&mut sql)
        .await
        .unwrap();
    wizard.send("repair").await;
    wizard.until("Done.").await;
    player.until("You have been destroyed!").await;
    other.until("You have been destroyed!").await;
    assert!(
        persistence::load(&c.database())
            .await
            .unwrap()
            .find_player("Doomed")
            .is_none()
    );
    wizard.send("stop").await;
    assert!(
        tokio::time::timeout(Duration::from_secs(10), running.child.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
    sqlx::Connection::close(sql).await.unwrap();
}

/// Repair movement bypasses locks, carries the moved session, and rolls back all callback mutations.
#[tokio::test(flavor = "current_thread")]
async fn tcp_dbck_relocation_callbacks_rollback_and_context() {
    let (d, c) = populated().await;
    let mut w = persistence::load(&c.database()).await.unwrap();
    w.accounts.get_mut(&ObjectId(2)).unwrap().hash = Some(accounts::hash("secret", &c).unwrap());
    w.objects.get_mut(&ObjectId(2)).unwrap().location = Some(ObjectId(c.start()));
    w.objects.get_mut(&ObjectId(2)).unwrap().home = Some(ObjectId(0));
    for id in [c.start(), 0] {
        w.objects.get_mut(&ObjectId(id)).unwrap().lua_parent = "repair_hooks.lua".into();
    }
    persistence::save(&c.database(), &w).await.unwrap();
    std::fs::write(d.path().join("lua/object_logic/repair_hooks.lua"),r#"return {
      locks={teleport=function() error('repair must bypass locks') end, teleport_out=function() error('repair must bypass locks') end},
      events={on_exit=function(ctx)
        assert(ctx.source==ctx.object and ctx.cause==2)
        local o=mux.world.object(ctx.enactor);o:state('repair'):set('exit',true)
        if ctx.enactor==2 then assert(ctx.descriptor~=nil);assert(o:flags():has(mux.world.flags.CONNECTED)) end
      end,on_enter=function(ctx)
        assert(ctx.destination==ctx.object and ctx.cause==2)
        local o=mux.world.object(ctx.enactor);o:state('repair'):set('enter',true)
        if not repair_allowed then mux.world.pemit(ctx.enactor,'LEAKED');error('repair callback failed') end
      end},commands={{name='allow-repair',permission='wizard',pattern='^allow%-repair$',handler=function(ctx)
        repair_allowed=true;mux.world.pemit(ctx.enactor,'Repair enabled.');return true
      end}}}"#).unwrap();
    std::fs::write(d.path().join("lua/global_logic/repair_control.lua"), r#"return {commands={{name='allow-repair',permission='wizard',pattern='^allow%-repair$',handler=function(ctx)
      repair_allowed=true;mux.world.pemit(ctx.enactor,'Repair enabled.');return true
    end}}}"#).unwrap();
    let running = Running::start(&c).await;
    let mut wizard = Client::connect(&running).await;
    wizard.login("#2").await;
    let mut sql = <sqlx::SqliteConnection as sqlx::Connection>::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(c.database()),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE objects SET contents=-1 WHERE dbref=?")
        .bind(c.start())
        .execute(&mut sql)
        .await
        .unwrap();
    let bytes = std::fs::read(c.database()).unwrap();
    wizard.send("@dbck").await;
    let failed = wizard.until("no repairs committed").await;
    assert!(!failed.contains("LEAKED"));
    assert_eq!(bytes, std::fs::read(c.database()).unwrap());
    let loaded = persistence::load(&c.database()).await.unwrap();
    assert!(!loaded.objects[&ObjectId(2)].state.contains_key("repair"));
    wizard.send("allow-repair").await;
    wizard.until("Repair enabled.").await;
    wizard.send("@dbck").await;
    wizard.until("Done.").await;
    let loaded = persistence::load(&c.database()).await.unwrap();
    assert_eq!(loaded.objects[&ObjectId(2)].location, Some(ObjectId(0)));
    assert_eq!(
        loaded.objects[&ObjectId(2)].state["repair"]["exit"],
        Scalar::Boolean(true)
    );
    assert_eq!(
        loaded.objects[&ObjectId(2)].state["repair"]["enter"],
        Scalar::Boolean(true)
    );
    sqlx::Connection::close(sql).await.unwrap();
    running.stop().await;
    server::prepare(&c).await.unwrap();
}

/// Read raw protocol bytes through a text marker without decoding away IAC commands.
async fn telnet_until(socket: &mut TcpStream, marker: &[u8]) -> Vec<u8> {
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut out = Vec::new();
        while !out.windows(marker.len()).any(|v| v == marker) {
            let mut buffer = [0; 4096];
            let n = socket.read(&mut buffer).await.unwrap();
            assert_ne!(
                n,
                0,
                "closed before marker: {:?}",
                String::from_utf8_lossy(&out)
            );
            out.extend_from_slice(&buffer[..n]);
        }
        out
    })
    .await
    .unwrap()
}
/// Login actions must run before an acknowledgement later in the same packet.
#[tokio::test(flavor = "current_thread")]
async fn tcp_q_echo_ordering_reversals_and_refusal() {
    let (_d, c) = populated().await;
    // Exercise repeated protocol reversals without exhausting the credential policy.
    let path = c.root.join("stompymux.toml");
    let mut doc: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    doc["mux"]
        .as_table_mut()
        .unwrap()
        .insert("retry_limit".into(), 10.into());
    std::fs::write(&path, toml::to_string(&doc).unwrap()).unwrap();
    let c = Config::load(&c.root).unwrap();
    let running = Running::start(&c).await;
    let mut socket = TcpStream::connect(&running.address).await.unwrap();
    let initial = telnet_until(&mut socket, b"Who are you? ").await;
    assert!(initial.starts_with(&[
        255, 253, 24, 255, 253, 31, 255, 253, 39, 255, 251, 70, 255, 251, 86, 255, 251, 42, 255,
        251, 201
    ]));
    socket.write_all(b"GOD\r\n\xff\xfd\x01").await.unwrap();
    let password = telnet_until(&mut socket, b"Password: ").await;
    assert!(password.windows(3).any(|w| w == [255, 251, 1]));
    assert!(!password.windows(3).any(|w| w == [255, 252, 1]));
    socket.write_all(b"wrong\r\n").await.unwrap();
    let failed = telnet_until(&mut socket, b"Who are you? ").await;
    assert_eq!(failed.windows(3).filter(|w| *w == [255, 252, 1]).count(), 1);
    // The next password flow starts before acknowledgement of WONT. The queued reversal
    // is sent only when DONT arrives; its DO acknowledgement follows in this same packet.
    socket
        .write_all(b"GOD\r\n\xff\xfe\x01\xff\xfd\x01wrong\r\n")
        .await
        .unwrap();
    let retry = telnet_until(&mut socket, b"Who are you? ").await;
    assert_eq!(retry.windows(3).filter(|w| *w == [255, 251, 1]).count(), 1);
    assert_eq!(retry.windows(3).filter(|w| *w == [255, 252, 1]).count(), 1);
    socket
        .write_all(b"\xff\xfe\x01GOD\r\n\xff\xfe\x01wrong\r\n")
        .await
        .unwrap();
    let refused = telnet_until(&mut socket, b"Who are you? ").await;
    assert_eq!(
        refused.windows(3).filter(|w| *w == [255, 251, 1]).count(),
        1
    );
    assert!(!refused.windows(3).any(|w| w == [255, 252, 1]));
    running.stop().await;
}
/// Registration proceeds without negotiation replies and restores echo once a delayed reply arrives.
#[tokio::test(flavor = "current_thread")]
async fn tcp_q_registration_allows_unanswered_echo_and_coalesces_prompts() {
    let (_d, c) = populated().await;
    let running = Running::start(&c).await;
    let mut socket = TcpStream::connect(&running.address).await.unwrap();
    telnet_until(&mut socket, b"Who are you? ").await;
    socket.write_all(b"QTester\r\n").await.unwrap();
    telnet_until(&mut socket, b"[Y/n] ").await;
    socket
        .write_all(b"y\r\nsecret\r\nsecret\r\n")
        .await
        .unwrap();
    let registered = telnet_until(&mut socket, b"Starter Room").await;
    assert_eq!(
        registered
            .windows(3)
            .filter(|w| *w == [255, 251, 1])
            .count(),
        1
    );
    assert!(!registered.windows(3).any(|w| w == [255, 252, 1]));
    assert!(
        persistence::load(&c.database())
            .await
            .unwrap()
            .find_player("QTester")
            .is_some()
    );
    socket.write_all(&[255, 253, 1]).await.unwrap();
    let restored = telnet_until(&mut socket, &[255, 252, 1]).await;
    assert_eq!(
        restored.windows(3).filter(|w| *w == [255, 252, 1]).count(),
        1
    );
    socket.write_all(b"\xff\xfe\x01look\r\n").await.unwrap();
    let looked = telnet_until(&mut socket, b"Starter Room").await;
    assert!(
        !looked
            .windows(3)
            .any(|w| w == [255, 251, 1] || w == [255, 252, 1])
    );
    running.stop().await;
}

/// Live options and both diagnostic commands remain session-private and read-only.
#[tokio::test(flavor = "current_thread")]
async fn tcp_extended_telnet_and_session_diagnostics() {
    let (d, c) = populated().await;
    let aliases = d.path().join("aliases.toml");
    std::fs::write(
        &aliases,
        std::fs::read_to_string(&aliases).unwrap().replace(
            "[aliases.commands]",
            "[aliases.commands]\nss='@session'\ntn='@telnet'",
        ),
    )
    .unwrap();
    let mut w = persistence::load(&c.database()).await.unwrap();
    w.accounts.get_mut(&ObjectId(2)).unwrap().hash = Some(accounts::hash("secret", &c).unwrap());
    w.objects.get_mut(&ObjectId(2)).unwrap().location = Some(ObjectId(c.start()));
    persistence::save(&c.database(), &w).await.unwrap();
    let running = Running::start(&c).await;
    let mut wizard = Client::connect(&running).await;
    wizard.login("#2").await;
    let mut other = Client::connect(&running).await;
    other.login("#2").await;
    let mut ordinary = Client::connect(&running).await;
    ordinary.register("Spectator").await;
    ordinary.send("@session").await;
    ordinary.until("Permission denied.").await;
    ordinary.send("@telnet Wizard").await;
    ordinary.until("Permission denied.").await;
    wizard.send("@session/bad").await;
    wizard.until("Unsupported command switch.").await;
    wizard.send("@telnet").await;
    wizard.until("Usage: @telnet <player>").await;
    wizard.send("@telnet absent").await;
    wizard.until("No such player.").await;
    wizard.send("@telnet GOD").await;
    wizard.until("That player is not connected.").await;
    let db = std::fs::read(c.database()).unwrap();
    wizard.socket.write_all(b"\xff\xfb\x27\xff\xfa\x27\x00\x00CLIENT\x01hello\xff\xf0\xff\xfd\x46\xff\xfd\xc9\xff\xfa\xc9Core.Ping {}\xff\xf0").await.unwrap();
    wizard.send("tn #2").await;
    let bytes = telnet_until(&mut wizard.socket, b"Client echo (requested): enabled").await;
    let text = String::from_utf8_lossy(&bytes);
    assert!(bytes.windows(3).any(|v| v == [255, 250, 70]));
    assert!(text.contains("NAME\u{2}"));
    assert!(text.contains("PLAYERS\u{2}3"));
    assert!(text.contains("CODEBASE\u{2}stompymux-rs"));
    assert!(text.contains("Core.Ping"));
    assert!(text.contains("VAR \"CLIENT\" = \"hello\""));
    assert!(text.contains("Q local:"));
    // Use a subsequent marker to synchronize with both complete diagnostic blocks.
    wizard.send("ss Wiz").await;
    let rows = wizard.until("maximum.").await;
    assert!(rows.contains("2 Players logged in"));
    assert!(rows.contains("Session"));
    assert!(!rows.contains("Spectator"));
    other.send("@telnet #2").await;
    let second = other.until("Client echo (requested): enabled").await;
    assert!(second.contains("session"));
    assert_eq!(db, std::fs::read(c.database()).unwrap());
    running.stop().await;
}

/// A real compressed connection handles Telnet negotiation and graceful shutdown in one zlib stream.
#[tokio::test(flavor = "current_thread")]
async fn tcp_mccp2_stream_and_shutdown() {
    use std::io::Read;
    let (_d, c) = populated().await;
    let mut running = Running::start(&c).await;
    let mut socket = TcpStream::connect(&running.address).await.unwrap();
    telnet_until(&mut socket, b"Who are you? ").await;
    socket.write_all(&[255, 253, 86]).await.unwrap();
    let marker = telnet_until(&mut socket, &[255, 250, 86, 255, 240]).await;
    let boundary = marker
        .windows(5)
        .position(|v| v == [255, 250, 86, 255, 240])
        .unwrap()
        + 5;
    let mut compressed = marker[boundary..].to_vec();
    socket
        .write_all(b"\xff\xfd\x56\xff\xfe\x56\xff\xfd\xc9\xff\xfa\xc9Core.Ping\xff\xf0Nobody\r\n")
        .await
        .unwrap();
    // Keep the peer open while verifying sync-flushed protocol output.
    let mut inflater = flate2::Decompress::new(true);
    let mut plain = Vec::new();
    let mut offset = 0;
    tokio::time::timeout(Duration::from_secs(10), async {
        while !plain.windows(6).any(|v| v == b"[Y/n] ") {
            if offset == compressed.len() {
                let mut b = [0; 4096];
                let n = socket.read(&mut b).await.unwrap();
                assert!(n > 0);
                compressed.extend_from_slice(&b[..n]);
            }
            let mut out = [0; 4096];
            let before = (inflater.total_in(), inflater.total_out());
            inflater
                .decompress(
                    &compressed[offset..],
                    &mut out,
                    flate2::FlushDecompress::Sync,
                )
                .unwrap();
            offset += (inflater.total_in() - before.0) as usize;
            plain.extend_from_slice(&out[..(inflater.total_out() - before.1) as usize]);
        }
    })
    .await
    .unwrap();
    assert!(plain.windows(3).any(|v| v == [255, 252, 86]));
    assert!(plain.windows(9).any(|v| v == b"Core.Ping"));
    assert!(!plain.windows(5).any(|v| v == [255, 250, 86, 255, 240]));
    // Complete registration, then verify communication output within the same zlib stream.
    for (input, needle) in [
        (b"y\r\nsecret\r\nsecret\r\n".as_slice(), "Starter Room"),
        (
            b"pub CompressedChannel\r\npage Nobody=CompressedPage\r\n".as_slice(),
            "You paged Nobody",
        ),
        (b":CompressedPose\r\n".as_slice(), "Nobody CompressedPose"),
        (b"flow-demo confirm\r\ny\r\n".as_slice(), "Done."),
        (b"flow-demo menu\r\n".as_slice(), "Choice: "),
    ] {
        socket.write_all(input).await.unwrap();
        tokio::time::timeout(Duration::from_secs(10), async {
            while !String::from_utf8_lossy(&plain).contains(needle) {
                if offset == compressed.len() {
                    let mut input = [0; 4096];
                    let count = socket.read(&mut input).await.unwrap();
                    assert!(count > 0);
                    compressed.extend_from_slice(&input[..count]);
                }
                let mut output = [0; 4096];
                let before = (inflater.total_in(), inflater.total_out());
                inflater
                    .decompress(
                        &compressed[offset..],
                        &mut output,
                        flate2::FlushDecompress::Sync,
                    )
                    .unwrap();
                offset += (inflater.total_in() - before.0) as usize;
                plain.extend_from_slice(&output[..(inflater.total_out() - before.1) as usize]);
            }
        })
        .await
        .unwrap();
    }
    let rendered = String::from_utf8_lossy(&plain);
    assert!(rendered.contains("[Public] Nobody: CompressedChannel"));
    assert!(rendered.contains("Nobody pages: CompressedPage"));
    assert!(
        rendered.contains("\x1b[1mReally do the thing? (y/n) \x1b[0mDone."),
        "{rendered:?}"
    );
    Command::new("kill")
        .args(["-TERM", &running.child.id().unwrap().to_string()])
        .status()
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(10), socket.read_to_end(&mut compressed))
        .await
        .unwrap()
        .unwrap();
    assert!(running.child.wait().await.unwrap().success());
    let mut decoded = Vec::new();
    flate2::read::ZlibDecoder::new(compressed.as_slice())
        .read_to_end(&mut decoded)
        .unwrap();
    assert_eq!(decoded, plain);
}

/// Markdown and styled messages render independently per socket; help/color stay read-only.
#[tokio::test(flavor = "current_thread")]
async fn tcp_rich_text_help_color_and_rollback() {
    let (d, c) = populated().await;
    let aliases = d.path().join("aliases.toml");
    std::fs::write(
        &aliases,
        std::fs::read_to_string(&aliases)
            .unwrap()
            .replace("[aliases.commands]", "[aliases.commands]\ncol='color'"),
    )
    .unwrap();
    copy(Path::new("game/help"), &d.path().join("help"));
    std::fs::write(d.path().join("lua/global_logic/rich_test.lua"),r#"
return {commands={
 {name='rich',permission='everyone',pattern='^rich$',handler=function(ctx)
 mux.world.pemit(ctx.enactor,mux.text.markdown('**Strong** and `[fg=red]literal[/]`\n\n[Web](https://example.com)\n\nRICH-END'))
 return true end},
 {name='richfail',permission='everyone',pattern='^richfail$',handler=function(ctx)
 mux.world.pemit(ctx.enactor,mux.text.markdown('LEAKED MARKDOWN'))
 mux.world.object(ctx.enactor):set_description('LEAKED STATE')
 error('rich callback failure') end}
}}
"#).unwrap();
    let mut w = persistence::load(&c.database()).await.unwrap();
    w.accounts.get_mut(&ObjectId(2)).unwrap().hash = Some(accounts::hash("secret", &c).unwrap());
    w.objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(stompymux_rs::flags::Flag::Ansi);
    w.objects.get_mut(&ObjectId(2)).unwrap().location = Some(ObjectId(c.start()));
    persistence::save(&c.database(), &w).await.unwrap();
    let running = Running::start(&c).await;
    let mut first = Client::connect(&running).await;
    first.login("#2").await;
    let mut second = Client::connect(&running).await;
    second.login("#2").await;
    first.send("col truecolor").await;
    first.until("Color mode set to truecolor.").await;
    second.send("color off").await;
    second.until("Color mode set to off.").await;
    first.socket.write_all(b"\xff\xfb\x27\xff\xfa\x27\x00\x03OSC_HYPERLINKS\x011\x03OSC_HYPERLINKS_SEND\x011\x03OSC_HYPERLINKS_PRESETS\x011\xff\xf0").await.unwrap();
    first.send("color").await;
    let prefs = first.until("Client capability: 16.").await;
    assert!(prefs.contains("truecolor (override)"));
    assert!(prefs.contains("preset:osc8-demo-button"));
    let db = std::fs::read(c.database()).unwrap();
    first.send("h @session").await;
    let help = first.until("Pending output").await;
    assert!(help.contains("@session"));
    first.send("color").await;
    first.until("Client capability: 16.").await;
    assert_eq!(db, std::fs::read(c.database()).unwrap());
    first.send("rich").await;
    let rich = first.until("RICH-END").await;
    let plain = second.until("RICH-END").await;
    assert!(rich.contains("\x1b[1m"));
    assert!(rich.contains("\x1b]8;;https://example.com"));
    assert!(rich.contains("[fg=red]literal[/]"));
    assert!(!rich.contains("preset:osc8-demo-button"));
    assert!(!plain.contains('\x1b'));
    assert!(plain.contains("Web (https://example.com)"));
    let description = persistence::load(&c.database()).await.unwrap().objects[&ObjectId(2)]
        .description
        .clone();
    first.send("richfail").await;
    let failed = first.until("rich callback failure").await;
    assert!(!failed.contains("LEAKED MARKDOWN"));
    assert_eq!(
        description,
        persistence::load(&c.database()).await.unwrap().objects[&ObjectId(2)].description
    );
    let mut ordinary = Client::connect(&running).await;
    ordinary.register("Reader").await;
    ordinary.send("help @session").await;
    ordinary.until("No help found").await;
    ordinary.send("color/bogus").await;
    ordinary.until("Unsupported command switch.").await;
    ordinary.send("help").await;
    let index = ordinary.until("All about this game").await;
    assert!(!index.contains("wizard_commands"));
    running.stop().await;
}

/// Help browsing/reload is private, read-only and complete across bounded/compressed output.
#[tokio::test(flavor = "current_thread")]
async fn tcp_help_reload_navigation_and_compressed_chunks() {
    let (d, _c) = populated().await;
    copy(Path::new("game/help"), &d.path().join("help"));
    let aliases = d.path().join("aliases.toml");
    std::fs::write(
        &aliases,
        std::fs::read_to_string(&aliases).unwrap().replace(
            "[aliases.commands]",
            "[aliases.commands]\nhr='@help/reload'\nhh='@help'",
        ),
    )
    .unwrap();
    let config_path = d.path().join("stompymux.toml");
    let config_text = std::fs::read_to_string(&config_path).unwrap();
    std::fs::write(
        &config_path,
        format!(
            "{config_text}\n[runtime]\noutput_message_limit=512\nsession_output_queue_capacity=16\n"
        ),
    )
    .unwrap();
    let c = Config::load(d.path()).unwrap();
    let help_path = d.path().join("help/long.md");
    let front = "+++\ntitle='Long'\ndescription='A long example'\nkeywords=['long']\narticle_tags=['show_in_index']\n+++\n";
    let body = format!("# Long\n\n{}\nHELP_END_ONE\n", "entry-word ".repeat(2500));
    std::fs::write(&help_path, format!("{front}{body}")).unwrap();
    let mut w = persistence::load(&c.database()).await.unwrap();
    w.accounts.get_mut(&ObjectId(2)).unwrap().hash = Some(accounts::hash("secret", &c).unwrap());
    w.objects.get_mut(&ObjectId(2)).unwrap().location = Some(ObjectId(c.start()));
    w.objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(stompymux_rs::flags::Flag::Ansi);
    persistence::save(&c.database(), &w).await.unwrap();
    let running = Running::start(&c).await;
    let mut wizard = Client::connect(&running).await;
    wizard.login("#2").await;
    let mut other = Client::connect(&running).await;
    other.login("#2").await;
    let mut reader = Client::connect(&running).await;
    reader.register("HelpReader").await;
    reader.send("@help/reload").await;
    reader.until("Permission denied.").await;
    wizard.send("color off").await;
    wizard.until("Color mode set to off.").await;
    let db = std::fs::read(c.database()).unwrap();
    wizard.send("hh").await;
    wizard.until("Rebuild the help index.").await;
    wizard.send("@help/bogus").await;
    wizard.until("Invalid @help switch combination.").await;
    wizard.send("@help/reload extra").await;
    wizard.until("Usage: @help or @help/reload").await;
    wizard.send("h long").await;
    let output = wizard.until("HELP_END_ONE").await;
    assert_eq!(output.matches("entry-word").count(), 2500);
    other.send("color").await;
    let private = other.until("Client capability: 16.").await;
    assert!(!private.contains("entry-word"));
    std::fs::write(
        &help_path,
        format!(
            "{}{}",
            front.replace("['long']", "['fresh']"),
            body.replace("HELP_END_ONE", "HELP_END_TWO")
        ),
    )
    .unwrap();
    wizard.send("help long").await;
    wizard.until("HELP_END_TWO").await;
    wizard.send("help fresh").await;
    wizard.until("No help found for 'fresh'.").await;
    wizard.send("hr").await;
    wizard.until("0 error(s), 0 warning(s).").await;
    wizard.send("help fresh").await;
    wizard.until("HELP_END_TWO").await;
    std::fs::rename(d.path().join("help"), d.path().join("help-away")).unwrap();
    wizard.send("hr").await;
    wizard.until("previous index retained").await;
    std::fs::rename(d.path().join("help-away"), d.path().join("help")).unwrap();
    wizard.send("help fresh").await;
    wizard.until("HELP_END_TWO").await;
    // Confirm negotiated action links and narrow NAWS layout on a real connection.
    wizard.socket.write_all(b"\xff\xfb\x1f\xff\xfa\x1f\x00\x18\x00\x18\xff\xf0\xff\xfb\x27\xff\xfa\x27\x00\x03OSC_HYPERLINKS_SEND\x011\xff\xf0").await.unwrap();
    wizard.send("help").await;
    let linked = wizard.until("help%20fresh").await;
    assert!(linked.contains("\x1b]8;;send:"));
    wizard.send("color").await;
    wizard.until("Client capability: 16.").await;
    // One zlib stream spans all chunks, including consecutive help requests.
    other.socket.write_all(&[255, 253, 86]).await.unwrap();
    let marker = telnet_until(&mut other.socket, &[255, 250, 86, 255, 240]).await;
    let boundary = marker
        .windows(5)
        .position(|v| v == [255, 250, 86, 255, 240])
        .unwrap()
        + 5;
    let mut compressed = marker[boundary..].to_vec();
    let mut inflater = flate2::Decompress::new(true);
    let mut offset = 0;
    for _ in 0..2 {
        other.send("help fresh").await;
        let mut plain = Vec::new();
        tokio::time::timeout(Duration::from_secs(10), async {
            while !plain.windows(12).any(|v| v == b"HELP_END_TWO") {
                let mut out = [0; 4096];
                let before = (inflater.total_in(), inflater.total_out());
                inflater
                    .decompress(
                        &compressed[offset..],
                        &mut out,
                        flate2::FlushDecompress::Sync,
                    )
                    .unwrap();
                let consumed = (inflater.total_in() - before.0) as usize;
                let written = (inflater.total_out() - before.1) as usize;
                offset += consumed;
                plain.extend_from_slice(&out[..written]);
                if consumed == 0 && written == 0 {
                    let mut input = [0; 4096];
                    let n = other.socket.read(&mut input).await.unwrap();
                    assert!(n > 0);
                    compressed.extend_from_slice(&input[..n]);
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(
            stompymux_rs::text::Document::Literal(String::from_utf8_lossy(&plain).into_owned())
                .spans(&Default::default(), &Default::default())
                .iter()
                .map(|s| s.text.as_str())
                .collect::<String>()
                .matches("entry-word")
                .count(),
            2500
        );
    }
    assert_eq!(db, std::fs::read(c.database()).unwrap());
    running.stop().await;
}

/// Channel and page delivery is player-scoped, transactional and durable across TCP reconnects.
#[tokio::test(flavor = "current_thread")]
async fn tcp_comsys_pages_sessions_and_write_rollback() {
    let (d, c) = populated().await;
    std::fs::write(d.path().join("lua/global_logic/failing_comsys.lua"), r#"return {commands={{name='failcom',permission='everyone',pattern='^failcom$',handler=function(ctx) mux.comsys.channel('Public'):emit('Lua must never arrive'); error('comsys failure') end}}}"#).unwrap();
    let mut w = persistence::load(&c.database()).await.unwrap();
    w.accounts.get_mut(&ObjectId(2)).unwrap().hash = Some(accounts::hash("secret", &c).unwrap());
    w.objects.get_mut(&ObjectId(2)).unwrap().location = Some(ObjectId(c.start()));
    persistence::save(&c.database(), &w).await.unwrap();
    let running = Running::start(&c).await;
    let mut wizard = Client::connect(&running).await;
    wizard.login("#2").await;
    wizard.send("@chan/flags Public=loud").await;
    wizard.until("Set.").await;
    let mut alice = Client::connect(&running).await;
    alice.register("ComAlice").await;
    let mut bob = Client::connect(&running).await;
    bob.register("ComBob").await;
    alice.send("pub hello channel").await;
    alice.until("ComAlice: hello channel").await;
    bob.until("ComAlice: hello channel").await;
    let mut second = Client::connect(&running).await;
    second.login("ComAlice").await;
    bob.send("pub both sessions").await;
    alice.until("ComBob: both sessions").await;
    second.until("ComBob: both sessions").await;
    bob.until("ComBob: both sessions").await;
    alice.send("failcom").await;
    alice.until("That command could not be completed.").await;
    bob.send("page ComAlice=private hello").await;
    alice.until("ComBob pages: private hello").await;
    second.until("ComBob pages: private hello").await;
    bob.until("You paged ComAlice").await;
    let before = persistence::load(&c.database()).await.unwrap();
    second.send("quit").await;
    second.until("Goodbye").await;
    alice.send("pub still here").await;
    alice.until("ComAlice: still here").await;
    bob.until("ComAlice: still here").await;
    let after = persistence::load(&c.database()).await.unwrap();
    assert_eq!(
        after.channels["Public"].messages,
        before.channels["Public"].messages + 1
    );
    let mut db = <sqlx::SqliteConnection as sqlx::Connection>::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(c.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TRIGGER reject_channel BEFORE UPDATE ON comsys_channels BEGIN SELECT RAISE(FAIL,'channel write blocked'); END").execute(&mut db).await.unwrap();
    alice.send("pub must never arrive").await;
    alice.until("Unable to save your changes").await;
    // Read-only channel inspection still works while durable channel writes are forbidden.
    alice.send("comlist").await;
    alice.until("-- End of comlist --").await;
    bob.send("pub last").await;
    let history = bob.until("ComAlice has connected.").await;
    assert!(!history.contains("must never arrive"));
    assert_eq!(
        persistence::load(&c.database()).await.unwrap().channels["Public"].messages,
        after.channels["Public"].messages
    );
    sqlx::raw_sql("DROP TRIGGER reject_channel")
        .execute(&mut db)
        .await
        .unwrap();
    drop(alice);
    bob.until("ComAlice has disconnected.").await;
    wizard.send("@shutdown").await;
    assert!(
        tokio::time::timeout(Duration::from_secs(10), async {
            running
                .child
                .wait_with_output()
                .await
                .unwrap()
                .status
                .success()
        })
        .await
        .unwrap()
    );
    let w = persistence::load(&c.database()).await.unwrap();
    let bob_id = w.find_player("ComBob").unwrap();
    assert_eq!(
        w.last_pages[&bob_id],
        vec![w.find_player("ComAlice").unwrap()]
    );
    sqlx::Connection::close(db).await.unwrap();
    let restarted = Running::start(&c).await;
    let mut bob = Client::connect(&restarted).await;
    bob.login("ComBob").await;
    bob.send("page").await;
    bob.until("You last paged ComAlice.").await;
    bob.send("pub last").await;
    bob.until("ComAlice: still here").await;
    restarted.stop().await;
}

/// State commands drive the copied exit policy end-to-end and survive relational restart.
#[tokio::test(flavor = "current_thread")]
async fn tcp_state_default_exit_policy_and_failed_write_rollback() {
    let (_d, c) = populated().await;
    let mut world = persistence::load(&c.database()).await.unwrap();
    world.accounts.get_mut(&ObjectId(1)).unwrap().hash =
        Some(accounts::hash("secret", &c).unwrap());
    persistence::save(&c.database(), &world).await.unwrap();
    let running = Running::start(&c).await;
    let mut admin = Client::connect(&running).await;
    admin.send("#1").await;
    admin.until("Password: ").await;
    admin.send("secret").await;
    admin.until("Staff Nexus").await;
    let mut traveler = Client::connect(&running).await;
    traveler.register("StateWalker").await;
    let mut witness = Client::connect(&running).await;
    witness.register("StateWitness").await;
    let id = persistence::load(&c.database())
        .await
        .unwrap()
        .find_player("StateWalker")
        .unwrap();
    traveler
        .send("@state/set #13/locks.traverse flag/WIZARD=false")
        .await;
    traveler.until("Permission denied.").await;
    admin
        .send("@state/set #13/locks.traverse message/enactor=Your pass is missing.")
        .await;
    admin.until("State value set.").await;
    admin
        .send("@state/set #13/locks.traverse message/others=is stopped at the gate.")
        .await;
    admin.until("State value set.").await;
    traveler.send("out").await;
    traveler.until("Your pass is missing.").await;
    witness.until("StateWalker is stopped at the gate.").await;
    admin
        .send("@state/set #13/locks.traverse flag/WIZARD=")
        .await;
    admin.until("State value cleared.").await;
    admin
        .send("@state/set #13/locks.traverse state/access/pass=\"\\x00\\xFF\"")
        .await;
    admin.until("State value set.").await;
    admin
        .send(&format!("@state/set #{}/access pass=\"\\x00\\xFF\"", id.0))
        .await;
    admin.until("State value set.").await;
    admin
        .send(&format!("@state/examine #{}/access", id.0))
        .await;
    admin.until("pass (string): \"\\x00\\xFF\"").await;
    let mut db = <sqlx::SqliteConnection as sqlx::Connection>::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(c.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TRIGGER fail_state BEFORE UPDATE ON object_state BEGIN SELECT RAISE(ABORT,'injected state failure'); END").execute(&mut db).await.unwrap();
    admin
        .send(&format!("@state/set #{}/access pass=wrong", id.0))
        .await;
    let failure = admin.until("Unable to save your changes").await;
    assert!(!failure.contains("State value set."));
    admin
        .send(&format!("@state/examine #{}/access", id.0))
        .await;
    admin.until("pass (string): \"\\x00\\xFF\"").await;
    sqlx::raw_sql("DROP TRIGGER fail_state")
        .execute(&mut db)
        .await
        .unwrap();
    sqlx::Connection::close(db).await.unwrap();
    traveler.send("out").await;
    traveler.until("Staff Nexus").await;
    assert_ne!(
        persistence::load(&c.database()).await.unwrap().objects[&id].location,
        Some(ObjectId(4))
    );
    running.stop().await;
    let restarted = Running::start(&c).await;
    let mut traveler = Client::connect(&restarted).await;
    traveler.send("StateWalker").await;
    traveler.until("Password: ").await;
    traveler.send("secret").await;
    traveler.until("Staff Nexus").await;
    let loaded = persistence::load(&c.database()).await.unwrap();
    assert_eq!(
        loaded.objects[&id].state["access"]["pass"],
        Scalar::String(vec![0, 255])
    );
    restarted.stop().await;
}

/// Macros expand once through native/Lua dispatch, commit atomically and survive reconnects.
#[tokio::test(flavor = "current_thread")]
async fn tcp_player_macros_shared_sessions_restart_and_write_failures() {
    let (d, c) = populated().await;
    std::fs::write(d.path().join("lua/global_logic/macro_failure.lua"), r#"return {commands={{name='macro-fail',permission='everyone',pattern='^macro%-fail$',handler=function(ctx) mux.world.object(ctx.enactor):state('macro_test'):set('failed',true); mux.world.pemit(ctx.enactor,'must-not-arrive'); error('macro callback failed') end}}}"#).unwrap();
    let running = Running::start(&c).await;
    let mut alice = Client::connect(&running).await;
    alice.register("MacroAlice").await;
    let mut second = Client::connect(&running).await;
    second.login("MacroAlice").await;
    let mut bob = Client::connect(&running).await;
    bob.register("MacroBob").await;
    alice.send(".create Personal").await;
    alice.until("created in slot 0.").await;
    alice.send(".def hi=say hello * %*").await;
    alice.until("defined.").await;
    second.send(".HI everyone").await;
    alice.until("hello everyone *").await;
    second.until("hello everyone *").await;
    let other = bob.until("hello everyone *").await;
    assert!(!other.contains("created in slot") && !other.contains("defined."));
    alice.send(".def lua=global-hello").await;
    alice.until("defined.").await;
    second.send(".lua").await;
    second.until("Hello, world").await;
    alice.send(".def adm=@shutdown").await;
    alice.until("defined.").await;
    alice.send(".adm").await;
    alice.until("Permission denied.").await;
    bob.send(".add 0").await;
    bob.until("Permission denied.").await;
    alice.send(".chmod R").await;
    alice.until("Current set modes: -R-.").await;
    bob.send(".add 0").await;
    bob.until("added in the 0 slot.").await;
    bob.send(".HI shared").await;
    bob.until("hello shared *").await;

    let mut db = <sqlx::SqliteConnection as sqlx::Connection>::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(c.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TRIGGER deny_macro BEFORE INSERT ON macro_entries BEGIN SELECT RAISE(FAIL,'macro blocked'); END").execute(&mut db).await.unwrap();
    alice.send(".def bad=say should-not-exist").await;
    alice.until("Unable to save your changes.").await;
    alice.send(".ex").await;
    let inspection = alice.until("global-hello").await;
    assert!(!inspection.contains("should-not-exist"));
    assert!(
        persistence::load(&c.database()).await.unwrap().macros.sets[0]
            .entries
            .iter()
            .all(|e| e.alias != "bad")
    );
    sqlx::query("DROP TRIGGER deny_macro")
        .execute(&mut db)
        .await
        .unwrap();
    alice.send(".def fail=macro-fail").await;
    alice.until("defined.").await;
    alice.send(".fail").await;
    alice.until("That command could not be completed.").await;
    assert!(
        persistence::load(&c.database())
            .await
            .unwrap()
            .objects
            .values()
            .all(|o| !o.state.contains_key("macro_test"))
    );
    alice.send(".list").await;
    alice.until("Current slot: 0").await;
    // A sentinel on the other session proves confirmations and inspection stayed private.
    second.send("look").await;
    let private = second.until("Starter Room").await;
    assert!(!private.contains("Current slot:") && !private.contains("defined."));
    sqlx::Connection::close(db).await.unwrap();
    running.stop().await;
    let running = Running::start(&c).await;
    let mut alice = Client::connect(&running).await;
    alice.login("MacroAlice").await;
    alice.send(".hI persisted").await;
    alice.until("hello persisted *").await;
    alice.send(".undef HI").await;
    alice.until("deleted from set.").await;
    alice.send("quit").await;
    alice.until("Goodbye").await;
    let loaded = persistence::load(&c.database()).await.unwrap();
    assert!(
        loaded.macros.sets[0]
            .entries
            .iter()
            .all(|e| !e.alias.eq_ignore_ascii_case("hi"))
    );
    running.stop().await;
}

/// Native lock consumers remain transactional when reached through TCP aliases and macros.
#[tokio::test(flavor = "current_thread")]
async fn tcp_object_locks_builders_transfers_and_restart() {
    use sqlx::Connection;
    use stompymux_rs::{flags::Flag, world::Kind};
    let (d, c) = populated().await;
    std::fs::write(d.path().join("lua/object_logic/tcp_policy.lua"),r#"return {
      locks={take=function(ctx) return {passes=true} end,use=function(ctx) return true end,receive=function(ctx) return true end},
      messages={use=function(ctx) return {enactor_message='TCP activated'} end},
      events={on_use=function(ctx) mux.world.object(ctx.object):state('usage'):set('used',true) end}
    }"#).unwrap();
    let mut w = persistence::load(&c.database()).await.unwrap();
    w.accounts.get_mut(&ObjectId(1)).unwrap().hash = Some(accounts::hash("secret", &c).unwrap());
    w.objects.get_mut(&ObjectId(1)).unwrap().location = Some(ObjectId(c.start()));
    let item = w.create(&c, "TcpWidget".into(), Kind::Thing);
    {
        let o = w.objects.get_mut(&item).unwrap();
        o.location = Some(ObjectId(c.start()));
        o.home = Some(ObjectId(c.home()));
        o.lua_parent = "tcp_policy.lua".into();
    }
    persistence::save(&c.database(), &w).await.unwrap();
    let running = Running::start(&c).await;
    let mut wizard = Client::connect(&running).await;
    wizard.login("#1").await;
    let mut alice = Client::connect(&running).await;
    alice.register("LockAlice").await;
    let mut second = Client::connect(&running).await;
    second.login("LockAlice").await;
    alice.send(".create actions").await;
    alice.until("created in slot").await;
    alice.send(".def tk=take *").await;
    alice.until("defined.").await;
    alice.send(".tk TcpWidget").await;
    alice.until("Taken.").await;
    second.send("inv").await;
    second.until("TcpWidget").await;
    alice.send("use TcpWidget").await;
    alice.until("TCP activated").await;
    second.until("TCP activated").await;
    alice.send("@open forbidden").await;
    alice.until("Permission denied.").await;
    let copied = persistence::load(&c.database()).await.unwrap().next_id;
    wizard.send(&format!("@cl #{}=TcpCopy", item.0)).await;
    wizard.until("cloned, new copy").await;
    wizard.send("enter LockAlice").await;
    wizard.until("LockAlice").await;
    wizard.send("leave").await;
    wizard.until("Starter Room").await;
    alice.send("give #1=TcpWidget").await;
    alice.until("Given.").await;
    wizard.send("drop TcpWidget").await;
    wizard.until("Dropped.").await;
    let mut db = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(c.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TRIGGER block_objects BEFORE UPDATE ON objects BEGIN SELECT RAISE(FAIL,'object write blocked'); END").execute(&mut db).await.unwrap();
    alice.send("take TcpWidget").await;
    alice.until("Unable to save your changes.").await;
    let durable = persistence::load(&c.database()).await.unwrap();
    assert_eq!(durable.objects[&item].location, Some(ObjectId(c.start())));
    assert!(durable.objects.values().any(|o| o.name == "LockAlice"));
    sqlx::query("DROP TRIGGER block_objects")
        .execute(&mut db)
        .await
        .unwrap();
    alice.send("take TcpWidget").await;
    alice.until("Taken.").await;
    sqlx::Connection::close(db).await.unwrap();
    running.stop().await;
    let loaded = persistence::load(&c.database()).await.unwrap();
    assert_eq!(loaded.objects[&ObjectId(copied)].name, "TcpCopy");
    assert!(loaded.objects[&item].state.contains_key("usage"));
    assert!(!loaded.objects[&item].flags.contains(Flag::Connected));
    let running = Running::start(&c).await;
    let mut alice = Client::connect(&running).await;
    alice.login("LockAlice").await;
    alice.send("inventory").await;
    alice.until("TcpWidget").await;
    alice.send("use TcpWidget").await;
    alice.until("TCP activated").await;
    running.stop().await;
}

/// Build and inspect through real sessions, including bounded reports and write failure isolation.
#[tokio::test(flavor = "current_thread")]
async fn tcp_basic_building_inspection_and_alias_restart() {
    use sqlx::Connection;
    let (d, _) = populated().await;
    let path = d.path().join("stompymux.toml");
    let source = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        format!(
            "{source}\n[runtime]\noutput_message_limit=512\nsession_output_queue_capacity=16\n"
        ),
    )
    .unwrap();
    let c = Config::load(d.path()).unwrap();
    let mut w = persistence::load(&c.database()).await.unwrap();
    w.accounts.get_mut(&ObjectId(1)).unwrap().hash = Some(accounts::hash("secret", &c).unwrap());
    w.objects.get_mut(&ObjectId(1)).unwrap().location = Some(ObjectId(c.start()));
    persistence::save(&c.database(), &w).await.unwrap();
    let server = Running::start(&c).await;
    let mut wizard = Client::connect(&server).await;
    wizard.login("#1").await;
    let mut second = Client::connect(&server).await;
    second.login("#1").await;
    wizard.send("@create TcpChest").await;
    wizard.until("created as object").await;
    wizard
        .send("@description TcpChest=[bold]A beautiful chest[/]")
        .await;
    wizard.until("Set.").await;
    wizard.send("look TcpChest").await;
    wizard.until("A beautiful chest").await;
    wizard.send("@dig TcpWorkshop=tcpdoor,return").await;
    wizard.until("Linked.").await;
    let loaded = persistence::load(&c.database()).await.unwrap();
    let room = loaded
        .objects
        .values()
        .find(|o| o.name == "TcpWorkshop")
        .unwrap()
        .id;
    let chest = loaded
        .objects
        .values()
        .find(|o| o.name == "TcpChest")
        .unwrap()
        .id;
    wizard.send(&format!("@link TcpChest=#{}", room.0)).await;
    wizard.until("Home set.").await;
    wizard.send(&format!("@chzone TcpChest=#{}", room.0)).await;
    wizard.until("Zone changed.").await;
    wizard.send(&format!("@entrances #{}", room.0)).await;
    wizard.until("2 entrances found.").await;
    wizard.send("@examine/debug TcpChest").await;
    wizard.until("Lua state entries:").await;
    wizard.send("@name me=MasterBuilder").await;
    wizard.until("Name set.").await;
    wizard.send("@alias me=BuilderLogin").await;
    wizard.until("Alias set.").await;
    let description = format!(
        "[bold]{}END_DESCRIPTION[/]",
        "日 e\u{301} text ".repeat(180)
    );
    wizard
        .send(&format!("@description TcpChest={description}"))
        .await;
    wizard.until("Set.").await;
    // Drain account-wide mutation confirmations before checking private report delivery.
    loop {
        let mut bytes = [0; 4096];
        if tokio::time::timeout(Duration::from_millis(50), second.socket.read(&mut bytes))
            .await
            .is_err()
        {
            break;
        }
    }
    let mut db = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(c.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TRIGGER building_block BEFORE UPDATE ON objects BEGIN SELECT RAISE(FAIL,'blocked write'); END;").execute(&mut db).await.unwrap();
    // Confirm NAWS before requesting a narrow, multi-chunk literal report.
    wizard
        .socket
        .write_all(&[255, 251, 31, 255, 250, 31, 0, 24, 0, 20, 255, 240])
        .await
        .unwrap();
    wizard.send("@examine TcpChest").await;
    let report = wizard.until("END_DESCRIPTION[/]").await;
    assert!(report.contains("[bold]"), "{report}");
    let mut bytes = [0; 4096];
    assert!(
        tokio::time::timeout(Duration::from_millis(100), second.socket.read(&mut bytes))
            .await
            .is_err()
    );
    wizard.send("@name TcpChest=Unsaved").await;
    wizard.until("Unable to save your changes").await;
    assert_eq!(
        persistence::load(&c.database()).await.unwrap().objects[&chest].name,
        "TcpChest"
    );
    wizard.send("@examine/brief TcpChest").await;
    wizard.until("END_DESCRIPTION[/]").await;
    sqlx::query("DROP TRIGGER building_block")
        .execute(&mut db)
        .await
        .unwrap();
    db.close().await.unwrap();
    server.stop().await;
    let loaded = persistence::load(&c.database()).await.unwrap();
    assert_eq!(loaded.objects[&chest].home, Some(room));
    assert_eq!(loaded.objects[&chest].zone, Some(room));
    assert_eq!(loaded.find_player("BuilderLogin"), Some(ObjectId(1)));
    let server = Running::start(&c).await;
    let mut by_alias = Client::connect(&server).await;
    by_alias.login("builderlogin").await;
    by_alias.send("look TcpChest").await;
    by_alias.until("日").await;
    let mut by_name = Client::connect(&server).await;
    by_name.login("MasterBuilder").await;
    server.stop().await;
}

/// New and changed code is published atomically while old sessions and persistent state survive.
#[tokio::test(flavor = "current_thread")]
async fn tcp_lua_parent_check_reload_and_default_exit() {
    use sqlx::Connection;
    let (d, c) = populated().await;
    let package = d.path().join("lua/packages/reload_value.lua");
    let module = d.path().join("lua/global_logic/reload_probe.lua");
    std::fs::write(&package, "return 'VERSION_ONE'").unwrap();
    std::fs::write(&module,r#"local value=require('reload_value');return {
      commands={{name='reload-probe',permission='everyone',pattern='^reload%-probe$',handler=function(ctx)
        assert(mux.world.object(ctx.enactor):flags():has(mux.world.flags.CONNECTED));mux.world.pemit(ctx.enactor,value);return true end}},
      events={on_server_startup=function() local s=mux.world.object(1):state('startup');s:set('count',s:get('count',0)+1) end}
    }"#).unwrap();
    let mut w = persistence::load(&c.database()).await.unwrap();
    w.accounts.get_mut(&ObjectId(1)).unwrap().hash = Some(accounts::hash("secret", &c).unwrap());
    w.objects.get_mut(&ObjectId(1)).unwrap().location = Some(ObjectId(c.start()));
    persistence::save(&c.database(), &w).await.unwrap();
    let server = Running::start(&c).await;
    let mut wizard = Client::connect(&server).await;
    wizard.login("#1").await;
    let mut second = Client::connect(&server).await;
    second.login("#1").await;
    let mut alice = Client::connect(&server).await;
    alice.register("LuaAlice").await;
    alice.send("@lua/reload").await;
    alice.until("Permission denied.").await;
    wizard.send("@dig LuaGarden").await;
    wizard.until("created with room number").await;
    let w = persistence::load(&c.database()).await.unwrap();
    let room = w
        .objects
        .values()
        .find(|o| o.name == "LuaGarden")
        .unwrap()
        .id;
    wizard.send(&format!("@open PolicyGate=#{}", room.0)).await;
    wizard.until("Linked.").await;
    let w = persistence::load(&c.database()).await.unwrap();
    let gate = w
        .objects
        .values()
        .find(|o| o.name == "PolicyGate")
        .unwrap()
        .id;
    wizard
        .send(&format!("@lua/parent #{}=default_exit.lua", gate.0))
        .await;
    wizard.until("Lua parent set.").await;
    wizard
        .send(&format!(
            "@state/set #{}/locks.traverse flag/WIZARD=true",
            gate.0
        ))
        .await;
    wizard.until("State value set.").await;
    alice.send("PolicyGate").await;
    alice.until("You cannot go that way.").await;
    wizard
        .send(&format!(
            "@state/set #{}/locks.traverse flag/WIZARD=false",
            gate.0
        ))
        .await;
    wizard.until("State value set.").await;
    alice.send("PolicyGate").await;
    alice.until("LuaGarden").await;
    // Teleport matching uses explicit dbrefs for remote targets.
    let alice_id = persistence::load(&c.database())
        .await
        .unwrap()
        .find_player("LuaAlice")
        .unwrap();
    wizard
        .send(&format!("@teleport #{}=#{}", alice_id.0, c.start()))
        .await;
    alice.until("Starter Room").await;
    wizard.send("@lua/check").await;
    wizard.until("All Lua module checks passed.").await;
    std::fs::write(&package, "return 'VERSION_TWO'").unwrap();
    alice.send("reload-probe").await;
    alice.until("VERSION_ONE").await;
    std::fs::write(
        d.path().join("lua/object_logic/new_parent.lua"),
        "-- [bold]literal[/]\nreturn {}\n",
    )
    .unwrap();
    wizard
        .send(&format!("@lua/parent #{}=new_parent.lua", gate.0))
        .await;
    wizard.until("not loaded").await;
    wizard.send("@lua/viewparent new_parent.lua").await;
    let viewed = wizard.until("-- End Lua parent --").await;
    assert!(viewed.contains("[bold]literal[/]"));
    wizard.send("@lua/reload").await;
    wizard.until("Lua reloaded.").await;
    alice.send("reload-probe").await;
    alice.until("VERSION_TWO").await;
    assert_eq!(
        persistence::load(&c.database()).await.unwrap().objects[&ObjectId(1)].state["startup"]["count"],
        stompymux_rs::world::Scalar::Integer(1)
    );
    wizard
        .send(&format!("@lua/parent #{}=new_parent.lua", gate.0))
        .await;
    wizard.until("Lua parent set.").await;
    std::fs::remove_file(d.path().join("lua/object_logic/new_parent.lua")).unwrap();
    wizard.send("@lua/reload").await;
    wizard.until("Lua reload failed:").await;
    alice.send("reload-probe").await;
    alice.until("VERSION_TWO").await;
    wizard
        .send(&format!("@lua/parent #{}=default_exit.lua", gate.0))
        .await;
    wizard.until("Lua parent set.").await;
    let init = d.path().join("lua/global_logic/reload_init.lua");
    std::fs::write(&init,"assert(#mux.session.connected_players()==3);mux.world.object(1):state('reload'):set('committed',true);mux.world.pemit(1,'CANDIDATE_SAVED');return {}").unwrap();
    let mut db = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(c.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TRIGGER fail_reload BEFORE INSERT ON object_state BEGIN SELECT RAISE(FAIL,'reload blocked'); END").execute(&mut db).await.unwrap();
    wizard.send("@lua/reload").await;
    let failed = wizard.until("Lua reload failed:").await;
    assert!(!failed.contains("CANDIDATE_SAVED"));
    assert!(
        !persistence::load(&c.database()).await.unwrap().objects[&ObjectId(1)]
            .state
            .contains_key("reload")
    );
    alice.send("reload-probe").await;
    alice.until("VERSION_TWO").await;
    sqlx::query("DROP TRIGGER fail_reload")
        .execute(&mut db)
        .await
        .unwrap();
    db.close().await.unwrap();
    wizard.send("@lua/reload").await;
    wizard.until("Lua reloaded.").await;
    assert!(
        persistence::load(&c.database()).await.unwrap().objects[&ObjectId(1)]
            .state
            .contains_key("reload")
    );
    std::fs::remove_file(init).unwrap();
    std::fs::remove_file(&module).unwrap();
    wizard.send("@lua/reload").await;
    wizard.until("Lua reloaded.").await;
    alice.send("reload-probe").await;
    alice.until("Huh?").await;
    wizard.send("@lua/reload").await;
    wizard.until("Lua reloaded.").await;
    server.stop().await;
    let saved = persistence::load(&c.database()).await.unwrap();
    assert_eq!(saved.objects[&gate].lua_parent, "default_exit.lua");
    let server = Running::start(&c).await;
    let mut alice = Client::connect(&server).await;
    alice.login("LuaAlice").await;
    alice.send("PolicyGate").await;
    alice.until("LuaGarden").await;
    server.stop().await;
}

/// Test commands use a separate VM while retaining live, durable world changes.
#[tokio::test(flavor = "current_thread")]
async fn tcp_lua_test_runner_live_mutations_and_reports() {
    let (d, _) = populated().await;
    let alias_path = d.path().join("aliases.toml");
    let aliases = std::fs::read_to_string(&alias_path).unwrap();
    std::fs::write(
        alias_path,
        aliases.replace(
            "[aliases.commands]",
            "[aliases.commands]\nlt='@lua/test/unit/verbose'\n",
        ),
    )
    .unwrap();
    let c = Config::load(d.path()).unwrap();
    std::fs::create_dir_all(d.path().join("lua/tests/unit")).unwrap();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("game/lua/packages/testing.lua"),
        d.path().join("lua/packages/testing.lua"),
    )
    .unwrap();
    std::fs::write(
        d.path().join("lua/tests/unit/live.lua"),
        r#"
local t=require('testing'); return t.suite('live',{
 after_all=function()mux.world.pemit(2,'RUNNER_TEARDOWN')end,
 tests={t.test('live failure',function(ctx,e)
 mux.world.object(2):state('runner'):set('durable',42)
 e.equal(1,2)
 end), t.test('passing',function()end)}})
"#,
    )
    .unwrap();
    std::fs::write(d.path().join("lua/global_logic/active_probe.lua"),r#"
local n=0;return {commands={{name='active-probe',permission='everyone',pattern='^active%-probe$',handler=function(ctx)n=n+1;mux.world.pemit(ctx.enactor,'ACTIVE_'..n);return true end}}}
"#).unwrap();
    let mut world = persistence::load(&c.database()).await.unwrap();
    world.accounts.get_mut(&ObjectId(2)).unwrap().hash =
        Some(accounts::hash("secret", &c).unwrap());
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(ObjectId(c.start()));
    persistence::save(&c.database(), &world).await.unwrap();
    let server = Running::start(&c).await;
    let mut wizard = Client::connect(&server).await;
    wizard.login("#2").await;
    let mut other = Client::connect(&server).await;
    other.login("#2").await;
    let mut ordinary = Client::connect(&server).await;
    ordinary.register("RunnerUser").await;
    ordinary.send("lt").await;
    ordinary.until("Permission denied.").await;
    wizard.send("active-probe").await;
    wizard.until("ACTIVE_1").await;
    wizard.send("@lua/check").await;
    wizard.until("All Lua module checks passed.").await;
    wizard.send("lt").await;
    let report = wizard.until("skipped in").await;
    assert!(
        report.contains("1 passed, 1 failed, 0 errored, 0 skipped in"),
        "{report}"
    );
    assert!(report.contains("unit/live.lua:passing"), "{report}");
    let peer = other.until("RUNNER_TEARDOWN").await;
    assert!(!peer.contains("passed, 1 failed"));
    let w = persistence::load(&c.database()).await.unwrap();
    let scripts =
        stompymux_rs::lua::Scripts::new(&c, std::rc::Rc::new(std::cell::RefCell::new(w))).unwrap();
    scripts
        .eval_callback::<()>("assert(mux.world.object(2):state('runner'):get('durable')==42)")
        .unwrap();
    wizard.send("active-probe").await;
    wizard.until("ACTIVE_2").await;
    wizard.send("@lua/test/reload").await;
    wizard.until("Invalid @lua switch combination.").await;
    std::fs::write(
        d.path().join("lua/tests/unit/invalid.lua"),
        "return {tests={false}} ",
    )
    .unwrap();
    wizard.send("@lua/check").await;
    wizard.until("checking tests/unit/invalid.lua").await;
    server.stop().await;
    let server = Running::start(&c).await;
    let mut wizard = Client::connect(&server).await;
    wizard.login("#2").await;
    wizard.send("@state/examine #2/runner").await;
    wizard.until("42").await;
    server.stop().await;
}

/// Account creation/reset/boot use the same durable world and session lifecycle as login.
#[tokio::test(flavor = "current_thread")]
async fn tcp_account_administration_and_restart() {
    let (d, _) = populated().await;
    let aliases = d.path().join("aliases.toml");
    std::fs::write(
        &aliases,
        std::fs::read_to_string(&aliases).unwrap().replace(
            "[aliases.commands]",
            "[aliases.commands]\npc='@pcreate'\nnp='@newpassword'\nbt='@boot'\nll='@last'\n",
        ),
    )
    .unwrap();
    let c = Config::load(d.path()).unwrap();
    let mut w = persistence::load(&c.database()).await.unwrap();
    w.accounts.get_mut(&ObjectId(1)).unwrap().hash = Some(accounts::hash("secret", &c).unwrap());
    w.objects.get_mut(&ObjectId(1)).unwrap().location = Some(ObjectId(c.start()));
    persistence::save(&c.database(), &w).await.unwrap();
    let server = Running::start(&c).await;
    let mut god = Client::connect(&server).await;
    god.login("#1").await;
    let mut second = Client::connect(&server).await;
    second.login("#1").await;
    let mut ordinary = Client::connect(&server).await;
    ordinary.register("AccountUser").await;
    for command in [
        "pc Denied=secret",
        "np #1=secret",
        "bt #1",
        "ll #1",
        "@boot/port 1",
    ] {
        ordinary.send(command).await;
        ordinary.until("Permission denied.").await;
    }
    god.send("pc Offline=secret").await;
    let reply = god.until("created.").await;
    assert!(reply.contains("New player 'Offline'"));
    assert!(!reply.contains("secret"));
    let w = persistence::load(&c.database()).await.unwrap();
    let p = w.find_player("Offline").unwrap();
    assert_eq!(w.objects[&p].location, Some(ObjectId(c.start())));
    assert_eq!(w.objects[&p].home, Some(ObjectId(c.home())));
    assert!(
        !w.objects[&p]
            .flags
            .contains(stompymux_rs::flags::Flag::Connected)
    );
    assert_eq!(w.accounts[&p].successes, 0);
    assert!(w.accounts[&p].history.is_empty());
    assert!(!w.objects[&p].lua_parent.is_empty());
    god.send("pc offline=secret").await;
    god.until("That name is not available.").await;
    god.send("np #1=changed").await;
    god.until("You cannot change that player's password.").await;
    god.send("np Offline=").await;
    god.until("Invalid password:").await;
    god.send("np Offline=changed").await;
    god.until("Password changed.").await;
    let mut offline = Client::connect(&server).await;
    offline.send("Offline").await;
    offline.until("Password: ").await;
    offline.send("secret").await;
    offline.until("different password.").await;
    offline.send(&format!("#{}", p.0)).await;
    offline.until("Password: ").await;
    offline.send("changed").await;
    offline.until("Starter Room").await;
    god.send("np Offline=secret").await;
    god.until("Password changed.").await;
    offline.until("Your password has been changed by").await;
    let mut another = Client::connect(&server).await;
    another.login("Offline").await;
    let before = std::fs::read(c.database()).unwrap();
    god.send("ll Offline").await;
    let history = god.until("Total failed connects: 1").await;
    assert!(history.contains("Total successful connects: 2"));
    assert!(history.contains("From: 127.0.0.1"));
    assert!(history.contains('Z'));
    assert_eq!(std::fs::read(c.database()).unwrap(), before);
    god.send("bt #1").await;
    god.until("You cannot boot that player!").await;
    god.send("bt/port/quiet 2").await;
    god.until("1 connection closed.").await;
    let mut closed = Vec::new();
    second.socket.read_to_end(&mut closed).await.unwrap();
    assert!(!String::from_utf8_lossy(&closed).contains("gently shows"));
    god.send("bt Offline").await;
    god.until("2 connections closed.").await;
    offline.until("gently shows you the door.").await;
    another.until("gently shows you the door.").await;
    let mut closed = Vec::new();
    offline.socket.read_to_end(&mut closed).await.unwrap();
    let w = persistence::load(&c.database()).await.unwrap();
    assert!(
        !w.objects[&p]
            .flags
            .contains(stompymux_rs::flags::Flag::Connected)
    );
    server.stop().await;
    let server = Running::start(&c).await;
    let mut player = Client::connect(&server).await;
    player.login("Offline").await;
    server.stop().await;
}

/// Native routed messages render per session, avoid writes, and roll back lock failures.
#[tokio::test(flavor = "current_thread")]
async fn tcp_speech_routing_styles_and_lock_persistence() {
    use sqlx::Connection;
    let (d, _) = populated().await;
    let aliases = d.path().join("aliases.toml");
    std::fs::write(
        &aliases,
        std::fs::read_to_string(&aliases).unwrap().replace(
            "[aliases.commands]",
            "[aliases.commands]\nspem='@pemit'\nspos='pose'\n",
        ),
    )
    .unwrap();
    let c = Config::load(d.path()).unwrap();
    std::fs::write(d.path().join("lua/object_logic/speech_room.lua"),"return {locks={speak=function(ctx)local s=mux.world.object(ctx.object):state('speech');s:set('count',s:get('count',0)+1);return true end}}").unwrap();
    let mut w = persistence::load(&c.database()).await.unwrap();
    w.accounts.get_mut(&ObjectId(2)).unwrap().hash = Some(accounts::hash("secret", &c).unwrap());
    w.objects.get_mut(&ObjectId(2)).unwrap().location = Some(ObjectId(c.start()));
    w.objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(stompymux_rs::flags::Flag::Ansi);
    w.objects.get_mut(&ObjectId(c.start())).unwrap().lua_parent = "speech_room.lua".into();
    persistence::save(&c.database(), &w).await.unwrap();
    let running = Running::start(&c).await;
    let mut wizard = Client::connect(&running).await;
    wizard.login("#2").await;
    let mut second = Client::connect(&running).await;
    second.login("#2").await;
    let mut alice = Client::connect(&running).await;
    alice.register("SpeechAlice").await;
    wizard.send("color truecolor").await;
    wizard.until("Color mode set to truecolor.").await;
    second.send("color off").await;
    second.until("Color mode set to off.").await;
    let before = std::fs::read(c.database()).unwrap();
    wizard
        .send("spem me=[fg=red]StyledMessage[/] END-STYLE")
        .await;
    let styled = wizard.until("END-STYLE").await;
    let plain = second.until("END-STYLE").await;
    assert!(styled.contains("\x1b["));
    assert!(!plain.contains("\x1b["));
    assert!(plain.contains("StyledMessage"));
    alice.send("spos waves").await;
    wizard.until("SpeechAlice waves").await;
    alice.send("\\LOCAL-EMIT").await;
    wizard.until("LOCAL-EMIT").await;
    alice.send("@emit DENIED-EMIT").await;
    alice.until("Permission denied.").await;
    wizard.send("@wall/wizard/emit WIZARD-ONLY").await;
    wizard.until("WIZARD-ONLY").await;
    second.until("WIZARD-ONLY").await;
    wizard.send("@wall/emit PUBLIC-END").await;
    let audience = alice.until("PUBLIC-END").await;
    assert!(!audience.contains("WIZARD-ONLY"));
    assert!(!audience.contains("DENIED-EMIT"));
    assert_eq!(std::fs::read(c.database()).unwrap(), before);
    wizard.send("@flag here=auditorium").await;
    wizard.until("set.").await;
    let mut db = sqlx::SqliteConnection::connect(c.database().to_str().unwrap())
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER reject_speech BEFORE INSERT ON object_state BEGIN SELECT RAISE(FAIL,'injected speech failure'); END").execute(&mut db).await.unwrap();
    db.close().await.unwrap();
    let before = std::fs::read(c.database()).unwrap();
    alice.send(":LEAKED-SPEECH").await;
    alice.until("Unable to save your changes.").await;
    wizard.send("@wall/emit AFTER-FAILURE").await;
    let observer = wizard.until("AFTER-FAILURE").await;
    assert!(!observer.contains("LEAKED-SPEECH"));
    assert_eq!(std::fs::read(c.database()).unwrap(), before);
    running.stop().await;
}

/// Real sockets exercise descriptor-free queues, aliases, cancellation, persistence and shutdown.
#[tokio::test(flavor = "current_thread")]
async fn tcp_command_queue_force_wait_halt_and_shutdown() {
    let (d, _) = populated().await;
    let aliases = d.path().join("aliases.toml");
    std::fs::write(
        &aliases,
        std::fs::read_to_string(&aliases).unwrap().replace(
            "[aliases.commands]",
            "[aliases.commands]\nqforce='@force'\nqwait='@wait'\nqhalt='@halt'\n",
        ),
    )
    .unwrap();
    std::fs::write(
        d.path().join("lua/global_logic/queued_tcp.lua"),
        r#"return {commands={
      {name='queued-probe',permission='everyone',pattern='^queued%-probe$',handler=function(ctx)
        assert(ctx.cause==2 and ctx.descriptor==nil)
        mux.world.object(ctx.enactor):state('queue'):set('probe',true)
        mux.world.pemit(2,'QUEUED-PROBE-DONE')
        return true
      end}
    }}"#,
    )
    .unwrap();
    let c = Config::load(d.path()).unwrap();
    let mut w = persistence::load(&c.database()).await.unwrap();
    w.accounts.get_mut(&ObjectId(2)).unwrap().hash = Some(accounts::hash("secret", &c).unwrap());
    w.objects.get_mut(&ObjectId(2)).unwrap().location = Some(ObjectId(c.start()));
    let thing = w.create(&c, "QueueRobot".into(), stompymux_rs::world::Kind::Thing);
    w.objects.get_mut(&thing).unwrap().location = Some(ObjectId(c.start()));
    persistence::save(&c.database(), &w).await.unwrap();
    let mut running = Running::start(&c).await;
    let mut wizard = Client::connect(&running).await;
    wizard.login("#2").await;
    let mut other = Client::connect(&running).await;
    other.login("#2").await;
    let mut alice = Client::connect(&running).await;
    alice.register("QueueAlice").await;
    alice.send("@wait 0=say DENIED").await;
    alice.until("Permission denied.").await;
    let before = std::fs::read(c.database()).unwrap();
    wizard
        .send(&format!("qforce #{}=say ROBOT-SPEAKS", thing.0))
        .await;
    wizard.until("QueueRobot says \"ROBOT-SPEAKS\"").await;
    assert_eq!(before, std::fs::read(c.database()).unwrap());
    wizard.send("qforce me=quit;say STILL-CONNECTED").await;
    wizard.until("STILL-CONNECTED").await;
    let rejection = other.until("STILL-CONNECTED").await;
    assert!(rejection.contains("requires an interactive session"));
    wizard
        .send(&format!("qforce #{}=queued-probe", thing.0))
        .await;
    wizard.until("QUEUED-PROBE-DONE").await;
    assert_eq!(
        persistence::load(&c.database()).await.unwrap().objects[&thing].state["queue"]["probe"],
        Scalar::Boolean(true)
    );
    // The target's current permissions apply, even with a Wizard cause.
    wizard
        .send("qforce QueueAlice=@wait 0=say PRIVILEGE-LEAK")
        .await;
    alice.until("Permission denied.").await;
    wizard.send("qwait 1=say DELAYED-DONE").await;
    wizard.until("DELAYED-DONE").await;
    wizard.send("qwait 60=say CANCELLED\r\nqhalt/all").await;
    wizard.until("1 queue entries removed.").await;
    wizard.send("qwait 0=qwait 0=say NESTED-DONE").await;
    wizard.until("NESTED-DONE").await;
    // Admission remains valid after its executing player disconnects.
    wizard
        .send("qforce QueueAlice=queued-probe\r\n@boot QueueAlice")
        .await;
    wizard.until("QUEUED-PROBE-DONE").await;
    let world = persistence::load(&c.database()).await.unwrap();
    let alice_id = world.find_player("QueueAlice").unwrap();
    assert_eq!(
        world.objects[&alice_id].state["queue"]["probe"],
        Scalar::Boolean(true)
    );
    // Shutdown consumes the triggering command but never the remaining command-list tail.
    wizard
        .send("qwait 0=@shutdown;@description me=SHUTDOWN-LEAK")
        .await;
    wizard.until("Game: Shutdown by Wizard").await;
    assert!(
        tokio::time::timeout(Duration::from_secs(10), running.child.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
    let world = persistence::load(&c.database()).await.unwrap();
    assert_ne!(
        world.objects[&ObjectId(2)].description.as_deref(),
        Some("SHUTDOWN-LEAK")
    );
    assert!(
        !world.objects[&ObjectId(2)]
            .flags
            .contains(stompymux_rs::flags::Flag::Connected)
    );
    let running = Running::start(&c).await;
    let mut wizard = Client::connect(&running).await;
    wizard.login("#2").await;
    wizard.send("qhalt/all").await;
    wizard.until("0 queue entries removed.").await;
    running.stop().await;
}

/// Zero processing chunks stop background work while configured admission limits still apply.
#[tokio::test(flavor = "current_thread")]
async fn tcp_queue_nondefault_limits_and_zero_chunks() {
    let (d, _) = populated().await;
    let path = d.path().join("stompymux.toml");
    std::fs::write(
        &path,
        std::fs::read_to_string(&path)
            .unwrap()
            .replace(
                "command_queue_active_chunk = 100",
                "command_queue_active_chunk = 0",
            )
            .replace(
                "command_queue_idle_chunk = 200",
                "command_queue_idle_chunk = 0",
            )
            .replace("[mux]", "[mux]\ncommand_queue_limit = 1"),
    )
    .unwrap();
    let c = Config::load(d.path()).unwrap();
    let mut w = persistence::load(&c.database()).await.unwrap();
    w.accounts.get_mut(&ObjectId(2)).unwrap().hash = Some(accounts::hash("secret", &c).unwrap());
    w.objects.get_mut(&ObjectId(2)).unwrap().location = Some(ObjectId(c.start()));
    persistence::save(&c.database(), &w).await.unwrap();
    let running = Running::start(&c).await;
    let mut wizard = Client::connect(&running).await;
    wizard.login("#2").await;
    wizard
        .send("@wait 0=say NEVER-RUN\r\n@wait 0=say OVERFLOW")
        .await;
    let output = wizard.until("Halted.").await;
    assert!(!output.contains("NEVER-RUN"));
    assert!(
        persistence::load(&c.database()).await.unwrap().objects[&ObjectId(2)]
            .flags
            .contains(stompymux_rs::flags::Flag::Halted)
    );
    wizard.send("@flag me=!halted").await;
    wizard.until("cleared.").await;
    wizard.send("@wait 0=say NEVER-RUN\r\n@halt/all").await;
    let output = wizard.until("1 queue entries removed.").await;
    assert!(!output.contains("NEVER-RUN"));
    running.stop().await;
}

/// Copied flows consume raw lines privately, including pipelined and empty input.
#[tokio::test(flavor = "current_thread")]
async fn tcp_interactive_flow_examples_and_independent_sessions() {
    let (_d, c) = populated().await;
    let running = Running::start(&c).await;
    let mut a = Client::connect(&running).await;
    a.register("FlowPlayer").await;
    let mut b = Client::connect(&running).await;
    b.login("FlowPlayer").await;
    let before = std::fs::read(c.database()).unwrap();
    a.send("flow-demo confirm").await;
    a.until("Really do the thing? (y/n) ").await;
    a.send("quit").await;
    a.until("Please answer y or n: ").await;
    b.send("global-hello").await;
    let other = b.until("Hello, world").await;
    assert!(!other.contains("Really do") && !other.contains("Please answer"));
    a.send("").await;
    a.until("Please answer y or n: ").await;
    a.send("y").await;
    a.until("Done.").await;
    a.send("flow-demo menu").await;
    a.until("Choice: ").await;
    a.send("2").await;
    a.until("Farewell!").await;
    a.socket
        .write_all(b"flow-demo signup\r\n  Ada  \r\n1\r\ny\r\n")
        .await
        .unwrap();
    a.until("Recorded   Ada   (Inner Sphere).").await;
    assert_eq!(
        before,
        std::fs::read(c.database()).unwrap(),
        "flow-only input must not write SQLite"
    );
    a.send("flow-demo confirm").await;
    a.until("Really do the thing? (y/n) ").await;
    drop(a);
    b.send("flow-demo menu").await;
    b.until("Choice: ").await;
    b.send("3").await;
    b.until("Nevermind, then.").await;
    running.stop().await;
}

/// Failed durable mutations keep the committed prompt/scratch; script failures cancel.
#[tokio::test(flavor = "current_thread")]
async fn tcp_flow_persistence_retry_reload_and_failure_cancellation() {
    let (d, c) = populated().await;
    let mut world = persistence::load(&c.database()).await.unwrap();
    world.accounts.get_mut(&ObjectId(1)).unwrap().hash =
        Some(accounts::hash("secret", &c).unwrap());
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(ObjectId(c.start()));
    persistence::save(&c.database(), &world).await.unwrap();
    let path = d.path().join("lua/global_logic/flow_transaction.lua");
    let source = r#"return {
      commands={{name='flow-test',permission='everyone',pattern='^flow%-test$',handler=function(ctx)
        mux.session.flow_start(ctx.descriptor,'flow_transaction.lua','step'); return true
      end}},
      flows={step=function(ctx)
        if ctx.input==nil then ctx.flow.n=10;return {prompt='Retry prompt: '} end
        assert(ctx.flow.n=='10')
        mux.world.object(ctx.enactor):state('flow_test'):set('answer',ctx.input)
        if ctx.input=='error' then error('injected flow error') end
        ctx.flow.n=99
        return {action='done',message='Committed '..ctx.input}
      end}}
    "#;
    std::fs::write(&path, source).unwrap();
    let running = Running::start(&c).await;
    let mut wizard = Client::connect(&running).await;
    wizard.login("#1").await;
    let mut player = Client::connect(&running).await;
    player.register("FlowRetry").await;
    player.send("flow-test").await;
    player.until("Retry prompt: ").await;
    let mut db = <sqlx::SqliteConnection as sqlx::Connection>::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(c.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TRIGGER reject_flow BEFORE UPDATE ON snapshot BEGIN SELECT RAISE(ABORT,'flow persistence failure'); END;").execute(&mut db).await.unwrap();
    player.send("first").await;
    let response = player.until("Please try again.").await;
    assert!(!response.contains("Committed"));
    sqlx::raw_sql("DROP TRIGGER reject_flow")
        .execute(&mut db)
        .await
        .unwrap();
    player.send("second").await;
    player.until("Committed second").await;
    let saved = persistence::load(&c.database()).await.unwrap();
    let id = saved.find_player("FlowRetry").unwrap();
    assert_eq!(
        saved.objects[&id].state["flow_test"]["answer"],
        Scalar::String("second".into())
    );
    player.send("flow-test").await;
    player.until("Retry prompt: ").await;
    player.send("error").await;
    player
        .until("Interactive flow failed and was cancelled.")
        .await;
    player.send("global-hello").await;
    player.until("Hello, world").await;
    player.send("flow-test").await;
    player.until("Retry prompt: ").await;
    std::fs::write(&path, "return broken Lua").unwrap();
    wizard.send("@lua/reload").await;
    wizard.until("Lua reload failed").await;
    std::fs::write(&path, source.replace("Committed ", "Reloaded ")).unwrap();
    wizard.send("@lua/reload").await;
    wizard.until("Lua reloaded.").await;
    player.send("third").await;
    player.until("Reloaded third").await;
    player.send("flow-test").await;
    player.until("Retry prompt: ").await;
    std::fs::remove_file(&path).unwrap();
    wizard.send("@lua/reload").await;
    wizard.until("Lua reloaded.").await;
    player.send("fourth").await;
    player
        .until("Interactive flow failed and was cancelled.")
        .await;
    sqlx::Connection::close(db).await.unwrap();
    running.stop().await;
}

/// Live test callbacks and queued commands may explicitly target a real authenticated session.
#[tokio::test(flavor = "current_thread")]
async fn tcp_flow_from_hosted_test_and_background_command() {
    let (d, c) = populated().await;
    let mut world = persistence::load(&c.database()).await.unwrap();
    world.accounts.get_mut(&ObjectId(1)).unwrap().hash =
        Some(accounts::hash("secret", &c).unwrap());
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(ObjectId(c.start()));
    world.objects.get_mut(&ObjectId(13)).unwrap().lua_parent = "hosted_flow.lua".into();
    persistence::save(&c.database(), &world).await.unwrap();
    std::fs::write(
        d.path().join("lua/object_logic/hosted_flow.lua"),
        r#"return {
      locks={use=function(ctx) mux.session.flow_start(2,'hosted_flow.lua','step');return true end},
      flows={step=function(ctx) if ctx.input==nil then return {prompt='Hosted TCP: '} end
          return {action='done',message='Active VM '..ctx.input} end}}
    "#,
    )
    .unwrap();
    std::fs::write(d.path().join("lua/global_logic/remote_flow.lua"),r#"return {
      commands={{name='flow-remote',permission='wizard',pattern='^flow%-remote$',handler=function(ctx)
        assert(ctx.descriptor==nil); mux.session.flow_start(2,'remote_flow.lua','step'); return true end}},
      flows={step=function(ctx) if ctx.input==nil then return {prompt='Background TCP: '} end
          return {action='done',message='Background done'} end}}
    "#).unwrap();
    let tests = d.path().join("lua/tests/integration");
    std::fs::create_dir_all(&tests).unwrap();
    std::fs::write(
        tests.join("hosted_flow.lua"),
        r#"return {expect={},tests={{name='start',run=function()
      assert(mux.world.lock_passes({object=13,enactor=1,lock=mux.world.locks.USE}))
    end}}}"#,
    )
    .unwrap();
    let running = Running::start(&c).await;
    let mut wizard = Client::connect(&running).await;
    wizard.login("#1").await;
    let mut player = Client::connect(&running).await;
    player.register("ExplicitTarget").await;
    wizard.send("@lua/test/integration hosted_flow").await;
    wizard.until("1 passed, 0 failed, 0 errored").await;
    player.until("Hosted TCP: ").await;
    player.send("resume").await;
    player.until("Active VM resume").await;
    wizard.send("@wait 0=flow-remote").await;
    player.until("Background TCP: ").await;
    player.send("done").await;
    player.until("Background done").await;
    running.stop().await;
}

/// Database reports retain complete Unicode rows across compressed transport chunks.
#[tokio::test(flavor = "current_thread")]
async fn tcp_compressed_database_reports() {
    let (d, _) = populated().await;
    let path = d.path().join("stompymux.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        path,
        format!("{text}\n[runtime]\noutput_message_limit=256\n"),
    )
    .unwrap();
    let c = Config::load(d.path()).unwrap();
    let mut world = persistence::load(&c.database()).await.unwrap();
    world.accounts.get_mut(&ObjectId(2)).unwrap().hash =
        Some(accounts::hash("secret", &c).unwrap());
    for i in 0..80 {
        world.create(
            &c,
            format!("Unicode{i:03} é👩‍🚀"),
            stompymux_rs::world::Kind::Room,
        );
    }
    persistence::save(&c.database(), &world).await.unwrap();
    let running = Running::start(&c).await;
    let mut client = Client::connect(&running).await;
    client.send("#2").await;
    client.until("Password: ").await;
    client.send("secret").await;
    client.until("Staff Nexus").await;
    let before = std::fs::read(c.database()).unwrap();
    let mut socket = client.socket;
    socket.write_all(&[255, 253, 86]).await.unwrap();
    let marker = telnet_until(&mut socket, &[255, 250, 86, 255, 240]).await;
    let boundary = marker
        .windows(5)
        .position(|v| v == [255, 250, 86, 255, 240])
        .unwrap()
        + 5;
    let mut bytes = marker[boundary..].to_vec();
    socket
        .write_all(b"@search rooms=Unicode\r\n@find Unicode\r\n@list switches\r\n@stats\r\n")
        .await
        .unwrap();
    let mut inflater = flate2::Decompress::new(true);
    let mut decoded = Vec::new();
    let mut offset = 0;
    let mut drain = false;
    tokio::time::timeout(Duration::from_secs(10), async {
        while !String::from_utf8_lossy(&decoded).contains("garbage)") {
            if offset == bytes.len() && !drain {
                let mut b = [0; 4096];
                let n = socket.read(&mut b).await.unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&b[..n]);
            }
            let mut output = [0; 4096];
            let prior = (inflater.total_in(), inflater.total_out());
            inflater
                .decompress(&bytes[offset..], &mut output, flate2::FlushDecompress::Sync)
                .unwrap();
            offset += (inflater.total_in() - prior.0) as usize;
            let produced = (inflater.total_out() - prior.1) as usize;
            drain = produced == output.len();
            decoded.extend_from_slice(&output[..produced]);
        }
    })
    .await
    .unwrap_or_else(|error| panic!("{error}: {}", String::from_utf8_lossy(&decoded)));
    let text = String::from_utf8(decoded).unwrap();
    assert_eq!(text.matches("Unicode079 é👩‍🚀").count(), 2);
    assert!(
        text.contains("Rooms...80")
            && text.contains("***End of List***")
            && text.contains("@clone: /inventory")
    );
    assert!(!text.contains("truncated"));
    assert_eq!(before, std::fs::read(c.database()).unwrap());
    running.stop().await;
}

/// Carried commands and false-returning callbacks remain transactional before native reports.
#[tokio::test(flavor = "current_thread")]
async fn tcp_portable_commands_inventory_and_report_rollback() {
    use stompymux_rs::world::Kind;
    let (d, c) = populated().await;
    let mut world = persistence::load(&c.database()).await.unwrap();
    world.accounts.get_mut(&ObjectId(1)).unwrap().hash =
        Some(accounts::hash("secret", &c).unwrap());
    let item = world.create(&c, "PortableWidget".into(), Kind::Thing);
    let o = world.objects.get_mut(&item).unwrap();
    o.location = Some(ObjectId(1));
    o.lua_parent = "portable_tcp.lua".into();
    o.flags.remove(stompymux_rs::flags::Flag::NoCommand);
    persistence::save(&c.database(), &world).await.unwrap();
    std::fs::write(d.path().join("lua/object_logic/portable_tcp.lua"), r#"return {commands={
      {name='portable',permission='everyone',pattern='^portable$',handler=function(ctx) mux.world.pemit(ctx.enactor,'Portable active');return true end},
      {name='inventory',permission='everyone',pattern='^inventory$',handler=function(ctx)
        local s=mux.world.object(ctx.object):state('portable');s:set('calls',s:get('calls',0)+1)
        mux.world.pemit(ctx.enactor,'Callback committed');return false end}
    }}"#).unwrap();
    let running = Running::start(&c).await;
    let mut wizard = Client::connect(&running).await;
    wizard.send("#1").await;
    wizard.until("Password: ").await;
    wizard.send("secret").await;
    wizard.until("Staff Nexus").await;
    wizard.send("portable").await;
    wizard.until("Portable active").await;
    wizard.send("inventory").await;
    wizard.until("PortableWidget").await;
    let saved = persistence::load(&c.database()).await.unwrap();
    assert_eq!(
        saved.objects[&item].state["portable"]["calls"],
        Scalar::Integer(1)
    );
    let mut db = <sqlx::SqliteConnection as sqlx::Connection>::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(c.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TRIGGER reject_portable BEFORE UPDATE ON object_state BEGIN SELECT RAISE(ABORT,'portable failure'); END;").execute(&mut db).await.unwrap();
    wizard.send("inventory").await;
    let failed = wizard.until("Please try again.").await;
    assert!(!failed.contains("Callback committed") && !failed.contains("You are carrying:"));
    sqlx::raw_sql("DROP TRIGGER reject_portable")
        .execute(&mut db)
        .await
        .unwrap();
    sqlx::Connection::close(db).await.unwrap();
    wizard.send("drop PortableWidget").await;
    wizard.until("Dropped.").await;
    wizard.send("@teleport #4").await;
    wizard.until("Starter Room").await;
    wizard.send("portable").await;
    wizard.until("Huh?").await;
    running.stop().await;
    let saved = persistence::load(&c.database()).await.unwrap();
    assert_eq!(
        saved.objects[&item].state["portable"]["calls"],
        Scalar::Integer(1)
    );
}
