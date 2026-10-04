//! Engine heat-sink allocation overrides retain authored construction metadata.
use anyhow::{Context, Result};
use std::collections::BTreeMap;

/// Zero selects the engine-derived allocation; signed values follow the unit-field contract.
pub fn read_engine_sink_override(attributes: &BTreeMap<String, String>) -> Result<i32> {
    attributes
        .get("hsengoverride")
        .map_or(Ok(0), |value| parse_engine_sink_override(value))
}

/// Validate an override before any template or world mutation.
pub fn parse_engine_sink_override(value: &str) -> Result<i32> {
    value
        .trim()
        .parse()
        .context("Expected a signed 32-bit engine heat-sink override")
}

/// Store one canonical value without rebuilding construction or changing current cooling.
pub fn write_engine_sink_override(attributes: &mut BTreeMap<String, String>, value: i32) {
    attributes.insert("hsengoverride".into(), value.to_string());
}
