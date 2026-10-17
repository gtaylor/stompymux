//! Units inside building hexes on the battlefield (Total Warfare pp. 166–177). A ground unit
//! that walks or drives into a building hex below its roof is inside the building: it stands on
//! the level it came in at, pays the building's movement cost, pushes through the walls under a
//! piloting roll that damages the building and may damage the unit, is shielded by the
//! building against fire from outside and takes its share of the rubble when the hex
//! collapses. A roof is ordinary terrain. The `enter` command's interior maps are a separate
//! feature that this module leaves alone.
use super::{
    ConstructionClass, FallRules, Hex, HexCoordinate, HitArc, HitTable, Notice, PilotNotice,
    Position,
};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result};
use serde::Serialize;

/// Where a placed unit stands inside a building hex.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interior {
    pub map: ObjectId,
    pub coordinate: HexCoordinate,
    pub tile: Hex,
    /// The level the unit stands at, between the building's ground and the level below its roof.
    pub level: i32,
}

impl Interior {
    /// The building's construction class.
    pub fn class(self) -> ConstructionClass {
        self.tile
            .construction_class()
            .unwrap_or(ConstructionClass::Light)
    }

    /// The building hex's remaining construction factor.
    pub fn cf(self) -> u16 {
        self.tile.structure().map_or(0, |structure| structure.cf)
    }

    /// Building levels above the unit, the roof excluded (Total Warfare p. 176).
    pub fn floors_above(self) -> u16 {
        u16::try_from(i32::from(self.tile.surface_height()) - 1 - self.level).unwrap_or(0)
    }
}

/// Whether a unit at `level` in `tile` is inside its building: below the roof and no lower
/// than the building's ground.
pub fn inside(tile: Hex, level: i32) -> bool {
    tile.has_building()
        && level >= i32::from(tile.level())
        && level < i32::from(tile.surface_height())
}

/// The level a ground unit stands at after stepping into `tile` from level `from`: the roof
/// when it arrives at roof height or above, otherwise inside at the level it came from, no
/// lower than the building's ground floor (Total Warfare p. 167). Hexes without a building keep
/// their ordinary standing height.
pub fn entry_level(tile: Hex, from: i32) -> i32 {
    let roof = i32::from(tile.standing_height());
    if !tile.has_building() || from >= roof {
        return roof;
    }
    from.max(i32::from(tile.level()))
}

/// Movement points a unit inside pays for every building hex it enters, by class (Building
/// Modifiers Table, Total Warfare p. 167).
pub const fn floor_cost(class: ConstructionClass) -> u8 {
    match class {
        ConstructionClass::Light => 1,
        ConstructionClass::Medium => 2,
        ConstructionClass::Heavy => 3,
        ConstructionClass::Hardened => 4,
    }
}

/// Piloting or driving modifier for pushing through a building's walls, by class.
pub const fn entry_modifier(class: ConstructionClass) -> i16 {
    match class {
        ConstructionClass::Light => 0,
        ConstructionClass::Medium => 1,
        ConstructionClass::Heavy => 2,
        ConstructionClass::Hardened => 5,
    }
}

/// Piloting or driving modifier for the hexes a unit has moved in the turn as it enters a
/// building (Building Movement Modifiers Table, Total Warfare p. 167): none for one or two,
/// rising to +6 for twenty-five or more. The realtime game reads the hexes moved in a turn
/// from the unit's speed in movement points, as the skid modifiers do.
pub(super) fn hexes_moved_modifier(speed: f64) -> i16 {
    let hexes = speed.abs() / super::MP_KPH;
    match hexes {
        h if h < 2.1 => 0,
        h if h < 4.1 => 1,
        h if h < 6.1 => 2,
        h if h < 9.1 => 3,
        h if h < 17.1 => 4,
        h if h < 24.1 => 5,
        _ => 6,
    }
}

/// Damage a building hex absorbs from each hit grouping on a unit inside it, and the damage
/// a failed entry roll does to the unit: a tenth of the construction factor, rounded up.
pub const fn absorption(cf: u16) -> u16 {
    cf.div_ceil(10)
}

/// Damage a unit's weight does to a building hex it enters: a tenth of its tonnage, rounded up.
pub const fn weight_damage(tons: u16) -> u16 {
    tons.div_ceil(10)
}

/// Damage a collapsing building does to a unit inside: a tenth of the construction factor for
/// the unit's own level and for every level above it, rounded up (Total Warfare p. 176).
pub fn collapse_damage(cf: u16, floors_above: u16) -> u16 {
    u16::try_from((u32::from(cf) * (1 + u32::from(floors_above))).div_ceil(10)).unwrap_or(u16::MAX)
}

/// Where `id` stands inside a building, or `None` when it is elsewhere or not placed.
pub fn unit_inside(world: &World, id: ObjectId) -> Result<Option<Interior>> {
    let unit = super::scanner::scanner_unit(world, id).context("Unit is unavailable")?;
    let Some(position) = unit.position else {
        return Ok(None);
    };
    interior_at(world, id, position)
}

/// Whether a shot from `shooter` reaches `target` from another level of the building both stand
/// inside, and if so whether it comes from above. Such a shot hits a 'Mech on the Special 'Mech
/// Hit Location Table (Total Warfare p. 175). Buildings are kept hex by hex, so only units
/// sharing a building hex count as inside the same building; a unit on the roof is not inside.
pub(super) fn shot_between_levels(
    world: &World,
    shooter: ObjectId,
    target: ObjectId,
) -> Result<Option<bool>> {
    let Some(target) = unit_inside(world, target)? else {
        return Ok(None);
    };
    let Some(shooter) = unit_inside(world, shooter)? else {
        return Ok(None);
    };
    if shooter.map != target.map
        || shooter.coordinate != target.coordinate
        || shooter.level == target.level
    {
        return Ok(None);
    }
    Ok(Some(shooter.level > target.level))
}

/// Where a unit at `position` stands inside a building, reading its live level.
fn interior_at(world: &World, id: ObjectId, position: Position) -> Result<Option<Interior>> {
    let map = world
        .btech
        .maps()
        .get(&position.map)
        .context("Map not found")?;
    let tile = map.base_hex(i64::from(position.x), i64::from(position.y))?;
    if !tile.has_building() {
        return Ok(None);
    }
    let level = super::with_unit!(
        world.btech.unit(id).context("Unit is unavailable")?,
        |unit| unit.elevation_level(tile)
    );
    if !inside(tile, level) {
        return Ok(None);
    }
    Ok(Some(Interior {
        map: position.map,
        coordinate: HexCoordinate {
            x: i32::from(position.x),
            y: i32::from(position.y),
        },
        tile,
        level,
    }))
}

/// A unit's weight in tons, whichever chassis it is.
fn tonnage(world: &World, id: ObjectId) -> u16 {
    world.btech.unit(id).map_or(0, |unit| {
        super::with_unit!(unit, |unit| unit.definition().tons)
    })
}

/// What pushing through a building hex's walls did to the unit and the building.
#[derive(Default)]
pub struct WallBreach {
    pub notices: Vec<Notice>,
    pub pilot_notices: Vec<PilotNotice>,
    pub experience_messages: Vec<super::DiagnosticMessage>,
    /// Material damage the unit took from a failed roll.
    pub impacts: Vec<super::blast_damage::BlastImpact>,
    /// The collapse the entry brought down, when the building could not take the weight.
    pub collapse: Option<super::SurfaceBreak>,
}

impl WallBreach {
    /// Join this breach's messages onto a movement report, keeping pilot-only rolls ordered.
    pub(super) fn append_to(
        self,
        notices: &mut Vec<Notice>,
        pilot_notices: &mut Vec<PilotNotice>,
        experience_messages: &mut Vec<super::DiagnosticMessage>,
    ) -> Option<super::SurfaceBreak> {
        super::piloting::append_feedback(pilot_notices, self.pilot_notices, notices.len());
        notices.extend(self.notices);
        experience_messages.extend(self.experience_messages);
        self.collapse
    }
}

/// Resolve `id` having just moved into a building hex below its roof (Total Warfare p. 167):
/// the pilot rolls against the building's class and the hexes moved this turn, taking a tenth of the construction factor
/// from the front (the rear when `reverse`) on a failure, a vehicle also rolling for motive
/// damage; the hex loses a tenth of the unit's tonnage either way, and collapses when that
/// exhausts it or when the units sharing a level above the ground outweigh what is left.
/// Units standing on a roof or outside a building are untouched and return `None`.
pub(super) fn breach(
    world: &mut World,
    id: ObjectId,
    reverse: bool,
    rules: FallRules,
    character: bool,
) -> Result<Option<WallBreach>> {
    breach_with(world, id, reverse, rules, character, false)
}

/// Resolve `id` having just been displaced into a building hex below its roof: the building
/// and the unit are damaged as if it had moved in and failed its roll, so no roll is made
/// (Total Warfare pp. 151 and 167).
pub(super) fn breach_displaced(
    world: &mut World,
    id: ObjectId,
    rules: FallRules,
    character: bool,
) -> Result<Option<WallBreach>> {
    breach_with(world, id, false, rules, character, true)
}

/// Shared wall breach, with the entry roll either made or automatically failed.
fn breach_with(
    world: &mut World,
    id: ObjectId,
    reverse: bool,
    rules: FallRules,
    character: bool,
    automatic_failure: bool,
) -> Result<Option<WallBreach>> {
    let Some(interior) = unit_inside(world, id)? else {
        return Ok(None);
    };
    let class = interior.class();
    let cf = interior.cf();
    let mut breach = WallBreach::default();
    breach.notices.push(Notice {
        unit: id,
        text: format!(
            "You {} the walls of the {} building.",
            if automatic_failure {
                "are thrown through"
            } else {
                "push through"
            },
            class.label().to_ascii_lowercase()
        ),
    });
    let success = if automatic_failure {
        false
    } else {
        // The building's class and the hexes moved this turn both count (Total Warfare
        // p. 167).
        let speed = super::with_unit!(world.btech.unit(id).unwrap(), |unit| unit
            .motion()
            .map_or(0.0, |motion| motion.speed));
        let control = super::terrain_control::check(
            world,
            id,
            entry_modifier(class) + hexes_moved_modifier(speed),
            rules.extended_piloting,
            character,
        )?;
        control.capture_feedback(id, &mut breach.notices, &mut breach.pilot_notices);
        breach
            .experience_messages
            .extend(control.experience_messages);
        control.success
    };
    if !success && cf > 0 {
        let damage = absorption(cf);
        breach.notices.push(Notice {
            unit: id,
            text: format!(
                "The structure batters you for {damage} points as you force your way in!"
            ),
        });
        let arc = if reverse { HitArc::Rear } else { HitArc::Front };
        let mut remaining = damage;
        while remaining > 0 {
            let destroyed =
                super::with_unit!(world.btech.unit(id).unwrap(), |unit| unit.is_destroyed());
            if destroyed {
                break;
            }
            let group = remaining.min(5);
            remaining -= group;
            let (impact, notices) = super::blast_damage::resolve_packet(
                world,
                id,
                super::blast_damage::MaterialPacket {
                    amount: group,
                    table: HitTable::Weapon,
                    arc,
                    character,
                    attacker: None,
                    class: super::DamageClass::Impact,
                },
                arc == HitArc::Rear,
                rules,
            )?;
            impact.append_feedback(&mut breach.pilot_notices, breach.notices.len());
            breach.notices.extend(notices);
            breach.impacts.push(impact);
        }
        motive_damage(world, id, &mut breach)?;
    }
    let weight = weight_damage(tonnage(world, id));
    let record = world
        .btech
        .maps
        .get_mut(&interior.map)
        .context("Map not found")?;
    let indestructible = record.has_flag(super::MapFlag::IndestructibleStructures);
    let collapses = !indestructible && (weight >= cf || overloaded(world, interior, weight)?);
    if collapses {
        breach.notices.push(Notice {
            unit: id,
            text: "The building groans under your weight and comes down around you!".into(),
        });
        breach.collapse = Some(super::collapse_structure(
            world,
            interior.map,
            interior.coordinate,
            rules,
        )?);
        return Ok(Some(breach));
    }
    if !indestructible {
        let record = world.btech.maps.get_mut(&interior.map).unwrap();
        record.write_hex(
            i64::from(interior.coordinate.x),
            i64::from(interior.coordinate.y),
            interior.tile.with_structure_damage(weight),
        )?;
        breach.notices.push(Notice {
            unit: id,
            text: format!(
                "Your weight costs the building {weight} points; it stands at {}.",
                cf - weight
            ),
        });
    }
    Ok(Some(breach))
}

/// Whether the units sharing the entering unit's level, above the ground floor, outweigh the
/// construction factor the hex will have left (Total Warfare p. 167).
fn overloaded(world: &World, interior: Interior, weight: u16) -> Result<bool> {
    if interior.level <= i32::from(interior.tile.level()) {
        return Ok(false);
    }
    let remaining = interior.cf().saturating_sub(weight);
    let mut tons = 0_u32;
    for other in super::map_slots::all_unit_order(world, interior.map)? {
        let Some(unit) = super::scanner::scanner_unit(world, other) else {
            continue;
        };
        let Some(position) = unit.position else {
            continue;
        };
        if unit.destroyed
            || i32::from(position.x) != interior.coordinate.x
            || i32::from(position.y) != interior.coordinate.y
            || world
                .objects
                .get(&other)
                .is_none_or(|object| object.flags.contains(Flag::Going))
        {
            continue;
        }
        let level = super::with_unit!(world.btech.unit(other).unwrap(), |unit| unit
            .elevation_level(interior.tile));
        if level == interior.level {
            tons += u32::from(tonnage(world, other));
        }
    }
    Ok(tons > u32::from(remaining))
}

/// A vehicle that fails its driving roll into a building rolls once on the Motive System
/// Damage Table (Total Warfare p. 167).
fn motive_damage(world: &mut World, id: ObjectId, breach: &mut WallBreach) -> Result<()> {
    let Some(vehicle) = world.btech.vehicles().get(&id) else {
        return Ok(());
    };
    if vehicle.is_destroyed() {
        return Ok(());
    }
    let movement = vehicle.definition().movement;
    let roll = super::dice::unit_dice_mut(world, id)?.two_d6();
    let (motive, penalty) = super::vehicle_motive_effects::outcome(movement, roll);
    let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
    let (notices, mut broadcasts) =
        super::vehicle_motive_effects::apply(vehicle, id, motive, penalty, Some(roll));
    breach.notices.extend(notices);
    let (sank, observed) = super::vehicle_motive_effects::sink_immobilized_hovercraft(world, id)?;
    breach.notices.extend(sank);
    broadcasts.extend(observed);
    for broadcast in broadcasts {
        breach.notices.extend(super::broadcast::observer_notices(
            world,
            id,
            &broadcast.text,
        ));
    }
    Ok(())
}

/// What a building did to a salvo aimed at a unit inside it (Total Warfare p. 171).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BuildingCover {
    pub coordinate: HexCoordinate,
    /// Points the building took off every hit grouping.
    pub absorbed: u16,
    /// Hit groupings the building stood between.
    pub groups: u16,
    /// Construction factor the hex lost to those groupings.
    pub damage: u16,
    /// Construction factor the hex has left; zero means it collapses once the salvo lands.
    pub remaining: u16,
}

impl BuildingCover {
    /// The cockpit's view of the cover.
    pub fn notice(&self, target: ObjectId) -> Notice {
        Notice {
            unit: target,
            text: if self.remaining == 0 {
                format!(
                    "The building around you soaks up {} points of each hit, and gives way!",
                    self.absorbed
                )
            } else {
                format!(
                    "The building around you soaks up {} points of each hit.",
                    self.absorbed
                )
            },
        }
    }
}

/// Take the building's share off every hit grouping aimed at a unit inside it and charge the
/// hex the same for each grouping, dropping groupings the building absorbs whole. Fire from
/// inside the same hex is not shielded. The hex is written with its reduced construction
/// factor unless the groupings exhaust it, in which case it is left for [`collapse`] to bring
/// down after the salvo lands.
pub(super) fn absorb(
    world: &mut World,
    shooter: Option<ObjectId>,
    target: ObjectId,
    groups: &mut Vec<u16>,
) -> Result<Option<BuildingCover>> {
    let Some(interior) = unit_inside(world, target)? else {
        return Ok(None);
    };
    if groups.is_empty() || interior.cf() == 0 {
        return Ok(None);
    }
    if let Some(shooter) = shooter {
        let same_hex = super::scanner::scanner_unit(world, shooter)
            .and_then(|unit| unit.position)
            .is_some_and(|position| {
                position.map == interior.map
                    && i32::from(position.x) == interior.coordinate.x
                    && i32::from(position.y) == interior.coordinate.y
            });
        if same_hex {
            return Ok(None);
        }
    }
    let absorbed = absorption(interior.cf());
    let count = u16::try_from(groups.len()).unwrap_or(u16::MAX);
    for group in groups.iter_mut() {
        *group = group.saturating_sub(absorbed);
    }
    groups.retain(|group| *group > 0);
    let indestructible =
        world.btech.maps()[&interior.map].has_flag(super::MapFlag::IndestructibleStructures);
    let damage = if indestructible {
        0
    } else {
        absorbed.saturating_mul(count).min(interior.cf())
    };
    let remaining = interior.cf() - damage;
    if remaining > 0 && damage > 0 {
        world.btech.maps.get_mut(&interior.map).unwrap().write_hex(
            i64::from(interior.coordinate.x),
            i64::from(interior.coordinate.y),
            interior.tile.with_structure_damage(damage),
        )?;
    }
    Ok(Some(BuildingCover {
        coordinate: interior.coordinate,
        absorbed,
        groups: count,
        damage,
        remaining,
    }))
}

impl BuildingCover {
    /// Charge the building a hit its partial cover stopped: a leg hit on a standing 'Mech it
    /// covers strikes the building instead, damaging it as normal (Total Warfare p. 171). The
    /// hex keeps its reduced construction factor unless the hit exhausts it, in which case
    /// [`collapse`] brings it down once the salvo lands, as for the absorbed share.
    pub(super) fn strike(&mut self, world: &mut World, map: ObjectId, damage: u16) -> Result<()> {
        if self.remaining == 0
            || world.btech.maps()[&map].has_flag(super::MapFlag::IndestructibleStructures)
        {
            return Ok(());
        }
        let damage = damage.min(self.remaining);
        self.damage += damage;
        self.remaining -= damage;
        if self.remaining > 0 && damage > 0 {
            let tile = world.btech.maps()[&map]
                .base_hex(i64::from(self.coordinate.x), i64::from(self.coordinate.y))?;
            world.btech.maps.get_mut(&map).unwrap().write_hex(
                i64::from(self.coordinate.x),
                i64::from(self.coordinate.y),
                tile.with_structure_damage(damage),
            )?;
        }
        Ok(())
    }
}

/// Bring down the building a salvo exhausted, once its hit groupings have landed.
pub(super) fn collapse(
    world: &mut World,
    target: ObjectId,
    cover: &BuildingCover,
    rules: FallRules,
) -> Result<Option<super::SurfaceBreak>> {
    if cover.remaining > 0 {
        return Ok(None);
    }
    let map = super::scanner::scanner_unit(world, target)
        .and_then(|unit| unit.position)
        .context("Target is not placed")?
        .map;
    let standing = world.btech.maps()[&map]
        .base_hex(i64::from(cover.coordinate.x), i64::from(cover.coordinate.y))?
        .has_standing_structure();
    if !standing {
        return Ok(None);
    }
    Ok(Some(super::collapse_structure(
        world,
        map,
        cover.coordinate,
        rules,
    )?))
}

/// A missed shot from an adjacent hex striking the building around the target.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StrayHit {
    pub coordinate: HexCoordinate,
    pub damage: u16,
    pub remaining: u16,
    pub collapse: Option<super::SurfaceBreak>,
    pub notices: Vec<Notice>,
}

/// A shot that misses a unit inside a building from the hex next door strikes the building
/// instead (Total Warfare p. 171); from farther away it flies on and does nothing.
pub(super) fn stray_shot(
    world: &mut World,
    shooter: ObjectId,
    target: ObjectId,
    damage: u16,
    rules: FallRules,
) -> Result<Option<StrayHit>> {
    let Some(interior) = unit_inside(world, target)? else {
        return Ok(None);
    };
    if damage == 0
        || interior.cf() == 0
        || super::unit_range(world, shooter, target)?.hex_distance != 1
        || world.btech.maps()[&interior.map].has_flag(super::MapFlag::IndestructibleStructures)
    {
        return Ok(None);
    }
    let (x, y) = (interior.coordinate.x, interior.coordinate.y);
    let remaining = interior.cf().saturating_sub(damage);
    let mut notices = Vec::new();
    for id in super::map_slots::all_unit_order(world, interior.map)? {
        if super::hex_visible(world, id, interior.coordinate).unwrap_or(false) {
            notices.push(Notice {
                unit: id,
                text: if remaining == 0 {
                    format!("The building at {x},{y} collapses!")
                } else {
                    format!("The building at {x},{y} shudders as the shot strikes it!")
                },
            });
        }
    }
    let collapse = if remaining == 0 {
        Some(super::collapse_structure(
            world,
            interior.map,
            interior.coordinate,
            rules,
        )?)
    } else {
        world.btech.maps.get_mut(&interior.map).unwrap().write_hex(
            i64::from(x),
            i64::from(y),
            interior.tile.with_structure_damage(damage),
        )?;
        None
    };
    Ok(Some(StrayHit {
        coordinate: interior.coordinate,
        damage: damage.min(interior.cf()),
        remaining,
        collapse,
        notices,
    }))
}

/// Physical attacks cannot reach a unit inside a building from outside its hex (Total Warfare
/// p. 171); charges are the exception and do not come through here.
pub(super) fn check_physical(world: &World, attacker: ObjectId, target: ObjectId) -> Result<()> {
    let Some(interior) = unit_inside(world, target)? else {
        return Ok(());
    };
    let same_hex = super::scanner::scanner_unit(world, attacker)
        .and_then(|unit| unit.position)
        .is_some_and(|position| {
            position.map == interior.map
                && i32::from(position.x) == interior.coordinate.x
                && i32::from(position.y) == interior.coordinate.y
        });
    anyhow::ensure!(
        same_hex,
        "Your target is inside a building; you cannot reach it from out here."
    );
    Ok(())
}

/// Units in a collapsing building hex take its rubble (Total Warfare pp. 176–177): a tenth of
/// the construction factor for their own level and each level above them, in five-point
/// groups from the front, 'Mechs inside on the Punch Location Table and 'Mechs on the roof on
/// the Hit Location Table. Units inside above the ground floor then fall to the rubble; those
/// on the ground floor stand on it. The caller drops the roof's occupants afterwards. `tile` is
/// the hex before the collapse. Returns the buried units' ids.
pub(super) fn bury_occupants(
    world: &mut World,
    map: ObjectId,
    coordinate: HexCoordinate,
    tile: Hex,
    rules: FallRules,
    character: bool,
    notices: &mut Vec<Notice>,
    pilot_notices: &mut Vec<PilotNotice>,
) -> Result<Vec<ObjectId>> {
    let cf = tile.structure().map_or(0, |structure| structure.cf);
    let mut buried = Vec::new();
    for id in super::map_slots::all_unit_order(world, map)? {
        let Some(unit) = super::scanner::scanner_unit(world, id) else {
            continue;
        };
        if unit.destroyed
            || world
                .objects
                .get(&id)
                .is_none_or(|object| object.flags.contains(Flag::Going))
            || unit.position.is_none_or(|position| {
                position.map != map
                    || i32::from(position.x) != coordinate.x
                    || i32::from(position.y) != coordinate.y
            })
        {
            continue;
        }
        let (level, airborne) = match world.btech.unit(id).context("Unit is unavailable")? {
            super::UnitRef::Mech(mech) => (mech.elevation_level(tile), mech.airborne()),
            super::UnitRef::Vehicle(vehicle) => (
                vehicle.elevation_level(tile),
                vehicle.vtol_flight().is_some(),
            ),
        };
        let roof = level == i32::from(tile.surface_height());
        if airborne || !(roof || inside(tile, level)) {
            continue;
        }
        let interior = Interior {
            map,
            coordinate,
            tile,
            level,
        };
        let damage = collapse_damage(cf, interior.floors_above());
        notices.push(Notice {
            unit: id,
            text: if roof {
                format!("The roof gives way beneath you for {damage} points!")
            } else {
                format!("The building comes down on top of you for {damage} points!")
            },
        });
        notices.extend(super::broadcast::observer_notices(
            world,
            id,
            if roof {
                "drops as the roof gives way!"
            } else {
                "is buried as the building collapses!"
            },
        ));
        let in_character = character && world.objects[&id].flags.contains(Flag::InCharacter);
        let mut remaining = damage;
        while remaining > 0 {
            let destroyed =
                super::with_unit!(world.btech.unit(id).unwrap(), |unit| unit.is_destroyed());
            if destroyed {
                break;
            }
            let group = remaining.min(5);
            remaining -= group;
            let (impact, impact_notices) = super::blast_damage::resolve_packet(
                world,
                id,
                super::blast_damage::MaterialPacket {
                    amount: group,
                    table: if roof {
                        HitTable::Weapon
                    } else {
                        HitTable::Punch
                    },
                    arc: HitArc::Front,
                    character: in_character,
                    attacker: None,
                    class: super::DamageClass::Ordinary,
                },
                false,
                rules,
            )?;
            impact.append_feedback(pilot_notices, notices.len());
            notices.extend(impact_notices);
        }
        if roof {
            buried.push(id);
            continue;
        }
        let drop = level - i32::from(tile.level());
        if drop > 0 {
            // Floors above the ground give way: a fall of that many levels onto the rubble.
            notices.push(Notice {
                unit: id,
                text: "The floor drops away beneath you!".into(),
            });
            collapse_fall(world, id, drop, rules, in_character, notices, pilot_notices)?;
            buried.push(id);
            continue;
        }
        // The rubble is the ground now; the unit stands on it.
        match world.btech.unit_mut(id).context("Unit is unavailable")? {
            super::UnitMut::Mech(mech) => mech.ground_elevation = None,
            super::UnitMut::Vehicle(vehicle) => vehicle.ground_elevation = None,
        }
        buried.push(id);
    }
    Ok(buried)
}

/// Drop a unit `levels` floors from inside a collapsing building to its rubble, whichever
/// chassis it is, keeping the fall's feedback in the collapse's notice stream.
fn collapse_fall(
    world: &mut World,
    id: ObjectId,
    levels: i32,
    rules: FallRules,
    character: bool,
    notices: &mut Vec<Notice>,
    pilot_notices: &mut Vec<PilotNotice>,
) -> Result<()> {
    let destroyed = super::with_unit!(world.btech.unit(id).unwrap(), |unit| unit.is_destroyed());
    if destroyed {
        return Ok(());
    }
    if world.btech.vehicles().contains_key(&id) {
        let fall = super::vehicle_fall::resolve_material(
            world,
            id,
            u32::try_from(levels + 1).unwrap_or(u32::MAX),
            rules,
            character,
        )?;
        super::piloting::append_feedback(
            pilot_notices,
            fall.feedback.pilot_notices.iter().cloned(),
            notices.len(),
        );
        notices.extend(fall.feedback.notices.iter().cloned());
        return Ok(());
    }
    let fall = super::fall::resolve_material(world, id, levels + 1, rules, character)?;
    fall.append_notices(id, notices, pilot_notices);
    Ok(())
}

/// The construction class of the building a unit stands in, for movement cost.
pub(super) fn floor_cost_at(tile: Hex, level: i32) -> u8 {
    if !inside(tile, level) {
        return 0;
    }
    floor_cost(
        tile.construction_class()
            .unwrap_or(ConstructionClass::Light),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::{Structure, StructureKind, Terrain};

    fn building(level: u8, height: u8, class: ConstructionClass) -> Hex {
        Hex::new(Terrain::Clear, level).with_structure(Some(Structure::new(
            StructureKind::Building,
            height,
            class,
        )))
    }

    /// The Building Movement Modifiers Table's hexes-moved column, read from speed in MP
    /// (Total Warfare p. 167): a Locust that moved four hexes adds one.
    #[test]
    fn hexes_moved_add_to_building_entry_rolls() {
        for (mp, modifier) in [
            (1.0, 0),
            (2.0, 0),
            (3.0, 1),
            (4.0, 1),
            (6.0, 2),
            (9.0, 3),
            (12.0, 4),
            (20.0, 5),
            (30.0, 6),
        ] {
            assert_eq!(
                hexes_moved_modifier(mp * super::super::MP_KPH),
                modifier,
                "{mp}"
            );
        }
        assert_eq!(hexes_moved_modifier(-4.0 * super::super::MP_KPH), 1);
    }

    /// A unit enters at the level it comes from, clamped to the building's floors, and takes
    /// the roof when it arrives at roof height.
    #[test]
    fn entry_levels_follow_the_approach() {
        let tile = building(1, 3, ConstructionClass::Medium);
        assert_eq!(entry_level(tile, 0), 1);
        assert_eq!(entry_level(tile, 1), 1);
        assert_eq!(entry_level(tile, 2), 2);
        assert_eq!(entry_level(tile, 3), 3);
        assert_eq!(entry_level(tile, 4), 4);
        assert_eq!(entry_level(tile, 7), 4);
        assert!(inside(tile, 1) && inside(tile, 3));
        assert!(!inside(tile, 4) && !inside(tile, 0));
        let hill = Hex::new(Terrain::Clear, 2);
        assert_eq!(entry_level(hill, 0), 2);
        assert!(!inside(hill, 2));
    }

    /// The tables of Total Warfare pp. 167, 171 and 176.
    #[test]
    fn building_numbers_follow_total_warfare() {
        assert_eq!(floor_cost(ConstructionClass::Light), 1);
        assert_eq!(floor_cost(ConstructionClass::Hardened), 4);
        assert_eq!(entry_modifier(ConstructionClass::Heavy), 2);
        assert_eq!(entry_modifier(ConstructionClass::Hardened), 5);
        assert_eq!(absorption(38), 4);
        assert_eq!(absorption(40), 4);
        assert_eq!(weight_damage(65), 7);
        assert_eq!(collapse_damage(40, 0), 4);
        assert_eq!(collapse_damage(45, 2), 14);
        let tile = building(0, 3, ConstructionClass::Heavy);
        let ground = Interior {
            map: ObjectId(1),
            coordinate: HexCoordinate { x: 0, y: 0 },
            tile,
            level: 0,
        };
        assert_eq!(ground.floors_above(), 2);
        assert_eq!(Interior { level: 2, ..ground }.floors_above(), 0);
        assert_eq!(floor_cost_at(tile, 1), 3);
        assert_eq!(floor_cost_at(tile, 3), 0);
    }
}
