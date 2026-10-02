//! Narrow read and trusted mutation services used by the C-compatible Lua map package.

use super::*;
use crate::{Config, Flag, Kind, ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};
use std::collections::{BTreeMap, BTreeSet};

/// A live unit sample in the persisted mixed map-slot order.
#[derive(Debug, Clone)]
pub struct BattleMapMember {
    pub id: ObjectId,
    pub label: String,
    pub position: BattlePosition,
    pub point: BattlePoint,
    /// Continuous altitude in the C API's map-coordinate scale.
    pub z: f64,
}

/// Return constructed Mechs and vehicles in their common battlefield order.
pub fn battle_map_members(world: &World, map: ObjectId) -> Result<Vec<BattleMapMember>> {
    super::map_slots::all_unit_order(world, map)?
        .into_iter()
        .map(|id| {
            let unit = super::scanner::scanner_unit(world, id).context("Unit is unavailable")?;
            let position = unit.position.context("Unit is not placed")?;
            let point = unit.point.context("Unit runtime state is unavailable")?;
            let tile = world
                .btech
                .maps()
                .get(&map)
                .context("Map not found")?
                .base_hex(i64::from(position.x), i64::from(position.y))?;
            let z = if let Some(vehicle) = world.btech.vehicles().get(&id) {
                vehicle.altitude(tile)
            } else {
                world.btech.constructed_units()[&id]
                    .retained_altitude()
                    .unwrap_or(f64::from(tile.standing_height()))
            };
            Ok(BattleMapMember {
                id,
                label: unit.label().context("Placed unit lacks an ID")?,
                position,
                point,
                z,
            })
        })
        .collect()
}

/// Locate a unit label relative to either a unit's battlefield or a map.
pub fn battle_map_unit_by_label(
    world: &World,
    origin: ObjectId,
    label: &str,
) -> Result<Option<ObjectId>> {
    let map = if world.btech.maps().contains_key(&origin) {
        origin
    } else {
        let unit = super::scanner::scanner_unit(world, origin)
            .context("object must be a BTech unit or map")?;
        let Some(position) = unit.position else {
            return Ok(None);
        };
        position.map
    };
    Ok(battle_map_members(world, map)?
        .into_iter()
        .find_map(|unit| unit.label.eq_ignore_ascii_case(label).then_some(unit.id)))
}

/// A range endpoint in continuous horizontal coordinates and terrain-height units.
#[derive(Debug, Clone, Copy)]
pub struct BattleMapSpatialPoint {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Project a hex center with the reference runtime's exact float32 arithmetic.
pub fn battle_map_hex_point(coordinate: BattleHexCoordinate, z: f64) -> BattleMapSpatialPoint {
    const ALPHA: f32 = 93.09773;
    const SCALE_MAP: f32 = 322.5;
    BattleMapSpatialPoint {
        x: (2.0 + 3.0 * coordinate.x as f32) * ALPHA,
        y: (if coordinate.x % 2 == 0 {
            0.5 * SCALE_MAP
        } else {
            0.0
        }) + coordinate.y as f32 * SCALE_MAP,
        z: z as f32 * 64.5,
    }
}

/// Return the live battlefield containing a unit.
pub fn battle_map_unit_map(world: &World, unit: ObjectId) -> Result<ObjectId> {
    super::scanner::scanner_unit(world, unit)
        .context("unit runtime state is unavailable")?
        .position
        .context("observer is not on a live map")
        .map(|position| position.map)
}

/// Read a live unit endpoint and require membership on the supplied map.
pub fn battle_map_unit_point(
    world: &World,
    map: ObjectId,
    unit: ObjectId,
) -> Result<BattleMapSpatialPoint> {
    let sample = battle_map_members(world, map)?
        .into_iter()
        .find(|sample| sample.id == unit)
        .context("unit endpoint is not on the supplied map")?;
    Ok(BattleMapSpatialPoint {
        x: (sample.point.x * 322.5) as f32,
        y: (sample.point.y * 322.5) as f32,
        z: (sample.z * 64.5) as f32,
    })
}

/// Measure the C API's three-dimensional map range.
pub fn battle_map_spatial_range(
    from: BattleMapSpatialPoint,
    to: BattleMapSpatialPoint,
) -> Result<f64> {
    let dx = from.x - to.x;
    let dy = from.y - to.y;
    let dz = from.z - to.z;
    Ok(((dx * dx + dy * dy + dz * dz).sqrt() / 322.5_f32) as f64)
}

/// Result vocabulary exposed by `btech.map.line_of_sight`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleMapLos {
    Clear,
    Blocked,
    None,
}

/// Query a unit-to-hex terrain ray without updating contacts.
pub fn battle_map_hex_los(
    world: &World,
    observer: ObjectId,
    target: BattleHexCoordinate,
) -> Result<BattleMapLos> {
    let (terrain, _) = super::los::unit_hex_los(world, observer, target)?;
    Ok(if terrain.blocked {
        BattleMapLos::Blocked
    } else {
        BattleMapLos::Clear
    })
}

/// Query current perception, retaining the C clear/blocked/none distinction.
pub fn battle_map_unit_los(
    world: &World,
    observer: ObjectId,
    target: ObjectId,
) -> Result<BattleMapLos> {
    let source = super::scanner::scanner_unit(world, observer)
        .context("observer runtime state is unavailable")?;
    let destination = super::scanner::scanner_unit(world, target)
        .context("target runtime state is unavailable")?;
    let source_position = source.position.context("observer is not on a live map")?;
    let Some(target_position) = destination.position else {
        return Ok(BattleMapLos::None);
    };
    if source_position.map != target_position.map {
        return Ok(BattleMapLos::None);
    }
    let terrain = super::unit_terrain_los(world, observer, target)?;
    let visible =
        source.visibility.clairvoyant || super::perceive(world, observer, target)?.is_some();
    Ok(if !visible {
        BattleMapLos::None
    } else if terrain.blocked {
        BattleMapLos::Blocked
    } else {
        BattleMapLos::Clear
    })
}

/// Trusted exact placement preserves a tow pair and commits only a valid complete candidate.
pub fn place_battle_map_unit(
    world: &mut World,
    unit: ObjectId,
    map: ObjectId,
    coordinate: BattleHexCoordinate,
    z: Option<i32>,
) -> Result<()> {
    world.attempt(|world| {
        ensure!(
            world.objects.get(&unit).is_some_and(
                |object| object.kind == Kind::Thing && !object.flags.contains(Flag::Going)
            ),
            "Unit is unavailable"
        );
        let tile = world
            .btech
            .maps()
            .get(&map)
            .context("Map not found")?
            .base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
        let towed = world.btech.tows().get(&unit).copied();
        super::scenario_map::reassign_in_candidate(world, unit, map, None)?;
        if let Some(target) = towed {
            super::scenario_map::reassign_in_candidate(world, target, map, None)?;
            super::towing::set_tow(world, unit, Some(target))?;
        }
        let position = BattlePosition {
            map,
            x: u16::try_from(coordinate.x)?,
            y: u16::try_from(coordinate.y)?,
        };
        super::scenario_position::relocate(world, unit, position, tile, z)?;
        world.btech.validate(world)?;
        Ok(())
    })
}

/// Trusted map load with C ordering: validate asset, replace it, clear membership and map objects.
pub fn load_battle_map_trusted_action(
    scripts: &Scripts,
    config: &Config,
    map: ObjectId,
    name: &str,
) -> Result<()> {
    scripts.atomic(|before| {
        ensure!(
            before
                .objects
                .get(&map)
                .is_some_and(|object| matches!(object.kind, Kind::Room | Kind::Thing)
                    && !object.flags.contains(Flag::Going)),
            "Map is unavailable"
        );
        let asset = super::assets::read_map_with_flags(
            &config.path(&config.database.map_database),
            name,
            before
                .btech
                .maps()
                .get(&map)
                .map_or(0, |record| record.flags),
        )?;
        {
            let mut world = scripts.world_mut();
            super::state::replace_map_asset(&mut world, map, name, asset)?;
            world.validate(config)?;
        }
        super::map_clear::clear_map_units_action(scripts, config, ObjectId(1), map)?;
        super::map_objects::clear(&mut scripts.world_mut(), map)?;
        Ok(())
    })
}

/// Rebuild links as a trusted call and discard only the operator acknowledgement.
pub fn update_battle_map_links_trusted_action(
    scripts: &Scripts,
    config: &Config,
    root: ObjectId,
) -> Result<()> {
    enum Work {
        Visit {
            parent: Option<ObjectId>,
            map: ObjectId,
            depth: usize,
        },
        Child {
            parent: ObjectId,
            child: ObjectId,
            coordinate: BattleHexCoordinate,
            slot: u32,
            depth: usize,
        },
    }
    let before = scripts.world().clone();
    ensure!(
        before
            .objects
            .get(&root)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Map is unavailable"
    );
    ensure!(before.btech.maps().contains_key(&root), "Map not found");
    let mut candidate = before.clone();
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
                    &mut candidate,
                    parent,
                    slot,
                    Some(BattleBuildingEntrance {
                        coordinate,
                        interior: child,
                        data_char: 0,
                        data_short: 0,
                        data_int: 0,
                    }),
                )?;
                pending.push(Work::Visit {
                    parent: Some(parent),
                    map: child,
                    depth,
                });
            }
            Work::Visit { parent, map, depth } => {
                if depth >= 1024 || !visited.insert(map) {
                    continue;
                }
                let record = candidate
                    .btech
                    .maps()
                    .get(&map)
                    .context("Map not found")?
                    .clone();
                if let Some(parent) = parent {
                    let stored = candidate.btech.maps.get_mut(&map).unwrap();
                    stored.building_parent = parent.0;
                    stored.building_exits = Default::default();
                    stored.building_entry_points = Default::default();
                    super::set_building_exit(&mut candidate, map, 0, Some(parent))?;
                    if let Some(link) = record.authored_link() {
                        let points: Vec<_> = link
                            .entrances
                            .into_iter()
                            .enumerate()
                            .filter_map(|(direction, entrance)| {
                                entrance
                                    .coordinate(&record, direction)
                                    .map(|coordinate| (direction, coordinate))
                            })
                            .collect();
                        for (slot, (direction, coordinate)) in points.into_iter().rev().enumerate()
                        {
                            super::set_building_entry_point(
                                &mut candidate,
                                map,
                                slot as u32,
                                Some(BattleBuildingEntryPoint {
                                    coordinate,
                                    direction: b"nesw"[direction],
                                    object: ObjectId(-1),
                                    data_short: 0,
                                    data_int: 0,
                                }),
                            )?;
                        }
                    }
                }
                for slot in record
                    .building_entrances()
                    .keys()
                    .copied()
                    .collect::<Vec<_>>()
                {
                    super::set_building_entrance(&mut candidate, map, slot, None)?;
                }
                let valid: Vec<_> = children
                    .get(&map)
                    .into_iter()
                    .flatten()
                    .filter(|(_, coordinate)| {
                        coordinate.x >= 0
                            && coordinate.y >= 0
                            && i64::from(coordinate.x) < record.width
                            && i64::from(coordinate.y) < record.height
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
    candidate.validate(config)?;
    *scripts.world_mut() = candidate;
    Ok(())
}

fn substitute_hex_message(
    text: &str,
    origin: BattleHexCoordinate,
    recipient: BattlePosition,
) -> String {
    let current = origin.x == i32::from(recipient.x) && origin.y == i32::from(recipient.y);
    let mut output = String::new();
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if character != '$' {
            output.push(character);
            continue;
        }
        let Some(placeholder) = characters.next() else {
            break;
        };
        match placeholder {
            'h' if current => output.push_str("your hex"),
            'h' => output.push_str(&format!("{},{}", origin.x, origin.y)),
            'H' if current => output.push_str("[fg=red bold]YOUR HEX[reset]"),
            'H' => output.push_str(&format!("[fg=yellow bold]{},{}[reset]", origin.x, origin.y)),
            other => {
                output.push('$');
                output.push(other);
            }
        }
    }
    output
}

/// Audience selection for a trusted Lua map broadcast.
#[derive(Debug, Clone, Copy)]
pub enum BattleMapEmitAudience {
    All,
    Range {
        origin: BattlePoint,
        z: Option<f64>,
        range: f64,
    },
    LineOfSight {
        origin: BattleHexCoordinate,
    },
}

/// Broadcast without requiring or acknowledging a wizard actor.
pub fn emit_battle_map_trusted_action(
    scripts: &Scripts,
    map: ObjectId,
    text: &str,
    audience: BattleMapEmitAudience,
) -> Result<()> {
    let before = scripts.world().clone();
    ensure!(
        before.objects.get(&map).is_some_and(
            |object| object.kind != Kind::Garbage && !object.flags.contains(Flag::Going)
        ),
        "Map is unavailable"
    );
    before.btech.maps().get(&map).context("Map not found")?;
    let units = battle_map_members(&before, map)?;
    for unit in units {
        let scanner =
            super::scanner::scanner_unit(&before, unit.id).context("Unit is unavailable")?;
        if scanner.power != BattlePower::Running {
            continue;
        }
        let selected = match audience {
            BattleMapEmitAudience::All => true,
            BattleMapEmitAudience::Range { origin, z, range } => {
                let actual = if let Some(z) = z {
                    battle_map_spatial_range(
                        BattleMapSpatialPoint {
                            x: (origin.x * 322.5) as f32,
                            y: (origin.y * 322.5) as f32,
                            z: (z * 64.5) as f32,
                        },
                        BattleMapSpatialPoint {
                            x: (unit.point.x * 322.5) as f32,
                            y: (unit.point.y * 322.5) as f32,
                            z: (unit.z * 64.5) as f32,
                        },
                    )?
                } else {
                    battle_map_spatial_range(
                        BattleMapSpatialPoint {
                            x: (origin.x * 322.5) as f32,
                            y: (origin.y * 322.5) as f32,
                            z: 0.0,
                        },
                        BattleMapSpatialPoint {
                            x: (unit.point.x * 322.5) as f32,
                            y: (unit.point.y * 322.5) as f32,
                            z: 0.0,
                        },
                    )?
                };
                actual <= range
            }
            BattleMapEmitAudience::LineOfSight { origin } => {
                super::hex_visible(&before, unit.id, origin)?
            }
        };
        if selected {
            crate::notification::send(
                &scripts.world(),
                &scripts.outbox,
                &crate::lua::configuration(&scripts.lua),
                crate::notification::Request {
                    target: unit.id,
                    sender: ObjectId(-1),
                    document: match audience {
                        BattleMapEmitAudience::LineOfSight { origin } => {
                            substitute_hex_message(text, origin, unit.position).into()
                        }
                        _ => text.into(),
                    },
                    policy: crate::notification::Policy::ROOM,
                    exclusions: Some(vec![unit.id]),
                },
            )?;
        }
    }
    scripts.effects.validate()?;
    Ok(())
}
