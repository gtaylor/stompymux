//! Transactional conventional jumping on supported terrain routes, with committed landing and stabilization.
use super::{
    BattleJumpFlight, BattleJumpOutcome, BattleJumpPath, BattleNotice, BattlePosture, BattlePower,
    BattleUnit, Terrain,
};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use std::sync::Arc;

/// Current administrative course values, retained after the flight cursor is retired.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct LastJump {
    pub heading: u16,
    pub length: i16,
}

impl LastJump {
    /// Capture historical field units without changing normalized flight geometry.
    fn from_path(path: BattleJumpPath) -> Result<Self> {
        Ok(Self {
            heading: path.heading()?,
            length: (path.distance() * 322.5).trunc().min(32767.0) as i16,
        })
    }

    /// Reject invalid bearings; the signed length is bounded by its storage type.
    pub(super) fn validate(self) -> Result<()> {
        ensure!(self.heading < 360, "Invalid last jump");
        Ok(())
    }
}

impl BattleUnit {
    /// Current airborne cursor; absent once a landing or fall has completed.
    pub fn flight(&self) -> Option<BattleJumpFlight> {
        self.flight
    }

    /// Whether the unit is jumping or falling through the air.
    pub fn airborne(&self) -> bool {
        self.orbital_drop.is_some()
            || self.flight.is_some()
            || self.free_fall.is_some_and(|fall| !fall.grounded())
    }

    /// Pending unpowered vertical descent, including its durable event countdown.
    pub fn free_fall(&self) -> Option<super::BattleFreeFall> {
        self.free_fall
    }

    /// Integer altitude used by terrain effects; in-flight samples use the game's rounding.
    pub(super) fn elevation_level(&self, tile: super::BattleHex) -> i32 {
        if let Some(drop) = self.orbital_drop {
            return drop.elevation();
        }
        if let Some(fall) = self.free_fall.filter(|fall| !fall.grounded()) {
            return fall.elevation();
        }
        let Some(flight) = self.flight() else {
            return self
                .ground_elevation
                .unwrap_or_else(|| f64::from(tile.standing_height())) as i32;
        };
        let elevation = flight.sample().elevation;
        if flight.total_travelled() == 0.0 {
            return elevation as i32;
        }
        (elevation + 0.5).trunc() as i32
    }

    /// Remaining committed seconds of post-jump stabilization.
    pub fn jump_stabilization(&self) -> u8 {
        self.jump_stabilization
    }
}

/// Validate the currently supported route on launch and persisted-world loading.
pub(super) fn validate_route(map: &super::StoredBattleMap, path: BattleJumpPath) -> Result<()> {
    ensure!(
        map.flags & 16 == 0,
        "The underground ceiling prevents jumping"
    );
    let start = path.sample(0.0, path.movement_points())?;
    let end = path.sample(1.0, path.movement_points())?;
    ensure!(
        path.is_continuation() || start.point.containing_hex()? != end.point.containing_hex()?,
        "You're already in the target hex."
    );
    // Endpoint heights are launch-time facts; ice may break beneath a saved destination.
    for coordinate in start.point.trace(end.point)? {
        let coordinate = map.motion_hex(coordinate)?;
        map.base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    }
    Ok(())
}

/// Launch toward a bearing/range projection, snapping the destination to its hex center.
/// Supported terrain uses destination height and resolves hills during flight.
/// Failed pre-launch checks resolve tactical falls; host actions additionally publish character consequences.
pub fn launch_jump(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    bearing: i32,
    range: f64,
) -> Result<Vec<BattleNotice>> {
    let mut candidate = world.clone();
    let notices = launch(
        &mut candidate,
        id,
        pilot,
        JumpRequest::Projected { bearing, range },
        super::BattleMovementRules::STANDARD.fall,
        super::SpeedPolicy::STANDARD,
        None,
    )?;
    candidate.btech.validate(&candidate)?;
    *world = candidate;
    Ok(notices)
}

/// Plan a DFA jump to an acquired target's current hex without chasing subsequent movement.
/// The active flight owns the target identity; landing dispatch is handled separately.
pub fn launch_dfa(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    target: Option<ObjectId>,
) -> Result<Vec<BattleNotice>> {
    let mut candidate = world.clone();
    let notices = launch(
        &mut candidate,
        id,
        pilot,
        JumpRequest::Target(target),
        super::BattleMovementRules::STANDARD.fall,
        super::SpeedPolicy::STANDARD,
        None,
    )?;
    candidate.btech.validate(&candidate)?;
    *world = candidate;
    Ok(notices)
}

/// Launch-time routing facts distinguish projections from fixed target identities.
#[derive(Clone, Copy)]
enum JumpDestination {
    Projected { bearing: i32, range: f64 },
    Target(ObjectId),
}

/// Uninterpreted native arguments and typed Lua requests enter the same ordered admission path.
#[derive(Clone, Copy)]
enum JumpRequest<'a> {
    Projected { bearing: i32, range: f64 },
    Target(Option<ObjectId>),
    Text(&'a str),
}

impl JumpRequest<'_> {
    /// Interpret only after the pre-launch roll; a failed roll never examines destination arguments.
    fn destination(self, world: &World, id: ObjectId) -> Result<JumpDestination> {
        match self {
            Self::Projected { bearing, range } => Ok(JumpDestination::Projected { bearing, range }),
            Self::Target(target) => Ok(JumpDestination::Target(
                target
                    .or_else(|| {
                        world.btech.constructed_units()[&id]
                            .target_lock()
                            .map(|lock| lock.target)
                    })
                    .context("Invalid Target!")?,
            )),
            Self::Text(text) => {
                let args: Vec<_> = text.split_whitespace().collect();
                match args.as_slice() {
                    [] => Self::Target(None).destination(world, id),
                    [target] => Self::Target(Some(ObjectId(
                        target
                            .strip_prefix('#')
                            .context("Usage: jump [#target | <bearing> <range>]")?
                            .parse()
                            .context("Invalid target number")?,
                    )))
                    .destination(world, id),
                    [bearing, range] => Self::Projected {
                        bearing: bearing.parse().context("Invalid jump bearing")?,
                        range: range.parse().context("Invalid jump range")?,
                    }
                    .destination(world, id),
                    _ => anyhow::bail!("Usage: jump [#target | <bearing> <range>]"),
                }
            }
        }
    }
}

/// Fully checked geometry remains uncommitted until all post-roll request validation succeeds.
struct PreparedJump {
    motion: super::BattleMotion,
    flight: BattleJumpFlight,
    last_jump: LastJump,
    dfa: bool,
}

/// Consequences that need the host's private injury, XP and casualty publication.
#[derive(Default)]
struct LaunchEffects {
    pilot_notices: Vec<super::BattlePilotNotice>,
    rejection: Option<String>,
    fall: Option<super::BattleFallReport>,
    experience_messages: Vec<super::BattleChannelMessage>,
}

/// Native and Lua conventional jumps publish a failed launch's consequences atomically.
pub(crate) fn launch_jump_action(
    scripts: &crate::Scripts,
    config: &crate::Config,
    id: ObjectId,
    pilot: ObjectId,
    bearing: i32,
    range: f64,
) -> Result<()> {
    launch_action(
        scripts,
        config,
        id,
        pilot,
        JumpRequest::Projected { bearing, range },
    )
}

/// DFA uses the same launch check and publication path as a projected jump.
pub(crate) fn launch_dfa_action(
    scripts: &crate::Scripts,
    config: &crate::Config,
    id: ObjectId,
    pilot: ObjectId,
    target: Option<ObjectId>,
) -> Result<()> {
    launch_action(scripts, config, id, pilot, JumpRequest::Target(target))
}

/// Mechanical effects and every publication share the caller's world/effects checkpoint.
fn launch_action(
    scripts: &crate::Scripts,
    config: &crate::Config,
    id: ObjectId,
    pilot: ObjectId,
    request: JumpRequest<'_>,
) -> Result<()> {
    let before = scripts.world.borrow().clone();
    let checkpoint = scripts.effects.checkpoint();
    let result = (|| {
        let mut effects = LaunchEffects::default();
        let mut rules = super::BattleFallRules::configured(config);
        rules.toughness =
            super::skills::boolean_advantage(&scripts.world.borrow(), pilot, "Toughness");
        let notices = launch(
            &mut scripts.world.borrow_mut(),
            id,
            pilot,
            request,
            rules,
            super::SpeedPolicy::configured(config),
            Some(&mut effects),
        )?;
        super::piloting::publish_ordered_notices(scripts, &notices, &effects.pilot_notices)?;
        super::channels::publish(scripts, config, &effects.experience_messages)?;
        if let Some(rejection) = &effects.rejection {
            super::notify_message(
                scripts,
                super::BattleMessageTarget::Player(pilot),
                rejection,
            )?;
        }
        if let Some(fall) = effects.fall {
            super::evacuation::publish_fall_consequences(scripts, config, &fall)?;
        }
        super::evacuation::publish_new_casualties(scripts, config, &before)?;
        scripts.world.borrow().validate(config)?;
        Ok(())
    })();
    if result.is_err() {
        *scripts.world.borrow_mut() = before;
        scripts.effects.restore(checkpoint);
    }
    result
}

/// Shared launch authorization, terrain validation and flight publication.
fn launch(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    request: JumpRequest<'_>,
    rules: super::BattleFallRules,
    speed: super::SpeedPolicy,
    mut effects: Option<&mut LaunchEffects>,
) -> Result<Vec<BattleNotice>> {
    ensure!(
        !world.btech.tows().contains_key(&id),
        "You cannot jump while towing another unit"
    );
    super::power::controlled_unit(world, id, pilot)?;
    let unit = &world.btech.constructed_units()[&id];
    unit.validate()?;
    super::fortification::require_mobile(world, id)?;
    unit.hull_down.require_mobile()?;
    ensure!(
        unit.power() == BattlePower::Running && !unit.is_destroyed(),
        "Start the unit first"
    );
    let maximum = unit.mobility().maximum_speed;
    let map = unit
        .position()
        .and_then(|position| world.btech.maps().get(&position.map));
    // Towing was rejected above, so the configured myomer towing discount cannot apply.
    let loaded = unit.effective_speed_with_load(
        map,
        super::unit_load(world, id, true)?,
        maximum,
        unit.pilot()
            .is_some_and(|pilot| super::skills::boolean_advantage(world, pilot, "Speed_Demon")),
        speed.tsm_sprint_bonus,
    )?;
    ensure!(maximum - loaded <= 10.75, "No, with this cargo you won't!");
    ensure!(
        unit.dumping().is_none(),
        "You are too busy dumping ammunition!"
    );
    ensure!(
        unit.unjam().is_none(),
        "You are too busy unjamming a weapon!"
    );
    ensure!(!unit.airborne(), "You're already airborne!");
    ensure!(
        !unit.free_fall().is_some_and(|fall| !fall.grounded()),
        "You cannot jump while falling!"
    );
    ensure!(
        unit.jump_stabilization == 0,
        "You haven't stabilized from your last jump yet."
    );
    ensure!(
        unit.posture() == BattlePosture::Standing,
        "You can't Jump from a FALLEN position"
    );
    ensure!(
        unit.stand_timer().is_none(),
        "You haven't finished standing up yet."
    );
    ensure!(
        map.context("Unit is not on a battlefield")?.flags & 16 == 0,
        "The underground ceiling prevents jumping"
    );
    let checked = unit.stagger().action_level() > 0;
    let mut notices = Vec::new();
    if checked {
        notices.push(BattleNotice {
            unit: id,
            text: "The damage inhibits your coordination...".into(),
        });
        let modifier = super::stagger::action_modifier(unit);
        let mut check = super::roll_piloting(world, id, modifier, rules.extended_piloting)?;
        if let Some(effects) = effects.as_deref_mut() {
            super::piloting::capture_feedback(
                id,
                Some(pilot),
                &check,
                &mut notices,
                &mut effects.pilot_notices,
            );
            effects
                .experience_messages
                .extend(super::piloting::award_control_check(
                    world,
                    id,
                    &mut check,
                    rules.extended_piloting,
                )?);
        }
        if !check.success {
            notices.push(BattleNotice {
                unit: id,
                text: "... something you apparently can't handle!".into(),
            });
            notices.extend(super::broadcast::observer_notices(
                world,
                id,
                "engages jumpjets, rolls to the side and slams into the ground!",
            ));
            let fall = if effects.is_some() && world.objects[&id].flags.contains(Flag::InCharacter)
            {
                super::fall::resolve_character_fall(world, id, 1, rules)?
            } else {
                super::resolve_fall(world, id, 1, rules)?
            };
            if let Some(effects) = effects {
                fall.append_notices(id, &mut notices, &mut effects.pilot_notices);
                effects.fall = Some(fall);
            } else {
                notices.extend(fall.notices(id));
            }
            return Ok(notices);
        }
    }
    let prepared = match prepare_jump(world, id, request) {
        Ok(prepared) => prepared,
        Err(error) if checked => {
            let text = format!("{error:#}");
            if let Some(effects) = effects {
                effects.rejection = Some(text);
            } else {
                notices.push(BattleNotice { unit: id, text });
            }
            return Ok(notices);
        }
        Err(error) => return Err(error),
    };
    let PreparedJump {
        mut motion,
        flight,
        last_jump,
        dfa,
    } = prepared;
    motion.speed = 0.0;
    motion.desired_heading = motion.heading;
    let unit = Arc::make_mut(&mut world.btech.constructed)
        .get_mut(&id)
        .unwrap();
    unit.motion = Some(motion);
    unit.ground_elevation = None;
    unit.flight = Some(flight);
    unit.last_jump = last_jump;
    notices.push(BattleNotice {
        unit: id,
        text: if dfa {
            "You engage your jump jets for a Death From Above attack!"
        } else {
            "You engage your jump jets."
        }
        .to_owned(),
    });
    notices.extend(super::broadcast::observer_notices(
        world,
        id,
        "engages jumpjets!",
    ));
    Ok(notices)
}

/// Route validation cannot mutate the launch attempt or undo its already-completed control roll.
fn prepare_jump(world: &World, id: ObjectId, request: JumpRequest<'_>) -> Result<PreparedJump> {
    let request = request.destination(world, id)?;
    let unit = &world.btech.constructed_units()[&id];
    let position = unit.position().context("Unit is not on a battlefield")?;
    let map = &world.btech.maps()[&position.map];
    let capacity = unit.jump_capacity(map.gravity)?;
    let motion = unit.motion().context("Unit has no motion state")?;
    let (destination, range, target) = match request {
        JumpDestination::Projected { bearing, range } => {
            ensure!(range.is_finite() && range > 0.0, "Invalid jump range");
            (
                motion
                    .point
                    .project(f64::from(bearing), range)?
                    .containing_hex()?,
                range,
                None,
            )
        }
        JumpDestination::Target(target) => {
            ensure!(target != id, "Cannot land on yourself");
            ensure!(
                world
                    .objects
                    .get(&target)
                    .is_some_and(|object| !object.flags.contains(Flag::Going)),
                "Invalid Target!"
            );
            let victim = world
                .btech
                .constructed_units()
                .get(&target)
                .context("Invalid Target!")?;
            let other = victim.position().context("Target is not placed")?;
            ensure!(
                !victim.is_destroyed() && other.map == position.map,
                "Invalid Target!"
            );
            ensure!(
                super::visible_contact(world, id, target)?.is_some()
                    && !super::unit_terrain_los(world, id, target)?.blocked,
                "Target is not in line of sight!"
            );
            (
                super::BattleHexCoordinate {
                    x: i32::from(other.x),
                    y: i32::from(other.y),
                },
                super::unit_range(world, id, target)?.spatial,
                Some(target),
            )
        }
    };
    ensure!(
        range <= f64::from(capacity.movement_points),
        "That target is out of range!"
    );
    ensure!(
        (destination.x, destination.y) != (i32::from(position.x), i32::from(position.y)),
        "You're already in the target hex."
    );
    let elevation =
        unit.elevation_level(map.base_hex(i64::from(position.x), i64::from(position.y))?) as i16;
    ensure!(elevation >= -1, "Cannot launch from below elevation -1");
    let resolved_destination = map.motion_hex(destination)?;
    let destination_elevation = map
        .base_hex(
            i64::from(resolved_destination.x),
            i64::from(resolved_destination.y),
        )?
        .standing_height();
    let path = match request {
        JumpDestination::Projected { bearing, range } => BattleJumpPath::projected(
            motion.point,
            bearing,
            range,
            elevation,
            destination_elevation,
            capacity.movement_points,
        )?,
        JumpDestination::Target(_) => BattleJumpPath::targeted(
            motion.point,
            destination.center(),
            range,
            elevation,
            destination_elevation,
            capacity.movement_points,
        )?,
    };
    validate_route(map, path)?;
    let last_jump = LastJump::from_path(path)?;
    let flight = BattleJumpFlight::new(path);
    Ok(PreparedJump {
        motion,
        last_jump,
        dfa: target.is_some(),
        flight: target.map_or(flight, |target| flight.with_dfa_target(target)),
    })
}

/// Advance jumps, falls, stabilization and shared orbital drops once; jumps also dispatch charge intent.
/// All units, counters, fall dice and notices share one candidate.
pub fn advance_jumps(
    world: &mut World,
    movement: super::BattleMovementRules,
) -> Result<Vec<BattleNotice>> {
    Ok(advance_jumps_inner(world, movement, false)?.notices)
}

/// Advance conventional airborne state; the host runs orbital continuations around Lua callbacks.
pub(super) fn advance_jumps_in_action(
    world: &mut World,
    movement: super::BattleMovementRules,
) -> Result<super::movement_report::MovementReport> {
    advance_jumps_inner(world, movement, true)
}

/// Shared tick; the explicit action mode owns character falls and casualty publication.
fn advance_jumps_inner(
    world: &mut World,
    movement: super::BattleMovementRules,
    character: bool,
) -> Result<super::movement_report::MovementReport> {
    let mut falls = Vec::new();
    let mut vehicle_falls = Vec::new();
    let mut mines = Vec::new();
    let mut charges = Vec::new();
    let mut experience_messages = Vec::new();
    let mut dfas = Vec::new();
    let mut collisions = super::stacking::StackingEffects::default();
    let rules = movement.fall;
    let ids: Vec<_> = world
        .btech
        .constructed_units()
        .iter()
        .filter_map(|(&id, unit)| {
            ((unit.airborne() || unit.free_fall.is_some() || unit.jump_stabilization > 0)
                && unit.orbital_drop.is_none()
                && unit.position().is_some()
                && world
                    .objects
                    .get(&id)
                    .is_some_and(|object| !object.flags.contains(Flag::Going)))
            .then_some(id)
        })
        .collect();
    let mut candidate = world.clone();
    let mut notices = Vec::new();
    let mut pilot_notices = Vec::new();
    for id in ids.iter().copied() {
        let unit = &candidate.btech.constructed_units()[&id];
        unit.validate()?;
        if unit.free_fall.is_some() {
            if unit.jump_stabilization > 0 {
                let unit = Arc::make_mut(&mut candidate.btech.constructed)
                    .get_mut(&id)
                    .unwrap();
                unit.jump_stabilization -= 1;
                if unit.jump_stabilization == 0 && unit.power() == BattlePower::Running {
                    notices.push(BattleNotice {
                        unit: id,
                        text: "You have finally stabilized after your jump.".to_owned(),
                    });
                }
            }
            notices.extend(super::free_fall::advance_unit(
                &mut candidate,
                id,
                rules,
                character,
                &mut falls,
                (&mut pilot_notices, notices.len()),
            )?);
            continue;
        }
        let Some(mut flight) = unit.flight else {
            let unit = Arc::make_mut(&mut candidate.btech.constructed)
                .get_mut(&id)
                .unwrap();
            unit.jump_stabilization -= 1;
            if unit.jump_stabilization == 0 && unit.power() == BattlePower::Running {
                notices.push(BattleNotice {
                    unit: id,
                    text: "You have finally stabilized after your jump.".to_owned(),
                });
            }
            continue;
        };
        notices.extend(super::charge_tracking::begin_update(
            &mut candidate,
            id,
            movement.charge,
        ));
        let unit = &candidate.btech.constructed_units()[&id];
        let mut rules = rules;
        rules.toughness = unit
            .pilot()
            .and_then(|pilot| candidate.btech.character_values().get(&pilot))
            .is_some_and(|values| super::advantages::enabled(values, "Toughness"));
        let mut position = unit.position().context("Airborne unit is not placed")?;
        let map = &candidate.btech.maps()[&position.map];
        flight.set_wrapping(map)?;
        let virtual_from = flight.virtual_sample().point;
        let step = flight.advance(unit.jump_capacity(map.gravity)?, map.movement_modifier)?;
        if step.outcome == BattleJumpOutcome::LostThrust {
            notices.push(BattleNotice {
                unit: id,
                text: "You lose control of your jump and fall!".to_owned(),
            });
            let fall = if character && candidate.objects[&id].flags.contains(Flag::InCharacter) {
                super::fall::resolve_character_fall(&mut candidate, id, 1, rules)?
            } else {
                super::resolve_fall(&mut candidate, id, 1, rules)?
            };
            fall.append_notices(id, &mut notices, &mut pilot_notices);
            falls.push(fall);
            continue;
        }
        let coordinate = step.to.point.containing_hex()?;
        if i64::from(coordinate.x) < 0
            || i64::from(coordinate.y) < 0
            || i64::from(coordinate.x) >= map.width
            || i64::from(coordinate.y) >= map.height
        {
            // Scenario reassignment can retain a destination beyond this map's ordinary edges.
            position.x = i64::from(coordinate.x).clamp(0, map.width - 1) as u16;
            position.y = i64::from(coordinate.y).clamp(0, map.height - 1) as u16;
            let point = super::BattleHexCoordinate {
                x: i32::from(position.x),
                y: i32::from(position.y),
            }
            .center();
            flight.relocate(
                point,
                f64::from(
                    step.to
                        .elevation
                        .clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16,
                ),
            );
            let unit = Arc::make_mut(&mut candidate.btech.constructed)
                .get_mut(&id)
                .unwrap();
            unit.position = Some(position);
            unit.hex_sync_pending = false;
            unit.motion
                .as_mut()
                .context("Airborne unit has no motion")?
                .point = point;
            unit.flight = Some(flight);
            notices.push(BattleNotice {
                unit: id,
                text: "You cannot move off this map!".into(),
            });
            let landing = finish_landing_inner(
                &mut candidate,
                id,
                false,
                super::BattleMovementRules {
                    fall: rules,
                    ..movement
                },
                character.then_some(&mut dfas),
                &mut falls,
                &mut collisions,
            )?;
            mines.extend(landing.mines);
            vehicle_falls.extend(landing.vehicle_falls);
            super::piloting::append_feedback(
                &mut pilot_notices,
                landing.pilot_notices,
                notices.len(),
            );
            notices.extend(landing.notices);
            experience_messages.extend(landing.experience_messages);
            notices.extend(super::charge_tracking::finish_update(
                &mut candidate,
                id,
                movement,
                character.then_some(&mut charges),
                (&mut pilot_notices, notices.len()),
            )?);
            continue;
        }
        let previous_tile = map.base_hex(i64::from(position.x), i64::from(position.y))?;
        let previous_elevation = unit.elevation_level(previous_tile);
        let next_elevation = (step.to.elevation + 0.5).trunc() as i32;
        if previous_tile.terrain == Terrain::Ice
            && step.outcome == BattleJumpOutcome::Landing
            && (coordinate.x, coordinate.y) == (i32::from(position.x), i32::from(position.y))
        {
            // Arrival in the current destination hex precedes vertical ice crossing checks.
            let unit = Arc::make_mut(&mut candidate.btech.constructed)
                .get_mut(&id)
                .unwrap();
            unit.motion
                .as_mut()
                .context("Airborne unit has no motion")?
                .point = step.to.point;
            unit.flight = Some(flight);
            let landing = finish_landing_inner(
                &mut candidate,
                id,
                false,
                super::BattleMovementRules {
                    fall: rules,
                    ..movement
                },
                character.then_some(&mut dfas),
                &mut falls,
                &mut collisions,
            )?;
            mines.extend(landing.mines);
            vehicle_falls.extend(landing.vehicle_falls);
            super::piloting::append_feedback(
                &mut pilot_notices,
                landing.pilot_notices,
                notices.len(),
            );
            notices.extend(landing.notices);
            experience_messages.extend(landing.experience_messages);
            if previous_elevation < -1
                && candidate.btech.maps()[&position.map]
                    .base_hex(i64::from(position.x), i64::from(position.y))?
                    .terrain
                    == Terrain::Ice
            {
                let resolve = if character {
                    super::surface_break::break_ice_upward_in_action
                } else {
                    super::surface_break::break_ice_upward
                };
                let report = resolve(&mut candidate, position.map, coordinate, id, rules)?;
                falls.extend(report.falls.into_iter().map(|(_, fall)| fall));
                vehicle_falls.extend(report.vehicle_falls.into_iter().map(|(_, fall)| fall));
                notices.extend(report.notices);
            }
            notices.extend(super::charge_tracking::finish_update(
                &mut candidate,
                id,
                movement,
                character.then_some(&mut charges),
                (&mut pilot_notices, notices.len()),
            )?);
            continue;
        }
        if previous_tile.terrain == Terrain::Ice
            && ((previous_elevation < -1 && next_elevation >= -1)
                || (previous_elevation >= -1 && next_elevation < -1))
        {
            let downward = next_elevation < -1;
            let old_coordinate = super::BattleHexCoordinate {
                x: i32::from(position.x),
                y: i32::from(position.y),
            };
            let fracture = if downward {
                let resolve = if character {
                    super::surface_break::break_ice_in_action
                } else {
                    super::break_ice
                };
                resolve(
                    &mut candidate,
                    position.map,
                    old_coordinate,
                    Some(id),
                    rules,
                )?
            } else {
                let resolve = if character {
                    super::surface_break::break_ice_upward_in_action
                } else {
                    super::surface_break::break_ice_upward
                };
                resolve(&mut candidate, position.map, old_coordinate, id, rules)?
            };
            falls.extend(fracture.falls.into_iter().map(|(_, fall)| fall));
            vehicle_falls.extend(fracture.vehicle_falls.into_iter().map(|(_, fall)| fall));
            super::piloting::append_feedback(
                &mut pilot_notices,
                fracture.pilot_notices,
                notices.len(),
            );
            notices.extend(fracture.notices);
            if downward && previous_tile.elevation > 0 {
                let events = if character {
                    let segment = super::motion::finish_interrupted_jump_in_action(
                        &mut candidate,
                        id,
                        (virtual_from, flight.virtual_sample().point),
                        rules,
                        &mut falls,
                    )?;
                    super::piloting::append_feedback(
                        &mut pilot_notices,
                        segment.pilot_notices,
                        notices.len(),
                    );
                    mines.extend(segment.mines);
                    vehicle_falls.extend(segment.vehicle_falls);
                    experience_messages.extend(segment.experience_messages);
                    segment.notices
                } else {
                    super::motion::finish_interrupted_jump(
                        &mut candidate,
                        id,
                        (virtual_from, flight.virtual_sample().point),
                        rules,
                    )?
                };
                notices.extend(events);
                let unit = &candidate.btech.constructed_units()[&id];
                if character && unit.position() != Some(position) && !unit.hex_sync_pending() {
                    experience_messages.extend(super::movement_experience::record_entry(
                        &mut candidate,
                        id,
                        rules.extended_piloting,
                    )?);
                }
                notices.extend(super::charge_tracking::finish_update(
                    &mut candidate,
                    id,
                    movement,
                    character.then_some(&mut charges),
                    (&mut pilot_notices, notices.len()),
                )?);
                continue;
            }
        }
        let unit = &candidate.btech.constructed_units()[&id];
        let map = &candidate.btech.maps()[&position.map];
        let previous_tile = map.base_hex(i64::from(position.x), i64::from(position.y))?;
        if previous_tile.strikes_bridge_during_jump((step.to.elevation + 0.5).trunc() as i32) {
            // Vertical integration precedes hex synchronization. Preserve both positions if interrupted.
            let unit = Arc::make_mut(&mut candidate.btech.constructed)
                .get_mut(&id)
                .unwrap();
            unit.motion
                .as_mut()
                .context("Airborne unit has no motion")?
                .point = step.to.point;
            unit.hex_sync_pending =
                (coordinate.x, coordinate.y) != (i32::from(position.x), i32::from(position.y));
            unit.flight = None;
            unit.ground_elevation = None;
            unit.jump_stabilization = 12;
            notices.push(BattleNotice {
                unit: id,
                text: "CRASH! You crash into the bridge!".to_owned(),
            });
            let fall = if character && candidate.objects[&id].flags.contains(Flag::InCharacter) {
                super::fall::resolve_character_fall(&mut candidate, id, 1, rules)?
            } else {
                super::resolve_fall(&mut candidate, id, 1, rules)?
            };
            fall.append_notices(id, &mut notices, &mut pilot_notices);
            falls.push(fall);
            continue;
        }
        let tile = map.base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
        let crossed_hex =
            (coordinate.x, coordinate.y) != (i32::from(position.x), i32::from(position.y));
        if crossed_hex && tile.blocks_jump_entry((step.to.elevation + 0.5).trunc() as i32) {
            // Reject the attempted transition before publishing position or flight progress.
            let previous_height = map
                .base_hex(i64::from(position.x), i64::from(position.y))?
                .surface_height();
            notices.push(BattleNotice {
                unit: id,
                text: "[bold]You attempt to jump over elevation that is too high![reset]"
                    .to_owned(),
            });
            let active_pilot = unit.pilot().is_some_and(|pilot| {
                candidate
                    .objects
                    .get(&pilot)
                    .is_some_and(|player| player.flags.contains(Flag::Connected))
            });
            let pilot = unit.pilot();
            let safe = if active_pilot {
                let mut check = super::roll_piloting(
                    &mut candidate,
                    id,
                    previous_height / 3,
                    rules.extended_piloting,
                )?;
                super::piloting::capture_feedback(
                    id,
                    pilot,
                    &check,
                    &mut notices,
                    &mut pilot_notices,
                );
                if character {
                    experience_messages.extend(super::piloting::award_control_check(
                        &mut candidate,
                        id,
                        &mut check,
                        rules.extended_piloting,
                    )?);
                }
                check.success
            } else {
                false
            };
            if safe {
                notices.push(BattleNotice {
                    unit: id,
                    text: "[bold]You land safely.[reset]".to_owned(),
                });
                let landing = finish_landing_inner(
                    &mut candidate,
                    id,
                    false,
                    super::BattleMovementRules {
                        fall: rules,
                        ..movement
                    },
                    character.then_some(&mut dfas),
                    &mut falls,
                    &mut collisions,
                )?;
                mines.extend(landing.mines);
                vehicle_falls.extend(landing.vehicle_falls);
                super::piloting::append_feedback(
                    &mut pilot_notices,
                    landing.pilot_notices,
                    notices.len(),
                );
                notices.extend(landing.notices);
                experience_messages.extend(landing.experience_messages);
            } else {
                notices.push(BattleNotice {
                    unit: id,
                    text: "[bold]You crash into the obstacle and fall from the sky![reset]"
                        .to_owned(),
                });
                let fall = if character && candidate.objects[&id].flags.contains(Flag::InCharacter)
                {
                    super::fall::resolve_character_fall(&mut candidate, id, 1, rules)?
                } else {
                    super::resolve_fall(&mut candidate, id, 1, rules)?
                };
                fall.append_notices(id, &mut notices, &mut pilot_notices);
                falls.push(fall);
                let input = super::stacking::physical_input(
                    &candidate,
                    id,
                    super::BattleStackingEntry::Fall,
                )?;
                notices.extend(resolve_airborne_stacking(
                    &mut candidate,
                    id,
                    input,
                    rules,
                    character.then_some(&mut collisions),
                    (&mut pilot_notices, notices.len()),
                )?);
            }
            notices.extend(super::charge_tracking::finish_update(
                &mut candidate,
                id,
                movement,
                character.then_some(&mut charges),
                (&mut pilot_notices, notices.len()),
            )?);
            continue;
        }
        position.x = u16::try_from(coordinate.x)?;
        position.y = u16::try_from(coordinate.y)?;
        let unit = Arc::make_mut(&mut candidate.btech.constructed)
            .get_mut(&id)
            .unwrap();
        unit.position = Some(position);
        unit.hex_sync_pending = false;
        unit.motion
            .as_mut()
            .context("Airborne unit has no motion")?
            .point = step.to.point;
        unit.flight = Some(flight);
        let identity = unit.identity();
        Arc::make_mut(&mut candidate.btech.units).insert(id, identity);
        if crossed_hex {
            notices.extend(super::hiding::movement(&mut candidate, id));
        }
        if step.outcome == BattleJumpOutcome::Airborne {
            if crossed_hex {
                notices.extend(flood_after_jump(
                    &mut candidate,
                    id,
                    rules,
                    character,
                    &mut falls,
                )?);
            }
            if crossed_hex && character {
                experience_messages.extend(super::movement_experience::record_entry(
                    &mut candidate,
                    id,
                    rules.extended_piloting,
                )?);
            }
            notices.extend(super::charge_tracking::finish_update(
                &mut candidate,
                id,
                movement,
                character.then_some(&mut charges),
                (&mut pilot_notices, notices.len()),
            )?);
            continue;
        }
        let landing = finish_landing_inner(
            &mut candidate,
            id,
            step.outcome == BattleJumpOutcome::LostThrust,
            super::BattleMovementRules {
                fall: rules,
                ..movement
            },
            character.then_some(&mut dfas),
            &mut falls,
            &mut collisions,
        )?;
        mines.extend(landing.mines);
        vehicle_falls.extend(landing.vehicle_falls);
        super::piloting::append_feedback(&mut pilot_notices, landing.pilot_notices, notices.len());
        notices.extend(landing.notices);
        experience_messages.extend(landing.experience_messages);
        if crossed_hex
            && character
            && candidate.btech.constructed_units()[&id].position() == Some(position)
            && !candidate.btech.constructed_units()[&id].hex_sync_pending()
        {
            experience_messages.extend(super::movement_experience::record_entry(
                &mut candidate,
                id,
                rules.extended_piloting,
            )?);
        }
        notices.extend(super::charge_tracking::finish_update(
            &mut candidate,
            id,
            movement,
            character.then_some(&mut charges),
            (&mut pilot_notices, notices.len()),
        )?);
    }
    for id in ids {
        let previous = world.btech.constructed_units()[&id].position().unwrap();
        let (notice, experience) = super::building_step::entered(&mut candidate, id, previous)?;
        notices.extend(notice);
        experience_messages.extend(experience);
    }
    super::towing::synchronize(&mut candidate)?;
    candidate.btech.validate(&candidate)?;
    let mut report = super::movement_report::MovementReport {
        pilot_notices,
        boundaries: Vec::new(),
        character_injuries: Vec::new(),
        vehicle_falls,
        experience_messages,
        dfas,
        mines,
        notices,
        falls,
        stacking: collisions,
        charges,
    };
    if !character {
        report.extend(super::orbital_drop_movement::advance_all(
            &mut candidate,
            movement,
            false,
        )?);
    }
    candidate.btech.validate(&candidate)?;
    *world = candidate;
    Ok(report)
}

/// Shared landing resolution at the current horizontal point; the caller owns rollback.
pub(super) fn finish_landing(
    world: &mut World,
    id: ObjectId,
    lost_thrust: bool,
    movement: super::BattleMovementRules,
) -> Result<Vec<BattleNotice>> {
    finish_landing_inner(
        world,
        id,
        lost_thrust,
        movement,
        None,
        &mut Vec::new(),
        &mut super::stacking::StackingEffects::default(),
    )
    .map(|landing| landing.notices)
}

/// Complete an early character landing while retaining every effect for its host checkpoint.
pub(super) fn finish_landing_in_action(
    world: &mut World,
    id: ObjectId,
    movement: super::BattleMovementRules,
    report: &mut super::movement_report::MovementReport,
) -> Result<()> {
    let landing = finish_landing_inner(
        world,
        id,
        false,
        movement,
        Some(&mut report.dfas),
        &mut report.falls,
        &mut report.stacking,
    )?;
    report.mines.extend(landing.mines);
    report.vehicle_falls.extend(landing.vehicle_falls);
    super::piloting::append_feedback(
        &mut report.pilot_notices,
        landing.pilot_notices,
        report.notices.len(),
    );
    report.notices.extend(landing.notices);
    report
        .experience_messages
        .extend(landing.experience_messages);
    Ok(())
}

/// Landing-local notices and XP; nested falls and collisions remain with their report owners.
struct LandingOutcome {
    vehicle_falls: Vec<super::BattleVehicleFallReport>,
    mines: Vec<super::BattleMineEventReport>,
    notices: Vec<BattleNotice>,
    pilot_notices: Vec<super::BattlePilotNotice>,
    experience_messages: Vec<super::BattleChannelMessage>,
}

/// Complete a landing while retaining nested character falls for the owning airborne action.
fn finish_landing_inner(
    world: &mut World,
    id: ObjectId,
    lost_thrust: bool,
    movement: super::BattleMovementRules,
    dfas: Option<&mut Vec<super::BattleDfaReport>>,
    falls: &mut Vec<super::BattleFallReport>,
    collisions: &mut super::stacking::StackingEffects,
) -> Result<LandingOutcome> {
    let character = dfas.is_some();
    let rules = movement.fall;
    let unit = &world.btech.constructed_units()[&id];
    // Tactical crew consciousness exists independently of an assigned player.
    let uncontrolled = super::crew::unit_unconscious(world, id);
    let target = unit.flight().and_then(|flight| flight.dfa_target());
    let action_stagger = unit.stagger().action_level() > 0;
    let pilot = unit.pilot();
    let mut notices = Vec::new();
    let mut pilot_notices = Vec::new();
    let mut experience_messages = Vec::new();
    let mut dfa = false;
    let mut vehicle_falls = Vec::new();
    if !lost_thrust
        && !uncontrolled
        && let Some(target) = target
    {
        let physical = movement.landing_physical();
        let profile = if character {
            super::dfa::dfa_profile_in_action(world, id, target, physical)
        } else {
            super::dfa_profile(world, id, target, physical)
        };
        match profile {
            Ok(_) => {
                if let Some(dfas) = dfas {
                    let report = super::dfa::resolve_dfa_in_action(world, id, target, physical)?;
                    super::piloting::append_feedback(
                        &mut pilot_notices,
                        report.pilot_notices.iter().cloned(),
                        notices.len(),
                    );
                    notices.extend(report.notices.iter().cloned());
                    dfas.push(report);
                } else {
                    let report = super::resolve_dfa(world, id, target, physical)?;
                    super::piloting::append_feedback(
                        &mut pilot_notices,
                        report.pilot_notices,
                        notices.len(),
                    );
                    notices.extend(report.notices);
                }
                dfa = true;
            }
            Err(error) => notices.push(BattleNotice {
                unit: id,
                text: format!("{error:#}"),
            }),
        }
    }
    let unit = Arc::make_mut(&mut world.btech.constructed)
        .get_mut(&id)
        .unwrap();
    unit.flight = None;
    if let Some(event) = &mut unit.free_fall {
        event.land();
    }
    if !lost_thrust && !dfa && !uncontrolled {
        notices.push(BattleNotice {
            unit: id,
            text: "You finish your jump.".to_owned(),
        });
    }

    let check_ice = if character {
        super::surface_break::check_ice_landing_in_action
    } else {
        super::surface_break::check_ice_landing
    };
    if !lost_thrust
        && !uncontrolled
        && let Some(fracture) = check_ice(world, id, rules)?
    {
        falls.extend(fracture.falls.into_iter().map(|(_, fall)| fall));
        vehicle_falls.extend(fracture.vehicle_falls.into_iter().map(|(_, fall)| fall));
        super::piloting::append_feedback(&mut pilot_notices, fracture.pilot_notices, notices.len());
        notices.extend(fracture.notices);
    }
    let mut fall = lost_thrust || uncontrolled;
    let mut landing_failure = None;
    let mut stagger_failed = false;
    if !lost_thrust && !uncontrolled && action_stagger {
        notices.push(BattleNotice {
            unit: id,
            text: "The damage you've taken makes the landing a bit harder...".into(),
        });
        let modifier = super::stagger::action_modifier(&world.btech.constructed_units()[&id]);
        let mut check = super::roll_piloting(world, id, modifier, rules.extended_piloting)?;
        super::piloting::capture_feedback(id, pilot, &check, &mut notices, &mut pilot_notices);
        if character {
            experience_messages.extend(super::piloting::award_control_check(
                world,
                id,
                &mut check,
                rules.extended_piloting,
            )?);
        }
        stagger_failed = !check.success;
        fall = stagger_failed;
        landing_failure = stagger_failed.then_some((
            "... something you apparently can't handle!",
            "lands, staggers, and falls down!",
        ));
    }
    let unit = &world.btech.constructed_units()[&id];
    if !lost_thrust && !fall && !unit.is_destroyed() && unit.mobility().piloting_modifier > 0 {
        let missing_leg = unit.unavailable_legs() > 0;
        let gyro_modifier = if unit.gyro_damage() > 0 { 3 } else { 0 };
        let (warning, failure, observed) = if missing_leg {
            (
                "Your missing leg makes it harder to land",
                "Your missing leg has caused you to fall upon landing!",
                "lands, unbalanced, and falls down!",
            )
        } else if unit.mobility().piloting_modifier > gyro_modifier {
            (
                "Your damaged leg actuators make it harder to land",
                "Your damaged leg actuators have caused you to fall upon landing!",
                "lands, stumbles, and falls down!",
            )
        } else {
            (
                "Your damaged gyro makes it harder to land",
                "Your damaged gyro has caused you to fall upon landing!",
                "lands, twists awkwardly, and falls down!",
            )
        };
        notices.push(BattleNotice {
            unit: id,
            text: warning.to_owned(),
        });
        let mut check = super::roll_piloting(world, id, 0, rules.extended_piloting)?;
        super::piloting::capture_feedback(id, pilot, &check, &mut notices, &mut pilot_notices);
        if character {
            experience_messages.extend(super::piloting::award_control_check(
                world,
                id,
                &mut check,
                rules.extended_piloting,
            )?);
        }
        fall = !check.success;
        landing_failure = fall.then_some((failure, observed));
    }
    if fall && !world.btech.constructed_units()[&id].is_destroyed() {
        notices.push(BattleNotice {
            unit: id,
            text: landing_failure
                .map(|(failure, _)| failure)
                .unwrap_or(if uncontrolled {
                    "Your lack of conciousness makes you fall to the ground. Not like you can read this anyway."
                } else {
                    "You lose control of your jump and fall!"
                })
                .to_owned(),
        });
        if let Some((_, observed)) = landing_failure {
            notices.extend(super::broadcast::observer_notices(world, id, observed));
        }
        let report = if character && world.objects[&id].flags.contains(Flag::InCharacter) {
            super::fall::resolve_character_fall(world, id, 1, rules)?
        } else {
            super::resolve_fall(world, id, 1, rules)?
        };
        report.append_notices(id, &mut notices, &mut pilot_notices);
        falls.push(report);
    }
    // A stagger failure terminates landing; its shared fall already handles immersion and mines.
    if stagger_failed {
        // Flight was cleared for surface resolution; retain the shared airborne fall's stabilization.
        let unit = Arc::make_mut(&mut world.btech.constructed)
            .get_mut(&id)
            .unwrap();
        if !dfa && !unit.is_destroyed() {
            unit.jump_stabilization = 12;
        }
        return Ok(LandingOutcome {
            vehicle_falls,
            mines: Vec::new(),
            notices,
            pilot_notices,
            experience_messages,
        });
    }
    let unit = &world.btech.constructed_units()[&id];
    if !dfa && !fall && !unit.is_destroyed() && unit.posture() != super::BattlePosture::Prone {
        let input = super::stacking::physical_input(world, id, super::BattleStackingEntry::Jump)?;
        let collision = resolve_airborne_stacking(
            world,
            id,
            input,
            rules,
            character.then_some(collisions),
            (&mut pilot_notices, notices.len()),
        )?;
        if collision.is_empty() {
            notices.extend(super::broadcast::observer_notices(
                world,
                id,
                "lands gracefully.",
            ));
        }
        notices.extend(collision);
    }
    let mut mines = Vec::new();
    if !dfa && !fall && !world.btech.constructed_units()[&id].is_destroyed() {
        let event = super::mine_event::resolve(
            world,
            id,
            super::BattleMineTriggerReason::Land,
            rules,
            character,
        )?;
        super::piloting::append_feedback(
            &mut pilot_notices,
            event.pilot_notices.iter().cloned(),
            notices.len(),
        );
        notices.extend(event.notices.iter().cloned());
        mines.push(event);
    }
    if !world.btech.constructed_units()[&id].is_destroyed() {
        notices.extend(flood_after_jump(world, id, rules, character, falls)?);
    }
    let unit = Arc::make_mut(&mut world.btech.constructed)
        .get_mut(&id)
        .unwrap();
    if !dfa && !unit.is_destroyed() {
        unit.jump_stabilization = 12;
    }
    // Only completed traditional landing stops the separate action-time stagger check.
    // The early stagger-failure return above deliberately retains its scalar.
    if rules.stagger == super::BattleStaggerMode::Traditional {
        unit.stagger.action_damage = 0;
    }
    Ok(LandingOutcome {
        vehicle_falls,
        mines,
        notices,
        pilot_notices,
        experience_messages,
    })
}

/// Select collision publication capability without changing ordinary movement callers.
fn resolve_airborne_stacking(
    world: &mut World,
    id: ObjectId,
    input: super::BattleStackingInput,
    rules: super::BattleFallRules,
    effects: Option<&mut super::stacking::StackingEffects>,
    feedback: (&mut Vec<super::BattlePilotNotice>, usize),
) -> Result<Vec<BattleNotice>> {
    if let Some(effects) = effects {
        return super::stacking::resolve_in_action(
            world,
            id,
            input,
            rules.stacking,
            rules,
            effects,
            feedback,
        );
    }
    super::resolve_stacking(world, id, input, rules.stacking, rules)
}

/// Apply immersion and preserve support-loss falls for private injury publication.
fn flood_after_jump(
    world: &mut World,
    id: ObjectId,
    rules: super::BattleFallRules,
    character: bool,
    falls: &mut Vec<super::BattleFallReport>,
) -> Result<Vec<BattleNotice>> {
    let reports = if character {
        super::flooding::flood_unit_in_action(world, id, rules)?
    } else {
        super::flood_unit(world, id, rules)?
    };
    let mut notices = Vec::new();
    for report in reports {
        notices.extend(report.notices);
        if let Some(fall) = report.fall {
            falls.push(fall);
        }
    }
    notices.extend(super::extinguish_inferno_in_water(world, id)?);
    Ok(notices)
}

/// Native launch uses the same owned mutation as Lua and restores the enclosing command on error.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| -> Result<()> {
        let id = ctx.scripts.world.borrow().objects[&ctx.player]
            .location
            .context("Enter a unit first")?;
        launch_action(
            ctx.scripts,
            ctx.config,
            id,
            ctx.player,
            JumpRequest::Text(&input.args),
        )?;
        Ok(())
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
