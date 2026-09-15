//! Collect ordered character injuries from vehicle damage without repeating health calculations.
use super::{
    BattleCharacterPilotInjury, BattleVehicleArmorDamage, BattleVehicleCriticalResolution,
};

/// Armor criticals occur before penetration and its internal criticals.
pub(super) fn collect_armor(
    report: &BattleVehicleArmorDamage,
    injuries: &mut Vec<BattleCharacterPilotInjury>,
) {
    for critical in &report.criticals {
        collect_critical(critical, injuries);
    }
    if let Some(internal) = &report.internal {
        collect_internal(internal, injuries);
    }
}

/// Nested weapon explosions finish before their surviving-pilot injury is applied.
pub(super) fn collect_critical(
    report: &BattleVehicleCriticalResolution,
    injuries: &mut Vec<BattleCharacterPilotInjury>,
) {
    for damage in &report.internal_damage {
        collect_internal(damage, injuries);
    }
    if let Some(injury) = &report.character_injury {
        injuries.push(injury.clone());
    }
}

/// Blast heat applies advanced fire damage after its ordinary impact packets.
pub(super) fn collect_heat(
    report: &super::BattleVehicleHeatExposure,
    injuries: &mut Vec<BattleCharacterPilotInjury>,
) {
    if let Some(fire) = &report.fire {
        for damage in &fire.effects.damage {
            collect_armor(damage, injuries);
        }
    }
}

/// A misload or standalone internal explosion retains injuries on its critical reports.
pub(super) fn collect_internal(
    report: &super::BattleVehicleInternalDamage,
    injuries: &mut Vec<BattleCharacterPilotInjury>,
) {
    for critical in &report.criticals {
        collect_critical(critical, injuries);
    }
}

/// Vehicle salvos resolve ordinary groups before their inferno effects.
pub(super) fn collect_salvo(
    report: &super::BattleVehicleSalvoReport,
    injuries: &mut Vec<BattleCharacterPilotInjury>,
) {
    for group in &report.groups {
        if let Some(damage) = &group.impact.damage {
            collect_armor(damage, injuries);
        }
    }
    if let Some(inferno) = &report.inferno {
        for damage in &inferno.damage {
            collect_armor(damage, injuries);
        }
    }
}
