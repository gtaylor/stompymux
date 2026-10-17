//! Skidding on pavement and ice, and hovercraft sideslips (Total Warfare pp. 62–63 and 67,
//! Tactical Operations p. 48). A 'Mech or ground vehicle that changes facing on a slick surface
//! and then moves on rolls to keep its footing; when it fails it drops and slides along its old
//! course, taking half a fall's damage for every hex it slides. The slide ends in water a level
//! deep, drops it over any edge steeper than it could step down, crashes it into higher ground
//! or a building, leaves a vehicle immobile in terrain it could never enter, and throws the
//! sliding unit into any unit in its path as an unintentional charge. A hovercraft that turns at
//! flank speed on any ground and fails instead sideslips: it drifts along its old course for as
//! many hexes as it missed the roll by, without damage unless it drifts into something.
use super::{
    FallRules, GroundMovement, Hex, HexCoordinate, HitArc, HitTable, MechFallReport, Notice,
    PilotNotice, Point, SalvoGroup, VehicleMovement,
};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// The heading a unit held when it came onto the slick surface it is crossing, and whether it
/// has turned since. Saved with the unit so a skid check survives a restart mid-turn.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlickTrack {
    /// Heading in degrees when the unit started across the surface; `None` off slick ground.
    #[serde(default)]
    pub heading: Option<f64>,
    /// Whether the unit has changed facing on the surface since.
    #[serde(default)]
    pub turned: bool,
}

impl SlickTrack {
    /// Watch a unit standing or moving on a hex: start tracking on a slick surface, note a
    /// facing change there, and forget everything elsewhere or when the unit stops.
    pub(super) fn observe(&mut self, slick: bool, moving: bool, heading: f64) {
        if !slick || !moving {
            *self = Self::default();
            return;
        }
        match self.heading {
            None => self.heading = Some(heading),
            Some(held) => {
                let difference = (heading - held + 180.0).rem_euclid(360.0) - 180.0;
                if difference.abs() > 0.5 {
                    self.turned = true;
                }
            }
        }
    }

    /// Forget the surface when the unit leaves the hex it turned on.
    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }

    /// Saved headings must be real bearings.
    pub(crate) fn validate(self) -> Result<()> {
        anyhow::ensure!(
            self.heading
                .is_none_or(|heading| heading.is_finite() && (0.0..360.0).contains(&heading)),
            "Invalid skid heading"
        );
        anyhow::ensure!(
            self.heading.is_some() || !self.turned,
            "A unit cannot have turned on a surface it is not tracking"
        );
        Ok(())
    }
}

/// Skid roll modifier from the hexes moved this turn, read from the unit's speed in movement
/// points (Total Warfare p. 63 skid modifiers table).
pub(super) fn modifier(speed: f64) -> i16 {
    let mp = speed.abs() / 10.75;
    if mp < 2.1 {
        return -1;
    }
    if mp < 4.1 {
        return 0;
    }
    if mp < 7.1 {
        return 1;
    }
    if mp < 10.1 {
        return 2;
    }
    if mp < 17.1 {
        return 4;
    }
    if mp < 24.1 {
        return 5;
    }
    6
}

/// Hexes a failed skid slides: half the hexes moved this turn, rounded up, at least one.
pub(super) fn distance(speed: f64) -> u8 {
    ((speed.abs() / 10.75) / 2.0).ceil().clamp(1.0, 255.0) as u8
}

/// Whether moving off `tile` after turning on it risks a skid: any speed on ice, and on
/// pavement, a road or a bridge deck only for a unit running or flanking, that is moving
/// faster than its walking or cruising speed `walking` (Total Warfare pp. 60, 62).
pub(super) fn risks_skid(tile: Hex, speed: f64, walking: f64) -> bool {
    tile.is_slick() && (tile.is_icy() || speed.abs() > walking + 0.1)
}

/// Whether a vehicle moving off `tile` after turning on it risks losing its line: a hovercraft
/// sideslips at flank speed whatever the ground under it (Total Warfare pp. 62, 67); any other
/// vehicle skids as [`risks_skid`] says. `cruising` is the vehicle's cruising speed.
pub(super) fn risks_slide(tile: Hex, speed: f64, cruising: f64, hover: bool) -> bool {
    if hover {
        return speed.abs() > cruising + 0.1;
    }
    risks_skid(tile, speed, cruising)
}

/// What a skid or sideslip check produced; the caller publishes notices and nested falls.
#[derive(Default)]
pub(super) struct SkidReport {
    pub notices: Vec<Notice>,
    pub pilot_notices: Vec<PilotNotice>,
    pub experience_messages: Vec<super::DiagnosticMessage>,
    pub falls: Vec<MechFallReport>,
    pub vehicle_falls: Vec<super::VehicleFallReport>,
    pub character_injuries: Vec<super::CharacterPilotInjury>,
    /// Unintentional charges the sliding 'Mech made into units in its path.
    pub charges: Vec<super::ChargeReport>,
    /// Buildings the slide brought down.
    pub collapses: Vec<super::SurfaceBreak>,
    /// Whether the unit lost control and its movement is over.
    pub skidded: bool,
}

/// What a sliding unit meets in the next hex.
enum Obstacle {
    /// Another unit stands there: the sliding unit charges it (Total Warfare p. 63).
    Unit(ObjectId),
    /// A building the unit slides into for a tenth of its construction factor.
    Building,
    /// Higher ground the unit crashes into for a point of damage per twenty tons.
    Collision,
    /// Terrain the unit could never enter: a point of damage per five tons, then it halts.
    Prohibited,
}

/// The hexes a unit sliding `hexes` cells along `heading` from `point` would pass through.
pub(super) fn path(point: Point, heading: f64, hexes: u8) -> Result<Vec<(HexCoordinate, Point)>> {
    let end = point.project(heading, f64::from(hexes))?;
    let mut cells = point.trace_positions(end)?;
    cells.remove(0);
    Ok(cells)
}

/// The first other unit standing in `hex`, in map-slot order.
fn occupant(
    world: &World,
    id: ObjectId,
    map: ObjectId,
    hex: HexCoordinate,
) -> Result<Option<ObjectId>> {
    Ok(super::map_slots::hex_occupants(world, map, hex)?
        .into_iter()
        .find(|other| {
            *other != id
                && world
                    .objects
                    .get(other)
                    .is_some_and(|object| !object.flags.contains(Flag::Going))
        }))
}

/// The level a sliding unit would rest at in `tile`: the water surface for a hovercraft or
/// amphibious vehicle, otherwise the ground, ice or structure it stands on.
fn resting_height(tile: Hex, surface_rider: bool) -> i32 {
    if surface_rider && tile.is_water_surface() {
        return i32::from(tile.water_line());
    }
    i32::from(tile.standing_height())
}

/// Classify the next hex of a slide for a unit at `height`; `surface_rider` units cross water
/// on its surface instead of dropping to the bed.
fn obstacle(
    world: &World,
    id: ObjectId,
    map: ObjectId,
    hex: HexCoordinate,
    tile: Hex,
    height: i32,
    movement: GroundMovement,
    surface_rider: bool,
) -> Result<Option<Obstacle>> {
    if let Some(other) = occupant(world, id, map, hex)? {
        return Ok(Some(Obstacle::Unit(other)));
    }
    let surface = resting_height(tile, surface_rider);
    if surface > height && tile.has_standing_structure() {
        return Ok(Some(Obstacle::Building));
    }
    if surface > height {
        return Ok(Some(Obstacle::Collision));
    }
    if tile
        .prohibition(movement)
        .is_some_and(|reason| reason != super::Prohibition::Water)
    {
        return Ok(Some(Obstacle::Prohibited));
    }
    Ok(None)
}

/// Resolve a 'Mech's skid check as it tries to leave a slick hex after turning on it. On
/// failure the 'Mech falls and slides along `heading`, its course before the turn.
pub(super) fn resolve_mech(
    world: &mut World,
    id: ObjectId,
    heading: f64,
    speed: f64,
    rules: FallRules,
    character: bool,
) -> Result<SkidReport> {
    let mut report = SkidReport::default();
    let unit = &world.btech.constructed_units()[&id];
    let position = unit.position().context("Skidding unit is not placed")?;
    let tile =
        world.btech.maps()[&position.map].base_hex(i64::from(position.x), i64::from(position.y))?;
    report.notices.push(Notice {
        unit: id,
        text: format!(
            "Your footing slips as you turn on the {}!",
            surface_name(tile)
        ),
    });
    let control = super::terrain_control::check(
        world,
        id,
        modifier(speed),
        rules.extended_piloting,
        character,
    )?;
    control.capture_feedback(id, &mut report.notices, &mut report.pilot_notices);
    report
        .experience_messages
        .extend(control.experience_messages);
    if control.success {
        report.notices.push(Notice {
            unit: id,
            text: "You keep your feet.".into(),
        });
        return Ok(report);
    }
    report.skidded = true;
    report.notices.push(Notice {
        unit: id,
        text: "You lose your footing and skid!".into(),
    });
    report
        .notices
        .extend(super::broadcast::observer_notices(world, id, "skids!"));
    let mut fall = if character && world.objects[&id].flags.contains(Flag::InCharacter) {
        super::fall::resolve_character_fall(world, id, 1, rules)?
    } else {
        super::resolve_fall(world, id, 1, rules)?
    };
    fall.append_notices(id, &mut report.notices, &mut report.pilot_notices);
    let per_hex = fall.damage.div_ceil(2);
    let tons = u32::from(world.btech.constructed_units()[&id].definition().tons);
    // Collisions use only the hexes moved before the skid, never the hexes slid.
    let hexes_moved = (speed.abs() / 10.75) as u32;
    let point = world.btech.constructed_units()[&id]
        .motion()
        .context("Skidding unit has no motion")?
        .point;
    // Hexes left to skid; terrain other than clear or paved ground uses them up faster
    // (Total Warfare p. 62).
    let mut left = i32::from(distance(speed));
    for (hex, _) in path(point, heading, distance(speed))? {
        let unit = &world.btech.constructed_units()[&id];
        if unit.is_destroyed() || left <= 0 {
            break;
        }
        let Ok(next) =
            world.btech.maps()[&position.map].base_hex(i64::from(hex.x), i64::from(hex.y))
        else {
            break;
        };
        let here = unit.position().unwrap();
        let current =
            world.btech.maps()[&position.map].base_hex(i64::from(here.x), i64::from(here.y))?;
        let height = unit.elevation_level(current);
        match obstacle(
            world,
            id,
            position.map,
            hex,
            next,
            height,
            GroundMovement::Legged,
            false,
        )? {
            Some(Obstacle::Unit(other)) => {
                // An unintentional charge; a hit ends the skid with one more hex of damage,
                // a miss slides on through the hex (Total Warfare p. 63).
                let Some(charge) = super::charge::resolve_skid_charge(
                    world,
                    id,
                    other,
                    hexes_moved,
                    rules,
                    character,
                )?
                else {
                    break;
                };
                super::piloting::append_feedback(
                    &mut report.pilot_notices,
                    charge.pilot_notices.iter().cloned(),
                    report.notices.len(),
                );
                report.notices.extend(charge.notices.iter().cloned());
                let hit = charge.hit;
                report.charges.push(charge);
                if hit {
                    slide_damage(world, id, per_hex, &mut fall, rules, character)?;
                    break;
                }
            }
            Some(Obstacle::Building) => {
                let destroyed = building_collision(
                    world,
                    id,
                    hex,
                    next,
                    height,
                    (per_hex, tons, hexes_moved),
                    &mut fall,
                    rules,
                    character,
                    &mut report,
                )?;
                // A building the skid brings down lets it go on, less the building's movement
                // cost; one that stands ends it (Total Warfare p. 63).
                left -= 1;
                let class = next
                    .construction_class()
                    .unwrap_or(super::ConstructionClass::Light);
                left -= i32::from(super::building_interior::floor_cost(class));
                if !destroyed || left <= 0 || world.btech.constructed_units()[&id].is_destroyed() {
                    break;
                }
                continue;
            }
            Some(Obstacle::Collision) => {
                report.notices.push(Notice {
                    unit: id,
                    text: "You slam into the rising ground!".into(),
                });
                slide_damage(world, id, tons.div_ceil(20), &mut fall, rules, character)?;
                break;
            }
            Some(Obstacle::Prohibited) => {
                move_mech(world, id, hex, next);
                report.notices.push(Notice {
                    unit: id,
                    text: format!(
                        "You crash into the {}!",
                        next.terrain().label().to_ascii_lowercase()
                    ),
                });
                slide_damage(world, id, tons.div_ceil(5), &mut fall, rules, character)?;
                break;
            }
            None => {}
        }
        let drop = height - i32::from(next.standing_height());
        move_mech(world, id, hex, next);
        report.notices.push(Notice {
            unit: id,
            text: format!("You skid into {},{}.", hex.x, hex.y),
        });
        if drop > 2 {
            // Too steep to step down: no skid damage for the hex, but a fall of that many
            // levels into it, and the skid ends there (Total Warfare p. 62).
            report.notices.push(Notice {
                unit: id,
                text: "You skid over the edge and tumble down!".into(),
            });
            let levels = u8::try_from(drop + 1).unwrap_or(u8::MAX);
            let tumble = if character && world.objects[&id].flags.contains(Flag::InCharacter) {
                super::fall::resolve_character_fall(world, id, levels, rules)?
            } else {
                super::resolve_fall(world, id, levels, rules)?
            };
            tumble.append_notices(id, &mut report.notices, &mut report.pilot_notices);
            report.falls.push(tumble);
            break;
        }
        slide_damage(world, id, per_hex, &mut fall, rules, character)?;
        let flooding = if character {
            super::flooding::flood_unit_in_action(world, id, rules)?
        } else {
            super::flood_unit(world, id, rules)?
        };
        for exposure in flooding {
            report.notices.extend(exposure.notices);
        }
        if next.water_depth() >= 1 {
            report.notices.push(Notice {
                unit: id,
                text: "The water brings your skid to an end.".into(),
            });
            break;
        }
        left -= i32::from(skid_cost(next, GroundMovement::Legged));
    }
    report.falls.insert(0, fall);
    if let Some(motion) = world
        .btech
        .constructed
        .get_mut(&id)
        .unwrap()
        .motion
        .as_mut()
    {
        motion.stop_translation();
        motion.desired_heading = motion.heading;
    }
    Ok(report)
}

/// Hexes of a skid that crossing `tile` uses up: what the Movement Cost Table charges to enter
/// it, so a clear or paved hex uses one and woods or rough ground more (Total Warfare p. 62).
/// The ice a unit skids on is the slick surface itself and uses no more than the ground under
/// it.
fn skid_cost(tile: Hex, movement: GroundMovement) -> u8 {
    let cost = tile.movement_cost(movement);
    if tile.is_icy() {
        return cost.saturating_sub(1).max(1);
    }
    cost
}

/// A sliding 'Mech crashes into a building hex (Total Warfare p. 63): it slides in and takes
/// its skid damage for the hex, then a tenth of the construction factor, while the building
/// takes the charge damage of the hexes moved before the skid. The skid stops in the hex,
/// inside the building, unless that brings it down; returns whether it did.
#[allow(clippy::too_many_arguments)]
fn building_collision(
    world: &mut World,
    id: ObjectId,
    hex: HexCoordinate,
    tile: Hex,
    height: i32,
    (per_hex, tons, hexes_moved): (u32, u32, u32),
    fall: &mut MechFallReport,
    rules: FallRules,
    character: bool,
    report: &mut SkidReport,
) -> Result<bool> {
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let level = super::building_interior::entry_level(tile, height);
    move_mech(world, id, hex, tile);
    let unit = world.btech.constructed.get_mut(&id).unwrap();
    unit.ground_elevation =
        super::building_interior::inside(tile, level).then_some(f64::from(level));
    report.notices.push(Notice {
        unit: id,
        text: format!("You skid into the building at {},{}!", hex.x, hex.y),
    });
    slide_damage(world, id, per_hex, fall, rules, character)?;
    let cf = tile.structure().map_or(0, |structure| structure.cf);
    slide_damage(
        world,
        id,
        u32::from(super::building_interior::absorption(cf)),
        fall,
        rules,
        character,
    )?;
    if world.btech.maps()[&map].has_flag(super::MapFlag::IndestructibleStructures) {
        return Ok(false);
    }
    let charge =
        super::charge::charge_damage(u16::try_from(tons).unwrap_or(u16::MAX), hexes_moved)?;
    if charge >= cf {
        report.notices.push(Notice {
            unit: id,
            text: "The building comes down around you!".into(),
        });
        let collapse = super::collapse_structure(world, map, hex, rules)?;
        super::piloting::append_feedback(
            &mut report.pilot_notices,
            collapse.pilot_notices.iter().cloned(),
            report.notices.len(),
        );
        report.notices.extend(collapse.notices.iter().cloned());
        report.collapses.push(collapse);
        return Ok(true);
    }
    world.btech.maps.get_mut(&map).unwrap().write_hex(
        i64::from(hex.x),
        i64::from(hex.y),
        tile.with_structure_damage(charge),
    )?;
    report.notices.push(Notice {
        unit: id,
        text: format!(
            "The building shudders, losing {charge} points; it stands at {}.",
            cf - charge
        ),
    });
    Ok(false)
}

/// Put a prone, sliding 'Mech in `hex`, resting on its surface.
fn move_mech(world: &mut World, id: ObjectId, hex: HexCoordinate, tile: Hex) {
    let unit = world.btech.constructed.get_mut(&id).unwrap();
    let map = unit.position.map(|position| position.map).unwrap();
    unit.position = Some(super::Position {
        map,
        x: hex.x as u16,
        y: hex.y as u16,
    });
    unit.ground_elevation = (tile.standing_height() != tile.surface_height())
        .then_some(f64::from(tile.standing_height()));
    unit.hex_sync_pending = false;
    if let Some(motion) = unit.motion.as_mut() {
        motion.point = hex.center();
    }
}

/// Apply `amount` points of sliding damage to a 'Mech in five-point groups, from the side the
/// fall turned toward, extending the fall's own damage record.
fn slide_damage(
    world: &mut World,
    id: ObjectId,
    amount: u32,
    fall: &mut MechFallReport,
    rules: FallRules,
    character: bool,
) -> Result<()> {
    let mut remaining = amount;
    while remaining > 0 {
        let unit = &world.btech.constructed_units()[&id];
        if unit.is_destroyed() {
            break;
        }
        let group = remaining.min(5);
        remaining -= group;
        let group = group as u16;
        let mut dice = unit.dice.clone();
        let roll = dice.generic_roll();
        let hit = super::resolve_hit(unit, fall.arc, roll, &mut dice)?;
        world.btech.constructed.get_mut(&id).unwrap().dice = dice;
        let outcome = if character && world.objects[&id].flags.contains(Flag::InCharacter) {
            super::impact::resolve_character_impact_with_rules(
                world,
                id,
                hit,
                group,
                Some(rules),
                Some(super::impact::WeaponEffect::Impact),
            )?
        } else {
            super::pilot_injury::resolve_tactical_impact_in_candidate(
                world,
                id,
                hit,
                group,
                rules,
                Some(super::impact::WeaponEffect::Impact),
            )?
        };
        fall.damage += u32::from(group);
        fall.groups.push(SalvoGroup {
            damage: group,
            hit,
            impact: outcome.impact,
            pilot_injuries: outcome.pilot_injuries,
            pilot_notices: outcome.pilot_notices,
            notices: outcome.notices,
            balance: outcome.balance,
            flooding: outcome.flooding,
        });
    }
    Ok(())
}

/// Resolve a ground vehicle's skid or a hovercraft's sideslip as it tries to leave a slick hex
/// after turning on it, sliding along `heading`, its course before the turn.
pub(super) fn resolve_vehicle(
    world: &mut World,
    id: ObjectId,
    heading: f64,
    speed: f64,
    rules: FallRules,
    character: bool,
) -> Result<SkidReport> {
    let mut report = SkidReport::default();
    let (movement, position, tons, point, surface_rider) = {
        let vehicle = &world.btech.vehicles()[&id];
        (
            vehicle.definition().movement,
            vehicle
                .position()
                .context("Skidding vehicle is not placed")?,
            u32::from(vehicle.definition().tons),
            vehicle
                .motion()
                .context("Skidding vehicle has no motion")?
                .point,
            vehicle.rides_water_surface(),
        )
    };
    let hover = movement == VehicleMovement::Hover;
    // Collisions use only the hexes moved before the skid, never the hexes slid.
    let hexes_moved = (speed.abs() / 10.75) as u32;
    let tile =
        world.btech.maps()[&position.map].base_hex(i64::from(position.x), i64::from(position.y))?;
    report.notices.push(Notice {
        unit: id,
        text: if hover {
            "Your skirts bite as you turn; you fight to hold your line!".into()
        } else {
            format!(
                "Your wheels slip as you turn on the {}!",
                surface_name(tile)
            )
        },
    });
    let control = super::terrain_control::check(
        world,
        id,
        if hover { 0 } else { modifier(speed) },
        rules.extended_piloting,
        character,
    )?;
    let margin = control.margin_of_failure();
    control.capture_feedback(id, &mut report.notices, &mut report.pilot_notices);
    report
        .experience_messages
        .extend(control.experience_messages);
    if control.success {
        report.notices.push(Notice {
            unit: id,
            text: "You hold your course.".into(),
        });
        return Ok(report);
    }
    report.skidded = true;
    let hexes = if hover {
        // A sideslip drifts one hex per point missed by, never farther than one less than the
        // hexes already entered this turn.
        margin.min(((speed.abs() / 10.75) as u8).saturating_sub(1))
    } else {
        distance(speed)
    };
    report.notices.push(Notice {
        unit: id,
        text: if hover {
            "You lose your line and sideslip!".into()
        } else {
            "You lose control and skid!".into()
        },
    });
    report.notices.extend(super::broadcast::observer_notices(
        world,
        id,
        if hover { "sideslips!" } else { "skids!" },
    ));
    let ground = super::vehicle_motion::ground_movement(movement);
    let per_hex = if hover { 0 } else { tons.div_ceil(20) };
    let mut flooded = false;
    // Hexes left to skid: terrain shortens a skid but not a sideslip, which never touches the
    // ground (Total Warfare pp. 62, 67).
    let mut left = i32::from(hexes);
    for (hex, _) in path(point, heading, hexes)? {
        let vehicle = &world.btech.vehicles()[&id];
        if vehicle.is_destroyed() || left <= 0 {
            break;
        }
        let Ok(next) =
            world.btech.maps()[&position.map].base_hex(i64::from(hex.x), i64::from(hex.y))
        else {
            break;
        };
        let here = vehicle.position().unwrap();
        let current =
            world.btech.maps()[&position.map].base_hex(i64::from(here.x), i64::from(here.y))?;
        let height = vehicle.elevation_level(current);
        match obstacle(
            world,
            id,
            position.map,
            hex,
            next,
            height,
            ground,
            surface_rider,
        )? {
            Some(Obstacle::Unit(other)) => {
                // An unintentional charge (Total Warfare p. 63): a hit ends the slide with
                // one more hex of damage, a miss carries the vehicle on through the hex.
                let Some(hit) =
                    vehicle_charge(world, id, other, hexes_moved, rules, character, &mut report)?
                else {
                    break;
                };
                if hit {
                    if per_hex > 0 {
                        vehicle_damage(world, id, per_hex, rules, character, &mut report)?;
                    }
                    break;
                }
            }
            Some(Obstacle::Building) => {
                // A vehicle sliding into a building takes a tenth of its construction factor
                // and stops there (Total Warfare p. 63).
                let cf = next.structure().map_or(0, |structure| structure.cf);
                move_vehicle(world, id, hex, next);
                report.notices.push(Notice {
                    unit: id,
                    text: format!("You skid into the building at {},{}!", hex.x, hex.y),
                });
                if per_hex > 0 {
                    vehicle_damage(world, id, per_hex, rules, character, &mut report)?;
                }
                vehicle_damage(
                    world,
                    id,
                    u32::from(super::building_interior::absorption(cf)),
                    rules,
                    character,
                    &mut report,
                )?;
                break;
            }
            Some(Obstacle::Collision) => {
                report.notices.push(Notice {
                    unit: id,
                    text: "You slam into the rising ground!".into(),
                });
                vehicle_damage(world, id, tons.div_ceil(20), rules, character, &mut report)?;
                break;
            }
            Some(Obstacle::Prohibited) => {
                // Terrain the drive could never enter: a point per five tons, and the vehicle
                // is immobile there for the rest of the game (Total Warfare p. 63).
                move_vehicle(world, id, hex, next);
                report.notices.push(Notice {
                    unit: id,
                    text: format!(
                        "You crash into the {}!",
                        next.terrain().label().to_ascii_lowercase()
                    ),
                });
                vehicle_damage(world, id, tons.div_ceil(5), rules, character, &mut report)?;
                let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
                if !vehicle.is_destroyed() && !vehicle.immobilized() {
                    vehicle.apply_motive_hit(super::VehicleMotiveHit::Immobilize);
                    report.notices.push(Notice {
                        unit: id,
                        text: "[fg=red bold]Your vehicle is stuck fast and will not move again![reset]"
                            .into(),
                    });
                }
                break;
            }
            None => {}
        }
        move_vehicle(world, id, hex, next);
        report.notices.push(Notice {
            unit: id,
            text: format!(
                "You {} into {},{}.",
                if hover { "drift" } else { "skid" },
                hex.x,
                hex.y
            ),
        });
        if next.prohibition(ground) == Some(super::Prohibition::Water)
            && !world.btech.vehicles()[&id]
                .definition()
                .has_special("Waterproof_Tech")
        {
            report
                .notices
                .push(super::vehicle_water::flood(world, id, character)?);
            flooded = true;
            break;
        }
        if per_hex > 0 {
            vehicle_damage(world, id, per_hex, rules, character, &mut report)?;
        }
        if !hover && next.water_depth() >= 1 {
            report.notices.push(Notice {
                unit: id,
                text: "The water brings your skid to an end.".into(),
            });
            break;
        }
        // A drop deeper than the one level a vehicle may change is a fall; a hovercraft
        // sideslipping off it falls too (Total Warfare pp. 62-63, 67).
        let drop = height - resting_height(next, surface_rider);
        if drop > 1 {
            report.notices.push(Notice {
                unit: id,
                text: "You skid over the edge and tumble down!".into(),
            });
            // The fall multiplier is the levels fallen plus one (Total Warfare p. 68).
            let levels = u8::try_from(drop + 1).unwrap_or(u8::MAX);
            let fall =
                super::vehicle_fall::resolve_in_candidate(world, id, levels, rules, character)?;
            super::piloting::append_feedback(
                &mut report.pilot_notices,
                fall.feedback.pilot_notices.iter().cloned(),
                report.notices.len(),
            );
            report.notices.extend(fall.feedback.notices.iter().cloned());
            report.vehicle_falls.push(fall);
            break;
        }
        left -= if hover {
            1
        } else {
            i32::from(skid_cost(next, ground))
        };
    }
    if !hover && !flooded && !world.btech.vehicles()[&id].is_destroyed() {
        // A skidding vehicle also rolls once for motive system damage.
        let (motive_notices, broadcasts) = motive_roll(world, id)?;
        report.notices.extend(motive_notices);
        for text in broadcasts {
            report
                .notices
                .extend(super::broadcast::observer_notices(world, id, &text.text));
        }
    }
    let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
    vehicle.halt();
    Ok(report)
}

/// A skidding or sideslipping vehicle's unintentional charge into `target` (Total Warfare
/// p. 63, charge rules p. 148): the driver rolls against the driving target number with the +3
/// unintentional modifier, the target's movement and the woods and smoke over it, ignoring the
/// skill difference. A hit deals the vehicle's charge damage (tons/10 per hex moved before the
/// skid) in five-point groups, the vehicle takes a point per ten tons of its target, and a
/// 'Mech it hit rolls at +2 to stay standing. `None` means the target could not be charged and
/// the slide stops short of it; otherwise whether the charge hit.
fn vehicle_charge(
    world: &mut World,
    id: ObjectId,
    target: ObjectId,
    hexes_moved: u32,
    rules: FallRules,
    character: bool,
    report: &mut SkidReport,
) -> Result<Option<bool>> {
    if world
        .objects
        .get(&target)
        .is_none_or(|object| object.flags.contains(Flag::Going))
        || (!character && world.objects[&target].flags.contains(Flag::InCharacter))
    {
        return Ok(None);
    }
    let Some(victim) = world.btech.unit(target) else {
        return Ok(None);
    };
    let mech = matches!(victim, super::UnitRef::Mech(_));
    let (chargeable, victim_tons, victim_heading, victim_position) =
        super::with_unit!(victim, |unit| {
            (
                !unit.is_destroyed() && unit.free_fall().is_none(),
                unit.definition().tons,
                unit.motion().map_or(0.0, |motion| motion.heading),
                unit.position(),
            )
        });
    let airborne = match victim {
        super::UnitRef::Mech(unit) => unit.airborne(),
        super::UnitRef::Vehicle(unit) => unit
            .vtol_flight()
            .is_some_and(|flight| flight.phase != super::VtolFlightPhase::Landed),
    };
    let prone = match victim {
        super::UnitRef::Mech(unit) => unit.posture() == super::Posture::Prone,
        super::UnitRef::Vehicle(_) => false,
    };
    let Some(victim_position) = victim_position.filter(|_| chargeable && !airborne) else {
        return Ok(None);
    };
    let tons = world.btech.vehicles()[&id].definition().tons;
    let cover = world
        .btech
        .maps()
        .get(&victim_position.map)
        .context("Map not found")?
        .hex(i64::from(victim_position.x), i64::from(victim_position.y))?;
    let range = super::geometry::unit_range(world, id, target)?;
    let target_number = i32::from(super::skills::unit_piloting_target(
        world,
        id,
        rules.extended_piloting,
    )?) + super::charge::UNINTENTIONAL_CHARGE_MODIFIER
        + i32::from(super::aim::unit_target_movement_modifier(
            world,
            target,
            range.horizontal,
        )?)
        + i32::from(cover.woods_density() + cover.smoke_density());
    let roll = world
        .btech
        .vehicles
        .get_mut(&id)
        .unwrap()
        .dice
        .generic_roll();
    let hit = i32::from(roll) >= target_number;
    report.notices.push(Notice {
        unit: id,
        text: format!("Charge: BTH {target_number}\tRoll: {roll}"),
    });
    if !hit {
        report.notices.push(Notice {
            unit: id,
            text: "You slide past your target!".into(),
        });
        return Ok(Some(false));
    }
    report.notices.extend(super::broadcast::interaction_notices(
        world,
        id,
        target,
        "slides into",
    ));
    report.notices.push(Notice {
        unit: id,
        text: "SMASH!!! You crash into your target!".into(),
    });
    report.notices.push(Notice {
        unit: target,
        text: format!("CRASH!!!\n#{} slides into you!", id.0),
    });
    let reverse = super::geometry::unit_range(world, target, id)?;
    let arc = HitArc::from_bearing(reverse.bearing.unwrap_or(180.0), victim_heading)?;
    let table = if mech && !prone {
        HitTable::Kick
    } else {
        HitTable::Weapon
    };
    let mut remaining = super::charge::charge_damage(tons, hexes_moved)?;
    while remaining > 0 {
        if super::with_unit!(world.btech.unit(target).unwrap(), |unit| unit
            .is_destroyed())
        {
            break;
        }
        let group = remaining.min(5);
        remaining -= group;
        let (impact, notices) = super::blast_damage::resolve_packet(
            world,
            target,
            super::blast_damage::MaterialPacket {
                amount: group,
                table,
                arc,
                character,
                attacker: Some(id),
                class: super::DamageClass::Impact,
            },
            arc == HitArc::Rear,
            rules,
        )?;
        impact.append_feedback(&mut report.pilot_notices, report.notices.len());
        report.notices.extend(notices);
        if let super::blast_damage::BlastImpact::Vehicle(impact) = &impact
            && let Some(damage) = &impact.damage
        {
            super::vehicle_injuries::collect_armor(damage, &mut report.character_injuries);
        }
    }
    vehicle_damage(
        world,
        id,
        u32::from(victim_tons.div_ceil(10)),
        rules,
        character,
        report,
    )?;
    if !mech
        || prone
        || super::with_unit!(world.btech.unit(target).unwrap(), |unit| unit
            .is_destroyed())
    {
        return Ok(Some(true));
    }
    // A charged 'Mech rolls at +2 to keep its feet (Total Warfare p. 148).
    let check = super::roll_piloting(world, target, 2, rules.extended_piloting)?;
    super::piloting::capture_feedback(
        target,
        world.btech.constructed_units()[&target].pilot(),
        &check,
        &mut report.notices,
        &mut report.pilot_notices,
    );
    if check.success {
        return Ok(Some(true));
    }
    report.notices.push(Notice {
        unit: target,
        text: "Your piloting skill fails and you fall over!!".into(),
    });
    report.notices.extend(super::broadcast::observer_notices(
        world,
        target,
        "falls down!",
    ));
    let fall = if character && world.objects[&target].flags.contains(Flag::InCharacter) {
        super::fall::resolve_character_fall(world, target, 1, rules)?
    } else {
        super::resolve_fall(world, target, 1, rules)?
    };
    fall.append_notices(target, &mut report.notices, &mut report.pilot_notices);
    report.falls.push(fall);
    Ok(Some(true))
}

/// Put a sliding vehicle in `hex`, resting on its surface.
fn move_vehicle(world: &mut World, id: ObjectId, hex: HexCoordinate, tile: Hex) {
    let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
    let position = vehicle.position().unwrap();
    let mut motion = vehicle.motion().unwrap();
    motion.point = hex.center();
    let under_bridge = false;
    vehicle.update_motion(
        motion,
        super::Position {
            map: position.map,
            x: hex.x as u16,
            y: hex.y as u16,
        },
        under_bridge,
    );
    let _ = tile;
}

/// Apply `amount` points of sliding damage to a vehicle from the front, in five-point groups.
fn vehicle_damage(
    world: &mut World,
    id: ObjectId,
    amount: u32,
    rules: FallRules,
    character: bool,
    report: &mut SkidReport,
) -> Result<()> {
    let mut remaining = amount;
    while remaining > 0 {
        if world.btech.vehicles()[&id].is_destroyed() {
            break;
        }
        let group = remaining.min(5);
        remaining -= group;
        let (impact, notices) = super::blast_damage::resolve_packet(
            world,
            id,
            super::blast_damage::MaterialPacket {
                amount: group as u16,
                table: HitTable::Weapon,
                arc: HitArc::Front,
                character,
                attacker: None,
                class: super::DamageClass::Ordinary,
            },
            false,
            rules,
        )?;
        impact.append_feedback(&mut report.pilot_notices, report.notices.len());
        report.notices.extend(notices);
        if let super::blast_damage::BlastImpact::Vehicle(impact) = &impact
            && let Some(damage) = &impact.damage
        {
            super::vehicle_injuries::collect_armor(damage, &mut report.character_injuries);
        }
    }
    Ok(())
}

/// One Motive System Damage Table roll for a skidding ground vehicle (Total Warfare p. 63);
/// an immobilized hovercraft over water then sinks (p. 193).
fn motive_roll(world: &mut World, id: ObjectId) -> Result<(Vec<Notice>, Vec<Notice>)> {
    let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
    let movement = vehicle.definition().movement;
    let roll = vehicle.dice.generic_roll();
    let (motive, penalty) = super::vehicle_motive_effects::outcome(movement, roll);
    let (mut notices, mut broadcasts) =
        super::vehicle_motive_effects::apply(vehicle, id, motive, penalty, Some(roll));
    let (sank, observed) = super::vehicle_motive_effects::sink_immobilized_hovercraft(world, id)?;
    notices.extend(sank);
    broadcasts.extend(observed);
    Ok((notices, broadcasts))
}

/// How the slick surface under a unit is named in its notices.
fn surface_name(tile: Hex) -> &'static str {
    if tile.is_icy() {
        "ice"
    } else if tile.has_bridge() {
        "bridge deck"
    } else {
        "pavement"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::Terrain;

    /// The skid modifier table reads hexes moved from speed in movement points.
    #[test]
    fn skid_modifier_and_distance_follow_hexes_moved() {
        for (mp, modifier_expected, hexes) in [
            (0.0, -1, 1),
            (2.0, -1, 1),
            (2.1, 0, 2),
            (4.0, 0, 2),
            (4.1, 1, 3),
            (7.0, 1, 4),
            (7.1, 2, 4),
            (10.0, 2, 5),
            (10.1, 4, 6),
            (17.0, 4, 9),
            (17.1, 5, 9),
            (24.0, 5, 12),
            (25.0, 6, 13),
        ] {
            assert_eq!(modifier(mp * 10.75), modifier_expected, "{mp}");
            assert_eq!(distance(mp * 10.75), hexes, "{mp}");
            assert_eq!(modifier(-mp * 10.75), modifier_expected, "{mp}");
        }
    }

    /// Ice skids at any speed; pavement only above walking or cruising speed (Total Warfare
    /// pp. 60, 62). A 6/9 'Mech walks at 64.5 kph: turning at that speed on pavement is safe,
    /// and any faster is running and checks.
    #[test]
    fn slick_surfaces_risk_skids_by_speed() {
        let pavement = Hex::new(Terrain::Pavement, 0);
        let ice = Hex::new(Terrain::Ice, 1);
        let clear = Hex::new(Terrain::Clear, 0);
        let walking = 6.0 * 10.75;
        assert!(!risks_skid(pavement, 4.5 * 10.75, walking));
        assert!(!risks_skid(pavement, walking, walking));
        assert!(!risks_skid(pavement, -walking, walking));
        assert!(risks_skid(pavement, 6.5 * 10.75, walking));
        assert!(risks_skid(pavement, 9.0 * 10.75, walking));
        assert!(risks_skid(pavement, -6.5 * 10.75, walking));
        assert!(risks_skid(ice, 1.0, walking));
        assert!(!risks_skid(clear, 9.0 * 10.75, walking));
    }

    /// A hovercraft above cruising speed risks a sideslip on any ground, and never at or below
    /// cruising speed.
    #[test]
    fn hovercraft_sideslip_at_flank_speed_regardless_of_terrain() {
        let clear = Hex::new(Terrain::Clear, 0);
        let ice = Hex::new(Terrain::Ice, 1);
        assert!(risks_slide(clear, 61.0, 60.0, true));
        assert!(!risks_slide(clear, 60.0, 60.0, true));
        assert!(!risks_slide(ice, 30.0, 60.0, true));
        assert!(!risks_slide(clear, 90.0, 60.0, false));
        assert!(risks_slide(ice, 1.0, 60.0, false));
    }

    /// Tracking starts on a slick hex, notes a turn there and forgets elsewhere.
    #[test]
    fn slick_tracking_notes_turns_on_the_surface() {
        let mut track = SlickTrack::default();
        track.observe(true, true, 90.0);
        assert_eq!(track.heading, Some(90.0));
        assert!(!track.turned);
        track.observe(true, true, 90.3);
        assert!(!track.turned);
        track.observe(true, true, 120.0);
        assert!(track.turned);
        track.validate().unwrap();
        track.observe(false, true, 120.0);
        assert_eq!(track, SlickTrack::default());
        track.observe(true, true, 359.0);
        track.observe(true, false, 10.0);
        assert_eq!(track, SlickTrack::default());
        assert!(
            SlickTrack {
                heading: None,
                turned: true,
            }
            .validate()
            .is_err()
        );
    }
}
