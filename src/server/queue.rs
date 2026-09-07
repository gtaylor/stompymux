//! World-owner queue admission and background execution share persistence/output boundaries.
use super::*;
use crate::commands::queue::{Request, Work};

impl Server {
    /// Publish queue changes only after their associated durable mutations commit.
    pub(super) async fn queue_request(
        &mut self,
        session: Option<SessionId>,
        actor: ObjectId,
        request: Request,
        before: World,
    ) {
        if matches!(&request, Request::Add { .. })
            && !self.controls.enabled(crate::controls::Control::Queueing)
        {
            *self.scripts.world.borrow_mut() = before;
            self.reconcile_connections();
            self.scripts.outbox.borrow_mut().clear();
            self.scripts.flows.rollback();
            self.queue_reply(
                session,
                actor,
                "Sorry, queueing and triggering are not allowed now.",
            );
            self.flush();
            return;
        }
        let recipient = match &request {
            Request::Add { executor, .. } => *executor,
            _ => actor,
        };
        let mut candidate = self.command_queue.clone();
        let result = candidate.apply(
            request,
            &mut self.scripts.world.borrow_mut(),
            self.config.mux.command_queue_limit as usize,
            self.config.runtime.input_line_limit,
            tokio::time::Instant::now(),
        );
        match result {
            Ok(reply) => {
                if self.commit(before).await {
                    candidate.reconcile(&self.scripts.world.borrow());
                    self.command_queue = candidate;
                    if let Some(reply) = reply {
                        self.queue_reply(
                            if recipient == actor { session } else { None },
                            recipient,
                            &reply,
                        );
                    }
                    self.flush();
                } else {
                    self.queue_reply(
                        session,
                        actor,
                        "Unable to save your changes. Please try again.",
                    );
                    self.flush();
                }
            }
            Err(error) => {
                *self.scripts.world.borrow_mut() = before;
                self.reconcile_connections();
                self.scripts.outbox.borrow_mut().clear();
                self.scripts.flows.rollback();
                self.queue_reply(session, actor, &error.to_string());
                self.flush();
            }
        }
    }

    /// Interactive diagnostics stay private; background replies use ordinary object notification.
    pub(super) fn queue_reply(&self, session: Option<SessionId>, actor: ObjectId, text: &str) {
        if let Some(id) = session {
            self.tell(id, &format!("{text}\r\n"));
        } else {
            let result = crate::notification::send(
                &self.scripts.world.borrow(),
                &self.scripts.outbox,
                &self.config,
                crate::notification::Request {
                    target: actor,
                    sender: actor,
                    document: reply_document(
                        &self.scripts.palette,
                        text,
                        self.config
                            .runtime
                            .output_message_limit
                            .min(self.config.lua.output_byte_limit),
                    ),
                    policy: crate::notification::Policy::DIRECT,
                    exclusions: Default::default(),
                },
            );
            if let Err(error) = result {
                eprintln!("Queued command reply: {error:#}");
            }
        }
    }

    /// One ready command per loop turn, with no descriptor or borrowed connection.
    pub(super) async fn queued(&mut self, work: Work) {
        let actor = work.execution.executor;
        if self.shutdown.is_some() {
            return;
        }
        if let Err(error) = self.snapshots() {
            eprintln!("Queued command snapshot: {error:#}");
            return;
        }
        let mut before = self.scripts.world.borrow().clone();
        let action = commands::execute(&self.scripts, &self.config, work.execution, &work.text);
        if action.is_ok()
            && self.scripts.command_callbacks_invoked()
            && !matches!(
                &action,
                Ok(Action::Continue | Action::CommitReply(_) | Action::Queue(_))
            )
        {
            if !self.commit(before.clone()).await {
                self.queue_reply(
                    None,
                    actor,
                    "Unable to save your changes. Please try again.",
                );
                self.flush();
                return;
            }
            self.flush();
            before = self.scripts.world.borrow().clone();
        }
        match action {
            Ok(Action::Queue(request)) => self.queue_request(None, actor, request, before).await,
            Ok(Action::Continue) => {
                if self.commit(before).await {
                    self.flush();
                } else {
                    self.queue_reply(
                        None,
                        actor,
                        "Unable to save your changes. Please try again.",
                    );
                    self.flush();
                }
            }
            Ok(Action::CommitReply(text)) => {
                if self.commit(before).await {
                    self.queue_reply(None, actor, &text);
                    self.flush();
                } else {
                    self.queue_reply(
                        None,
                        actor,
                        "Unable to save your changes. Please try again.",
                    );
                    self.flush();
                }
            }
            Ok(
                Action::Reply(text)
                | Action::Report(text)
                | Action::LiteralReport(text)
                | Action::StyledReport(text),
            ) => {
                self.queue_reply(None, actor, &text);
                self.flush();
            }
            Ok(Action::ExamineDebug(object)) => {
                let result = persistence::inspect_links(
                    &self.config.database(),
                    object,
                    self.config.database.busy_timeout_ms,
                )
                .await
                .and_then(|links| {
                    commands::inspection::debug(&self.scripts.world.borrow(), object, links)
                });
                match result {
                    Ok(text) => self.queue_reply(None, actor, &text),
                    Err(error) => {
                        eprintln!("Queued examination: {error:#}");
                        self.queue_reply(None, actor, "Unable to read object bookkeeping.");
                    }
                }
                self.flush();
            }
            Ok(Action::Shutdown) => self.request_shutdown(ShutdownRequest::Player(actor)).await,
            Ok(Action::DbCheck) => {
                self.dbck(crate::cleaning::CheckOrigin::Queued {
                    actor,
                    cause: work.execution.cause,
                })
                .await
            }
            Ok(Action::ConfigAdmin(request)) => {
                let text = self.configure(actor, request);
                self.queue_reply(None, actor, &text);
                self.flush();
            }
            Ok(Action::ReadCache) => self.readcache(None, actor).await,
            Ok(Action::GlobalControl(value)) => {
                let response = self.global_control(value);
                self.queue_reply(None, actor, &response);
                self.flush();
            }
            Ok(_) => {
                *self.scripts.world.borrow_mut() = before;
                self.reconcile_connections();
                self.scripts.outbox.borrow_mut().clear();
                self.scripts.flows.rollback();
                self.queue_reply(None, actor, "This command requires an interactive session.");
                self.flush();
            }
            Err(error) => {
                *self.scripts.world.borrow_mut() = before;
                self.reconcile_connections();
                self.scripts.outbox.borrow_mut().clear();
                self.scripts.flows.rollback();
                eprintln!(
                    "Queued command for #{} (cause #{}): {error:#}",
                    actor.0, work.execution.cause.0
                );
                self.queue_reply(None, actor, "That queued command could not be completed.");
                self.flush();
            }
        }
    }
}

/// Large background reports fall back to bounded visible text without cutting markup or graphemes.
fn reply_document(
    palette: &crate::text::Palette,
    text: &str,
    limit: usize,
) -> crate::text::Document {
    use unicode_segmentation::UnicodeSegmentation;
    if text.len() <= limit {
        return text.to_owned().into();
    }
    const SUFFIX: &str = " [truncated]";
    let plain = crate::text::plain_with(palette, text);
    let mut result = String::new();
    let budget = limit.saturating_sub(SUFFIX.len());
    for grapheme in plain.graphemes(true) {
        if result.len() + grapheme.len() > budget {
            break;
        }
        result.push_str(grapheme);
    }
    if limit >= SUFFIX.len() {
        result.push_str(SUFFIX);
    }
    crate::text::Document::Literal(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        commands::{ExecutionContext, InputOrigin},
        flags::Flag,
    };
    use sqlx::Connection;

    fn copy(source: &std::path::Path, destination: &std::path::Path) {
        std::fs::create_dir_all(destination).unwrap();
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let target = destination.join(entry.file_name());
            if entry.path().is_dir() {
                copy(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }

    async fn fixture() -> (tempfile::TempDir, Server) {
        let d = tempfile::tempdir().unwrap();
        copy(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"),
            d.path(),
        );
        std::fs::write(
            d.path().join("lua/global_logic/queued_test.lua"),
            r#"return {commands={
          {name='queue-write',permission='everyone',pattern='^queue%-write$',handler=function(ctx)
            assert(ctx.enactor==2 and ctx.cause==1 and ctx.descriptor==nil)
            mux.world.object(2):state('queue'):set('saved',true)
            mux.world.pemit(2,'SAVED-OUTPUT')
            return true
          end},
          {name='queue-error',permission='everyone',pattern='^queue%-error$',handler=function(ctx)
            mux.world.object(2):state('queue'):set('leaked',true)
            mux.world.pemit(2,'LEAKED-OUTPUT')
            error('injected queue failure')
          end}
        }}"#,
        )
        .unwrap();
        let config = Config::load(d.path()).unwrap();
        let mut world = persistence::load(&config.database()).await.unwrap();
        world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(ObjectId(config.start()));
        persistence::save(&config.database(), &world).await.unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let (events, _) = mpsc::channel(32);
        (
            d,
            Server {
                config,
                scripts,
                sessions: Default::default(),
                events,
                addresses: Default::default(),
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
            },
        )
    }

    fn work(text: &str) -> Work {
        Work {
            execution: ExecutionContext {
                executor: ObjectId(2),
                cause: ObjectId(1),
                session: None,
                origin: InputOrigin::Queued,
            },
            text: text.into(),
        }
    }

    async fn sql(s: &Server, query: &'static str) {
        let mut db = sqlx::SqliteConnection::connect(s.config.database().to_str().unwrap())
            .await
            .unwrap();
        sqlx::query(query).execute(&mut db).await.unwrap();
        db.close().await.unwrap();
    }

    fn attach(s: &mut Server) -> mpsc::Receiver<Output> {
        let (output, receiver) = mpsc::channel(128);
        let now = Instant::now();
        s.sessions.insert(
            SessionId(1),
            Session {
                output,
                stats: Default::default(),
                palette: Default::default(),
                color_override: Default::default(),
                presets_emitted: Default::default(),
                peer: "127.0.0.1".parse().unwrap(),
                site: Default::default(),
                player: Some(ObjectId(2)),
                flow: LoginFlow::Pending,
                connected: now,
                active: now,
                decoder: Default::default(),
                quota: 10,
                quota_at: now,
                failed: Default::default(),
                output_message_limit: 65536,
            },
        );
        s.reconcile_connections();
        receiver
    }

    fn drain(receiver: &mut mpsc::Receiver<Output>) -> String {
        let mut text = String::new();
        while let Ok(output) = receiver.try_recv() {
            if let Output::Bytes(bytes) = output {
                text.push_str(&String::from_utf8_lossy(&bytes));
            }
        }
        text
    }

    #[tokio::test]
    async fn queued_lua_and_persistence_failure_consume_work_and_rollback_output() {
        let (_d, mut s) = fixture().await;
        let mut receiver = attach(&mut s);
        let before = std::fs::read(s.config.database()).unwrap();
        s.queued(work("queue-error")).await;
        assert!(
            !s.scripts.world.borrow().objects[&ObjectId(2)]
                .state
                .contains_key("queue")
        );
        assert!(s.scripts.outbox.borrow().is_empty());
        assert_eq!(std::fs::read(s.config.database()).unwrap(), before);
        let output = drain(&mut receiver);
        assert!(output.contains("could not be completed") && !output.contains("LEAKED-OUTPUT"));
        assert!(
            s.scripts.world.borrow().objects[&ObjectId(2)]
                .flags
                .contains(Flag::Connected)
        );
        sql(&s, "CREATE TRIGGER reject_queue BEFORE INSERT ON object_state BEGIN SELECT RAISE(FAIL,'queue write failure'); END").await;
        s.queued(work("queue-write")).await;
        assert!(
            !s.scripts.world.borrow().objects[&ObjectId(2)]
                .state
                .contains_key("queue")
        );
        assert!(s.scripts.outbox.borrow().is_empty());
        let output = drain(&mut receiver);
        assert!(output.contains("Unable to save") && !output.contains("SAVED-OUTPUT"));
        assert!(
            s.scripts.world.borrow().objects[&ObjectId(2)]
                .flags
                .contains(Flag::Connected)
        );
        sql(&s, "DROP TRIGGER reject_queue").await;
        s.queued(work("queue-write")).await;
        assert!(drain(&mut receiver).contains("SAVED-OUTPUT"));
        let saved = persistence::load(&s.config.database()).await.unwrap();
        assert_eq!(
            saved.objects[&ObjectId(2)].state["queue"]["saved"],
            Scalar::Boolean(true)
        );
        assert!(!saved.objects[&ObjectId(2)].flags.contains(Flag::Connected));
    }

    #[tokio::test]
    async fn admission_overflow_and_halt_are_transactional() {
        let (_d, mut s) = fixture().await;
        let now = tokio::time::Instant::now();
        let request = || Request::Add {
            executor: ObjectId(2),
            cause: ObjectId(1),
            seconds: 0,
            text: "say retained".into(),
        };
        // Filling a candidate queue does not touch storage or another executor.
        for _ in 0..s.config.mux.command_queue_limit {
            s.command_queue
                .apply(
                    request(),
                    &mut s.scripts.world.borrow_mut(),
                    s.config.mux.command_queue_limit as usize,
                    8192,
                    now,
                )
                .unwrap();
        }
        sql(&s, "CREATE TRIGGER reject_queue BEFORE UPDATE ON objects BEGIN SELECT RAISE(FAIL,'queue flag failure'); END").await;
        let before = s.scripts.world.borrow().clone();
        s.queue_request(None, ObjectId(1), request(), before).await;
        assert!(
            !s.scripts.world.borrow().objects[&ObjectId(2)]
                .flags
                .contains(Flag::Halted)
        );
        assert!(s.command_queue.ready(now));
        // A failed originating mutation must not publish a staged halt either.
        let before = s.scripts.world.borrow().clone();
        s.scripts
            .world
            .borrow_mut()
            .objects
            .get_mut(&ObjectId(2))
            .unwrap()
            .description = Some("unsaved".into());
        s.queue_request(None, ObjectId(1), Request::Halt { target: None }, before)
            .await;
        assert!(s.command_queue.ready(now));
        assert_ne!(
            s.scripts.world.borrow().objects[&ObjectId(2)]
                .description
                .as_deref(),
            Some("unsaved")
        );
        sql(&s, "DROP TRIGGER reject_queue").await;
        let before = s.scripts.world.borrow().clone();
        s.queue_request(None, ObjectId(1), request(), before).await;
        assert!(!s.command_queue.ready(now));
        assert!(
            persistence::load(&s.config.database())
                .await
                .unwrap()
                .objects[&ObjectId(2)]
                .flags
                .contains(Flag::Halted)
        );
    }

    #[tokio::test]
    async fn failed_origin_does_not_admit_work_and_shutdown_drops_list_tail() {
        let (_d, mut s) = fixture().await;
        let before = s.scripts.world.borrow().clone();
        s.scripts
            .world
            .borrow_mut()
            .objects
            .get_mut(&ObjectId(2))
            .unwrap()
            .location = Some(ObjectId(999999));
        s.queue_request(
            None,
            ObjectId(2),
            Request::Add {
                executor: ObjectId(2),
                cause: ObjectId(1),
                seconds: 0,
                text: "say never".into(),
            },
            before,
        )
        .await;
        assert!(s.command_queue.deadline().is_none());
        s.queued(work("@shutdown")).await;
        assert!(s.shutdown.is_some());
        s.queued(work("queue-write")).await;
        assert!(
            !s.scripts.world.borrow().objects[&ObjectId(2)]
                .state
                .contains_key("queue")
        );
    }
    #[test]
    fn background_reports_are_bounded_and_do_not_split_graphemes_or_markup() {
        let p = crate::text::Palette::default();
        let doc = reply_document(&p, &format!("[fg=red]{}[/]", "👩‍🚀".repeat(10)), 30);
        assert!(doc.len() <= 30);
        assert_eq!(doc.source(), "👩‍🚀 [truncated]");
        assert!(matches!(doc, crate::text::Document::Literal(_)));
        assert!(reply_document(&p, "too long", 1).len() <= 1);
    }
}
