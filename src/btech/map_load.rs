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
    scripts.atomic(|before| {
        let asset = super::assets::read_map_with_flags(
            &config.path(&config.database.map_database),
            name,
            before.btech.maps().get(&id).map_or(0, |map| map.flags),
        )?;
        if create {
            super::create_map(&mut scripts.world_mut(), id, name, asset)
        } else {
            super::reload_map(&mut scripts.world_mut(), id, name, asset)
        }
    })
}

/// Load a map, or restore map state and log why an invalid file was rejected.
/// The error log waits for commit, so an enclosing failed callback discards it.
pub fn load_map_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    id: ObjectId,
    name: &str,
) -> Result<()> {
    let result = load_map_state_action(scripts, config, actor, id, name);
    let Some(error) = result.as_ref().err().filter(|error| {
        matches!(
            error.downcast_ref::<super::assets::MapFileFailure>(),
            Some(super::assets::MapFileFailure::Invalid)
        )
    }) else {
        return result;
    };
    let reason = error.root_cause().to_string();
    let text = format!("Map #{}: {name} is not a valid map file: {reason}", id.0);
    super::diagnostics::publish(
        scripts,
        &[super::DiagnosticMessage::new(
            super::TraceTopic::MapLoad,
            text,
        )],
    );
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
        let asset = super::assets::read_map_with_flags(
            &config.path(&config.database.map_database),
            name,
            before.btech.maps().get(&id).map_or(0, |map| map.flags),
        )
        .map_err(
            |error| match error.downcast_ref::<super::assets::MapFileFailure>() {
                // Report the failure itself, keeping the decoder's reason beneath it.
                Some(failure) => anyhow::anyhow!(error.root_cause().to_string()).context(*failure),
                None => error,
            },
        )?;
        let units = super::map_slots::all_unit_order(before, id)?;
        let mut occupied = BTreeSet::new();
        for unit in units {
            let position = super::scanner::scanner_unit(before, unit)
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
            super::MessageTarget::Player(actor),
            &format!("Loading {name}"),
        )?;
        {
            let mut world = scripts.world_mut();
            for (x, y) in occupied {
                super::terrain_edit::replace_hex(
                    &mut world,
                    id,
                    super::HexCoordinate { x, y },
                    asset.hex(x, y).context("Map coordinates out of bounds")?,
                )?;
            }
            super::map_objects::clear(&mut world, id)?;
            super::state::replace_map_asset(&mut world, id, name, asset)?;
            world.validate(config)?;
        }
        if actor != ObjectId(1) {
            super::notify_message(
                scripts,
                super::MessageTarget::Player(actor),
                "Clearing Units off Newly Loaded Map",
            )?;
            super::clear_map_units_action(scripts, config, actor, id)?;
            super::map_objects::clear(&mut scripts.world_mut(), id)?;
        }
        scripts.world().validate(config)?;
        scripts.effects.validate()?;
        Ok(())
    })
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
            let reply = if let Some(failure) = error.downcast_ref::<super::assets::MapFileFailure>()
            {
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
