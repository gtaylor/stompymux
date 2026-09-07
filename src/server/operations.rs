//! Snapshot and deliver administrative reports without borrowing the world across I/O.
use super::*;

impl Server {
    /// Include every authenticated descriptor, including DARK players and duplicate accounts.
    pub(super) fn who_report(&self, prefix: &str) -> String {
        let world = self.scripts.world.borrow();
        let prefix = prefix.trim().to_lowercase();
        let rows = self
            .sessions
            .values()
            .filter_map(|s| {
                let object = world.objects.get(&s.player?)?;
                let name = crate::text::plain_with(&self.scripts.palette, &object.name);
                if !name.to_lowercase().starts_with(&prefix) {
                    return None;
                }
                let mut markers = String::new();
                if object.flags.contains(crate::flags::Flag::Dark) {
                    markers.push('D');
                }
                if object.flags.contains(crate::flags::Flag::Suspect) {
                    markers.push('+');
                }
                Some(crate::operations::WhoRow {
                    name,
                    connected: s.connected.elapsed().as_secs(),
                    idle: s.active.elapsed().as_secs(),
                    markers,
                    location: object.location.map_or(-1, |id| id.0),
                    commands: s.stats.commands.load(std::sync::atomic::Ordering::Relaxed),
                    host: s.peer.to_string(),
                })
            })
            .collect::<Vec<_>>();
        crate::operations::who_report(&rows, world.record_players, self.config.mux.max_players)
    }

    /// Chunk literal rows privately through the established output budgets and transport.
    pub(super) async fn operation_report(&self, id: SessionId, text: &str) {
        if let Some(session) = self.sessions.get(&id)
            && let Err(error) = session.literal_report(text, &self.config).await
        {
            self.config.log(
                &[crate::logging::Category::Network],
                "NET",
                "REPORT",
                error.to_string(),
            );
            self.tell(id, "Unable to deliver complete report.\r\n");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::Path, sync::atomic::Ordering::Relaxed};

    fn copy(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            if entry.path().is_dir() {
                copy(&entry.path(), &to.join(entry.file_name()));
            } else {
                std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
            }
        }
    }

    /// Inspect exact monotonic state without timing-dependent TCP sleeps.
    #[tokio::test(flavor = "current_thread")]
    async fn idle_quota_accounting_and_audit_boundaries() {
        let d = tempfile::tempdir().unwrap();
        copy(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"),
            d.path(),
        );
        let path = d.path().join("stompymux.toml");
        let mut config: toml::Value =
            toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        config
            .as_table_mut()
            .unwrap()
            .entry("runtime")
            .or_insert(toml::Value::Table(Default::default()))
            .as_table_mut()
            .unwrap()
            .insert("write_timeout_ms".into(), 20.into());
        std::fs::write(path, toml::to_string(&config).unwrap()).unwrap();
        let c = Config::load(d.path()).unwrap();
        let world = persistence::load(&c.database()).await.unwrap();
        let scripts = Scripts::new(&c, Rc::new(RefCell::new(world))).unwrap();
        scripts.communication(&c).create("SuspectsLog").unwrap();
        scripts
            .world
            .borrow_mut()
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .insert(crate::flags::Flag::Suspect);
        let (events, _) = mpsc::channel(8);
        let mut server = Server {
            config: c,
            scripts,
            sessions: Default::default(),
            events,
            addresses: Default::default(),
            hashes: Bucket {
                tokens: 1,
                at: Instant::now(),
            },
            inflight: 0,
            pending_resets: Default::default(),
            started_at: 0,
            listen_port: 0,
            shutdown: None,
            shutdown_failed: false,
            command_queue: Default::default(),
            cleaning: Default::default(),
            controls: Default::default(),
            idle_recheck: false,
            message_cache: Default::default(),
        };
        let (output, mut received) = mpsc::channel(128);
        let now = Instant::now();
        let old = now - Duration::from_secs(20);
        server.sessions.insert(
            SessionId(1),
            Session {
                retry_remaining: 3,
                output,
                stats: Default::default(),
                peer: "127.0.0.1".parse().unwrap(),
                site: Default::default(),
                player: Some(ObjectId(1)),
                flow: LoginFlow::Name,
                connected: old,
                active: old,
                decoder: Default::default(),
                quota: 1,
                quota_at: now,
                failed: Default::default(),
                output_message_limit: 65536,
                palette: server.scripts.palette.clone(),
                color_override: Default::default(),
                presets_emitted: Default::default(),
            },
        );
        let before = std::fs::read(server.config.database()).unwrap();
        server.input(SessionId(1), b"iD").await.unwrap();
        server.input(SessionId(1), b"LE\r\n").await.unwrap();
        let session = &server.sessions[&SessionId(1)];
        assert_eq!(session.active, old);
        assert_eq!(session.quota, 0);
        assert_eq!(session.stats.commands.load(Relaxed), 0);
        assert!(received.try_recv().is_err());
        assert_eq!(
            server.scripts.world.borrow().channels["SuspectsLog"].messages,
            0
        );
        server
            .input(SessionId(1), b"IDLE\r\nversion\r\n")
            .await
            .unwrap();
        assert_eq!(
            server.sessions[&SessionId(1)].stats.commands.load(Relaxed),
            0
        );
        assert_eq!(
            server.scripts.world.borrow().channels["SuspectsLog"].messages,
            0
        );
        assert_eq!(before, std::fs::read(server.config.database()).unwrap());
        server
            .scripts
            .world
            .borrow_mut()
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .remove(crate::flags::Flag::Suspect);
        // Automatic command calls do not pass the accepted-input counter boundary.
        server
            .command(SessionId(1), ObjectId(1), "version")
            .await
            .unwrap();
        assert_eq!(
            server.sessions[&SessionId(1)].stats.commands.load(Relaxed),
            0
        );
        let session = server.sessions.get_mut(&SessionId(1)).unwrap();
        session.quota = 10;
        session.quota_at = Instant::now();
        session.stats.commands.store(u64::MAX, Relaxed);
        server.input(SessionId(1), b"version\r\n").await.unwrap();
        assert_eq!(
            server.sessions[&SessionId(1)].stats.commands.load(Relaxed),
            u64::MAX
        );
        // A reader that stops draining cannot hold a report beyond the write deadline.
        while server.sessions[&SessionId(1)]
            .output
            .try_send(Output::Bytes(vec![b'x']))
            .is_ok()
        {}
        tokio::time::timeout(
            Duration::from_secs(1),
            server.operation_report(SessionId(1), "blocked report"),
        )
        .await
        .unwrap();
        assert!(server.sessions[&SessionId(1)].failed.get());
        server.config.logger.shutdown(&server.config).await.unwrap();
    }
}
