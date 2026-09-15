//! Operator map broadcasts select running, conscious crews without requiring a sensor contact.
use crate::{ObjectId, Scripts, World};
use anyhow::{Result, ensure};

/// Capture eligible units in persisted map-slot order before publishing any messages.
fn recipients(world: &World, map: ObjectId) -> Result<Vec<ObjectId>> {
    Ok(super::map_slots::all_unit_order(world, map)?
        .into_iter()
        .filter(|&id| {
            world.objects.get(&id).is_some_and(|object| {
                object.kind != crate::Kind::Garbage && !object.flags.contains(crate::Flag::Going)
            }) && super::scanner::scanner_unit(world, id)
                .is_some_and(|unit| unit.power == super::BattlePower::Running)
                && !super::crew::unit_unconscious(world, id)
                && !super::battle_unit_blinded(world, id)
        })
        .collect())
}

/// Broadcast to cockpit occupants and confirm to the wizard in one notification transaction.
/// Returned dbrefs identify eligible units, including empty cockpits, rather than counting players.
pub fn emit_map_action(
    scripts: &Scripts,
    actor: ObjectId,
    map: ObjectId,
    text: &str,
) -> Result<Vec<ObjectId>> {
    let before = scripts.world().clone();
    let checkpoint = scripts.effects.checkpoint();
    let result = (|| {
        ensure!(
            crate::authority::is_wizard(&before, actor),
            "Permission denied."
        );
        ensure!(
            before
                .objects
                .get(&map)
                .is_some_and(|object| object.kind != crate::Kind::Garbage
                    && !object.flags.contains(crate::Flag::Going)),
            "Map is unavailable"
        );
        let text = text.trim_start_matches(' ');
        ensure!(!text.is_empty(), "What do you want to @mapemit?");
        ensure!(
            !text.contains('\0'),
            "message contains an embedded NUL byte"
        );
        let units = recipients(&before, map)?;
        for &unit in &units {
            crate::notification::send(
                &scripts.world(),
                &scripts.outbox,
                &crate::lua::configuration(&scripts.lua),
                crate::notification::Request {
                    target: unit,
                    sender: ObjectId(-1),
                    document: text.into(),
                    policy: crate::notification::Policy::ROOM,
                    exclusions: Some(vec![unit]),
                },
            )?;
        }
        super::notify_message(
            scripts,
            super::BattleMessageTarget::Player(actor),
            "Message sent!",
        )?;
        scripts.effects.validate()?;
        Ok(units)
    })();
    if result.is_err() {
        *scripts.world_mut() = before;
        scripts.effects.restore(checkpoint);
    }
    result
}

/// The native operator broadcasts to the map containing their player object.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| -> Result<()> {
        let map = super::special_dispatch::object(ctx)?;
        emit_map_action(ctx.scripts, ctx.player, map, &input.args)?;
        Ok(())
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
