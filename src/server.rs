//! Serialized world owner, connection lifecycle and common graceful shutdown coordinator.
use crate::{
    accounts,
    commands::{self, Action},
    config::Config,
    lua::Scripts,
    persistence,
    sessions::*,
    telnet::{self, Input},
    world::*,
};
use anyhow::{Context, Result};
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

/// Origin of a request handled exclusively by the world owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShutdownRequest {
    /// Interrupt signal.
    Sigint,
    /// Termination signal.
    Sigterm,
    /// Authenticated administrator.
    Player(ObjectId),
}

enum Event {
    Bytes(SessionId, Vec<u8>),
    Gone(SessionId),
    Authenticated(
        SessionId,
        String,
        bool,
        Option<ObjectId>,
        Result<Option<String>>,
    ),
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
    /// Runtime identity for MSSP replies.
    started_at: i64,
    listen_port: u16,
    /// Accepted request; also prevents further command and authentication dispatch.
    shutdown: Option<ShutdownRequest>,
    /// A shutdown write failed, even if a later snapshot succeeds.
    shutdown_failed: bool,
}

pub async fn prepare(c: &Config) -> Result<Scripts> {
    c.validate_for_serve()?;
    let existing = c.database().exists();
    let world = if existing {
        let path = c.database();
        let timeout = c.database.busy_timeout_ms;
        let mut loaded = persistence::load_with_timeout(&path, timeout).await?;
        loaded.palette = std::sync::Arc::new(crate::text::Palette::from_config(c)?);
        loaded.validate(c)?;
        persistence::validate_lists(&path, &loaded, timeout).await?;
        loaded
    } else {
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
    let help_config = c.clone();
    let help =
        tokio::task::spawn_blocking(move || crate::help::HelpIndex::load(&help_config)).await??;
    let scripts = Scripts::with_help(c, world, help)?;
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
    scripts.event("on_server_startup", None, None)?;
    let after = scripts.world.borrow().clone();
    after.validate(c)?;
    // Also clears stale stored CONNECTED values; unchanged durable fields are not rewritten.
    persistence::persist(c.database(), after, c.database.busy_timeout_ms).await?;
    scripts.outbox.borrow_mut().clear();
    Ok(scripts)
}
pub async fn serve(c: Config, shutdown: impl Future<Output = ShutdownRequest>) -> Result<()> {
    let address = c.listener();
    let scripts = prepare(&c).await?;
    let listener = TcpListener::bind(address).await?;
    run(c, scripts, listener, shutdown).await
}
pub async fn run(
    c: Config,
    scripts: Scripts,
    listener: TcpListener,
    shutdown: impl Future<Output = ShutdownRequest>,
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
        started_at: accounts::now(),
        listen_port: 0,
        shutdown: None,
        shutdown_failed: false,
    };
    server.listen_port = listener.local_addr()?.port();
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
            request = &mut shutdown => { server.request_shutdown(request).await; },
            accepted = listener.accept() => {
                let (stream,peer)=accepted?;
                if server.sessions.len()>=server.config.runtime.max_connections { drop(stream); continue; }
                next+=1;
                let id=SessionId(next);
                let (output,receiver)=mpsc::channel(server.config.runtime.session_output_queue_capacity);
                let now=Instant::now();
                let stats=std::sync::Arc::new(telnet::transport::Stats::default());
                server.sessions.insert(id,Session {
                    palette:server.scripts.palette.clone(), color_override:Default::default(), presets_emitted:Default::default(),
                    stats:stats.clone(),output, peer:peer.ip(), player:None, flow:LoginFlow::Name,
                    connected:now, active:now, decoder:telnet::Decoder::new(&server.config.runtime),
                    find_cursor: None,
                    output_message_limit:server.config.runtime.output_message_limit,
                    quota:server.config.mux.command_quota_increment.min(server.config.mux.command_quota_max),
                    quota_at:now, failed:Default::default(),
                });
                tasks.spawn(connection(stream,id,tx.clone(),receiver,server.config.runtime.write_timeout_ms,stats));
                let session=server.sessions.get_mut(&id).unwrap();
                let negotiation=session.decoder.initial();
                session.protocol(negotiation);
                session.text(&banner,true);
                session.text("Who are you? ",true);
            },
            event = rx.recv() => {
                if let Some(event)=event { match event {
                    Event::Gone(id) => server.disconnect(id).await?,
                    Event::Bytes(id,bytes) => server.input(id,&bytes).await?,
                    Event::Authenticated(id,name,create,identity,result) => {
                        server.authentication_result(id,name,create,identity,result).await?;
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
        if server.shutdown.is_some() {
            break;
        }
    }
    drop(listener);
    server.finish_shutdown().await;

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
    anyhow::ensure!(
        !server.shutdown_failed,
        "shutdown completed with unsaved changes; see server diagnostics"
    );
    Ok(())
}
async fn connection(
    stream: TcpStream,
    id: SessionId,
    events: mpsc::Sender<Event>,
    mut output: mpsc::Receiver<Output>,
    write_timeout_ms: u64,
    stats: std::sync::Arc<telnet::transport::Stats>,
) {
    use std::sync::atomic::Ordering::Relaxed;
    let mut writer = telnet::transport::Writer::default();
    let (mut read, mut write) = stream.into_split();
    const READ_BUFFER_SIZE: usize = 1024;
    let mut buffer = [0u8; READ_BUFFER_SIZE];
    loop {
        tokio::select! {
            result = read.read(&mut buffer), if !events.is_closed() => match result {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    stats.input_total.fetch_add(n as u64,Relaxed);
                    stats.input_pending.fetch_add(n as u64,Relaxed);
                    if events.send(Event::Bytes(id,buffer[..n].to_vec())).await.is_err(){stats.consumed(n as u64);stats.input_lost.fetch_add(n as u64,Relaxed);}
                }
            },
            message = output.recv() => match message {
                Some(Output::Bytes(bytes)) => {
                    let result=tokio::time::timeout(Duration::from_millis(write_timeout_ms),writer.bytes(&mut write,&stats,&bytes)).await;
                    stats.output_pending.fetch_sub(bytes.len() as u64,Relaxed);
                    if !matches!(result,Ok(Ok(()))) {stats.output_lost.fetch_add(bytes.len() as u64,Relaxed);break;}
                },
                Some(Output::StartCompression)=>{
                    if !matches!(tokio::time::timeout(Duration::from_millis(write_timeout_ms),writer.start(&mut write,&stats)).await,Ok(Ok(()))){break;}
                },
                Some(Output::Close) | None => {
                    let _=tokio::time::timeout(Duration::from_millis(write_timeout_ms),writer.finish(&mut write,&stats)).await;
                    break;
                },
            }
        }
    }
    output.close();
    while let Some(message) = output.recv().await {
        if let Output::Bytes(bytes) = message {
            stats.output_pending.fetch_sub(bytes.len() as u64, Relaxed);
            stats.output_lost.fetch_add(bytes.len() as u64, Relaxed);
        }
    }
    let _ = write.shutdown().await;
    let _ = events.send(Event::Gone(id)).await;
}
impl Server {
    /// Validate and save before accepting command shutdown; signals proceed on failure.
    async fn request_shutdown(&mut self, request: ShutdownRequest) {
        if self.shutdown.is_some() {
            return;
        }
        let snapshot = self.scripts.world.borrow().clone();
        let result = match snapshot.validate(&self.config) {
            Ok(()) => {
                persistence::persist(
                    self.config.database(),
                    snapshot,
                    self.config.database.busy_timeout_ms,
                )
                .await
            }
            Err(e) => Err(e),
        };
        if let Err(e) = result {
            eprintln!("Shutdown initial save failed: {e:#}");
            if let ShutdownRequest::Player(player) = request {
                for (id, session) in &self.sessions {
                    if session.player == Some(player) {
                        self.tell(*id, "Shutdown cancelled: unable to save the database.\r\n");
                    }
                }
                return;
            }
            self.shutdown_failed = true;
        }
        self.shutdown = Some(request);
        eprintln!("Graceful shutdown: {request:?}");
        if let ShutdownRequest::Player(player) = request {
            let name = self.scripts.world.borrow().objects[&player].name.clone();
            for id in self.sessions.keys() {
                self.tell(*id, &format!("Game: Shutdown by {name}\r\n"));
            }
        }
    }
    /// The sole shutdown cleanup path, after acceptance and before task draining.
    async fn finish_shutdown(&mut self) {
        for id in self.sessions.keys().copied().collect::<Vec<_>>() {
            if let Err(e) = self.disconnect(id).await {
                eprintln!("Shutdown disconnect failed: {e:#}");
                self.shutdown_failed = true;
            }
        }
        let snapshot = self.scripts.world.borrow().clone();
        let result = match snapshot.validate(&self.config) {
            Ok(()) => {
                persistence::persist(
                    self.config.database(),
                    snapshot,
                    self.config.database.busy_timeout_ms,
                )
                .await
            }
            Err(e) => Err(e),
        };
        if let Err(e) = result {
            eprintln!("Shutdown final save failed: {e:#}");
            self.shutdown_failed = true;
        }
    }

    /// MSSP snapshots are generated by the world owner at confirmed enablement.
    fn mssp(&self, id: SessionId) {
        let fields = [
            ("NAME", self.config.server.mud_name.clone()),
            (
                "PLAYERS",
                self.sessions
                    .values()
                    .filter(|s| s.player.is_some())
                    .count()
                    .to_string(),
            ),
            ("UPTIME", self.started_at.to_string()),
            ("CODEBASE", env!("CARGO_PKG_NAME").into()),
            ("PORT", self.listen_port.to_string()),
        ];
        let mut payload = Vec::new();
        for (name, value) in fields {
            payload.push(1);
            payload.extend(name.bytes());
            payload.push(2);
            payload.extend(
                value
                    .chars()
                    .filter(|c| !c.is_control())
                    .collect::<String>()
                    .bytes(),
            );
        }
        if let Some(session) = self.sessions.get(&id) {
            session.protocol(vec![telnet::Decoder::sub_reply(telnet::MSSP, &payload)]);
        }
    }
    /// Color overrides never affect another connection or persistent account state.
    fn color(&self, id: SessionId, mode: &str) {
        use crate::text::ColorDepth;
        let Some(session) = self.sessions.get(&id) else {
            return;
        };
        let mode = mode.trim().to_ascii_lowercase();
        let reply = if mode.is_empty() {
            format!(
                "Color mode: {}{}. Client capability: {}{}.",
                session
                    .color_override
                    .get()
                    .map_or("auto", ColorDepth::name),
                if session.color_override.get().is_some() {
                    " (override)"
                } else {
                    ""
                },
                ColorDepth::advertised(session.decoder.color_depth).name(),
                if session.decoder.screen_reader {
                    ", screen reader"
                } else {
                    ""
                }
            )
        } else {
            let selected = match mode.as_str() {
                "auto" => None,
                "off" => Some(ColorDepth::None),
                "16" => Some(ColorDepth::Ansi16),
                "256" => Some(ColorDepth::Ansi256),
                "truecolor" => Some(ColorDepth::Truecolor),
                _ => {
                    self.tell(id, "Use color auto, off, 16, 256, or truecolor.\r\n");
                    return;
                }
            };
            session.color_override.set(selected);
            format!(
                "Color mode set to {}.",
                selected.map_or("auto", ColorDepth::name)
            )
        };
        self.tell(id, &format!("{reply}\r\n"));
    }

    /// Read-only, invoking-session-only connection statistics in stable session-ID order.
    fn session_diagnostics(&self, id: SessionId, prefix: &str) {
        let mut report = telnet::diagnostics::Report::new(self.config.runtime.output_message_limit);
        let headers = [
            "Player Name",
            "On For",
            "Idle",
            "Session",
            "In Pend",
            "In Lost",
            "In Total",
            "Out Pend",
            "Out Lost",
            "Out Total",
        ];
        let mut rows = Vec::new();
        let world = self.scripts.world.borrow();
        let mut count = 0;
        for (sid, session) in &self.sessions {
            let Some(player) = session.player else {
                continue;
            };
            let name = crate::text::plain_with(&self.scripts.palette, &world.objects[&player].name);
            if !name
                .to_lowercase()
                .starts_with(&prefix.trim().to_lowercase())
            {
                continue;
            }
            count += 1;
            let mut stats = session.stats.snapshot();
            stats.input[0] += session.decoder.pending_text() as u64;
            rows.push([
                telnet::diagnostics::escape(crate::text::literal_prefix(&name, 16).as_bytes()),
                telnet::diagnostics::connected_time(session.connected.elapsed().as_secs()),
                telnet::diagnostics::idle_time(session.active.elapsed().as_secs()),
                sid.0.to_string(),
                stats.input[0].to_string(),
                stats.input[1].to_string(),
                stats.input[2].to_string(),
                stats.output[0].to_string(),
                stats.output[1].to_string(),
                stats.output[2].to_string(),
            ]);
        }
        // Size columns from this snapshot so large counters cannot shift later columns.
        let widths: [usize; 10] = std::array::from_fn(|column| {
            rows.iter()
                .map(|row| row[column].len())
                .chain(std::iter::once(headers[column].len()))
                .max()
                .unwrap()
        });
        let format_row = |row: [&str; 10]| {
            row.iter()
                .enumerate()
                .map(|(column, value)| {
                    let width = widths[column];
                    if column == 0 {
                        format!("{value:<width$}")
                    } else {
                        format!("{value:>width$}")
                    }
                })
                .collect::<Vec<_>>()
                .join("  ")
        };
        report.line(&format_row(headers));
        for row in &rows {
            report.line(&format_row(std::array::from_fn(|column| {
                row[column].as_str()
            })));
            if report.full() {
                break;
            }
        }
        report.line(&format!(
            "{count} Player{} logged in, {} record, {} maximum.",
            if count == 1 { "" } else { "s" },
            world.record_players,
            if self.config.mux.max_players < 0 {
                "no".into()
            } else {
                self.config.mux.max_players.to_string()
            }
        ));
        if let Some(session) = self.sessions.get(&id) {
            session.raw(report.finish());
        }
    }
    /// Player lookup follows login identity rules; no object visibility filter hides Wizard diagnostics.
    fn telnet_diagnostics(&self, id: SessionId, name: &str) {
        let mut report = telnet::diagnostics::Report::new(self.config.runtime.output_message_limit);
        let world = self.scripts.world.borrow();
        if name.is_empty() {
            report.line("Usage: @telnet <player>");
        } else if let Some(player) = world.find_player(name) {
            let mut found = false;
            for (sid, session) in &self.sessions {
                if session.player == Some(player) {
                    found = true;
                    let ansi = world.objects[&player]
                        .flags
                        .contains(crate::flags::Flag::Ansi);
                    let options = session.render_options(ansi);
                    report.line(&format!(
                        "Color effective: {}; override: {}; OSC capabilities: {}",
                        options.color.name(),
                        session
                            .color_override
                            .get()
                            .map_or("auto", crate::text::ColorDepth::name),
                        options
                            .capabilities
                            .iter()
                            .map(|suffix| {
                                if suffix.is_empty() {
                                    "OSC_HYPERLINKS".to_owned()
                                } else {
                                    format!("OSC_HYPERLINKS_{suffix}")
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                    telnet::diagnostics::telnet(
                        &mut report,
                        &world.objects[&player].name,
                        player.0,
                        sid.0,
                        &session.decoder,
                        &session.stats.snapshot(),
                    );
                    if report.full() {
                        break;
                    }
                }
            }
            if !found {
                report.line("That player is not connected.");
            }
        } else {
            report.line("No such player.");
        }
        if let Some(session) = self.sessions.get(&id) {
            session.raw(report.finish());
        }
    }
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
            let negotiation = s.decoder.echo(secret);
            s.protocol(negotiation);
        }
        self.tell(id, text);
    }
    fn snapshots(&self) -> Result<()> {
        let w = self.scripts.world.borrow();
        let mut hidden = 0;
        let list:Vec<_>=self.sessions.iter().filter_map(|(session_id,session)| {
            let id=session.player?;
            let object=&w.objects[&id];
            if object.flags.contains(crate::flags::Flag::Dark) { hidden+=1; return None; }
            Some(serde_json::json!({"name":object.name,"dbref":id.0,"session":session_id.0,"terminal_width":session.decoder.width,"connected_for":session.connected.elapsed().as_secs(),"idle_for":session.active.elapsed().as_secs()}))
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
            if self.shutdown.is_some() {
                self.shutdown_failed = true;
            }
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
                    let ansi = self.scripts.world.borrow().objects[&p]
                        .flags
                        .contains(crate::flags::Flag::Ansi);
                    if !s.document(&text, ansi, true) {
                        self.sessions[id].close();
                    }
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
                    .communication(&self.config)
                    .presence(p, false, None)
                    .and_then(|()| {
                        self.scripts
                            .event("on_player_disconnect", Some(p), Some(id.0))
                    })
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
        for &byte in bytes {
            if self.shutdown.is_some() {
                break;
            }
            let Some(session) = self.sessions.get_mut(&id) else {
                break;
            };
            session.stats.consumed(1);
            let decoded = session.decoder.feed_byte(byte);
            session.stats.input_lost.fetch_add(
                session.decoder.take_discarded(),
                std::sync::atomic::Ordering::Relaxed,
            );
            let inputs = match decoded {
                Ok(v) => v,
                Err(_) => {
                    self.tell(id, "Input limit exceeded.\r\n");
                    self.disconnect(id).await?;
                    return Ok(());
                }
            };
            for input in inputs {
                match input {
                    Input::Negotiated(_) => {}
                    Input::StartCompression => {
                        self.sessions[&id].protocol(vec![Input::StartCompression])
                    }
                    Input::Diagnostic(message) => eprintln!("Telnet session {}: {message}", id.0),
                    Input::StatusRequest => self.mssp(id),
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
                            let credits = periods
                                .saturating_mul(self.config.mux.command_quota_increment as u128);
                            s.quota = (s.quota as u128 + credits)
                                .min(self.config.mux.command_quota_max as u128)
                                as usize;
                            // Preserve the partial refill interval between commands.
                            s.quota_at =
                                Instant::now() - Duration::from_millis((elapsed % interval) as u64);
                        }
                        if s.quota == 0 {
                            s.stats
                                .input_lost
                                .fetch_add(line.len() as u64, std::sync::atomic::Ordering::Relaxed);
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
    /// Stage repair callbacks under the database transaction, detaching destroyed players only after commit.
    async fn dbck(&mut self, session: SessionId, actor: ObjectId) {
        let before = self.scripts.world.borrow().clone();
        let result = persistence::repair(
            &self.config.database(),
            self.config.database.busy_timeout_ms,
            |raw| {
                let (repaired, mut report) = crate::dbck::plan(&before, raw, &self.config)?;
                *self.scripts.world.borrow_mut() = repaired;
                for relocation in &report.plan.relocations {
                    let callable = |id: ObjectId| {
                        self.scripts
                            .world
                            .borrow()
                            .objects
                            .get(&id)
                            .is_some_and(|o| {
                                matches!(o.kind, Kind::Room | Kind::Player | Kind::Thing)
                            })
                    };
                    let movement = crate::movement::Move {
                        actor,
                        object: relocation.object,
                        source: relocation.source,
                        destination: relocation.destination,
                        session: self
                            .sessions
                            .iter()
                            .find(|(_, s)| s.player == Some(relocation.object))
                            .map(|(id, _)| id.0),
                    };
                    if let Some(source) = relocation.source.filter(|id| callable(*id)) {
                        self.scripts
                            .world
                            .borrow_mut()
                            .objects
                            .get_mut(&relocation.object)
                            .unwrap()
                            .location = Some(source);
                        self.scripts.movement_event("on_exit", source, &movement)?;
                    }
                    self.scripts
                        .world
                        .borrow_mut()
                        .objects
                        .get_mut(&relocation.object)
                        .context("callback removed repaired occupant")?
                        .location = Some(relocation.destination);
                    self.scripts
                        .movement_event("on_enter", relocation.destination, &movement)?;
                }
                let after = self.scripts.world.borrow().clone();
                after.validate(&self.config)?;
                for id in &report.plan.purges {
                    let o = after
                        .objects
                        .get(id)
                        .context("callback removed tombstone")?;
                    anyhow::ensure!(
                        o.kind == Kind::Garbage
                            && o.flags == [crate::flags::Flag::Going].into_iter().collect()
                            && o.powers == Default::default()
                            && o.state.is_empty()
                            && !after.accounts.contains_key(id),
                        "callback changed purged object #{}",
                        id.0
                    );
                }
                report.plan.links = crate::dbck::rebuild_links(&after, &report.plan.links);
                report.plan.list_changes = report
                    .plan
                    .links
                    .iter()
                    .filter(|(id, links)| raw.get(id) != Some(*links))
                    .map(|(id, _)| *id)
                    .collect();
                Ok((after, report))
            },
        )
        .await;
        match result {
            Ok(report) => {
                for finding in &report.findings {
                    eprintln!("DBCK: {finding}");
                }
                for id in self
                    .sessions
                    .iter()
                    .filter(|(_, s)| {
                        s.player
                            .is_some_and(|p| report.plan.detachments.contains(&p))
                    })
                    .map(|(id, _)| *id)
                    .collect::<Vec<_>>()
                {
                    self.tell(id, "You have been destroyed!\r\n");
                    if let Some(session) = self.sessions.remove(&id) {
                        session.close();
                    }
                }
                self.reconcile_connections();
                self.flush();
                if let Some(session) = self.sessions.get(&session) {
                    session.raw(report.response(self.config.runtime.output_message_limit));
                }
            }
            Err(e) => {
                *self.scripts.world.borrow_mut() = before;
                self.reconcile_connections();
                self.scripts.outbox.borrow_mut().clear();
                eprintln!("DBCK rolled back: {e:#}");
                self.tell(
                    session,
                    "Database check failed; no repairs committed. See server diagnostics.\r\n",
                );
            }
        }
    }
    async fn command(&mut self, id: SessionId, p: ObjectId, line: &str) -> Result<()> {
        self.snapshots()?;
        let before = self.scripts.world.borrow().clone();
        match commands::run(&self.scripts, &self.config, p, id.0, line) {
            Ok(Action::Color(mode)) => self.color(id, &mode),
            Ok(Action::Help(topic)) => {
                let wizard = p.0 == 1
                    || self.scripts.world.borrow().objects[&p]
                        .flags
                        .contains(crate::flags::Flag::Wizard);
                let index = self.scripts.help.clone();
                let response =
                    tokio::task::spawn_blocking(move || index.lookup(&topic, wizard)).await;
                match response {
                    Ok(Ok(response)) => {
                        let ansi = self.scripts.world.borrow().objects[&p]
                            .flags
                            .contains(crate::flags::Flag::Ansi);
                        if let Some(session) = self.sessions.get(&id)
                            && let Err(error) = session.help(&response, ansi, &self.config).await
                        {
                            eprintln!("Help rendering: {error:#}");
                            self.tell(
                                id,
                                "Unable to render help article. See server diagnostics.\r\n",
                            );
                        }
                    }
                    error => {
                        eprintln!("Help read: {error:?}");
                        self.tell(
                            id,
                            "Unable to render help article. See server diagnostics.\r\n",
                        );
                    }
                }
            }
            Ok(Action::HelpReload) => {
                let config = self.config.clone();
                match tokio::task::spawn_blocking(move || crate::help::HelpIndex::reload(&config))
                    .await
                {
                    Ok(Ok(index)) => {
                        index.report.log();
                        let report = &index.report;
                        let mut lines: Vec<_> = report
                            .errors
                            .iter()
                            .chain(&report.warnings)
                            .cloned()
                            .collect();
                        lines.push(report.summary());
                        let response = crate::help::HelpResponse::Message(lines.join("\n"));
                        self.scripts.help = index;
                        if let Some(session) = self.sessions.get(&id)
                            && let Err(error) = session.help(&response, false, &self.config).await
                        {
                            eprintln!("Help reload diagnostics: {error:#}");
                            self.tell(
                                id,
                                "Help reindexed; see server diagnostics for details.\r\n",
                            );
                        }
                    }
                    error => {
                        eprintln!("Help reload failed: {error:?}");
                        self.tell(id, "Help reload failed; previous index retained. See server diagnostics.\r\n");
                    }
                }
            }
            Ok(Action::Sessions(prefix)) => self.session_diagnostics(id, &prefix),
            Ok(Action::Telnet(player)) => self.telnet_diagnostics(id, &player),
            Ok(Action::Shutdown) => self.request_shutdown(ShutdownRequest::Player(p)).await,
            Ok(Action::DbCheck) => self.dbck(id, p).await,
            Ok(Action::Find(request)) => self.find(id, p, request),
            Ok(Action::Reply(text)) => {
                if let Some(session) = self.sessions.get(&id) {
                    session.raw(crate::find::bounded_error(
                        &text,
                        self.config.runtime.output_message_limit,
                    ));
                }
            }
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
        let identity = self.scripts.world.borrow().find_player(&name);
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
                .send(Event::Authenticated(id, name, create, identity, result))
                .await;
        });
    }
    /// Hash jobs retain their original identity; purging or shutdown invalidates their results.
    async fn authentication_result(
        &mut self,
        id: SessionId,
        name: String,
        create: bool,
        identity: Option<ObjectId>,
        result: Result<Option<String>>,
    ) -> Result<()> {
        self.inflight = self.inflight.saturating_sub(1);
        if self.shutdown.is_some() {
            return Ok(());
        }
        if !create && self.scripts.world.borrow().find_player(&name) != identity {
            self.prompt(
                id,
                LoginFlow::Name,
                "Account no longer available.\r\nWho are you? ",
                false,
            );
            return Ok(());
        }
        self.authenticated(id, name, create, result).await
    }
    async fn authenticated(
        &mut self,
        id: SessionId,
        name: String,
        create: bool,
        result: Result<Option<String>>,
    ) -> Result<()> {
        if self.shutdown.is_some() {
            return Ok(());
        }
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
                    persistence::trim_history(
                        &mut a.history,
                        self.config.security.login_history_limit,
                    );
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
        if create && !self.config.mux.public_channel.is_empty() {
            let service = self.scripts.communication(&self.config);
            if service.name(&self.config.mux.public_channel).is_ok()
                && let Err(error) =
                    service.add(p, &self.config.mux.public_channel, "pub", true, true)
            {
                *self.scripts.world.borrow_mut() = before;
                self.scripts.outbox.borrow_mut().clear();
                eprintln!("Registration channel: {error:#}");
                self.prompt(
                    id,
                    LoginFlow::Name,
                    "Unable to register.\r\nWho are you? ",
                    false,
                );
                return Ok(());
            }
        }
        {
            let mut w = self.scripts.world.borrow_mut();
            let a = w.accounts.get_mut(&p).unwrap();
            a.last_login = Some(accounts::now());
            a.last_site = Some(host.clone());
            a.successes += 1;
            a.history.push(Login {
                success: true,
                at: accounts::now(),
                host: host.clone(),
            });
            persistence::trim_history(&mut a.history, self.config.security.login_history_limit);
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
        let presence = if reconnect {
            Ok(())
        } else {
            self.scripts
                .communication(&self.config)
                .presence(p, true, Some(&host))
        };
        if let Err(e) = presence.and_then(|()| {
            self.scripts
                .lifecycle("on_player_connect", Some(p), Some(id.0), reconnect, "")
        }) {
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
            started_at: accounts::now(),
            listen_port: 0,
            shutdown: None,
            shutdown_failed: false,
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
    /// Late hashes cannot attach to a replacement identity, and all shutdown origins reject authentication.
    #[tokio::test(flavor = "current_thread")]
    async fn destroyed_identity_and_shutdown_invalidate_hash_results() {
        let mut c = Config::load(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"),
        )
        .unwrap();
        let mut world = persistence::load(&c.database()).await.unwrap();
        world.accounts.remove(&ObjectId(2));
        world.objects.get_mut(&ObjectId(2)).unwrap().kind = Kind::Garbage;
        let replacement = world.create(&c, "Wizard".into(), Kind::Player);
        world.accounts.insert(replacement, Account::default());
        let scripts = Scripts::new(&c, Rc::new(RefCell::new(world))).unwrap();
        // An accidental write in this regression can only target an isolated missing database.
        let d = tempfile::tempdir().unwrap();
        c.root = d.path().into();
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
            inflight: 1,
            started_at: accounts::now(),
            listen_port: 0,
            shutdown: None,
            shutdown_failed: false,
        };
        let (output, mut receiver) = mpsc::channel(16);
        let now = Instant::now();
        server.sessions.insert(
            SessionId(1),
            Session {
                output,
                stats: Default::default(),
                palette: Default::default(),
                color_override: Default::default(),
                presets_emitted: Default::default(),
                peer: "127.0.0.1".parse().unwrap(),
                player: None,
                flow: LoginFlow::Pending,
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
        server
            .authentication_result(
                SessionId(1),
                "Wizard".into(),
                false,
                Some(ObjectId(2)),
                Ok(None),
            )
            .await
            .unwrap();
        assert!(server.sessions[&SessionId(1)].player.is_none());
        assert!(matches!(
            server.sessions[&SessionId(1)].flow,
            LoginFlow::Name
        ));
        let mut text = Vec::new();
        while let Ok(output) = receiver.try_recv() {
            if let Output::Bytes(bytes) = output {
                text.extend(bytes);
            }
        }
        assert!(String::from_utf8_lossy(&text).contains("Account no longer available"));
        assert_eq!(
            server.scripts.world.borrow().accounts[&replacement].successes,
            0
        );
        for request in [
            ShutdownRequest::Sigint,
            ShutdownRequest::Sigterm,
            ShutdownRequest::Player(ObjectId(1)),
        ] {
            server.shutdown = Some(request);
            server.sessions.get_mut(&SessionId(1)).unwrap().flow = LoginFlow::Pending;
            server
                .authentication_result(
                    SessionId(1),
                    "LateRegistration".into(),
                    true,
                    None,
                    Ok(Some("unused".into())),
                )
                .await
                .unwrap();
            assert!(
                server
                    .scripts
                    .world
                    .borrow()
                    .find_player("LateRegistration")
                    .is_none()
            );
            assert!(matches!(
                server.sessions[&SessionId(1)].flow,
                LoginFlow::Pending
            ));
            server.request_shutdown(ShutdownRequest::Sigterm).await;
            assert_eq!(server.shutdown, Some(request));
            assert!(!server.shutdown_failed); // repeated requests did not attempt another save
        }
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
            persistence::load(&c.database()).await.unwrap();
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
                started_at: accounts::now(),
                listen_port: 0,
                shutdown: None,
                shutdown_failed: false,
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
                        stats: Default::default(),
                        palette: Default::default(),
                        color_override: Default::default(),
                        presets_emitted: Default::default(),
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
            sqlx::raw_sql("CREATE TRIGGER fail BEFORE UPDATE ON snapshot BEGIN SELECT RAISE(FAIL,'injected'); END;").execute(&mut db).await.unwrap();
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
