//! Deferred destruction schedules GOING and stages C-compatible notifications.
use super::*;
use crate::{
    flags::Flag,
    notification::{self, Policy, Request},
};

/// Protect the foundational identities independently of SAFE or Wizard control.
pub(crate) fn protected(config: &crate::config::Config, id: ObjectId) -> bool {
    [0, 1, config.start(), config.home(), config.mux.default_home].contains(&id.0)
}

/// Schedule one controlled object; cleaning performs evacuation and deletion later.
pub(super) fn destroy(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    transaction(ctx, || {
        ensure!(
            !input.args.trim().is_empty(),
            "Usage: @destroy[/override] <target>"
        );
        ensure!(
            input.switch.as_deref().is_none_or(|s| s == "override"),
            "Unsupported @destroy switch."
        );
        let id = builders::target(ctx, &input.args)?;
        let mut world = ctx.scripts.world.borrow_mut();
        ensure!(
            flags::controls(&world, ctx.player, id),
            "Permission denied."
        );
        let object = &world.objects[&id];
        ensure!(
            !object.flags.contains(Flag::Safe) || input.switch.is_some(),
            "Sorry, that object is protected. Use @destroy/override to destroy it."
        );
        ensure!(!protected(ctx.config, id), "You can't destroy that!");
        let noun = match object.kind {
            Kind::Room => "room",
            Kind::Exit => "exit",
            Kind::Player => "player",
            _ => "object",
        };
        ensure!(
            !object.flags.contains(Flag::Going),
            "No sense beating a dead {noun}."
        );
        ensure!(
            object.kind != Kind::Player || !object.flags.contains(Flag::Wizard),
            "You may not destroy Wizards!"
        );
        let room = object.kind == Kind::Room;
        world
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::Going);
        notification::send(
            &world,
            &ctx.scripts.outbox,
            ctx.config,
            Request {
                target: if room { id } else { ctx.player },
                sender: ctx.player,
                document: format!("The {noun} shakes and begins to crumble.").into(),
                policy: if room { Policy::ROOM } else { Policy::DIRECT },
                exclusions: None,
            },
        )
    })
}
