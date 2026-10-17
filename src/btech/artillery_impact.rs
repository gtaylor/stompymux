//! Transactional artillery arrivals composed with tactical damage, map smoke and minefields.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// One blast occupant, retaining each packet's character and material consequences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArtilleryHit {
    pub unit: ObjectId,
    pub coordinate: HexCoordinate,
    /// The blast's damage to this occupant, before its 5-point groups strike.
    pub damage: u16,
    pub arc: HitArc,
    pub impacts: Vec<BlastImpact>,
    pub vehicle_heat: Option<VehicleHeatExposure>,
}

/// Construction factor a struck structure lost to the blast, after its occupants took theirs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArtilleryStructureHit {
    pub coordinate: HexCoordinate,
    pub damage: u16,
    /// Construction factor left; zero once the structure collapses.
    pub cf: u16,
    pub collapse: Option<SurfaceBreak>,
}

/// Applied arrival effects; the enclosing host action publishes notices and character consequences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish artillery notices and character consequences in the same transaction"]
pub struct ArtilleryImpactReport {
    pub map: ObjectId,
    pub pattern: ArtilleryImpactPattern,
    pub hits: Vec<ArtilleryHit>,
    /// Buildings, walls and bridges the blast struck, in cell order.
    pub structures: Vec<ArtilleryStructureHit>,
    pub mines: Vec<u32>,
    pub notices: Vec<Notice>,
    /// Pilot-only packet feedback indexed into the arrival notice stream.
    pub pilot_notices: Vec<PilotNotice>,
}

/// Advance an admitted flight and atomically apply its arrival to an out-of-character battlefield.
/// The caller owns launch authorization and durable storage of the supplied cursor. With no
/// firer known, a homing missile has no friendly TAG to follow and falls unguided.
pub fn advance_artillery_flight(
    world: &mut World,
    map: ObjectId,
    flight: &mut ArtilleryFlight,
    rules: FallRules,
) -> Result<Option<ArtilleryImpactReport>> {
    advance(world, map, (flight, None), rules, false)
}

/// Character-capable arrival for a host action that also publishes casualty effects. The
/// firer, if still present, steers a homing missile onto a unit its side's TAG designates.
pub(super) fn advance(
    world: &mut World,
    map: ObjectId,
    (flight, shooter): (&mut ArtilleryFlight, Option<ObjectId>),
    rules: FallRules,
    character: bool,
) -> Result<Option<ArtilleryImpactReport>> {
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
    // The book has the firer declare whether a homing missile homes on a designated unit or
    // strikes its hex unguided (Tactical Operations: Advanced Units & Equipment p. 166). In
    // realtime nobody can know at launch whether a designation will hold when it lands, so
    // the missile decides on arrival: it homes when a friendly TAG designates a unit within
    // reach, and otherwise falls on its hex as the unguided attack.
    if cursor.mode() == ArtilleryMode::Homing
        && cursor.remaining() == 1
        && let Some(target) =
            shooter.and_then(|shooter| homing_target(&candidate, map, shooter, cursor.target()))
    {
        let report = home(&mut candidate, map, &mut cursor, target, rules, character)?;
        candidate.btech.validate_action(&candidate)?;
        *world = candidate;
        *flight = cursor;
        return Ok(Some(report));
    }
    let record = candidate.btech.maps.get_mut(&map).unwrap();
    let dimensions = (u16::try_from(record.width)?, u16::try_from(record.height)?);
    let dice = record
        .fire_dice
        .as_mut()
        .context("Map random stream is unavailable")?;
    let Some(pattern) = cursor.advance(dimensions, dice)? else {
        *flight = cursor;
        return Ok(None);
    };
    // A round that scattered off the battlefield lands unseen.
    let notices = if pattern.cells.is_empty() {
        Vec::new()
    } else {
        super::artillery_feedback::arrival_notices(
            &candidate,
            map,
            cursor.weapon(),
            cursor.mode(),
            pattern.impact,
        )?
    };
    let origin = cursor.origin();
    let mut report = ArtilleryImpactReport {
        map,
        pattern,
        hits: Vec::new(),
        structures: Vec::new(),
        mines: Vec::new(),
        notices,
        pilot_notices: Vec::new(),
    };
    let smoke = super::decorations::smoke_seconds(&candidate);
    // Units inside a building are safe from cluster bomblets (Tactical Operations: Advanced
    // Units & Equipment p. 166). A homing missile falling unguided harms only the units in
    // its hex.
    let sheltered = cursor.mode() == ArtilleryMode::Cluster;
    let structures = cursor.mode() != ArtilleryMode::Homing;
    for cell in report.pattern.cells.clone() {
        match cell.effect {
            ArtilleryEffect::Smoke => {
                super::decorations::raise_smoke(&mut candidate, map, cell.position, smoke, true)?;
            }
            ArtilleryEffect::Mine { strength } => {
                if let Some(ordinal) = deposit_mine(&mut candidate, map, cell.position, strength)? {
                    report.mines.push(ordinal);
                }
            }
            ArtilleryEffect::Damage {
                total,
                packet_size,
                table,
            } => {
                hit_cell(
                    &mut candidate,
                    &mut report,
                    &cell,
                    origin,
                    CellBlast {
                        damage: total,
                        packet_size,
                        table,
                        sheltered,
                        spared: None,
                        structures,
                    },
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

/// Densest field a weapon-delivered minefield may make of a pre-designated one (Tactical
/// Operations: Advanced Units & Equipment pp. 169, 185).
pub(super) const PREDESIGNATED_MAXIMUM_DENSITY: i16 = 30;

/// Lay a FASCAM field. It never combines with another weapon-delivered field; landing on a
/// pre-designated conventional field it thickens that field to at most thirty points
/// (Tactical Operations: Advanced Units & Equipment p. 169).
fn deposit_mine(
    world: &mut World,
    map: ObjectId,
    coordinate: HexCoordinate,
    strength: u16,
) -> Result<Option<u32>> {
    world.btech.maps()[&map].base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    let fields: Vec<_> = world.btech.maps()[&map]
        .ordered_minefields()
        .filter(|(_, field)| field.coordinate == coordinate)
        .map(|(ordinal, field)| (*ordinal, *field))
        .collect();
    if fields
        .iter()
        .any(|(_, field)| super::minefield::weapon_delivered(world, field))
    {
        return Ok(None);
    }
    let strength = i16::try_from(strength)?;
    if let Some((ordinal, field)) = fields
        .iter()
        .find(|(_, field)| field.kind == MineKind::Standard)
    {
        let field = Minefield {
            strength: field
                .strength
                .saturating_add(strength)
                .min(PREDESIGNATED_MAXIMUM_DENSITY.max(field.strength)),
            ..*field
        };
        super::set_minefield(world, map, *ordinal, Some(field))?;
        return Ok(Some(*ordinal));
    }
    let ordinal = super::insert_minefield(
        world,
        map,
        Minefield {
            coordinate,
            kind: MineKind::Standard,
            strength,
            extra: 0,
            owner: ObjectId(0),
        },
    )?;
    Ok(Some(ordinal))
}

/// Damage lost for every level above or below the shell's burst in its own hex (Tactical
/// Operations: Advanced Rules p. 150).
const VERTICAL_FALLOFF: u16 = 10;

/// How a blast reaches a unit at `level` in a building hex: the whole Damage Value inside a
/// light, medium or heavy building or on its roof (Tactical Operations: Advanced Rules p. 152),
/// and inside a hardened one the area-effect rules (Total Warfare p. 173), with the round
/// bursting on the roof: in the struck hex the roof and the level below take it whole and the
/// next level half; in the hexes around it the roof takes it whole and the level below half.
/// `None` for a unit above the roof, whom the ordinary vertical rule reaches.
fn building_damage(tile: Hex, level: i32, damage: u16, direct: bool) -> Option<u16> {
    let roof = i32::from(tile.surface_height());
    if !tile.has_building() || !(level == roof || super::building_interior::inside(tile, level)) {
        return None;
    }
    if tile.construction_class() != Some(ConstructionClass::Hardened) {
        return Some(damage);
    }
    let whole = if direct { 1 } else { 0 };
    Some(match roof - level {
        depth if depth <= whole => damage,
        depth if depth == whole + 1 => damage.div_ceil(2),
        _ => 0,
    })
}

/// Freeze each occupant's blast arc before ordered packets. The shell bursts on the ground of
/// its hex, or the roof of a structure, and strikes from the direction of the gun (Tactical
/// Operations: Advanced Rules p. 150); in the impact hex alone the blast also climbs and
/// sinks, losing ten points a level, while the outer rings reach only units standing on the
/// ground. Units inside a building take [`building_damage`] instead, before the structure
/// itself takes its share.
fn hit_cell(
    world: &mut World,
    report: &mut ArtilleryImpactReport,
    cell: &ArtilleryCell,
    origin: HexCoordinate,
    blast: CellBlast,
    rules: FallRules,
    character: bool,
) -> Result<()> {
    let CellBlast {
        damage,
        packet_size,
        table,
        sheltered,
        spared,
        structures,
    } = blast;
    let coordinate = cell.position;
    let tile = world.btech.maps()[&report.map]
        .base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    let ground = i32::from(tile.top_height());
    // The outer rings reach units on the ground or just above it, as a mine blast does.
    let (mut lower, upper) = if cell.direct {
        let reach = i32::from(damage.div_ceil(VERTICAL_FALLOFF));
        (ground - reach, ground + reach)
    } else {
        (ground - 2, ground + 2)
    };
    // Every level inside a building is within the blast's reach.
    if tile.has_building() {
        lower = lower.min(i32::from(tile.level()) - 1);
    }
    let blast = super::blast_damage::BlastCell {
        coordinate,
        origin: origin.center(),
        tile,
        lower,
        upper,
    };
    for id in super::map_slots::all_unit_order(world, report.map)? {
        if spared == Some(id) {
            continue;
        }
        let Some(target) = blast.target(world, id, rules)? else {
            continue;
        };
        let is_character = target.character;
        ensure!(
            character || !is_character,
            "Artillery impact requires character consequence publication"
        );
        let inside = super::building_interior::inside(tile, target.elevation);
        let damage = if sheltered && inside {
            0
        } else if let Some(damage) = building_damage(tile, target.elevation, damage, cell.direct) {
            damage
        } else if cell.direct {
            let levels =
                u16::try_from((target.elevation - ground).unsigned_abs()).unwrap_or(u16::MAX);
            damage.saturating_sub(levels.saturating_mul(VERTICAL_FALLOFF))
        } else {
            damage
        };
        if damage == 0 {
            continue;
        }
        let arc = target.arc;
        let rules = target.rules;
        let (observer, cockpit) = if table == HitTable::Punch {
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
        report.notices.push(Notice {
            unit: id,
            text: cockpit.into(),
        });
        let mut hit = ArtilleryHit {
            unit: id,
            coordinate,
            damage,
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
                class: DamageClass::AreaEffect,
            },
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
    if !structures {
        return Ok(());
    }
    strike_structure(world, report, cell, damage, rules, character)
}

/// How one cell's blast treats what stands in it.
#[derive(Debug, Clone, Copy)]
struct CellBlast {
    damage: u16,
    packet_size: u8,
    table: HitTable,
    /// Units inside a building take nothing, as from cluster bomblets.
    sheltered: bool,
    /// A unit the blast leaves alone: the one a homing missile struck, which takes its own hit.
    spared: Option<ObjectId>,
    /// The structure in the hex takes its share.
    structures: bool,
}

/// How far from its aim point an Arrow IV homing missile can find a designated unit (Tactical
/// Operations: Advanced Units & Equipment p. 166).
const HOMING_REACH: u64 = 8;

/// The unit an arriving homing missile attacks: one a TAG friendly to `shooter` designates,
/// within eight hexes of the aim point (Tactical Operations: Advanced Units & Equipment
/// p. 166). The firer would choose among several; in realtime it cannot, so the missile takes
/// the firer's own target lock when that is designated, otherwise the designated unit nearest
/// its aim point.
fn homing_target(
    world: &World,
    map: ObjectId,
    shooter: ObjectId,
    aim: HexCoordinate,
) -> Option<ObjectId> {
    let team = super::scanner::scanner_unit(world, shooter)?.signature.team;
    let mut candidates: Vec<(u64, ObjectId)> = super::map_slots::all_unit_order(world, map)
        .ok()?
        .into_iter()
        .filter_map(|id| {
            let unit = super::scanner::scanner_unit(world, id)?;
            let position = unit.position.filter(|position| position.map == map)?;
            if unit.destroyed {
                return None;
            }
            let tagger = super::tagged_by(world, id)?;
            let friendly = super::scanner::scanner_unit(world, tagger)
                .is_some_and(|tagger| tagger.signature.team == team);
            let distance = aim.distance(HexCoordinate {
                x: i32::from(position.x),
                y: i32::from(position.y),
            });
            (friendly && distance <= HOMING_REACH).then_some((distance, id))
        })
        .collect();
    let locked = match super::targeting::selection(world, shooter) {
        Some(TargetSelection::Unit(lock)) => Some(lock.target),
        _ => None,
    };
    if let Some(locked) = locked.filter(|locked| candidates.iter().any(|(_, id)| id == locked)) {
        return Some(locked);
    }
    candidates.sort_by_key(|(distance, _)| *distance);
    candidates.first().map(|(_, id)| *id)
}

/// Resolve a homing missile arriving on a designated unit (Tactical Operations: Advanced Units
/// & Equipment p. 166): on 4+ on 2D6 it strikes that unit for twenty points in 5-point groups,
/// a direct-fire ballistic hit from the firer's direction, and five points of area-effect
/// damage strike every other unit in its hex; on 3 or less it strikes the hex instead, five
/// points to every unit there, the target included.
fn home(
    world: &mut World,
    map: ObjectId,
    flight: &mut ArtilleryFlight,
    target: ObjectId,
    rules: FallRules,
    character: bool,
) -> Result<ArtilleryImpactReport> {
    let position = super::scanner::scanner_unit(world, target)
        .and_then(|unit| unit.position)
        .context("Homing target is not placed")?;
    let coordinate = HexCoordinate {
        x: i32::from(position.x),
        y: i32::from(position.y),
    };
    let roll = world
        .btech
        .maps
        .get_mut(&map)
        .unwrap()
        .fire_dice
        .as_mut()
        .context("Map random stream is unavailable")?
        .two_d6();
    let pattern = flight.home(coordinate)?;
    let notices = super::artillery_feedback::arrival_notices(
        world,
        map,
        flight.weapon(),
        flight.mode(),
        coordinate,
    )?;
    let origin = flight.origin();
    let mut report = ArtilleryImpactReport {
        map,
        pattern,
        hits: Vec::new(),
        structures: Vec::new(),
        mines: Vec::new(),
        notices,
        pilot_notices: Vec::new(),
    };
    let struck = roll >= 4;
    if struck {
        let tile =
            world.btech.maps()[&map].base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
        let blast = super::blast_damage::BlastCell {
            coordinate,
            origin: origin.center(),
            tile,
            lower: i32::MIN,
            upper: i32::MAX,
        };
        if let Some(admitted) = blast.target(world, target, rules)? {
            ensure!(
                character || !admitted.character,
                "Artillery impact requires character consequence publication"
            );
            report.notices.extend(super::broadcast::observer_notices(
                world,
                target,
                "is struck by a homing missile!",
            ));
            report.notices.push(Notice {
                unit: target,
                text: "A homing missile slams into you!".into(),
            });
            let effects = super::blast_damage::resolve(
                world,
                target,
                super::blast_damage::BlastDamage {
                    damage: HOMING_DAMAGE,
                    packet_size: 5,
                    table: HitTable::Weapon,
                    arc: admitted.arc,
                    heat: 0,
                    character: admitted.character,
                    class: DamageClass::Ordinary,
                },
                admitted.rules,
            )?;
            super::piloting::append_feedback(
                &mut report.pilot_notices,
                effects.pilot_notices,
                report.notices.len(),
            );
            report.notices.extend(effects.notices);
            report.hits.push(ArtilleryHit {
                unit: target,
                coordinate,
                damage: HOMING_DAMAGE,
                arc: admitted.arc,
                impacts: effects.impacts,
                vehicle_heat: effects.vehicle_heat,
            });
        }
    }
    let cell = report.pattern.cells[0].clone();
    hit_cell(
        world,
        &mut report,
        &cell,
        origin,
        CellBlast {
            damage: HOMING_SPLASH_DAMAGE,
            packet_size: 5,
            table: HitTable::Weapon,
            sheltered: false,
            spared: struck.then_some(target),
            structures: false,
        },
        rules,
        character,
    )?;
    Ok(report)
}

/// Construction factor a blast takes off a structure, as for any area-effect weapon that
/// strikes a hex and the six around it (Total Warfare p. 174): three times the Damage Value in
/// the struck hex, twice it for the level below the roof and once for the level below that;
/// twice the outer rings' Damage Value, and once more for the level below their roofs. Walls
/// and bridges have no levels below their tops.
fn structure_damage(tile: Hex, damage: u16, direct: bool) -> u16 {
    let levels = if tile.has_building() {
        tile.structure()
            .map_or(0, |structure| u16::from(structure.height))
    } else {
        0
    };
    let multiplier = if direct {
        3 + 2 * u16::from(levels >= 1) + u16::from(levels >= 2)
    } else {
        2 + u16::from(levels >= 1)
    };
    damage.saturating_mul(multiplier)
}

/// Damage the building, wall or bridge in a struck hex once its occupants have taken their
/// damage (Tactical Operations: Advanced Rules p. 152; Total Warfare p. 173), collapsing it
/// when the blast exhausts its construction factor.
fn strike_structure(
    world: &mut World,
    report: &mut ArtilleryImpactReport,
    cell: &ArtilleryCell,
    damage: u16,
    rules: FallRules,
    character: bool,
) -> Result<()> {
    let map = report.map;
    let coordinate = cell.position;
    let record = &world.btech.maps()[&map];
    let tile = record.base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    let Some(structure) = tile.structure() else {
        return Ok(());
    };
    if damage == 0 || record.has_flag(MapFlag::IndestructibleStructures) {
        return Ok(());
    }
    let damage = structure_damage(tile, damage, cell.direct).min(structure.cf);
    let cf = structure.cf - damage;
    let name = match structure.kind {
        StructureKind::Bridge => "bridge",
        StructureKind::Wall => "wall",
        _ => "building",
    };
    let text = if cf == 0 {
        format!("The {name} at {},{} collapses!", coordinate.x, coordinate.y)
    } else {
        format!(
            "The {name} at {},{} shudders from the blast!",
            coordinate.x, coordinate.y
        )
    };
    for id in super::map_slots::all_unit_order(world, map)? {
        if hex_visible(world, id, coordinate).unwrap_or(false) {
            report.notices.push(Notice {
                unit: id,
                text: text.clone(),
            });
        }
    }
    let collapse = if cf > 0 {
        world.btech.maps.get_mut(&map).unwrap().write_hex(
            i64::from(coordinate.x),
            i64::from(coordinate.y),
            tile.with_structure_damage(damage),
        )?;
        None
    } else {
        let surface = Surface::struck(tile).unwrap_or(Surface::Roof);
        let collapse = if character {
            super::surface_break::break_surface_in_action(world, map, coordinate, surface, rules)?
        } else if surface == Surface::Bridge {
            break_bridge(world, map, coordinate, rules)?
        } else {
            collapse_structure(world, map, coordinate, rules)?
        };
        super::piloting::append_feedback(
            &mut report.pilot_notices,
            collapse.pilot_notices.iter().cloned(),
            report.notices.len(),
        );
        report.notices.extend(collapse.notices.clone());
        Some(collapse)
    };
    report.structures.push(ArtilleryStructureHit {
        coordinate,
        damage,
        cf,
        collapse,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A building of `levels` levels of `class`.
    fn building(levels: u8, class: ConstructionClass) -> Hex {
        Hex::new(Terrain::Building, 0).with_structure(Some(Structure {
            class,
            ..Structure::building(levels)
        }))
    }

    /// Area-effect damage to a structure: three times the Damage Value in the struck hex plus
    /// twice and once for the two levels below the roof, twice in the ring plus once for its
    /// level below (Total Warfare p. 174).
    #[test]
    fn structures_take_area_effect_multiples() {
        let tall = building(3, ConstructionClass::Medium);
        let low = building(1, ConstructionClass::Medium);
        assert_eq!(structure_damage(tall, 20, true), 120);
        assert_eq!(structure_damage(low, 20, true), 100);
        assert_eq!(structure_damage(tall, 10, false), 30);
        let wall = Hex::new(Terrain::Wall, 0).with_structure(Some(Structure::wall(2)));
        assert_eq!(structure_damage(wall, 20, true), 60);
        assert_eq!(structure_damage(wall, 10, false), 20);
    }

    /// Occupants of ordinary buildings take the whole blast on every floor; a hardened
    /// building shelters the floors more than a level below its roof (Tactical Operations:
    /// Advanced Rules p. 152; Total Warfare p. 173). Units above the roof are left to the
    /// vertical rule.
    #[test]
    fn building_occupants_follow_the_building_rules() {
        let medium = building(3, ConstructionClass::Medium);
        for level in 0..=3 {
            assert_eq!(building_damage(medium, level, 20, true), Some(20));
        }
        assert_eq!(building_damage(medium, 4, 20, true), None);
        let hardened = building(3, ConstructionClass::Hardened);
        let direct: Vec<_> = (0..=3)
            .map(|level| building_damage(hardened, level, 15, true))
            .collect();
        assert_eq!(direct, [Some(0), Some(8), Some(15), Some(15)]);
        let ring: Vec<_> = (0..=3)
            .map(|level| building_damage(hardened, level, 15, false))
            .collect();
        assert_eq!(ring, [Some(0), Some(0), Some(8), Some(15)]);
        assert_eq!(
            building_damage(Hex::new(Terrain::Clear, 0), 0, 20, true),
            None
        );
    }
}
