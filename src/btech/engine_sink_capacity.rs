//! Live heat-sink capacity of a constructed Mech, reconstructed from installed equipment and
//! the template's engine heat-sink allocation.
use super::{BattleSystem, BattleUnit, rated_output, read_engine_sink_override};
use anyhow::Result;

/// Installed external cooling capacity is independent of an engine-allocation correction.
pub(super) fn external_capacity(unit: &BattleUnit) -> Result<u16> {
    let definition = unit.definition();
    let slots = unit
        .loadout()?
        .systems
        .iter()
        .filter(|part| part.system == BattleSystem::HeatSink)
        .count();
    Ok((slots / definition.heat_sink_slots()
        * if definition.has_double_heat_sinks() {
            2
        } else {
            1
        }) as u16)
}

/// Reconstruct total cooling from owned external equipment and the selected engine allocation.
pub(super) fn reconstructed_capacity(unit: &BattleUnit) -> Result<u16> {
    let definition = unit.definition();
    let allocation = read_engine_sink_override(&definition.attributes)?;
    anyhow::ensure!(
        allocation >= 0,
        "Negative engine heat-sink allocation cannot be reconstructed"
    );
    let internal = if allocation == 0 {
        rated_output(definition.tons, definition.max_speed)? / 25
    } else {
        allocation as u32
    };
    let efficiency = if definition.has_double_heat_sinks() {
        2
    } else {
        1
    };
    let internal = u32::from(definition.heat_sinks).min(internal.saturating_mul(efficiency));
    Ok(external_capacity(unit)? + internal as u16)
}
