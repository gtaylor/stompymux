//! Individually registered native command handlers.
use super::target::admin_target;
use super::{Action, CommandContext, CommandInput};
use crate::{
    flags::{self, Flag},
    movement::{self, Route},
    scripting::Scripts,
    world::{Kind, ObjectId},
};
use anyhow::{Context, Result, ensure};
/// Queue a traditional player-directed administrative response.
fn response(ctx: &CommandContext<'_>, result: Result<String>) -> Result<Action> {
    ctx.scripts
        .outbox
        .borrow_mut()
        .push((ctx.player, result.unwrap_or_else(|e| e.to_string()).into()));
    Ok(Action::Continue)
}
/// Handle the @list command after the registry permission check.
pub(super) fn list(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    response(
        ctx,
        (|| {
            let args = input.args.as_str();
            if args.trim().eq_ignore_ascii_case("powers") {
                return Ok(format!(
                    "Powers: {}",
                    crate::powers::ALL
                        .iter()
                        .map(|p| p.display_name())
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
            ensure!(
                args.trim().eq_ignore_ascii_case("flags"),
                "Usage: @list flags or @list powers"
            );
            Ok(format!(
                "Flags: {}",
                flags::ALL
                    .iter()
                    .map(|f| format!("{}({})", f.world_name(), f.letter()))
                    .collect::<Vec<_>>()
                    .join(" ")
            ))
        })(),
    )
}
/// Handle the @examine command after the registry permission check.
pub(super) fn examine(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    response(
        ctx,
        (|| {
            let args = input.args.as_str();
            let player = ctx.player;
            let w = ctx.scripts.world.borrow();
            let target = admin_target(&w, player, args)?;
            let o = &w.objects[&target];
            Ok(format!(
                "{}(#{})\r\nType: {} Flags: {}\r\nPowers: {}",
                o.name,
                o.id.0,
                format!("{:?}", o.kind).to_uppercase(),
                o.flags.names().join(" "),
                o.powers.description()
            ))
        })(),
    )
}
/// Handle the @power command after the registry permission check.
pub(super) fn power(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    response(
        ctx,
        (|| {
            let args = input.args.as_str();
            let player = ctx.player;
            let mut w = ctx.scripts.world.borrow_mut();
            let (target, power) = args
                .split_once('=')
                .context("Usage: @power <target>=<power> or !<power>")?;
            let target = admin_target(&w, player, target)?;
            ensure!(flags::controls(&w, player, target), "Permission denied.");
            let power = power.trim();
            let (value, name) = power
                .strip_prefix('!')
                .map_or((true, power), |name| (false, name.trim()));
            ensure!(
                !name.is_empty(),
                "You must specify a power to {}.",
                if value { "set" } else { "clear" }
            );
            let power = crate::powers::Power::parse(name)
                .map_err(|_| anyhow::anyhow!("I don't understand that power."))?;
            crate::powers::change(&mut w, player, target, power, value)?;
            Ok(format!(
                "{} - {} {}.",
                w.objects[&target].name,
                power.display_name(),
                if value { "granted" } else { "removed" }
            ))
        })(),
    )
}
/// Handle the @flag command after the registry permission check.
pub(super) fn flag(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    response(
        ctx,
        (|| {
            let args = input.args.as_str();
            let player = ctx.player;
            let mut w = ctx.scripts.world.borrow_mut();
            let c = ctx.config;
            let (target, flag) = args
                .split_once('=')
                .context("Usage: @flag <target>=<flag> or !<flag>")?;
            let target = admin_target(&w, player, target)?;
            ensure!(flags::controls(&w, player, target), "Permission denied.");
            let flag = flag.trim();
            let (value, name) = flag
                .strip_prefix('!')
                .map_or((true, flag), |name| (false, name.trim()));
            ensure!(
                !name.is_empty(),
                "You must specify a flag to {}.",
                if value { "set" } else { "clear" }
            );
            let flag = Flag::resolve(name, &c.aliases.flags)
                .map_err(|_| anyhow::anyhow!("I don't understand that flag."))?;
            flags::change(&mut w, player, target, flag, value)?;
            Ok(format!(
                "{} - {} {}.",
                w.objects[&target].name,
                flag.world_name(),
                if value { "set" } else { "cleared" }
            ))
        })(),
    )
}
/// Render the immediate container using Lua appearance callbacks.
pub(super) fn look(ctx: &CommandContext<'_>, _: &CommandInput) -> Result<Action> {
    let text = ctx
        .scripts
        .appearance(ctx.player, ctx.location()?, ctx.session)?;
    ctx.scripts
        .outbox
        .borrow_mut()
        .push((ctx.player, text.into()));
    Ok(Action::Continue)
}
/// Speak to occupants of the immediate location.
pub(super) fn say(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    broadcast_speech(ctx.scripts, ctx.player, ctx.location()?, &input.args);
    Ok(Action::Continue)
}
/// Ask the server to close this connection.
pub(super) fn quit(_: &CommandContext<'_>, _: &CommandInput) -> Result<Action> {
    Ok(Action::Quit)
}
/// Delegate read-only pagination to the session owner.
pub(super) fn find(_: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    Ok(Action::Find(crate::find::FindRequest::parse(
        &input.args,
        input.switch.as_deref(),
    )))
}
/// Return to the caller's configured home.
pub(super) fn home(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    let result = (|| {
        ensure!(input.args.trim().is_empty(), "Usage: home");
        let destination = ctx.scripts.world.borrow().objects[&ctx.player]
            .home
            .context("Your home is not set.")?;
        movement::perform(
            ctx.scripts,
            ctx.player,
            ctx.player,
            destination,
            Some(ctx.session),
            Route::Home,
        )
    })();
    movement_response(ctx, result)
}
/// Teleport the caller or a matched object into a destination.
pub(super) fn teleport(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    let result = (|| {
        let args = input.args.as_str();
        ensure!(
            !args.trim().is_empty(),
            "Usage: @teleport <destination> or <object>=<destination>"
        );
        let (object, destination) = {
            let w = ctx.scripts.world.borrow();
            match args.split_once('=') {
                Some((object, destination)) => {
                    ensure!(
                        !object.trim().is_empty() && !destination.trim().is_empty(),
                        "Both target and destination are required."
                    );
                    (
                        admin_target(&w, ctx.player, object)?,
                        admin_target(&w, ctx.player, destination)?,
                    )
                }
                None => (ctx.player, admin_target(&w, ctx.player, args)?),
            }
        };
        movement::perform(
            ctx.scripts,
            ctx.player,
            object,
            destination,
            (object == ctx.player).then_some(ctx.session),
            Route::Teleport,
        )
    })();
    movement_response(ctx, result)
}
/// Preserve movement's existing user-facing error reporting.
fn movement_response(ctx: &CommandContext<'_>, result: Result<()>) -> Result<Action> {
    if let Err(error) = result {
        ctx.scripts
            .outbox
            .borrow_mut()
            .push((ctx.player, error.to_string().into()));
    }
    Ok(Action::Continue)
}
fn broadcast_speech(s: &Scripts, player: ObjectId, room: ObjectId, message: &str) {
    // C speech strips user formatting; escape the resulting literal before composing the line.
    let message = crate::text::escape(&crate::text::plain_with(&s.palette, message));
    let w = s.world.borrow();
    let p = &w.objects[&player];
    if p.flags.contains(crate::flags::Flag::Gagged) {
        s.outbox
            .borrow_mut()
            .push((player, "You cannot speak.".into()));
        return;
    }
    for o in w
        .objects
        .values()
        .filter(|o| o.kind == Kind::Player && o.location == Some(room))
    {
        s.outbox.borrow_mut().push((
            o.id,
            if o.id == player {
                format!("You say, \"{message}\"")
            } else {
                format!("{} says, \"{message}\"", p.name)
            }
            .into(),
        ));
    }
}

/// Request shutdown without reasons or switches.
pub(super) fn shutdown(_: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    Ok(if input.args.is_empty() {
        Action::Shutdown
    } else {
        Action::Reply("Usage: @shutdown".into())
    })
}
/// Request semantic checking and repair from the world owner.
pub(super) fn dbck(_: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    Ok(if input.args.is_empty() {
        Action::DbCheck
    } else {
        Action::Reply("Usage: @dbck".into())
    })
}

/// Inspect live sessions without entering the world transaction path.
pub(super) fn sessions(_: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    Ok(Action::Sessions(input.args.clone()))
}
/// Inspect negotiated options for every session belonging to a player.
pub(super) fn telnet(_: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    Ok(Action::Telnet(input.args.clone()))
}

/// Inspect or change the invoking connection's rendering preference.
pub fn color(_: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    Ok(Action::Color(input.args.clone()))
}

/// Resolve a help topic without invoking Lua or writing the database.
pub fn help(_: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    Ok(Action::Help(input.args.clone()))
}
