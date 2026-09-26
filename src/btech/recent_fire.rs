//! Transient "fired this heartbeat" marks used by status bits and the simulation scheduler.
use super::{BattleUnit, BattleVehicle};
use crate::{Flag, World};

impl BattleUnit {
    /// A completed launch marks this unit until the next committed heartbeat, including launches that miss.
    pub fn fired_recently(&self) -> bool {
        self.fired_recently
    }
}

impl BattleVehicle {
    /// A completed launch marks this vehicle until the next committed heartbeat, including misses.
    pub fn fired_recently(&self) -> bool {
        self.fired_recently
    }
}

/// Reset transient weapon emission before the heartbeat's movement and thermal work; rollback restores it.
pub fn clear_recent_fire(world: &mut World) {
    let ids: Vec<_> = world
        .btech
        .constructed_units()
        .iter()
        .map(|(&id, unit)| (id, unit.fired_recently))
        .chain(
            world
                .btech
                .vehicles()
                .iter()
                .map(|(&id, unit)| (id, unit.fired_recently)),
        )
        .filter(|(id, fired)| {
            *fired
                && world
                    .objects
                    .get(id)
                    .is_some_and(|object| !object.flags.contains(Flag::Going))
        })
        .map(|(id, _)| id)
        .collect();
    for id in ids {
        if let Some(unit) = std::sync::Arc::make_mut(&mut world.btech.vehicles).get_mut(&id) {
            unit.fired_recently = false;
        } else {
            std::sync::Arc::make_mut(&mut world.btech.constructed)
                .get_mut(&id)
                .unwrap()
                .fired_recently = false;
        }
    }
}
