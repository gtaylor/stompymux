//! Bounded depth-first route rebuilding shares authored configuration and runtime route setters.
use crate::{Config, ObjectId, Scripts};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// Counts of installed runtime records and descents skipped for cycles or depth.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct BattleMapLinkUpdate {
    pub buildings: usize,
    pub leaves: usize,
    pub entrances: usize,
    pub skipped: usize,
}

/// Explicit work items preserve depth-first visitation without consuming the Rust call stack.
enum Work {
    Visit {
        parent: Option<ObjectId>,
        map: ObjectId,
        depth: usize,
    },
    Child {
        parent: ObjectId,
        child: ObjectId,
        coordinate: super::BattleHexCoordinate,
        slot: u32,
        depth: usize,
    },
}

/// Rebuild reachable maps once per invocation, rolling back all route and output changes on failure.
pub fn update_map_links_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    root: ObjectId,
) -> Result<BattleMapLinkUpdate> {
    scripts.atomic(|before| {
        ensure!(
            crate::authority::is_wizard(&before, actor),
            "Permission denied."
        );
        ensure!(
            before
                .objects
                .get(&root)
                .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
            "Map is unavailable"
        );
        ensure!(before.btech.maps().contains_key(&root), "Map not found");
        let mut children: BTreeMap<ObjectId, Vec<_>> = BTreeMap::new();
        for (&child, map) in before.btech.maps() {
            if let Some(link) = map.authored_link() {
                children
                    .entry(link.parent)
                    .or_default()
                    .push((child, link.coordinate));
            }
        }
        let mut visited = BTreeSet::new();
        let mut pending = vec![Work::Visit {
            parent: None,
            map: root,
            depth: 0,
        }];
        let mut stats = BattleMapLinkUpdate::default();
        while let Some(work) = pending.pop() {
            match work {
                Work::Child {
                    parent,
                    child,
                    coordinate,
                    slot,
                    depth,
                } => {
                    super::set_building_entrance(
                        &mut scripts.world_mut(),
                        parent,
                        slot,
                        Some(super::BattleBuildingEntrance {
                            coordinate,
                            interior: child,
                            data_char: 0,
                            data_short: 0,
                            data_int: 0,
                        }),
                    )?;
                    stats.buildings += 1;
                    pending.push(Work::Visit {
                        parent: Some(parent),
                        map: child,
                        depth,
                    });
                }
                Work::Visit { parent, map, depth } => {
                    if depth >= 1024 || !visited.insert(map) {
                        stats.skipped += 1;
                        continue;
                    }
                    let mut world = scripts.world_mut();
                    let record = world
                        .btech
                        .maps()
                        .get(&map)
                        .context("Map not found")?
                        .clone();
                    world
                        .btech
                        .maps
                        .get_mut(&map)
                        .unwrap()
                        .clear_lookup_kind(super::map_bits::LookupKind::Hangar);
                    if let Some(parent) = parent {
                        world.btech.maps.get_mut(&map).unwrap().building_parent = parent.0;
                        world.btech.maps.get_mut(&map).unwrap().building_exits = Default::default();
                        super::set_building_exit(&mut world, map, 0, Some(parent))?;
                        stats.leaves += 1;
                        world
                            .btech
                            .maps
                            .get_mut(&map)
                            .unwrap()
                            .building_entry_points = Default::default();
                        if let Some(link) = record.authored_link() {
                            let points: Vec<_> = link
                                .entrances
                                .into_iter()
                                .enumerate()
                                .filter_map(|(direction, entry)| {
                                    entry
                                        .coordinate(&record, direction)
                                        .map(|coordinate| (direction, coordinate))
                                })
                                .collect();
                            for (slot, (direction, coordinate)) in
                                points.into_iter().rev().enumerate()
                            {
                                super::set_building_entry_point(
                                    &mut world,
                                    map,
                                    slot as u32,
                                    Some(super::BattleBuildingEntryPoint {
                                        coordinate,
                                        direction: b"nesw"[direction],
                                        object: crate::ObjectId(-1),
                                        data_short: 0,
                                        data_int: 0,
                                    }),
                                )?;
                                stats.entrances += 1;
                            }
                        }
                    }
                    for &slot in record.building_entrances().keys() {
                        super::set_building_entrance(&mut world, map, slot, None)?;
                    }
                    let valid: Vec<_> = children
                        .get(&map)
                        .into_iter()
                        .flatten()
                        .filter(|(_, p)| {
                            p.x >= 0
                                && p.y >= 0
                                && i64::from(p.x) < record.width
                                && i64::from(p.y) < record.height
                        })
                        .copied()
                        .collect();
                    for (index, (child, coordinate)) in valid.iter().enumerate().rev() {
                        pending.push(Work::Child {
                            parent: map,
                            child: *child,
                            coordinate: *coordinate,
                            slot: u32::try_from(valid.len() - 1 - index)?,
                            depth: depth + 1,
                        });
                    }
                }
            }
        }
        super::notify_message(
            scripts,
            super::BattleMessageTarget::Player(actor),
            &format!(
                "Updated {} BUILD objs, {} LEAVE objs, {} ENTRANCE objs; skipped {} link descents.",
                stats.buildings, stats.leaves, stats.entrances, stats.skipped
            ),
        )?;
        scripts.world().validate(config)?;
        scripts.effects.validate()?;
        Ok(stats)
    })
}

/// Native UPDATELINKS ignores its argument and rebuilds from the wizard's selected map.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    _input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| -> Result<()> {
        let map = super::special_dispatch::object(ctx)?;
        update_map_links_action(ctx.scripts, ctx.config, ctx.player, map)?;
        Ok(())
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
