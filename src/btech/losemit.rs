//! Wizard battlefield emotes share live observer selection and transactional notification delivery.
use crate::{Flag, ObjectId, Scripts};
use anyhow::{Context, Result, ensure};

/// Publish to visible observers, excluding the source, then privately confirm to the wizard.
/// Source power and pilot assignment do not gate this scenario operation.
pub fn losemit_action(
    scripts: &Scripts,
    actor: ObjectId,
    unit: ObjectId,
    message: &str,
) -> Result<usize> {
    scripts.atomic(|_| {
        let world = scripts.world();
        ensure!(
            crate::authority::is_wizard(&world, actor),
            "Permission denied."
        );
        ensure!(!world.btech.unconscious(actor), "You are unconscious");
        ensure!(
            world
                .objects
                .get(&unit)
                .is_some_and(|object| object.kind == crate::Kind::Thing
                    && !object.flags.contains(Flag::Going)),
            "Unit is unavailable"
        );
        let source = super::scanner::scanner_unit(&world, unit)
            .context("Unit construction state is unavailable")?;
        let position = source.position.context("You are on no map!")?;
        ensure!(
            world.btech.maps().contains_key(&position.map),
            "You are on an invalid map!"
        );
        let messages = super::observer_messages(&world, unit, message);
        drop(world);
        for (observer, text) in &messages {
            super::notify_message(
                scripts,
                super::BattleMessageTarget::Unit(*observer),
                &crate::text::escape(text),
            )?;
        }
        super::notify_message(
            scripts,
            super::BattleMessageTarget::Player(actor),
            "Broadcast done.",
        )?;
        scripts.effects.validate()?;
        Ok(messages.len())
    })
}

/// Native @LOSEMIT uses the occupied physical unit and preserves the message payload.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let unit = ctx
            .scripts
            .world()
            .objects
            .get(&ctx.player)
            .and_then(|actor| actor.location)
            .context("Player has no location")?;
        losemit_action(ctx.scripts, ctx.player, unit, &input.args)
    })();
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
