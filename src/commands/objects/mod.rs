//! Inventory and builder operations sharing matching, locks and transactional output.
mod builders;
mod editing;
mod inventory;
pub(super) mod look;
use super::{
    Action, CommandContext, CommandDefinition, CommandInput, CommandPermissions, NativeHandler,
    SwitchPolicy,
};
use crate::{
    LockInvocation, LockType, flags,
    lua::ObjectAction,
    world::{Kind, ObjectId},
};
use anyhow::{Context, Result, ensure};

/// Register object operations alongside the existing native catalog.
pub fn definitions() -> Vec<CommandDefinition> {
    [
        ("get", inventory::get as NativeHandler, false),
        ("drop", inventory::drop, false),
        ("give", inventory::give, false),
        ("use", inventory::use_object, false),
        ("enter", inventory::enter, false),
        ("leave", inventory::leave, false),
        ("inventory", inventory::inventory, false),
        ("@create", builders::create, true),
        ("@dig", builders::dig, true),
        ("@name", editing::name, true),
        ("@alias", editing::alias, true),
        ("@description", editing::description, true),
        ("@internal-description", editing::description, true),
        ("@chzone", editing::zone, true),
        ("@open", builders::open, true),
        ("@link", builders::link, true),
        ("@unlink", builders::unlink, true),
        ("@clone", builders::clone_object, true),
    ]
    .into_iter()
    .map(|(name, handler, wizard)| {
        let mut d = CommandDefinition::native(
            name,
            if wizard {
                CommandPermissions::WIZARD
            } else {
                CommandPermissions::EVERYONE
            },
            handler,
        );
        if matches!(
            name,
            "get" | "drop" | "give" | "enter" | "leave" | "@dig" | "@open" | "@clone"
        ) {
            d.switches = SwitchPolicy::Handler;
        }
        d.private_errors = true;
        d
    })
    .collect()
}

/// Restore every provisional mutation on failed validation, callback or operation.
fn transaction(ctx: &CommandContext<'_>, work: impl FnOnce() -> Result<()>) -> Result<Action> {
    let before = ctx.scripts.world.borrow().clone();
    let pending = ctx.scripts.outbox.borrow().clone();
    let flow_effects = crate::lua::flows::snapshot(&ctx.scripts.lua);
    match work() {
        Ok(()) => Ok(Action::Continue),
        Err(e) => {
            *ctx.scripts.world.borrow_mut() = before;
            *ctx.scripts.outbox.borrow_mut() = pending;
            crate::lua::flows::restore(&ctx.scripts.lua, flow_effects);
            Ok(Action::Reply(e.to_string()))
        }
    }
}

fn tell(ctx: &CommandContext<'_>, message: impl Into<String>) {
    ctx.scripts
        .outbox
        .borrow_mut()
        .push((ctx.player, message.into().into()));
}

fn quiet(
    ctx: &CommandContext<'_>,
    input: &CommandInput,
    target: ObjectId,
    wizard_only: bool,
) -> Result<bool> {
    let Some(switch) = &input.switch else {
        return Ok(false);
    };
    ensure!(
        super::discovery::switches(&input.name)
            .iter()
            .any(|s| s.name == "quiet" && s.accepts(switch)),
        "Unsupported command switch."
    );
    let w = ctx.scripts.world.borrow();
    ensure!(
        !wizard_only || flags::is_wizard(&w, ctx.player),
        "Permission denied."
    );
    Ok(if wizard_only {
        true
    } else {
        flags::controls(&w, ctx.player, target)
    })
}

fn display(ctx: &CommandContext<'_>, object: ObjectId) -> Result<String> {
    ctx.scripts
        .world
        .borrow()
        .objects
        .get(&object)
        .map(|o| o.name.clone())
        .context("Object disappeared during callback.")
}

/// Match within an explicit candidate set; dbrefs never widen ordinary-player scope.
fn matched(
    ctx: &CommandContext<'_>,
    name: &str,
    mut ids: Vec<ObjectId>,
    preferred: Option<Kind>,
    keys: bool,
) -> Result<ObjectId> {
    ids.sort();
    ids.dedup();
    let name = name.trim();
    ensure!(!name.is_empty(), "Specify an object.");
    let w = ctx.scripts.world.borrow();
    let explicit = if name.eq_ignore_ascii_case("me") {
        Some(ctx.player)
    } else if name.eq_ignore_ascii_case("here") {
        w.objects[&ctx.player].location
    } else {
        name.strip_prefix('#')
            .and_then(|n| n.parse::<i64>().ok())
            .map(ObjectId)
    };
    let mut matches = Vec::new();
    for id in ids {
        let Some(o) = w.objects.get(&id).filter(|o| o.kind != Kind::Garbage) else {
            continue;
        };
        let plain = crate::text::plain_with(&w.palette, &o.name);
        let exact = explicit == Some(id)
            || (explicit.is_none()
                && plain
                    .split(if o.kind == Kind::Exit { ';' } else { '\0' })
                    .any(|n| n.eq_ignore_ascii_case(name)));
        let prefix = explicit.is_none() && plain.to_lowercase().starts_with(&name.to_lowercase());
        if exact || prefix {
            matches.push((id, exact, preferred == Some(o.kind)));
        }
    }
    drop(w);
    let exact = matches.iter().any(|m| m.1);
    matches.retain(|m| !exact || m.1);
    let ids = matches.iter().map(|m| m.0).collect();
    let mut ids = if keys {
        ctx.scripts.prefer_matches(ctx.player, ids, ctx.session)?
    } else {
        ids
    };
    let typed = matches.iter().any(|m| m.2 && ids.contains(&m.0));
    if typed {
        ids.retain(|id| matches.iter().any(|m| m.0 == *id && m.2));
    }
    match ids.as_slice() {
        [id] => Ok(*id),
        [] => anyhow::bail!("I don't see that here."),
        _ => anyhow::bail!("I don't know which object you mean."),
    }
}

fn located(ctx: &CommandContext<'_>, location: ObjectId) -> Vec<ObjectId> {
    ctx.scripts
        .world
        .borrow()
        .objects
        .values()
        .filter(|o| o.location == Some(location) && o.kind != Kind::Garbage)
        .map(|o| o.id)
        .collect()
}

fn check(
    ctx: &CommandContext<'_>,
    kind: LockType,
    object: ObjectId,
    subject: ObjectId,
    silent: bool,
    event: Option<&str>,
    default: &str,
) -> Result<bool> {
    let invocation = LockInvocation {
        kind,
        object,
        enactor: ctx.player,
        cause: ctx.cause,
        subject,
        descriptor: ctx.session,
        silent,
    };
    let result = ctx.scripts.evaluate_lock(invocation)?;
    if !result.passes {
        ctx.scripts
            .deny_action(invocation, &result, default, event)?;
    }
    Ok(result.passes)
}

fn action(
    ctx: &CommandContext<'_>,
    object: ObjectId,
    operation: &'static str,
    source: Option<ObjectId>,
    destination: Option<ObjectId>,
    silent: bool,
) -> ObjectAction<'static> {
    ObjectAction {
        object,
        enactor: ctx.player,
        cause: ctx.cause,
        descriptor: ctx.session,
        source,
        destination,
        operation,
        silent,
    }
}

fn relocate(
    ctx: &CommandContext<'_>,
    object: ObjectId,
    destination: ObjectId,
    _silent: bool,
) -> Result<()> {
    crate::movement::perform(
        ctx.scripts,
        ctx.player,
        object,
        destination,
        ctx.session.filter(|_| object == ctx.player),
        crate::movement::Route::Generic,
    )
}
