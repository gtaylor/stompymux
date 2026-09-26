//! Atomic conventional mine blasts, with ordered kick packets, neighboring fire and field removal.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Damage applied to one occupant, retaining packet consequences for character publication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleMineBlastHit {
    pub unit: ObjectId,
    pub damage: u16,
    /// Signed adjustment to the burn timer, after material packets.
    pub burn_seconds: i64,
    pub arc: BattleHitArc,
    pub impacts: Vec<BattleBlastImpact>,
    pub vehicle_heat: Option<BattleVehicleHeatExposure>,
}

/// One complete mine blast; callers publish notices and character consequences in the same transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish mine blast notices and consequences with the enclosing action"]
pub struct BattleMineBlastReport {
    pub map: ObjectId,
    pub mine: BattleMinefield,
    pub hits: Vec<BattleMineBlastHit>,
    pub ignited: Vec<BattleHexCoordinate>,
    pub removed: Vec<u32>,
    pub notices: Vec<BattleNotice>,
    /// Pilot-only messages indexed into the enclosing notice stream.
    pub pilot_notices: Vec<BattlePilotNotice>,
}

/// Resolve an admitted conventional blast; movement/radio authority and trigger feedback belong to the caller.
pub fn resolve_mine_blast(
    world: &mut World,
    map: ObjectId,
    ordinal: u32,
    rules: BattleFallRules,
) -> Result<BattleMineBlastReport> {
    resolve(world, map, ordinal, rules, false)
}

/// Character-capable blast within a host checkpoint that publishes health and evacuation effects.
pub(super) fn resolve_in_action(
    world: &mut World,
    map: ObjectId,
    ordinal: u32,
    rules: BattleFallRules,
) -> Result<BattleMineBlastReport> {
    resolve(world, map, ordinal, rules, true)
}

/// Keep every damage packet, map random draw and deletion in one candidate world.
fn resolve(
    world: &mut World,
    map: ObjectId,
    ordinal: u32,
    rules: BattleFallRules,
    character: bool,
) -> Result<BattleMineBlastReport> {
    ensure!(
        world
            .objects
            .get(&map)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Map is unavailable"
    );
    let record = world.btech.maps().get(&map).context("Map not found")?;
    record.validate()?;
    let mine = *record
        .minefields()
        .get(&ordinal)
        .context("Minefield not found")?;
    ensure!(
        matches!(
            mine.kind,
            BattleMineKind::Standard
                | BattleMineKind::Inferno
                | BattleMineKind::Command
                | BattleMineKind::Vibra
        ),
        "Trigger mines require their script handler"
    );
    let neighbors = matches!(mine.kind, BattleMineKind::Command | BattleMineKind::Vibra);
    let damage = if mine.kind == BattleMineKind::Inferno {
        mine.strength.max(0) as u16 / 3
    } else {
        mine.strength.max(0) as u16
    };
    let mut cells = vec![(mine.coordinate, damage)];
    if neighbors && mine.strength / 2 != 0 {
        // Neighbor hexes follow coordinate order after the center, including at map edges.
        for x in mine.coordinate.x - 1..=mine.coordinate.x + 1 {
            for y in mine.coordinate.y - 1..=mine.coordinate.y + 1 {
                let point = BattleHexCoordinate { x, y };
                if point != mine.coordinate
                    && point.distance(mine.coordinate) == 1
                    && record.base_hex(i64::from(x), i64::from(y)).is_ok()
                {
                    cells.push((point, mine.strength.max(0) as u16 / 2));
                }
            }
        }
    }
    world.attempt(|world| {
        if neighbors {
            world.btech.maps.get_mut(&map).unwrap().set_lookup_bit(
                mine.coordinate,
                super::map_bits::LookupKind::Mine,
                false,
            )?;
        }
        let mut report = BattleMineBlastReport {
            map,
            mine,
            hits: Vec::new(),
            ignited: Vec::new(),
            removed: Vec::new(),
            notices: Vec::new(),
            pilot_notices: Vec::new(),
        };
        for (coordinate, damage) in cells {
            hit_hex(world, &mut report, coordinate, damage, rules, character)?;
            if coordinate == mine.coordinate {
                continue;
            }
            if super::blast_damage::ignite_forest(world, map, coordinate)? {
                report.ignited.push(coordinate);
            }
        }
        if neighbors || mine.strength < 5 {
            report.removed = world.btech.maps()[&map]
                .ordered_minefields()
                .filter_map(|(&id, field)| (field.coordinate == mine.coordinate).then_some(id))
                .collect();
            for &ordinal in &report.removed {
                super::set_minefield(world, map, ordinal, None)?;
            }
        }
        world
            .btech
            .maps
            .get_mut(&map)
            .unwrap()
            .rebuild_mine_lookup()?;
        world.btech.validate_action(world)?;
        Ok(report)
    })
}

/// Resolve a blast cell in map-slot order, freezing each occupant's arc before its packets.
fn hit_hex(
    world: &mut World,
    report: &mut BattleMineBlastReport,
    coordinate: BattleHexCoordinate,
    damage: u16,
    rules: BattleFallRules,
    character: bool,
) -> Result<()> {
    let tile = world.btech.maps()[&report.map]
        .base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    let ground = if matches!(tile.terrain, Terrain::Water | Terrain::Ice) {
        -i32::from(tile.elevation)
    } else {
        i32::from(tile.elevation)
    };
    let neighbor = coordinate != report.mine.coordinate;
    let inferno = report.mine.kind == BattleMineKind::Inferno;
    // Rear armor remains selected after a rear-facing occupant within this blast cell.
    let blast = super::blast_damage::BlastCell {
        coordinate,
        origin: report.mine.coordinate.center(),
        tile,
        lower: ground - 1,
        upper: ground + 2,
    };
    let mut rear = false;
    for id in super::map_slots::all_unit_order(world, report.map)? {
        let Some(target) = blast.target(world, id, rules)? else {
            continue;
        };
        let is_character = target.character;
        ensure!(
            character || !is_character,
            "Mine blast requires character consequence publication"
        );
        let arc = target.arc;
        rear |= arc == BattleHitArc::Rear;
        let rules = target.rules;
        report.notices.extend(super::broadcast::observer_notices(
            world,
            id,
            if inferno {
                "is hit by globs of flaming gel!"
            } else if neighbor {
                "is hit by some of the shrapnel!"
            } else {
                "is hit by shrapnel!"
            },
        ));
        report.notices.push(BattleNotice {
            unit: id,
            text: if inferno {
                "Globs of flaming gel hit you!"
            } else if neighbor {
                "A little blast of shrapnel hits you!"
            } else {
                "A blast of shrapnel hits you!"
            }
            .into(),
        });
        let mut hit = BattleMineBlastHit {
            unit: id,
            damage,
            burn_seconds: 0,
            arc,
            impacts: Vec::new(),
            vehicle_heat: None,
        };
        let effects = super::blast_damage::resolve(
            world,
            id,
            super::blast_damage::BlastDamage {
                damage,
                packet_size: 5,
                table: BattleHitTable::Kick,
                arc,
                heat: if inferno {
                    i32::from(report.mine.strength)
                } else {
                    0
                },
                character: is_character,
            },
            &mut rear,
            rules,
        )?;
        hit.impacts = effects.impacts;
        hit.vehicle_heat = effects.vehicle_heat;
        hit.burn_seconds = effects.burn_seconds;
        super::piloting::append_feedback(
            &mut report.pilot_notices,
            effects.pilot_notices.iter().cloned(),
            report.notices.len(),
        );
        report.notices.extend(effects.notices);
        report.hits.push(hit);
    }
    Ok(())
}
