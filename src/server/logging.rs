//! Nontransactional command audits and explicit file-write acknowledgments.
use super::*;
use crate::logging::{FileRequest, targets};
impl Server {
    pub(super) async fn write_log(&self, request: FileRequest) -> String {
        match self.config.logger.write(&self.config, request).await {
            Ok(()) => "Message logged.".into(),
            Err(e) => {
                tracing::error!(error = %format_args!("{e:#}"), "@log write failed");
                "Request failed.".into()
            }
        }
    }
    /// Record an attempt before command callbacks; channel auditing owns a separate transaction.
    pub(super) async fn audit(&mut self, execution: commands::ExecutionContext, line: &str) {
        let (message, suspect) = {
            let w = self.scripts.world.borrow();
            (
                crate::logging::audit::message(&self.config, &w, execution, line),
                w.objects
                    .get(&execution.executor)
                    .is_some_and(|o| o.flags.contains(crate::flags::Flag::Suspect)),
            )
        };
        if suspect {
            tracing::info!(target: targets::SUSPECT_COMMANDS, "{message}");
        } else {
            tracing::info!(target: targets::COMMANDS, "{message}");
        }
        if !suspect
            || self
                .scripts
                .communication(&self.config)
                .name("SuspectsLog")
                .is_err()
        {
            return;
        }
        let before = self.scripts.world.borrow().clone();
        let result = self.scripts.communication(&self.config).emit(
            "SuspectsLog",
            &crate::text::escape(&message),
            false,
        );
        if let Err(e) = result {
            *self.scripts.world.borrow_mut() = before;
            self.reconcile_connections();
            self.scripts.effects.rollback();
            tracing::error!(error = %e, "suspect audit failed");
            return;
        }
        if self.commit(before).await {
            self.flush();
        }
    }
}
