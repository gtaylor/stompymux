//! Blocking help index reads and atomic help reload publication.

use super::*;

impl Server {
    pub(super) async fn help(&self, id: SessionId, actor: ObjectId, topic: String) {
        let wizard = crate::authority::is_wizard(&self.scripts.world.borrow(), actor);
        let index = self.scripts.help.clone();
        let response = tokio::task::spawn_blocking(move || index.lookup(&topic, wizard)).await;
        match response {
            Ok(Ok(response)) => {
                let ansi = self.scripts.world.borrow().objects[&actor]
                    .flags
                    .contains(crate::flags::Flag::Ansi);
                if let Some(session) = self.sessions.get(&id)
                    && let Err(error) = session.help(&response, ansi, &self.config).await
                {
                    tracing::error!("Help rendering: {error:#}");
                    self.tell(
                        id,
                        "Unable to render help article. See server diagnostics.\r\n",
                    );
                }
            }
            error => {
                tracing::error!("Help read: {error:?}");
                self.tell(
                    id,
                    "Unable to render help article. See server diagnostics.\r\n",
                );
            }
        }
    }

    pub(super) async fn help_reload(&mut self, id: SessionId) {
        let config = self.config.clone();
        match tokio::task::spawn_blocking(move || crate::help::HelpIndex::reload(&config)).await {
            Ok(Ok(index)) => {
                index.report.log();
                let report = &index.report;
                let mut lines: Vec<_> = report
                    .errors
                    .iter()
                    .chain(&report.warnings)
                    .cloned()
                    .collect();
                lines.push(report.summary());
                let response = crate::help::HelpResponse::Message(lines.join("\n"));
                self.scripts.help = index;
                if let Some(session) = self.sessions.get(&id)
                    && let Err(error) = session.help(&response, false, &self.config).await
                {
                    tracing::warn!("Help reload diagnostics: {error:#}");
                    self.tell(
                        id,
                        "Help reindexed; see server diagnostics for details.\r\n",
                    );
                }
            }
            error => {
                tracing::error!("Help reload failed: {error:?}");
                self.tell(
                    id,
                    "Help reload failed; previous index retained. See server diagnostics.\r\n",
                );
            }
        }
    }
}
