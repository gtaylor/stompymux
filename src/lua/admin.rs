//! Typed Lua administration requests and active-runtime parent assignment.
use super::Scripts;
use crate::{
    commands::{Action, CommandContext, CommandInput},
    flags::Flag,
    world::{Kind, ObjectId},
};
use anyhow::{Result, ensure};

/// Operations requiring filesystem work by the asynchronous world owner.
pub enum AdminRequest {
    Check,
    Reload,
    Test(super::testing::Request),
    View {
        path: String,
        object: Option<ObjectId>,
    },
}

impl Scripts {
    /// Attach only known code; a successful reload makes newly discovered modules available.
    pub fn attach_parent(&self, id: ObjectId, path: Option<&str>) -> Result<()> {
        if let Some(path) = path {
            self.sources.contains_parent(path)?;
        }
        let mut world = self.world.borrow_mut();
        let o = world
            .objects
            .get_mut(&id)
            .filter(|o| o.kind != Kind::Garbage)
            .ok_or_else(|| anyhow::anyhow!("No such object."))?;
        ensure!(!o.flags.contains(Flag::Going), "Object is being destroyed.");
        o.lua_parent = path.unwrap_or_default().into();
        Ok(())
    }
}

/// Exact switch parsing and Wizard permissions are supplied by the native registry.
pub(crate) fn command(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    let result = (|| -> Result<Action> {
        Ok(match input.switch.as_deref() {
            None if input.args.is_empty() => Action::Report(crate::commands::Report::Reply("@lua command switches:\n  /parent    Attach or clear a Lua parent.\n  /viewparent Display current source.\n  /check     Check modules in isolation.\n  /reload    Replace modules atomically.\n  /schedule  Inspect active schedules.\n  /test      Run live-world Lua test suites.".into())),
            Some(switch) if switch.split('/').any(|s| s == "test") => {
                Action::Server(crate::commands::ServerRequest::LuaAdmin(AdminRequest::Test(super::testing::Request::parse(switch, &input.args)?)))
            }
            Some("schedule") => Action::Server(crate::commands::ServerRequest::LuaSchedules(input.args.clone())),
            Some("check" | "reload") => {
                ensure!(input.args.is_empty(), "This @lua switch takes no arguments.");
                Action::Server(crate::commands::ServerRequest::LuaAdmin(if input.switch.as_deref() == Some("check") { AdminRequest::Check } else { AdminRequest::Reload }))
            }
            Some("parent") => {
                let (target, path) = input.args.split_once('=').unwrap_or((&input.args, ""));
                let id = crate::commands::target::builder_target(&ctx.scripts.world.borrow(), ctx.player, target)?;
                let path = path.trim();
                ctx.scripts.attach_parent(id, (!path.is_empty()).then_some(path))?;
                Action::CommitReply(if path.is_empty() { "Lua parent cleared." } else { "Lua parent set." }.into())
            }
            Some("viewparent") => {
                let arg = input.args.trim();
                ensure!(!arg.is_empty(), "View which Lua parent?");
                let (path, object) = if arg.starts_with('#') {
                    let world = ctx.scripts.world.borrow();
                    let id = crate::commands::target::builder_target(&world, ctx.player, arg)?;
                    let path = world.objects[&id].lua_parent.clone();
                    ensure!(!path.is_empty(), "That object has no Lua parent.");
                    (path, Some(id))
                } else { (arg.to_string(), None) };
                super::sources::parent_path(&path)?;
                Action::Server(crate::commands::ServerRequest::LuaAdmin(AdminRequest::View { path, object }))
            }
            _ => anyhow::bail!("Invalid @lua switch combination."),
        })
    })();
    Ok(result.unwrap_or_else(|e| Action::Report(crate::commands::Report::Reply(e.to_string()))))
}
