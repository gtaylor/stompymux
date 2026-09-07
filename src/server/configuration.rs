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
            for bucket in self.addresses.values_mut() {
                bucket.tokens = bucket
                    .tokens
                    .min(candidate.config.security.login_attempt_burst);
            }
            self.hashes.tokens = self
                .hashes
                .tokens
                .min(candidate.config.security.login_hash_limit);
            self.config = candidate.config;
            Ok(candidate.diagnostics)
        })();
        match result {
            Ok(mut messages) => {
                eprintln!(
                    "Configuration: #{} edited {}: {}",
                    player.0,
                    request.directive,
                    if messages.is_empty() {
                        "Success"
                    } else {
                        "Partial success"
                    }
                );
                messages.push("Set.".into());
                messages.join("\n")
            }
            Err(error) => {
                eprintln!(
                    "Configuration: #{} {} failed: {error:#}",
                    player.0, request.directive
                );
                error.to_string()
            }
        }
    }
}
