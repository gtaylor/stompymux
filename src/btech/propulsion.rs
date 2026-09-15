//! Saved administrative propulsion state, separate from construction and firing baselines.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Ordinary units derive speed from construction and damage; edits retain only independent state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub(super) struct Propulsion {
    /// Baseline adopted at a Mech actuator recalculation, before material penalties.
    baseline: Option<f64>,
    /// Explicit live maximum, valid until a chassis-specific recalculation replaces it.
    maximum: Option<f64>,
    /// Corrected jump baseline before equipment losses, preserving existing damage at edit time.
    jump: Option<f64>,
}

impl Propulsion {
    /// Adopt construction baselines after a complete critical-state reconstruction.
    pub(super) fn reconstruct(&mut self, baseline: f64, jump: f64) {
        self.recalculate(baseline);
        self.jump = Some(jump);
    }

    /// Surviving thrust uses the same loss arithmetic for both chassis families.
    pub(super) fn jump(self, authored: f64, lost: usize) -> f64 {
        (self.jump.unwrap_or(authored) - lost as f64 * 10.75).max(0.0)
    }

    /// A current-thrust edit compensates existing losses without repairing their equipment.
    pub(super) fn set_jump(&mut self, speed: f64, lost: usize) {
        self.jump = Some(speed + lost as f64 * 10.75);
    }
    /// Construction is the baseline until a damage event adopts an edited template speed.
    pub(super) fn baseline(self, authored: f64) -> f64 {
        self.baseline.unwrap_or(authored)
    }

    /// An administrative maximum replaces the derived speed without manufacturing damage.
    pub(super) fn maximum(self, derived: f64) -> f64 {
        self.maximum.unwrap_or(derived)
    }

    /// Explicit live correction leaves the future recalculation baseline unchanged.
    pub(super) fn set(&mut self, speed: f64) {
        self.maximum = Some(speed);
    }

    /// Actuator recalculation replaces an explicit correction and adopts the template baseline.
    pub(super) fn recalculate(&mut self, baseline: f64) {
        self.baseline = Some(baseline);
        self.maximum = None;
    }

    /// Vehicle motive hits reduce an edited live maximum without double-counting material losses.
    pub(super) fn lower(&mut self, amount: f64) {
        if let Some(maximum) = &mut self.maximum {
            *maximum = (*maximum - amount).max(0.0);
        }
    }

    /// All saved speed values must remain nonnegative and finite.
    pub(super) fn validate(self) -> Result<()> {
        ensure!(
            self.baseline
                .into_iter()
                .chain(self.maximum)
                .chain(self.jump)
                .all(|speed| speed.is_finite() && speed >= 0.0),
            "Invalid propulsion state"
        );
        Ok(())
    }
}

/// Match the numeric field width while rejecting impossible propulsion values.
pub(super) fn parse(value: &str) -> Result<f64> {
    let speed = value
        .trim()
        .parse::<f32>()
        .context("Expected a nonnegative finite maximum speed")?;
    ensure!(
        speed.is_finite() && speed >= 0.0,
        "Expected a nonnegative finite maximum speed"
    );
    Ok(f64::from(speed))
}
