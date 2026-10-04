//! Map-side VIEW reuses tactical terrain rendering and transactional direct publication.
use super::{HexCoordinate, TacticalMap};
use crate::{Config, ObjectId, Scripts};
use anyhow::{Context, Result, ensure};

/// Publish a wizard's map-only view without changing unit state, contacts, timers or dice.
pub fn view_map_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    map: ObjectId,
    center: HexCoordinate,
) -> Result<TacticalMap> {
    scripts.atomic(|before| {
        ensure!(
            crate::authority::is_wizard(before, actor),
            "Permission denied."
        );
        ensure!(
            before
                .objects
                .get(&map)
                .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
            "Map is unavailable"
        );
        let report = super::tactical_map::map_view(
            before,
            map,
            actor,
            center,
            super::view_dimensions(before, actor)?,
        )?;
        for line in report.text.lines() {
            super::notify_message(scripts, super::MessageTarget::Player(actor), line)?;
        }
        scripts.world().validate_action(config)?;
        scripts.effects.validate()?;
        Ok(report)
    })
}

/// VIEW accepts two signed coordinates, then clamps the center through the shared viewport.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| -> Result<()> {
        let args: Vec<_> = input.args.split_whitespace().collect();
        ensure!(args.len() == 2, "Usage: VIEW X Y");
        let center = HexCoordinate {
            x: args[0].parse().context("Invalid map coordinates!")?,
            y: args[1].parse().context("Invalid map coordinates!")?,
        };
        let map = super::special_dispatch::object(ctx)?;
        view_map_action(ctx.scripts, ctx.config, ctx.player, map, center)?;
        Ok(())
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
