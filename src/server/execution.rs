//! Shared orchestration for interactive and queued command attempts.

use super::*;
use crate::commands::queue::Work;
use crate::commands::{Report, ServerRequest};

/// Where command-owned replies are delivered after dispatch.
#[derive(Clone, Copy)]
pub(super) enum ReplyDestination {
    /// A private response to the invoking connection.
    Session(SessionId),
    /// Ordinary object notification for descriptor-free queue work.
    Object(ObjectId),
}

impl ReplyDestination {
    fn session(self) -> Option<SessionId> {
        match self {
            Self::Session(id) => Some(id),
            Self::Object(_) => None,
        }
    }

    fn is_queued(self) -> bool {
        matches!(self, Self::Object(_))
    }
}

impl Server {
    pub(super) async fn command(
        &mut self,
        id: SessionId,
        player: ObjectId,
        line: &str,
    ) -> Result<()> {
        let execution = commands::ExecutionContext {
            executor: player,
            cause: player,
            session: Some(id.0),
            origin: commands::InputOrigin::Interactive,
        };
        self.execute_command(execution, line, ReplyDestination::Session(id))
            .await
    }

    /// Consume one ready queued attempt without borrowing a connection.
    pub(super) async fn queued(&mut self, work: Work) {
        if self.shutdown.is_some() {
            return;
        }
        if let Err(error) = self
            .execute_command(
                work.execution,
                &work.text,
                ReplyDestination::Object(work.execution.executor),
            )
            .await
        {
            self.config.log(
                &[crate::logging::Category::Problems],
                "SRV",
                "ERROR",
                format!("Queued command snapshot: {error:#}"),
            );
        }
    }

    /// Snapshot, audit, dispatch, commit callbacks, and classify every command outcome.
    async fn execute_command(
        &mut self,
        execution: commands::ExecutionContext,
        line: &str,
        destination: ReplyDestination,
    ) -> Result<()> {
        self.snapshots()?;
        if commands::executable(&self.scripts.world.borrow(), execution) {
            self.audit(execution, line).await;
        }
        let mut before = self.scripts.world.borrow().clone();
        let action = commands::execute(&self.scripts, &self.config, execution, line);
        if action.is_ok()
            && self.scripts.command_callbacks_invoked()
            && !matches!(
                &action,
                Ok(Action::Continue | Action::CommitReply(_) | Action::Queue(_))
            )
        {
            if !self.commit(before.clone()).await {
                self.persistence_failure(destination);
                return Ok(());
            }
            self.flush();
            before = self.scripts.world.borrow().clone();
        }
        let action = match action {
            Ok(action) => action,
            Err(error) => {
                self.rollback_command(before);
                self.command_error(execution, destination, &error);
                return Ok(());
            }
        };
        self.complete_action(execution, destination, before, action)
            .await
    }

    async fn complete_action(
        &mut self,
        execution: commands::ExecutionContext,
        destination: ReplyDestination,
        before: World,
        action: Action,
    ) -> Result<()> {
        let actor = execution.executor;
        match action {
            Action::Continue => {
                if self.commit(before).await {
                    self.flush();
                } else {
                    self.persistence_failure(destination);
                }
            }
            Action::CommitReply(text) => {
                if self.commit(before).await {
                    self.command_reply(destination, &text);
                    self.flush_for(destination);
                } else {
                    self.persistence_failure(destination);
                }
            }
            Action::Queue(request) => {
                self.queue_request(destination, actor, request, before)
                    .await
            }
            Action::Report(report) => self.command_report(destination, actor, report).await,
            Action::Server(request) => {
                self.server_request(execution, destination, before, request)
                    .await?
            }
        }
        Ok(())
    }

    async fn command_report(
        &mut self,
        destination: ReplyDestination,
        actor: ObjectId,
        report: Report,
    ) {
        let Some(id) = destination.session() else {
            let text = match report {
                Report::Reply(text)
                | Report::Inspection(text)
                | Report::Literal(text)
                | Report::Styled(text) => text,
            };
            self.command_reply(destination, &text);
            self.flush();
            return;
        };
        match report {
            Report::Reply(text) => {
                if let Some(session) = self.sessions.get(&id) {
                    session.raw(crate::telnet::bounded_error(
                        &text,
                        self.config.runtime.output_message_limit,
                    ));
                }
            }
            Report::Inspection(text) => self.inspection_report(id, text).await,
            Report::Literal(text) => {
                if let Some(session) = self.sessions.get(&id)
                    && let Err(error) = session.literal_report(&text, &self.config).await
                {
                    self.report_delivery_error(session, error);
                }
            }
            Report::Styled(text) => {
                if let Some(session) = self.sessions.get(&id) {
                    let ansi = self
                        .scripts
                        .world
                        .borrow()
                        .objects
                        .get(&actor)
                        .is_some_and(|object| object.flags.contains(crate::flags::Flag::Ansi));
                    if let Err(error) = session.styled_report(&text, ansi, &self.config).await {
                        self.report_delivery_error(session, error);
                    }
                }
            }
        }
    }

    async fn server_request(
        &mut self,
        execution: commands::ExecutionContext,
        destination: ReplyDestination,
        before: World,
        request: ServerRequest,
    ) -> Result<()> {
        let actor = execution.executor;
        match request {
            ServerRequest::AccountAdmin(request) => {
                let Some(id) = self.require_session(destination, before) else {
                    return Ok(());
                };
                self.account_admin(id, actor, request).await?;
            }
            ServerRequest::Color(mode) => {
                let Some(id) = self.require_session(destination, before) else {
                    return Ok(());
                };
                self.color(id, &mode);
            }
            ServerRequest::Help(topic) => {
                let Some(id) = self.require_session(destination, before) else {
                    return Ok(());
                };
                self.help(id, actor, topic).await;
            }
            ServerRequest::HelpReload => {
                let Some(id) = self.require_session(destination, before) else {
                    return Ok(());
                };
                self.help_reload(id).await;
            }
            ServerRequest::Sessions(prefix) => {
                let Some(id) = self.require_session(destination, before) else {
                    return Ok(());
                };
                self.session_diagnostics(id, &prefix);
            }
            ServerRequest::Who(prefix) => {
                let Some(id) = self.require_session(destination, before) else {
                    return Ok(());
                };
                let text = self.who_report(&prefix);
                self.operation_report(id, &text).await;
            }
            ServerRequest::Telnet(player) => {
                let Some(id) = self.require_session(destination, before) else {
                    return Ok(());
                };
                self.telnet_diagnostics(id, &player);
            }
            ServerRequest::LuaSchedules(target) => {
                let Some(id) = self.require_session(destination, before) else {
                    return Ok(());
                };
                let bytes = self.scripts.schedules.inspect(
                    &self.scripts.world.borrow(),
                    actor,
                    &target,
                    self.config.runtime.output_message_limit,
                );
                if let Some(session) = self.sessions.get(&id) {
                    session.raw(bytes);
                }
            }
            ServerRequest::LuaAdmin(request) => {
                let Some(id) = self.require_session(destination, before) else {
                    return Ok(());
                };
                self.lua_admin(id, request).await;
            }
            ServerRequest::Quit => {
                let Some(id) = self.require_session(destination, before) else {
                    return Ok(());
                };
                self.cache_close(id, crate::message_cache::File::Quit, "", "Goodbye.")
                    .await?;
            }
            ServerRequest::ProcessReport => {
                let text = crate::operations::process_report(&self.config).await;
                if let Some(id) = destination.session() {
                    self.operation_report(id, &text).await;
                } else {
                    self.command_reply(destination, &text);
                    self.flush();
                }
            }
            ServerRequest::ExamineDebug(object) => {
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
                    Ok(text) => {
                        if let Some(id) = destination.session() {
                            self.inspection_report(id, text).await;
                        } else {
                            self.command_reply(destination, &text);
                            self.flush();
                        }
                    }
                    Err(error) => {
                        self.config.log(
                            &[crate::logging::Category::Problems],
                            "SRV",
                            "ERROR",
                            if destination.is_queued() {
                                format!("Queued examination: {error:#}")
                            } else {
                                format!("Debug examination: {error:#}")
                            },
                        );
                        self.command_reply(destination, "Unable to read object bookkeeping.");
                        self.flush_for(destination);
                    }
                }
            }
            ServerRequest::Shutdown => self.request_shutdown(ShutdownRequest::Player(actor)).await,
            ServerRequest::DbCheck => {
                let origin = match destination {
                    ReplyDestination::Session(session) => {
                        crate::cleaning::CheckOrigin::Interactive {
                            session,
                            actor,
                            cause: execution.cause,
                        }
                    }
                    ReplyDestination::Object(_) => crate::cleaning::CheckOrigin::Queued {
                        actor,
                        cause: execution.cause,
                    },
                };
                self.dbck(origin).await;
            }
            ServerRequest::ConfigAdmin(request) => {
                let text = self.configure(actor, request);
                if let Some(id) = destination.session() {
                    self.inspection_report(id, text).await;
                } else {
                    self.command_reply(destination, &text);
                    self.flush();
                }
            }
            ServerRequest::Log(request) => {
                let text = self.write_log(request).await;
                if let Some(id) = destination.session() {
                    self.inspection_report(id, text).await;
                } else {
                    self.command_reply(destination, &text);
                    self.flush();
                }
            }
            ServerRequest::ReadCache => self.readcache(destination.session(), actor).await,
            ServerRequest::GlobalControl(value) => {
                let response = self.global_control(value);
                self.command_reply(destination, &response);
                self.flush_for(destination);
            }
        }
        Ok(())
    }

    fn require_session(
        &mut self,
        destination: ReplyDestination,
        before: World,
    ) -> Option<SessionId> {
        if let Some(id) = destination.session() {
            return Some(id);
        }
        self.rollback_command(before);
        self.command_reply(destination, "This command requires an interactive session.");
        self.flush();
        None
    }

    fn rollback_command(&mut self, before: World) {
        *self.scripts.world.borrow_mut() = before;
        self.reconcile_connections();
        self.scripts.effects.rollback();
    }

    fn command_reply(&self, destination: ReplyDestination, text: &str) {
        match destination {
            ReplyDestination::Session(id) => {
                if let Some(session) = self.sessions.get(&id) {
                    session.raw(crate::telnet::bounded_error(
                        text,
                        self.config.runtime.output_message_limit,
                    ));
                }
            }
            ReplyDestination::Object(actor) => {
                self.queue_reply(ReplyDestination::Object(actor), text)
            }
        }
    }

    fn persistence_failure(&self, destination: ReplyDestination) {
        match destination {
            ReplyDestination::Session(id) => {
                self.tell(id, "Unable to save your changes. Please try again.\r\n")
            }
            ReplyDestination::Object(actor) => {
                self.queue_reply(
                    ReplyDestination::Object(actor),
                    "Unable to save your changes. Please try again.",
                );
                self.flush();
            }
        }
    }

    fn flush_for(&self, destination: ReplyDestination) {
        if destination.is_queued() {
            self.flush();
        }
    }

    fn command_error(
        &self,
        execution: commands::ExecutionContext,
        destination: ReplyDestination,
        error: &anyhow::Error,
    ) {
        if let ReplyDestination::Object(actor) = destination {
            self.config.log(
                &[crate::logging::Category::Problems],
                "SRV",
                "ERROR",
                format!(
                    "Queued command for #{} (cause #{}): {error:#}",
                    actor.0, execution.cause.0
                ),
            );
            self.queue_reply(
                ReplyDestination::Object(actor),
                "That queued command could not be completed.",
            );
            self.flush();
            return;
        }
        self.config.log(
            &[crate::logging::Category::Bugs],
            "LUA",
            "ERROR",
            format!("Command callback failed: {error:#}"),
        );
        let actor = execution.executor;
        let report = self.config.lua.error_reporting;
        let wizard = self.scripts.world.borrow().objects[&actor]
            .flags
            .contains(crate::flags::Flag::Wizard);
        let id = destination.session().unwrap();
        if report == crate::config::ErrorReporting::All
            || (report == crate::config::ErrorReporting::Wizards && wizard)
        {
            self.tell(id, &format!("Lua error: {error}\r\n"));
        } else {
            self.tell(id, "That command could not be completed.\r\n");
        }
    }

    fn report_delivery_error(&self, session: &Session, error: anyhow::Error) {
        self.config.log(
            &[crate::logging::Category::Network],
            "NET",
            "ERROR",
            format!("Report delivery: {error:#}"),
        );
        session.raw(crate::telnet::bounded_error(
            "Unable to deliver complete report.",
            self.config.runtime.output_message_limit,
        ));
    }
}
