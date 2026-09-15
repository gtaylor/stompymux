//! Operator map loading composes asset activation, retained physical altitude and shared shutdown.
use crate::{Config, ObjectId, Scripts};
use anyhow::{Context, Result, ensure};
use std::collections::BTreeSet;

/// Initialize or replace a map asset atomically with its terrain diagnostics.
pub(crate) fn initialize_map_action(
    scripts: &Scripts,
    config: &Config,
    id: ObjectId,
    name: &str,
    create: bool,
) -> Result<()> {
    let before = scripts.world().clone();
    let checkpoint = scripts.effects.checkpoint();
    let result = (|| {
        let (asset, warnings) = super::assets::read_map_diagnostics(
            &config.path(&config.database.map_database),
            name,
            i32::try_from(before.btech.maps().get(&id).map_or(0, |map| map.flags))
                .context("Invalid map flags")?,
        )?;
        if create {
            super::create_map(&mut scripts.world_mut(), id, name, asset)?;
        } else {
            super::reload_map(&mut scripts.world_mut(), id, name, asset)?;
        }
        super::assets::publish_map_warnings(scripts, config, id, &warnings)
    })();
    if result.is_err() {
        *scripts.world_mut() = before;
        scripts.effects.restore(checkpoint);
    }
    result
}

/// Load a map or publish its structural preflight failure after restoring map state.
/// Diagnostic publication is atomic; an enclosing failed callback also restores it.
pub fn load_map_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    id: ObjectId,
    name: &str,
) -> Result<()> {
    let result = load_map_state_action(scripts, config, actor, id, name);
    let Some(failure) = result
        .as_ref()
        .err()
        .and_then(|error| error.downcast_ref::<super::map::MapFileFailure>())
    else {
        return result;
    };
    let text = match failure {
        super::map::MapFileFailure::Unavailable => return result,
        super::map::MapFileFailure::Dimensions => {
            format!("Map #{}: Invalid height and or/width on {name}", id.0)
        }
        super::map::MapFileFailure::Rows => format!(
            "Map #{}: Mapfile possibly corrupt and/or height/width flipped. Height != what was read in {name}",
            id.0
        ),
    };
    let before = scripts.world().clone();
    let checkpoint = scripts.effects.checkpoint();
    let published = super::channels::publish(
        scripts,
        config,
        &[super::BattleChannelMessage::new(
            super::BattleChannel::MapErrors,
            text,
        )],
    )
    .and_then(|()| scripts.effects.validate());
    if let Err(error) = published {
        *scripts.world_mut() = before;
        scripts.effects.restore(checkpoint);
        return Err(error);
    }
    result
}

/// Load validated terrain before clearing ordinary wizard membership; GOD retains placed units.
fn load_map_state_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    id: ObjectId,
    name: &str,
) -> Result<()> {
    let before = scripts.world().clone();
    let checkpoint = scripts.effects.checkpoint();
    let result = (|| {
        ensure!(
            crate::authority::is_wizard(&before, actor),
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
        let (mut asset, warnings) = super::assets::read_map_diagnostics(
            &config.path(&config.database.map_database),
            name,
            i32::try_from(before.btech.maps().get(&id).map_or(0, |map| map.flags))
                .context("Invalid map flags")?,
        )
        .map_err(
            |error| match error.downcast_ref::<super::map::MapFileFailure>() {
                Some(failure) => anyhow::anyhow!(*failure),
                None => error,
            },
        )?;
        asset.generate_bridges()?;
        let units = super::map_slots::all_unit_order(&before, id)?;
        let mut occupied = BTreeSet::new();
        for unit in units {
            let position = super::scanner::scanner_unit(&before, unit)
                .context("Unit is unavailable")?
                .position
                .context("Unit is not placed")?;
            ensure!(
                position.x < asset.width && position.y < asset.height,
                "Clear or move units before loading a smaller map"
            );
            occupied.insert((i32::from(position.x), i32::from(position.y)));
        }
        super::notify_message(
            scripts,
            super::BattleMessageTarget::Player(actor),
            &format!("Loading {name}"),
        )?;
        {
            let mut world = scripts.world_mut();
            for (x, y) in occupied {
                super::terrain_edit::replace_hex(
                    &mut world,
                    id,
                    super::BattleHexCoordinate { x, y },
                    asset.hex(x, y).context("Map coordinates out of bounds")?,
                )?;
            }
            super::map_objects::clear(&mut world, id)?;
            super::state::replace_map_asset(&mut world, id, name, asset)?;
            world.validate(config)?;
        }
        super::assets::publish_map_warnings(scripts, config, id, &warnings)?;
        if actor != ObjectId(1) {
            super::notify_message(
                scripts,
                super::BattleMessageTarget::Player(actor),
                "Clearing Mechs off Newly Loaded Map",
            )?;
            super::clear_map_units_action(scripts, config, actor, id)?;
            super::map_objects::clear(&mut scripts.world_mut(), id)?;
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

/// Load one relative asset into the operator's selected map.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| -> Result<()> {
        let name = input
            .args
            .split([' ', '\t'])
            .find(|part| !part.is_empty())
            .context("Invalid number of arguments!")?;
        let map = super::special_dispatch::object(ctx)?;
        load_map_action(ctx.scripts, ctx.config, ctx.player, map, name)
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            let reply = if let Some(failure) = error.downcast_ref::<super::map::MapFileFailure>() {
                let name = input
                    .args
                    .split([' ', '\t'])
                    .find(|part| !part.is_empty())
                    .unwrap_or("");
                format!("Loading {name}\r\n{failure}")
            } else {
                format!("{error:#}")
            };
            crate::CommandAction::Report(crate::CommandReport::Reply(reply))
        }
    })
}
