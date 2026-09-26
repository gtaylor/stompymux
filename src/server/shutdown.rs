//! Graceful shutdown acceptance, persistence, and session cleanup.
use super::*;

impl Server {
    /// Validate and save before accepting command shutdown; signals proceed on failure.
    pub(super) async fn request_shutdown(&mut self, request: ShutdownRequest) {
        if self.shutdown.is_some() {
            return;
        }
        self.durable = None;
        let snapshot = self.scripts.world.borrow().clone();
        let result = match snapshot.validate(&self.config) {
            Ok(()) => {
                persistence::persist(
                    self.config.database(),
                    snapshot,
                    self.config.database.busy_timeout_ms,
                )
                .await
            }
            Err(e) => Err(e),
        };
        if let Err(e) = result {
            self.config.log(
                &[crate::logging::Category::Problems],
                "SHT",
                "ERROR",
                format!("Shutdown initial save failed: {e:#}"),
            );
            if let ShutdownRequest::Player(player) = request {
                for (id, session) in &self.sessions {
                    if session.player == Some(player) {
                        self.tell(*id, "Shutdown cancelled: unable to save the database.\r\n");
                    }
                }
                return;
            }
            self.shutdown_failed = true;
        }
        self.shutdown = Some(request);
        self.scripts.flows.stop();
        self.config.log(
            &[crate::logging::Category::Startup],
            "SHT",
            "START",
            format!("Graceful shutdown: {request:?}"),
        );
        if let ShutdownRequest::Player(player) = request {
            let name = self.scripts.world.borrow().objects[&player].name.clone();
            for id in self.sessions.keys() {
                self.tell(*id, &format!("Game: Shutdown by {name}\r\n"));
            }
        }
    }
    /// The sole shutdown cleanup path, after acceptance and before task draining.
    pub(super) async fn finish_shutdown(&mut self) {
        self.scripts.flows.stop();
        for id in self.sessions.keys().copied().collect::<Vec<_>>() {
            if let Err(e) = self.disconnect(id).await {
                self.config.log(
                    &[crate::logging::Category::Problems],
                    "SHT",
                    "ERROR",
                    format!("Shutdown disconnect failed: {e:#}"),
                );
                self.shutdown_failed = true;
            }
        }
        self.durable = None;
        let snapshot = self.scripts.world.borrow().clone();
        let result = match snapshot.validate(&self.config) {
            Ok(()) => {
                persistence::persist(
                    self.config.database(),
                    snapshot,
                    self.config.database.busy_timeout_ms,
                )
                .await
            }
            Err(e) => Err(e),
        };
        if let Err(e) = result {
            self.config.log(
                &[crate::logging::Category::Problems],
                "SHT",
                "ERROR",
                format!("Shutdown final save failed: {e:#}"),
            );
            self.shutdown_failed = true;
        }
        if let Some(anchor) = self.database_anchor.take()
            && let Err(e) = anchor.close().await
        {
            self.config.log(
                &[crate::logging::Category::Problems],
                "SHT",
                "ERROR",
                format!("Closing the database failed: {e:#}"),
            );
        }
    }
}
