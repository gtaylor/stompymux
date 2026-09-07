//! Controlled names, account aliases, descriptions and zone assignment.
use super::*;
use crate::flags::Flag;

/// Parse a controlled target without changing either side of the assignment.
fn assignment<'a>(
    ctx: &CommandContext<'_>,
    input: &'a CommandInput,
) -> Result<(ObjectId, &'a str)> {
    let (target, value) = input
        .args
        .split_once('=')
        .context("Specify <object>=<value>.")?;
    let id = builders::target(ctx, target)?;
    ensure!(
        flags::controls(&ctx.scripts.world.borrow(), ctx.player, id),
        "Permission denied."
    );
    Ok((id, value))
}

/// Rename objects while maintaining the shared account-name namespace.
pub(super) fn name(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    transaction(ctx, || {
        let (id, value) = assignment(ctx, input)?;
        let name = builders::object_name(ctx, value)?;
        let plain = crate::text::plain_with(&ctx.scripts.palette, &name);
        let mut world = ctx.scripts.world.borrow_mut();
        if world.objects[&id].kind == Kind::Player {
            crate::accounts::validate_name(plain.trim(), ctx.config)?;
            ensure!(
                world
                    .find_player(plain.trim())
                    .is_none_or(|found| found == id),
                "That name is already in use."
            );
        }
        world.objects.get_mut(&id).unwrap().name = name;
        drop(world);
        tell(ctx, "Name set.");
        Ok(())
    })
}

/// Assign or clear a player login alias after collision checks.
pub(super) fn alias(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    transaction(ctx, || {
        let (id, value) = assignment(ctx, input)?;
        let value = value.trim();
        let mut world = ctx.scripts.world.borrow_mut();
        ensure!(
            world.objects[&id].kind == Kind::Player,
            "Only players may have aliases."
        );
        if !value.is_empty() {
            ensure!(
                world.find_player(value).is_none(),
                "That name is already in use."
            );
            crate::accounts::validate_name(value, ctx.config)?;
        }
        world
            .accounts
            .get_mut(&id)
            .context("Player account missing.")?
            .alias = (!value.is_empty()).then(|| value.to_owned());
        drop(world);
        tell(
            ctx,
            if value.is_empty() {
                "Alias removed."
            } else {
                "Alias set."
            },
        );
        Ok(())
    })
}

/// Validate and retain the selected description as source markup.
pub(super) fn description(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    transaction(ctx, || {
        let (id, value) = assignment(ctx, input)?;
        crate::text::validate(&ctx.scripts.palette, value)?;
        ensure!(value.len() < 8192, "Description exceeds 8191 bytes.");
        let internal = input.name == "@internal-description";
        let mut world = ctx.scripts.world.borrow_mut();
        let object = world.objects.get_mut(&id).unwrap();
        let field = if internal {
            &mut object.internal_description
        } else {
            &mut object.description
        };
        *field = (!value.is_empty()).then(|| value.to_owned());
        let message = format!(
            "{}/{} - {}",
            object.name,
            if internal {
                "InternalDescription"
            } else {
                "Description"
            },
            if value.is_empty() { "Cleared." } else { "Set." }
        );
        drop(world);
        tell(ctx, message);
        Ok(())
    })
}

/// Assign a controlled zone and clear non-player administrative privileges.
pub(super) fn zone(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    transaction(ctx, || {
        let (id, value) = assignment(ctx, input)?;
        let zone = if value.trim().eq_ignore_ascii_case("none") {
            None
        } else {
            Some(builders::target(ctx, value)?)
        };
        let mut world = ctx.scripts.world.borrow_mut();
        if let Some(zone) = zone {
            ensure!(
                matches!(world.objects[&zone].kind, Kind::Room | Kind::Thing),
                "Invalid zone object type."
            );
            ensure!(
                flags::controls(&world, ctx.player, zone),
                "You cannot move that object to that zone."
            );
        }
        let object = world.objects.get_mut(&id).unwrap();
        object.zone = zone;
        if object.kind != Kind::Player {
            object.flags.remove(Flag::Wizard);
            object.powers = Default::default();
        }
        drop(world);
        tell(ctx, "Zone changed.");
        Ok(())
    })
}
