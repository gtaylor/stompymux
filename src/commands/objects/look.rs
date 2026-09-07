//! Local and possessive looking with distinct external and internal appearance callbacks.
use super::*;
use crate::{
    flags::Flag,
    lua::{ActionContent, AppearanceMode},
};

/// Select visible candidates without widening local dbref scope.
fn candidates(ctx: &CommandContext<'_>, origin: ObjectId) -> Vec<ObjectId> {
    let w = ctx.scripts.world.borrow();
    let location = w.objects[&origin].location;
    w.objects
        .values()
        .filter(|o| {
            o.kind != Kind::Garbage
                && w.visible(o, ctx.player)
                && (o.id == origin
                    || Some(o.id) == location
                    || o.location == Some(origin)
                    || (location.is_some() && o.location == location))
        })
        .map(|o| o.id)
        .collect()
}

/// Match local and nested possessive names without evaluating MATCH locks for inspection.
fn target(ctx: &CommandContext<'_>, origin: ObjectId, name: &str) -> Result<ObjectId> {
    if name.trim().eq_ignore_ascii_case("me") {
        return Ok(origin);
    }
    if name.trim().eq_ignore_ascii_case("here") {
        return ctx.scripts.world.borrow().objects[&origin]
            .location
            .context("You have no location.");
    }
    if let Some((container, item)) = name.rsplit_once("'s ") {
        let container = target(ctx, origin, container)?;
        let ids = located(ctx, container)
            .into_iter()
            .filter(|id| {
                let w = ctx.scripts.world.borrow();
                w.visible(&w.objects[id], ctx.player)
            })
            .collect();
        return matched(ctx, item, ids, None, false);
    }
    matched(ctx, name, candidates(ctx, origin), None, false)
}

fn contents(ctx: &CommandContext<'_>, object: ObjectId, label: &str, exits: bool) {
    let w = ctx.scripts.world.borrow();
    let dark = w.objects[&object].flags.contains(Flag::Dark);
    let names: Vec<_> = w
        .objects
        .values()
        .filter(|o| {
            o.location == Some(object)
                && o.id != ctx.player
                && o.kind != Kind::Garbage
                && (o.kind == Kind::Exit) == exits
                && !o.flags.contains(Flag::Dark)
                && (!dark || o.flags.contains(Flag::Light))
                && (o.kind != Kind::Player || o.flags.contains(Flag::Connected))
        })
        .map(|o| o.name.clone())
        .collect();
    drop(w);
    if !names.is_empty() {
        tell(ctx, format!("{label}\n{}", names.join("\n")));
    }
}

/// Render one object. Transparent exits reveal their destination without recursive exit traversal.
fn show(ctx: &CommandContext<'_>, id: ObjectId, through: bool) -> Result<()> {
    let o = ctx
        .scripts
        .world
        .borrow()
        .objects
        .get(&id)
        .context("Appearance object missing")?
        .clone();
    ensure!(o.kind != Kind::Garbage, "Appearance object destroyed");
    let mode = if o.kind == Kind::Room
        || ctx.scripts.world.borrow().objects[&ctx.player].location == Some(id)
    {
        AppearanceMode::Internal
    } else {
        AppearanceMode::External
    };
    if let Some(text) = ctx
        .scripts
        .render_appearance(ctx.player, id, ctx.session, mode)?
    {
        if !text.is_empty() {
            tell(ctx, text);
        }
        return ctx.scripts.object_event(
            action(ctx, id, "describe", None, None, false),
            "on_describe",
        );
    }
    if o.kind == Kind::Room || flags::is_wizard(&ctx.scripts.world.borrow(), ctx.player) {
        tell(
            ctx,
            format!(
                "{}{}",
                o.name,
                if flags::is_wizard(&ctx.scripts.world.borrow(), ctx.player) {
                    crate::find::suffix(&o)
                } else {
                    String::new()
                }
            ),
        );
    }
    {
        let internal = mode == AppearanceMode::Internal
            && o.kind != Kind::Room
            && o.internal_description
                .as_ref()
                .is_some_and(|v| !v.is_empty());
        let content = if internal {
            ActionContent::InternalDescription
        } else {
            ActionContent::Description
        };
        ctx.scripts.action_message_content(
            action(
                ctx,
                id,
                if internal {
                    "inside_describe"
                } else {
                    "describe"
                },
                None,
                None,
                false,
            ),
            "describe",
            Some("on_describe"),
            content,
            (
                (mode == AppearanceMode::External && !through)
                    .then_some("You see nothing special."),
                None,
            ),
        )?;
    }
    match o.kind {
        Kind::Room => {
            contents(ctx, id, "Contents:", false);
            contents(ctx, id, "Obvious exits:", true);
        }
        Kind::Thing | Kind::Player => {
            contents(
                ctx,
                id,
                if mode == AppearanceMode::Internal {
                    "Contents:"
                } else {
                    "Carrying:"
                },
                false,
            );
            if mode == AppearanceMode::Internal {
                contents(ctx, id, "Obvious exits:", true);
            }
        }
        Kind::Exit if !through && o.flags.contains(Flag::Transparent) => {
            if let Some(dest) = o.destination {
                show(ctx, dest, true)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Look can mutate through callbacks, so use the same rollback boundary as object actions.
pub(in crate::commands) fn look(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    transaction(ctx, || {
        let origin = ctx.player;
        let id = if input.args.trim().is_empty() {
            ctx.scripts.world.borrow().objects[&origin]
                .location
                .context("You have no location.")?
        } else {
            target(ctx, origin, &input.args)?
        };
        show(ctx, id, false)
    })
}
