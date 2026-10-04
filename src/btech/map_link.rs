//! Native linked-map control reuses the shared wrapping transition and notification checkpoint.
use anyhow::{Context, Result};

/// SETLINKED enables wrapping on the operator's selected map; its argument is ignored.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    _input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = ctx.scripts.atomic(|before| -> Result<()> {
        let map = super::special_dispatch::object(ctx)?;
        let markers = before
            .btech
            .maps()
            .get(&map)
            .context("Map not found")?
            .linked_markers();
        let ordinal = (0..=u32::try_from(markers.len())?)
            .find(|slot| !markers.contains_key(slot))
            .context("No linked marker slot available")?;
        super::set_linked_marker(
            &mut ctx.scripts.world_mut(),
            map,
            ordinal,
            Some(super::HexCoordinate { x: 0, y: 0 }),
        )?;
        super::notify_message(
            ctx.scripts,
            super::BattleMessageTarget::Player(ctx.player),
            "Map set to linked.",
        )?;
        ctx.scripts.world().validate(ctx.config)?;
        ctx.scripts.effects.validate()?;
        Ok(())
    });
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
