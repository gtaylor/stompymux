//! Native weapon-safety controls share persistent preferences and cockpit notification routing.
use crate::{CommandAction, CommandContext, CommandInput, CommandReport, ObjectId, Scripts};
use anyhow::{Context, Result};

/// Apply or inspect the safety setting inside one state-and-output checkpoint.
fn action(scripts: &Scripts, id: ObjectId, pilot: ObjectId, argument: &str) -> Result<()> {
    scripts.atomic(|before| {
        super::radio::controlled(before, id, pilot)?;
        let enabled = match argument.trim().to_ascii_lowercase().as_str() {
            "on" => Some(true),
            "off" => Some(false),
            _ => None,
        };
        if let Some(enabled) = enabled {
            super::set_mw_safety(&mut scripts.world_mut(), id, pilot, enabled)?;
            super::notify_unit_text(
                scripts,
                id,
                if enabled {
                    "Safeties flipped [fg=green bold]ON[reset]."
                } else {
                    "Safeties flipped [fg=red bold]OFF[reset]."
                },
            )?;
        } else {
            let text = if super::auxiliary_preferences::mw_safety(before, id)? {
                "Weapon safeties are [bold][fg=green]ON[reset]"
            } else {
                "Weapon safeties are [bold][fg=red]OFF[reset]"
            };
            super::notify_message(scripts, super::BattleMessageTarget::Player(pilot), text)?;
        }
        Ok(())
    })
}

/// Native safety uses the assigned pilot's current cockpit, independently of engine state.
pub(crate) fn command(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<CommandAction> {
    let result = (|| {
        let id = ctx
            .scripts
            .world()
            .objects
            .get(&ctx.player)
            .and_then(|o| o.location)
            .context("Enter a unit first")?;
        action(ctx.scripts, id, ctx.player, &input.args)
    })();
    Ok(match result {
        Ok(()) => CommandAction::Continue,
        Err(error) => CommandAction::Report(CommandReport::Reply(format!("{error:#}"))),
    })
}
