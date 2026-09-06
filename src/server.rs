use crate::{
    accounts,
    commands::{self, Action},
    config::Config,
    persistence,
    scripting::Scripts,
    sessions::*,
    telnet::{self, Input},
    world::*,
};
use anyhow::{Context, Result, ensure};
use mlua::LuaSerdeExt;
use std::{
    cell::RefCell,
    collections::BTreeMap,
    future::Future,
    net::IpAddr,
    rc::Rc,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc,
};
use zeroize::Zeroizing;

enum Event {
    Bytes(SessionId, Vec<u8>),
    Gone(SessionId),
    Authenticated(SessionId, String, bool, Result<Option<String>>),
}
struct Bucket {
    tokens: usize,
    at: Instant,
}
struct Server {
    config: Config,
    scripts: Scripts,
    sessions: BTreeMap<SessionId, Session>,
    events: mpsc::Sender<Event>,
    addresses: BTreeMap<IpAddr, Bucket>,
    hashes: Bucket,
    inflight: usize,
}

pub async fn prepare(c: &Config) -> Result<Scripts> {
    let existing = c.database().exists();
    let world = if existing {
        let path = c.database();
        tokio::task::spawn_blocking(move || persistence::load(&path)).await??
    } else {
        let legacy = c
            .root
            .join(c.string("database.game_database", "data/stompymux.db"));
        ensure!(
            !legacy.exists() || std::fs::metadata(&legacy)?.len() == 0,
            "Legacy database exists. Run import-legacy explicitly before serving this world."
        );
        World::default()
    };
    let world = Rc::new(RefCell::new(world));
    if !existing {
        let mut w = world.borrow_mut();
        for (name, kind) in [
            ("Limbo", Kind::Room),
            ("GOD", Kind::Player),
            ("Wizard", Kind::Player),
            ("Used Mech Store", Kind::Room),
            ("Starter Room", Kind::Room),
            ("Afterlife", Kind::Room),
        ] {
            let id = w.create(c, name.into(), kind);
            if kind == Kind::Player {
                let o = w.objects.get_mut(&id).unwrap();
                o.location = Some(ObjectId(c.start()));
                o.home = Some(ObjectId(c.home()));
                o.flags.insert("WIZARD".into());
            }
        }
    }
    let scripts = Scripts::new(c, world)?;
    if !existing {
        let god = Zeroizing::new(accounts::random_password());
        let wizard = Zeroizing::new(accounts::random_password());
        let cc = c.clone();
        let g = god.clone();
        let z = wizard.clone();
        let (gh, zh) = tokio::task::spawn_blocking(move || -> Result<_> {
            Ok((accounts::hash(&g, &cc)?, accounts::hash(&z, &cc)?))
        })
        .await??;
        {
            let mut w = scripts.world.borrow_mut();
            w.accounts.insert(
                ObjectId(1),
                Account {
                    hash: Some(gh),
                    ..Default::default()
                },
            );
            w.accounts.insert(
                ObjectId(2),
                Account {
                    hash: Some(zh),
                    ..Default::default()
                },
            );
        }
        scripts.event("on_server_first_startup", None, None)?;
        scripts.world.borrow_mut().initialized = true;
        scripts.world.borrow().validate(c)?;
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let credentials = c.root.join("bootstrap-credentials.txt");
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&credentials)
            .context("cannot create bootstrap credentials; move any stale file before retrying")?;
        f.write_all(format!("GOD: {}\nWizard: {}\n", god.as_str(), wizard.as_str()).as_bytes())?;
        f.sync_all()?;
        let path = c.database();
        let snapshot = scripts.world.borrow().clone();
        if let Err(e) =
            tokio::task::spawn_blocking(move || persistence::initialize(&path, &snapshot)).await?
        {
            let _ = std::fs::remove_file(&credentials);
            return Err(e);
        }
        eprintln!("Bootstrap credentials written to {}", credentials.display());
    }
    scripts.world.borrow().validate(c)?;
    let before = scripts.world.borrow().clone();
    scripts.event("on_server_startup", None, None)?;
    let after = scripts.world.borrow().clone();
    if serde_json::to_vec(&before)? != serde_json::to_vec(&after)? {
        persistence::persist(c.database(), after).await?;
    }
    scripts.outbox.borrow_mut().clear();
    Ok(scripts)
}
pub async fn serve(c: Config, address: &str, shutdown: impl Future<Output = ()>) -> Result<()> {
    let scripts = prepare(&c).await?;
    let listener = TcpListener::bind(address).await?;
    run(c, scripts, listener, shutdown).await
}
pub async fn run(
    c: Config,
    scripts: Scripts,
    listener: TcpListener,
    shutdown: impl Future<Output = ()>,
) -> Result<()> {
    for warning in c.warnings.iter().chain(&scripts.warnings) {
        eprintln!("Warning: {warning}");
    }
    println!("Listening on {}", listener.local_addr()?);
    let (tx, mut rx) = mpsc::channel(256);
    let mut server = Server {
        config: c,
        scripts,
        sessions: BTreeMap::new(),
        events: tx.clone(),
        addresses: BTreeMap::new(),
        hashes: Bucket {
            tokens: 0,
            at: Instant::now() - Duration::from_secs(1),
        },
        inflight: 0,
    };
    let mut next = 0;
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    let mut tasks = tokio::task::JoinSet::new();
    tokio::pin!(shutdown);
    let banner =
        std::fs::read_to_string(server.config.root.join("text/connect.txt")).unwrap_or_default();
    loop {
        tokio::select! {
            _ = &mut shutdown => break,
            accepted = listener.accept() => {
                let (stream,peer)=accepted?;
                if server.sessions.len()>=1024 { drop(stream); continue; }
                next+=1;
                let id=SessionId(next);
                let (output,receiver)=mpsc::channel(128);
                let now=Instant::now();
                server.sessions.insert(id,Session {
                    output, peer:peer.ip(), player:None, flow:LoginFlow::Name,
                    connected:now, active:now, decoder:Default::default(),
                    quota:server.config.int("mux.command_quota_increment",1000) as usize,
                    quota_at:now, failed:Default::default(),
                });
                tasks.spawn(connection(stream,id,tx.clone(),receiver));
                let session=&server.sessions[&id];
                session.raw(telnet::Decoder::initial());
                session.text(&banner,true);
                session.text("Who are you? ",true);
            },
            event = rx.recv() => {
                if let Some(event)=event { match event {
                    Event::Gone(id) => server.disconnect(id).await?,
                    Event::Bytes(id,bytes) => server.input(id,&bytes).await?,
                    Event::Authenticated(id,name,create,result) => {
                        server.inflight=server.inflight.saturating_sub(1);
                        server.authenticated(id,name,create,result).await?;
                    }
                }}
            },
            _ = tick.tick() => {
                let timeout=Duration::from_secs(server.config.int("mux.idle_timeout",3600) as u64);
                let idle:Vec<_>=server.sessions.iter()
                    .filter(|(_,s)|s.active.elapsed()>timeout||s.output.is_closed()||s.failed.get())
                    .map(|(id,_)|*id).collect();
                for id in idle { server.disconnect(id).await?; }
                server.addresses.retain(|_,b|b.at.elapsed()<Duration::from_secs(86400));
            },
            _ = tasks.join_next(), if !tasks.is_empty() => {}
        }
    }
    for id in server.sessions.keys().copied().collect::<Vec<_>>() {
        server.disconnect(id).await?;
    }
    drop(rx);
    if tokio::time::timeout(Duration::from_secs(5), async {
        while tasks.join_next().await.is_some() {}
    })
    .await
    .is_err()
    {
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
    }
    Ok(())
}
async fn connection(
    stream: TcpStream,
    id: SessionId,
    events: mpsc::Sender<Event>,
    mut output: mpsc::Receiver<Output>,
) {
    let (mut read, mut write) = stream.into_split();
    let mut buffer = [0u8; 1024];
    loop {
        tokio::select! {
            result = read.read(&mut buffer) => match result {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if events.send(Event::Bytes(id,buffer[..n].to_vec())).await.is_err() { break; }
                }
            },
            message = output.recv() => match message {
                Some(Output::Bytes(bytes)) => {
                    if !matches!(tokio::time::timeout(Duration::from_secs(5),write.write_all(&bytes)).await,Ok(Ok(()))) { break; }
                },
                Some(Output::Close) | None => break,
            }
        }
    }
    let _ = write.shutdown().await;
    let _ = events.send(Event::Gone(id)).await;
}
impl Server {
    fn tell(&self, id: SessionId, s: &str) {
        if let Some(session) = self.sessions.get(&id) {
            let ansi = session.player.is_none_or(|p| {
                self.scripts.world.borrow().objects[&p]
                    .flags
                    .contains("ANSI")
            });
            if !session.text(s, ansi) {
                session.close();
            }
        }
    }
    fn prompt(&mut self, id: SessionId, flow: LoginFlow, text: &str, secret: bool) {
        if let Some(s) = self.sessions.get_mut(&id) {
            s.flow = flow;
            s.raw(telnet::Decoder::echo(secret));
        }
        self.tell(id, text);
    }
    fn snapshots(&self) -> Result<()> {
        let w = self.scripts.world.borrow();
        let mut hidden = 0;
        let list:Vec<_>=self.sessions.values().filter_map(|session| {
            let id=session.player?;
            let object=&w.objects[&id];
            if object.flags.contains("DARK") { hidden+=1; return None; }
            Some(serde_json::json!({"name":object.name,"dbref":id.0,"connected_for":session.connected.elapsed().as_secs(),"idle_for":session.active.elapsed().as_secs()}))
        }).collect();
        self.scripts
            .lua
            .globals()
            .set(
                "_connected_players",
                self.scripts
                    .lua
                    .to_value(&list)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?,
            )
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        self.scripts
            .lua
            .globals()
            .set(
                "_who_summary",
                self.scripts
                    .lua
                    .to_value(&serde_json::json!({"hidden":hidden,"record":w.record_players}))
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?,
            )
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        Ok(())
    }
    async fn commit(&mut self, before: World) -> bool {
        let after = self.scripts.world.borrow().clone();
        let result = match after.validate(&self.config) {
            Err(e) => Err(e),
            Ok(()) => match (serde_json::to_vec(&before), serde_json::to_vec(&after)) {
                (Ok(a), Ok(b)) if a == b => Ok(()),
                _ => persistence::persist(self.config.database(), after).await,
            },
        };
        if let Err(e) = result {
            eprintln!("Persistence failed: {e:#}");
            *self.scripts.world.borrow_mut() = before;
            self.scripts.outbox.borrow_mut().clear();
            false
        } else {
            true
        }
    }
    fn flush(&self) {
        let messages = std::mem::take(&mut *self.scripts.outbox.borrow_mut());
        for (p, text) in messages {
            for (id, s) in &self.sessions {
                if s.player == Some(p) {
                    self.tell(*id, &format!("{text}\r\n"));
                }
            }
        }
    }
    async fn disconnect(&mut self, id: SessionId) -> Result<()> {
        if let Some(s) = self.sessions.remove(&id) {
            s.close();
            if let Some(p) = s.player {
                if self.sessions.values().any(|s| s.player == Some(p)) {
                    return Ok(());
                }
                let before = self.scripts.world.borrow().clone();
                if !self.sessions.values().any(|s| s.player == Some(p)) {
                    self.scripts
                        .world
                        .borrow_mut()
                        .objects
                        .get_mut(&p)
                        .unwrap()
                        .flags
                        .remove("CONNECTED");
                }
                if let Err(e) = self
                    .scripts
                    .event("on_player_disconnect", Some(p), Some(id.0))
                {
                    eprintln!("Disconnect hook: {e:#}");
                    *self.scripts.world.borrow_mut() = before.clone();
                    self.scripts.outbox.borrow_mut().clear();
                }
                self.commit(before).await;
                self.flush();
            }
        }
        Ok(())
    }
    async fn input(&mut self, id: SessionId, bytes: &[u8]) -> Result<()> {
        let Some(session) = self.sessions.get_mut(&id) else {
            return Ok(());
        };
        let inputs = match session.decoder.feed(bytes) {
            Ok(v) => v,
            Err(_) => {
                self.tell(id, "Input limit exceeded.\r\n");
                self.disconnect(id).await?;
                return Ok(());
            }
        };
        for input in inputs {
            if !self.sessions.contains_key(&id) {
                break;
            }
            match input {
                Input::Reply(v) => {
                    self.sessions[&id].raw(v);
                }
                Input::InvalidUtf8 => self.tell(id, "Invalid UTF-8 input.\r\n"),
                Input::Line(line) => {
                    let line = Zeroizing::new(line);
                    let s = self.sessions.get_mut(&id).unwrap();
                    s.active = Instant::now();
                    if s.quota_at.elapsed()
                        >= Duration::from_millis(
                            self.config.int("mux.command_quota_interval", 50) as u64
                        )
                    {
                        s.quota = self.config.int("mux.command_quota_increment", 1000) as usize;
                        s.quota_at = Instant::now();
                    }
                    if s.quota == 0 {
                        self.tell(id, "Command quota exceeded.\r\n");
                        continue;
                    }
                    s.quota -= 1;
                    if let Some(p) = s.player {
                        self.command(id, p, &line).await?;
                    } else {
                        self.login(id, &line).await?;
                    }
                }
            }
        }
        Ok(())
    }
    async fn command(&mut self, id: SessionId, p: ObjectId, line: &str) -> Result<()> {
        self.snapshots()?;
        let before = self.scripts.world.borrow().clone();
        match commands::run(&self.scripts, &self.config, p, id.0, line) {
            Ok(Action::Quit) => {
                self.tell(
                    id,
                    &std::fs::read_to_string(self.config.root.join("text/quit.txt"))
                        .unwrap_or_else(|_| "Goodbye.\r\n".into()),
                );
                self.disconnect(id).await?;
            }
            Ok(Action::Continue) => {
                if self.commit(before).await {
                    self.flush();
                } else {
                    self.tell(id, "Unable to save your changes. Please try again.\r\n");
                }
            }
            Err(e) => {
                *self.scripts.world.borrow_mut() = before;
                self.scripts.outbox.borrow_mut().clear();
                eprintln!("Command callback failed: {e:#}");
                if e.to_string().contains("Interactive Lua flows") {
                    self.tell(
                        id,
                        "Interactive Lua flows are unavailable in this milestone.\r\n",
                    );
                } else {
                    let report = self.config.string("lua.error_reporting", "wizards");
                    let wizard = self.scripts.world.borrow().objects[&p]
                        .flags
                        .contains("WIZARD");
                    if report == "all" || (report == "wizards" && wizard) {
                        self.tell(id, &format!("Lua error: {e}\r\n"));
                    } else {
                        self.tell(id, "That command could not be completed.\r\n");
                    }
                }
            }
        }
        Ok(())
    }
    async fn login(&mut self, id: SessionId, input: &str) -> Result<()> {
        let flow = std::mem::replace(
            &mut self.sessions.get_mut(&id).unwrap().flow,
            LoginFlow::Name,
        );
        match flow {
            LoginFlow::Name => {
                if input.is_empty() {
                    self.prompt(id, LoginFlow::Name, "Who are you? ", false);
                } else if self.scripts.world.borrow().find_player(input).is_some() {
                    self.prompt(id, LoginFlow::Password(input.into()), "Password: ", true);
                } else if let Err(e) = accounts::validate_name(input, &self.config) {
                    self.prompt(id, LoginFlow::Name, &format!("{e}\r\nWho are you? "), false);
                } else {
                    self.prompt(
                        id,
                        LoginFlow::ConfirmCreate(input.into()),
                        &format!("Create a new player named {input}? [Y/n] "),
                        false,
                    );
                }
            }
            LoginFlow::Password(name) => {
                self.authenticate(id, name, Zeroizing::new(input.into()), false)
            }
            LoginFlow::ConfirmCreate(name) => match input.to_ascii_lowercase().as_str() {
                "" | "y" | "yes" => self.prompt(
                    id,
                    LoginFlow::NewPassword(name),
                    "Choose a password: ",
                    true,
                ),
                "n" | "no" => self.prompt(id, LoginFlow::Name, "Who are you? ", false),
                _ => self.prompt(
                    id,
                    LoginFlow::ConfirmCreate(name),
                    "Please answer y or n: ",
                    false,
                ),
            },
            LoginFlow::NewPassword(name) => {
                if let Err(e) = accounts::validate_password(input, &self.config) {
                    self.prompt(
                        id,
                        LoginFlow::NewPassword(name),
                        &format!("{e}\r\nChoose a password: "),
                        true,
                    );
                } else {
                    self.prompt(
                        id,
                        LoginFlow::ConfirmPassword(name, Zeroizing::new(input.into())),
                        "Retype password: ",
                        true,
                    );
                }
            }
            LoginFlow::ConfirmPassword(name, password) => {
                if input != password.as_str() {
                    self.prompt(
                        id,
                        LoginFlow::NewPassword(name),
                        "Passwords did not match.\r\nChoose a password: ",
                        true,
                    );
                } else {
                    self.authenticate(id, name, password, true);
                }
            }
            LoginFlow::Pending => {
                self.sessions.get_mut(&id).unwrap().flow = LoginFlow::Pending;
                self.tell(id, "Authentication in progress.\r\n");
            }
        }
        Ok(())
    }
    fn authenticate(
        &mut self,
        id: SessionId,
        name: String,
        password: Zeroizing<String>,
        create: bool,
    ) {
        let now = Instant::now();
        let burst = self.config.int("security.login_attempt_burst", 3) as usize;
        let refill = self.config.int("security.login_attempt_refill", 10) as u64;
        let address = self.sessions[&id].peer;
        if self.addresses.len() >= 4096 && !self.addresses.contains_key(&address) {
            self.prompt(
                id,
                LoginFlow::Name,
                "Login capacity reached. Try later.\r\nWho are you? ",
                false,
            );
            return;
        }
        let bucket = self.addresses.entry(address).or_insert(Bucket {
            tokens: burst,
            at: now,
        });
        let count = bucket.at.elapsed().as_secs() / refill;
        if count > 0 {
            bucket.tokens = (bucket.tokens + count as usize).min(burst);
            bucket.at = now;
        }
        if self.hashes.at.elapsed() >= Duration::from_secs(1) {
            self.hashes.tokens = self.config.int("security.login_hash_limit", 5) as usize;
            self.hashes.at = now;
        }
        if bucket.tokens == 0
            || self.hashes.tokens == 0
            || self.inflight >= self.config.int("security.login_hash_limit", 5) as usize
        {
            self.prompt(
                id,
                LoginFlow::Name,
                "Too many login attempts. Please wait.\r\nWho are you? ",
                false,
            );
            return;
        }
        bucket.tokens -= 1;
        self.hashes.tokens -= 1;
        self.inflight += 1;
        let hash = self
            .scripts
            .world
            .borrow()
            .find_player(&name)
            .and_then(|p| self.scripts.world.borrow().accounts[&p].hash.clone());
        self.prompt(id, LoginFlow::Pending, "", false);
        let c = self.config.clone();
        let tx = self.events.clone();
        tokio::spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                if create {
                    accounts::hash(&password, &c).map(Some)
                } else if hash.is_some_and(|h| accounts::verify(&password, &h)) {
                    Ok(None)
                } else {
                    Err(anyhow::anyhow!("invalid credentials"))
                }
            })
            .await
            .unwrap_or_else(|e| Err(e.into()));
            let _ = tx
                .send(Event::Authenticated(id, name, create, result))
                .await;
        });
    }
    async fn authenticated(
        &mut self,
        id: SessionId,
        name: String,
        create: bool,
        result: Result<Option<String>>,
    ) -> Result<()> {
        if !self
            .sessions
            .get(&id)
            .is_some_and(|s| matches!(s.flow, LoginFlow::Pending))
        {
            return Ok(());
        }
        let before = self.scripts.world.borrow().clone();
        let existing = self.scripts.world.borrow().find_player(&name);
        let host = self.sessions[&id].peer.to_string();
        let hash = match result {
            Ok(h) => h,
            Err(_) => {
                if let Some(p) = existing {
                    let mut w = self.scripts.world.borrow_mut();
                    let a = w.accounts.get_mut(&p).unwrap();
                    a.failures += 1;
                    a.unreported_failures += 1;
                    a.history.push(Login {
                        success: false,
                        at: accounts::now(),
                        host,
                    });
                    if a.history.len() > 32 {
                        a.history.remove(0);
                    }
                }
                self.commit(before).await;
                self.prompt(id,LoginFlow::Name,"Either that player does not exist, or has a different password.\r\nWho are you? ",false);
                return Ok(());
            }
        };
        let p = if create {
            if existing.is_some() {
                self.prompt(
                    id,
                    LoginFlow::Name,
                    "That name has just been registered.\r\nWho are you? ",
                    false,
                );
                return Ok(());
            }
            let mut w = self.scripts.world.borrow_mut();
            let p = w.create(&self.config, name, Kind::Player);
            let o = w.objects.get_mut(&p).unwrap();
            o.location = Some(ObjectId(self.config.start()));
            o.home = Some(ObjectId(self.config.home()));
            w.accounts.insert(
                p,
                Account {
                    hash,
                    ..Default::default()
                },
            );
            p
        } else {
            existing.context("authenticated player disappeared")?
        };
        {
            let mut w = self.scripts.world.borrow_mut();
            let a = w.accounts.get_mut(&p).unwrap();
            a.last_login = Some(accounts::now());
            a.last_site = Some(host.clone());
            a.successes += 1;
            a.history.push(Login {
                success: true,
                at: accounts::now(),
                host,
            });
            if a.history.len() > 32 {
                a.history.remove(0);
            }
            w.objects
                .get_mut(&p)
                .unwrap()
                .flags
                .insert("CONNECTED".into());
        }
        if !self.commit(before).await {
            self.prompt(
                id,
                LoginFlow::Name,
                "Unable to save login.\r\nWho are you? ",
                false,
            );
            return Ok(());
        }
        let reconnect = self.sessions.values().any(|s| s.player == Some(p));
        let session = self.sessions.get_mut(&id).unwrap();
        session.player = Some(p);
        session.connected = Instant::now();
        session.flow = LoginFlow::Name;
        let before = self.scripts.world.borrow().clone();
        let count = self
            .sessions
            .values()
            .filter(|s| s.player.is_some())
            .count();
        {
            let mut w = self.scripts.world.borrow_mut();
            w.record_players = w.record_players.max(count);
        }
        self.snapshots()?;
        if let Err(e) =
            self.scripts
                .lifecycle("on_player_connect", Some(p), Some(id.0), reconnect, "")
        {
            eprintln!("Connect hook: {e:#}");
            *self.scripts.world.borrow_mut() = before.clone();
            self.scripts.outbox.borrow_mut().clear();
        }
        self.commit(before).await;
        self.flush();
        self.tell(id, "Connected.\r\n");
        self.command(id, p, "look").await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test(flavor = "current_thread")]
    async fn stale_authentication_cannot_create_a_player() {
        let c = Config::load(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"),
        )
        .unwrap();
        let world = persistence::read_legacy(&c.root.join("data/stompymux.db"), &c).unwrap();
        let scripts = Scripts::new(&c, Rc::new(RefCell::new(world))).unwrap();
        let (events, _) = mpsc::channel(1);
        let mut server = Server {
            config: c,
            scripts,
            sessions: BTreeMap::new(),
            events,
            addresses: BTreeMap::new(),
            hashes: Bucket {
                tokens: 1,
                at: Instant::now(),
            },
            inflight: 0,
        };
        server
            .authenticated(
                SessionId(999),
                "Disconnected".into(),
                true,
                Ok(Some("unused-hash".into())),
            )
            .await
            .unwrap();
        assert_eq!(server.scripts.world.borrow().accounts.len(), 2);
        assert!(
            server
                .scripts
                .world
                .borrow()
                .find_player("Disconnected")
                .is_none()
        );
    }
}
