//! One movement budget for rotorcraft horizontal controls, climb/descent and cruise
//! restrictions. Total Warfare p. 54 charges a VTOL 1 MP per hex entered and 1 MP per level
//! climbed or descended from the same pool, so horizontal and vertical speed add up to the
//! VTOL's maximum rather than combining as a vector.
use super::Vehicle;
use anyhow::{Result, ensure};

/// Speed left for the other axis after `component` is spent: the two share one additive
/// budget (hexes plus levels per turn, TW p. 54). Overspeed after damage leaves no capacity.
pub(super) fn remaining_speed(maximum: f64, component: f64) -> Result<f64> {
    ensure!(
        maximum.is_finite() && maximum >= 0.0 && component.is_finite(),
        "Invalid flight speed"
    );
    Ok((maximum - component.abs()).max(0.0))
}

impl Vehicle {
    /// Change climb or descent within the movement budget left by the current horizontal
    /// command. The enclosing action owns operator authority and flight-event scheduling.
    pub fn set_vtol_vertical_speed(&mut self, requested: f64) -> Result<()> {
        self.set_vtol_vertical_at(requested, self.maximum_speed())
    }

    /// Apply the shared additive budget to a world-supplied loaded ceiling.
    pub(super) fn set_vtol_vertical_at(&mut self, requested: f64, maximum: f64) -> Result<()> {
        super::fortification::require_unfortified(self.fortified)?;
        ensure!(requested.is_finite(), "Invalid vertical speed");
        ensure!(
            self.definition().is_vtol(),
            "Vertical speed requires a VTOL"
        );
        ensure!(
            self.power() == super::Power::Running
                && !self.is_destroyed()
                && !self.rotor_destroyed()
                && self
                    .vtol_flight
                    .is_some_and(|flight| flight.phase == super::VtolFlightPhase::Airborne),
            "Vertical control requires an airborne, running VTOL with lift"
        );
        ensure!(self.has_vtol_fuel(), "The VTOL is out of fuel");
        let motion = self
            .motion
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Place the aircraft on a battlefield first"))?;
        let maximum = remaining_speed(maximum, motion.desired_speed)?;
        ensure!(
            requested.abs() <= maximum,
            "Vertical speed exceeds the remaining movement budget"
        );
        self.vtol_flight.as_mut().unwrap().vertical_speed = requested;
        Ok(())
    }

    /// Maximum horizontal speed after spending part of the movement budget on climb or descent.
    pub fn vtol_horizontal_limit(&self, vertical_speed: f64) -> Result<f64> {
        ensure!(self.definition().is_vtol(), "Flight speed requires a VTOL");
        remaining_speed(self.maximum_speed(), vertical_speed)
    }

    /// Symmetric climb/descent limit based on the commanded horizontal speed.
    pub fn vtol_vertical_limit(&self, horizontal_throttle: f64) -> Result<f64> {
        ensure!(self.definition().is_vtol(), "Flight speed requires a VTOL");
        remaining_speed(self.maximum_speed(), horizontal_throttle)
    }

    /// Clamp a horizontal request to forward/reverse limits, then enforce tail-rotor cruise safety.
    /// Authority, startup, fuel and takeoff admission belong to the enclosing flight control action.
    pub fn vtol_throttle(&self, requested: f64, vertical_speed: f64) -> Result<f64> {
        self.vtol_throttle_at(requested, vertical_speed, self.maximum_speed())
    }

    /// Share tail-rotor and movement-budget checks with world-owned towing loads.
    pub(super) fn vtol_throttle_at(
        &self,
        requested: f64,
        vertical_speed: f64,
        maximum: f64,
    ) -> Result<f64> {
        ensure!(requested.is_finite(), "Invalid flight throttle");
        ensure!(self.definition().is_vtol(), "Flight speed requires a VTOL");
        let maximum = remaining_speed(maximum, vertical_speed)?;
        let cruise = maximum * 2.0 / 3.0;
        let requested = requested.clamp(-cruise, maximum);
        ensure!(
            !self.tail_rotor_destroyed() || requested <= cruise + 0.1,
            "A destroyed tail rotor prevents travel faster than cruise speed"
        );
        Ok(requested)
    }

    /// Reduce an existing forward command after tail-rotor damage, preserving actual momentum.
    /// The flight action supplies current vertical speed and owns the surrounding damage checkpoint.
    pub fn constrain_vtol_cruise(&mut self, vertical_speed: f64) -> Result<()> {
        self.constrain_vtol_cruise_at(vertical_speed, self.maximum_speed())
    }

    /// Apply first-hit cruise restriction using the world-owned load, mode and gravity ceiling.
    pub(super) fn constrain_vtol_cruise_at(
        &mut self,
        vertical_speed: f64,
        maximum: f64,
    ) -> Result<()> {
        let cruise = remaining_speed(maximum, vertical_speed)? * 2.0 / 3.0;
        if let Some(motion) = &mut self.motion
            && motion.desired_speed > cruise
        {
            motion.desired_speed = (cruise - 0.1).max(0.0);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::remaining_speed;

    /// Hexes and levels come from one pool: 12 MP of flight spent 4 MP on climbing leaves 8.
    #[test]
    fn horizontal_and_vertical_speed_share_one_additive_budget() {
        assert_eq!(remaining_speed(129.0, 43.0).unwrap(), 86.0);
        assert_eq!(remaining_speed(129.0, -43.0).unwrap(), 86.0);
        assert_eq!(remaining_speed(129.0, 129.0).unwrap(), 0.0);
        assert_eq!(remaining_speed(129.0, 200.0).unwrap(), 0.0);
        assert_eq!(remaining_speed(0.0, 0.0).unwrap(), 0.0);
        assert!(remaining_speed(f64::NAN, 0.0).is_err());
        assert!(remaining_speed(-1.0, 0.0).is_err());
    }
}
