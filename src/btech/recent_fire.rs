//! Transient "fired this heartbeat" marks used by status bits and the simulation scheduler,
//! and the durable times of each unit's last launch, of each Mech section's last launch and
//! of its last jump, read against the 30-second turn.
use super::{Mech, MechSection, Vehicle};
use crate::{Flag, ObjectId, World};

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

    /// Simulation time a weapon mounted in each section last launched. A weapon with critical
    /// slots in several sections marks each of them.
    pub fn section_fire(&self) -> &std::collections::BTreeMap<MechSection, i64> {
        &self.section_fire
    }

    /// Whether a weapon in `section` launched within the current turn: the last
    /// [`TURN_SECONDS`](super::spotter::TURN_SECONDS) of simulation time. A limb that fired this
    /// turn cannot also punch, kick, club or swing a physical weapon (Total Warfare pp. 145-147).
    pub fn section_fired_this_turn(&self, section: MechSection, now: i64) -> bool {
        self.section_fire
            .get(&section)
            .is_some_and(|&time| within_turn(time, now))
    }

    /// Whether any weapon on the unit launched within the current turn. A Mech that fired
    /// cannot charge or make a death from above attack that turn (Total Warfare pp. 148-149).
    pub fn fired_this_turn(&self, now: i64) -> bool {
        self.section_fire
            .values()
            .any(|&time| within_turn(time, now))
    }

    /// Simulation time this unit was last the target of a charge, death from above or push.
    pub fn assaulted_at(&self) -> Option<i64> {
        self.assaulted_at
    }

    /// Whether the unit launched a jump within the current turn. A Mech that jumped may not
    /// charge that turn (Total Warfare p. 148).
    pub fn jumped_this_turn(&self, now: i64) -> bool {
        self.last_jump_launch
            .is_some_and(|time| within_turn(time, now))
    }
}

/// Whether an event at simulation time `time` happened in the turn ending at `now`.
fn within_turn(time: i64, now: i64) -> bool {
    now.saturating_sub(time) < super::spotter::TURN_SECONDS
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

    /// Simulation time this vehicle was last the target of a charge or death from above.
    pub fn assaulted_at(&self) -> Option<i64> {
        self.assaulted_at
    }
}

/// Refuse a charge, death from above or push at a unit that was already the target of one
/// this turn: a unit may be the target of only one of them a turn (Total Warfare pp. 148-149).
pub(super) fn require_unassaulted(world: &World, target: ObjectId) -> anyhow::Result<()> {
    let now = world.btech.simulation_time();
    let assaulted = world.btech.unit(target).is_some_and(|unit| {
        super::with_unit!(unit, |unit| unit
            .assaulted_at()
            .is_some_and(|time| within_turn(time, now)))
    });
    anyhow::ensure!(
        !assaulted,
        "Your target has already been charged, pushed or landed on this turn."
    );
    Ok(())
}

/// Record that `target` has just been the target of a charge, death from above or push.
pub(super) fn mark_assaulted(world: &mut World, target: ObjectId) {
    let now = world.btech.simulation_time();
    if let Some(unit) = world.btech.constructed.get_mut(&target) {
        unit.assaulted_at = Some(now);
    } else if let Some(vehicle) = world.btech.vehicles.get_mut(&target) {
        vehicle.assaulted_at = Some(now);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::MechTemplate;

    /// A Jenner straight from its template, with no fire recorded.
    fn jenner() -> Mech {
        Mech::from_template(
            MechTemplate::parse(
                "JR7-D",
                include_str!("../../tests/fixtures/btech/units/JR7-D.toml"),
            )
            .unwrap(),
        )
        .unwrap()
    }

    /// A section that fired stays marked for exactly one 30-second turn, and only that section
    /// is marked (Total Warfare pp. 145-149).
    #[test]
    fn section_fire_lasts_one_turn() {
        let mut unit = jenner();
        assert!(!unit.fired_this_turn(100));
        unit.section_fire.insert(MechSection::RightArm, 100);
        for (now, fired) in [
            (100, true),
            (120, true),
            (129, true),
            (130, false),
            (200, false),
        ] {
            assert_eq!(
                unit.section_fired_this_turn(MechSection::RightArm, now),
                fired,
                "{now}"
            );
            assert_eq!(unit.fired_this_turn(now), fired, "{now}");
            assert!(!unit.section_fired_this_turn(MechSection::LeftArm, now));
        }
    }
}
