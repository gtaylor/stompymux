//! Wizard exit creation, linking, homes, droptos and object cloning.
use super::*;
use crate::flags::Flag;

fn target(ctx: &CommandContext<'_>, name: &str) -> Result<ObjectId> {
    super::super::target::admin_target(&ctx.scripts.world.borrow(), ctx.player, name)
}

fn destination(ctx: &CommandContext<'_>, name: &str) -> Result<ObjectId> {
    let name = name.trim();
    let id = if name.bytes().all(|b| b.is_ascii_digit()) && !name.is_empty() {
        target(ctx, &format!("#{name}"))?
    } else {
        target(ctx, name)?
    };
    let w = ctx.scripts.world.borrow();
    let o = &w.objects[&id];
    ensure!(
        matches!(o.kind, Kind::Room | Kind::Player | Kind::Thing) && !o.flags.contains(Flag::Going),
        "You can't link to that."
    );
    Ok(id)
}

fn placement(ctx: &CommandContext<'_>, input: &CommandInput, clone: bool) -> Result<ObjectId> {
    match input.switch.as_deref() {
        None => ctx.location(),
        Some(s) if s.len() >= if clone { 3 } else { 1 } && "inventory".starts_with(s) => {
            Ok(ctx.player)
        }
        Some(s) if !s.is_empty() && "location".starts_with(s) => ctx.location(),
        _ => anyhow::bail!("Unsupported command switch."),
    }
}

fn object_name(ctx: &CommandContext<'_>, name: &str) -> Result<String> {
    let name = name.trim();
    let plain = crate::text::plain_with(&ctx.scripts.palette, name);
    ensure!(
        !plain.is_empty()
            && name.len() < 8192
            && !plain.chars().any(char::is_control)
            && !plain.starts_with(['#', '*', '!'])
            && !plain.contains('='),
        "That is not a reasonable name."
    );
    crate::text::validate(&ctx.scripts.palette, name)?;
    Ok(name.into())
}

/// A normal rejected link leaves an already-created exit unlinked; errors abort its transaction.
fn link_to(
    ctx: &CommandContext<'_>,
    object: ObjectId,
    dest: ObjectId,
    kind: LockType,
) -> Result<bool> {
    let w = ctx.scripts.world.borrow();
    if !flags::controls(&w, ctx.player, dest) {
        drop(w);
        tell(ctx, "Permission denied.");
        return Ok(false);
    }
    let o = &w.objects[&object];
    let allowed = if o.kind == Kind::Exit {
        o.destination.is_none() || flags::controls(&w, ctx.player, object)
    } else {
        flags::controls(&w, ctx.player, object)
    };
    ensure!(allowed, "Permission denied.");
    if matches!(o.kind, Kind::Player | Kind::Thing) {
        w.validate_move(object, dest)?;
    }
    if o.kind == Kind::Room {
        ensure!(
            w.objects[&dest].kind == Kind::Room,
            "Room droptos must be rooms."
        );
    }
    drop(w);
    if !check(
        ctx,
        kind,
        dest,
        ctx.player,
        false,
        None,
        "You can't link to there.",
    )? {
        return Ok(false);
    }
    let mut w = ctx.scripts.world.borrow_mut();
    // A callback can change the identities and containment checked above.
    ensure!(
        flags::controls(&w, ctx.player, dest),
        "Destination control changed during callback."
    );
    let object_kind = w
        .objects
        .get(&object)
        .context("Link object removed by callback")?
        .kind;
    let dest_kind = w
        .objects
        .get(&dest)
        .context("Link destination removed by callback")?
        .kind;
    ensure!(
        matches!(dest_kind, Kind::Player | Kind::Thing | Kind::Room)
            && !w.objects[&dest].flags.contains(Flag::Going),
        "Invalid link destination after callback."
    );
    if matches!(object_kind, Kind::Player | Kind::Thing) {
        w.validate_move(object, dest)?;
    }
    let o = w.objects.get_mut(&object).unwrap();
    match object_kind {
        Kind::Exit => o.destination = Some(dest),
        Kind::Room => {
            ensure!(dest_kind == Kind::Room, "Room droptos must be rooms.");
            o.dropto = Some(dest);
        }
        Kind::Player | Kind::Thing => o.home = Some(dest),
        _ => anyhow::bail!("You can't link that."),
    }
    Ok(true)
}

fn open_one(
    ctx: &CommandContext<'_>,
    name: &str,
    location: ObjectId,
    dest: Option<ObjectId>,
) -> Result<ObjectId> {
    let name = object_name(ctx, name)?;
    ensure!(
        flags::controls(&ctx.scripts.world.borrow(), ctx.player, location),
        "Permission denied."
    );
    let id = ctx
        .scripts
        .world
        .borrow_mut()
        .create(ctx.config, name, Kind::Exit);
    ctx.scripts.world.borrow().validate_move(id, location)?;
    ctx.scripts
        .world
        .borrow_mut()
        .objects
        .get_mut(&id)
        .unwrap()
        .location = Some(location);
    tell(ctx, format!("Opened exit #{id}.", id = id.0));
    if let Some(dest) = dest
        && link_to(ctx, id, dest, LockType::Link)?
    {
        tell(ctx, "Linked.");
    }
    Ok(id)
}

pub(super) fn open(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    transaction(ctx, || {
        let location = placement(ctx, input, false)?;
        let (name, links) = input
            .args
            .split_once('=')
            .map_or((input.args.as_str(), None), |(a, b)| (a, Some(b)));
        let links = links
            .map(|s| s.split(',').collect::<Vec<_>>())
            .unwrap_or_default();
        ensure!(
            links.len() <= 2,
            "Usage: @open <name>[=<destination>[,<return exit>]]"
        );
        // Parse optional links independently: rejected targets leave the first exit usable for later linking.
        let dest = links
            .first()
            .filter(|s| !s.trim().is_empty())
            .map(|s| destination(ctx, s))
            .transpose();
        let dest = match dest {
            Ok(d) => d,
            Err(e) => {
                open_one(ctx, name, location, None)?;
                tell(ctx, e.to_string());
                return Ok(());
            }
        };
        open_one(ctx, name, location, dest)?;
        if let (Some(dest), Some(back)) = (dest, links.get(1)) {
            if flags::controls(&ctx.scripts.world.borrow(), ctx.player, dest) {
                open_one(ctx, back, dest, Some(location))?;
            } else {
                tell(ctx, "Permission denied.");
            }
        }
        Ok(())
    })
}

fn unlink_one(ctx: &CommandContext<'_>, id: ObjectId) -> Result<()> {
    let mut w = ctx.scripts.world.borrow_mut();
    ensure!(flags::controls(&w, ctx.player, id), "Permission denied.");
    let o = w.objects.get_mut(&id).context("Object missing")?;
    let message = match o.kind {
        Kind::Exit => {
            o.destination = None;
            "Unlinked."
        }
        Kind::Room => {
            o.dropto = None;
            "Dropto removed."
        }
        _ => anyhow::bail!("You can't unlink that!"),
    };
    drop(w);
    tell(ctx, message);
    Ok(())
}

pub(super) fn unlink(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    transaction(ctx, || unlink_one(ctx, target(ctx, &input.args)?))
}

pub(super) fn link(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    transaction(ctx, || {
        let (name, dest) = input.args.split_once('=').unwrap_or((&input.args, ""));
        let id = target(ctx, name)?;
        if dest.trim().is_empty() {
            return unlink_one(ctx, id);
        }
        let dest = destination(ctx, dest)?;
        let kind = if matches!(
            ctx.scripts.world.borrow().objects[&id].kind,
            Kind::Player | Kind::Thing
        ) {
            LockType::SetHome
        } else {
            LockType::Link
        };
        if link_to(ctx, id, dest, kind)? {
            tell(
                ctx,
                if kind == LockType::SetHome {
                    "Home set."
                } else {
                    "Linked."
                },
            );
        }
        Ok(())
    })
}

fn clone_home(
    ctx: &CommandContext<'_>,
    source: Option<ObjectId>,
    clone: ObjectId,
) -> Result<ObjectId> {
    let w = ctx.scripts.world.borrow();
    for id in [
        source,
        w.objects[&ctx.player].location,
        w.objects[&ctx.player].home,
        Some(ObjectId(ctx.config.mux.default_home)),
        Some(ObjectId(ctx.config.home())),
        Some(ObjectId(ctx.config.start())),
    ]
    .into_iter()
    .flatten()
    {
        if flags::controls(&w, ctx.player, id)
            && !w.objects[&id].flags.contains(Flag::Going)
            && w.validate_move(clone, id).is_ok()
        {
            return Ok(id);
        }
    }
    anyhow::bail!("No valid home for clone.")
}

pub(super) fn clone_object(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    transaction(ctx, || {
        let location = placement(ctx, input, true)?;
        let (name, new_name) = input
            .args
            .split_once('=')
            .map_or((input.args.as_str(), None), |(a, b)| (a, Some(b)));
        let source = target(ctx, name)?;
        let original = ctx.scripts.world.borrow().objects[&source].clone();
        ensure!(
            matches!(original.kind, Kind::Room | Kind::Thing | Kind::Exit),
            "You cannot clone players or garbage!"
        );
        if original.kind == Kind::Exit {
            ensure!(
                flags::controls(&ctx.scripts.world.borrow(), ctx.player, location),
                "Permission denied."
            );
        }
        let name = object_name(
            ctx,
            new_name
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(&original.name),
        )?;
        let id = ctx
            .scripts
            .world
            .borrow_mut()
            .create(ctx.config, name, original.kind);
        {
            let mut w = ctx.scripts.world.borrow_mut();
            let clone = w.objects.get_mut(&id).unwrap();
            clone.state = original.state;
            clone.description = original.description;
            clone.internal_description = original.internal_description;
            clone.lua_parent = original.lua_parent;
            clone.flags.remove(Flag::Wizard);
        }
        ctx.scripts.sync_parents()?;
        match original.kind {
            Kind::Thing => {
                let home = clone_home(ctx, original.home, id)?;
                ctx.scripts
                    .world
                    .borrow_mut()
                    .objects
                    .get_mut(&id)
                    .unwrap()
                    .home = Some(home);
                relocate(ctx, id, location, false)?;
            }
            Kind::Exit => {
                ctx.scripts.world.borrow().validate_move(id, location)?;
                ctx.scripts
                    .world
                    .borrow_mut()
                    .objects
                    .get_mut(&id)
                    .unwrap()
                    .location = Some(location);
                if let Some(dest) = original.destination {
                    link_to(ctx, id, dest, LockType::Link)?;
                }
            }
            Kind::Room => {
                if let Some(dest) = original.dropto {
                    link_to(ctx, id, dest, LockType::Link)?;
                }
            }
            _ => unreachable!(),
        }
        ctx.scripts
            .object_event(action(ctx, id, "clone", None, None, false), "on_clone")?;
        tell(
            ctx,
            format!(
                "{} cloned, new copy is object #{}.",
                display(ctx, source)?,
                id.0
            ),
        );
        Ok(())
    })
}
