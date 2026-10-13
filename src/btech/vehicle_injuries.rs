//! Collect ordered character injuries from vehicle damage without repeating health calculations.
use super::{CharacterPilotInjury, VehicleArmorDamage, VehicleCriticalResolution};

/// Armor criticals occur before penetration and its internal criticals.
pub(super) fn collect_armor(report: &VehicleArmorDamage, injuries: &mut Vec<CharacterPilotInjury>) {
    for critical in &report.criticals {
        collect_critical(critical, injuries);
    }
    if let Some(internal) = &report.internal {
        collect_internal(internal, injuries);
    }
}

/// Critical hits injure vehicle crews only through nested explosions.
pub(super) fn collect_critical(
    report: &VehicleCriticalResolution,
    injuries: &mut Vec<CharacterPilotInjury>,
) {
    for damage in &report.internal_damage {
        collect_internal(damage, injuries);
    }
}

/// A misload or standalone internal explosion retains injuries on its critical reports.
pub(super) fn collect_internal(
    report: &super::VehicleInternalDamage,
    injuries: &mut Vec<CharacterPilotInjury>,
) {
    for critical in &report.criticals {
        collect_critical(critical, injuries);
    }
}

/// Vehicle salvos resolve ordinary groups before their inferno effects.
pub(super) fn collect_salvo(
    report: &super::VehicleSalvoReport,
    injuries: &mut Vec<CharacterPilotInjury>,
) {
    for group in &report.groups {
        if let Some(damage) = &group.impact.damage {
            collect_armor(damage, injuries);
        }
    }
    if let Some(inferno) = &report.inferno {
        for missile in &inferno.criticals {
            collect_critical(&missile.critical, injuries);
        }
    }
}
