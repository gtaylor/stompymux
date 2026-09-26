//! Scenario map reassignment retains live controls while replacing shared battlefield membership.
use super::*;
use crate::{Flag, Kind, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Assigned position and identity, with the reference's out-of-bounds origin-reset diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleMapAssignment {
    pub position: BattlePosition,
    pub label: String,
    pub reset_origin: bool,
}

/// Trusted scenario reassignment; callers own wizard authority and publication.
/// Invalid destination, capacity, containment or resulting state leaves the world and dice unchanged.
pub fn reassign_map(
    world: &mut World,
    id: ObjectId,
    map: ObjectId,
    preferred: Option<&str>,
) -> Result<BattleMapAssignment> {
    world.attempt(|world| {
        let report = reassign_in_candidate(world, id, map, preferred)?;
        world.btech.validate(world)?;
        Ok(report)
    })
}

/// Change membership inside the host's transaction, retaining the unit's physical condition.
pub(super) fn reassign_in_candidate(
    world: &mut World,
    id: ObjectId,
    map: ObjectId,
    preferred: Option<&str>,
) -> Result<BattleMapAssignment> {
    ensure!(world.objects.get(&id).is_some_and(|object| object.kind == Kind::Thing
        && !object.flags.contains(Flag::Going)), "Unit is unavailable");
    let source = super::scanner::scanner_unit(world, id).context("Unit is not constructed")?;
    let original_flight = world
        .btech
        .constructed_units()
        .get(&id)
        .and_then(BattleUnit::flight);
    let original = world
        .btech
        .vehicles()
        .get(&id)
        .and_then(BattleVehicle::retained_position)
        .or_else(|| {
            world
                .btech
                .constructed_units()
                .get(&id)
                .and_then(|unit| unit.position)
        });
    if let Some(position) = source.position {
        ensure!(
            world.btech.maps().contains_key(&position.map)
                && world
                    .objects
                    .get(&position.map)
                    .is_some_and(|object| !object.flags.contains(Flag::Going)),
            "Current map index is invalid!"
        );
    }
    ensure!(
        world
            .objects
            .get(&map)
            .is_some_and(|object| matches!(object.kind, Kind::Room | Kind::Thing)
                && !object.flags.contains(Flag::Going)),
        "Invalid map index!"
    );
    let destination = world.btech.maps().get(&map).context("Invalid map index!")?;
    let (mut x, mut y) = original.map_or((0, 0), |position| (position.x, position.y));
    let reset_origin = destination.base_hex(i64::from(x), i64::from(y)).is_err();
    if reset_origin {
        x = 0;
        y = 0;
    }
    let tile = destination.base_hex(i64::from(x), i64::from(y))?;
    let members = super::map_slots::all_unit_order(world, map)?;
    ensure!(
        members.len() < 250 || members.contains(&id),
        "There are too many mechs on that map!"
    );
    world.validate_move(id, map)?;
    let slot = super::map_slots::placement_slot(world, id, map)?;
    let position = BattlePosition { map, x, y };
    let point = BattleHexCoordinate {
        x: i32::from(x),
        y: i32::from(y),
    }
    .center();
    let mut motion = world
        .btech
        .vehicles()
        .get(&id)
        .and_then(BattleVehicle::motion)
        .or_else(|| {
            world
                .btech
                .constructed_units()
                .get(&id)
                .and_then(BattleUnit::motion)
        })
        .unwrap_or_else(|| BattleMotion::stationary(point));
    if reset_origin {
        motion.point = point;
    }
    let height = retained_height(world, id)?
        .map(|height| height.clamp(i32::from(i16::MIN), i32::from(i16::MAX)));
    // Removing one member breaks a tow rather than transporting another unit implicitly.
    let towed = world.btech.towed_by(id);
    let carrier = towed.unwrap_or(id);
    super::towing::detach(world, carrier);
    if towed.is_some() {
        motion.stop_translation();
        motion.desired_heading = motion.heading;
    }
    super::map_slots::arrive(&mut world.btech, id, position.map, slot);
    super::contacts::forget_unit(world, id);
    let identity = if let Some(unit) = world.btech.vehicles.get_mut(&id) {
        unit.assign_membership(position, slot);
        unit.motion = Some(motion);
        if unit.vtol_flight.is_none() && unit.free_fall.is_none() && unit.orbital_drop.is_none() {
            unit.ground_elevation = height.map(f64::from);
        }
        unit.update_motion(
            motion,
            position,
            unit.definition().movement == BattleVehicleMovement::Hover
                && tile.terrain == Terrain::Bridge
                && height.is_some_and(|height| height < i32::from(tile.surface_height())),
        );
        unit.identity()
    } else {
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        unit.detached = false;
        unit.position = Some(position);
        unit.map_slot = Some(slot);
        unit.battlefield_label = None;
        unit.motion = Some(motion);
        unit.c3_network = None;
        unit.c3i_network = None;
        unit.tag.target = None;
        if unit.flight.is_none() && unit.free_fall.is_none() && unit.orbital_drop.is_none() {
            unit.ground_elevation = height.map(f64::from);
        }
        unit.identity()
    };
    world.btech.units.insert(id, identity);
    world.objects.get_mut(&id).unwrap().location = Some(map);
    // A bounds reset updates the active flight sample as well as the ordinary motion cursor.
    if reset_origin || original.is_none() {
        super::scenario_position::relocate(world, id, position, tile, height)?;
    }
    if let Some(mut flight) = original_flight {
        let point = world.btech.constructed_units()[&id]
            .motion()
            .context("Airborne unit has no motion")?
            .point;
        if flight.rebind(&world.btech.maps()[&map], point)? {
            world.btech.constructed.get_mut(&id).unwrap().flight = Some(flight);
        }
    }
    let label = super::battlefield_identity::assign_in_candidate(world, id, preferred)?;
    Ok(BattleMapAssignment {
        position,
        label,
        reset_origin,
    })
}

/// Current altitude remains available while map membership is absent.
fn retained_height(world: &World, id: ObjectId) -> Result<Option<i32>> {
    if super::scanner::scanner_unit(world, id).is_some_and(|unit| unit.position.is_some()) {
        return super::unit_elevation(world, id);
    }
    if let Some(unit) = world.btech.vehicles().get(&id) {
        if !unit.detached {
            return Ok(None);
        }
        return Ok(unit
            .vtol_flight()
            .map(|flight| flight.altitude as i32)
            .or_else(|| unit.orbital_drop().map(|drop| drop.elevation()))
            .or_else(|| unit.free_fall().map(|fall| fall.elevation()))
            .or_else(|| unit.ground_elevation.map(|height| height as i32)));
    }
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit is not constructed")?;
    Ok(unit
        .flight()
        .map(|flight| flight.sample().elevation as i32)
        .or_else(|| unit.orbital_drop().map(|drop| drop.elevation()))
        .or_else(|| unit.free_fall().map(|fall| fall.elevation()))
        .or_else(|| unit.ground_elevation.map(|height| height as i32)))
}

/// Remove tactical membership immediately; retain physical pose, power and crew until an update.
/// The enclosing object stays in its existing container. Failure restores all world state.
pub fn remove_map_membership(world: &mut World, id: ObjectId) -> Result<()> {
    world.attempt(|world| {
        ensure!(
            world.objects.get(&id).is_some_and(
                |object| object.kind == Kind::Thing && !object.flags.contains(Flag::Going)
            ),
            "Unit is unavailable"
        );
        let source = super::scanner::scanner_unit(world, id).context("Unit is not constructed")?;
        if let Some(position) = source.position {
            ensure!(
                world.btech.maps().contains_key(&position.map)
                    && world
                        .objects
                        .get(&position.map)
                        .is_some_and(|object| !object.flags.contains(Flag::Going)),
                "Current map index is invalid!"
            );
        }
        let height = retained_height(world, id)?;
        let carrier = world.btech.towed_by(id).unwrap_or(id);
        super::towing::detach(world, carrier);
        super::map_slots::depart(&mut world.btech, id);
        super::contacts::forget_unit(world, id);
        let identity = if let Some(unit) = world.btech.vehicles.get_mut(&id) {
            if unit.vtol_flight.is_none() && unit.free_fall.is_none() && unit.orbital_drop.is_none()
            {
                unit.ground_elevation = height.map(f64::from);
            }
            unit.detach_scenario_membership();
            unit.identity()
        } else {
            let unit = world.btech.constructed.get_mut(&id).unwrap();
            if unit.flight.is_none() && unit.free_fall.is_none() && unit.orbital_drop.is_none() {
                unit.ground_elevation = height.map(f64::from);
            }
            unit.detached = unit.position.is_some();
            unit.map_slot = None;
            unit.c3_network = None;
            unit.c3i_network = None;
            unit.tag.target = None;
            unit.hull_down = Default::default();
            unit.building_entry = None;
            unit.identity()
        };
        world.btech.units.insert(id, identity);
        world.btech.validate(world)?;
        Ok(())
    })
}

/// Resolve powered off-map state at the next simulation update using normal shutdown cleanup.
/// There is no destination terrain on which to resolve a fall or landing impact.
pub(super) fn advance_detached(world: &mut World) -> Vec<BattleNotice> {
    let mut notices = Vec::new();
    let mut dropped_clubs = Vec::new();
    for (&id, unit) in world.btech.constructed.iter_mut() {
        if !unit.detached
            || (unit.power == BattlePower::Off
                && unit.flight.is_none()
                && unit.free_fall.is_none()
                && unit.orbital_drop.is_none())
        {
            continue;
        }
        notices.push(BattleNotice {
            unit: id,
            text: "You are on an invalid map! Map index reset!".into(),
        });
        if let Some(flight) = unit.flight.take() {
            unit.ground_elevation = Some(flight.sample().elevation);
        }
        if let Some(drop) = unit.orbital_drop.take() {
            unit.ground_elevation = Some(f64::from(drop.elevation()));
        }
        if let Some(fall) = unit.free_fall.take() {
            unit.ground_elevation = Some(f64::from(fall.elevation()));
        }
        shutdown_notice(&mut notices, id, unit.power);
        if super::power::finish_shutdown(unit, id, &mut notices) {
            dropped_clubs.push(id);
        }
        unit.facing.torso = BattleTorso::Center;
    }
    for (&id, unit) in world.btech.vehicles.iter_mut() {
        if !unit.detached
            || (unit.power == BattlePower::Off
                && unit.orbital_drop.is_none()
                && unit.free_fall().is_none()
                && unit
                    .vtol_flight()
                    .is_none_or(|flight| flight.phase == BattleVtolFlightPhase::Landed))
        {
            continue;
        }
        notices.push(BattleNotice {
            unit: id,
            text: "You are on an invalid map! Map index reset!".into(),
        });
        if let Some(drop) = unit.orbital_drop.take() {
            unit.ground_elevation = Some(f64::from(drop.elevation()));
        }
        if let Some(fall) = unit.free_fall.take() {
            unit.ground_elevation = Some(f64::from(fall.elevation()));
        }
        if let Some(flight) = &mut unit.vtol_flight {
            flight.phase = BattleVtolFlightPhase::Landed;
            flight.vertical_speed = 0.0;
            flight.fall = None;
        }
        shutdown_notice(&mut notices, id, unit.power);
        super::vehicle_power::finish_shutdown(unit);
    }
    for id in dropped_clubs {
        notices.extend(super::club::dropped_notices(world, id));
    }
    notices
}

/// Preserve the ordinary shutdown confirmation for a deferred off-map transition.
fn shutdown_notice(notices: &mut Vec<BattleNotice>, id: ObjectId, power: BattlePower) {
    let text = match power {
        BattlePower::Off => return,
        BattlePower::Starting { .. } => "The startup sequence has been aborted.",
        BattlePower::Running => "All systems shut down.",
    };
    notices.push(BattleNotice {
        unit: id,
        text: text.into(),
    });
}

/// Native/Lua result distinguishes removal from assignment without inventing a destination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleMapIndexReport {
    pub assignment: Option<BattleMapAssignment>,
}

/// Wizard SETMAPINDX transaction includes state, durable dice and all private confirmations.
pub fn set_map_index_action(
    scripts: &crate::Scripts,
    config: &crate::Config,
    actor: ObjectId,
    unit: ObjectId,
    map: ObjectId,
    preferred: Option<&str>,
) -> Result<BattleMapIndexReport> {
    scripts.atomic(|before| {
        ensure!(
            crate::authority::is_wizard(&before, actor),
            "Permission denied."
        );
        ensure!(map.0 >= -1, "Invalid map index!");
        let assignment = if map.0 == -1 {
            remove_map_membership(&mut scripts.world_mut(), unit)?;
            None
        } else {
            Some(reassign_map(
                &mut scripts.world_mut(),
                unit,
                map,
                preferred,
            )?)
        };
        let messages = match &assignment {
            None => vec!["Mech removed from map.".into()],
            Some(report) => {
                let mut messages = Vec::new();
                if report.reset_origin {
                    messages.push(
                        "You're current position is out of bounds, Pos changed to 0,0".into(),
                    );
                }
                messages.push(format!("MapIndex changed to {}", map.0));
                messages.push(format!("Your ID: {}", report.label));
                messages
            }
        };
        scripts.world().validate(config)?;
        for message in messages {
            super::notify_message(scripts, BattleMessageTarget::Player(actor), &message)?;
        }
        scripts.effects.validate()?;
        Ok(BattleMapIndexReport { assignment })
    })
}

/// SETMAPINDX accepts a decimal map dbref (-1 removes membership) and an optional preferred ID.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let args: Vec<_> = input.args.split_whitespace().take(2).collect();
        ensure!(
            !args.is_empty(),
            "Invalid number of arguments to SETMAPINDX!"
        );
        let map = ObjectId(args[0].parse::<i64>().context("Invalid map index!")?);
        let unit = ctx
            .scripts
            .world()
            .objects
            .get(&ctx.player)
            .and_then(|actor| actor.location)
            .context("Player has no location")?;
        set_map_index_action(
            ctx.scripts,
            ctx.config,
            ctx.player,
            unit,
            map,
            args.get(1).copied(),
        )
    })();
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
