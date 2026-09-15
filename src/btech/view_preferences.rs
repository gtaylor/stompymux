//! Saved player display preferences, independent of cockpit state and explicit renderer overrides.
use super::BattleViewDimensions;
use crate::{Flag, Kind, ObjectId, World};
use anyhow::{Result, ensure};
use std::sync::Arc;

/// Player-owned map dimensions and unit-list inclusion settings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BattlePlayerPreferences {
    pub dimensions: BattleViewDimensions,
    pub contacts: super::BattleContactPreferences,
}

/// Read saved dimensions or the standard defaults for a live player.
pub fn view_dimensions(world: &World, player: ObjectId) -> Result<BattleViewDimensions> {
    ensure!(
        world.objects.get(&player).is_some_and(
            |object| object.kind == Kind::Player && !object.flags.contains(Flag::Going)
        ),
        "Player is unavailable"
    );
    Ok(world
        .btech
        .player_preferences
        .get(&player)
        .copied()
        .unwrap_or_default()
        .dimensions)
}

/// Save validated dimensions; trusted host code owns authorization to change the selected player.
/// Passing the default dimensions resets sizing without clearing unrelated player configuration.
pub fn set_view_dimensions(
    world: &mut World,
    player: ObjectId,
    dimensions: BattleViewDimensions,
) -> Result<()> {
    view_dimensions(world, player)?;
    dimensions.validate()?;
    Arc::make_mut(&mut world.btech.player_preferences)
        .entry(player)
        .or_default()
        .dimensions = dimensions;
    Ok(())
}

/// Self-service sizing; parse and validate before changing the invoking player's saved state.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let arguments: Vec<_> = input.args.split_whitespace().collect();
        let replacement = match arguments.as_slice() {
            [] => None,
            [reset] if reset.eq_ignore_ascii_case("reset") => Some(BattleViewDimensions::default()),
            [width, height, lrs] => Some(BattleViewDimensions {
                tactical_width: width
                    .parse::<u16>()
                    .map_err(|_| anyhow::anyhow!("Invalid tactical width"))?,
                tactical_height: height
                    .parse::<u16>()
                    .map_err(|_| anyhow::anyhow!("Invalid tactical height"))?,
                long_range_height: lrs
                    .parse::<u16>()
                    .map_err(|_| anyhow::anyhow!("Invalid long-range height"))?,
            }),
            _ => anyhow::bail!("Usage: mapdisplay [width height lrs-height | reset]"),
        };
        if let Some(dimensions) = replacement {
            set_view_dimensions(&mut ctx.scripts.world.borrow_mut(), ctx.player, dimensions)?;
        }
        let dimensions = view_dimensions(&ctx.scripts.world.borrow(), ctx.player)?;
        Ok(format!(
            "Map display: tactical {} wide by {} high; long-range height {}.",
            dimensions.tactical_width, dimensions.tactical_height, dimensions.long_range_height
        ))
    })();
    Ok(crate::CommandAction::Report(crate::CommandReport::Reply(
        match result {
            Ok(text) => text,
            Err(error) => format!("{error:#}"),
        },
    )))
}
