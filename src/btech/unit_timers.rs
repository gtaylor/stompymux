//! The per-second counters of a BattleMech record and how each moves with the clock.
//!
//! Every countdown, count and phase a heartbeat steps is listed here with the condition
//! under which it steps, mirroring the advance functions that own it. A counter that
//! steps unconditionally while nonzero is simply `Down`; one that pauses, such as weapon
//! recycling on a shut-down unit, is `Held` while paused.
use super::timers::{BattleTimer as T, SavedTimer, SavedTimers, TimerList, TimerMotion::*};
use super::{BattleHeat, BattlePower, BattleUnit};

impl SavedTimers for BattleUnit {
    fn saved_timers(&self) -> Vec<SavedTimer> {
        let mut list = TimerList::new();
        let running = self.power() == BattlePower::Running;
        let destroyed = self.is_destroyed();
        if let BattlePower::Starting { remaining } = self.power {
            list.add(T::Startup, 0, remaining, Down);
        }
        // Thermal samples run while the unit is powered or still holds heat.
        let thermal = !destroyed && (running || self.heat != BattleHeat::default());
        let clock = self.overheat_clock;
        list.add(
            T::OverheatElapsed,
            0,
            clock.elapsed,
            if thermal && clock.elapsed < 30 {
                Up
            } else {
                Held
            },
        );
        list.add(
            T::OverheatPhase,
            0,
            clock.phase,
            if thermal { Wrap } else { Held },
        );
        list.add(T::Inferno, 0, self.inferno_remaining, Down);
        let stagger = &self.stagger;
        for (index, hit) in stagger.hits.iter().enumerate() {
            list.add(
                T::StaggerHit,
                index as i64,
                hit.remaining,
                if destroyed { Held } else { Down },
            );
        }
        // The rolling damage window counts while damage is on record. The turn phase
        // advances only under traditional rules, which is when it is ever nonzero; a
        // phase at zero is held, so a traditional unit rewrites its row twice a turn.
        let windowed = !destroyed && (!stagger.hits.is_empty() || stagger.turn_damage > 0);
        list.add(
            T::StaggerElapsed,
            0,
            i64::try_from(stagger.elapsed).unwrap_or(i64::MAX),
            if windowed { Up } else { Held },
        );
        list.add(
            T::StaggerPhase,
            0,
            stagger.phase,
            if !destroyed && running && stagger.phase != 0 {
                Wrap
            } else {
                Held
            },
        );
        if let Some(timer) = self.stand_timer {
            list.add(T::Stand, 0, timer.remaining(), Down);
        }
        for (&index, &remaining) in &self.weapon_recycle {
            list.add(
                T::WeaponRecycle,
                index as i64,
                remaining,
                if running { Down } else { Held },
            );
        }
        for (&section, &remaining) in &self.limb_recycle {
            list.add(
                T::LimbRecycle,
                T::section_slot(section),
                remaining,
                if running { Down } else { Held },
            );
        }
        if let Some(remaining) = self.reactor_instability_remaining {
            list.add(T::ReactorInstability, 0, remaining, Down);
        }
        list.add(T::Stun, 0, self.stun_remaining, Down);
        if let Some(fall) = self.free_fall {
            list.add(T::FreeFall, 0, fall.remaining(), Down);
        }
        if self.charge.target.is_some() {
            let moving = running
                && self.charge.elapsed < 60
                && self.motion.is_some_and(|motion| motion.active());
            list.add(
                T::Charge,
                0,
                self.charge.elapsed,
                if moving { Up } else { Held },
            );
        }
        // Stabilization waits out a jump in progress and resumes on landing or falling.
        let stabilizing = self.orbital_drop.is_none()
            && self.position.is_some()
            && (self.free_fall.is_some() || self.flight.is_none());
        list.add(
            T::JumpStabilization,
            0,
            self.jump_stabilization,
            if stabilizing { Down } else { Held },
        );
        if let Some(elapsed) = self.hide_elapsed {
            list.add(T::Hide, 0, elapsed, Up);
        }
        list.add(T::Masc, 0, self.masc.remaining, Down);
        list.add(T::Supercharger, 0, self.supercharger.remaining, Down);
        if let Some(dump) = self.dumping {
            list.add(
                T::Dump,
                0,
                i64::try_from(dump.phase).unwrap_or(i64::MAX),
                Up,
            );
        }
        if let Some(attempt) = self.unjam {
            list.add(T::Unjam, 0, attempt.remaining, Down);
        }
        for (index, remaining) in self.spotter_events.countdowns() {
            list.add(T::SpotterEvent, index as i64, remaining, Down);
        }
        if let Some(remaining) = self.heat_cutoff.remaining {
            list.add(T::HeatCutoff, 0, remaining, Down);
        }
        list.add(T::HullDown, 0, self.hull_down.remaining, Down);
        list.add(T::Lateral, 0, self.lateral.remaining, Down);
        list.add(T::Searchlight, 0, self.searchlight.remaining, Down);
        if let Some(timer) = self.self_destruct {
            list.add(T::SelfDestruct, 0, timer.remaining, Down);
        }
        if let Some(entry) = self.building_entry {
            list.add(T::BuildingEntry, 0, entry.remaining, Down);
        }
        list.add(T::Tag, 0, self.tag.remaining, Down);
        if let Some(pending) = self.null_signature.pending {
            list.add(T::NullSignature, 0, pending.remaining, Down);
        }
        if let Some(pending) = self.stealth.pending {
            list.add(T::Stealth, 0, pending.remaining, Down);
        }
        if let Some(lock) = self.target_lock {
            list.add(T::TargetLock, 0, lock.remaining(), Down);
        }
        list.add(T::RadioExperience, 0, self.radio_experience_remaining, Down);
        list.add(T::CrewRecovery, 0, self.crew_recovery.remaining, Down);
        list.finish()
    }
}
