//! Read-only schedule reports from captured definitions, never re-evaluated Lua modules.
use super::Catalog;
use crate::{
    commands::target::admin_target,
    telnet::diagnostics::{Report, escape},
    world::{Kind, ObjectId, World},
};
use anyhow::{Result, ensure};

impl Catalog {
    /// Bound and escape inspection independently of game styling and Lua output callbacks.
    pub fn inspect(&self, world: &World, viewer: ObjectId, target: &str, limit: usize) -> Vec<u8> {
        let mut report = Report::new(limit);
        if let Err(error) = self.describe(world, viewer, target.trim(), &mut report) {
            report.line(&format!(
                "Lua schedule unavailable: {}",
                escape(error.to_string().as_bytes())
            ));
        }
        report.finish()
    }

    /// C summary and module/object detail views, resolving only registered source paths.
    fn describe(
        &self,
        world: &World,
        viewer: ObjectId,
        target: &str,
        report: &mut Report,
    ) -> Result<()> {
        if target.is_empty() {
            let mut any = false;
            for (module, definitions) in &self.modules {
                if definitions.is_empty() {
                    continue;
                }
                if let Some(parent) = module.strip_prefix("object_logic/") {
                    let count = world
                        .objects
                        .values()
                        .filter(|o| o.kind != Kind::Garbage && o.lua_parent == parent)
                        .count();
                    if count == 0 {
                        continue;
                    }
                    report.line(&format!(
                        "{}: {} schedules ({count} objects)",
                        escape(module.as_bytes()),
                        definitions.len()
                    ));
                } else {
                    report.line(&format!(
                        "{}: {} schedules (global)",
                        escape(module.as_bytes()),
                        definitions.len()
                    ));
                }
                any = true;
                if report.full() {
                    break;
                }
            }
            if !any {
                report.line("No active Lua schedules.");
            }
            return Ok(());
        }
        let is_path = target.contains('/') || target.ends_with(".lua");
        let (module, show_objects) = if is_path {
            ensure!(
                !target.starts_with('/')
                    && !target.contains('\\')
                    && target
                        .split('/')
                        .all(|s| !s.is_empty() && s != "." && s != ".."),
                "invalid module path"
            );
            if target.starts_with("global_logic/") {
                (target.to_string(), false)
            } else {
                (
                    if target.starts_with("object_logic/") {
                        target.to_string()
                    } else {
                        format!("object_logic/{target}")
                    },
                    true,
                )
            }
        } else {
            let id = admin_target(world, viewer, target)?;
            let parent = &world.objects[&id].lua_parent;
            ensure!(!parent.is_empty(), "That object has no Luaparent.");
            (format!("object_logic/{parent}"), false)
        };
        let definitions = self
            .modules
            .get(&module)
            .ok_or_else(|| anyhow::anyhow!("unknown module {module}"))?;
        report.line(&format!("Schedules for {}:", escape(module.as_bytes())));
        if definitions.is_empty() {
            report.line("  (none)");
        }
        for definition in definitions {
            report.line(&format!(
                "  {}: {}",
                escape(definition.name.as_bytes()),
                escape(definition.cron.source().as_bytes())
            ));
            if report.full() {
                return Ok(());
            }
        }
        if show_objects {
            let parent = module.strip_prefix("object_logic/").expect("object module");
            report.line("Objects:");
            for object in world
                .objects
                .values()
                .filter(|o| o.kind != Kind::Garbage && o.lua_parent == parent)
            {
                report.line(&format!(
                    "  {} (#{})",
                    escape(crate::text::plain_with(&world.palette, &object.name).as_bytes()),
                    object.id.0
                ));
                if report.full() {
                    break;
                }
            }
        }
        Ok(())
    }
}
