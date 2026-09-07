//! Bounded account hashing and administrative session lifecycle on the world owner.
use super::*;
use crate::{
    account_admin::{BootTarget, Request},
    flags::Flag,
    state::Generation,
};

/// Non-secret identity captured for an outstanding administrative hash job.
pub(super) struct Job {
    session: SessionId,
    caller: ObjectId,
    caller_generation: Generation,
    target: Target,
    password_length: usize,
}
enum Target {
    Create(String),
    Reset(ObjectId, Generation),
}

impl Server {
    /// Shared durable player initialization, without any login/session side effects.
    pub(super) fn create_account(&self, name: String, hash: Option<String>) -> Result<ObjectId> {
        accounts::validate_name(&name, &self.config)?;
        anyhow::ensure!(
            self.scripts.world.borrow().find_player(&name).is_none(),
            "That name is not available."
        );
        let p = {
            let mut w = self.scripts.world.borrow_mut();
            let p = w.create_with(
                &self.config,
                name,
                Kind::Player,
                crate::CreationContext::Player,
            )?;
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
        };
        if !self.config.mux.public_channel.is_empty() {
            let service = self.scripts.communication(&self.config);
            if service.name(&self.config.mux.public_channel).is_ok() {
                service.add(p, &self.config.mux.public_channel, "pub", true, true)?;
            }
        }
        Ok(p)
    }

    /// Check global hashing capacity shared with registration and authentication.
    fn reserve_admin_hash(&mut self) -> bool {
        if self.hashes.at.elapsed() >= HASH_RATE_WINDOW {
            self.hashes.at = Instant::now();
            self.hashes.tokens = self.config.security.login_hash_limit;
        }
        if self.hashes.tokens == 0 || self.inflight >= self.config.security.login_hash_concurrency {
            return false;
        }
        self.hashes.tokens -= 1;
        self.inflight += 1;
        true
    }

    pub(super) async fn account_admin(
        &mut self,
        id: SessionId,
        caller: ObjectId,
        request: Request,
    ) -> Result<()> {
        if let Request::Boot { target, quiet } = request {
            return self.boot(id, caller, target, quiet).await;
        }
        let (target, password) = match request {
            Request::Create { name, password } => (Target::Create(name), password),
            Request::Reset { target, password } => {
                if self.pending_resets.contains(&target) {
                    self.tell(
                        id,
                        "A password reset is already pending for that player.\r\n",
                    );
                    return Ok(());
                }
                (
                    Target::Reset(
                        target,
                        self.scripts.world.borrow().objects[&target].generation,
                    ),
                    password,
                )
            }
            Request::Boot { .. } => unreachable!(),
        };
        if !self.reserve_admin_hash() {
            self.tell(id, "Password hashing capacity reached. Try later.\r\n");
            return Ok(());
        }
        if let Target::Reset(p, _) = target {
            self.pending_resets.insert(p);
        }
        let job = Job {
            password_length: password.len(),
            session: id,
            caller,
            caller_generation: self.scripts.world.borrow().objects[&caller].generation,
            target,
        };
        let config = self.config.clone();
        let tx = self.events.clone();
        tokio::spawn(async move {
            let result = tokio::task::spawn_blocking(move || accounts::hash(&password, &config))
                .await
                .unwrap_or_else(|e| Err(e.into()));
            let _ = tx.send(Event::AdminHashed(job, result)).await;
        });
        Ok(())
    }

    pub(super) async fn admin_hashed(&mut self, job: Job, result: Result<String>) {
        self.inflight = self.inflight.saturating_sub(1);
        if let Target::Reset(p, _) = job.target {
            self.pending_resets.remove(&p);
        }
        if self.shutdown.is_some()
            || !self
                .sessions
                .get(&job.session)
                .is_some_and(|s| s.player == Some(job.caller))
        {
            return;
        }
        let authorized = self
            .scripts
            .world
            .borrow()
            .objects
            .get(&job.caller)
            .is_some_and(|o| {
                o.generation == job.caller_generation
                    && !o.flags.contains(Flag::Going)
                    && (job.caller == ObjectId(1) || o.flags.contains(Flag::Wizard))
            });
        if !authorized {
            self.tell(job.session, "Permission denied.\r\n");
            return;
        }
        if job.password_length > self.config.security.player_password_length_limit {
            self.tell(
                job.session,
                "Password policy changed. Please try again.\r\n",
            );
            return;
        }
        let Ok(hash) = result else {
            self.tell(job.session, "Unable to hash password.\r\n");
            return;
        };
        let before = self.scripts.world.borrow().clone();
        let changed = (|| -> Result<(ObjectId, bool)> {
            match job.target {
                Target::Create(name) => Ok((self.create_account(name, Some(hash))?, true)),
                Target::Reset(p, generation) => {
                    let mut world = self.scripts.world.borrow_mut();
                    anyhow::ensure!(
                        p != ObjectId(1)
                            && world
                                .objects
                                .get(&p)
                                .is_some_and(|o| o.generation == generation
                                    && o.kind == Kind::Player
                                    && !o.flags.contains(Flag::Going)),
                        "Account no longer available."
                    );
                    world
                        .accounts
                        .get_mut(&p)
                        .context("Account no longer available.")?
                        .hash = Some(hash);
                    Ok((p, false))
                }
            }
        })();
        let (target, created) = match changed {
            Ok(v) => v,
            Err(e) => {
                *self.scripts.world.borrow_mut() = before;
                self.reconcile_connections();
                self.scripts.outbox.borrow_mut().clear();
                self.scripts.flows.rollback();
                self.tell(job.session, &format!("{e}\r\n"));
                return;
            }
        };
        if !self.commit(before).await {
            self.tell(job.session, "Unable to save account change.\r\n");
            return;
        }
        self.flush();
        self.config.log(
            if created {
                &[
                    crate::logging::Category::Wizard,
                    crate::logging::Category::Create,
                ]
            } else {
                &[crate::logging::Category::Wizard]
            },
            "WIZ",
            "ACCOUNT",
            format!(
                "Account administration: #{} {} #{}",
                job.caller.0,
                if created {
                    "created"
                } else {
                    "reset password for"
                },
                target.0
            ),
        );
        if created {
            let name = self.scripts.world.borrow().objects[&target].name.clone();
            self.tell(
                job.session,
                &format!("New player '{name}' (#{}) created.\r\n", target.0),
            );
        } else {
            self.tell(job.session, "Password changed.\r\n");
            let name = self.scripts.world.borrow().objects[&job.caller]
                .name
                .clone();
            for (id, s) in &self.sessions {
                if s.player == Some(target) {
                    self.tell(
                        *id,
                        &format!("Your password has been changed by {name}.\r\n"),
                    );
                }
            }
        }
    }

    async fn boot(
        &mut self,
        id: SessionId,
        caller: ObjectId,
        target: BootTarget,
        quiet: bool,
    ) -> Result<()> {
        let ids: Vec<_> = match target {
            BootTarget::Player(p) => {
                if p == ObjectId(1) {
                    self.tell(id, "You cannot boot that player!\r\n");
                    return Ok(());
                }
                if p == caller {
                    self.tell(id, "You can only boot off other players!\r\n");
                    return Ok(());
                }
                let name = self.scripts.world.borrow().objects[&p].name.clone();
                self.tell(id, &format!("You booted {name} off!\r\n"));
                self.sessions
                    .iter()
                    .filter(|(_, s)| s.player == Some(p))
                    .map(|(id, _)| *id)
                    .collect()
            }
            BootTarget::Session(port) => self
                .sessions
                .get(&SessionId(port))
                .filter(|s| caller == ObjectId(1) || s.player != Some(ObjectId(1)))
                .map(|_| SessionId(port))
                .into_iter()
                .collect(),
        };
        self.tell(
            id,
            &format!(
                "{} connection{} closed.\r\n",
                ids.len(),
                if ids.len() == 1 { "" } else { "s" }
            ),
        );
        let name = self.scripts.world.borrow().objects[&caller].name.clone();
        for victim in ids {
            if !quiet {
                self.tell(victim, &format!("{name} gently shows you the door.\r\n"));
            }
            self.config.log(
                &[crate::logging::Category::Wizard],
                "WIZ",
                "ACCOUNT",
                format!(
                    "Account administration: #{} booted session {}",
                    caller.0, victim.0
                ),
            );
            self.disconnect(victim).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn copy(source: &std::path::Path, target: &std::path::Path) {
        std::fs::create_dir_all(target).unwrap();
        for e in std::fs::read_dir(source).unwrap() {
            let e = e.unwrap();
            if e.path().is_dir() {
                copy(&e.path(), &target.join(e.file_name()));
            } else {
                std::fs::copy(e.path(), target.join(e.file_name())).unwrap();
            }
        }
    }
    fn session(player: Option<ObjectId>) -> (Session, mpsc::Receiver<Output>) {
        let (output, rx) = mpsc::channel(128);
        let now = Instant::now();
        (
            Session {
                retry_remaining: 3,
                output,
                stats: Default::default(),
                palette: Default::default(),
                color_override: Default::default(),
                presets_emitted: Default::default(),
                peer: "127.0.0.1".parse().unwrap(),
                site: Default::default(),
                player,
                flow: LoginFlow::Pending,
                connected: now,
                active: now,
                decoder: Default::default(),
                quota: 10,
                quota_at: now,
                failed: Default::default(),
                output_message_limit: 65536,
            },
            rx,
        )
    }
    async fn fixture() -> (
        tempfile::TempDir,
        Server,
        mpsc::Receiver<Event>,
        Vec<mpsc::Receiver<Output>>,
    ) {
        let d = tempfile::tempdir().unwrap();
        copy(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"),
            d.path(),
        );
        let config = Config::load(d.path()).unwrap();
        let mut w = persistence::load(&config.database()).await.unwrap();
        for p in [ObjectId(1), ObjectId(2)] {
            w.objects.get_mut(&p).unwrap().location = Some(ObjectId(config.start()));
        }
        persistence::save(&config.database(), &w).await.unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(w))).unwrap();
        let (events, rx) = mpsc::channel(32);
        let mut server = Server {
            config,
            scripts,
            sessions: BTreeMap::new(),
            events,
            addresses: BTreeMap::new(),
            hashes: Bucket {
                tokens: 100,
                at: Instant::now(),
            },
            inflight: 0,
            pending_resets: Default::default(),
            started_at: accounts::now(),
            listen_port: 0,
            shutdown: None,
            shutdown_failed: false,
            command_queue: Default::default(),
            cleaning: Default::default(),
            controls: Default::default(),
            idle_recheck: false,
            message_cache: Default::default(),
        };
        let mut outputs = Vec::new();
        for (id, p) in [(1, Some(ObjectId(1))), (2, Some(ObjectId(2))), (3, None)] {
            let (s, rx) = session(p);
            server.sessions.insert(SessionId(id), s);
            outputs.push(rx);
        }
        server.reconcile_connections();
        (d, server, rx, outputs)
    }
    fn job(s: &Server, target: Target) -> Job {
        Job {
            password_length: 5,
            session: SessionId(1),
            caller: ObjectId(1),
            caller_generation: s.scripts.world.borrow().objects[&ObjectId(1)].generation,
            target,
        }
    }
    fn reset(s: &Server) -> Job {
        job(
            s,
            Target::Reset(
                ObjectId(2),
                s.scripts.world.borrow().objects[&ObjectId(2)].generation,
            ),
        )
    }
    fn output(rx: &mut mpsc::Receiver<Output>) -> String {
        let mut text = Vec::new();
        while let Ok(o) = rx.try_recv() {
            if let Output::Bytes(b) = o {
                text.extend(b);
            }
        }
        String::from_utf8_lossy(&text).into_owned()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn committed_reset_rejects_old_hash_results_and_preserves_sessions() {
        let (_d, mut s, _rx, mut output) = fixture().await;
        let old = s.scripts.world.borrow().accounts[&ObjectId(2)].hash.clone();
        s.admin_hashed(
            reset(&s),
            Ok(accounts::hash("new-secret", &s.config).unwrap()),
        )
        .await;
        assert!(super::tests::output(&mut output[0]).contains("Password changed."));
        assert!(super::tests::output(&mut output[1]).contains("Your password has been changed"));
        s.authentication_result(
            SessionId(3),
            "#2".into(),
            false,
            Some(ObjectId(2)),
            old,
            Ok(None),
        )
        .await
        .unwrap();
        assert!(s.sessions[&SessionId(3)].player.is_none());
        assert!(
            s.scripts.world.borrow().objects[&ObjectId(2)]
                .flags
                .contains(Flag::Connected)
        );
        let w = persistence::load(&s.config.database()).await.unwrap();
        assert!(accounts::verify(
            "new-secret",
            w.accounts[&ObjectId(2)].hash.as_ref().unwrap()
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn failed_reset_keeps_credentials_and_abandoned_jobs_cannot_create() {
        use sqlx::Connection;
        let (_d, mut s, _rx, mut outputs) = fixture().await;
        let old = s.scripts.world.borrow().accounts[&ObjectId(2)].hash.clone();
        let mut db = sqlx::SqliteConnection::connect(s.config.database().to_str().unwrap())
            .await
            .unwrap();
        sqlx::query("CREATE TRIGGER reject_reset BEFORE UPDATE ON player_state BEGIN SELECT RAISE(FAIL,'injected reset failure'); END").execute(&mut db).await.unwrap();
        db.close().await.unwrap();
        s.admin_hashed(reset(&s), Ok("replacement".into())).await;
        assert_eq!(s.scripts.world.borrow().accounts[&ObjectId(2)].hash, old);
        assert!(!output(&mut outputs[0]).contains("Password changed."));
        let j = job(&s, Target::Create("Abandoned".into()));
        s.sessions.remove(&SessionId(1));
        s.admin_hashed(j, Ok("unused".into())).await;
        assert!(s.scripts.world.borrow().find_player("Abandoned").is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reset_capacity_duplicate_guard_permissions_and_shutdown() {
        let (_d, mut s, mut events, _outputs) = fixture().await;
        s.account_admin(
            SessionId(1),
            ObjectId(1),
            Request::Reset {
                target: ObjectId(2),
                password: Zeroizing::new("secret".into()),
            },
        )
        .await
        .unwrap();
        assert!(s.pending_resets.contains(&ObjectId(2)));
        let inflight = s.inflight;
        s.account_admin(
            SessionId(1),
            ObjectId(1),
            Request::Reset {
                target: ObjectId(2),
                password: Zeroizing::new("other".into()),
            },
        )
        .await
        .unwrap();
        assert_eq!(s.inflight, inflight);
        let Event::AdminHashed(j, r) = events.recv().await.unwrap() else {
            panic!("expected hash completion")
        };
        s.shutdown = Some(ShutdownRequest::Sigterm);
        let before = s.scripts.world.borrow().accounts[&ObjectId(2)].hash.clone();
        s.admin_hashed(j, r).await;
        assert_eq!(s.scripts.world.borrow().accounts[&ObjectId(2)].hash, before);
        assert!(s.pending_resets.is_empty());
        s.shutdown = None;
        let j = Job {
            password_length: 5,
            session: SessionId(2),
            caller: ObjectId(2),
            caller_generation: s.scripts.world.borrow().objects[&ObjectId(2)].generation,
            target: Target::Create("Revoked".into()),
        };
        s.scripts
            .world
            .borrow_mut()
            .objects
            .get_mut(&ObjectId(2))
            .unwrap()
            .flags
            .remove(Flag::Wizard);
        s.admin_hashed(j, Ok("unused".into())).await;
        assert!(s.scripts.world.borrow().find_player("Revoked").is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn competing_creations_and_boot_pending_authentication() {
        let (_d, mut s, _rx, _outputs) = fixture().await;
        let first = job(&s, Target::Create("Concurrent".into()));
        let second = job(&s, Target::Create("concurrent".into()));
        s.admin_hashed(first, Ok("hash".into())).await;
        s.admin_hashed(second, Ok("hash".into())).await;
        {
            let w = s.scripts.world.borrow();
            assert_eq!(
                w.objects
                    .values()
                    .filter(|o| o.name.eq_ignore_ascii_case("concurrent"))
                    .count(),
                1
            );
            let p = w.find_player("Concurrent").unwrap();
            assert_eq!(w.accounts[&p].successes, 0);
            assert!(!w.objects[&p].flags.contains(Flag::Connected));
        }
        s.boot(SessionId(1), ObjectId(1), BootTarget::Session(3), true)
            .await
            .unwrap();
        s.authentication_result(
            SessionId(3),
            "Late".into(),
            true,
            None,
            None,
            Ok(Some("unused".into())),
        )
        .await
        .unwrap();
        assert!(s.scripts.world.borrow().find_player("Late").is_none());
        s.boot(SessionId(2), ObjectId(2), BootTarget::Session(1), false)
            .await
            .unwrap();
        assert!(s.sessions.contains_key(&SessionId(1)));
        s.boot(SessionId(1), ObjectId(1), BootTarget::Session(1), true)
            .await
            .unwrap();
        assert!(!s.sessions.contains_key(&SessionId(1)));
        assert!(
            !s.scripts.world.borrow().objects[&ObjectId(1)]
                .flags
                .contains(Flag::Connected)
        );
    }
    #[tokio::test(flavor = "current_thread")]
    async fn failed_creation_and_disconnect_callbacks_preserve_actual_sessions() {
        use sqlx::Connection;
        let (d, mut s, _rx, mut outputs) = fixture().await;
        let mut db = sqlx::SqliteConnection::connect(s.config.database().to_str().unwrap())
            .await
            .unwrap();
        sqlx::query("CREATE TRIGGER reject_creation BEFORE INSERT ON player_state BEGIN SELECT RAISE(FAIL,'injected creation failure'); END").execute(&mut db).await.unwrap();
        db.close().await.unwrap();
        let next = s.scripts.world.borrow().next_id;
        s.admin_hashed(
            job(&s, Target::Create("Unsaved".into())),
            Ok("unused".into()),
        )
        .await;
        assert_eq!(s.scripts.world.borrow().next_id, next);
        assert!(s.scripts.world.borrow().find_player("Unsaved").is_none());
        assert!(!output(&mut outputs[0]).contains("created."));
        std::fs::write(d.path().join("lua/global_logic/boot_failure.lua"),"return {events={on_player_disconnect=function(ctx) local o=mux.world.object(ctx.enactor);assert(not o:flags():has(mux.world.flags.CONNECTED));o:state('boot'):set('bad',true);error('injected boot hook failure')end}}").unwrap();
        s.scripts = Scripts::new(&s.config, s.scripts.world.clone()).unwrap();
        s.boot(
            SessionId(1),
            ObjectId(1),
            BootTarget::Player(ObjectId(2)),
            false,
        )
        .await
        .unwrap();
        assert!(!s.sessions.contains_key(&SessionId(2)));
        assert!(
            !s.scripts.world.borrow().objects[&ObjectId(2)]
                .flags
                .contains(Flag::Connected)
        );
        s.scripts
            .eval_callback::<()>("assert(mux.world.object(2):state('boot'):get('bad')==nil)")
            .unwrap();
    }
    /// Publication clamps balances without writing storage, and abandoned policy candidates stay invisible.
    #[tokio::test(flavor = "current_thread")]
    async fn runtime_configuration_clamps_and_pending_password_policy() {
        let (_d, mut server, _events, mut output) = fixture().await;
        let before = std::fs::read(server.config.database()).unwrap();
        let edit = |name: &str, value: &str| crate::config::administration::Request {
            directive: name.into(),
            value: value.into(),
        };
        assert_eq!(
            server.configure(ObjectId(1), edit("command_quota_max", "2")),
            "Set."
        );
        assert!(server.sessions.values().all(|s| s.quota <= 2));
        assert_eq!(
            server.configure(ObjectId(1), edit("command_quota_max", "100")),
            "Set."
        );
        assert!(server.sessions.values().all(|s| s.quota <= 2));
        let limit = server.config.mux.check_interval;
        assert_ne!(
            server.configure(ObjectId(1), edit("check_interval", "0")),
            "Set."
        );
        assert_eq!(server.config.mux.check_interval, limit);
        assert_eq!(
            server
                .scripts
                .lua
                .app_data_ref::<Config>()
                .unwrap()
                .mux
                .check_interval,
            limit
        );
        let mut pending = job(&server, Target::Create("PolicyRace".into()));
        pending.password_length = 40;
        server.inflight = 1;
        assert_eq!(
            server.configure(ObjectId(1), edit("player_password_length_limit", "30")),
            "Set."
        );
        server.admin_hashed(pending, Ok("unused hash".into())).await;
        assert!(
            server
                .scripts
                .world
                .borrow()
                .find_player("PolicyRace")
                .is_none()
        );
        assert_eq!(server.inflight, 0);
        assert_eq!(before, std::fs::read(server.config.database()).unwrap());
        // Keep output receivers alive for all session assertions.
        assert!(!output.is_empty());
        while output[0].try_recv().is_ok() {}
    }
    /// Runtime retry accounting survives history rollback and excludes worker/stale results.
    #[tokio::test(flavor = "current_thread")]
    async fn retries_ignore_internal_failures_and_survive_persistence_failure() {
        use sqlx::Connection;
        let (_d, mut s, _events, mut outputs) = fixture().await;
        let id = SessionId(3);
        s.sessions.get_mut(&id).unwrap().flow = LoginFlow::Pending;
        s.authenticated(
            id,
            "GOD".into(),
            false,
            Err(anyhow::anyhow!("worker failed")),
        )
        .await
        .unwrap();
        assert_eq!(s.sessions[&id].retry_remaining, 3);
        assert!(output(&mut outputs[2]).contains("Authentication unavailable"));
        s.sessions.get_mut(&id).unwrap().flow = LoginFlow::Pending;
        s.authentication_result(
            id,
            "GOD".into(),
            false,
            Some(ObjectId(999)),
            None,
            Err(accounts::IncorrectCredentials.into()),
        )
        .await
        .unwrap();
        assert_eq!(s.sessions[&id].retry_remaining, 3);
        let mut db = sqlx::SqliteConnection::connect_with(
            &sqlx::sqlite::SqliteConnectOptions::new()
                .filename(s.config.database())
                .foreign_keys(false),
        )
        .await
        .unwrap();
        sqlx::raw_sql("CREATE TRIGGER reject_history BEFORE UPDATE ON player_state BEGIN SELECT RAISE(ABORT,'history blocked'); END").execute(&mut db).await.unwrap();
        for remaining in [2, 1, 0] {
            s.sessions.get_mut(&id).unwrap().flow = LoginFlow::Pending;
            s.authenticated(
                id,
                "GOD".into(),
                false,
                Err(accounts::IncorrectCredentials.into()),
            )
            .await
            .unwrap();
            if remaining > 0 {
                assert_eq!(s.sessions[&id].retry_remaining, remaining);
            } else {
                assert!(!s.sessions.contains_key(&id));
            }
        }
        sqlx::raw_sql("DROP TRIGGER reject_history")
            .execute(&mut db)
            .await
            .unwrap();
        db.close().await.unwrap();
    }
    /// Registrations resolve current policy after hashing; unavailable zones roll back account creation.
    #[tokio::test(flavor = "current_thread")]
    async fn registration_rechecks_zone_after_hashing() {
        let (_d, mut s, _events, mut outputs) = fixture().await;
        let response = s.configure(
            ObjectId(1),
            crate::config::administration::Request {
                directive: "player_zone".into(),
                value: "4".into(),
            },
        );
        assert_eq!(response, "Set.");
        s.authenticated(
            SessionId(3),
            "FreshZone".into(),
            true,
            Ok(Some(accounts::hash("secret", &s.config).unwrap())),
        )
        .await
        .unwrap();
        let p = s.scripts.world.borrow().find_player("FreshZone").unwrap();
        assert_eq!(s.scripts.world.borrow().objects[&p].zone, Some(ObjectId(4)));
        assert_eq!(
            persistence::load(&s.config.database())
                .await
                .unwrap()
                .objects[&p]
                .zone,
            Some(ObjectId(4))
        );
        let (connection, _rx) = session(None);
        s.sessions.insert(SessionId(4), connection);
        s.scripts
            .world
            .borrow_mut()
            .objects
            .get_mut(&ObjectId(4))
            .unwrap()
            .flags
            .insert(Flag::Going);
        let next = s.scripts.world.borrow().next_id;
        s.authenticated(
            SessionId(4),
            "NoZone".into(),
            true,
            Ok(Some("unused".into())),
        )
        .await
        .unwrap();
        assert_eq!(s.scripts.world.borrow().next_id, next);
        assert!(s.scripts.world.borrow().find_player("NoZone").is_none());
        assert_eq!(s.sessions[&SessionId(4)].retry_remaining, 3);
        let _ = output(&mut outputs[2]);
    }
}
