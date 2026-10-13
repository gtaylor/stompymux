//! Atomic mine blasts (Tactical Operations pp. 176–177): ordered kick packets against the
//! front of every unit in the hex, half damage to the neighbours of a command-detonated field,
//! and the field's depletion afterwards.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Damage applied to one occupant, retaining packet consequences for character publication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MineBlastHit {
    pub unit: ObjectId,
    pub damage: u16,
    /// Signed adjustment to a vehicle's burn timer, after material packets.
    pub burn_seconds: i64,
    /// Heat an inferno mine's 1d6 missiles put on a 'Mech (Tactical Operations p. 42).
    pub heat: u8,
    pub arc: HitArc,
    pub impacts: Vec<BlastImpact>,
    pub vehicle_heat: Option<VehicleHeatExposure>,
}

/// One complete mine blast; callers publish notices and character consequences in the same transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish mine blast notices and consequences with the enclosing action"]
pub struct MineBlastReport {
    pub map: ObjectId,
    pub mine: Minefield,
    pub hits: Vec<MineBlastHit>,
    pub ignited: Vec<HexCoordinate>,
    pub removed: Vec<u32>,
    /// The conventional field's depletion roll, when it had density to lose.
    pub depletion_roll: Option<u8>,
    /// The field's density after the blast, when it stayed on the map.
    pub remaining_strength: Option<i16>,
    pub notices: Vec<Notice>,
    /// Pilot-only messages indexed into the enclosing notice stream.
    pub pilot_notices: Vec<PilotNotice>,
}

/// Resolve an admitted conventional blast; movement/radio authority and trigger feedback belong to the caller.
pub fn resolve_mine_blast(
    world: &mut World,
    map: ObjectId,
    ordinal: u32,
    rules: FallRules,
) -> Result<MineBlastReport> {
    resolve(world, map, ordinal, rules, false)
}

/// Character-capable blast within a host checkpoint that publishes health and evacuation effects.
pub(super) fn resolve_in_action(
    world: &mut World,
    map: ObjectId,
    ordinal: u32,
    rules: FallRules,
) -> Result<MineBlastReport> {
    resolve(world, map, ordinal, rules, true)
}

/// Keep every damage packet, map random draw and deletion in one candidate world.
fn resolve(
    world: &mut World,
    map: ObjectId,
    ordinal: u32,
    rules: FallRules,
    character: bool,
) -> Result<MineBlastReport> {
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
            MineKind::Standard
                | MineKind::Inferno
                | MineKind::Command
                | MineKind::Vibra
                | MineKind::Active
        ),
        "Trigger mines require their script handler"
    );
    // Only a command-detonated field reaches the adjacent hexes; a vibrabomb does not (p. 177).
    let neighbors = mine.kind == MineKind::Command;
    let damage = if mine.kind == MineKind::Inferno {
        mine.strength.max(0) as u16 / 3
    } else {
        mine.strength.max(0) as u16
    };
    let mut cells = vec![(mine.coordinate, damage)];
    if neighbors && mine.strength / 2 != 0 {
        // Neighbor hexes follow coordinate order after the center, including at map edges.
        for x in mine.coordinate.x - 1..=mine.coordinate.x + 1 {
            for y in mine.coordinate.y - 1..=mine.coordinate.y + 1 {
                let point = HexCoordinate { x, y };
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
        let mut report = MineBlastReport {
            map,
            mine,
            hits: Vec::new(),
            ignited: Vec::new(),
            removed: Vec::new(),
            depletion_roll: None,
            remaining_strength: None,
            notices: Vec::new(),
            pilot_notices: Vec::new(),
        };
        for (coordinate, damage) in cells {
            hit_hex(world, &mut report, coordinate, damage, rules, character)?;
        }
        // Tactical Operations p. 177: an inferno mine sets its hex on fire; other mines
        // start no fires.
        if mine.kind == MineKind::Inferno
            && super::blast_damage::ignite_forest(world, map, mine.coordinate)?
        {
            report.ignited.push(mine.coordinate);
        }
        deplete(world, &mut report, ordinal)?;
        world.btech.validate_action(world)?;
        Ok(report)
    })
}

/// Lowest density a conventional field keeps while it stays on the map.
const MINIMUM_DENSITY: i16 = 5;

/// One density step lost to an explosion.
const DENSITY_STEP: i16 = 5;

/// Thin the field after it goes off (Tactical Operations p. 176). A conventional field rolls
/// against its density target and drops a step on success, never below five points; a
/// command-detonated, vibrabomb or active field drops a step automatically and is spent once a
/// five-point field explodes. A field with no density left is cleared with everything
/// colocated with it.
fn deplete(world: &mut World, report: &mut MineBlastReport, ordinal: u32) -> Result<()> {
    let map = report.map;
    let mine = report.mine;
    let conventional = matches!(mine.kind, MineKind::Standard | MineKind::Inferno);
    let remaining = if mine.strength < MINIMUM_DENSITY {
        None
    } else if conventional {
        let roll = world
            .btech
            .maps
            .get_mut(&map)
            .unwrap()
            .fire_dice
            .as_mut()
            .context("Map random stream is unavailable")?
            .two_d6();
        report.depletion_roll = Some(roll);
        let reduced = roll >= super::mine_activation::density_target(mine.strength);
        Some(if reduced {
            (mine.strength - DENSITY_STEP).max(MINIMUM_DENSITY)
        } else {
            mine.strength
        })
    } else if mine.strength - DENSITY_STEP < MINIMUM_DENSITY {
        None
    } else {
        Some(mine.strength - DENSITY_STEP)
    };
    let Some(strength) = remaining else {
        report.removed = world.btech.maps()[&map]
            .ordered_minefields()
            .filter_map(|(&id, field)| (field.coordinate == mine.coordinate).then_some(id))
            .collect();
        for &ordinal in &report.removed {
            super::set_minefield(world, map, ordinal, None)?;
        }
        return Ok(());
    };
    report.remaining_strength = Some(strength);
    if strength != mine.strength {
        super::set_minefield(world, map, ordinal, Some(Minefield { strength, ..mine }))?;
    }
    Ok(())
}

/// Resolve a blast cell in map-slot order, freezing each occupant's arc before its packets.
fn hit_hex(
    world: &mut World,
    report: &mut MineBlastReport,
    coordinate: HexCoordinate,
    damage: u16,
    rules: FallRules,
    character: bool,
) -> Result<()> {
    let tile = world.btech.maps()[&report.map]
        .base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    let ground = i32::from(tile.surface_height());
    let neighbor = coordinate != report.mine.coordinate;
    let inferno = report.mine.kind == MineKind::Inferno;
    let blast = super::blast_damage::BlastCell {
        coordinate,
        origin: report.mine.coordinate.center(),
        tile,
        lower: ground - 1,
        upper: ground + 2,
    };
    // Mines strike from below: every unit takes the blast on its front (Tactical Operations
    // p. 176).
    let arc = HitArc::Front;
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
        report.notices.push(Notice {
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
        let mut hit = MineBlastHit {
            unit: id,
            damage,
            burn_seconds: 0,
            heat: 0,
            arc,
            impacts: Vec::new(),
            vehicle_heat: None,
        };
        // A 'Mech in an inferno field is hit by 1d6 infernos (Tactical Operations p. 42); a
        // vehicle's fire check reads the field's strength.
        let heat = if !inferno {
            0
        } else if world.btech.constructed_units().contains_key(&id) {
            if report.mine.strength > 0 {
                i32::from(super::dice::unit_dice_mut(world, id)?.d6())
            } else {
                0
            }
        } else {
            i32::from(report.mine.strength)
        };
        let effects = super::blast_damage::resolve(
            world,
            id,
            super::blast_damage::BlastDamage {
                damage,
                packet_size: 5,
                class: DamageClass::Ordinary,
                table: HitTable::Kick,
                arc,
                heat,
                character: is_character,
            },
            &mut rear,
            rules,
        )?;
        hit.impacts = effects.impacts;
        hit.vehicle_heat = effects.vehicle_heat;
        hit.burn_seconds = effects.burn_seconds;
        hit.heat = effects.mech_heat;
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
