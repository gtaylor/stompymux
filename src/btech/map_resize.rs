//! Transactional map resizing preserves overlapping visible terrain and removes map objects.
use crate::{Config, ObjectId, Scripts};
use anyhow::{Context, Result, ensure};
use std::sync::Arc;

/// Resize a wizard's map, retaining valid unit positions and clearing reference map-object state.
/// Cropping an occupied coordinate or active event is rejected by world validation, atomically.
pub fn resize_map_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    id: ObjectId,
    width: i64,
    height: i64,
) -> Result<()> {
    scripts.atomic(|before| {
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
        let old = before.btech.maps().get(&id).context("Map not found")?;
        require_no_lookup(old)?;
        ensure!(
            (1..=1000).contains(&width) && (1..=1000).contains(&height),
            "Map dimensions must be between 1 and 1000"
        );
        ensure!(old.terrain_ready(), "Map terrain is unavailable");
        let mut tiles = vec![
            super::BattleHex {
                terrain: super::Terrain::Grassland,
                elevation: 0
            };
            (width * height) as usize
        ];
        for y in 0..height.min(old.height) {
            for x in 0..width.min(old.width) {
                tiles[(y * width + x) as usize] = old.hex(x, y)?;
            }
        }
        {
            let mut world = scripts.world_mut();
            let map = world.btech.maps.get_mut(&id).unwrap();
            map.width = width;
            map.height = height;
            map.terrain = Some(Arc::new(tiles));
            super::map_objects::clear(&mut world, id)?;
            world.validate(config)?;
        }
        super::notify_message(
            scripts,
            super::BattleMessageTarget::Player(actor),
            "Size set.",
        )?;
        scripts.effects.validate()?;
        Ok(())
    })
}

/// The native operator resizes the selected map using two checked integer dimensions.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| -> Result<()> {
        let map = super::special_dispatch::object(ctx)?;
        require_no_lookup(
            ctx.scripts
                .world()
                .btech
                .maps()
                .get(&map)
                .context("Map not found")?,
        )?;
        let args: Vec<_> = input.args.split_whitespace().collect();
        ensure!(args.len() == 2, "Usage: SETMAPSIZE <X> <Y>");
        resize_map_action(
            ctx.scripts,
            ctx.config,
            ctx.player,
            map,
            args[0].parse()?,
            args[1].parse()?,
        )
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

/// The information-object refusal precedes dimension parsing, even for empty cache rows.
fn require_no_lookup(map: &super::StoredBattleMap) -> Result<()> {
    ensure!(
        !map.has_lookup_object(),
        "Invalid map for size change, sorry."
    );
    Ok(())
}
