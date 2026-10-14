//! Deliberate rotorcraft landing checks and material touchdown, separate from crash damage.
use super::{Ground, Hex, StructureKind, Vehicle, VtolFlight, VtolFlightPhase};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// The enclosing flight action applies altitude and publishes landing effects transactionally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[must_use = "Apply touchdown elevation and publish landing effects in the enclosing action"]
pub enum VtolLanding {
    LaunchCancelled,
    Touchdown { elevation: i16 },
}

impl Vehicle {
    /// Land on the supplied current map surface, or cancel an outstanding launch.
    /// The host owns map lookup, cockpit authority, messages, callbacks and landing mines.
    /// Failed admission does not spend fuel, draw dice, or alter motion.
    pub fn land_vtol(&mut self, hex: Hex) -> Result<VtolLanding> {
        ensure!(self.definition().is_vtol(), "Landing requires a VTOL");
        ensure!(
            self.has_vtol_fuel(),
            "You lack fuel to maneuver for landing!"
        );
        let flight = self
            .vtol_flight
            .context("Aircraft flight state is unavailable")?;
        ensure!(!self.rotor_destroyed(), "The rotor's dead!");
        ensure!(
            !self.is_destroyed() && flight.phase != VtolFlightPhase::Falling,
            "The rotor cannot provide controlled landing"
        );
        if self.cancel_vtol_takeoff() {
            return Ok(VtolLanding::LaunchCancelled);
        }
        ensure!(
            flight.phase == VtolFlightPhase::Airborne,
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
        self.vtol_flight = Some(VtolFlight {
            altitude: f64::from(elevation),
            ..VtolFlight::default()
        });
        self.ground_elevation = None;
        self.under_bridge = false;
        Ok(VtolLanding::Touchdown { elevation })
    }
}

/// Where a VTOL may land (Total Warfare p. 54): open ground or a building roof, with no fire
/// or smoke over it.
pub(super) fn supported_surface(hex: Hex) -> bool {
    hex.is_open_ground()
        || (hex.overlay().is_none()
            && hex
                .structure()
                .is_some_and(|structure| structure.kind == StructureKind::Building))
}

/// Where a VTOL with Engine Damage may attempt its emergency landing (Total Warfare p. 197):
/// anywhere it could land normally, or bare rough ground clear of fire and smoke.
pub(super) fn engine_landing_surface(hex: Hex) -> bool {
    supported_surface(hex)
        || (hex.is_bare()
            && hex.overlay().is_none()
            && matches!(hex.ground(), Ground::Rough | Ground::UltraRough))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::{DecorationKind, Terrain};

    #[test]
    fn aircraft_land_on_open_ground_or_roofs_clear_of_fire_and_smoke() {
        for terrain in [Terrain::Clear, Terrain::Road, Terrain::Building] {
            let hex = Hex::new(terrain, 1);
            assert!(supported_surface(hex), "{terrain:?}");
            for kind in [DecorationKind::Fire, DecorationKind::Smoke] {
                assert!(
                    !supported_surface(hex.with_overlay(Some(kind))),
                    "{terrain:?}"
                );
            }
        }
        assert!(!supported_surface(Hex::new(Terrain::LightWoods, 0)));
    }

    /// Engine Damage adds rough ground to the landing surfaces (Total Warfare p. 197).
    #[test]
    fn engine_damage_may_land_on_rough_ground_but_not_in_woods_or_water() {
        let rough = Hex::new(Terrain::Rough, 1);
        assert!(!supported_surface(rough));
        assert!(engine_landing_surface(rough));
        assert!(!engine_landing_surface(
            rough.with_overlay(Some(DecorationKind::Fire))
        ));
        assert!(engine_landing_surface(Hex::new(Terrain::Clear, 0)));
        assert!(!engine_landing_surface(Hex::new(Terrain::LightWoods, 0)));
        assert!(!engine_landing_surface(Hex::new(Terrain::Water, 1)));
    }
}
