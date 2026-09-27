//! Generic special-object commands dispatch to the same map and unit field services.
use crate::{CommandAction, CommandContext, CommandInput, CommandReport};
use anyhow::{Context, Result, bail};

/// Registered field-service families.
enum FieldObjectKind {
    Map,
    Unit,
}

/// Resolve the occupied object's type once, then delegate authority, mutation and publication.
pub(crate) fn command(
    ctx: &CommandContext<'_>,
    input: &CommandInput,
    edit: bool,
) -> Result<CommandAction> {
    let result = (|| {
        let (id, kind) = {
            let world = ctx.scripts.world();
            let id = world
                .objects
                .get(&ctx.player)
                .and_then(|object| object.location)
                .context("Player has no location")?;
            let kind = if world.btech.maps().contains_key(&id) {
                FieldObjectKind::Map
            } else if world.btech.constructed_units().contains_key(&id)
                || world.btech.vehicles().contains_key(&id)
            {
                FieldObjectKind::Unit
            } else {
                bail!("Error: No fields for this BTech type were found.")
            };
            (id, kind)
        };
        if edit {
            let (field, value) = super::field_report::assignment(&input.args)?;
            match kind {
                FieldObjectKind::Map => super::set_map_field_action(
                    ctx.scripts,
                    ctx.config,
                    ctx.player,
                    id,
                    field,
                    value,
                )?,
                FieldObjectKind::Unit => super::set_unit_field_action(
                    ctx.scripts,
                    ctx.config,
                    ctx.player,
                    id,
                    field,
                    value,
                )?,
            }
        } else {
            match kind {
                FieldObjectKind::Map => {
                    super::view_map_fields_action(
                        ctx.scripts,
                        ctx.config,
                        ctx.player,
                        id,
                        &input.args,
                    )?;
                }
                FieldObjectKind::Unit => {
                    super::view_unit_fields_action(
                        ctx.scripts,
                        ctx.config,
                        ctx.player,
                        id,
                        &input.args,
                    )?;
                }
            }
        }
        Ok(())
    })();
    Ok(match result {
        Ok(()) => CommandAction::Continue,
        Err(error) => CommandAction::Report(CommandReport::Reply(format!("{error:#}"))),
    })
}
