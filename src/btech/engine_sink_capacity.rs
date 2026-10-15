//! Live heat-sink capacity of a constructed Mech, reconstructed from installed equipment and
//! the heat sinks its engine holds.
use super::{Mech, System, rated_output};
use anyhow::Result;

/// Cooling of the heat sinks installed in critical slots, counting complete installations.
pub(super) fn external_capacity(unit: &Mech) -> Result<u16> {
    let definition = unit.definition();
    let slots = unit
        .loadout()?
        .systems
        .iter()
        .filter(|part| part.system == System::HeatSink)
        .count();
    Ok((slots / definition.heat_sink_slots() * usize::from(definition.heat_per_sink())) as u16)
}

/// Reconstruct total cooling from the surviving external sinks and the sinks the engine holds:
/// its rating / 25, rounded down (TechManual pp. 52-53), never more than the template's
/// dissipation.
pub(super) fn reconstructed_capacity(unit: &Mech) -> Result<u16> {
    let definition = unit.definition();
    let internal = rated_output(definition.tons, definition.walk_mp) / 25;
    let efficiency = u32::from(definition.heat_per_sink());
    let internal =
        u32::from(definition.heat_dissipation()).min(internal.saturating_mul(efficiency));
    Ok(external_capacity(unit)? + internal as u16)
}
