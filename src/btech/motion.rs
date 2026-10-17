//! Mech ground motion and independent jump facing; unsupported hazards stop before entry.
use super::{Notice, Point, Power};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Durable sub-hex position and commanded versus actual motion.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Motion {
    pub point: Point,
    pub heading: f64,
    pub desired_heading: f64,
    pub speed: f64,
    pub desired_speed: f64,
}

impl Motion {
    /// A stationary unit facing north at the supplied position.
    pub fn stationary(point: Point) -> Self {
        Self {
            point,
            heading: 0.0,
            desired_heading: 0.0,
            speed: 0.0,
            desired_speed: 0.0,
        }
    }

    /// Project one second of movement, preserving reverse travel. One movement point
    /// (10.75 speed units) covers one hex every 60 seconds.
    pub(super) fn project_step(self) -> Result<Point> {
        self.point.project(
            self.heading + if self.speed < 0.0 { 180.0 } else { 0.0 },
            self.speed.abs() / 645.0,
        )
    }

    /// Whether the unit needs another movement update.
    pub fn active(self) -> bool {
        self.speed != 0.0 || self.desired_speed != 0.0 || self.heading != self.desired_heading
    }

    /// Whether the unit is travelling or has been ordered to. A pending turn alone is
    /// not travel: a unit without power keeps its heading order until it restarts.
    pub fn translating(self) -> bool {
        self.speed != 0.0 || self.desired_speed != 0.0
    }

    /// Stop horizontal travel without cancelling an independent heading command.
    pub(super) fn stop_translation(&mut self) {
        self.speed = 0.0;
        self.desired_speed = 0.0;
    }

    /// Advance heading toward the commanded bearing. Facing changes are continuous: a unit
    /// turns through as many degrees a second as it has running MP, half again as fast from
    /// a standstill, and more slowly while running hard through a wide turn. Total Warfare
    /// p. 50 charges 1 MP per hexside instead; this rate-limited turn with the throttle cut in
    /// `motion_controls::turning_throttle` is the realtime adaptation of that cost, and it is
    /// the same for bipeds and quads.
    pub(super) fn turn_toward(&mut self, maximum: f64, walking: f64) {
        if maximum <= 0.0 {
            return;
        }
        let difference = (self.desired_heading - self.heading + 180.0).rem_euclid(360.0) - 180.0;
        let base = if self.speed.abs() < 1.0 { 1.5 } else { 1.0 } * maximum / 10.75;
        let turn = if difference.abs() > base && self.speed > walking + 0.1 {
            base * (2.0 - 1.5 * self.speed / maximum)
        } else {
            base
        };
        self.turn_by(turn);
    }

    /// Rotate along the shortest arc without overshooting the commanded bearing.
    pub(super) fn turn_by(&mut self, turn: f64) {
        let difference = (self.desired_heading - self.heading + 180.0).rem_euclid(360.0) - 180.0;
        self.heading = if difference.abs() <= turn {
            self.desired_heading
        } else {
            (self.heading + difference.signum() * turn).rem_euclid(360.0)
        };
    }

    /// Approach a speed target without overshoot, tolerating accumulated floating-point error.
    pub(super) fn accelerate(&mut self, target: f64, acceleration: f64, maximum: f64) {
        self.speed = if (target - self.speed).abs() <= acceleration + maximum * f64::EPSILON * 4.0 {
            target
        } else {
            self.speed + (target - self.speed).signum() * acceleration
        };
    }

    /// Keep a Mech's controls inside its current envelope after its load or damage changed.
    pub(super) fn limit_to(&mut self, envelope: super::SpeedEnvelope) {
        self.desired_speed = envelope.clamp(self.desired_speed);
        self.speed = envelope.clamp(self.speed);
        if envelope.is_stopped() {
            self.desired_heading = self.heading;
        }
    }

    /// Reconcile a changed vehicle load: the throttle stays inside `envelope`, and actual speed
    /// inside `actual`, which keeps the extra point a ground vehicle gains on pavement.
    pub(super) fn limit_load(
        &mut self,
        envelope: super::SpeedEnvelope,
        actual: super::SpeedEnvelope,
    ) {
        self.desired_speed = envelope.clamp(self.desired_speed);
        self.speed = actual.clamp(self.speed);
        if envelope.is_stopped() {
            self.desired_heading = self.heading;
        }
    }

    /// Isolate self-propelled motion for chassis checks. Unpowered units can be moved
    /// externally; the world relationship validator authorizes that actual velocity.
    pub(super) fn propelled(self, power: Power) -> Result<Self> {
        ensure!(self.speed.is_finite(), "Invalid unit speed");
        if power != Power::Off {
            return Ok(self);
        }
        Ok(Self { speed: 0.0, ..self })
    }

    /// Validate persistent Mech or vehicle motion against the widest envelope it may have
    /// retained: backward no faster than walking or cruising, forward no faster than running
    /// or flanking, since no unit moves backward at running or flank speed (TW p. 53).
    pub(crate) fn validate_within(self, envelope: super::SpeedEnvelope) -> Result<()> {
        self.point.range(self.point)?;
        ensure!(
            [self.heading, self.desired_heading]
                .iter()
                .all(|value| value.is_finite() && (0.0..360.0).contains(value)),
            "Invalid unit heading"
        );
        ensure!(
            [self.speed, self.desired_speed]
                .iter()
                .all(|value| value.is_finite()
                    && *value >= -envelope.walking - 1e-6
                    && *value <= envelope.running + 1e-6),
            "Invalid unit speed"
        );
        Ok(())
    }
}

/// Shared ground/jump movement configuration, including charge, terrain and fall rules.
#[derive(Debug, Clone, Copy)]
pub struct MovementRules {
    /// Enable hot-myomer assistance when calculating towing load.
    pub tsm_tow_bonus: bool,
    /// Damage and pilot protection rules for failed movement checks.
    pub fall: super::FallRules,
}

impl MovementRules {
    /// Physical landing policy shares movement, hit arcs and fall settings with the tick.
    pub(super) fn landing_physical(self) -> super::PhysicalRules {
        super::PhysicalRules { fall: self.fall }
    }

    /// Conventional movement and fall settings, suitable for an isolated simulation.
    pub const STANDARD: Self = Self {
        tsm_tow_bonus: true,
        fall: super::FallRules {
            vehicle_impact: crate::VehicleImpactRules::STANDARD,
            extended_piloting: true,
            toughness: false,
        },
    };
}

/// The levels a ground unit climbed or descended to enter the hex it stands in. Each level
/// costs a Mech one MP and a ground vehicle two (Total Warfare pp. 48 and 52), charged like
/// the hex's own terrain cost: the unit crosses the hex that many hex-widths slower (see
/// `ground_proposal`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LevelEntry {
    pub x: u16,
    pub y: u16,
    pub levels: u8,
}

impl LevelEntry {
    /// The extra movement cost this record adds while the unit stands at `position`.
    pub(super) fn cost_at(self, position: super::Position) -> u8 {
        if self.x == position.x && self.y == position.y {
            self.levels
        } else {
            0
        }
    }
}

impl super::Mech {
    /// Levels climbed or descended into the current hex, still being paid for.
    pub fn level_entry_cost(&self) -> u8 {
        match (self.level_entry, self.position()) {
            (Some(entry), Some(position)) => entry.cost_at(position),
            _ => 0,
        }
    }
}

/// A checked crossing carries both map elevation and the selected physical bridge surface.
struct GroundStep {
    hex: super::HexCoordinate,
    change: i16,
    old_position: super::Position,
    old_point: Point,
    elevation: Option<f64>,
    old_elevation: Option<f64>,
    /// Levels climbed or descended to enter this hex, charged while crossing it.
    levels: u8,
    ice_check: bool,
    /// Why the unit must stop at this hex's edge instead of entering it: terrain it cannot
    /// enter, a slope too steep to climb or descend, or water it is running at.
    stop: Option<String>,
}

/// Set desired speed in kilometers per hour with standard enabled myomer towing assistance.
pub fn set_speed(world: &mut World, id: ObjectId, pilot: ObjectId, speed: f64) -> Result<Notice> {
    let notice = set_speed_by_actor(
        world,
        id,
        super::combat_operator::ControlActor::Player(pilot),
        speed,
        super::SpeedPolicy::STANDARD,
    )?;
    let _ = super::autopilot::manual_takeover(world, id);
    Ok(notice)
}

/// Set speed from the attached autopilot while retaining all ordinary motion checks.
pub(crate) fn set_speed_autopilot(world: &mut World, id: ObjectId, speed: f64) -> Result<Notice> {
    set_speed_by_actor(
        world,
        id,
        super::combat_operator::ControlActor::Autopilot,
        speed,
        super::SpeedPolicy::STANDARD,
    )
}

fn set_speed_by_actor(
    world: &mut World,
    id: ObjectId,
    actor: super::combat_operator::ControlActor,
    speed: f64,
    policy: super::SpeedPolicy,
) -> Result<Notice> {
    if world.btech.vehicles().contains_key(&id) {
        return super::vehicle_driving::set_control_by_actor(
            world,
            id,
            actor,
            Some(speed),
            None,
            policy,
        );
    }
    super::power::controlled_unit_by_actor(world, id, actor)?;
    super::fortification::require_mobile(world, id)?;
    super::bog_down::require_free(world, id)?;
    ensure!(
        !world.btech.constructed_units()[&id].airborne(),
        "Land before changing ground speed"
    );
    let unit = &world.btech.constructed_units()[&id];
    ensure!(unit.power() == Power::Running, "Start the unit first");
    unit.hull_down.require_mobile()?;
    ensure!(
        !matches!(unit.stand_timer(), Some(super::StandTimer::Rising { .. })),
        "Unit is still standing up"
    );
    ensure!(
        unit.posture() != super::Posture::Prone,
        "Stand the unit up first"
    );
    // Every "may not run" gate compares against the current walking ceiling, the
    // template's walking MP after damage, heat and load (Total Warfare p. 53).
    let envelope = super::motion_controls::throttle_envelope(world, id, policy)?;
    // An autopilot steers in fractions of the running ceiling; its reverse requests are
    // held to the walking ceiling rather than refused.
    let speed = if matches!(actor, super::combat_operator::ControlActor::Autopilot) {
        envelope.clamp(speed)
    } else {
        speed
    };
    let running = envelope.is_running(speed);
    ensure!(
        unit.dumping().is_none() || !running,
        "You can not run while dumping ammo!"
    );
    ensure!(
        unit.unjam().is_none() || !running,
        "You can not run while unjamming your weapon!"
    );
    ensure!(
        speed.is_finite() && speed >= -envelope.walking && speed <= envelope.running,
        "Speed exceeds unit limits"
    );
    super::motion_controls::require_reverse_allowed(world, id, speed)?;
    if speed == 0.0 && matches!(actor, super::combat_operator::ControlActor::Player(_)) {
        super::stacking::require_stop_allowed(world, id)?;
    }
    ensure!(
        unit.stun_remaining() == 0 || !running,
        "You cannot move faster than cruise speed while stunned!"
    );
    let mut motion = unit.motion().context("Unit is not placed")?;
    let position = unit.position().context("Unit is not placed")?;
    let tile =
        world.btech.maps()[&position.map].base_hex(i64::from(position.x), i64::from(position.y))?;
    let wet = tile.immerses(unit.elevation_level(tile));
    ensure!(!wet || !running, "You can't run through water!");
    motion.desired_speed = speed;
    world.btech.constructed.get_mut(&id).unwrap().motion = Some(motion);
    Ok(Notice {
        unit: id,
        text: super::motion_controls::speed_confirmation(speed),
    })
}

/// Set speed using the host's towing assistance and fusion-aircraft fuel policies.
pub(crate) fn set_speed_configured(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    speed: f64,
    policy: super::SpeedPolicy,
) -> Result<Notice> {
    let notice = set_speed_by_actor(
        world,
        id,
        super::combat_operator::ControlActor::Player(pilot),
        speed,
        policy,
    )?;
    let _ = super::autopilot::manual_takeover(world, id);
    Ok(notice)
}

/// Set a desired compass heading, including jump facing, prone pivots and stand countdowns.
pub fn set_heading(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    heading: f64,
) -> Result<Notice> {
    let notice = set_heading_by_actor(
        world,
        id,
        super::combat_operator::ControlActor::Player(pilot),
        heading,
    )?;
    let _ = super::autopilot::manual_takeover(world, id);
    Ok(notice)
}

/// Set heading from the attached autopilot while retaining ordinary turning gates.
pub(crate) fn set_heading_autopilot(
    world: &mut World,
    id: ObjectId,
    heading: f64,
) -> Result<Notice> {
    set_heading_by_actor(
        world,
        id,
        super::combat_operator::ControlActor::Autopilot,
        heading,
    )
}

fn set_heading_by_actor(
    world: &mut World,
    id: ObjectId,
    actor: super::combat_operator::ControlActor,
    heading: f64,
) -> Result<Notice> {
    if world.btech.vehicles().contains_key(&id) {
        return super::vehicle_driving::set_control_by_actor(
            world,
            id,
            actor,
            None,
            Some(heading),
            super::SpeedPolicy::STANDARD,
        );
    }
    super::power::controlled_unit_by_actor(world, id, actor)?;
    super::fortification::require_mobile(world, id)?;
    super::bog_down::require_free(world, id)?;
    let unit = &world.btech.constructed_units()[&id];
    ensure!(unit.power() == Power::Running, "Start the unit first");
    unit.hull_down.require_mobile()?;
    ensure!(
        unit.flight().is_some() || unit.mobility().maximum_speed > 0.0,
        "Unit cannot turn with this damage"
    );
    ensure!(heading.is_finite(), "Invalid heading");
    let mut motion = unit.motion().context("Unit is not placed")?;
    motion.desired_heading = heading.rem_euclid(360.0);
    world.btech.constructed.get_mut(&id).unwrap().motion = Some(motion);
    Ok(Notice {
        unit: id,
        text: format!("Desired heading: {:.1} degrees.", motion.desired_heading),
    })
}

/// Traverse one committed second on supported ground, resolving reverse steps and falls atomically.
/// Errors leave every unit and random stream unchanged; callers stage returned notices after saving.
pub fn advance_motion(world: &mut World, rules: MovementRules) -> Result<Vec<Notice>> {
    Ok(advance_motion_candidate(world, rules, false)?.notices)
}

/// Ground updates whose host can publish character injury and crew movement.
pub(super) fn advance_motion_in_action(
    world: &mut World,
    rules: MovementRules,
) -> Result<super::movement_report::MovementReport> {
    advance_motion_candidate(world, rules, true)
}

/// Validate the complete ground update before publishing the candidate.
fn advance_motion_candidate(
    world: &mut World,
    rules: MovementRules,
    character: bool,
) -> Result<super::movement_report::MovementReport> {
    let mut candidate = world.clone();
    let mut report = advance_motion_inner(&mut candidate, rules, character)?;
    // Recovered aircraft already used powered turning; only still-falling units turn here.
    report
        .notices
        .extend(advance_fall_headings(&mut candidate, rules)?);
    super::towing::synchronize(&mut candidate)?;
    report
        .notices
        .extend(super::hiding::movement_changes(&mut candidate, world));
    for unit in candidate.btech.constructed_units().values() {
        unit.validate()?;
    }
    *world = candidate;
    Ok(report)
}

/// Powered facing remains independent of the shared forced-descent cursor.
fn advance_fall_headings(world: &mut World, rules: MovementRules) -> Result<Vec<Notice>> {
    let mut notices = Vec::new();
    let ids: Vec<_> = world
        .btech
        .constructed_units()
        .keys()
        .chain(world.btech.vehicles().keys())
        .copied()
        .collect();
    for id in ids {
        if world
            .objects
            .get(&id)
            .is_none_or(|object| object.flags.contains(Flag::Going))
        {
            continue;
        }
        let (fall, power, destroyed, motion) =
            crate::btech::with_unit!(world.btech.unit(id).unwrap(), |unit| {
                (
                    unit.free_fall(),
                    unit.power(),
                    unit.is_destroyed(),
                    unit.motion(),
                )
            });
        if fall.is_none() || power != Power::Running || destroyed {
            continue;
        }
        let Some(mut motion) = motion else {
            continue;
        };
        if motion.heading == motion.desired_heading {
            continue;
        }
        if let Some(flight) = world
            .btech
            .vehicles()
            .get(&id)
            .and_then(|unit| unit.vtol_flight())
        {
            let fuel = world
                .btech
                .vehicles
                .get_mut(&id)
                .unwrap()
                .consume_vtol_fuel(flight.vertical_speed, flight.altitude as i32, false)?;
            if let super::VtolFuelUse::Exhausted { newly } = fuel {
                if newly {
                    notices.push(Notice {
                        unit: id,
                        text: "You run out of fuel and begin to fall!".into(),
                    });
                }
                continue;
            }
        }
        let envelope = super::motion_controls::throttle_envelope(
            world,
            id,
            super::SpeedPolicy {
                tsm_tow_bonus: rules.tsm_tow_bonus,
            },
        )?;
        motion.turn_toward(envelope.running, envelope.walking);
        crate::btech::with_unit_mut!(world.btech.unit_mut(id).unwrap(), |unit| {
            unit.motion = Some(motion);
        })
    }
    Ok(notices)
}

/// Resolve the whole tick in the caller's disposable world candidate.
fn advance_motion_inner(
    world: &mut World,
    rules: MovementRules,
    character: bool,
) -> Result<super::movement_report::MovementReport> {
    let ids: Vec<_> = world
        .btech
        .constructed_units()
        .iter()
        .filter_map(|(&id, unit)| {
            (unit.power() == Power::Running
                && (!unit.airborne() || unit.flight().is_some())
                && unit.motion().is_some_and(Motion::active)
                && world
                    .objects
                    .get(&id)
                    .is_some_and(|object| !object.flags.contains(Flag::Going)))
            .then_some(id)
        })
        .collect();
    let mut boundaries = Vec::new();
    let mut notices = Vec::new();
    let mut pilot_notices = Vec::new();
    let mut falls = Vec::new();
    let mut vehicle_falls = Vec::new();
    let mut mines = Vec::new();
    let mut charges = Vec::new();
    let mut surface_breaks = Vec::new();
    let mut experience_messages = Vec::new();
    let collisions = super::stacking::StackingEffects::default();
    for id in ids {
        let unit = &world.btech.constructed_units()[&id];
        if unit.power() != Power::Running
            || (unit.airborne() && unit.flight().is_none())
            || !unit.motion().is_some_and(Motion::active)
        {
            continue;
        }
        let Some(position) = unit.position() else {
            continue;
        };
        let Some(mut motion) = unit.motion() else {
            continue;
        };
        let Some(map) = world.btech.maps().get(&position.map) else {
            continue;
        };
        if unit.flight().is_some() {
            // An airborne Mech turns three degrees a second per jumping MP, whatever its chassis.
            let turn = 3.0 * f64::from(unit.jump_capacity(map.gravity)?.movement_points);
            motion.turn_by(turn);
            world.btech.constructed.get_mut(&id).unwrap().motion = Some(motion);
            continue;
        }
        let proposal = super::propose_mech_ground_motion(
            world,
            id,
            motion,
            super::HexCoordinate {
                x: i32::from(position.x),
                y: i32::from(position.y),
            },
            rules,
        )?;
        motion = proposal.motion;
        if proposal.immobilized {
            let unit = world.btech.constructed.get_mut(&id).unwrap();
            unit.motion = Some(motion);
            unit.reconcile_damage();
            continue;
        }
        let proposed = proposal.destination;
        // A unit that turned on pavement or ice must keep its footing before it moves on.
        let current = map.base_hex(i64::from(position.x), i64::from(position.y))?;
        let walking_ceiling = world.btech.constructed_units()[&id].walking_speed();
        let normal_running = world.btech.constructed_units()[&id]
            .mobility()
            .maximum_speed;
        let leaving = proposed.containing_hex().ok()
            != Some(super::HexCoordinate {
                x: i32::from(position.x),
                y: i32::from(position.y),
            });
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        unit.motion = Some(motion);
        unit.turn_movement
            .observe(motion.speed, walking_ceiling, normal_running);
        unit.slick
            .observe(current.is_slick(), motion.speed != 0.0, motion.heading);
        let held = unit.slick.heading;
        if leaving
            && unit.slick.turned
            && super::skid::risks_skid(current, motion.speed, walking_ceiling)
        {
            unit.slick.reset();
            let skid = super::skid::resolve_mech(
                world,
                id,
                held.unwrap_or(motion.heading),
                motion.speed,
                rules.fall,
                character,
            )?;
            super::piloting::append_feedback(&mut pilot_notices, skid.pilot_notices, notices.len());
            notices.extend(skid.notices);
            experience_messages.extend(skid.experience_messages);
            vehicle_falls.extend(skid.vehicle_falls);
            charges.extend(skid.charges);
            surface_breaks.extend(skid.collapses);
            if character {
                falls.extend(skid.falls);
            }
            if skid.skidded {
                continue;
            }
            motion = world.btech.constructed_units()[&id].motion().unwrap();
        }
        notices.extend(super::charge_tracking::begin_update(world, id));
        super::charge_tracking::record_distance(world, id, motion.point, proposed);
        let segment = resolve_ground_segment(
            world,
            id,
            motion,
            (motion.point, proposed),
            rules,
            true,
            character.then_some(&mut falls),
        )?;
        boundaries.extend(segment.boundary);
        mines.extend(segment.mines);
        vehicle_falls.extend(segment.vehicle_falls);
        super::piloting::append_feedback(&mut pilot_notices, segment.pilot_notices, notices.len());
        notices.extend(segment.notices);
        experience_messages.extend(segment.experience_messages);
        let (notice, experience) = super::building_step::entered(world, id, position)?;
        notices.extend(notice);
        experience_messages.extend(experience);
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        if unit.position() != Some(position) {
            unit.slick.reset();
        }
        let unit = &world.btech.constructed_units()[&id];
        if character && unit.position() != Some(position) && !unit.hex_sync_pending() {
            experience_messages.extend(super::movement_experience::record_entry(
                world,
                id,
                rules.fall.extended_piloting,
            )?);
        }
        if motion.speed != 0.0 {
            notices.extend(super::charge_tracking::finish_update(
                world,
                id,
                rules,
                character.then_some(&mut charges),
                (&mut pilot_notices, notices.len()),
            )?);
        }
    }
    let aircraft = super::vtol_updates::advance_all(world, rules, character)?;
    let vehicles = super::vehicle_driving::advance(world, rules, character)?;
    let mut report = super::movement_report::MovementReport {
        pilot_notices,
        boundaries,
        character_injuries: Vec::new(),
        vehicle_falls,
        experience_messages,
        dfas: Vec::new(),
        mines,
        notices,
        falls,
        stacking: collisions,
        charges,
        surface_breaks,
    };
    report.extend(aircraft);
    report.extend(vehicles);
    Ok(report)
}

/// Complete horizontal synchronization after a fall during vertical jump integration.
/// The fall already selected altitude; this segment must not apply a second surface update.
pub(super) fn finish_interrupted_jump(
    world: &mut World,
    id: ObjectId,
    point: (Point, Point),
    fall: super::FallRules,
) -> Result<Vec<Notice>> {
    finish_interrupted_jump_inner(world, id, point, fall, None).map(|segment| segment.notices)
}

/// Interrupted airborne synchronization whose host can publish character falls and immersion.
pub(super) fn finish_interrupted_jump_in_action(
    world: &mut World,
    id: ObjectId,
    point: (Point, Point),
    fall: super::FallRules,
    falls: &mut Vec<super::MechFallReport>,
) -> Result<GroundSegmentReport> {
    finish_interrupted_jump_inner(world, id, point, fall, Some(falls))
}

/// Share the horizontal path while preserving the altitude already selected by the jump fall.
fn finish_interrupted_jump_inner(
    world: &mut World,
    id: ObjectId,
    point: (Point, Point),
    fall: super::FallRules,
    falls: Option<&mut Vec<super::MechFallReport>>,
) -> Result<GroundSegmentReport> {
    let motion = world.btech.constructed_units()[&id]
        .motion()
        .context("Fallen unit has no motion")?;
    resolve_ground_segment(
        world,
        id,
        motion,
        point,
        MovementRules {
            fall,
            ..MovementRules::STANDARD
        },
        false,
        falls,
    )
}

/// Segment-local notifications and XP; nested falls retain their own reports.
pub(super) struct GroundSegmentReport {
    pub boundary: Option<super::movement_report::BoundaryCrossing>,
    pub mines: Vec<super::MineEventReport>,
    pub vehicle_falls: Vec<super::VehicleFallReport>,
    pub notices: Vec<Notice>,
    pub pilot_notices: Vec<super::PilotNotice>,
    pub experience_messages: Vec<super::DiagnosticMessage>,
}

/// Resolve one planned segment without advancing turning, acceleration or the simulation clock.
fn resolve_ground_segment(
    world: &mut World,
    id: ObjectId,
    mut motion: Motion,
    (trace_start, mut proposed): (Point, Point),
    rules: MovementRules,
    settle: bool,
    mut falls: Option<&mut Vec<super::MechFallReport>>,
) -> Result<GroundSegmentReport> {
    let mut notices = Vec::new();
    let mut pilot_notices = Vec::new();
    let mut mines = Vec::new();
    let mut vehicle_falls = Vec::new();
    let mut experience_messages = Vec::new();
    let unit = &world.btech.constructed_units()[&id];
    let position = unit
        .position()
        .context("Ground segment requires placement")?;
    let map = &world.btech.maps()[&position.map];
    let current = map.base_hex(i64::from(position.x), i64::from(position.y))?;
    let traversed = trace_start.trace_positions(proposed)?;
    let walking = unit.walking_speed() + 0.1;
    let mut destination = proposed.containing_hex().ok();
    let mut previous_height = current.surface_height();
    let mut previous_tile = current;
    let mut actual_height = unit.elevation_level(current) as i16;
    let mut ground_elevation = unit.ground_elevation;
    let old_entry = unit.level_entry;
    let mut entry = old_entry;
    let mut checked_steps = Vec::new();
    let mut previous_position = position;
    let mut previous_point = motion.point;
    let mut boundary = None;
    let blocked = if settle && motion.speed == 0.0 && !unit.hex_sync_pending {
        None
    } else if actual_height != current.standing_height()
        && !(current.has_bridge() && actual_height == current.water_line() - 1)
        && !super::building_interior::inside(current, i32::from(actual_height))
        && !(current.is_ice()
            && (actual_height == current.surface_height()
                || actual_height == current.water_line() - 1))
    {
        Some("Terrain transition unavailable; movement stopped.")
    } else {
        let mut obstruction = None;
        for (hex, _) in traversed {
            let Ok(tile) = map.base_hex(i64::from(hex.x), i64::from(hex.y)) else {
                boundary = Some(super::movement_report::BoundaryCrossing::new(
                    id,
                    position.map,
                    motion,
                    "Map edge reached; movement stopped.",
                ));
                obstruction = Some("Map edge reached; movement stopped.");
                break;
            };
            let enters_hex =
                previous_position.x != hex.x as u16 || previous_position.y != hex.y as u16;
            if !enters_hex {
                continue;
            }
            let ice_check = settle && tile.is_ice() && actual_height == tile.water_line();
            let next_height = if !settle {
                actual_height
            } else if tile.is_ice() && actual_height < tile.water_line() {
                if actual_height == tile.water_line() - 1 && tile.water_depth() == 1 {
                    tile.water_line()
                } else {
                    tile.surface_height()
                }
            } else if tile.has_bridge() && actual_height < tile.standing_height() - 2 {
                tile.water_line() - 1
            } else if tile.has_building() {
                // A building is entered at the level the unit arrives at, or crossed on its
                // roof from roof height (Total Warfare p. 167).
                i16::try_from(super::building_interior::entry_level(
                    tile,
                    i32::from(actual_height),
                ))?
            } else {
                tile.standing_height()
            };
            let enters_building = tile.has_building() && next_height < tile.surface_height();
            let next_elevation =
                (next_height != tile.standing_height()).then_some(f64::from(next_height));
            let last_surface =
                if previous_tile.is_ice() && next_height >= previous_tile.water_line() {
                    previous_tile.water_line()
                } else if (previous_tile.has_bridge() || previous_tile.has_building())
                    && actual_height < previous_tile.standing_height()
                {
                    // Under a bridge the deck is not the surface the unit stands on, and inside a
                    // building the unit stands on its floor, not the roof.
                    actual_height
                } else {
                    previous_height
                };
            let collision_height = if tile.is_ice() && last_surface >= tile.water_line() {
                tile.water_line()
            } else if tile.has_bridge() && next_height < tile.water_line() {
                // Passing beneath the span, the unit stays on the river bed.
                next_height
            } else if enters_building {
                // Inside, the floor the unit enters at is the surface it meets.
                next_height
            } else {
                tile.surface_height()
            };
            let change = collision_height - last_surface;
            let enters_water =
                tile.is_open_water() || (tile.has_bridge() && next_height < tile.water_line());
            // Terrain the unit cannot enter, a change of more than two levels and water it is
            // running at all stop it at the hex's edge (Total Warfare pp. 48 and 52).
            let stop = if let Some(reason) = tile.prohibition(super::GroundMovement::Legged) {
                Some(reason.describe())
            } else if change.abs() > 2 {
                Some(
                    if change > 0 {
                        "The slope ahead is too steep to climb."
                    } else {
                        "The drop ahead is too steep to walk down."
                    }
                    .to_owned(),
                )
            } else if tile.is_open_water()
                && tile.water_depth() >= 1
                && motion.speed.abs() > walking
            {
                Some("You cannot run into the water!".to_owned())
            } else {
                // Stacking (Total Warfare p. 57): an enemy in the hex ends movement at its
                // edge, and a unit meaning to stop may not enter a hex already full.
                super::stacking::ground_entry_block(world, id, hex, motion.desired_speed)
            };
            // Backing across a level change takes the reverse-slope check, a kept house rule:
            // Total Warfare p. 49 forbids backward level changes outright.
            let backward = unit.moving_backward(motion.speed);
            let checked_height = stop.is_some() || (change != 0 && backward);
            // Inside a building the floor entered is the level climbed to, not the roof.
            let climbed = if enters_building {
                next_height
            } else if next_height == tile.surface_height() {
                tile.surface_height()
            } else {
                last_surface
            };
            let levels = (climbed - last_surface)
                .unsigned_abs()
                .min(u16::from(u8::MAX)) as u8;
            entry = Some(LevelEntry {
                x: hex.x as u16,
                y: hex.y as u16,
                levels,
            });
            let hazardous_ground = tile
                .entry_piloting_modifier(super::GroundMovement::Legged)
                .is_some()
                || tile.on_magma_crust(i32::from(next_height))
                || tile.can_bog_down(super::GroundMovement::Legged, i32::from(next_height))
                || tile.ground_heat(i32::from(next_height)) > 0;
            let stopping = stop.is_some();
            if checked_height
                || enters_water
                || enters_building
                || tile.is_ice()
                || hazardous_ground
                || map.mine_coverage(hex)?
            {
                checked_steps.push(GroundStep {
                    hex,
                    change,
                    old_position: previous_position,
                    old_point: previous_point,
                    elevation: next_elevation,
                    old_elevation: ground_elevation,
                    levels,
                    ice_check,
                    stop,
                });
            }
            actual_height = next_height;
            ground_elevation = next_elevation;
            if stopping {
                // The unit halts at this edge once its other hazards have been resolved.
                if destination != Some(hex) {
                    proposed = hex.center();
                }
                destination = Some(hex);
                break;
            }
            if previous_position.x != hex.x as u16 || previous_position.y != hex.y as u16 {
                previous_position.x = hex.x as u16;
                previous_position.y = hex.y as u16;
                previous_point = hex.center();
            }
            previous_height = tile.surface_height();
            previous_tile = tile;
        }
        obstruction
    };
    let mut position = position;
    if let Some(text) = blocked {
        motion.stop_translation();
        notices.push(Notice {
            unit: id,
            text: text.to_owned(),
        });
    } else {
        motion.point = proposed;
        let hex = destination.unwrap();
        position.x = hex.x as u16;
        position.y = hex.y as u16;
    }
    // Moving out of a burning hex adds two heat (Tactical Operations p. 42).
    let leaving_fire = {
        let unit = &world.btech.constructed_units()[&id];
        unit.position().is_some_and(|previous| {
            previous != position
                && !unit.airborne()
                && world.btech.maps()[&previous.map]
                    .base_hex(i64::from(previous.x), i64::from(previous.y))
                    .is_ok_and(|tile| tile.is_burning())
        })
    };
    // Magma heats a 'Mech that walks through a hex without occupying it at the turn's heat
    // reckoning: two points for crust and five for liquid magma, charged as it leaves along
    // the ground. A 'Mech that stays takes only the hex's per-turn heat (Tactical Operations:
    // Advanced Rules pp. 35, 61); magma heat is never capped.
    let leaving_magma = {
        let unit = &world.btech.constructed_units()[&id];
        unit.position()
            .filter(|previous| *previous != position && !unit.airborne())
            .and_then(|previous| {
                world.btech.maps()[&previous.map]
                    .base_hex(i64::from(previous.x), i64::from(previous.y))
                    .ok()
            })
            .map_or(0.0, |tile| {
                let level = unit.elevation_level(tile);
                if tile.on_magma_crust(level) {
                    2.0
                } else if tile.in_liquid_magma(level) {
                    5.0
                } else {
                    0.0
                }
            })
    };
    let unit = world.btech.constructed.get_mut(&id).unwrap();
    if unit
        .motion
        .is_some_and(|previous| previous.point != motion.point)
        || unit.position != Some(position)
    {
        unit.ground_elevation = ground_elevation;
    }
    if blocked.is_none() {
        unit.hex_sync_pending = false;
        unit.level_entry = entry;
    }
    unit.motion = Some(motion);
    unit.position = Some(position);
    if leaving_fire && unit.absorb_outside_heat(2) > 0 {
        notices.push(Notice {
            unit: id,
            text: "The flames you leave behind heat your 'Mech.".to_owned(),
        });
    }
    if leaving_magma > 0.0 {
        unit.heat.stored += leaving_magma;
        notices.push(Notice {
            unit: id,
            text: "Heat washes up from the magma you cross.".to_owned(),
        });
    }
    if blocked.is_some() || checked_steps.is_empty() {
        return Ok(GroundSegmentReport {
            boundary,
            vehicle_falls,
            mines,
            notices,
            pilot_notices,
            experience_messages,
        });
    }
    let mut fall_rules = rules.fall;
    fall_rules.toughness = unit
        .pilot()
        .and_then(|pilot| world.btech.character_values().get(&pilot))
        .is_some_and(|values| super::advantages::enabled(values, "Toughness"));
    let mut stopped = false;
    for GroundStep {
        hex,
        change,
        old_position,
        old_point,
        elevation,
        old_elevation,
        levels,
        ice_check,
        stop,
    } in checked_steps
    {
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        unit.position = Some(super::Position {
            map: position.map,
            x: hex.x as u16,
            y: hex.y as u16,
        });
        unit.ground_elevation = elevation;
        unit.level_entry = Some(LevelEntry {
            x: hex.x as u16,
            y: hex.y as u16,
            levels,
        });
        unit.motion.as_mut().unwrap().point = if hex == destination.unwrap() {
            proposed
        } else {
            hex.center()
        };
        if unit.position != Some(old_position) {
            notices.extend(super::hiding::movement(world, id));
        }
        let check_ice = if falls.is_some() {
            super::surface_break::check_ice_landing_in_action
        } else {
            super::surface_break::check_ice_landing
        };
        if ice_check && let Some(fracture) = check_ice(world, id, fall_rules)? {
            super::piloting::append_feedback(
                &mut pilot_notices,
                fracture.pilot_notices,
                notices.len(),
            );
            notices.extend(fracture.notices);
            vehicle_falls.extend(fracture.vehicle_falls.into_iter().map(|(_, fall)| fall));
            if let Some(falls) = falls.as_deref_mut() {
                falls.extend(fracture.falls.into_iter().map(|(_, fall)| fall));
            }
            motion = world.btech.constructed_units()[&id].motion().unwrap();
            if world.btech.constructed_units()[&id].posture() == super::Posture::Prone
                || world.btech.constructed_units()[&id].is_destroyed()
            {
                stopped = true;
                break;
            }
            continue;
        }
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        if let Some(text) = stop {
            restore_ground_position(unit, old_position, old_point, old_elevation, old_entry);
            let motion = unit.motion.as_mut().unwrap();
            motion.speed = 0.0;
            motion.desired_speed = 0.0;
            notices.push(Notice { unit: id, text });
            stopped = true;
            break;
        }
        let backward = world.btech.constructed_units()[&id].moving_backward(motion.speed);
        if change == 0 || !backward {
            let entry = super::water_movement::enter_water(
                world,
                id,
                true,
                fall_rules,
                falls.as_deref_mut(),
            )?;
            super::piloting::append_feedback(
                &mut pilot_notices,
                entry.pilot_notices,
                notices.len(),
            );
            notices.extend(entry.notices);
            experience_messages.extend(entry.experience_messages);
            motion = world.btech.constructed_units()[&id].motion().unwrap();
            if entry.stopped {
                stopped = true;
                break;
            }
            let event = super::mine_event::resolve(
                world,
                id,
                super::MineTriggerReason::Step,
                fall_rules,
                falls.is_some(),
            )?;
            super::piloting::append_feedback(
                &mut pilot_notices,
                event.pilot_notices.iter().cloned(),
                notices.len(),
            );
            notices.extend(event.notices.iter().cloned());
            mines.push(event);
            let entry = super::terrain_entry::enter(world, id, fall_rules, falls.as_deref_mut())?;
            super::piloting::append_feedback(
                &mut pilot_notices,
                entry.pilot_notices,
                notices.len(),
            );
            notices.extend(entry.notices);
            experience_messages.extend(entry.experience_messages);
            if let Some(breach) =
                super::building_interior::breach(world, id, backward, fall_rules, falls.is_some())?
                && let Some(collapse) =
                    breach.append_to(&mut notices, &mut pilot_notices, &mut experience_messages)
            {
                vehicle_falls.extend(collapse.vehicle_falls.into_iter().map(|(_, fall)| fall));
                if let Some(falls) = falls.as_deref_mut() {
                    falls.extend(collapse.falls.into_iter().map(|(_, fall)| fall));
                }
            }
            let unit = &world.btech.constructed_units()[&id];
            motion = unit.motion().unwrap();
            if unit.is_destroyed()
                || unit.posture() == super::Posture::Prone
                || unit.power() != Power::Running
            {
                stopped = true;
                break;
            }
            continue;
        }
        let control = super::reverse_slope::check(
            world,
            id,
            change,
            fall_rules.extended_piloting,
            falls.is_some(),
        )?;
        super::piloting::append_feedback(&mut pilot_notices, control.pilot_notices, notices.len());
        notices.extend(control.notices);
        experience_messages.extend(control.experience_messages);
        if control.success {
            let entry = super::water_movement::enter_water(
                world,
                id,
                false,
                fall_rules,
                falls.as_deref_mut(),
            )?;
            super::piloting::append_feedback(
                &mut pilot_notices,
                entry.pilot_notices,
                notices.len(),
            );
            notices.extend(entry.notices);
            experience_messages.extend(entry.experience_messages);
            motion = world.btech.constructed_units()[&id].motion().unwrap();
            if entry.stopped {
                stopped = true;
                break;
            }
            let event = super::mine_event::resolve(
                world,
                id,
                super::MineTriggerReason::Step,
                fall_rules,
                falls.is_some(),
            )?;
            super::piloting::append_feedback(
                &mut pilot_notices,
                event.pilot_notices.iter().cloned(),
                notices.len(),
            );
            notices.extend(event.notices.iter().cloned());
            mines.push(event);
            let entry = super::terrain_entry::enter(world, id, fall_rules, falls.as_deref_mut())?;
            super::piloting::append_feedback(
                &mut pilot_notices,
                entry.pilot_notices,
                notices.len(),
            );
            notices.extend(entry.notices);
            experience_messages.extend(entry.experience_messages);
            if let Some(breach) =
                super::building_interior::breach(world, id, backward, fall_rules, falls.is_some())?
                && let Some(collapse) =
                    breach.append_to(&mut notices, &mut pilot_notices, &mut experience_messages)
            {
                vehicle_falls.extend(collapse.vehicle_falls.into_iter().map(|(_, fall)| fall));
                if let Some(falls) = falls.as_deref_mut() {
                    falls.extend(collapse.falls.into_iter().map(|(_, fall)| fall));
                }
            }
            let unit = &world.btech.constructed_units()[&id];
            motion = unit.motion().unwrap();
            if unit.is_destroyed()
                || unit.posture() == super::Posture::Prone
                || unit.power() != Power::Running
            {
                stopped = true;
                break;
            }
            continue;
        }
        let report = resolve_segment_fall(world, id, change.abs(), fall_rules, falls.is_some())?;
        report.append_notices(id, &mut notices, &mut pilot_notices);
        if let Some(falls) = falls.as_deref_mut() {
            falls.push(report);
        }
        if change > 0 {
            let unit = world.btech.constructed.get_mut(&id).unwrap();
            restore_ground_position(unit, old_position, old_point, old_elevation, old_entry);
        }
        stopped = true;
        break;
    }
    if !stopped {
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        unit.position = Some(position);
        unit.ground_elevation = ground_elevation;
        unit.level_entry = entry;
        motion.point = proposed;
        unit.motion = Some(motion);
    }
    Ok(GroundSegmentReport {
        boundary: None,
        vehicle_falls,
        mines,
        notices,
        pilot_notices,
        experience_messages,
    })
}

/// Resolve signed segment severity using the publication capability of the owning action.
fn resolve_segment_fall(
    world: &mut World,
    id: ObjectId,
    levels: i16,
    rules: super::FallRules,
    character: bool,
) -> Result<super::MechFallReport> {
    if character && world.objects[&id].flags.contains(Flag::InCharacter) {
        return super::fall::resolve_character_signed_fall(world, id, levels, rules);
    }
    super::fall::resolve_signed_fall(world, id, levels, rules)
}

/// Rollback can restore an interrupted bridge hex update; retain its explicit marker.
fn restore_ground_position(
    unit: &mut super::Mech,
    position: super::Position,
    point: Point,
    elevation: Option<f64>,
    entry: Option<LevelEntry>,
) {
    unit.position = Some(position);
    unit.ground_elevation = elevation;
    unit.level_entry = entry;
    unit.hex_sync_pending = point.containing_hex().ok()
        != Some(super::HexCoordinate {
            x: i32::from(position.x),
            y: i32::from(position.y),
        });
    let motion = unit.motion.as_mut().unwrap();
    motion.point = point;
    if unit.hex_sync_pending {
        motion.speed = 0.0;
    }
}
