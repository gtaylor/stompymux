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

// login_hash_limit is measured per second; this is its unit, not a tunable.
const HASH_RATE_WINDOW: Duration = Duration::from_secs(1);

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
    c.validate_for_serve()?;
    let existing = c.database().exists();
    let world = if existing {
        let path = c.database();
        let timeout = c.database.busy_timeout_ms;
        persistence::load_with_timeout(&path, timeout).await?
    } else {
        let legacy = c.legacy_database();
        ensure!(
            !legacy.exists() || std::fs::metadata(&legacy)?.len() == 0,
            "Legacy database exists. Run import-legacy explicitly before serving this world."
        );
        World::default()
    };
    let world = Rc::new(RefCell::new(world));
    if !existing {
        let mut w = world.borrow_mut();
        for (dbref, entry) in &c.database.bootstrap.objects {
            w.next_id = dbref.0;
            let kind = match entry.r#type {
                crate::config::BootstrapKind::Room => Kind::Room,
                crate::config::BootstrapKind::Player => Kind::Player,
            };
            let id = w.create(c, entry.name.clone(), kind);
            if kind == Kind::Player {
                let object = w.objects.get_mut(&id).unwrap();
                object.location = Some(ObjectId(c.start()));
                object.home = Some(ObjectId(c.home()));
                if entry.wizard {
                    object.flags.insert(crate::flags::Flag::Wizard);
                }
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
        let credentials = c.path(&c.database.bootstrap.credentials_file);
        if let Some(parent) = credentials.parent() {
            std::fs::create_dir_all(parent)?;
        }
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
        let busy_timeout_ms = c.database.busy_timeout_ms;
        if let Err(e) =
            persistence::initialize_with_timeout(&path, &snapshot, busy_timeout_ms).await
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
        persistence::persist(c.database(), after, c.database.busy_timeout_ms).await?;
    }
    scripts.outbox.borrow_mut().clear();
    Ok(scripts)
}
pub async fn serve(c: Config, shutdown: impl Future<Output = ()>) -> Result<()> {
    let address = c.listener();
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
    let (tx, mut rx) = mpsc::channel(c.runtime.event_queue_capacity);
    let mut server = Server {
        config: c,
        scripts,
        sessions: BTreeMap::new(),
        events: tx.clone(),
        addresses: BTreeMap::new(),
        hashes: Bucket {
            tokens: 0,
            at: Instant::now() - HASH_RATE_WINDOW,
        },
        inflight: 0,
    };
    let mut next = 0;
    let mut tick = tokio::time::interval(Duration::from_millis(
        server.config.runtime.maintenance_interval_ms,
    ));
    let mut tasks = tokio::task::JoinSet::new();
    let mut idle_check =
        tokio::time::interval(Duration::from_secs(server.config.mux.idle_interval));
    tokio::pin!(shutdown);
    let banner = std::fs::read_to_string(server.config.path(&server.config.mux.connect_file))
        .unwrap_or_default();
    loop {
        tokio::select! {
            _ = &mut shutdown => break,
            accepted = listener.accept() => {
                let (stream,peer)=accepted?;
                if server.sessions.len()>=server.config.runtime.max_connections { drop(stream); continue; }
                next+=1;
                let id=SessionId(next);
                let (output,receiver)=mpsc::channel(server.config.runtime.session_output_queue_capacity);
                let now=Instant::now();
                server.sessions.insert(id,Session {
                    output, peer:peer.ip(), player:None, flow:LoginFlow::Name,
                    connected:now, active:now, decoder:telnet::Decoder::new(&server.config.runtime),
                    find_cursor: None,
                    output_message_limit:server.config.runtime.output_message_limit,
                    quota:server.config.mux.command_quota_increment.min(server.config.mux.command_quota_max),
                    quota_at:now, failed:Default::default(),
                });
                tasks.spawn(connection(stream,id,tx.clone(),receiver,server.config.runtime.write_timeout_ms));
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
                let timeout=Duration::from_secs(server.config.mux.conn_timeout);
                let idle:Vec<_>=server.sessions.iter()
                    .filter(|(_,s)|(s.player.is_none() && s.connected.elapsed()>timeout)||s.output.is_closed()||s.failed.get())
                    .map(|(id,_)|*id).collect();
                for id in idle { server.disconnect(id).await?; }
                server.addresses.retain(|_,b|b.at.elapsed()<Duration::from_secs(server.config.security.login_address_retention_seconds));
            },
            _ = idle_check.tick() => {
                let timeout=Duration::from_secs(server.config.mux.idle_timeout);
                let idle:Vec<_>=server.sessions.iter().filter(|(_,s)|s.player.is_some() && s.active.elapsed()>timeout).map(|(id,_)|*id).collect();
                for id in idle {server.disconnect(id).await?;}
            },
            _ = tasks.join_next(), if !tasks.is_empty() => {}
        }
    }
    for id in server.sessions.keys().copied().collect::<Vec<_>>() {
        server.disconnect(id).await?;
    }
    drop(rx);
    if tokio::time::timeout(
        Duration::from_millis(server.config.runtime.shutdown_timeout_ms),
        async { while tasks.join_next().await.is_some() {} },
    )
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
    write_timeout_ms: u64,
) {
    let (mut read, mut write) = stream.into_split();
    const READ_BUFFER_SIZE: usize = 1024;
    let mut buffer = [0u8; READ_BUFFER_SIZE];
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
                    if !matches!(tokio::time::timeout(Duration::from_millis(write_timeout_ms),write.write_all(&bytes)).await,Ok(Ok(()))) { break; }
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
                    .contains(crate::flags::Flag::Ansi)
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
            if object.flags.contains(crate::flags::Flag::Dark) { hidden+=1; return None; }
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
    /// Session state is authoritative even when durable world mutations roll back.
    fn reconcile_connections(&self) {
        let connected: std::collections::BTreeSet<_> =
            self.sessions.values().filter_map(|s| s.player).collect();
        for o in self.scripts.world.borrow_mut().objects.values_mut() {
            if o.kind == Kind::Player && connected.contains(&o.id) {
                o.flags.insert(crate::flags::Flag::Connected);
            } else {
                o.flags.remove(crate::flags::Flag::Connected);
            }
        }
    }
    async fn commit(&mut self, before: World) -> bool {
        let after = self.scripts.world.borrow().clone();
        let result = match after.validate(&self.config) {
            Err(e) => Err(e),
            Ok(()) => match (serde_json::to_vec(&before), serde_json::to_vec(&after)) {
                (Ok(a), Ok(b)) if a == b => Ok(()),
                _ => {
                    persistence::persist(
                        self.config.database(),
                        after,
                        self.config.database.busy_timeout_ms,
                    )
                    .await
                }
            },
        };
        if let Err(e) = result {
            eprintln!("Persistence failed: {e:#}");
            *self.scripts.world.borrow_mut() = before;
            self.reconcile_connections();
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
            self.reconcile_connections();
            if let Some(p) = s.player {
                if self.sessions.values().any(|s| s.player == Some(p)) {
                    return Ok(());
                }
                let before = self.scripts.world.borrow().clone();
                self.reconcile_connections();
                if let Err(e) = self
                    .scripts
                    .event("on_player_disconnect", Some(p), Some(id.0))
                {
                    eprintln!("Disconnect hook: {e:#}");
                    *self.scripts.world.borrow_mut() = before.clone();
                    self.reconcile_connections();
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
                    let elapsed = s.quota_at.elapsed().as_millis();
                    let interval = u128::from(self.config.mux.command_quota_interval);
                    let periods = elapsed / interval;
                    if periods > 0 {
                        let credits =
                            periods.saturating_mul(self.config.mux.command_quota_increment as u128);
                        s.quota = (s.quota as u128 + credits)
                            .min(self.config.mux.command_quota_max as u128)
                            as usize;
                        // Preserve the partial refill interval between commands.
                        s.quota_at =
                            Instant::now() - Duration::from_millis((elapsed % interval) as u64);
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
    /// Deliver a read-only page only to the invoking session, bypassing persistence.
    fn find(&mut self, id: SessionId, player: ObjectId, request: crate::find::FindRequest) {
        use crate::find::{FindCursor, FindRequest, bounded_error, page};
        let Some(session) = self.sessions.get_mut(&id) else {
            return;
        };
        let world = self.scripts.world.borrow();
        let limit = self.config.runtime.output_message_limit;
        let error = if !crate::flags::is_wizard(&world, player) {
            Some("Permission denied.".to_string())
        } else {
            match request {
                FindRequest::Search(args) => {
                    session.find_cursor = Some(FindCursor::new(&args, &world));
                    None
                }
                FindRequest::Next if session.find_cursor.is_none() => {
                    Some("No active @find search.".into())
                }
                FindRequest::Next => None,
                FindRequest::Error(error) => Some(error),
            }
        };
        if let Some(error) = error {
            session.raw(bounded_error(&error, limit));
            return;
        }
        let cursor = session
            .find_cursor
            .as_ref()
            .expect("validated search cursor");
        match page(
            &world,
            player,
            cursor,
            self.config.runtime.find_page_size,
            limit,
        ) {
            Ok(page) => {
                if session.raw(page.bytes) {
                    session.find_cursor = page.cursor;
                }
            }
            Err(error) => {
                session.raw(bounded_error(error, limit));
            }
        }
    }
    async fn command(&mut self, id: SessionId, p: ObjectId, line: &str) -> Result<()> {
        self.snapshots()?;
        let before = self.scripts.world.borrow().clone();
        match commands::run(&self.scripts, &self.config, p, id.0, line) {
            Ok(Action::Find(request)) => self.find(id, p, request),
            Ok(Action::Quit) => {
                self.tell(
                    id,
                    &std::fs::read_to_string(self.config.path(&self.config.mux.quit_file))
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
                self.reconcile_connections();
                self.scripts.outbox.borrow_mut().clear();
                eprintln!("Command callback failed: {e:#}");
                if e.to_string().contains("Interactive Lua flows") {
                    self.tell(
                        id,
                        "Interactive Lua flows are unavailable in this milestone.\r\n",
                    );
                } else {
                    let report = self.config.lua.error_reporting;
                    let wizard = self.scripts.world.borrow().objects[&p]
                        .flags
                        .contains(crate::flags::Flag::Wizard);
                    if report == crate::config::ErrorReporting::All
                        || (report == crate::config::ErrorReporting::Wizards && wizard)
                    {
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
        let burst = self.config.security.login_attempt_burst;
        let refill = self.config.security.login_attempt_refill;
        let address = self.sessions[&id].peer;
        if self.addresses.len() >= self.config.security.login_address_limit
            && !self.addresses.contains_key(&address)
        {
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
        if self.hashes.at.elapsed() >= HASH_RATE_WINDOW {
            self.hashes.tokens = self.config.security.login_hash_limit;
            self.hashes.at = now;
        }
        if bucket.tokens == 0
            || self.hashes.tokens == 0
            || self.inflight >= self.config.security.login_hash_concurrency
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
                    let excess = a
                        .history
                        .len()
                        .saturating_sub(self.config.security.login_history_limit);
                    a.history.drain(..excess);
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
            let excess = a
                .history
                .len()
                .saturating_sub(self.config.security.login_history_limit);
            a.history.drain(..excess);
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
        self.reconcile_connections();
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
            self.reconcile_connections();
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
        let world = persistence::read_legacy(&c.root.join("data/stompymux.db"), &c)
            .await
            .unwrap();
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
    /// Failures cannot undo the final session detachment or its CONNECTED state.
    #[tokio::test(flavor = "current_thread")]
    async fn connection_state_survives_hook_and_persistence_rollback() {
        fn copy(source: &std::path::Path, target: &std::path::Path) {
            std::fs::create_dir_all(target).unwrap();
            for entry in std::fs::read_dir(source).unwrap() {
                let entry = entry.unwrap();
                if entry.path().is_dir() {
                    copy(&entry.path(), &target.join(entry.file_name()));
                } else {
                    std::fs::copy(entry.path(), target.join(entry.file_name())).unwrap();
                }
            }
        }
        for hook_failure in [false, true] {
            let d = tempfile::tempdir().unwrap();
            copy(
                &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"),
                d.path(),
            );
            std::fs::write(
                d.path().join("lua/global_logic/failure_test.lua"),
                format!(
                    r#"return {{events={{on_player_disconnect=function(ctx)
                local o=mux.world.object(ctx.enactor)
                assert(not o:flags():has(mux.world.flags.CONNECTED))
                o:state('failure'):set('changed',true)
                {}
            end}}}}"#,
                    if hook_failure {
                        "error('injected disconnect failure')"
                    } else {
                        ""
                    }
                ),
            )
            .unwrap();
            let c = Config::load(d.path()).unwrap();
            persistence::import(&c.legacy_database(), &c).await.unwrap();
            let world = persistence::load(&c.database()).await.unwrap();
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
            let mut receivers = Vec::new();
            for id in [1, 2] {
                let (output, receiver) = mpsc::channel(16);
                receivers.push(receiver);
                let now = Instant::now();
                server.sessions.insert(
                    SessionId(id),
                    Session {
                        output,
                        peer: "127.0.0.1".parse().unwrap(),
                        player: Some(ObjectId(1)),
                        flow: LoginFlow::Name,
                        connected: now,
                        active: now,
                        decoder: Default::default(),
                        quota: 1,
                        quota_at: now,
                        failed: Default::default(),
                        find_cursor: None,
                        output_message_limit: 65536,
                    },
                );
            }
            server.reconcile_connections();
            assert!(
                server.scripts.world.borrow().objects[&ObjectId(1)]
                    .flags
                    .contains(crate::flags::Flag::Connected)
            );
            let mut db = <sqlx::SqliteConnection as sqlx::Connection>::connect_with(
                &sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(server.config.database())
                    .foreign_keys(false),
            )
            .await
            .unwrap();
            sqlx::raw_sql("CREATE TRIGGER fail BEFORE UPDATE ON world BEGIN SELECT RAISE(FAIL,'injected'); END;").execute(&mut db).await.unwrap();
            server.disconnect(SessionId(1)).await.unwrap();
            assert!(
                server.scripts.world.borrow().objects[&ObjectId(1)]
                    .flags
                    .contains(crate::flags::Flag::Connected)
            );
            server.disconnect(SessionId(2)).await.unwrap();
            let world = server.scripts.world.borrow().clone();
            assert!(
                !world.objects[&ObjectId(1)]
                    .flags
                    .contains(crate::flags::Flag::Connected)
            );
            assert!(!world.objects[&ObjectId(1)].state.contains_key("failure"));
            server
                .scripts
                .lua
                .load("assert(not mux.world.object(1):flags():has(mux.world.flags.CONNECTED))")
                .exec()
                .unwrap();
            sqlx::Connection::close(db).await.unwrap();
        }
    }
}
