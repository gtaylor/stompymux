//! Wizard-directed predictive firing composes motion prediction, targeting and normal weapon actions.
use crate::{Config, ObjectId, Scripts};
use anyhow::{Context, Result, ensure};

/// Predict each selected shot against current target state and use the ordinary launch transaction.
/// Recoverable weapon rejections are reported individually; host/output failures roll back the batch.
pub fn snipe_action(
    scripts: &Scripts,
    config: &Config,
    shooter: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    selection: &str,
) -> Result<()> {
    let before = scripts.world().clone();
    let checkpoint = scripts.effects.checkpoint();
    let result = (|| {
        ensure!(
            crate::authority::is_wizard(&before, pilot),
            "Permission denied."
        );
        super::targeting::controlled(&before, shooter, pilot)?;
        let weapons = super::tic::selection(selection, 96)?;
        let rules = super::BattleMovementRules {
            fasa_turning: config.battletech.fasaturn != 0,
            slowdown: config.battletech.slowdown,
            tsm_tow_bonus: config.battletech.tsm_tow_bonus != 0,
            ..super::BattleMovementRules::STANDARD
        };
        for weapon in weapons {
            let prediction =
                super::predict_artillery_target(&scripts.world(), shooter, target, rules)?;
            let selected = super::targeting::selection(&scripts.world(), shooter);
            if !matches!(selected, Some(super::BattleTargetSelection::Hex(lock)) if lock.hex == prediction.coordinate)
            {
                let notice = super::select_hex_target(
                    &mut scripts.world_mut(),
                    shooter,
                    pilot,
                    prediction.coordinate,
                    super::BattleHexTargetMode::UnitAtHex,
                )?;
                super::notify_unit_text(scripts, notice.unit, &notice.text)?;
            }
            let attempt = super::evacuation::attempt_configured_firing_action(
                scripts,
                config,
                shooter,
                pilot,
                weapon,
                super::fire_target::FireTargetRequest::Target(super::BattleFireTarget::Selected),
            )?;
            if let Err(message) = attempt {
                super::notify_message(
                    scripts,
                    super::BattleMessageTarget::Player(pilot),
                    &message,
                )?;
            }
        }
        scripts.world().validate(config)?;
        scripts.effects.validate()?;
        Ok(())
    })();
    if result.is_err() {
        *scripts.world_mut() = before;
        scripts.effects.restore(checkpoint);
    }
    result
}

/// Native syntax accepts a battlefield target ID and the shared comma/range weapon selection.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let args: Vec<_> = input.args.split_whitespace().collect();
        ensure!(
            args.len() == 2,
            "Please supply target ID _and_ weapon(s) to use"
        );
        let world = ctx.scripts.world();
        ensure!(
            crate::authority::is_wizard(&world, ctx.player),
            "Permission denied."
        );
        let shooter = world
            .objects
            .get(&ctx.player)
            .and_then(|p| p.location)
            .context("Enter a unit first")?;
        let target = super::radio_targeted::target(&world, shooter, args[0])?;
        drop(world);
        snipe_action(
            ctx.scripts,
            ctx.config,
            shooter,
            ctx.player,
            target,
            args[1],
        )
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
