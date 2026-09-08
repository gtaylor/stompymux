//! Socket transport, session input, cached connection messages, and lifecycle coordination.
use super::*;

pub(super) async fn connection(
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
    pub(super) fn tell(&self, id: SessionId, s: &str) {
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
    pub(super) fn prompt(&mut self, id: SessionId, flow: LoginFlow, text: &str, secret: bool) {
        if let Some(s) = self.sessions.get_mut(&id) {
            s.flow = flow;
            let negotiation = s.decoder.echo(secret);
            s.protocol(negotiation, &self.config);
        }
        self.tell(id, text);
    }
    pub(super) fn snapshots(&self) -> Result<()> {
        self.snapshots_for(&self.scripts)
    }
    pub(super) fn snapshots_for(&self, scripts: &Scripts) -> Result<()> {
        scripts
            .queue_enabled
            .set(self.controls.enabled(crate::controls::Control::Queueing));
        let w = scripts.world.borrow();
        let mut hidden = 0;
        let players = self
            .sessions
            .iter()
            .filter_map(|(session_id, session)| {
                let id = session.player?;
                let object = &w.objects[&id];
                if object.flags.contains(crate::flags::Flag::Dark) {
                    hidden += 1;
                    return None;
                }
                Some(crate::lua::sessions::Player {
                    name: object.name.clone(),
                    dbref: id.0,
                    session: session_id.0,
                    terminal_width: session.decoder.width,
                    connected_for: session.connected.elapsed().as_secs(),
                    idle_for: session.active.elapsed().as_secs(),
                })
            })
            .collect();
        scripts.lua.set_app_data(crate::lua::sessions::Sessions {
            players,
            hidden,
            record: w.record_players as i64,
            maximum: (self.config.mux.max_players != -1).then_some(self.config.mux.max_players),
            environments: self
                .sessions
                .iter()
                .map(|(id, s)| (id.0, s.decoder.environment.clone()))
                .collect(),
        });
        Ok(())
    }
    /// Session state is authoritative even when durable world mutations roll back.
    pub(super) fn reconcile_connections(&self) {
        self.scripts.flows.sessions(
            self.sessions
                .iter()
                .filter_map(|(id, session)| {
                    let player = session.player?;
                    let world = self.scripts.world.borrow();
                    let object = world.objects.get(&player)?;
                    (object.kind == Kind::Player).then_some((
                        id.0,
                        crate::runtime::transaction::FlowIdentity {
                            player,
                            generation: object.generation,
                        },
                    ))
                })
                .collect(),
        );
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
    pub(super) fn flow_output(&self, output: crate::runtime::PrivateOutput) {
        if let Some(session) = self.sessions.get(&SessionId(output.session))
            && let Some(player) = session.player
        {
            let ansi = self
                .scripts
                .world
                .borrow()
                .objects
                .get(&player)
                .is_some_and(|o| o.flags.contains(crate::flags::Flag::Ansi));
            if !session.document(&output.document, ansi, false) {
                session.close();
            }
        }
    }
    pub(super) async fn disconnect(&mut self, id: SessionId) -> Result<()> {
        if let Some(s) = self.sessions.remove(&id) {
            let stats = s.stats.snapshot();
            self.config.log(
                &[crate::logging::Category::Accounting],
                "NET",
                "DISC",
                format!(
                    "Session {} player {:?} peer {} duration {}s input {} output {} wire {}",
                    id.0,
                    s.player,
                    s.peer,
                    s.connected.elapsed().as_secs(),
                    stats.input[2],
                    stats.output[2],
                    stats.wire_output
                ),
            );
            s.close();
            self.reconcile_connections();
            if let Some(p) = s.player {
                let partial = self.sessions.values().any(|s| s.player == Some(p));
                let transition = self.transition(
                    p,
                    if partial {
                        TransitionKind::PartialDisconnect
                    } else {
                        TransitionKind::Disconnected
                    },
                    s.peer,
                    s.site,
                );
                if partial {
                    self.announce_transition(transition).await;
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
                    self.config.log(
                        &[crate::logging::Category::Problems],
                        "SRV",
                        "ERROR",
                        format!("Disconnect hook: {e:#}"),
                    );
                    *self.scripts.world.borrow_mut() = before.clone();
                    self.reconcile_connections();
                    self.scripts.effects.rollback();
                }
                self.commit(before).await;
                self.flush();
                self.announce_transition(transition).await;
            }
        }
        Ok(())
    }
    pub(super) async fn input(&mut self, id: SessionId, bytes: &[u8]) -> Result<()> {
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
                        self.sessions[&id].protocol(vec![Input::StartCompression], &self.config)
                    }
                    Input::Problem(secondary, message) => self.config.log(
                        &[crate::logging::Category::Problems],
                        "TELNET",
                        secondary,
                        format!("Telnet session {}: {message}", id.0),
                    ),
                    Input::StatusRequest => self.mssp(id),
                    Input::Reply(v) => {
                        self.sessions[&id].raw(v);
                    }
                    Input::InvalidUtf8 => self.tell(id, "Invalid UTF-8 input.\r\n"),
                    Input::Line(line) => {
                        let line = Zeroizing::new(line);
                        let s = self.sessions.get_mut(&id).unwrap();
                        let keepalive = s.player.is_some()
                            && !self.scripts.flows.active(id.0)
                            && line.eq_ignore_ascii_case("IDLE");
                        if !keepalive {
                            s.active = Instant::now();
                        }
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
                        if keepalive {
                            continue;
                        }
                        if let Some(p) = s.player {
                            if self.scripts.flows.active(id.0) {
                                self.flow_input(id, &line).await;
                            } else {
                                let _ = s.stats.commands.fetch_update(
                                    std::sync::atomic::Ordering::Relaxed,
                                    std::sync::atomic::Ordering::Relaxed,
                                    |n| Some(n.saturating_add(1)),
                                );
                                self.command(id, p, &line).await?;
                            }
                        } else {
                            self.login(id, &line).await?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
    /// Publish successful file replacements together; retain last-good content on failures.
    pub(super) async fn readcache(&mut self, session: Option<SessionId>, actor: ObjectId) {
        let previous = self.message_cache.clone();
        let config = self.config.clone();
        let response = match tokio::task::spawn_blocking(move || previous.reload(&config)).await {
            Ok((cache, report)) => {
                self.message_cache = cache;
                for row in &report {
                    self.config.log(
                        &[crate::logging::Category::Startup],
                        "INI",
                        "INFO",
                        format!("File cache: {row}"),
                    );
                }
                format!("File sizes: {}", report.join("  "))
            }
            Err(error) => format!("File cache unchanged: {error}"),
        };
        if let Some(id) = session {
            self.inspection_report(id, response).await;
        } else {
            self.queue_reply(ReplyDestination::Object(actor), &response);
            self.flush();
        }
    }
    /// Render cached text and then close through the common connection lifecycle.
    pub(super) async fn cache_close(
        &mut self,
        id: SessionId,
        file: crate::message_cache::File,
        extra: &str,
        fallback: &str,
    ) -> Result<()> {
        let cached = self.message_cache.text(file);
        let message = if cached.is_empty() && extra.is_empty() {
            fallback.to_string()
        } else {
            format!(
                "{cached}{}{}",
                if !cached.is_empty() && !cached.ends_with('\n') {
                    "\n"
                } else {
                    ""
                },
                extra
            )
        };
        if let Some(session) = self.sessions.get_mut(&id) {
            let echo = session.decoder.echo(false);
            session.protocol(echo, &self.config);
            let ansi = session.player.is_none_or(|p| {
                self.scripts
                    .world
                    .borrow()
                    .objects
                    .get(&p)
                    .is_some_and(|o| o.flags.contains(crate::flags::Flag::Ansi))
            });
            if let Err(error) = session.styled_report(&message, ansi, &self.config).await {
                self.config.log(
                    &[crate::logging::Category::Problems],
                    "SRV",
                    "ERROR",
                    format!("Closing message: {error:#}"),
                );
                session.raw(crate::telnet::bounded_error(
                    fallback,
                    self.config.runtime.output_message_limit,
                ));
            }
        }
        self.disconnect(id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
                authentication: authentication::State::with_hash_capacity(1, Instant::now(), 0),
                started_at: crate::clock::wall_time(),
                listen_port: 0,
                shutdown: None,
                shutdown_failed: false,
                command_queue: Default::default(),
                cleaning: Default::default(),
                controls: Default::default(),
                idle_recheck: false,
                message_cache: Default::default(),
            };
            let mut receivers = Vec::new();
            for id in [1, 2] {
                let (output, receiver) = mpsc::channel(16);
                receivers.push(receiver);
                let now = Instant::now();
                server.sessions.insert(
                    SessionId(id),
                    Session {
                        retry_remaining: 3,
                        output,
                        stats: Default::default(),
                        palette: Default::default(),
                        color_override: Default::default(),
                        presets_emitted: Default::default(),
                        peer: "127.0.0.1".parse().unwrap(),
                        site: Default::default(),
                        player: Some(ObjectId(1)),
                        flow: LoginFlow::Name,
                        connected: now,
                        active: now,
                        decoder: Default::default(),
                        quota: 1,
                        quota_at: now,
                        failed: Default::default(),
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
