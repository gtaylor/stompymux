//! Deferred destruction schedules GOING and stages C-compatible notifications.
use super::*;
use crate::notification::{self, Policy, Request};

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
        let kind = crate::destruction::schedule(
            &mut world,
            ctx.config,
            ctx.player,
            id,
            input.switch.is_some(),
        )?;
        let noun = match kind {
            Kind::Room => "room",
            Kind::Exit => "exit",
            Kind::Player => "player",
            _ => "object",
        };
        let room = kind == Kind::Room;
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
