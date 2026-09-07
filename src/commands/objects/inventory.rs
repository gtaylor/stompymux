//! Player object transfer and container commands with ordered native lock checks.
use super::*;
use crate::flags::Flag;

pub(super) fn get(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    transaction(ctx, || {
        let location = ctx.location()?;
        let target = if let Some((owner, name)) = input.args.split_once("'s ") {
            let mut owners = located(ctx, location);
            owners.push(ctx.player);
            let owner = matched(ctx, owner, owners, None, false)?;
            matched(ctx, name, located(ctx, owner), Some(Kind::Thing), true)?
        } else {
            matched(
                ctx,
                &input.args,
                located(ctx, location),
                Some(Kind::Thing),
                true,
            )?
        };
        let silent = quiet(ctx, input, target, false)?;
        let (kind, source) = {
            let w = ctx.scripts.world.borrow();
            let o = &w.objects[&target];
            (o.kind, o.location)
        };
        ensure!(target != ctx.player, "You cannot get yourself!");
        ensure!(source != Some(ctx.player), "You already have that!");
        if kind == Kind::Exit {
            let w = ctx.scripts.world.borrow();
            ensure!(
                flags::controls(&w, ctx.player, target)
                    || flags::controls(&w, ctx.player, location),
                "Permission denied."
            );
            std::mem::drop(w);
            relocate(ctx, target, ctx.player, silent)?;
            tell(ctx, "Exit taken.");
            return Ok(());
        }
        ensure!(
            matches!(kind, Kind::Player | Kind::Thing),
            "You can't take that!"
        );
        ctx.scripts
            .world
            .borrow()
            .validate_move(target, ctx.player)?;
        if !check(
            ctx,
            LockType::Take,
            target,
            ctx.player,
            silent,
            Some("on_fail"),
            "You can't take that.",
        )? {
            return Ok(());
        }
        if let Some(owner) = source.filter(|o| *o != location) {
            ctx.scripts.outbox.borrow_mut().push((
                owner,
                format!("{} was taken from you.", display(ctx, target)?).into(),
            ));
        }
        relocate(ctx, target, ctx.player, silent)?;
        ctx.scripts
            .outbox
            .borrow_mut()
            .push((target, "Taken.".into()));
        ctx.scripts.action_message(
            action(ctx, target, "take", source, Some(ctx.player), silent),
            "success",
            Some("on_success"),
            Some("Taken."),
            Some(&format!("takes {}.", display(ctx, target)?)),
        )
    })
}

pub(super) fn drop(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    transaction(ctx, || {
        let location = ctx.location()?;
        let target = matched(
            ctx,
            &input.args,
            located(ctx, ctx.player),
            Some(Kind::Thing),
            false,
        )?;
        let silent = quiet(ctx, input, target, false)?;
        let kind = ctx.scripts.world.borrow().objects[&target].kind;
        if kind == Kind::Exit {
            ensure!(
                flags::controls(&ctx.scripts.world.borrow(), ctx.player, location),
                "Permission denied."
            );
            relocate(ctx, target, location, silent)?;
            tell(ctx, "Exit dropped.");
            return Ok(());
        }
        ensure!(
            matches!(kind, Kind::Player | Kind::Thing),
            "You can't drop that."
        );
        ctx.scripts.world.borrow().validate_move(target, location)?;
        if !check(
            ctx,
            LockType::Drop,
            target,
            ctx.player,
            false,
            Some("on_drop_fail"),
            "You can't drop that.",
        )? {
            return Ok(());
        }
        relocate(ctx, target, location, silent)?;
        ctx.scripts
            .outbox
            .borrow_mut()
            .push((target, "Dropped.".into()));
        ctx.scripts.action_message(
            action(
                ctx,
                target,
                "drop",
                Some(ctx.player),
                Some(location),
                silent,
            ),
            "drop",
            Some("on_drop"),
            Some("Dropped."),
            Some(&format!("drops {}.", display(ctx, target)?)),
        )?;
        let dropto = {
            let w = ctx.scripts.world.borrow();
            let loc = w.objects[&target].location;
            loc.and_then(|id| w.objects.get(&id))
                .filter(|o| o.kind == Kind::Room)
                .and_then(|o| o.dropto)
        };
        if let Some(destination) = dropto {
            relocate(ctx, target, destination, false)?;
        }
        Ok(())
    })
}

pub(super) fn give(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    transaction(ctx, || {
        let (recipient, item) = input
            .args
            .split_once('=')
            .context("Usage: give <recipient>=<object>")?;
        let mut nearby = located(ctx, ctx.location()?);
        nearby.extend(located(ctx, ctx.player));
        nearby.push(ctx.player);
        let recipient = matched(ctx, recipient, nearby, Some(Kind::Player), false)?;
        let silent = quiet(ctx, input, recipient, true)?;
        let mut inventory = located(ctx, ctx.player);
        inventory.push(ctx.player);
        let target = matched(ctx, item, inventory, Some(Kind::Thing), false)?;
        ensure!(target != ctx.player, "You can't give yourself away!");
        ensure!(
            matches!(
                ctx.scripts.world.borrow().objects[&target].kind,
                Kind::Player | Kind::Thing
            ),
            "Permission denied."
        );
        ctx.scripts
            .world
            .borrow()
            .validate_move(target, recipient)?;
        if !check(
            ctx,
            LockType::Give,
            target,
            ctx.player,
            false,
            Some("on_give_fail"),
            "You can't give that away.",
        )? {
            return Ok(());
        }
        if !check(
            ctx,
            LockType::Receive,
            recipient,
            target,
            false,
            Some("on_give_receive_fail"),
            "That recipient won't accept the object.",
        )? {
            return Ok(());
        }
        relocate(ctx, target, recipient, false)?;
        if !silent {
            tell(ctx, "Given.");
            ctx.scripts.outbox.borrow_mut().push((
                recipient,
                format!(
                    "{} gave you {}.",
                    display(ctx, ctx.player)?,
                    display(ctx, target)?
                )
                .into(),
            ));
            ctx.scripts.outbox.borrow_mut().push((
                target,
                format!(
                    "{} gave you to {}.",
                    display(ctx, ctx.player)?,
                    display(ctx, recipient)?
                )
                .into(),
            ));
        }
        ctx.scripts.action_message(
            action(ctx, target, "give", None, None, false),
            "drop",
            Some("on_drop"),
            None,
            None,
        )?;
        let mut receive = action(ctx, target, "receive", None, None, false);
        receive.enactor = recipient;
        ctx.scripts
            .action_message(receive, "success", Some("on_success"), None, None)
    })
}

pub(super) fn use_object(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    transaction(ctx, || {
        let mut ids = located(ctx, ctx.location()?);
        ids.extend(located(ctx, ctx.player));
        ids.push(ctx.player);
        ids.push(ctx.location()?);
        if flags::is_wizard(&ctx.scripts.world.borrow(), ctx.player)
            && input.args.starts_with('#')
            && let Ok(id) = super::super::target::admin_target(
                &ctx.scripts.world.borrow(),
                ctx.player,
                &input.args,
            )
        {
            ids.push(id);
        }
        if flags::is_wizard(&ctx.scripts.world.borrow(), ctx.player)
            && let Some(name) = input.args.strip_prefix('*')
        {
            let id = ctx
                .scripts
                .world
                .borrow()
                .find_player(name)
                .context("No such player.")?;
            if !check(
                ctx,
                LockType::Use,
                id,
                ctx.player,
                false,
                Some("on_use_fail"),
                "You can't figure out how to use that.",
            )? {
                return Ok(());
            }
            if !ctx.scripts.has_action(id, "use", "on_use")? {
                tell(ctx, "You can't figure out how to use that.");
                return Ok(());
            }
            return ctx.scripts.action_message(
                action(ctx, id, "use", None, None, false),
                "use",
                Some("on_use"),
                Some(&format!("You use {}", display(ctx, id)?)),
                Some(&format!("uses {}", display(ctx, id)?)),
            );
        }
        ids.sort();
        ids.dedup();
        let target = matched(ctx, &input.args, ids, None, false)?;
        if !check(
            ctx,
            LockType::Use,
            target,
            ctx.player,
            false,
            Some("on_use_fail"),
            "You can't figure out how to use that.",
        )? {
            return Ok(());
        }
        if !ctx.scripts.has_action(target, "use", "on_use")? {
            tell(ctx, "You can't figure out how to use that.");
            return Ok(());
        }
        ctx.scripts.action_message(
            action(ctx, target, "use", None, None, false),
            "use",
            Some("on_use"),
            Some(&format!("You use {}", display(ctx, target)?)),
            Some(&format!("uses {}", display(ctx, target)?)),
        )
    })
}

pub(super) fn enter(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    transaction(ctx, || {
        let source = ctx.location()?;
        let target = matched(
            ctx,
            &input.args,
            located(ctx, source),
            Some(Kind::Thing),
            false,
        )?;
        ensure!(
            matches!(
                ctx.scripts.world.borrow().objects[&target].kind,
                Kind::Player | Kind::Thing
            ),
            "Permission denied."
        );
        let silent = quiet(ctx, input, target, false)?;
        ctx.scripts
            .world
            .borrow()
            .validate_move(ctx.player, target)?;
        if !check(
            ctx,
            LockType::Enter,
            target,
            ctx.player,
            silent,
            Some("on_enter_fail"),
            "You can't enter that.",
        )? {
            return Ok(());
        }
        if !check(
            ctx,
            LockType::Leave,
            source,
            ctx.player,
            silent,
            Some("on_enter_fail"),
            "You can't enter that.",
        )? {
            return Ok(());
        }
        crate::movement::perform(
            ctx.scripts,
            ctx.player,
            ctx.player,
            target,
            ctx.session,
            if silent {
                crate::movement::Route::EnterQuiet
            } else {
                crate::movement::Route::Generic
            },
        )
    })
}

pub(super) fn leave(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    transaction(ctx, || {
        ensure!(input.args.is_empty(), "leave takes no arguments.");
        let source = ctx.location()?;
        let destination = {
            let w = ctx.scripts.world.borrow();
            let o = &w.objects[&source];
            ensure!(
                o.kind != Kind::Room && !o.flags.contains(Flag::Going),
                "You can't leave."
            );
            o.location.context("You can't leave.")?
        };
        let silent = quiet(ctx, input, source, false)?;
        ctx.scripts
            .world
            .borrow()
            .validate_move(ctx.player, destination)?;
        if !check(
            ctx,
            LockType::Leave,
            source,
            ctx.player,
            silent,
            Some("on_leave_fail"),
            "You can't leave.",
        )? {
            return Ok(());
        }
        if !check(
            ctx,
            LockType::Enter,
            destination,
            ctx.player,
            silent,
            Some("on_leave_fail"),
            "You can't leave.",
        )? {
            return Ok(());
        }
        crate::movement::perform(
            ctx.scripts,
            ctx.player,
            ctx.player,
            destination,
            ctx.session,
            if silent {
                crate::movement::Route::LeaveQuiet
            } else {
                crate::movement::Route::Generic
            },
        )
    })
}

pub(super) fn inventory(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    if !input.args.is_empty() {
        return Ok(Action::Reply("inventory takes no arguments.".into()));
    }
    let w = ctx.scripts.world.borrow();
    let mut report = crate::reports::Report::new(ctx.config.lua.output_byte_limit, "")?;
    let contents: Vec<_> = w
        .objects
        .values()
        .filter(|o| {
            o.kind != Kind::Garbage && o.kind != Kind::Exit && o.location == Some(ctx.player)
        })
        .collect();
    report.row(if contents.is_empty() {
        "You aren't carrying anything."
    } else {
        "You are carrying:"
    });
    for o in contents {
        let name = format!("{}[reset]", o.name);
        report.row(&if crate::flags::is_wizard(&w, ctx.player) {
            format!("{name}{}", crate::find::suffix(o))
        } else {
            name
        });
    }
    let exits: Vec<_> = w
        .objects
        .values()
        .filter(|o| o.kind == Kind::Exit && o.location == Some(ctx.player))
        .collect();
    if !exits.is_empty() {
        report.row("Exits:");
        for exit in exits {
            report.row(&format!(
                "{}[reset]",
                exit.name.split(';').next().unwrap_or_default()
            ));
        }
    }
    Ok(Action::StyledReport(report.finish()?))
}
