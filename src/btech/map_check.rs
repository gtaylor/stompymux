//! Map membership inspection uses the derived shared index and typed world invariants.
use crate::{Config, ObjectId, Scripts};
use anyhow::{Result, ensure};
use serde::Serialize;

/// Checked members in persisted map-slot order, including destroyed units.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleMapCheck {
    pub map: ObjectId,
    pub units: Vec<ObjectId>,
}

/// Validate map membership before publishing success; no independent index needs rebuilding.
/// Invalid snapshots reject atomically instead of clearing placement or invoking shutdown.
pub fn check_map_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    map: ObjectId,
) -> Result<BattleMapCheck> {
    scripts.atomic(|_| {
        let world = scripts.world();
        ensure!(
            crate::authority::is_wizard(&world, actor),
            "Permission denied."
        );
        ensure!(
            world.objects.get(&map).is_some_and(
                |o| o.kind != crate::Kind::Garbage && !o.flags.contains(crate::Flag::Going)
            ),
            "Map is unavailable"
        );
        let units = super::map_slots::all_unit_order(&world, map)?;
        world.validate(config)?;
        drop(world);
        let span = super::map_slots::extent(&scripts.world().btech, map);
        let report = BattleMapCheck { map, units };
        super::notify_message(
            scripts,
            super::BattleMessageTarget::Player(actor),
            &format!("Checking {span} entries.."),
        )?;
        super::notify_message(scripts, super::BattleMessageTarget::Player(actor), "Done.")?;
        scripts.effects.validate()?;
        Ok(report)
    })
}

/// FIXMAP checks the actor's selected map; trailing input has no effect.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    _input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let map = super::special_dispatch::object(ctx)?;
        check_map_action(ctx.scripts, ctx.config, ctx.player, map)
    })();
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
