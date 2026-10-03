//! Publish validated runtime edits at the world-loop command boundary.
use super::*;
impl Server {
    /// No callbacks, database operations or awaits occur while publishing an edit.
    pub(super) fn configure(
        &mut self,
        player: ObjectId,
        request: crate::config::administration::Request,
    ) -> String {
        let result = (|| -> Result<Vec<String>> {
            let candidate = self.config.administer(
                &request,
                &self.scripts.world.borrow(),
                player,
                &self.scripts.commands,
            )?;
            self.scripts.configure(&candidate.config)?;
            self.cleaning
                .set_interval(candidate.config.mux.check_interval as u64);
            for session in self.sessions.values_mut() {
                session.quota = session.quota.min(candidate.config.mux.command_quota_max);
            }
            for bucket in self.authentication.addresses.values_mut() {
                bucket.tokens = bucket
                    .tokens
                    .min(candidate.config.security.login_attempt_burst);
            }
            self.authentication.hashes.tokens = self
                .authentication
                .hashes
                .tokens
                .min(candidate.config.security.login_hash_limit);
            if request.directive == "log_filter" {
                crate::logging::apply_filter(&candidate.config)?;
            }
            self.config = candidate.config;
            Ok(candidate.diagnostics)
        })();
        match result {
            Ok(mut messages) => {
                tracing::info!(
                    target: crate::logging::targets::CONFIG,
                    player = player.0,
                    directive = %request.directive,
                    partial = !messages.is_empty(),
                    "configuration edited"
                );
                messages.push("Set.".into());
                messages.join("\n")
            }
            Err(error) => {
                tracing::info!(
                    target: crate::logging::targets::CONFIG,
                    player = player.0,
                    directive = %request.directive,
                    error = %format_args!("{error:#}"), "configuration edit failed"
                );
                error.to_string()
            }
        }
    }
}
