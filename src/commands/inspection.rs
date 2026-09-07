//! Read-only object reports; debug bookkeeping is fetched by the asynchronous world owner.
use super::{Action, CommandContext, CommandInput, target::builder_target};
use crate::{
    flags,
    world::{Kind, ObjectId, World},
};
use anyhow::{Context, Result};

/// Stable object identity including type and flag letters, retaining stored markup for inspection.
pub(crate) fn identity(world: &World, id: Option<ObjectId>) -> String {
    let Some(id) = id else {
        return "*NOTHING*".into();
    };
    let Some(object) = world.objects.get(&id) else {
        return format!("*INVALID*(#{})", id.0);
    };
    format!("{}{}", object.name, crate::find::suffix(object))
}

/// Resolve the same default location used by bare look and examine.
fn target(ctx: &CommandContext<'_>, name: &str) -> Result<ObjectId> {
    builder_target(
        &ctx.scripts.world.borrow(),
        ctx.player,
        if name.trim().is_empty() { "here" } else { name },
    )
}

/// Build ordinary and brief reports without invoking Lua functions.
pub(super) fn examine(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    let result = (|| -> Result<Action> {
        let mode = match input.switch.as_deref() {
            None => "full",
            Some(s) if !s.is_empty() && "brief".starts_with(s) => "brief",
            Some(s) if !s.is_empty() && "debug".starts_with(s) => "debug",
            _ => anyhow::bail!("Unsupported command switch."),
        };
        let id = target(ctx, &input.args)?;
        if mode == "debug" {
            return Ok(Action::ExamineDebug(id));
        }
        let world = ctx.scripts.world.borrow();
        let o = &world.objects[&id];
        let mut lines = vec![
            identity(&world, Some(id)),
            format!("Type: {}", format!("{:?}", o.kind).to_uppercase()),
            format!("Flags: {}", o.flags.names().join(" ")),
            format!("Powers: {}", o.powers.description()),
        ];
        let descriptions = [
            ("Description", &o.description),
            ("InternalDescription", &o.internal_description),
        ];
        if descriptions
            .iter()
            .any(|(_, v)| v.as_ref().is_some_and(|s| !s.is_empty()))
        {
            lines.push("Attributes:".into());
        }
        for (label, value) in descriptions {
            if let Some(value) = value.as_ref().filter(|s| !s.is_empty()) {
                lines.push(format!("  {label}: {value}"));
            }
        }
        lines.push(format!("Zone: {}", identity(&world, o.zone)));
        lines.push(format!("Affiliation: {}", identity(&world, o.affiliation)));
        lines.extend(ctx.scripts.inspect_parent(id)?);
        if mode != "brief" {
            lines.push(crate::state::commands::summary(&o.state));
        }
        let contents: Vec<_> = world
            .objects
            .values()
            .filter(|v| v.location == Some(id) && !matches!(v.kind, Kind::Exit | Kind::Garbage))
            .collect();
        if !contents.is_empty() {
            lines.push("Contents:".into());
            lines.extend(
                contents
                    .iter()
                    .map(|v| format!("  {}", identity(&world, Some(v.id)))),
            );
        }
        if o.kind != Kind::Exit {
            let exits: Vec<_> = world
                .objects
                .values()
                .filter(|v| v.kind == Kind::Exit && v.location == Some(id))
                .collect();
            lines.push(
                if exits.is_empty() {
                    "No exits."
                } else {
                    "Exits:"
                }
                .into(),
            );
            lines.extend(
                exits
                    .iter()
                    .map(|v| format!("  {}", identity(&world, Some(v.id)))),
            );
        }
        match o.kind {
            Kind::Room => {
                if o.dropto.is_some() {
                    lines.push(format!(
                        "Dropped objects go to: {}",
                        identity(&world, o.dropto)
                    ));
                }
            }
            Kind::Thing | Kind::Player => {
                lines.push(format!("Home: {}", identity(&world, o.home)));
                lines.push(format!("Location: {}", identity(&world, o.location)));
            }
            Kind::Exit => {
                lines.push(format!("Source: {}", identity(&world, o.location)));
                if o.destination.is_some() {
                    lines.push(format!("Destination: {}", identity(&world, o.destination)));
                }
            }
            _ => {}
        }
        Ok(Action::Report(lines.join("\n")))
    })();
    Ok(result.unwrap_or_else(|e| Action::Reply(e.to_string())))
}

/// List incoming homes, droptos and exits with shared legacy bounds.
pub(super) fn entrances(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    let result = (|| -> Result<String> {
        let world = ctx.scripts.world.borrow();
        let range = crate::find::FindCursor::new(&input.args, &world);
        let id = if range.query.is_empty() {
            world.objects[&ctx.player].location.unwrap_or(ctx.player)
        } else {
            target(ctx, &range.query)?
        };
        let mut lines = Vec::new();
        if range.next <= range.upper {
            for (_, o) in world
                .objects
                .range(ObjectId(range.next)..=ObjectId(range.upper))
            {
                if !flags::is_wizard(&world, ctx.player) {
                    continue;
                }
                match o.kind {
                    Kind::Exit if o.destination == Some(id) => {
                        lines.push(format!("{} ({})", identity(&world, o.location), o.name))
                    }
                    Kind::Room if o.dropto == Some(id) => {
                        lines.push(format!("{} [dropto]", identity(&world, Some(o.id))))
                    }
                    Kind::Thing | Kind::Player if o.home == Some(id) => {
                        lines.push(format!("{} [home]", identity(&world, Some(o.id))))
                    }
                    _ => {}
                }
            }
        }
        lines.push(format!(
            "{} entrance{} found.",
            lines.len(),
            if lines.len() == 1 { "" } else { "s" }
        ));
        Ok(lines.join("\n"))
    })();
    Ok(match result {
        Ok(text) => Action::Report(text),
        Err(e) => Action::Reply(e.to_string()),
    })
}

/// Use actual durable list pointers rather than reconstructing their order from object IDs.
pub(crate) fn debug(world: &World, id: ObjectId, links: [i64; 3]) -> Result<String> {
    let o = world.objects.get(&id).context("Object missing")?;
    let raw = |id: Option<ObjectId>| id.map_or(-1, |v| v.0);
    Ok(format!(
        "Number  = {}\nName    = {}\nLocation= {}\nContents= {}\nExits   = {}\nLink    = {}\nNext    = {}\nZone    = {}\nAffil.  = {}\nFlags   = {}\nPowers  = {}\nLua state entries: {}",
        id.0,
        o.name,
        raw(match o.kind {
            Kind::Room => o.dropto,
            Kind::Exit => o.destination,
            _ => o.location,
        }),
        links[0],
        links[1],
        raw(o.home),
        links[2],
        raw(o.zone),
        raw(o.affiliation),
        o.flags.names().join(" "),
        o.powers.description(),
        o.state.values().map(|v| v.len()).sum::<usize>()
    ))
}
