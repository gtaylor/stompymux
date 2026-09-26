//! The simulation scheduler and operator diagnostics share one live-work predicate.
use crate::{Config, World};

/// Decide whether a tick has work, reusing already collected scanners in the server path.
pub(crate) fn pending(world: &World, config: &Config, has_scanner_observers: bool) -> bool {
    super::autopilot::runtime::pending(world)
        || super::computer_runtime::pending(world, config)
        || super::sixth_sense::pending(world)
        || world
            .btech
            .gunner_stations()
            .values()
            .any(|station| station.lock_remaining > 0)
        || world.btech.vehicles().values().any(|unit| {
            (unit.detached && unit.power() != crate::BattlePower::Off)
                || unit.vtol_flight().is_some_and(|flight| {
                    matches!(
                        flight.phase,
                        crate::BattleVtolFlightPhase::Falling
                            | crate::BattleVtolFlightPhase::Launching { .. }
                    ) || flight.phase == crate::BattleVtolFlightPhase::Airborne
                        && flight.vertical_speed != 0.0
                })
                || unit.orbital_drop().is_some()
                || unit.free_fall().is_some()
                || unit.spotter_events.pending()
                || unit.tag().target.is_some()
                || unit.tag().remaining > 0
                || unit.searchlight().remaining > 0
                || unit.inferno_remaining() > 0
                || !unit.burning_sections().is_empty()
                || unit.extinguishing().is_some()
                || unit.crew_recovery().remaining > 0
                || unit.unjam().is_some()
                || unit.pod_removal().is_some()
                || unit
                    .target_selection()
                    .is_some_and(|lock| lock.remaining() > 0)
                || !unit.turret_repairs().is_empty()
                || unit.dig_state().remaining() > 0
                || unit.crew_stun_remaining() > 0
                || matches!(unit.power(), crate::BattlePower::Starting { .. })
                || unit.power() == crate::BattlePower::Running
                    && (unit.motion().is_some_and(crate::BattleMotion::active)
                        || !unit.weapon_recycle().is_empty())
        })
        || crate::battle_automatic_turrets_pending(world)
        || crate::battle_hiding_pending(world)
        || crate::battle_self_destructs_pending(world)
        || crate::battle_reactor_windows_pending(world)
        || crate::battle_wrecks_pending(world)
        || has_scanner_observers
        || crate::battle_building_entries_pending(world)
        || crate::building_repair_pending(world)
        || crate::artillery_pending(world)
        || crate::map_fire_pending(world)
        || crate::map_smoke_pending(world)
        || crate::battle_illumination_pending(world)
        || crate::battle_electronic_fields_pending(world)
        || world
            .btech
            .recoveries()
            .values()
            .any(|recovery| recovery.remaining > 0)
        || world.btech.constructed_units().values().any(|unit| {
            unit.radio_experience_remaining() > 0
                || unit.inferno_remaining() > 0
                || unit.spotter_events.pending()
                || unit.tag().target.is_some()
                || unit.tag().remaining > 0
                || unit.fired_recently()
                || unit.power() == crate::BattlePower::Running
                || unit.stagger_active(crate::BattleStaggerMode::from_setting(
                    config.battletech.newstagger,
                ))
                || unit.stand_timer().is_some()
                || unit.crew_recovery().remaining > 0
                || unit.unjam().is_some()
                || unit.dumping().is_some()
                || unit.masc().remaining > 0
                || unit.supercharger().remaining > 0
                || unit.airborne()
                || unit.orbital_drop().is_some()
                || unit.free_fall().is_some()
                || unit.jump_stabilization() > 0
                || unit
                    .target_selection()
                    .is_some_and(|lock| lock.remaining() > 0)
                || unit.searchlight().remaining > 0
                || unit.stealth().pending.is_some()
                || unit.null_signature().pending.is_some()
                || unit.stun_remaining() > 0
                || unit.heat_active(world)
                || unit.overheat_active()
                || matches!(unit.power(), crate::BattlePower::Starting { .. })
                || (unit.fired_recently()
                    || unit.power() == crate::BattlePower::Running
                        && (unit.motion().is_some_and(crate::BattleMotion::active)
                            || !unit.weapon_recycle().is_empty()
                            || !unit.limb_recycle().is_empty()))
        })
}
