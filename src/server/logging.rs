//! Nontransactional command audits and explicit file-write acknowledgments.
use super::*;
use crate::logging::{Category, FileRequest};
impl Server {
    pub(super) async fn write_log(&self, request: FileRequest) -> String {
        match self.config.logger.write(&self.config, request).await {
            Ok(()) => "Message logged.".into(),
            Err(e) => {
                self.config.log(
                    crate::logging::LogLevel::Error,
                    &[Category::Problems],
                    "LOG",
                    "WRITE",
                    e.to_string(),
                );
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
        let categories = if suspect {
            &[Category::AllCommands, Category::SuspectCommands][..]
        } else {
            &[Category::AllCommands][..]
        };
        self.config.log(
            crate::logging::LogLevel::Info,
            categories,
            "CMD",
            if suspect { "SUS" } else { "ALL" },
            &message,
        );
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
            self.config.log(
                crate::logging::LogLevel::Error,
                &[Category::Problems],
                "CMD",
                "SUS",
                format!("Suspect audit failed: {e}"),
            );
            return;
        }
        if self.commit(before).await {
            self.flush();
        }
    }
}
