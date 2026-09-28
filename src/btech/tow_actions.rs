//! Native and Lua towing share authorization, material changes and consequence publication.
use super::*;
use crate::{Config, ObjectId, Scripts};
use anyhow::{Context, Result, ensure};

/// Pickup when a target is supplied, otherwise release the carrier's existing tow.
/// Roll back state and staged effects together if any casualty callback or validation fails.
pub fn tow_action(
    scripts: &Scripts,
    config: &Config,
    carrier: ObjectId,
    pilot: ObjectId,
    target: Option<ObjectId>,
) -> Result<()> {
    scripts.atomic(|before| {
        if let Some(target) = target {
            let report = super::pickup_transaction::pickup(
                &mut scripts.world_mut(),
                carrier,
                pilot,
                target,
                BattleFallRules::configured(config),
                config.battletech.tsm_tow_bonus != 0,
                true,
            )?;
            super::piloting::publish_ordered_notices(
                scripts,
                &report.notices,
                &report.pilot_notices,
            )?;
            for flood in &report.flooding {
                super::evacuation::publish_section_exposure_consequences(scripts, config, flood)?;
            }
            if let Some(ice) = report.ice {
                super::evacuation::publish_surface_consequences(scripts, config, &ice)?;
            }
        } else {
            super::targeting::controlled(&scripts.world(), carrier, pilot)?;
            let notices = super::release_tow(&mut scripts.world_mut(), carrier)?;
            for notice in notices {
                super::notify_unit(scripts, notice)?;
            }
        }
        super::evacuation::publish_new_casualties(scripts, config, before)?;
        scripts.world().validate_action(config)?;
        Ok(())
    })
}

/// Parse a map label or dbref and delegate both native commands to the shared action.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let world = ctx.scripts.world();
        let carrier = world
            .objects
            .get(&ctx.player)
            .and_then(|player| player.location)
            .context("Enter a unit first")?;
        let text = input.args.trim();
        let target = if input.name.eq_ignore_ascii_case("pickup") {
            ensure!(!text.is_empty(), "Usage: pickup <target>");
            Some(super::radio_targeted::target(&world, carrier, text)?)
        } else {
            ensure!(text.is_empty(), "Usage: dropoff");
            None
        };
        drop(world);
        tow_action(ctx.scripts, ctx.config, carrier, ctx.player, target)
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
