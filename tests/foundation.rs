use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};
use stompymux_rs::{
    accounts,
    config::Config,
    persistence,
    scripting::Scripts,
    server,
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
fn imported() -> (tempfile::TempDir, Config) {
    let d = fixture();
    let c = Config::load(d.path()).unwrap();
    persistence::import(&d.path().join("data/stompymux.db"), &c).unwrap();
    (d, c)
}
#[test]
fn import_is_explicit_lossless_for_supported_data_and_read_only() {
    let d = fixture();
    let c = Config::load(d.path()).unwrap();
    let source = d.path().join("data/stompymux.db");
    let before = std::fs::read(&source).unwrap();
    let w = persistence::read_legacy(&source, &c).unwrap();
    assert_eq!(w.objects.len(), 16);
    assert_eq!(w.accounts.len(), 2);
    assert_eq!(w.channels.len(), 2);
    assert_eq!(
        w.objects[&ObjectId(13)].state["locks.traverse"]["flag/WIZARD"],
        Scalar::Boolean(true)
    );
    persistence::import(&source, &c).unwrap();
    let loaded = persistence::load(&c.database()).unwrap();
    assert_eq!(
        serde_json::to_value(w).unwrap(),
        serde_json::to_value(loaded).unwrap()
    );
    assert!(persistence::import(&source, &c).is_err());
    assert_eq!(before, std::fs::read(&source).unwrap());
}
#[test]
fn rejects_unsupported_schema_and_bad_references() {
    let d = fixture();
    let source = d.path().join("data/stompymux.db");
    let c = Config::load(d.path()).unwrap();
    let sql = rusqlite::Connection::open(&source).unwrap();
    sql.execute("UPDATE snapshot SET schema_version=29", [])
        .unwrap();
    assert!(persistence::import(&source, &c).is_err());
    assert!(!c.database().exists());
    sql.execute("UPDATE snapshot SET schema_version=32", [])
        .unwrap();
    sql.execute("UPDATE objects SET location=99999 WHERE dbref=1", [])
        .unwrap();
    assert!(persistence::import(&source, &c).is_err());
}
#[test]
fn import_all_scalar_types_and_argon_hash() {
    let d = fixture();
    let c = Config::load(d.path()).unwrap();
    let source = d.path().join("data/stompymux.db");
    let sql = rusqlite::Connection::open(&source).unwrap();
    let hash = accounts::hash("known-secret", &c).unwrap();
    sql.execute(
        "UPDATE player_state SET password_hash=?1 WHERE object_dbref=1",
        [&hash],
    )
    .unwrap();
    for (key, kind, value) in [
        ("string", 1, rusqlite::types::Value::Text("hi".into())),
        ("integer", 3, rusqlite::types::Value::Integer(42)),
        ("number", 4, rusqlite::types::Value::Real(1.5)),
    ] {
        sql.execute(
            "INSERT INTO object_state VALUES(1,'test',?1,?2,?3)",
            rusqlite::params![key, kind, value],
        )
        .unwrap();
    }
    persistence::import(&source, &c).unwrap();
    let w = persistence::load(&c.database()).unwrap();
    assert!(accounts::verify(
        "known-secret",
        w.accounts[&ObjectId(1)].hash.as_ref().unwrap()
    ));
    assert!(!accounts::verify("wrong", &hash));
    assert_eq!(
        w.objects[&ObjectId(1)].state["test"]["integer"],
        Scalar::Integer(42)
    );
}
#[test]
fn config_includes_override_and_detect_cycles() {
    let d = fixture();
    let path = d.path().join("stompymux.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("{text}\n[aliases.commands]\nl = 'look'\n")).unwrap();
    let c = Config::load(d.path()).unwrap();
    assert_eq!(c.string("aliases.commands.l", ""), "look");
    std::fs::write(d.path().join("aliases.toml"), "include=['stompymux.toml']").unwrap();
    assert!(Config::load(d.path()).is_err());
}
#[test]
fn telnet_fragmentation_echo_and_utf8() {
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
    assert_eq!(Decoder::echo(true), [255, 251, 1]);
    assert_eq!(Decoder::echo(false), [255, 252, 1]);
}
#[test]
fn copied_lua_renders_rooms_locks_and_commands() {
    let (_d, c) = imported();
    let w = Rc::new(RefCell::new(persistence::load(&c.database()).unwrap()));
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
        .remove("WIZARD");
    assert!(!s.lock(ObjectId(2), ObjectId(13)).unwrap());
    assert!(s.dispatch(ObjectId(1), 1, "global-hello").unwrap());
    assert!(s.outbox.borrow()[0].1.contains("Hello, world"));
    assert!(s.dispatch(ObjectId(1), 1, "flow-demo confirm").is_err());
}
#[test]
fn lua_resource_limits_and_fail_closed() {
    let (_d, c) = imported();
    let w = Rc::new(RefCell::new(persistence::load(&c.database()).unwrap()));
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
async fn bootstrap_once_and_import_does_not_bootstrap() {
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
    let (_d, c) = imported();
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
    let (_d, c) = imported();
    let running = Running::start(&c).await;
    let mut alice = Client::connect(&running).await;
    alice.register("Alice").await;
    let mut bob = Client::connect(&running).await;
    bob.register("Bob").await;
    alice.send("WHO").await;
    let who = alice.until("maximum.").await;
    assert!(who.contains("Alice") && who.contains("Bob"));
    alice.send("say Hello Bob").await;
    bob.until("Alice says, \"Hello Bob\"").await;
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
    let w = persistence::load(&c.database()).unwrap();
    assert_eq!(w.objects.len(), 18);
    assert_eq!(
        w.objects[&w.find_player("alice").unwrap()].location,
        Some(ObjectId(4))
    );
    running.stop().await;
}
#[tokio::test(flavor = "current_thread")]
async fn tcp_mismatch_bad_password_and_duplicate_registration() {
    let (_d, c) = imported();
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
    let w = persistence::load(&c.database()).unwrap();
    assert_eq!(w.accounts.len(), 3);
    running.stop().await;
}

#[tokio::test(flavor = "current_thread")]
async fn movement_persists_and_failed_registration_rolls_back() {
    let (_d, c) = imported();
    let mut w = persistence::load(&c.database()).unwrap();
    w.accounts.get_mut(&ObjectId(2)).unwrap().hash = Some(accounts::hash("secret", &c).unwrap());
    persistence::save(&c.database(), &w).unwrap();
    let running = Running::start(&c).await;
    let mut wizard = Client::connect(&running).await;
    wizard.send("Wizard").await;
    wizard.until("Password: ").await;
    wizard.send("secret").await;
    wizard.until("Staff Nexus").await;
    wizard.send("np").await;
    wizard.until("Starter Room").await;
    assert_eq!(
        persistence::load(&c.database()).unwrap().objects[&ObjectId(2)].location,
        Some(ObjectId(4))
    );
    let db = rusqlite::Connection::open(c.database()).unwrap();
    db.execute_batch("CREATE TRIGGER reject_write BEFORE UPDATE ON world BEGIN SELECT RAISE(ABORT,'injected persistence failure'); END;").unwrap();
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
            .unwrap()
            .find_player("Unsaved")
            .is_none()
    );
    wizard.send("out").await;
    wizard.until("Unable to save your changes.").await;
    assert_eq!(
        persistence::load(&c.database()).unwrap().objects[&ObjectId(2)].location,
        Some(ObjectId(4))
    );
    db.execute_batch("DROP TRIGGER reject_write").unwrap();
    newcomer.register("Unsaved").await;
    running.stop().await;
}
#[tokio::test(flavor = "current_thread")]
async fn login_throttle_utf8_and_echo_over_tcp() {
    let (d, _) = imported();
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
#[test]
fn bounded_output_marks_slow_clients_for_disconnect() {
    use std::time::Instant;
    use stompymux_rs::sessions::{LoginFlow, Session};
    let (output, _receiver) = tokio::sync::mpsc::channel(1);
    let now = Instant::now();
    let session = Session {
        output,
        peer: "127.0.0.1".parse().unwrap(),
        player: None,
        flow: LoginFlow::Name,
        connected: now,
        active: now,
        decoder: Decoder::default(),
        quota: 1,
        quota_at: now,
        failed: Default::default(),
    };
    assert!(session.raw(vec![1]));
    assert!(!session.raw(vec![2]));
    assert!(session.failed.get());
}
#[test]
fn missing_required_parent_prevents_startup() {
    let (d, c) = imported();
    std::fs::remove_file(d.path().join("lua/object_logic/default_room.lua")).unwrap();
    let w = Rc::new(RefCell::new(persistence::load(&c.database()).unwrap()));
    assert!(Scripts::new(&c, w).is_err());
}

#[tokio::test(flavor = "current_thread")]
async fn connection_hooks_only_disconnect_last_session_and_shutdown_cleanly() {
    let (d, c) = imported();
    let mut w = persistence::load(&c.database()).unwrap();
    w.accounts.get_mut(&ObjectId(1)).unwrap().hash = Some(accounts::hash("secret", &c).unwrap());
    persistence::save(&c.database(), &w).unwrap();
    std::fs::write(d.path().join("lua/global_logic/session_test.lua"),r#"return {events={
 on_player_connect=function(ctx)
  assert(ctx.scope=='global' and ctx.object==nil and ctx.cause==ctx.enactor)
  local s=mux.world.object(ctx.enactor):state('connections')
  s:set('connects',s:get('connects',0)+1);s:set('reconnect',ctx.reconnect)
 end,
 on_player_disconnect=function(ctx)
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
    let w = persistence::load(&c.database()).unwrap();
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
        !persistence::load(&c.database()).unwrap().objects[&ObjectId(1)].state["connections"]
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
    let w = persistence::load(&c.database()).unwrap();
    assert_eq!(
        w.objects[&ObjectId(1)].state["connections"]["disconnects"],
        Scalar::Integer(1)
    );
}

#[test]
fn player_lookup_accepts_dbrefs_only_for_player_accounts() {
    let (_d, c) = imported();
    let mut world = persistence::load(&c.database()).unwrap();
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
    let (_d, c) = imported();
    let mut world = persistence::load(&c.database()).unwrap();
    world.accounts.get_mut(&ObjectId(2)).unwrap().hash =
        Some(accounts::hash("secret", &c).unwrap());
    persistence::save(&c.database(), &world).unwrap();
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
    let world = persistence::load(&c.database()).unwrap();
    assert_eq!(world.accounts.len(), 2);
    assert_eq!(world.objects.len(), 16);
    running.stop().await;
}
