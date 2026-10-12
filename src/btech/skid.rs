//! Skidding on pavement and ice, and hovercraft sideslips (Total Warfare pp. 62 and 67,
//! Tactical Operations p. 48). A 'Mech or ground vehicle that changes facing on a slick surface
//! and then moves on rolls to keep its footing; when it fails it drops and slides along its old
//! course, taking half a fall's damage for every hex it slides. A hovercraft that fails instead
//! sideslips: it drifts along its old course for as many hexes as it missed the roll by, without
//! damage unless it drifts into something.
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

/// Whether moving off `tile` after turning on it risks a skid: any speed on ice, and running
/// or flank speed, above two-thirds of `maximum`, on pavement, a road or a bridge deck.
pub(super) fn risks_skid(tile: Hex, speed: f64, maximum: f64) -> bool {
    tile.is_slick() && (tile.is_icy() || speed.abs() > maximum * 2.0 / 3.0 + 0.1)
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
    /// Whether the unit lost control and its movement is over.
    pub skidded: bool,
}

/// What stops a sliding unit.
enum Obstacle {
    /// Higher ground, another unit or the map edge: the slide ends before the hex.
    Stop,
    /// Higher ground the unit crashes into for a point of damage per twenty tons.
    Collision,
    /// Terrain the unit could never enter: a point of damage per five tons, then it halts.
    Prohibited,
}

/// The hexes a unit sliding `hexes` cells along `heading` from `point` would pass through.
fn path(point: Point, heading: f64, hexes: u8) -> Result<Vec<(HexCoordinate, Point)>> {
    let end = point.project(heading, f64::from(hexes))?;
    let mut cells = point.trace_positions(end)?;
    cells.remove(0);
    Ok(cells)
}

/// Whether any other unit stands in `hex`.
fn occupied(world: &World, id: ObjectId, map: ObjectId, hex: HexCoordinate) -> Result<bool> {
    Ok(super::map_slots::hex_occupants(world, map, hex)?
        .into_iter()
        .any(|other| {
            other != id
                && world
                    .objects
                    .get(&other)
                    .is_some_and(|object| !object.flags.contains(Flag::Going))
        }))
}

/// Classify the next hex of a slide for a unit at `height` that can step down `allowed` levels.
fn obstacle(
    world: &World,
    id: ObjectId,
    map: ObjectId,
    hex: HexCoordinate,
    tile: Hex,
    height: i32,
    movement: GroundMovement,
) -> Result<Option<Obstacle>> {
    if occupied(world, id, map, hex)? {
        return Ok(Some(Obstacle::Stop));
    }
    let surface = if movement == GroundMovement::Hover && tile.is_water_surface() {
        i32::from(tile.water_line())
    } else {
        i32::from(tile.standing_height())
    };
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
    let point = world.btech.constructed_units()[&id]
        .motion()
        .context("Skidding unit has no motion")?
        .point;
    for (hex, _) in path(point, heading, distance(speed))? {
        let unit = &world.btech.constructed_units()[&id];
        if unit.is_destroyed() {
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
        )? {
            Some(Obstacle::Stop) => break,
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
        move_mech(world, id, hex, next);
        report.notices.push(Notice {
            unit: id,
            text: format!("You skid into {},{}.", hex.x, hex.y),
        });
        slide_damage(world, id, per_hex, &mut fall, rules, character)?;
        let drop = height - i32::from(next.standing_height());
        if drop > 2 {
            report.notices.push(Notice {
                unit: id,
                text: "You skid over the edge and tumble down!".into(),
            });
            let levels = u8::try_from(drop).unwrap_or(u8::MAX);
            let tumble = if character && world.objects[&id].flags.contains(Flag::InCharacter) {
                super::fall::resolve_character_fall(world, id, levels, rules)?
            } else {
                super::resolve_fall(world, id, levels, rules)?
            };
            tumble.append_notices(id, &mut report.notices, &mut report.pilot_notices);
            report.falls.push(tumble);
            break;
        }
        let flooding = if character {
            super::flooding::flood_unit_in_action(world, id, rules)?
        } else {
            super::flood_unit(world, id, rules)?
        };
        for exposure in flooding {
            report.notices.extend(exposure.notices);
        }
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
        let hit = rules.hit.resolve(unit, fall.arc, roll, &mut dice)?;
        world.btech.constructed.get_mut(&id).unwrap().dice = dice;
        let outcome = if character && world.objects[&id].flags.contains(Flag::InCharacter) {
            super::impact::resolve_character_impact_with_rules(world, id, hit, group, Some(rules))?
        } else {
            super::pilot_injury::resolve_tactical_impact_in_candidate(
                world, id, hit, group, rules, None,
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
    let (movement, position, tons, point) = {
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
        )
    };
    let hover = movement == VehicleMovement::Hover;
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
    for (hex, _) in path(point, heading, hexes)? {
        let vehicle = &world.btech.vehicles()[&id];
        if vehicle.is_destroyed() {
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
        match obstacle(world, id, position.map, hex, next, height, ground)? {
            Some(Obstacle::Stop) => break,
            Some(Obstacle::Collision) => {
                report.notices.push(Notice {
                    unit: id,
                    text: "You slam into the rising ground!".into(),
                });
                vehicle_damage(world, id, tons.div_ceil(20), rules, character, &mut report)?;
                break;
            }
            Some(Obstacle::Prohibited) => {
                move_vehicle(world, id, hex, next);
                report.notices.push(Notice {
                    unit: id,
                    text: format!(
                        "You crash into the {}!",
                        next.terrain().label().to_ascii_lowercase()
                    ),
                });
                vehicle_damage(world, id, tons.div_ceil(5), rules, character, &mut report)?;
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
        let drop = height - i32::from(next.standing_height());
        if !hover && drop > 1 {
            report.notices.push(Notice {
                unit: id,
                text: "You skid over the edge and tumble down!".into(),
            });
            let levels = u8::try_from(drop).unwrap_or(u8::MAX);
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
    }
    if !hover && !flooded && !world.btech.vehicles()[&id].is_destroyed() {
        // A skidding vehicle also rolls once for motive system damage.
        let (motive_notices, broadcasts) = motive_roll(world, id);
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

/// A vehicle's driving roll to step onto ice (Tactical Operations p. 48). Failure is a loss of
/// control: the vehicle halts where it is and rolls once for motive system damage.
pub(super) fn vehicle_entry_check(
    world: &mut World,
    id: ObjectId,
    tile: Hex,
    modifier: i16,
    rules: FallRules,
    character: bool,
) -> Result<SkidReport> {
    let mut report = SkidReport::default();
    report.notices.push(Notice {
        unit: id,
        text: format!("You ease onto the {}.", surface_name(tile)),
    });
    let control = super::terrain_control::check_entry(
        world,
        id,
        modifier,
        rules.extended_piloting,
        character,
    )?;
    control.capture_feedback(id, &mut report.notices, &mut report.pilot_notices);
    report
        .experience_messages
        .extend(control.experience_messages);
    if control.success {
        return Ok(report);
    }
    report.skidded = true;
    report.notices.push(Notice {
        unit: id,
        text: "Your drive loses its grip and you slew to a halt!".into(),
    });
    report.notices.extend(super::broadcast::observer_notices(
        world,
        id,
        "slews to a halt on the ice!",
    ));
    let (motive_notices, broadcasts) = motive_roll(world, id);
    report.notices.extend(motive_notices);
    for text in broadcasts {
        report
            .notices
            .extend(super::broadcast::observer_notices(world, id, &text.text));
    }
    world.btech.vehicles.get_mut(&id).unwrap().halt();
    Ok(report)
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

/// One motive system damage roll for a skidding ground vehicle.
fn motive_roll(world: &mut World, id: ObjectId) -> (Vec<Notice>, Vec<Notice>) {
    let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
    let movement = vehicle.definition().movement;
    let roll = vehicle
        .dice
        .generic_roll()
        .saturating_add(super::vehicle_motive_effects::modifier(movement));
    let (motive, penalty) = super::vehicle_motive_effects::outcome(movement, roll);
    super::vehicle_motive_effects::apply(vehicle, id, motive, penalty, Some(roll))
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

    /// Ice skids at any speed; pavement only above walking or cruising speed.
    #[test]
    fn slick_surfaces_risk_skids_by_speed() {
        let pavement = Hex::new(Terrain::Pavement, 0);
        let ice = Hex::new(Terrain::Ice, 1);
        let clear = Hex::new(Terrain::Clear, 0);
        assert!(!risks_skid(pavement, 60.0, 90.0));
        assert!(risks_skid(pavement, 61.0, 90.0));
        assert!(risks_skid(pavement, -61.0, 90.0));
        assert!(risks_skid(ice, 1.0, 90.0));
        assert!(!risks_skid(clear, 90.0, 90.0));
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
