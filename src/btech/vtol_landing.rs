//! Deliberate rotorcraft landing checks and material touchdown, separate from crash damage.
use super::{BattleHex, BattleVehicle, BattleVtolFlight, BattleVtolFlightPhase, Terrain};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// The enclosing flight action applies altitude and publishes landing effects transactionally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[must_use = "Apply touchdown elevation and publish landing effects in the enclosing action"]
pub enum BattleVtolLanding {
    LaunchCancelled,
    Touchdown { elevation: i16 },
}

impl BattleVehicle {
    /// Land on the supplied current map surface, or cancel an outstanding launch.
    /// The host owns map lookup, cockpit authority, messages, callbacks and landing mines.
    /// Failed admission does not spend fuel, draw dice, or alter motion.
    pub fn land_vtol(
        &mut self,
        hex: BattleHex,
        free_fusion_fuel: bool,
    ) -> Result<BattleVtolLanding> {
        ensure!(self.definition().is_vtol(), "Landing requires a VTOL");
        ensure!(
            self.has_vtol_fuel(free_fusion_fuel),
            "You lack fuel to maneuver for landing!"
        );
        let flight = self
            .vtol_flight
            .context("Aircraft flight state is unavailable")?;
        ensure!(!self.rotor_destroyed(), "The rotor's dead!");
        ensure!(
            !self.is_destroyed() && flight.phase != BattleVtolFlightPhase::Falling,
            "The rotor cannot provide controlled landing"
        );
        if self.cancel_vtol_takeoff() {
            return Ok(BattleVtolLanding::LaunchCancelled);
        }
        ensure!(
            flight.phase == BattleVtolFlightPhase::Airborne,
            "You're already landed!"
        );
        let motion = self
            .motion
            .context("Place the aircraft on a battlefield first")?;
        let elevation = hex.surface_height();
        ensure!(
            (flight.altitude as i32) <= i32::from(elevation) + 1,
            "You are too high to land here."
        );
        // Landing uses the difference of squared commanded and vertical speeds,
        // with the larger magnitude first; this differs from total airspeed.
        let larger = motion.desired_speed.abs().max(flight.vertical_speed.abs());
        let smaller = motion.desired_speed.abs().min(flight.vertical_speed.abs());
        let horizontal = super::vtol_speed::remaining_speed(larger, smaller)?;
        ensure!(horizontal < 16.0, "You're moving too fast to land.");
        ensure!(
            (-60.0..=10.0).contains(&flight.vertical_speed),
            "You are moving too fast to land. "
        );
        ensure!(motion.speed >= -15.0, "Reverse speed prevents landing");
        ensure!(
            supported_surface(hex),
            "You can't land on this type of terrain."
        );
        // Touchdown stops actual movement but retains the operator's horizontal command.
        self.motion.as_mut().unwrap().speed = 0.0;
        self.vtol_flight = Some(BattleVtolFlight {
            altitude: f64::from(elevation),
            ..BattleVtolFlight::default()
        });
        self.ground_elevation = None;
        self.under_bridge = false;
        Ok(BattleVtolLanding::Touchdown { elevation })
    }
}

/// Both deliberate and emergency landings require a supported surface.
pub(super) fn supported_surface(hex: BattleHex) -> bool {
    matches!(
        hex.terrain,
        Terrain::Grassland | Terrain::Road | Terrain::Building | Terrain::Sand
    )
}
