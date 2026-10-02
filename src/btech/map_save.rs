//! Map saving encodes the selected map and stages the file write in the host transaction.
use crate::{Config, ObjectId, Scripts};
use anyhow::{Context, Result, ensure};

/// Prepare a map replacement; the host reports completion only after committed file publication.
pub fn save_map_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    id: ObjectId,
    name: &str,
) -> Result<()> {
    scripts.atomic(|before| {
        ensure!(
            crate::authority::is_wizard(before, actor),
            "Permission denied."
        );
        ensure!(
            before.objects.get(&id).is_some_and(|object| matches!(
                object.kind,
                crate::Kind::Room | crate::Kind::Thing
            ) && !object
                .flags
                .contains(crate::Flag::Going)),
            "Map is unavailable"
        );
        let source = before
            .btech
            .maps()
            .get(&id)
            .context("Map not found")?
            .export_asset()?;
        let request =
            crate::runtime::MapAssetWrite::new(config, actor, &format!("{name}.toml"), source)?;
        super::notify_message(
            scripts,
            super::BattleMessageTarget::Player(actor),
            &format!("Saving {name}"),
        )?;
        scripts.effects.stage_map_write(request)?;
        scripts.world().validate(config)?;
        scripts.effects.validate()?;
        Ok(())
    })
}

/// Save the operator's selected map using one relative asset name.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| -> Result<()> {
        let args: Vec<_> = input.args.split_whitespace().collect();
        ensure!(args.len() == 1, "Usage: SAVEMAP <NAME>");
        let map = super::special_dispatch::object(ctx)?;
        save_map_action(ctx.scripts, ctx.config, ctx.player, map, args[0])
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
