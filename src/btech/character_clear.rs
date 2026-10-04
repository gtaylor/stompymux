//! Wizard character resets restore effective defaults without resetting unit or recovery lifecycles.
use crate::{Flag, Kind, ObjectId, World};
use anyhow::{Result, ensure};

/// Clear personal stats atomically while preserving pending recovery and its random stream.
pub fn clear_character(world: &mut World, actor: ObjectId, player: ObjectId) -> Result<()> {
    ensure!(
        crate::authority::is_wizard(world, actor),
        "Permission denied."
    );
    ensure!(
        world.objects.get(&player).is_some_and(
            |object| object.kind == Kind::Player && !object.flags.contains(Flag::Going)
        ),
        "I don't know who that is"
    );
    world.attempt(|world| {
        // An explicit profile represents the reference's absent-state read defaults.
        // Removing attributes would invalidate active character recovery in this model.
        world.btech.characters.insert(
            player,
            super::Character {
                build: 1,
                reflexes: 1,
                intuition: 1,
                learn: 1,
                charisma: 1,
                bruise: 0,
                lethal: 0,
            },
        );
        world.btech.character_values.remove(&player);
        if let Some(recovery) = world.btech.recoveries.get_mut(&player) {
            // Future checks see the cleared advantages; the countdown and dice survive.
            recovery.pain_resistance = false;
            recovery.toughness = false;
        }
        world.btech.validate(world)?;
        Ok(())
    })
}

/// Native lookup accepts the same player names, account aliases and dbrefs as other controls.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let name = input.args.trim();
    let text = if name.is_empty() {
        "Who do you want to clear the stats from?".to_owned()
    } else {
        let player = ctx.scripts.world().find_player(name);
        match player {
            Some(player) => match clear_character(&mut ctx.scripts.world_mut(), ctx.player, player)
            {
                Ok(()) => format!("Player #{} stats cleared", player.0),
                Err(error) => format!("{error:#}"),
            },
            None => "I don't know who that is".to_owned(),
        }
    };
    Ok(crate::CommandAction::Report(crate::CommandReport::Reply(
        text,
    )))
}
