//! Rotorcraft movement proposals share horizontal geometry and expose altitude consequences.
//! Vertical movement follows Total Warfare pp. 49 and 54: one MP climbs or descends one level,
//! and on this engine's time scale one MP of vertical speed (10.75 kph) covers one level in
//! 60 seconds, exactly as one MP of horizontal speed covers one hex.
use super::{Hex, Motion, Vehicle, VtolFlightPhase};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Kilometres per hour of vertical speed that climb one level per second: 10.75 kph (one MP)
/// is one level per 60 seconds, the same scale that carries one MP across one hex.
pub const VERTICAL_KPH_PER_LEVEL_SECOND: f64 = 645.0;

/// The highest elevation a VTOL can reach (Total Warfare p. 54).
pub const MAXIMUM_VTOL_ELEVATION: f64 = 500.0;

/// Surface decision for the host to resolve before committing aircraft movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VtolSurfaceContact {
    Clear,
    /// The VTOL has come down to the water: it crashes and is lost (TW p. 54).
    Water,
    /// The VTOL has flown into a woods hex at or below the treetops (TW p. 54): it crashes.
    Forest,
    /// The VTOL has entered a hex horizontally at or below the level of its terrain or a
    /// building in it (TW p. 54): it crashes.
    Elevation,
    /// The VTOL has come down to the terrain's level: it lands if it can (TW p. 54), and
    /// otherwise crashes with this many levels of severity.
    Ground {
        fall_levels: u32,
    },
}

/// A pure proposal: the host traces crossed hexes before publishing or committing movement.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[must_use = "Trace and resolve surface contacts before committing flight movement"]
pub struct VtolMotionStep {
    pub motion: Motion,
    /// Continuous altitude in elevation levels, retaining fractions across movement events.
    pub altitude: f64,
    /// Vertical speed after applying the elevation ceiling.
    pub vertical_speed: f64,
    pub ceiling_reached: bool,
    /// Inputs bind a proposal to the material state from which it was calculated.
    pub(super) origin: (Motion, super::VtolFlight, super::Position),
}

impl VtolMotionStep {
    /// Integer altitude truncates toward zero, including fractional levels below water.
    pub fn elevation(self) -> i32 {
        self.altitude as i32
    }

    /// Check the surface of the hex the VTOL is in; horizontal path traversal remains the
    /// host's responsibility. A VTOL whose elevation is the terrain's level is on the ground
    /// (TW p. 54): descending or hovering there is a landing, or a crash over water or terrain
    /// it cannot land in, while a climbing VTOL at that elevation is still lifting off.
    pub fn surface_contact(self, hex: Hex) -> VtolSurfaceContact {
        let altitude = self.elevation();
        let descending = self.vertical_speed <= 0.0;
        if hex.is_open_water() {
            if altitude < i32::from(hex.water_line())
                || (altitude == i32::from(hex.water_line()) && descending)
            {
                return VtolSurfaceContact::Water;
            }
            return VtolSurfaceContact::Clear;
        }
        if let Some(deck) = hex.deck_height() {
            // The band under the span is open; the ground beneath it is still the ground, the
            // underside of the deck stops a VTOL rising into it, and the deck itself is a
            // surface it comes down on.
            let deck = i32::from(deck);
            let underside = altitude == deck - 1;
            let surface =
                (altitude <= i32::from(hex.water_line()) || altitude == deck) && descending;
            if underside || surface {
                return VtolSurfaceContact::Ground {
                    fall_levels: self.fall_levels(),
                };
            }
            return VtolSurfaceContact::Clear;
        }
        let surface = i32::from(hex.surface_height());
        if hex.is_woods() && !hex.is_road() && altitude <= surface + i32::from(hex.foliage_height())
        {
            return VtolSurfaceContact::Forest;
        }
        if altitude > surface || (altitude == surface && !descending) {
            return VtolSurfaceContact::Clear;
        }
        VtolSurfaceContact::Ground {
            fall_levels: self.fall_levels(),
        }
    }

    /// Crash severity of an uncontrolled descent: one level plus one per MP of descent.
    fn fall_levels(self) -> u32 {
        1u32.saturating_add((self.vertical_speed.abs() / 10.75) as u32)
    }
}

impl Vehicle {
    /// Project an airborne movement event after horizontal turning and acceleration.
    /// Uses the saved continuous altitude; commit only after resolving terrain traversal.
    /// Falling uses the separate fall event.
    pub fn vtol_motion_step(&self) -> Result<VtolMotionStep> {
        ensure!(
            self.definition().is_vtol(),
            "Flight movement requires a VTOL"
        );
        let flight = self
            .vtol_flight
            .context("Aircraft flight state is unavailable")?;
        ensure!(
            flight.phase == VtolFlightPhase::Airborne && self.power() == super::Power::Running,
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
        motion.point = motion.project_step()?;
        motion.point.containing_hex()?;
        let mut altitude = flight.altitude + flight.vertical_speed / VERTICAL_KPH_PER_LEVEL_SECOND;
        ensure!(
            altitude.is_finite() && altitude >= f64::from(i32::MIN),
            "Flight altitude exceeds coordinate limits"
        );
        let ceiling_reached = altitude > MAXIMUM_VTOL_ELEVATION;
        if ceiling_reached {
            altitude = MAXIMUM_VTOL_ELEVATION;
        }
        Ok(VtolMotionStep {
            motion,
            altitude,
            vertical_speed: if ceiling_reached {
                0.0
            } else {
                flight.vertical_speed
            },
            ceiling_reached,
            origin,
        })
    }

    /// Commit a still-current movement proposal after the host has resolved its entire path.
    /// This material operation does not authorize map bounds or publish terrain consequences.
    pub fn commit_vtol_motion(&mut self, step: VtolMotionStep) -> Result<()> {
        self.commit_vtol_motion_at(step, step.motion.point)
    }

    /// Commit an observed proposal at the map adapter's resolved boundary position.
    pub(super) fn commit_vtol_motion_at(
        &mut self,
        step: VtolMotionStep,
        point: super::Point,
    ) -> Result<()> {
        ensure!(
            self.vtol_motion_step()? == step,
            "Stale or altered flight proposal"
        );
        let coordinate = point.containing_hex()?;
        let position = super::Position {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::Terrain;

    /// A step at a given altitude and vertical speed over a hex, for surface decisions.
    fn step(altitude: f64, vertical_speed: f64) -> VtolMotionStep {
        let motion = Motion::stationary(super::super::HexCoordinate { x: 0, y: 0 }.center());
        VtolMotionStep {
            motion,
            altitude,
            vertical_speed,
            ceiling_reached: false,
            origin: (
                motion,
                super::super::VtolFlight::default(),
                super::super::Position {
                    map: crate::ObjectId(0),
                    x: 0,
                    y: 0,
                },
            ),
        }
    }

    /// Elevation equal to the terrain is the ground: a descent there lands, a climb lifts off.
    #[test]
    fn ground_level_lands_unless_the_aircraft_is_climbing() {
        let hex = Hex::new(Terrain::Clear, 2);
        assert_eq!(
            step(3.5, 0.0).surface_contact(hex),
            VtolSurfaceContact::Clear
        );
        assert_eq!(
            step(2.5, 0.0).surface_contact(hex),
            VtolSurfaceContact::Ground { fall_levels: 1 }
        );
        assert_eq!(
            step(2.0, -21.5).surface_contact(hex),
            VtolSurfaceContact::Ground { fall_levels: 3 }
        );
        assert_eq!(
            step(2.1, 60.0).surface_contact(hex),
            VtolSurfaceContact::Clear
        );
    }

    /// Water at the VTOL's level is a crash; the treetops of a woods hex are an obstacle.
    #[test]
    fn water_and_canopy_are_contacts() {
        let water = Hex::new(Terrain::Water, 1);
        assert_eq!(
            step(0.5, 0.0).surface_contact(water),
            VtolSurfaceContact::Water
        );
        assert_eq!(
            step(0.5, 30.0).surface_contact(water),
            VtolSurfaceContact::Clear
        );
        assert_eq!(
            step(-1.5, 30.0).surface_contact(water),
            VtolSurfaceContact::Water
        );
        let woods = Hex::new(Terrain::LightWoods, 0);
        assert_eq!(
            step(2.9, 60.0).surface_contact(woods),
            VtolSurfaceContact::Forest
        );
        assert_eq!(
            step(3.0, 0.0).surface_contact(woods),
            VtolSurfaceContact::Clear
        );
    }
}
