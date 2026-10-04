//! The template movement baseline controls firing penalties independently of live propulsion.
use anyhow::{Context, Result, ensure};
use std::collections::BTreeMap;

/// Parse the reference floating-point field without admitting negative or nonfinite baselines.
pub fn parse_template_speed(value: &str) -> Result<f64> {
    let speed = value
        .trim()
        .parse::<f32>()
        .context("Expected a nonnegative finite template speed")?;
    ensure!(
        speed.is_finite() && speed >= 0.0,
        "Expected a nonnegative finite template speed"
    );
    Ok(f64::from(speed))
}

/// Unedited units use their authored speed; edits retain an independent construction baseline.
pub fn read_template_speed(attributes: &BTreeMap<String, String>, authored: f64) -> Result<f64> {
    attributes
        .get("template_speed")
        .map_or(Ok(authored), |value| parse_template_speed(value))
}

/// Store one canonical baseline without changing engine, damage, throttle or actual speed.
pub fn write_template_speed(attributes: &mut BTreeMap<String, String>, speed: f64) {
    attributes.insert("template_speed".into(), speed.to_string());
}
