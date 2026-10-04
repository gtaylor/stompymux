//! Login admission, hash throttling, and authentication completion.
use super::*;
use crate::Login;

/// Result of one password hash or verification job, including its original identity snapshot.
pub(super) struct Outcome {
    /// Session that submitted the credential.
    pub(super) session: SessionId,
    /// Submitted or proposed player name.
    pub(super) name: String,
    /// Whether the job is creating a player rather than connecting one.
    pub(super) create: bool,
    /// Player resolved when an existing-account job was submitted.
    pub(super) identity: Option<ObjectId>,
    /// Stored credential observed when an existing-account job was submitted.
    pub(super) credential: Option<String>,
    /// New hash for registration, or successful verification without a hash.
    pub(super) result: Result<Option<String>>,
    /// Submitted byte length used to recheck a changed password policy.
    pub(super) password_length: usize,
}

/// Token bucket measured from its last refill point.
pub(super) struct Bucket {
    /// Currently available operations.
    pub(super) tokens: usize,
    /// Last refill point.
    pub(super) at: Instant,
}

/// All mutable admission and password-hashing coordination owned by the server loop.
pub(super) struct State {
    /// Per-address login attempt buckets.
    pub(super) addresses: BTreeMap<IpAddr, Bucket>,
    /// Global password hash rate bucket.
    pub(super) hashes: Bucket,
    /// Password jobs currently running.
    pub(super) inflight: usize,
    /// Accounts with an administrator password reset job currently running.
    pub(super) pending_resets: std::collections::BTreeSet<ObjectId>,
}

impl State {
    /// Create empty authentication state with the global bucket ready for its first refill.
    pub(super) fn new() -> Self {
        Self::with_hash_capacity(0, Instant::now() - HASH_RATE_WINDOW, 0)
    }

    /// Build state with explicit global capacity for deterministic fixtures.
    pub(super) fn with_hash_capacity(tokens: usize, at: Instant, inflight: usize) -> Self {
        Self {
            addresses: BTreeMap::new(),
            hashes: Bucket { tokens, at },
            inflight,
            pending_resets: Default::default(),
        }
    }
}

impl Server {
    pub(super) fn admission(
        &self,
        privileged: bool,
    ) -> std::result::Result<(), crate::controls::Admission> {
        self.controls.admission(
            self.sessions
                .values()
                .filter(|s| s.player.is_some())
                .count(),
            self.config.mux.max_players,
            privileged,
        )
    }
    pub(super) fn registration_admission(
        &self,
    ) -> std::result::Result<(), crate::controls::Admission> {
        self.controls.registration(
            self.sessions
                .values()
                .filter(|s| s.player.is_some())
                .count(),
            self.config.mux.max_players,
        )
    }
    pub(super) async fn reject_admission(
        &mut self,
        id: SessionId,
        reason: crate::controls::Admission,
    ) -> Result<()> {
        use crate::{controls::Admission, message_cache::File};
        let (file, extra, fallback) = match reason {
            Admission::Down => (
                File::Down,
                self.config.mux.down_message.clone(),
                "Logins are disabled.",
            ),
            Admission::Full => (
                File::Full,
                self.config.mux.full_message.clone(),
                "The game is full.",
            ),
        };
        self.cache_close(id, file, &extra, fallback).await
    }
    pub(super) async fn login(&mut self, id: SessionId, input: &str) -> Result<()> {
        let flow = std::mem::replace(
            &mut self.sessions.get_mut(&id).unwrap().flow,
            LoginFlow::Name,
        );
        match flow {
            LoginFlow::Name => {
                if input.bytes().all(|byte| byte.is_ascii_whitespace()) {
                    self.prompt(id, LoginFlow::Name, "Who are you? ", false);
                } else if self.scripts.world.borrow().find_player(input).is_some() {
                    self.prompt(id, LoginFlow::Password(input.into()), "Password: ", true);
                } else if input.len() > self.config.names.maximum_length {
                    self.prompt(
                        id,
                        LoginFlow::Name,
                        &format!(
                            "New usernames may be at most {} characters long.\r\nWho are you? ",
                            self.config.names.maximum_length
                        ),
                        false,
                    );
                } else if accounts::validate_name_syntax(input, &self.config).is_err() {
                    self.prompt(
                        id,
                        LoginFlow::Name,
                        "New usernames must start with a letter and be at least two characters long.\r\nWho are you? ",
                        false,
                    );
                } else {
                    self.prompt(
                        id,
                        LoginFlow::ConfirmCreate(input.into()),
                        &format!("No character named '{input}' exists. Create a new one? (Y/n) "),
                        false,
                    );
                }
            }
            LoginFlow::Password(name) => {
                self.authenticate(id, name, Zeroizing::new(input.into()), false)
                    .await
            }
            LoginFlow::ConfirmCreate(name) => match input
                .bytes()
                .find(|byte| !byte.is_ascii_whitespace())
                .map(|byte| byte.to_ascii_lowercase())
            {
                None | Some(b'y') => self.prompt(
                    id,
                    LoginFlow::NewPassword(name),
                    "Choose a password: ",
                    true,
                ),
                Some(b'n') => self.prompt(id, LoginFlow::Name, "Who are you? ", false),
                _ => self.prompt(
                    id,
                    LoginFlow::ConfirmCreate(name),
                    "Please answer y or n: ",
                    false,
                ),
            },
            LoginFlow::NewPassword(name) => {
                self.prompt(
                    id,
                    LoginFlow::ConfirmPassword(name, Zeroizing::new(input.into())),
                    "Retype password: ",
                    true,
                );
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
                    self.authenticate(id, name, password, true).await;
                }
            }
            LoginFlow::Pending => {
                self.sessions.get_mut(&id).unwrap().flow = LoginFlow::Pending;
                self.tell(id, "Authentication in progress.\r\n");
            }
        }
        Ok(())
    }
    pub(super) async fn authenticate(
        &mut self,
        id: SessionId,
        name: String,
        password: Zeroizing<String>,
        create: bool,
    ) {
        if create && let Err(reason) = self.registration_admission() {
            if let Err(error) = self.reject_admission(id, reason).await {
                tracing::error!(error = %format_args!("{error:#}"), "admission close failed");
            }
            return;
        }
        let now = Instant::now();
        let burst = self.config.security.login_attempt_burst;
        let refill = self.config.security.login_attempt_refill;
        let address = self.sessions[&id].peer;
        if self.authentication.addresses.len() >= self.config.security.login_address_limit
            && !self.authentication.addresses.contains_key(&address)
        {
            self.prompt(
                id,
                LoginFlow::Name,
                "Login capacity reached. Try later.\r\nWho are you? ",
                false,
            );
            return;
        }
        let bucket = self
            .authentication
            .addresses
            .entry(address)
            .or_insert(Bucket {
                tokens: burst,
                at: now,
            });
        let count = bucket.at.elapsed().as_secs() / refill;
        if count > 0 {
            bucket.tokens = (bucket.tokens + count as usize).min(burst);
            bucket.at = now;
        }
        if self.authentication.hashes.at.elapsed() >= HASH_RATE_WINDOW {
            self.authentication.hashes.tokens = self.config.security.login_hash_limit;
            self.authentication.hashes.at = now;
        }
        if bucket.tokens == 0
            || self.authentication.hashes.tokens == 0
            || self.authentication.inflight >= self.config.security.login_hash_concurrency
        {
            let message = if create {
                "Either there is already a player with that name, or that name is illegal.\r\n"
            } else {
                "Either that player does not exist, or has a different password.\r\n"
            };
            tracing::warn!(target: crate::logging::targets::LOGINS, name = ?name, peer = %address, "authentication throttled");
            if let Err(error) = self
                .cache_close(id, crate::message_cache::File::Connect, message, message)
                .await
            {
                tracing::error!(error = %format_args!("{error:#}"), "authentication throttle close failed");
            }
            return;
        }
        bucket.tokens -= 1;
        self.authentication.hashes.tokens -= 1;
        let password = if create {
            let password = Zeroizing::new(
                password
                    .trim_matches(|character: char| character.is_ascii_whitespace())
                    .to_owned(),
            );
            if accounts::validate_password(&password, &self.config).is_err() {
                self.prompt(
                    id,
                    LoginFlow::Name,
                    "Either there is already a player with that name, or that name is illegal.\r\nWho are you? ",
                    false,
                );
                return;
            }
            password
        } else {
            password
        };
        self.authentication.inflight += 1;
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
        let credential = hash.clone();
        let password_length = password.len();
        tokio::spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                if create {
                    accounts::hash(&password, &c).map(Some)
                } else if hash
                    .as_deref()
                    .map(|h| accounts::verify_checked(&password, h))
                    .transpose()?
                    .unwrap_or(false)
                {
                    Ok(None)
                } else {
                    Err(accounts::IncorrectCredentials.into())
                }
            })
            .await
            .unwrap_or_else(|e| Err(e.into()));
            let _ = tx
                .send(Event::Authenticated(Outcome {
                    session: id,
                    name,
                    create,
                    identity,
                    credential,
                    result,
                    password_length,
                }))
                .await;
        });
    }
    /// Hash jobs retain their original identity; purging or shutdown invalidates their results.
    pub(super) async fn authentication_result(
        &mut self,
        id: SessionId,
        name: String,
        create: bool,
        identity: Option<ObjectId>,
        credential: Option<String>,
        result: Result<Option<String>>,
    ) -> Result<()> {
        self.authentication.inflight = self.authentication.inflight.saturating_sub(1);
        if self.shutdown.is_some() {
            return Ok(());
        }
        if !create
            && (self.scripts.world.borrow().find_player(&name) != identity
                || identity.and_then(|p| {
                    self.scripts
                        .world
                        .borrow()
                        .accounts
                        .get(&p)
                        .and_then(|a| a.hash.clone())
                }) != credential)
        {
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
    pub(super) async fn authenticated(
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
        let failure_notice = existing.and_then(|p| {
            let world = self.scripts.world.borrow();
            let account = &world.accounts[&p];
            (account.unreported_failures > 0).then(|| {
                (
                    account.unreported_failures,
                    account
                        .history
                        .iter()
                        .rev()
                        .find(|record| !record.success)
                        .cloned(),
                )
            })
        });
        let hash = match result {
            Ok(h) => h,
            Err(error) => {
                if create || !error.is::<accounts::IncorrectCredentials>() {
                    tracing::error!(error = %format_args!("{error:#}"), "authentication worker failed");
                    self.prompt(
                        id,
                        LoginFlow::Name,
                        "Authentication unavailable. Please try again.\r\nWho are you? ",
                        false,
                    );
                    return Ok(());
                }
                let exhausted = self.sessions.get_mut(&id).unwrap().failed_login();
                tracing::warn!(target: crate::logging::targets::LOGINS, name = ?name, peer = %host, "authentication failed");
                if let Some(p) = existing {
                    let mut w = self.scripts.world.borrow_mut();
                    let a = w.accounts.get_mut(&p).unwrap();
                    a.failures = a.failures.saturating_add(1);
                    a.unreported_failures = a.unreported_failures.saturating_add(1);
                    a.history.push(Login {
                        success: false,
                        at: crate::clock::wall_time(),
                        host,
                    });
                    persistence::trim_history(
                        &mut a.history,
                        self.config.security.login_history_limit,
                    );
                }
                self.commit(before).await;
                if exhausted {
                    self.prompt(
                        id,
                        LoginFlow::Pending,
                        "Either that player does not exist, or has a different password.\r\n",
                        false,
                    );
                    self.disconnect(id).await?;
                } else {
                    self.prompt(id, LoginFlow::Name, "Either that player does not exist, or has a different password.\r\nWho are you? ", false);
                }
                return Ok(());
            }
        };
        let privileged = !create
            && existing
                .is_some_and(|p| crate::authority::is_wizard(&self.scripts.world.borrow(), p));
        let admission = if create {
            self.registration_admission()
        } else {
            self.admission(privileged)
        };
        if let Err(reason) = admission {
            self.reject_admission(id, reason).await?;
            return Ok(());
        }
        let p = if create {
            if existing.is_some() {
                self.prompt(
                    id,
                    LoginFlow::Name,
                    "Either there is already a player with that name, or that name is illegal.\r\nWho are you? ",
                    false,
                );
                return Ok(());
            }
            tracing::info!(target: crate::logging::targets::ACCOUNTS, name = ?name, "registering character");
            match self.create_account(name, hash) {
                Ok(p) => p,
                Err(e) => {
                    *self.scripts.world.borrow_mut() = before;
                    self.reconcile_connections();
                    self.scripts.effects.rollback();
                    tracing::error!(error = %format_args!("{e:#}"), "registration failed");
                    self.prompt(
                        id,
                        LoginFlow::Name,
                        "Either there is already a player with that name, or that name is illegal.\r\nWho are you? ",
                        false,
                    );
                    return Ok(());
                }
            }
        } else {
            existing.context("authenticated player disappeared")?
        };
        {
            let mut w = self.scripts.world.borrow_mut();
            let a = w.accounts.get_mut(&p).unwrap();
            a.unreported_failures = 0;
            a.last_login = Some(crate::clock::wall_time());
            a.last_site = Some(host.clone());
            a.successes = a.successes.saturating_add(1);
            a.history.push(Login {
                success: true,
                at: crate::clock::wall_time(),
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
        let transition = self.transition(
            p,
            if reconnect {
                TransitionKind::Reconnected
            } else {
                TransitionKind::Connected
            },
            self.sessions[&id].peer,
            self.sessions[&id].site,
        );
        let session = self.sessions.get_mut(&id).unwrap();
        session.player = Some(p);
        session.connected = tokio::time::Instant::now();
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
            tracing::error!(error = %format_args!("{e:#}"), "connect hook failed");
            *self.scripts.world.borrow_mut() = before.clone();
            self.reconcile_connections();
            self.scripts.effects.rollback();
        }
        self.commit(before).await;
        self.flush();
        self.announce_transition(transition).await;
        self.tell(id, "Connected.\r\n");
        if let Some((count, latest)) = failure_notice {
            let mut notice = format!(
                "\r\n**** {count} failed connect{} since your last successful connect. ****",
                if count == 1 { "" } else { "s" }
            );
            if let Some(latest) = latest
                && let Some(at) = chrono::DateTime::from_timestamp(latest.at, 0)
            {
                notice.push_str(&format!(
                    "\r\nMost recent attempt was from {} on {}.",
                    latest.host,
                    at.format("%Y-%m-%dT%H:%M:%SZ")
                ));
            }
            crate::notification::direct(
                &self.scripts.outbox,
                &self.config,
                p,
                crate::text::Document::Literal(notice),
            )?;
            // Successful-login notices use ordinary player routing, including other sessions.
            self.flush();
        }
        if !self.controls.enabled(crate::controls::Control::Logins) {
            self.tell(id, "*** Logins are disabled.\r\n");
        }
        let before = self.scripts.world.borrow().clone();
        match commands::look_in(&self.scripts, &self.config, p, id.0) {
            Ok(None) => {
                if self.commit(before).await {
                    self.flush();
                } else {
                    self.tell(id, "Unable to save your changes. Please try again.\r\n");
                }
            }
            Ok(Some(error)) => {
                if let Some(session) = self.sessions.get(&id) {
                    session.raw(crate::telnet::bounded_error(
                        &error,
                        self.config.runtime.output_message_limit,
                    ));
                }
            }
            Err(error) => {
                *self.scripts.world.borrow_mut() = before;
                self.reconcile_connections();
                self.scripts.effects.rollback();
                tracing::error!(error = %format_args!("{error:#}"), "connect appearance failed");
                self.tell(id, "Unable to render your location.\r\n");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Account;

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
            authentication: authentication::State::with_hash_capacity(1, Instant::now(), 1),
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
        let (output, mut receiver) = mpsc::channel(16);
        let now = tokio::time::Instant::now();
        server.sessions.insert(
            SessionId(1),
            Session {
                retry_remaining: 3,
                output,
                stats: Default::default(),
                palette: Default::default(),
                color_override: Default::default(),
                presets_emitted: Default::default(),
                peer: "127.0.0.1".parse().unwrap(),
                site: Default::default(),
                player: None,
                flow: LoginFlow::Pending,
                connected: now,
                active: now,
                decoder: Default::default(),
                quota: 1,
                quota_at: now,
                failed: Default::default(),
                output_message_limit: 65536,
            },
        );
        server
            .authentication_result(
                SessionId(1),
                "Wizard".into(),
                false,
                Some(ObjectId(2)),
                None,
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
    /// A completed hash must use current admission policy before touching persistent state.
    #[tokio::test(flavor = "current_thread")]
    async fn admission_rechecked_after_hashing_without_history_or_creation() {
        for create in [false, true] {
            let mut c = Config::load(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"),
            )
            .unwrap();
            let mut world = persistence::load(&c.database()).await.unwrap();
            world
                .objects
                .get_mut(&ObjectId(2))
                .unwrap()
                .flags
                .remove(crate::flags::Flag::Wizard);
            let credential = world.accounts[&ObjectId(2)].hash.clone();
            let scripts = Scripts::new(&c, Rc::new(RefCell::new(world))).unwrap();
            let d = tempfile::tempdir().unwrap();
            c.root = d.path().into();
            let (events, _) = mpsc::channel(1);
            let mut server = Server {
                config: c,
                scripts,
                sessions: BTreeMap::new(),
                events,
                authentication: authentication::State::with_hash_capacity(1, Instant::now(), 1),
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
            let (output, _receiver) = mpsc::channel(16);
            let now = tokio::time::Instant::now();
            server.sessions.insert(
                SessionId(1),
                Session {
                    retry_remaining: 3,
                    output,
                    stats: Default::default(),
                    palette: Default::default(),
                    color_override: Default::default(),
                    presets_emitted: Default::default(),
                    peer: "127.0.0.1".parse().unwrap(),
                    site: Default::default(),
                    player: None,
                    flow: LoginFlow::Pending,
                    connected: now,
                    active: now,
                    decoder: Default::default(),
                    quota: 1,
                    quota_at: now,
                    failed: Default::default(),
                    output_message_limit: 65536,
                },
            );

            let before = serde_json::to_vec(&*server.scripts.world.borrow()).unwrap();
            server.controls.set(crate::controls::Control::Logins, false);
            server
                .authentication_result(
                    SessionId(1),
                    if create { "NewAdmission" } else { "Wizard" }.into(),
                    create,
                    if create { None } else { Some(ObjectId(2)) },
                    if create { None } else { credential },
                    Ok(if create { Some("unused".into()) } else { None }),
                )
                .await
                .unwrap();
            assert!(!server.sessions.contains_key(&SessionId(1)));
            assert_eq!(
                before,
                serde_json::to_vec(&*server.scripts.world.borrow()).unwrap()
            );
            assert!(!server.config.database().exists());
        }
    }
}
