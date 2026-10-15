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

/// The walking (cruising) MP a Mech or combat vehicle template states as `walk_mp`, and the
/// running (flanking) speed in km/h derived from it: walking x 1.5 rounded up, one hex a
/// turn per MP (TM pp. 48 and 100). A template without a speed stands still; one that states
/// a bare `max_speed` is refused, since construction needs the walking figure.
pub fn template_walking_mp(fields: &BTreeMap<String, String>) -> Result<(u32, f64)> {
    let Some(walking) = fields.get("walk_mp") else {
        ensure!(
            !fields.contains_key("max_speed"),
            "state the speed as walk_mp, the walking or cruising MP"
        );
        return Ok((0, 0.0));
    };
    let walking: u32 = walking.trim().parse().context("invalid walk_mp")?;
    let running = super::running_mp_of_walking(walking);
    Ok((
        walking,
        f64::from(running) * super::construction::SPEED_PER_MP,
    ))
}

/// Store one canonical baseline without changing engine, damage, throttle or actual speed.
pub fn write_template_speed(attributes: &mut BTreeMap<String, String>, speed: f64) {
    attributes.insert("template_speed".into(), speed.to_string());
}
