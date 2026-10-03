//! Persistence transactions, scheduled work, cache reloads, and database checks.
use super::*;

impl Server {
    /// Each job is consumed once and owns its commit-before-output transaction.
    pub(super) async fn scheduled(&mut self, job: crate::lua::schedules::Job) {
        if self.shutdown.is_some() {
            return;
        }
        let before = self.scripts.world.borrow().clone();
        match self.scripts.run_schedule(&job) {
            Ok(true) => {
                if self.commit(before).await {
                    self.flush();
                } else {
                    tracing::error!("Lua schedule persistence failed: {}", job.description());
                }
            }
            Ok(false) => {}
            Err(error) => {
                *self.scripts.world.borrow_mut() = before;
                self.reconcile_connections();
                self.scripts.effects.rollback();
                tracing::error!("Lua schedule failed: {error:#}");
            }
        }
    }

    /// Flow input owns one world transaction; only persistence failure permits retry.
    pub(super) async fn flow_input(&mut self, id: SessionId, input: &str) {
        let before = self.scripts.world.borrow().clone();
        let result = self
            .scripts
            .flow_input(id.0, input)
            .and_then(|()| self.scripts.world.borrow().validate(&self.config));
        match result {
            Ok(()) => {
                if self.commit(before).await {
                    self.flush();
                } else {
                    self.tell(id, "Unable to save your changes. Please try again.\r\n");
                }
            }
            Err(error) => {
                *self.scripts.world.borrow_mut() = before;
                self.scripts.effects.rollback();
                self.scripts.flows.cancel(id.0);
                self.reconcile_connections();
                tracing::error!(session = id.0, "interactive flow failed: {error:#}");
                self.tell(id, "Interactive flow failed and was cancelled.\r\n");
            }
        }
    }
    pub(super) async fn check_idle(&mut self) -> Result<()> {
        let expired: Vec<_> = self
            .sessions
            .iter()
            .filter_map(|(id, session)| {
                self.controls
                    .timeout(
                        &self.scripts.world.borrow(),
                        session.player,
                        session.connected.elapsed(),
                        session.active.elapsed(),
                        self.config.mux.conn_timeout,
                        self.config.mux.idle_timeout,
                    )
                    .map(|reason| (*id, reason))
            })
            .collect();
        for (id, reason) in expired {
            self.tell(id, reason);
            self.disconnect(id).await?;
        }
        Ok(())
    }
    pub(super) fn global_control(
        &mut self,
        value: Option<(crate::controls::Control, bool)>,
    ) -> String {
        if let Some((control, enabled)) = value {
            if control == crate::controls::Control::IdleChecking
                && enabled
                && !self.controls.enabled(control)
            {
                self.idle_recheck = true;
            }
            self.controls.set(control, enabled);
            self.scripts
                .queue_enabled
                .set(self.controls.enabled(crate::controls::Control::Queueing));
            if control == crate::controls::Control::Cleaning {
                self.cleaning.enabled = enabled;
            }
            return if enabled { "Enabled." } else { "Disabled." }.into();
        }
        self.controls.status()
    }
    /// Stage repair callbacks under the database transaction, detaching destroyed players only after commit.
    pub(super) async fn dbck(&mut self, origin: crate::cleaning::CheckOrigin) {
        use crate::cleaning::CheckOrigin;
        let (session, actor, cause) = match origin {
            CheckOrigin::Interactive {
                session,
                actor,
                cause,
            } => (Some(session), actor, cause),
            CheckOrigin::Queued { actor, cause } => (None, actor, cause),
            CheckOrigin::Automatic => (None, ObjectId(1), ObjectId(1)),
        };
        let automatic = matches!(origin, CheckOrigin::Automatic);
        let destination =
            session.map_or(ReplyDestination::Object(actor), ReplyDestination::Session);
        if let Err(error) = self.snapshots() {
            tracing::error!("DBCK session snapshot failed: {error:#}");
            if !automatic {
                self.queue_reply(
                    destination,
                    "Database check failed; no repairs committed. See server diagnostics.",
                );
                self.flush();
            }
            return;
        }
        let before = self.scripts.world.borrow().clone();
        self.durable = None;
        let result = match self.take_database().await {
            Err(error) => Err(error),
            Ok(mut database) => {
                let result = database
                    .repair(|raw| {
                        crate::lua::transactions::with_cause(&self.scripts.lua, cause, || {
                            let (repaired, mut report) =
                                crate::dbck::plan(&before, raw, &self.config)?;
                            *self.scripts.world.borrow_mut() = repaired;
                            crate::lua::maintenance::apply_relocations(
                                &self.scripts,
                                &before,
                                &report,
                                |object| {
                                    session.and_then(|_| {
                                        self.sessions
                                            .iter()
                                            .find(|(_, s)| s.player == Some(object))
                                            .map(|(id, _)| id.0)
                                    })
                                },
                            )?;
                            if let Some(nested) = self.scripts.effects.maintenance() {
                                report.plan.purges.extend(nested.plan.purges);
                                report.plan.detachments.extend(nested.plan.detachments);
                                report.findings.extend(nested.findings);
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
                                        && o.flags
                                            == [crate::flags::Flag::Going].into_iter().collect()
                                        && o.powers == Default::default()
                                        && o.state.is_empty()
                                        && !after.accounts.contains_key(id),
                                    "callback changed purged object #{}",
                                    id.0
                                );
                            }
                            report.plan.links =
                                crate::dbck::rebuild_links(&after, &report.plan.links);
                            report.plan.list_changes = report
                                .plan
                                .links
                                .iter()
                                .filter(|(id, links)| raw.get(id) != Some(*links))
                                .map(|(id, _)| *id)
                                .collect();
                            Ok((after, report))
                        })
                    })
                    .await;
                self.restore_database(database, &result);
                result
            }
        };
        match result {
            Ok((report, links)) => {
                self.scripts.world.borrow_mut().links = links;
                self.scripts.effects.drain_maintenance();
                for finding in &report.findings {
                    tracing::warn!("database check: {finding}");
                }
                let mut transitions = Vec::new();
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
                        if let Some(player) = session.player {
                            let partial = self.sessions.values().any(|s| s.player == Some(player));
                            transitions.push(presence::Transition::capture(
                                &before,
                                player,
                                if partial {
                                    TransitionKind::PartialDisconnect
                                } else {
                                    TransitionKind::Disconnected
                                },
                                session.peer,
                                session.site,
                            ));
                        }
                    }
                }
                self.reconcile_connections();
                self.flush();
                for transition in transitions {
                    self.announce_transition(transition).await;
                }
                self.command_queue.reconcile(&self.scripts.world.borrow());
                if let Some(session) = session.and_then(|id| self.sessions.get(&id)) {
                    session.raw(report.response(self.config.runtime.output_message_limit));
                } else if automatic {
                    tracing::info!("Automatic {}", report.summary());
                } else {
                    self.queue_reply(ReplyDestination::Object(actor), &report.summary());
                    self.flush();
                }
            }
            Err(e) => {
                *self.scripts.world.borrow_mut() = before;
                self.reconcile_connections();
                self.scripts.effects.rollback();
                tracing::error!("DBCK rolled back: {e:#}");
                if !automatic {
                    self.queue_reply(
                        destination,
                        "Database check failed; no repairs committed. See server diagnostics.",
                    );
                }
                self.flush();
            }
        }
    }
}
