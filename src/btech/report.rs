//! Silent brief reports of selected units and coordinate occupants, without detailed armor disclosure.
use crate::{CommandAction, CommandContext, CommandInput, CommandReport};
use anyhow::{Context, Result, bail};

/// Resolve native report selection and return the read-only unit summary.
pub(crate) fn command(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<CommandAction> {
    let result = (|| {
        let world = ctx.scripts.world.borrow();
        let source = world
            .objects
            .get(&ctx.player)
            .and_then(|p| p.location)
            .context("Enter a unit first")?;
        let owner = super::combat_operator::for_owner(&world, source, ctx.player)?.source;
        let source = owner.unit;
        let args: Vec<_> = input.args.split_whitespace().collect();
        let target = match args.as_slice() {
            [] => match owner.selection(&world).context("No default target set!")? {
                super::BattleTargetSelection::Unit(lock) => lock.target,
                super::BattleTargetSelection::Hex(lock) => {
                    let map =
                        super::scan::check_coordinate(&world, source, ctx.player, lock.hex, true)?;
                    super::scan::occupant(&world, source, map, lock.hex)?
                        .context("You don't see a thing.")?
                }
            },
            [target] => super::radio_targeted::target(&world, source, target)?,
            [x, y] => {
                let coordinate = super::BattleHexCoordinate {
                    x: x.parse().context("Invalid coordinates!")?,
                    y: y.parse().context("Invalid coordinates!")?,
                };
                let map =
                    super::scan::check_coordinate(&world, source, ctx.player, coordinate, false)?;
                super::scan::occupant(&world, source, map, coordinate)?
                    .context("No target found.")?
            }
            _ => bail!("Usage: report [target] or report x y"),
        };
        super::report_unit(&world, source, ctx.player, target)
    })();
    Ok(CommandAction::Report(match result {
        Ok(text) => CommandReport::Styled(text),
        Err(error) => CommandReport::Reply(format!("{error:#}")),
    }))
}
