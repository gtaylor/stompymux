//! Transient "fired this heartbeat" marks used by status bits and the simulation scheduler,
//! and the durable time of each unit's last launch.
use super::{Mech, Vehicle};
use crate::{Flag, World};

impl Mech {
    /// A completed launch marks this unit until the next committed heartbeat, including launches that miss.
    pub fn fired_recently(&self) -> bool {
        self.fired_recently
    }

    /// Simulation time of the last weapon this unit launched, if it ever has.
    pub fn last_weapon_fire(&self) -> Option<i64> {
        self.last_weapon_fire
    }

    /// Simulation time of this unit's last charge or death-from-above attack, if any.
    pub fn physical_assault_time(&self) -> Option<i64> {
        self.physical_assault_time
    }
}

impl Vehicle {
    /// A completed launch marks this vehicle until the next committed heartbeat, including misses.
    pub fn fired_recently(&self) -> bool {
        self.fired_recently
    }

    /// Simulation time of the last weapon this vehicle launched, if it ever has.
    pub fn last_weapon_fire(&self) -> Option<i64> {
        self.last_weapon_fire
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
        crate::btech::with_unit_mut!(world.btech.unit_mut(id).unwrap(), |unit| {
            unit.fired_recently = false;
        })
    }
}
