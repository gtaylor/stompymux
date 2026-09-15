//! Wizard team changes reuse sensor signatures and shared network invalidation across unit types.
use crate::{Config, ObjectId, Scripts};
use anyhow::{Context, Result, ensure};

/// Change a placed unit's team atomically and publish the normalized value to the administrator.
pub fn set_team_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    unit: ObjectId,
    team: i32,
) -> Result<i32> {
    let before = scripts.world().clone();
    let checkpoint = scripts.effects.checkpoint();
    let result = (|| {
        ensure!(
            crate::authority::is_wizard(&before, actor),
            "Permission denied."
        );
        let scanner = super::scanner::scanner_unit(&before, unit)
            .context("Unit construction state is unavailable")?;
        let position = scanner
            .position
            .context("Mech is not on a map:  Can't set team")?;
        ensure!(
            before.btech.maps().contains_key(&position.map),
            "Map is unavailable"
        );
        let team = team.max(0);
        let mut signature = scanner.signature;
        signature.team = team;
        super::set_sensor_signature(&mut scripts.world_mut(), unit, signature)?;
        scripts.world().validate(config)?;
        super::notify_message(
            scripts,
            super::BattleMessageTarget::Player(actor),
            &format!("Team set to {team}"),
        )?;
        scripts.effects.validate()?;
        Ok(team)
    })();
    if result.is_err() {
        *scripts.world_mut() = before;
        scripts.effects.restore(checkpoint);
    }
    result
}

/// Native SETTEAM edits the wizard's occupied physical unit and accepts one signed integer.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let team = input.args.trim().parse::<i32>().context("Invalid team!")?;
        let unit = ctx
            .scripts
            .world()
            .objects
            .get(&ctx.player)
            .and_then(|actor| actor.location)
            .context("Player has no location")?;
        set_team_action(ctx.scripts, ctx.config, ctx.player, unit, team)
    })();
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
