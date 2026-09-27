//! Durable world commits and ordered publication of runtime effects.

use super::*;

impl Server {
    /// Validate and persist the live world, or restore `before` and roll back every
    /// staged effect. Only rows that differ from the stored world are written.
    pub(super) async fn commit(&mut self, before: World) -> bool {
        let mut after = self.scripts.world.borrow().clone();
        let maintenance = self.scripts.effects.maintenance();
        let requested = maintenance.is_some() || self.scripts.effects.save_requested();
        // Maintenance rewrites rows from the stored world, so it leaves no trusted baseline.
        let trusted = maintenance.is_none();
        let durable = self.durable.take();
        let result = match self
            .scripts
            .effects
            .validate()
            .map_err(|e| anyhow::anyhow!("{e}"))
            .and_then(|()| after.validate(&self.config))
        {
            Err(e) => Err(e),
            // A transaction that changed nothing never touches the database.
            Ok(()) if !requested && after.saved_state_eq(&before) => {
                self.durable = durable;
                return self.committed(false);
            }
            Ok(()) => {
                let interval = self.config.database.clock_save_interval;
                match self.database().await {
                    Err(e) => Err(e),
                    Ok(database) => {
                        database
                            .persist_effects(&after, maintenance, durable.as_ref(), interval)
                            .await
                    }
                }
            }
        };
        match result {
            Err(e) => {
                self.database = None;
                self.config.log(
                    &[
                        crate::logging::Category::Checkpoints,
                        crate::logging::Category::Problems,
                    ],
                    "DB",
                    "CHECK",
                    format!("Persistence failed: {e:#}"),
                );
                if self.shutdown.is_some() {
                    self.shutdown_failed = true;
                }
                *self.scripts.world.borrow_mut() = before;
                self.reconcile_connections();
                self.scripts.effects.rollback();
                false
            }
            Ok(saved) => {
                // The stored containment order is now this, for the next save's diff and
                // for commands that list objects in stored order.
                after.links = saved.links.clone();
                self.scripts.world.borrow_mut().links = saved.links;
                self.durable = trusted.then_some(after);
                self.committed(saved.changed || requested)
            }
        }
    }

    /// The write connection, opened on first use and kept for the rest of the run.
    pub(super) async fn database(&mut self) -> anyhow::Result<&mut persistence::Database> {
        if self.database.is_none() {
            self.database = Some(
                persistence::Database::open(
                    &self.config.database(),
                    self.config.database.busy_timeout_ms,
                )
                .await?,
            );
        }
        Ok(self.database.as_mut().expect("database just opened"))
    }

    /// Take the write connection out of the server for a write whose callback needs the
    /// rest of the server; put it back with [`Self::restore_database`] on success.
    pub(super) async fn take_database(&mut self) -> anyhow::Result<persistence::Database> {
        self.database().await?;
        Ok(self.database.take().expect("database just opened"))
    }

    /// Keep a connection taken with [`Self::take_database`] when its write succeeded.
    pub(super) fn restore_database<T>(
        &mut self,
        database: persistence::Database,
        result: &anyhow::Result<T>,
    ) {
        if result.is_ok() {
            self.database = Some(database);
        }
    }

    /// Finish a successful commit, logging a save when rows were written or requested.
    fn committed(&mut self, saved: bool) -> bool {
        self.finish_maintenance();
        if saved {
            self.config.log(
                &[crate::logging::Category::Checkpoints],
                "DB",
                "SAVE",
                "World changes committed.",
            );
        }
        true
    }
    /// Apply session effects only after the maintenance transaction is durable.
    pub(super) fn finish_maintenance(&mut self) {
        let Some(report) = self.scripts.effects.drain_maintenance() else {
            return;
        };
        for finding in report.findings {
            self.config.log(
                &[
                    crate::logging::Category::Checkpoints,
                    crate::logging::Category::Problems,
                ],
                "DB",
                "CHECK",
                finding,
            );
        }
        let ids: Vec<_> = self
            .sessions
            .iter()
            .filter(|(_, s)| {
                s.player
                    .is_some_and(|p| report.plan.detachments.contains(&p))
            })
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.tell(id, "Your character has been destroyed.\r\n");
            if let Some(session) = self.sessions.remove(&id) {
                session.close();
            }
            self.scripts.flows.cancel(id.0);
        }
        self.reconcile_connections();
        self.command_queue.reconcile(&self.scripts.world.borrow());
    }
    pub(super) fn flush(&self) {
        for record in self.scripts.effects.drain_records() {
            self.config.logger.record(&self.config, record);
        }
        for request in self.scripts.effects.drain_logs() {
            self.config.logger.submit(&self.config, request);
        }
        for request in self.scripts.effects.drain_map_writes() {
            let message = match request.publish(&self.config) {
                Ok(()) => "Saving complete!".to_owned(),
                Err(error) => format!("Unable to finish saving the map file: {error:#}"),
            };
            self.scripts
                .outbox
                .borrow_mut()
                .push((request.actor, message.into()));
        }
        self.scripts.effects.commit();
        let mut private = self.scripts.effects.drain_private().into_iter().peekable();
        let messages = std::mem::take(&mut *self.scripts.outbox.borrow_mut());
        let count = messages.len();
        for (index, (p, text)) in messages.into_iter().enumerate() {
            while private.peek().is_some_and(|output| output.after <= index) {
                self.flow_output(private.next().unwrap());
            }
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
        for output in private {
            debug_assert!(output.after <= count);
            self.flow_output(output);
        }
    }
}
