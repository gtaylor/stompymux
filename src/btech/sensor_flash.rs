//! Reactor flashes temporarily blind susceptible primary sensors without erasing acquired contacts.
use crate::{ObjectId, World};
use std::sync::Arc;

impl super::BattleUnit {
    /// Seconds until vision returns, independently of power or map placement.
    pub fn blinded_remaining(&self) -> u8 {
        self.blinded_remaining
    }
}

impl super::BattleVehicle {
    /// Seconds until vision returns, shared by ground vehicles and rotorcraft.
    pub fn blinded_remaining(&self) -> u8 {
        self.blinded_remaining
    }
}

/// Shared observer and cockpit gate; unknown objects have no temporary sensor condition.
pub fn battle_unit_blinded(world: &World, id: ObjectId) -> bool {
    world
        .btech
        .constructed_units()
        .get(&id)
        .map(|u| u.blinded_remaining())
        .or_else(|| {
            world
                .btech
                .vehicles()
                .get(&id)
                .map(|u| u.blinded_remaining())
        })
        .is_some_and(|remaining| remaining > 0)
}

/// Keep mutable anatomy access separate from flash admission and timer behavior.
fn remaining_mut(world: &mut World, id: ObjectId) -> &mut u8 {
    if world.btech.vehicles().contains_key(&id) {
        return &mut Arc::make_mut(&mut world.btech.vehicles)
            .get_mut(&id)
            .unwrap()
            .blinded_remaining;
    }
    &mut Arc::make_mut(&mut world.btech.constructed)
        .get_mut(&id)
        .unwrap()
        .blinded_remaining
}

/// Capture susceptible observers before applying any flashes, preserving simultaneous visibility.
pub(super) fn scramble(
    world: &mut World,
    source: ObjectId,
    visibility: &World,
) -> Vec<super::BattleNotice> {
    let notices: Vec<_> = super::scanner::scanner_ids(world)
        .into_iter()
        .filter_map(|id| {
            if id == source
                || battle_unit_blinded(world, id)
                || super::crew::unit_unconscious(world, id)
                || world
                    .objects
                    .get(&id)
                    .is_none_or(|o| o.flags.contains(crate::Flag::Going))
                || super::visible_contact(visibility, id, source)
                    .ok()
                    .flatten()
                    .is_none()
            {
                return None;
            }
            let unit = super::scanner::scanner_unit(world, id)?;
            let text = match unit.pair.primary {
                super::BattleSensorMode::Infrared => {
                    "The searing blast of heat burns out your sensors!"
                }
                super::BattleSensorMode::LightAmplification => {
                    "The blinding flash of light overloads your sensors!"
                }
                _ => return None,
            };
            Some(super::BattleNotice {
                unit: id,
                text: text.into(),
            })
        })
        .collect();
    for notice in &notices {
        *remaining_mut(world, notice.unit) = 4;
    }
    notices
}

/// A saved flash wakes the heartbeat even for a stopped, unplaced unit.
pub fn battle_sensor_flashes_pending(world: &World) -> bool {
    world
        .btech
        .constructed_units()
        .values()
        .any(|u| u.blinded_remaining() > 0)
        || world
            .btech
            .vehicles()
            .values()
            .any(|u| u.blinded_remaining() > 0)
}

/// Recover after four committed seconds; unconscious crews do not receive a recovery notice.
pub fn advance_battle_sensor_flashes(world: &mut World) -> Vec<super::BattleNotice> {
    let ids: Vec<_> = super::scanner::scanner_ids(world)
        .into_iter()
        .filter(|&id| battle_unit_blinded(world, id))
        .collect();
    let mut notices = Vec::new();
    for id in ids {
        let remaining = remaining_mut(world, id);
        *remaining -= 1;
        if *remaining == 0
            && !super::crew::unit_unconscious(world, id)
            && world
                .objects
                .get(&id)
                .is_some_and(|o| !o.flags.contains(crate::Flag::Going))
        {
            notices.push(super::BattleNotice {
                unit: id,
                text: "Your sight recovers.".into(),
            });
        }
    }
    notices
}
