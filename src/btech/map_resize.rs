//! Transactional map resizing preserves overlapping visible terrain, fire, smoke and
//! decorations, and removes other map objects.
use crate::{Config, ObjectId, Scripts};
use anyhow::{Context, Result, ensure};
use std::sync::Arc;

/// Resize a wizard's map, retaining valid unit positions and the fire, smoke and decorations
/// still on it, and clearing other map-object state.
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
        let old = before.btech.maps().get(&id).context("Map not found")?;
        ensure!(
            (1..=1000).contains(&width) && (1..=1000).contains(&height),
            "Map dimensions must be between 1 and 1000"
        );
        ensure!(old.terrain_ready(), "Map terrain is unavailable");
        let mut tiles =
            vec![super::Hex::new(super::Terrain::Grassland, 0); (width * height) as usize];
        for y in 0..height.min(old.height) {
            for x in 0..width.min(old.width) {
                tiles[(y * width + x) as usize] = old.base_hex(x, y)?;
            }
        }
        // Fire, smoke and decorations stay where they are if their hex is still on the map.
        let inside = |x: i64, y: i64| x < width && y < height;
        let decorations: std::collections::BTreeMap<_, _> = old
            .decorations
            .iter()
            .filter_map(|(&index, &effect)| {
                let (x, y) = (i64::from(index) % old.width, i64::from(index) / old.width);
                inside(x, y).then(|| ((y * width + x) as u32, effect))
            })
            .collect();
        let static_decorations = old.static_decorations.clone().map(|records| {
            Arc::new(
                records
                    .iter()
                    .filter(|(_, record)| {
                        inside(
                            i64::from(record.coordinate.x),
                            i64::from(record.coordinate.y),
                        )
                    })
                    .map(|(&ordinal, &record)| (ordinal, record))
                    .collect(),
            )
        });
        {
            let mut world = scripts.world_mut();
            super::map_objects::clear(&mut world, id)?;
            let map = world.btech.maps.get_mut(&id).unwrap();
            map.width = width;
            map.height = height;
            map.terrain = Some(Arc::new(tiles));
            map.decorations = Arc::new(decorations);
            map.static_decorations = static_decorations;
            // Points of interest cropped off the map are dropped with the terrain under them.
            Arc::make_mut(&mut map.points_of_interest)
                .retain(|point| inside(i64::from(point.x), i64::from(point.y)));
            world.validate(config)?;
        }
        super::notify_message(scripts, super::MessageTarget::Player(actor), "Size set.")?;
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
