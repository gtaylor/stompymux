//! The per-second counters of a vehicle record and how each moves with the clock.
//!
//! Every countdown and count a heartbeat steps is listed here with the condition under
//! which it steps, mirroring the advance functions that own it.
use super::timers::{BattleTimer as T, SavedTimer, SavedTimers, TimerList, TimerMotion::*};
use super::{BattlePower, BattleVehicle};

impl SavedTimers for BattleVehicle {
    fn saved_timers(&self) -> Vec<SavedTimer> {
        let mut list = TimerList::new();
        let running = self.power() == BattlePower::Running;
        if let BattlePower::Starting { remaining } = self.power {
            list.add(T::Startup, 0, remaining, Down);
        }
        list.add(T::Inferno, 0, self.inferno_remaining, Down);
        for (&section, &remaining) in &self.burning_sections {
            list.add(
                T::BurningSection,
                T::vehicle_section_slot(section),
                remaining,
                Down,
            );
        }
        if let Some(remaining) = self.extinguishing() {
            list.add(T::Extinguishing, 0, remaining, Down);
        }
        if let Some(remaining) = self.pod_removal() {
            list.add(T::PodRemoval, 0, remaining, Down);
        }
        list.add(T::CrewStun, 0, self.crew_stun_remaining, Down);
        for (index, &remaining) in self.turret_repairs.iter().enumerate() {
            list.add(T::TurretRepair, index as i64, remaining, Down);
        }
        for (&index, &remaining) in &self.weapon_recycle {
            list.add(
                T::WeaponRecycle,
                index as i64,
                remaining,
                if running { Down } else { Held },
            );
        }
        if let Some(attempt) = self.unjam {
            list.add(T::Unjam, 0, attempt.remaining, Down);
        }
        for (index, remaining) in self.spotter_events.countdowns() {
            list.add(T::SpotterEvent, index as i64, remaining, Down);
        }
        list.add(T::Tag, 0, self.tag.remaining, Down);
        list.add(T::RadioExperience, 0, self.radio_experience_remaining, Down);
        list.add(T::CrewRecovery, 0, self.crew_recovery.remaining, Down);
        if let Some(fall) = self.free_fall {
            list.add(T::FreeFall, 0, fall.remaining(), Down);
        }
        if let Some(timer) = self.self_destruct {
            list.add(T::SelfDestruct, 0, timer.remaining, Down);
        }
        if let Some(entry) = self.building_entry {
            list.add(T::BuildingEntry, 0, entry.remaining, Down);
        }
        list.add(T::Searchlight, 0, self.searchlight.remaining, Down);
        if let Some(elapsed) = self.hide_elapsed {
            list.add(T::Hide, 0, elapsed, Up);
        }
        // Flight burns one unit of fuel a second at ordinary speeds while the engine runs.
        if let Some(fuel) = self.vtol_fuel {
            let burning = running && self.vtol_flight.is_some() && fuel.remaining() > 0;
            list.add(
                T::VtolFuel,
                0,
                fuel.remaining(),
                if burning { Down } else { Held },
            );
        }
        list.finish()
    }
}
