//! Transactional publication of Lua source and inspection requests.
use super::*;

impl Server {
    /// Build candidates separately; publish code only after candidate state is durable.
    pub(super) async fn lua_admin(&mut self, id: SessionId, request: crate::lua::AdminRequest) {
        use crate::lua::{AdminRequest, RuntimeMode, sources::Sources};
        if let AdminRequest::Test(request) = request {
            let result = async {
                let config = self.config.clone();
                let sources = tokio::task::spawn_blocking(move || {
                    Sources::read(&config)?.with_tests(&config)
                })
                .await??;
                let mut scripts = Scripts::from_sources_with(
                    &self.config,
                    self.scripts.world.clone(),
                    self.scripts.help.clone(),
                    std::sync::Arc::new(sources),
                    RuntimeMode::Testing,
                    |candidate| self.snapshots_for(candidate),
                )?;
                scripts.host_effects(&self.scripts);
                let test_config = self.config.clone();
                crate::lua::testing::run(&scripts, &test_config, &request, || {
                    self.finish_maintenance();
                    self.scripts
                        .outbox
                        .borrow_mut()
                        .extend(scripts.outbox.borrow_mut().drain(..));
                    self.flush();
                })
                .await
            }
            .await;
            self.reconcile_connections();
            let text = match result {
                Ok(report) => report.render(request.verbose),
                Err(error) => format!("Lua tests could not start: {error:#}"),
            };
            self.inspection_report(id, text).await;
            return;
        }
        if let AdminRequest::View { path, object } = request {
            let config = self.config.clone();
            let label = path.clone();
            let result = tokio::task::spawn_blocking(move || Sources::view(&config, &path)).await;
            let text = match result {
                Ok(Ok(source)) => format!(
                    "Lua parent object_logic/{label}{}:\nCurrent disk source; active code changes only after reload.\n{source}\n-- End Lua parent --",
                    object.map_or(String::new(), |o| format!(" (attached on #{})", o.0))
                ),
                error => format!("Lua parent unavailable: {error:?}"),
            };
            if let Some(session) = self.sessions.get(&id)
                && let Err(error) = session.literal_report(&text, &self.config).await
            {
                self.config.log(
                    &[crate::logging::Category::Bugs],
                    "LUA",
                    "ERROR",
                    format!("Lua source output: {error:#}"),
                );
                self.tell(id, "Unable to deliver complete Lua source.\r\n");
            }
            return;
        }
        let checking = matches!(request, AdminRequest::Check);
        let result = async {
            let config = self.config.clone();
            let sources = tokio::task::spawn_blocking(move || {
                let sources = Sources::read(&config)?;
                if checking {
                    sources.with_tests(&config)
                } else {
                    Ok(sources)
                }
            })
            .await??;
            let before = self.scripts.world.borrow().clone();
            let world = std::rc::Rc::new(std::cell::RefCell::new(before.clone()));
            let candidate = Scripts::from_sources_with(
                &self.config,
                world,
                self.scripts.help.clone(),
                std::sync::Arc::new(sources),
                if checking {
                    RuntimeMode::Checking
                } else {
                    RuntimeMode::Live
                },
                |candidate| self.snapshots_for(candidate),
            )?;
            if checking {
                return Ok::<_, anyhow::Error>(None);
            }
            self.snapshots_for(&candidate)?;
            let after = candidate.world.borrow().clone();
            after.validate(&self.config)?;
            if serde_json::to_vec(&before)? != serde_json::to_vec(&after)?
                || candidate.effects.maintenance().is_some()
            {
                persistence::persist_effects(
                    self.config.database(),
                    after,
                    self.config.database.busy_timeout_ms,
                    candidate.effects.maintenance(),
                )
                .await?;
            }
            Ok(Some(candidate))
        }
        .await;
        match result {
            Ok(None) => {
                self.inspection_report(id, "All Lua module checks passed.".into())
                    .await
            }
            Ok(Some(candidate)) => {
                candidate.effects.inherit(&self.scripts.effects);
                self.scripts = candidate;
                self.finish_maintenance();
                self.reconcile_connections();
                self.flush();
                self.inspection_report(id, "Lua reloaded.".into()).await;
            }
            Err(error) => {
                self.config.log(
                    &[crate::logging::Category::Bugs],
                    "LUA",
                    "ERROR",
                    format!(
                        "Lua {} failed: {error:#}",
                        if checking { "check" } else { "reload" }
                    ),
                );
                self.inspection_report(
                    id,
                    format!(
                        "Lua {} failed: {error:#}",
                        if checking { "check" } else { "reload" }
                    ),
                )
                .await;
            }
        }
    }

    /// Reports share bounded, grapheme-safe delivery with help and never enter persistence.
    pub(super) async fn inspection_report(&self, id: SessionId, text: String) {
        if let Some(session) = self.sessions.get(&id) {
            let report = crate::help::HelpResponse::Message(text);
            if let Err(error) = session.help(&report, false, &self.config).await {
                self.config.log(
                    &[crate::logging::Category::Problems],
                    "SRV",
                    "ERROR",
                    format!("Inspection report: {error:#}"),
                );
                self.tell(id, "Unable to deliver complete report.\r\n");
            }
        }
    }
}
