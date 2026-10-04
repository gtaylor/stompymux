//! Serialized server event loop and runtime task coordination.
use super::*;

pub async fn run(
    c: Config,
    scripts: Scripts,
    listener: TcpListener,
    shutdown: impl Future<Output = ShutdownRequest>,
) -> Result<()> {
    run_with_schedule_clock(c, scripts, listener, shutdown, crate::clock::wall_time).await
}

/// Run with an injected schedule-only UTC clock for deterministic embedding and TCP tests.
/// Connection timeouts and authentication continue to use their normal clocks.
pub async fn run_with_schedule_clock(
    c: Config,
    scripts: Scripts,
    listener: TcpListener,
    shutdown: impl Future<Output = ShutdownRequest>,
    schedule_now: impl Fn() -> i64,
) -> Result<()> {
    run_with_clocks(
        c,
        scripts,
        listener,
        shutdown,
        schedule_now,
        tokio::time::Instant::now,
    )
    .await
}

/// Inject independent UTC schedule and monotonic cleaning clocks; socket clocks stay real.
pub async fn run_with_clocks(
    c: Config,
    scripts: Scripts,
    listener: TcpListener,
    shutdown: impl Future<Output = ShutdownRequest>,
    schedule_now: impl Fn() -> i64,
    cleaning_now: impl Fn() -> tokio::time::Instant,
) -> Result<()> {
    c.validate_for_serve()?;
    c.site_policy
        .validate_listener(listener.local_addr()?.ip())?;
    for warning in c.warnings.iter().chain(&scripts.warnings) {
        tracing::warn!("{warning}");
    }
    println!("Listening on {}", listener.local_addr()?);
    let (tx, mut rx) = mpsc::channel(c.runtime.event_queue_capacity);
    let mut server = Server {
        config: c,
        scripts,
        sessions: BTreeMap::new(),
        events: tx.clone(),
        authentication: authentication::State::new(),
        started_at: crate::clock::wall_time(),
        listen_port: 0,
        shutdown: None,
        shutdown_failed: false,
        command_queue: Default::default(),
        cleaning: Default::default(),
        controls: Default::default(),
        idle_recheck: false,
        message_cache: Default::default(),
        durable: None,
        database: None,
    };
    server.cleaning = crate::cleaning::Cleaning::new(
        cleaning_now(),
        server.config.mux.check_interval as u64,
        server.config.mux.check_offset as u64,
    );
    server.listen_port = listener.local_addr()?.port();
    let mut next = 0;
    let mut tick = tokio::time::interval(Duration::from_millis(
        server.config.runtime.maintenance_interval_ms,
    ));
    let mut btech_tick = tokio::time::interval_at(
        tokio::time::Instant::now() + Duration::from_secs(1),
        Duration::from_secs(1),
    );
    btech_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut tasks = tokio::task::JoinSet::new();
    let mut idle_deadline = tokio::time::Instant::now();
    tokio::pin!(shutdown);
    let cache_config = server.config.clone();
    let (cache, report) = tokio::task::spawn_blocking(move || {
        crate::message_cache::MessageCache::default().reload(&cache_config)
    })
    .await?;
    server.message_cache = cache;
    for diagnostic in report {
        tracing::debug!("file cache: {diagnostic}");
    }
    let mut runtime_sources = server.scripts.sources.clone();
    let mut schedules = crate::lua::schedules::Queue::default();
    schedules.observe(
        &server.scripts.schedules,
        &server.scripts.world.borrow(),
        schedule_now(),
    );
    let mut queue_credit = 0usize;
    loop {
        let now = tokio::time::Instant::now();
        let queue_ready = queue_credit > 0 && server.command_queue.ready(now);
        let deadline = server.command_queue.next_wakeup(now);
        let ready = schedules.ready(schedule_now());
        tokio::select! {
            _ = std::future::ready(()), if queue_ready => {
                queue_credit -= 1;
                let work = server.command_queue.take(tokio::time::Instant::now(), &server.scripts.world.borrow(), server.config.mux.space_compress);
                if let Some(work) = work { server.queued(work).await; }
                tokio::task::yield_now().await;
            },
            _ = async { if let Some(due) = deadline { tokio::time::sleep_until(due).await; } else { std::future::pending::<()>().await; } } => {
                queue_credit = server.config.mux.command_queue_idle_chunk as usize;
            },
            _ = std::future::ready(()), if ready => {
                if let Some(job)=schedules.take_due(schedule_now()) { server.scheduled(job).await; }
                tokio::task::yield_now().await;
            },
            request = &mut shutdown => { server.request_shutdown(request).await; },
            accepted = listener.accept() => {
                let (stream,peer)=accepted?;
                if tasks.len()>=server.config.runtime.max_connections { drop(stream); continue; }
                let site = server.config.site_policy.classify(peer.ip());
                if site.forbidden {
                    tracing::warn!(%peer, "connection refused: forbidden site");
                    tasks.spawn(presence::reject_site(stream, server.message_cache.text(crate::message_cache::File::BadSite).to_owned(), server.scripts.palette.clone(), server.config.clone()));
                    continue;
                }
                next+=1;
                let id=SessionId(next);
                tracing::info!(session = id.0, %peer, "connection accepted");
                let (output,receiver)=mpsc::channel(server.config.runtime.session_output_queue_capacity);
                let now=tokio::time::Instant::now();
                let stats=std::sync::Arc::new(telnet::transport::Stats::default());
                server.sessions.insert(id,Session { retry_remaining: server.config.mux.retry_limit,
                    palette:server.scripts.palette.clone(), color_override:Default::default(), presets_emitted:Default::default(),
                    stats:stats.clone(),output, site, peer:peer.ip(), player:None, flow:LoginFlow::Name,
                    connected:now, active:now, decoder:telnet::Decoder::new(&server.config.runtime),
                    output_message_limit:server.config.runtime.output_message_limit,
                    quota:server.config.mux.command_quota_increment.min(server.config.mux.command_quota_max),
                    quota_at:now, failed:Default::default(),
                });
                tasks.spawn(connection(stream,id,tx.clone(),receiver,server.config.runtime.write_timeout_ms,stats));
                let session=server.sessions.get_mut(&id).unwrap();
                let negotiation=session.decoder.initial();
                session.protocol(negotiation);
                let index = if server.message_cache.banner_count() == 0 { 0 } else { rand::random_range(0..server.message_cache.banner_count()) };
                let banner = server.message_cache.welcome(index);
                if let Err(error) = session.styled_report(banner, true, &server.config).await { tracing::warn!(error = %format_args!("{error:#}"), "welcome delivery failed"); }
                session.text("Who are you? ",true);
            },
            event = rx.recv() => {
                queue_credit = server.config.mux.command_queue_active_chunk as usize;
                if let Some(event)=event { match event {
                    Event::AdminHashed(job,result) => server.admin_hashed(job,result).await,
                    Event::Gone(id) => server.disconnect(id).await?,
                    Event::Bytes(id,bytes) => server.input(id,&bytes).await?,
                    Event::Authenticated(outcome) => {
                        if outcome.create && outcome.password_length > server.config.security.player_password_length_limit {
                            server.authentication.inflight = server.authentication.inflight.saturating_sub(1);
                            if server.sessions.get(&outcome.session).is_some_and(|s| matches!(s.flow, LoginFlow::Pending)) { server.prompt(outcome.session, LoginFlow::Name, "Password policy changed. Please try again.\r\nWho are you? ", false); }
                        } else { server.authentication_result(outcome.session,outcome.name,outcome.create,outcome.identity,outcome.credential,outcome.result).await?; }
                    }
                }}
            },
            _ = btech_tick.tick(), if server.shutdown.is_none() => { server.btech_tick(schedule_now()).await; },
            _ = tick.tick() => {
                queue_credit = server.config.mux.command_queue_idle_chunk as usize;
                if server.shutdown.is_none() && server.cleaning.take_due(cleaning_now()) { server.dbck(crate::cleaning::CheckOrigin::Automatic).await; }
                if server.idle_recheck { server.idle_recheck = false; server.check_idle().await?; }
                schedules.observe(&server.scripts.schedules,&server.scripts.world.borrow(),schedule_now());
                let timeout=Duration::from_secs(server.config.mux.conn_timeout);
                let idle:Vec<_>=server.sessions.iter()
                    .filter(|(_,s)|(server.controls.enabled(crate::controls::Control::IdleChecking) && s.player.is_none() && s.connected.elapsed()>timeout)||s.output.is_closed()||s.failed.get())
                    .map(|(id,_)|*id).collect();
                for id in idle { if server.sessions.get(&id).is_some_and(|s| s.player.is_none() && s.connected.elapsed()>timeout && !s.failed.get() && !s.output.is_closed()) { server.tell(id,"*** Login Timeout ***\r\n"); } server.disconnect(id).await?; }
                server.authentication.addresses.retain(|_,b|b.at.elapsed()<Duration::from_secs(server.config.security.login_address_retention_seconds));
                server.scripts.record_progress(crate::RuntimeProgress::record_maintenance);
            },
            _ = tokio::time::sleep_until(idle_deadline) => { server.check_idle().await?; idle_deadline = tokio::time::Instant::now() + Duration::from_secs(server.config.mux.idle_interval); },
            _ = tasks.join_next(), if !tasks.is_empty() => {}
        }
        if !std::sync::Arc::ptr_eq(&runtime_sources, &server.scripts.sources) {
            // Old jobs own old VM functions. Cancel before the next loop turn, retaining minute history.
            schedules.clear();
            runtime_sources = server.scripts.sources.clone();
        }
        if server.shutdown.is_some() {
            break;
        }
    }
    server.command_queue = Default::default();
    drop(listener);
    schedules.clear();
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
    if let Err(error) = server.config.logger.shutdown(&server.config).await {
        tracing::error!(error = %format_args!("{error:#}"), "logging shutdown failed");
        server.shutdown_failed = true;
    }
    anyhow::ensure!(
        !server.shutdown_failed,
        "shutdown completed with unsaved changes; see server diagnostics"
    );
    Ok(())
}
