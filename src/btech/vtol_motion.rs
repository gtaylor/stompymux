//! Rotorcraft movement proposals share horizontal geometry and expose altitude consequences.
use super::{BattleHex, BattleMotion, BattleVehicle, BattleVtolFlightPhase};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Surface decision for the host to resolve before committing aircraft movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BattleVtolSurfaceContact {
    Clear,
    Water,
    /// A horizontal entry below forest canopy requires a host piloting check.
    Forest,
    /// Horizontal terrain entry requires rollback and an emergency landing check.
    Elevation,
    /// Attempt deliberate landing first; otherwise resolve this many levels of falling damage.
    Ground {
        fall_levels: u32,
    },
}

/// A pure proposal: the host traces crossed hexes before publishing or committing movement.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[must_use = "Trace and resolve surface contacts before committing flight movement"]
pub struct BattleVtolMotionStep {
    pub motion: BattleMotion,
    /// Continuous altitude in elevation levels, retaining fractions across movement events.
    pub altitude: f64,
    /// Vertical speed after applying the orbit ceiling.
    pub vertical_speed: f64,
    pub ceiling_reached: bool,
    /// Inputs bind a proposal to the material state from which it was calculated.
    pub(super) origin: (BattleMotion, super::BattleVtolFlight, super::BattlePosition),
    movement_modifier: i64,
}

impl BattleVtolMotionStep {
    /// Integer altitude truncates toward zero, including fractional levels below water.
    pub fn elevation(self) -> i32 {
        self.altitude as i32
    }

    /// Check the destination surface; horizontal path traversal remains the host's responsibility.
    pub fn surface_contact(self, hex: BattleHex) -> BattleVtolSurfaceContact {
        let altitude = self.elevation();
        if hex.is_open_water() && altitude < 0 {
            return BattleVtolSurfaceContact::Water;
        }
        let surface = i32::from(hex.surface_height());
        if altitude >= surface || (hex.has_bridge() && altitude != surface - 1) {
            return BattleVtolSurfaceContact::Clear;
        }
        BattleVtolSurfaceContact::Ground {
            fall_levels: 1u32.saturating_add((self.vertical_speed.abs() / 10.75) as u32),
        }
    }
}

impl BattleVehicle {
    /// Project an airborne movement event after horizontal turning and acceleration.
    /// Uses the saved continuous altitude; commit only after resolving terrain traversal.
    /// Movement modifiers affect horizontal distance only. Falling uses the separate fall event.
    pub fn vtol_motion_step(&self, movement_modifier: i64) -> Result<BattleVtolMotionStep> {
        ensure!(
            self.definition().is_vtol(),
            "Flight movement requires a VTOL"
        );
        let flight = self
            .vtol_flight
            .context("Aircraft flight state is unavailable")?;
        ensure!(
            flight.phase == BattleVtolFlightPhase::Airborne
                && self.power() == super::BattlePower::Running,
            "Movement requires a powered airborne aircraft"
        );
        ensure!(
            !self.is_destroyed() && !self.rotor_destroyed(),
            "Aircraft has lost lift"
        );
        let mut motion = self
            .motion
            .context("Place the aircraft on a battlefield first")?;
        let origin = (
            motion,
            flight,
            self.position().context("Aircraft is not placed")?,
        );
        let rate = if movement_modifier > 0 {
            movement_modifier as f64 / 100.0
        } else {
            1.0
        };
        motion.point = motion.project_step(rate)?;
        motion.point.containing_hex()?;
        let mut altitude = flight.altitude + flight.vertical_speed / 129.0;
        ensure!(
            altitude.is_finite() && altitude >= f64::from(i32::MIN),
            "Flight altitude exceeds coordinate limits"
        );
        let ceiling_reached = altitude >= 300.0;
        if ceiling_reached {
            altitude = 299.0;
        }
        Ok(BattleVtolMotionStep {
            motion,
            altitude,
            vertical_speed: if ceiling_reached {
                0.0
            } else {
                flight.vertical_speed
            },
            ceiling_reached,
            origin,
            movement_modifier,
        })
    }

    /// Commit a still-current movement proposal after the host has resolved its entire path.
    /// This material operation does not authorize map bounds or publish terrain consequences.
    pub fn commit_vtol_motion(&mut self, step: BattleVtolMotionStep) -> Result<()> {
        self.commit_vtol_motion_at(step, step.motion.point)
    }

    /// Commit an observed proposal at the map adapter's resolved boundary position.
    pub(super) fn commit_vtol_motion_at(
        &mut self,
        step: BattleVtolMotionStep,
        point: super::BattlePoint,
    ) -> Result<()> {
        ensure!(
            self.vtol_motion_step(step.movement_modifier)? == step,
            "Stale or altered flight proposal"
        );
        let coordinate = point.containing_hex()?;
        let position = super::BattlePosition {
            map: step.origin.2.map,
            x: u16::try_from(coordinate.x)?,
            y: u16::try_from(coordinate.y)?,
        };
        let mut motion = step.motion;
        motion.point = point;
        self.update_motion(motion, position, false);
        let flight = self.vtol_flight.as_mut().unwrap();
        flight.altitude = step.altitude;
        flight.vertical_speed = step.vertical_speed;
        self.ground_elevation = None;
        Ok(())
    }
}
