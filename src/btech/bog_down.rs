//! Bogging down in soft ground (Tactical Operations p. 60): swamp, mud, deep snow, tundra and
//! sand can trap a 'Mech or ground vehicle as it enters. A stuck unit cannot move or turn,
//! attackers find it two easier to hit, and at the start of every turn its crew rolls to pull
//! free. Jumping out always works, and a 'Mech that falls in such a hex is stuck at once.
use super::{GroundMovement, Notice, PilotNotice};
use crate::{Config, Flag, ObjectId, Scripts, World};
use anyhow::{Result, ensure};

/// How a bog-down roll went; the caller publishes the notices.
#[derive(Default)]
pub(super) struct BogDownReport {
    pub notices: Vec<Notice>,
    pub pilot_notices: Vec<PilotNotice>,
    pub experience_messages: Vec<super::DiagnosticMessage>,
    pub stuck: bool,
}

/// Where a unit stands: its hex, its level there and how it moves over the ground.
fn footing(world: &World, id: ObjectId) -> Option<(super::Hex, i32, GroundMovement)> {
    if let Some(unit) = world.btech.constructed_units().get(&id) {
        let position = unit.position()?;
        let tile = world
            .btech
            .maps()
            .get(&position.map)?
            .base_hex(i64::from(position.x), i64::from(position.y))
            .ok()?;
        return Some((tile, unit.elevation_level(tile), GroundMovement::Legged));
    }
    let vehicle = world.btech.vehicles().get(&id)?;
    let position = vehicle.position()?;
    let tile = world
        .btech
        .maps()
        .get(&position.map)?
        .base_hex(i64::from(position.x), i64::from(position.y))
        .ok()?;
    Some((
        tile,
        vehicle.elevation_level(tile),
        super::vehicle_motion::ground_movement(vehicle.definition().movement),
    ))
}

/// Whether `id` is stuck in soft ground.
pub(super) fn is_stuck(world: &World, id: ObjectId) -> bool {
    world.btech.constructed_units().get(&id).map_or_else(
        || {
            world
                .btech
                .vehicles()
                .get(&id)
                .is_some_and(|vehicle| vehicle.bogged_down)
        },
        |unit| unit.bogged_down,
    )
}

/// Movement controls are refused while the unit is stuck.
pub(super) fn require_free(world: &World, id: ObjectId) -> Result<()> {
    ensure!(
        !is_stuck(world, id),
        "You are stuck fast in the {}; wait for your next chance to pull free.",
        footing(world, id)
            .map_or("ground", |(tile, ..)| tile.terrain().label())
            .to_ascii_lowercase()
    );
    Ok(())
}

/// Mark `id` stuck and bring it to a halt.
pub(super) fn stick(world: &mut World, id: ObjectId) {
    super::with_unit_mut!(world.btech.unit_mut(id).unwrap(), |unit| {
        unit.bogged_down = true;
        if let Some(motion) = unit.motion.as_mut() {
            motion.stop_translation();
            motion.desired_heading = motion.heading;
        }
    });
}

/// Release `id`, as jumping clear of the ground does.
pub(super) fn free(world: &mut World, id: ObjectId) {
    if let Some(unit) = world.btech.unit_mut(id) {
        super::with_unit_mut!(unit, |unit| unit.bogged_down = false);
    }
}

/// Attacks against a stuck unit are two easier.
pub(super) fn target_modifier(world: &World, target: ObjectId) -> i8 {
    if is_stuck(world, target) { -2 } else { 0 }
}

/// Roll against getting stuck as `id` enters the soft ground it now stands on, or `None` when
/// the ground cannot hold it. `automatic` sticks it without a roll, as jumping into a swamp does.
pub(super) fn enter(
    world: &mut World,
    id: ObjectId,
    extended: bool,
    character: bool,
    automatic: bool,
) -> Result<Option<BogDownReport>> {
    let Some((tile, level, movement)) = footing(world, id) else {
        return Ok(None);
    };
    let destroyed = super::with_unit!(world.btech.unit(id).unwrap(), |unit| unit.is_destroyed());
    if destroyed || !tile.can_bog_down(movement, level) {
        return Ok(None);
    }
    let Some(modifier) = tile.bog_down_modifier(movement) else {
        return Ok(None);
    };
    let name = tile.terrain().label().to_ascii_lowercase();
    if automatic {
        let mut report = BogDownReport {
            stuck: true,
            ..BogDownReport::default()
        };
        report.notices.push(Notice {
            unit: id,
            text: format!("You sink into the {name} and stick fast!"),
        });
        stick(world, id);
        return Ok(Some(report));
    }
    roll_to_stick(world, id, modifier, &name, extended, character).map(Some)
}

/// Roll at `modifier` against sticking in the `name`d ground the unit has just come down in.
pub(super) fn roll_to_stick(
    world: &mut World,
    id: ObjectId,
    modifier: i16,
    name: &str,
    extended: bool,
    character: bool,
) -> Result<BogDownReport> {
    let mut report = BogDownReport::default();
    report.notices.push(Notice {
        unit: id,
        text: format!("The {name} drags at you as you push through it."),
    });
    let control = super::terrain_control::check(world, id, modifier, extended, character)?;
    control.capture_feedback(id, &mut report.notices, &mut report.pilot_notices);
    report
        .experience_messages
        .extend(control.experience_messages);
    if control.success {
        return Ok(report);
    }
    report.stuck = true;
    report.notices.push(Notice {
        unit: id,
        text: format!("You bog down in the {name} and stick fast!"),
    });
    report.notices.extend(super::broadcast::observer_notices(
        world,
        id,
        &format!("bogs down in the {name}!"),
    ));
    stick(world, id);
    Ok(report)
}

/// At the start of a turn, every stuck unit rolls to pull free.
fn advance(world: &mut World, config: &Config) -> Result<(Vec<Notice>, Vec<PilotNotice>)> {
    let mut notices = Vec::new();
    let mut pilot_notices = Vec::new();
    if !world.btech.turn_clock.starts_turn() {
        return Ok((notices, pilot_notices));
    }
    let extended = config.battletech.extended_piloting != 0;
    let ids: Vec<_> = world
        .btech
        .constructed_units()
        .keys()
        .chain(world.btech.vehicles().keys())
        .copied()
        .filter(|&id| {
            is_stuck(world, id)
                && world
                    .objects
                    .get(&id)
                    .is_some_and(|object| !object.flags.contains(Flag::Going))
        })
        .collect();
    for id in ids {
        let (running, destroyed) = super::with_unit!(world.btech.unit(id).unwrap(), |unit| {
            (unit.power() == super::Power::Running, unit.is_destroyed())
        });
        if !running || destroyed {
            continue;
        }
        let Some((tile, _, movement)) = footing(world, id) else {
            free(world, id);
            continue;
        };
        let name = tile.terrain().label().to_ascii_lowercase();
        let Some(modifier) = tile.bog_down_modifier(movement) else {
            // The ground changed under the unit; nothing holds it now.
            free(world, id);
            continue;
        };
        notices.push(Notice {
            unit: id,
            text: format!("You strain to pull free of the {name}."),
        });
        let control = super::terrain_control::check(world, id, modifier, extended, true)?;
        control.capture_feedback(id, &mut notices, &mut pilot_notices);
        if control.success {
            free(world, id);
            notices.push(Notice {
                unit: id,
                text: format!("You haul yourself clear of the {name}."),
            });
            notices.extend(super::broadcast::observer_notices(
                world,
                id,
                &format!("pulls free of the {name}."),
            ));
        } else {
            notices.push(Notice {
                unit: id,
                text: format!("You remain stuck in the {name}."),
            });
        }
    }
    Ok((notices, pilot_notices))
}

/// Give every stuck unit its start-of-turn chance to pull free, publishing the rolls.
pub fn advance_bog_down_action(scripts: &Scripts, config: &Config) -> Result<()> {
    scripts.atomic(|_| {
        let (notices, pilot_notices) = advance(&mut scripts.world_mut(), config)?;
        super::piloting::publish_ordered_notices(scripts, &notices, &pilot_notices)?;
        scripts.world().validate_action(config)?;
        Ok(())
    })
}

/// Whether any unit is stuck, so the turn boundary has rolls to make even on a quiet field.
pub(crate) fn pending(world: &World) -> bool {
    world
        .btech
        .constructed_units()
        .values()
        .any(|unit| unit.bogged_down)
        || world
            .btech
            .vehicles()
            .values()
            .any(|vehicle| vehicle.bogged_down)
}

/// Stick a 'Mech that has fallen in soft ground.
pub(super) fn stick_after_fall(world: &mut World, id: ObjectId) -> Option<Notice> {
    let (tile, level, movement) = footing(world, id)?;
    if !tile.can_bog_down(movement, level) || is_stuck(world, id) {
        return None;
    }
    stick(world, id);
    Some(Notice {
        unit: id,
        text: format!(
            "You are stuck fast in the {}!",
            tile.terrain().label().to_ascii_lowercase()
        ),
    })
}
