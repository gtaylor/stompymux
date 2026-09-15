//! Rebuild Mech system baselines after a damage-field critical replacement.
use super::{BattleBeaconKind, BattleUnit};
use anyhow::Result;

/// Adopt owned construction and surviving equipment without combat rolls or template-file reloads.
pub(super) fn recalculate(unit: &mut BattleUnit, gyro_protection_used: bool) -> Result<()> {
    let loadout = unit.loadout()?;
    unit.reconstructed_cooling = Some(super::engine_sink_override::reconstructed_capacity(unit)?);
    unit.heat_cutoff.disabled = unit.heat_cutoff.disabled.min(unit.cooling_capacity());
    let improved = unit.definition().has_special("ImprovedJJ_Tech");
    let installed = loadout.jump_jet_groups(improved)?.len();
    let maximum = (unit.template_speed() / 10.75 * if improved { 1.0 } else { 2.0 / 3.0 }).floor();
    let jump = (installed as f64).min(maximum) * 10.75;
    unit.propulsion.reconstruct(unit.template_speed(), jump);
    unit.reconstruct_gyro(gyro_protection_used);
    unit.hardware.tactical = None;
    unit.hardware.long_range = None;
    unit.hardware.scan = None;
    // Thermal production and cooling are derived from installed equipment at the next sample.
    // Recalculation does not advance that sample or replace stored heat.
    unit.beacons.retain(|_, kinds| {
        kinds.retain(|kind| *kind == BattleBeaconKind::Narc);
        !kinds.is_empty()
    });
    let available: std::collections::BTreeSet<_> = loadout
        .weapons
        .iter()
        .enumerate()
        .filter(|(_, mount)| {
            mount
                .criticals
                .iter()
                .all(|location| !unit.critical_unavailable(*location))
        })
        .map(|(index, _)| index)
        .collect();
    unit.spent_launchers
        .retain(|index| !available.contains(index));
    Ok(())
}
