//! Engine heat-sink allocation overrides retain authored construction metadata.
use anyhow::{Context, Result};
use std::collections::BTreeMap;

/// Zero selects the engine-derived allocation; signed values follow the unit-field contract.
pub(super) fn read(attributes: &BTreeMap<String, String>) -> Result<i32> {
    attributes
        .get("hsengoverride")
        .map_or(Ok(0), |value| parse(value))
}

/// Validate an override before any template or world mutation.
pub(super) fn parse(value: &str) -> Result<i32> {
    value
        .trim()
        .parse()
        .context("Expected a signed 32-bit engine heat-sink override")
}

/// Store one canonical value without rebuilding construction or changing current cooling.
pub(super) fn write(attributes: &mut BTreeMap<String, String>, value: i32) {
    attributes.insert("hsengoverride".into(), value.to_string());
}

/// Installed external cooling capacity is independent of an engine-allocation correction.
pub(super) fn external_capacity(unit: &super::BattleUnit) -> Result<u16> {
    let definition = unit.definition();
    let slots = unit
        .loadout()?
        .systems
        .iter()
        .filter(|part| part.system == super::BattleSystem::HeatSink)
        .count();
    Ok((slots / definition.heat_sink_slots()
        * if definition.has_double_heat_sinks() {
            2
        } else {
            1
        }) as u16)
}

/// Reconstruct total cooling from owned external equipment and the selected engine allocation.
pub(super) fn reconstructed_capacity(unit: &super::BattleUnit) -> Result<u16> {
    let definition = unit.definition();
    let allocation = read(&definition.attributes)?;
    anyhow::ensure!(
        allocation >= 0,
        "Negative engine heat-sink allocation cannot be reconstructed"
    );
    let internal = if allocation == 0 {
        super::engine::rated_output(definition.tons, definition.max_speed)? / 25
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
