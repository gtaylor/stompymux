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

    /// Project one movement event, preserving reverse travel and shared map scaling.
    pub(super) fn project_step(self, rate: f64) -> Result<Point> {
        ensure!(rate.is_finite() && rate >= 0.0, "Invalid movement rate");
        self.point.project(
            self.heading + if self.speed < 0.0 { 180.0 } else { 0.0 },
            self.speed.abs() / 645.0 * rate,
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

    /// Advance heading toward the commanded bearing with the configured ground-turn model.
    pub(super) fn turn_toward(&mut self, maximum: f64, fasa_turning: bool, multiplier: f64) {
        if maximum <= 0.0 {
            return;
        }
        let difference = (self.desired_heading - self.heading + 180.0).rem_euclid(360.0) - 180.0;
        let turn = if fasa_turning {
            let cap = if self.speed < 0.0 {
                maximum * 2.0 / 3.0
            } else {
                maximum
            };
            (1.5 * (cap - self.speed.abs()).max(0.0) * (maximum / cap) / 10.75).max(0.0)
        } else {
            let base = if self.speed.abs() < 1.0 { 1.5 } else { 1.0 } * maximum / 10.75;
            if difference.abs() > base && self.speed > maximum * 2.0 / 3.0 + 0.1 {
                base * (2.0 - 1.5 * self.speed / maximum)
            } else {
                base
            }
        };
        self.turn_by(turn * multiplier);
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

    /// Reconcile a changed load while retaining paved-surface headroom where applicable.
    pub(super) fn limit_load(&mut self, maximum: f64, actual_maximum: f64) {
        self.desired_speed = self.desired_speed.clamp(-maximum * 2.0 / 3.0, maximum);
        self.speed = self
            .speed
            .clamp(-actual_maximum * 2.0 / 3.0, actual_maximum);
        if maximum == 0.0 {
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

    /// Validate persistent motion before it can enter the simulation.
    pub(crate) fn validate(self, maximum: f64) -> Result<()> {
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
                    && *value >= -maximum * 2.0 / 3.0
                    && *value <= maximum),
            "Invalid unit speed"
        );
        Ok(())
    }
}

/// Shared ground/jump movement configuration, including charge, terrain and fall rules.
#[derive(Debug, Clone, Copy)]
pub struct MovementRules {
    /// Fusion-powered VTOLs may omit fuel consumption when configured by the host.
    pub free_fusion_vtol_fuel: bool,
    /// Enable hot-myomer assistance when calculating towing load.
    pub tsm_tow_bonus: bool,
    /// Use pilot-based aim for DFA landing attacks.
    pub physical_pilot_skill: bool,
    pub fasa_turning: bool,
    /// Charge tracking and collision options for each committed movement update.
    pub charge: super::ChargePolicy,
    pub slowdown: i64,
    /// Enable tracked/wheeled tree and rock avoidance checks. Hovercraft tree checks always apply.
    pub new_terrain: bool,
    /// Require a control check when backing across an elevation change.
    pub roll_on_backwalk: bool,
    /// Use the skid speed bands for cliff avoidance and a one-level uphill fall.
    pub skid_cliff: bool,
    /// Damage and pilot protection rules for failed movement checks.
    pub fall: super::FallRules,
}

impl MovementRules {
    /// Physical landing policy shares movement, hit arcs and fall settings with the tick.
    pub(super) fn landing_physical(self) -> super::PhysicalRules {
        super::PhysicalRules {
            use_pilot_skill: self.physical_pilot_skill,
            fasa_turning: self.fasa_turning,
            extended_movement: self.charge.extended_movement,
            hit_arc_mode: self.charge.hit_arc_mode,
            glancing: super::GlancingMode::Disabled,
            fall: self.fall,
        }
    }

    /// Conventional movement and fall settings, suitable for an isolated simulation.
    pub const STANDARD: Self = Self {
        free_fusion_vtol_fuel: false,
        tsm_tow_bonus: true,
        physical_pilot_skill: true,
        fasa_turning: false,
        charge: super::ChargePolicy::STANDARD,
        slowdown: 2,
        new_terrain: false,
        roll_on_backwalk: true,
        skid_cliff: false,
        fall: super::FallRules {
            vehicle_impact: crate::VehicleImpactRules::STANDARD,
            stacking: crate::StackingRules::STANDARD,
            stagger: super::StaggerMode::Retain,
            hit: super::HitRules {
                inferno_penalty: false,
                exile_stun_mode: 0,
            },
            extended_piloting: true,
            toughness: false,
        },
    };
}

/// A checked crossing carries both map elevation and the selected physical bridge surface.
struct GroundStep {
    hex: super::HexCoordinate,
    change: i16,
    old_position: super::Position,
    old_point: Point,
    elevation: Option<f64>,
    old_elevation: Option<f64>,
    ice_check: bool,
}

/// Set desired speed in kilometers per hour with standard enabled myomer towing assistance.
pub fn set_speed(world: &mut World, id: ObjectId, pilot: ObjectId, speed: f64) -> Result<Notice> {
    let notice = set_speed_by_actor(
        world,
        id,
        super::combat_operator::ControlActor::Player(pilot),
        speed,
        super::SpeedPolicy::STANDARD,
        false,
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
        false,
    )
}

fn set_speed_by_actor(
    world: &mut World,
    id: ObjectId,
    actor: super::combat_operator::ControlActor,
    speed: f64,
    policy: super::SpeedPolicy,
    free_fusion_fuel: bool,
) -> Result<Notice> {
    if world.btech.vehicles().contains_key(&id) {
        return super::vehicle_driving::set_control_by_actor(
            world,
            id,
            actor,
            Some(speed),
            None,
            policy,
            free_fusion_fuel,
        );
    }
    super::power::controlled_unit_by_actor(world, id, actor)?;
    super::fortification::require_mobile(world, id)?;
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
    let maximum = super::motion_controls::throttle_configured(world, id, policy)?;
    ensure!(
        unit.dumping().is_none() || speed <= maximum * 2.0 / 3.0 + 0.1,
        "You can not run while dumping ammo!"
    );
    ensure!(
        unit.unjam().is_none() || speed <= maximum * 2.0 / 3.0 + 0.1,
        "You can not run while unjamming your weapon!"
    );
    ensure!(
        speed.is_finite() && speed >= -maximum * 2.0 / 3.0 && speed <= maximum,
        "Speed exceeds unit limits"
    );
    super::motion_controls::require_reverse_allowed(world, id, speed)?;
    ensure!(
        unit.stun_remaining() == 0 || speed <= maximum * 2.0 / 3.0 + 0.1,
        "You cannot move faster than cruise speed while stunned!"
    );
    let mut motion = unit.motion().context("Unit is not placed")?;
    let position = unit.position().context("Unit is not placed")?;
    let tile =
        world.btech.maps()[&position.map].base_hex(i64::from(position.x), i64::from(position.y))?;
    let wet = tile.immerses(unit.elevation_level(tile));
    ensure!(
        !wet || speed <= maximum * 2.0 / 3.0 + 0.1,
        "You can't run through water!"
    );
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
    free_fusion_fuel: bool,
) -> Result<Notice> {
    let notice = set_speed_by_actor(
        world,
        id,
        super::combat_operator::ControlActor::Player(pilot),
        speed,
        policy,
        free_fusion_fuel,
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
            false,
        );
    }
    super::power::controlled_unit_by_actor(world, id, actor)?;
    super::fortification::require_mobile(world, id)?;
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
                .consume_vtol_fuel(
                    flight.vertical_speed,
                    flight.altitude as i32,
                    false,
                    rules.free_fusion_vtol_fuel,
                )?;
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
        let maximum = world.btech.unit(id).expect("moving unit").maximum_speed();
        let maximum = super::load::movement_maximum(world, id, maximum, rules.tsm_tow_bonus)?;
        let maximum = if let Some(unit) = world.btech.constructed_units().get(&id) {
            let effective = super::speed_bonus::on_map(
                world,
                unit.position(),
                unit.movement_maximum_at(maximum),
            )?;
            unit.turning_from_maximum(effective)
        } else {
            super::speed_bonus::vehicle_on_map(
                world,
                world.btech.vehicles()[&id].position(),
                maximum,
            )?
        };
        let multiplier = world
            .btech
            .constructed_units()
            .get(&id)
            .map_or(1.0, |unit| unit.chassis().turn_multiplier());
        motion.turn_toward(maximum, rules.fasa_turning, multiplier);
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
    let mut experience_messages = Vec::new();
    let mut collisions = super::stacking::StackingEffects::default();
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
            let turn = if rules.fasa_turning {
                18.0
            } else {
                let chassis = unit.chassis().turn_multiplier();
                3.0 * f64::from(unit.jump_capacity(map.gravity)?.movement_points) * chassis
            };
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
        notices.extend(super::charge_tracking::begin_update(
            world,
            id,
            rules.charge,
        ));
        super::charge_tracking::record_distance(world, id, motion.point, proposed, rules.charge);
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
        let unit = &world.btech.constructed_units()[&id];
        if character && unit.position() != Some(position) && !unit.hex_sync_pending() {
            experience_messages.extend(super::movement_experience::record_entry(
                world,
                id,
                rules.fall.extended_piloting,
            )?);
        }
        let unit = &world.btech.constructed_units()[&id];
        if !unit.is_destroyed() && unit.position() != Some(position) && !unit.hex_sync_pending() {
            let input = super::StackingInput {
                entry: super::StackingEntry::Ground,
                mass: unit.effective_mass()?,
                jump_movement_points: 0,
            };
            let events = if character {
                super::stacking::resolve_in_action(
                    world,
                    id,
                    input,
                    rules.fall.stacking,
                    rules.fall,
                    &mut collisions,
                    (&mut pilot_notices, notices.len()),
                )?
            } else {
                super::resolve_stacking(world, id, input, rules.fall.stacking, rules.fall)?
            };
            notices.extend(events);
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
    let mut destination = proposed.containing_hex().ok();
    let mut previous_height = current.surface_height();
    let mut previous_tile = current;
    let mut actual_height = unit.elevation_level(current) as i16;
    let mut ground_elevation = unit.ground_elevation;
    let mut elevation_cost = 0.0;
    let mut checked_steps = Vec::new();
    let mut previous_position = position;
    let mut previous_point = motion.point;
    let mut boundary = None;
    let blocked = if settle && motion.speed == 0.0 && !unit.hex_sync_pending {
        None
    } else if actual_height != current.standing_height()
        && !(current.has_bridge() && actual_height == current.water_line() - 1)
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
            } else {
                tile.standing_height()
            };
            let next_elevation =
                (next_height != tile.standing_height()).then_some(f64::from(next_height));
            let last_surface =
                if previous_tile.is_ice() && next_height >= previous_tile.water_line() {
                    previous_tile.water_line()
                } else {
                    previous_height
                };
            let collision_height = if tile.is_ice() && last_surface >= tile.water_line() {
                tile.water_line()
            } else {
                tile.surface_height()
            };
            let change = collision_height - last_surface;
            let checked_height =
                change.abs() > 2 || (change != 0 && motion.speed < 0.0 && rules.roll_on_backwalk);
            let enters_water =
                tile.is_open_water() || (tile.has_bridge() && next_height < tile.water_line());
            let hazardous_ground = tile.entry_piloting_modifier().is_some()
                || tile.on_magma_crust(i32::from(next_height));
            if checked_height
                || enters_water
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
                    ice_check,
                });
            }
            if !checked_height && next_height == tile.surface_height() {
                elevation_cost +=
                    f64::from((tile.surface_height() - last_surface).unsigned_abs()) * 10.75;
            }
            actual_height = next_height;
            ground_elevation = next_elevation;
            if change.abs() > 2 {
                // A cliff ends this tick at the first hazardous crossing.
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
        motion.speed = motion.speed.signum() * (motion.speed.abs() - elevation_cost).max(0.0);
        motion.point = proposed;
        let hex = destination.unwrap();
        position.x = hex.x as u16;
        position.y = hex.y as u16;
    }
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
    }
    unit.motion = Some(motion);
    unit.position = Some(position);
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
        ice_check,
    } in checked_steps
    {
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        unit.position = Some(super::Position {
            map: position.map,
            x: hex.x as u16,
            y: hex.y as u16,
        });
        unit.ground_elevation = elevation;
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
        if change.abs() > 2 {
            if change > 0 {
                restore_ground_position(unit, old_position, old_point, old_elevation);
            }
            notices.push(Notice {
                unit: id,
                text: (if change > 0 {
                    "You attempt to climb a hill too steep for you."
                } else {
                    "You notice a large drop in front of you"
                })
                .to_owned(),
            });
            let control = super::cliff::avoids(world, id, change, motion.speed, rules)?;
            control.capture_feedback(id, &mut notices, &mut pilot_notices);
            let success = control.success;
            if success {
                notices.push(Notice {
                    unit: id,
                    text: (if change > 0 {
                        "You manage to stop before crashing."
                    } else {
                        "You manage to stop before falling off."
                    })
                    .to_owned(),
                });
                let unit = world.btech.constructed.get_mut(&id).unwrap();
                restore_ground_position(unit, old_position, old_point, old_elevation);
            } else {
                notices.push(Notice {
                    unit: id,
                    text: (if change > 0 {
                        "You run headlong into the cliff and fall down!"
                    } else {
                        "You run off the cliff and fall to the ground below."
                    })
                    .to_owned(),
                });
                let levels = if change < 0 {
                    -change
                } else {
                    cliff_fall_levels(motion.speed, rules.skid_cliff)
                };
                let report = resolve_segment_fall(world, id, levels, fall_rules, falls.is_some())?;
                report.append_notices(id, &mut notices, &mut pilot_notices);
                if let Some(falls) = falls.as_deref_mut() {
                    falls.push(report);
                }
            }
            let unit = world.btech.constructed.get_mut(&id).unwrap();
            let motion = unit.motion.as_mut().unwrap();
            motion.speed = 0.0;
            motion.desired_speed = 0.0;
            stopped = true;
            break;
        }
        if change == 0 || motion.speed >= 0.0 || !rules.roll_on_backwalk {
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
            restore_ground_position(unit, old_position, old_point, old_elevation);
        }
        stopped = true;
        break;
    }
    if !stopped {
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        unit.position = Some(position);
        unit.ground_elevation = ground_elevation;
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

/// Ordinary uphill crash damage uses signed speed and truncates before dividing.
fn cliff_fall_levels(speed: f64, skid: bool) -> i16 {
    if skid {
        return 1;
    }
    (1.0 + speed / 10.75) as i16 / 4
}

/// Rollback can restore an interrupted bridge hex update; retain its explicit marker.
fn restore_ground_position(
    unit: &mut super::Mech,
    position: super::Position,
    point: Point,
    elevation: Option<f64>,
) {
    unit.position = Some(position);
    unit.ground_elevation = elevation;
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

#[cfg(test)]
mod cliff_tests {
    use super::*;

    #[test]
    fn cliff_speed_thresholds_preserve_reverse_asymmetry_and_truncation() {
        for (mp, ordinary, skid, levels) in [
            (0.0, 0, -1, 0),
            (2.0, 1, -1, 0),
            (2.1, 1, 0, 0),
            (3.0, 1, 0, 1),
            (4.1, 1, 1, 1),
            (7.1, 2, 2, 2),
            (10.1, 3, 4, 2),
            (-2.0, 0, -1, 0),
            (-5.0, 1, 1, -1),
        ] {
            assert_eq!(super::super::cliff::modifier(mp * 10.75, false), ordinary);
            assert_eq!(super::super::cliff::modifier(mp * 10.75, true), skid);
            assert_eq!(cliff_fall_levels(mp * 10.75, false), levels);
            assert_eq!(cliff_fall_levels(mp * 10.75, true), 1);
        }
    }
}
