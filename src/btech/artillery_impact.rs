//! Transactional artillery arrivals composed with tactical damage, map smoke and minefields.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// One blast occupant, retaining each packet's character and material consequences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleArtilleryHit {
    pub unit: ObjectId,
    pub coordinate: BattleHexCoordinate,
    pub arc: BattleHitArc,
    pub impacts: Vec<BattleBlastImpact>,
    pub vehicle_heat: Option<BattleVehicleHeatExposure>,
}

/// Applied arrival effects; the enclosing host action publishes notices and character consequences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish artillery notices and character consequences in the same transaction"]
pub struct BattleArtilleryImpactReport {
    pub map: ObjectId,
    pub pattern: BattleArtilleryImpactPattern,
    pub hits: Vec<BattleArtilleryHit>,
    pub mines: Vec<u32>,
    pub notices: Vec<BattleNotice>,
    /// Pilot-only packet feedback indexed into the arrival notice stream.
    pub pilot_notices: Vec<BattlePilotNotice>,
}

/// Advance an admitted flight and atomically apply its arrival to an out-of-character battlefield.
/// The caller owns launch authorization and durable storage of the supplied cursor.
pub fn advance_artillery_flight(
    world: &mut World,
    map: ObjectId,
    flight: &mut BattleArtilleryFlight,
    rules: BattleFallRules,
) -> Result<Option<BattleArtilleryImpactReport>> {
    advance(world, map, flight, rules, false)
}

/// Character-capable arrival for a host action that also publishes casualty effects.
pub(super) fn advance(
    world: &mut World,
    map: ObjectId,
    flight: &mut BattleArtilleryFlight,
    rules: BattleFallRules,
    character: bool,
) -> Result<Option<BattleArtilleryImpactReport>> {
    ensure!(
        world
            .objects
            .get(&map)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Map is unavailable"
    );
    let record = world.btech.maps().get(&map).context("Map not found")?;
    record.validate()?;
    ensure!(record.terrain_ready(), "Map terrain is unavailable");
    let mut candidate = world.clone();
    let mut cursor = flight.clone();
    let record = candidate.btech.maps.get_mut(&map).unwrap();
    let dimensions = (u16::try_from(record.width)?, u16::try_from(record.height)?);
    let wind = u16::try_from(record.wind_speed)?;
    let dice = record
        .fire_dice
        .as_mut()
        .context("Map random stream is unavailable")?;
    let Some(pattern) = cursor.advance(dimensions, wind, dice)? else {
        *flight = cursor;
        return Ok(None);
    };
    let notices = super::artillery_feedback::arrival_notices(
        &candidate,
        map,
        cursor.weapon(),
        cursor.mode(),
        pattern.impact,
    )?;
    let mut report = BattleArtilleryImpactReport {
        map,
        pattern,
        hits: Vec::new(),
        mines: Vec::new(),
        notices,
        pilot_notices: Vec::new(),
    };
    for cell in report.pattern.cells.clone() {
        match cell.effect {
            BattleArtilleryEffect::Smoke { seconds } => {
                set_map_decoration(
                    &mut candidate,
                    map,
                    cell.position,
                    Some(BattleDecoration::new(
                        BattleDecorationKind::Smoke,
                        i64::from(seconds),
                        None,
                    )),
                )?;
            }
            BattleArtilleryEffect::Mine { strength } => {
                if let Some(ordinal) = deposit_mine(&mut candidate, map, cell.position, strength)? {
                    report.mines.push(ordinal);
                }
            }
            BattleArtilleryEffect::Damage {
                total,
                packet_size,
                table,
            } => {
                hit_cell(
                    &mut candidate,
                    &mut report,
                    &cell,
                    (total, packet_size, table),
                    rules,
                    character,
                )?;
            }
        }
    }
    candidate.btech.validate_action(&candidate)?;
    *world = candidate;
    *flight = cursor;
    Ok(Some(report))
}

/// Artillery does not stack a second field on a coordinate that already holds one.
fn deposit_mine(
    world: &mut World,
    map: ObjectId,
    coordinate: BattleHexCoordinate,
    strength: u16,
) -> Result<Option<u32>> {
    world.btech.maps()[&map].base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    if world.btech.maps()[&map]
        .minefields()
        .values()
        .any(|field| field.coordinate == coordinate)
    {
        return Ok(None);
    }
    let ordinal = super::insert_minefield(
        world,
        map,
        BattleMinefield {
            coordinate,
            kind: BattleMineKind::Standard,
            strength: i16::try_from(strength)?,
            extra: 0,
            owner: ObjectId(0),
        },
    )?;
    Ok(Some(ordinal))
}

/// Freeze each occupant's blast arc before ordered packets; height limits are exclusive.
fn hit_cell(
    world: &mut World,
    report: &mut BattleArtilleryImpactReport,
    cell: &BattleArtilleryCell,
    (damage, packet_size, table): (u16, u8, BattleHitTable),
    rules: BattleFallRules,
    character: bool,
) -> Result<()> {
    let coordinate = cell.position;
    let tile = world.btech.maps()[&report.map]
        .base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    let ground = if matches!(tile.terrain, Terrain::Water | Terrain::Ice) {
        0
    } else {
        i32::from(tile.elevation)
    };
    let blast = super::blast_damage::BlastCell {
        coordinate,
        origin: coordinate.center(),
        tile,
        lower: ground - 4,
        upper: ground + 10,
    };
    let mut rear = false;
    for id in super::map_slots::all_unit_order(world, report.map)? {
        let Some(target) = blast.target(world, id, rules)? else {
            continue;
        };
        let is_character = target.character;
        ensure!(
            character || !is_character,
            "Artillery impact requires character consequence publication"
        );
        let arc = target.arc;
        rear |= arc == BattleHitArc::Rear;
        let rules = target.rules;
        let (observer, cockpit) = if table == BattleHitTable::Punch {
            if damage > 2 {
                ("is hit by bomblets!", "You are hit by bomblets!")
            } else {
                ("is hit by a bomblet!", "You are hit by a bomblet!")
            }
        } else if cell.direct {
            ("receives a direct hit!", "You receive a direct hit!")
        } else {
            ("is hit by fragments!", "You are hit by fragments!")
        };
        report
            .notices
            .extend(super::broadcast::observer_notices(world, id, observer));
        report.notices.push(BattleNotice {
            unit: id,
            text: cockpit.into(),
        });
        let mut hit = BattleArtilleryHit {
            unit: id,
            coordinate,
            arc,
            impacts: Vec::new(),
            vehicle_heat: None,
        };
        let effects = super::blast_damage::resolve(
            world,
            id,
            super::blast_damage::BlastDamage {
                damage,
                packet_size,
                table,
                arc,
                heat: 0,
                character: is_character,
                class: BattleDamageClass::AreaEffect,
            },
            &mut rear,
            rules,
        )?;
        hit.impacts = effects.impacts;
        hit.vehicle_heat = effects.vehicle_heat;
        super::piloting::append_feedback(
            &mut report.pilot_notices,
            effects.pilot_notices,
            report.notices.len(),
        );
        report.notices.extend(effects.notices);
        report.hits.push(hit);
    }
    Ok(())
}
