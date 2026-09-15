//! Building weapon packets resolve shared interior integrity and captured container notices.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result};
use serde::Serialize;
use std::sync::Arc;

/// One accepted building packet, including immune structures and clamped actual damage.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleBuildingImpact {
    pub interior: ObjectId,
    pub damage: u16,
    pub remaining: i64,
    pub destroyed: bool,
    pub immune: bool,
    pub notices: Vec<BattleNotice>,
}

/// Apply a successful building packet inside the firing transaction; absent or ruined buildings are quiet.
pub(super) fn resolve(
    world: &mut World,
    shooter: ObjectId,
    coordinate: BattleHexCoordinate,
    damage: u16,
) -> Result<Option<BattleBuildingImpact>> {
    let map = super::scanner::scanner_unit(world, shooter)
        .and_then(|unit| unit.position)
        .context("Shooter is not placed")?
        .map;
    let source = &world.btech.maps()[&map];
    let Some(entrance) = source.building_at(coordinate)? else {
        return Ok(None);
    };
    let interior = entrance.interior;
    let object = world
        .objects
        .get(&interior)
        .context("Building interior is unavailable")?;
    if object.flags.contains(crate::Flag::Going) {
        return Ok(None);
    }
    let Some(interior_map) = world.btech.maps().get(&interior) else {
        return Ok(None);
    };
    let building = interior_map.building;
    if damage == 0 {
        return Ok(None);
    }
    if source.building.is_complex() || building.is_command_center() {
        return Ok(Some(BattleBuildingImpact {
            interior,
            damage: 0,
            remaining: building.integrity,
            destroyed: false,
            immune: true,
            notices: vec![BattleNotice {
                unit: shooter,
                text: "Your shot only scratches the paint!".into(),
            }],
        }));
    }
    if building.integrity == 0 {
        return Ok(None);
    }
    let damage = i64::from(damage).min(building.integrity) as u16;
    let remaining = building.integrity - i64::from(damage);
    let destroyed = remaining == 0;
    let name = super::building_entrance::structure_name(world, interior)?;
    let suffix = if destroyed { ", destroying it!" } else { "." };
    let mut notices = vec![
        BattleNotice {
            unit: shooter,
            text: format!("You hit {name} for {damage} points of damage{suffix}"),
        },
        BattleNotice {
            unit: interior,
            text: format!(
                "The {} is hit for {damage} {}points of damage{suffix}",
                object.name,
                if destroyed { "more " } else { "" }
            ),
        },
    ];
    if destroyed {
        notices.extend(super::broadcast::observer_notices(
            world,
            shooter,
            &format!("hits {name}, destroying it!"),
        ));
    }
    let record = Arc::make_mut(&mut world.btech.maps)
        .get_mut(&interior)
        .unwrap();
    record.building.integrity = remaining;
    if remaining > 0 && building.integrity == building.maximum_integrity {
        record.building_repair = record.building.repair_delay();
    }
    Ok(Some(BattleBuildingImpact {
        interior,
        damage,
        remaining,
        destroyed,
        immune: false,
        notices,
    }))
}

/// Whether a committed building repair countdown needs the server heartbeat.
pub fn building_repair_pending(world: &World) -> bool {
    world
        .btech
        .maps()
        .values()
        .any(|map| map.building_repair.is_some())
}

/// Advance saved countdowns once; destroyed buildings stop at their next scheduled event.
/// Signed repair factors use bounded integrity arithmetic, and no elapsed offline time is applied.
pub fn advance_building_repairs(world: &mut World) {
    if !building_repair_pending(world) {
        return;
    }
    for map in Arc::make_mut(&mut world.btech.maps).values_mut() {
        let Some(remaining) = map.building_repair else {
            continue;
        };
        if remaining > 1 {
            map.building_repair = Some(remaining - 1);
            continue;
        }
        map.building_repair = None;
        if map.building.integrity == 0 {
            continue;
        }
        map.building.integrity = (map.building.integrity + map.building.regeneration)
            .clamp(0, map.building.maximum_integrity);
        map.building_repair = map.building.repair_delay();
    }
}
