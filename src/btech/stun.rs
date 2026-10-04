//! Ten-second BattleMech cockpit stun, independent of player unconsciousness.
use super::{Mech, Notice, Power};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};

impl Mech {
    /// Remaining committed seconds of cockpit stun; zero allows weapon operation.
    pub fn stun_remaining(&self) -> u8 {
        self.stun_remaining
    }
}

/// Apply or refresh cockpit stun and reduce a currently running forward throttle to walking speed.
pub fn stun_unit(world: &mut World, id: ObjectId) -> Result<Notice> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Unit is unavailable"
    );
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit construction state is unavailable")?;
    ensure!(!unit.is_destroyed(), "Unit is already destroyed");
    let walking = unit.movement_maximum_speed() * 2.0 / 3.0;
    let unit = world.btech.constructed.get_mut(&id).unwrap();
    unit.stun_remaining = 10;
    if let Some(motion) = &mut unit.motion
        && motion.speed > walking
    {
        motion.desired_speed = walking;
    }
    Ok(Notice {
        unit: id,
        text: "The cockpit violently shakes from a grazing blow! You are momentarily stunned!"
            .to_owned(),
    })
}

/// Expire stun even during shutdown; only operational units announce recovery.
pub fn advance_stun(world: &mut World) -> Vec<Notice> {
    let ids: Vec<_> = world
        .btech
        .constructed_units()
        .iter()
        .filter(|(id, unit)| {
            unit.stun_remaining > 0
                && world
                    .objects
                    .get(id)
                    .is_some_and(|object| !object.flags.contains(Flag::Going))
        })
        .map(|(&id, _)| id)
        .collect();
    let mut notices = Vec::new();
    for id in ids {
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        unit.stun_remaining -= 1;
        if unit.stun_remaining == 0 && unit.power() == Power::Running && !unit.is_destroyed() {
            notices.push(Notice {
                unit: id,
                text: "You recover from your stunning experience!".to_owned(),
            });
        }
    }
    notices
}
