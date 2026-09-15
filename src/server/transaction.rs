//! Durable world commits and ordered publication of runtime effects.

use super::*;

impl Server {
    pub(super) async fn commit(&mut self, before: World) -> bool {
        let after = self.scripts.world.borrow().clone();
        let mut saved = false;
        let result = match self
            .scripts
            .effects
            .validate()
            .map_err(|e| anyhow::anyhow!("{e}"))
            .and_then(|()| after.validate(&self.config))
        {
            Err(e) => Err(e),
            Ok(()) => match (serde_json::to_vec(&before), serde_json::to_vec(&after)) {
                (Ok(a), Ok(b))
                    if a == b
                        && self.scripts.effects.maintenance().is_none()
                        && !self.scripts.effects.save_requested() =>
                {
                    Ok(())
                }
                _ => {
                    saved = true;
                    persistence::persist_effects(
                        self.config.database(),
                        after,
                        self.config.database.busy_timeout_ms,
                        self.scripts.effects.maintenance(),
                    )
                    .await
                }
            },
        };
        if let Err(e) = result {
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
        } else {
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
