//! Saved rotorcraft launch and lift-loss transitions; movement owns altitude and crash integration.
use super::{BattlePower, BattleVehicle, BattleVehicleMovement};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// A launch timer and flight mode cannot coexist as independent flags.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BattleVtolFlightPhase {
    #[default]
    Landed,
    Launching {
        remaining: u32,
    },
    /// Physical flight pose; an inserted aircraft may wait here without powered movement.
    Airborne,
    Falling,
}

/// Rotorcraft motion state independent of shared horizontal vehicle motion.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BattleVtolFlight {
    pub phase: BattleVtolFlightPhase,
    pub vertical_speed: f64,
    /// Continuous height in elevation levels, retained across movement events.
    pub altitude: f64,
    /// Shared forced-descent cursor; present only while falling.
    pub fall: Option<super::BattleFreeFall>,
}

impl BattleVtolFlight {
    /// Reject impossible saved timers and nonfinite vertical motion.
    pub(super) fn validate(self) -> Result<()> {
        ensure!(
            self.vertical_speed.is_finite(),
            "Invalid VTOL vertical speed"
        );
        ensure!(
            self.altitude.is_finite()
                && (f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&self.altitude),
            "Invalid VTOL altitude"
        );
        ensure!(
            self.fall.is_some() == (self.phase == BattleVtolFlightPhase::Falling),
            "VTOL fall cursor does not match flight phase"
        );
        if let Some(fall) = self.fall {
            ensure!(
                !fall.grounded() && fall.elevation() == self.altitude as i32,
                "VTOL fall cursor differs from altitude"
            );
        }
        match self.phase {
            BattleVtolFlightPhase::Landed => ensure!(
                self.vertical_speed == 0.0,
                "Landed VTOL retains vertical speed"
            ),
            BattleVtolFlightPhase::Launching { remaining } => ensure!(
                (1..=65536).contains(&remaining) && self.vertical_speed == 0.0,
                "Invalid VTOL launch timer"
            ),
            _ => {}
        }
        Ok(())
    }
}

/// The enclosing action publishes launch messages after committing state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[must_use = "Publish launch results with the enclosing flight action"]
pub enum BattleVtolTakeoff {
    Idle,
    Waiting { remaining: u32 },
    Aborted { reason: String },
    LiftedOff,
}

impl BattleVehicle {
    /// Fuel admission shared by deliberate takeoff and landing maneuvers.
    pub(super) fn has_vtol_fuel(&self, free_fusion_fuel: bool) -> bool {
        (free_fusion_fuel && !self.definition().has_special("ICEEngine_Tech"))
            || self.vtol_fuel().is_some_and(|fuel| fuel.remaining() > 0)
    }

    /// Saved aircraft mode; absent for ground vehicles.
    pub fn vtol_flight(&self) -> Option<BattleVtolFlight> {
        self.vtol_flight
    }

    /// Shared admission checks run both at request time and before committing liftoff.
    fn check_vtol_takeoff(&self, underground: bool, free_fusion_fuel: bool) -> Result<()> {
        ensure!(
            self.definition().is_vtol()
                && self.definition().movement == BattleVehicleMovement::Vtol,
            "Only flying VTOLs can take off"
        );
        ensure!(
            self.position().is_some() && self.power() == BattlePower::Running,
            "Takeoff requires a placed, running VTOL"
        );
        ensure!(
            !self.is_destroyed() && !self.rotor_destroyed() && self.maximum_speed() > 10.75,
            "The rotor cannot provide lift"
        );
        ensure!(!underground, "The ceiling is too low for takeoff");
        ensure!(
            self.has_vtol_fuel(free_fusion_fuel),
            "The VTOL is out of fuel"
        );
        Ok(())
    }

    /// Queue takeoff; zero delay still lifts off on the next one-second event.
    /// The host owns pilot authority and permission to override the ordinary zero delay.
    pub fn begin_vtol_takeoff(
        &mut self,
        underground: bool,
        free_fusion_fuel: bool,
        delay: u16,
    ) -> Result<()> {
        super::fortification::require_unfortified(self.fortified)?;
        self.check_vtol_takeoff(underground, free_fusion_fuel)?;
        ensure!(
            self.vtol_flight
                .is_some_and(|flight| flight.phase == BattleVtolFlightPhase::Landed),
            "The VTOL has not landed or is already launching"
        );
        self.vtol_flight = Some(BattleVtolFlight {
            fall: None,
            phase: BattleVtolFlightPhase::Launching {
                remaining: u32::from(delay) + 1,
            },
            vertical_speed: 0.0,
            altitude: self.vtol_flight.unwrap().altitude,
        });
        Ok(())
    }

    /// Advance a queued launch, revalidating conditions before producing vertical motion.
    pub fn advance_vtol_takeoff(
        &mut self,
        underground: bool,
        free_fusion_fuel: bool,
    ) -> Result<BattleVtolTakeoff> {
        ensure!(self.definition().is_vtol(), "Takeoff requires a VTOL");
        let Some(BattleVtolFlight {
            fall: None,
            phase: BattleVtolFlightPhase::Launching { remaining },
            ..
        }) = self.vtol_flight
        else {
            return Ok(BattleVtolTakeoff::Idle);
        };
        if let Err(error) = self.check_vtol_takeoff(underground, free_fusion_fuel) {
            self.vtol_flight = Some(BattleVtolFlight {
                altitude: self.vtol_flight.unwrap().altitude,
                ..BattleVtolFlight::default()
            });
            return Ok(BattleVtolTakeoff::Aborted {
                reason: error.to_string(),
            });
        }
        if remaining > 1 {
            self.vtol_flight.as_mut().unwrap().phase = BattleVtolFlightPhase::Launching {
                remaining: remaining - 1,
            };
            return Ok(BattleVtolTakeoff::Waiting {
                remaining: remaining - 1,
            });
        }
        // Liftoff cancels horizontal translation but retains the pilot's selected bearing.
        if let Some(motion) = &mut self.motion {
            motion.stop_translation();
        }
        self.vtol_flight = Some(BattleVtolFlight {
            fall: None,
            phase: BattleVtolFlightPhase::Airborne,
            vertical_speed: 60.0,
            altitude: self.vtol_flight.unwrap().altitude,
        });
        Ok(BattleVtolTakeoff::LiftedOff)
    }

    /// Landing during launch cancels only the pending takeoff, without moving the aircraft.
    pub fn cancel_vtol_takeoff(&mut self) -> bool {
        if !self
            .vtol_flight
            .is_some_and(|flight| matches!(flight.phase, BattleVtolFlightPhase::Launching { .. }))
        {
            return false;
        }
        self.vtol_flight = Some(BattleVtolFlight {
            altitude: self.vtol_flight.unwrap().altitude,
            ..BattleVtolFlight::default()
        });
        true
    }

    /// Loss of lift starts one fall in flight or cancels a launch still on the surface.
    pub(super) fn lose_vtol_lift(&mut self) {
        let Some(flight) = &mut self.vtol_flight else {
            return;
        };
        match flight.phase {
            BattleVtolFlightPhase::Airborne => {
                flight.fall = Some(super::BattleFreeFall::new(flight.altitude as i32));
                flight.phase = BattleVtolFlightPhase::Falling;
                flight.vertical_speed = 0.0;
            }
            BattleVtolFlightPhase::Launching { .. } => {
                *flight = BattleVtolFlight {
                    altitude: flight.altitude,
                    ..BattleVtolFlight::default()
                }
            }
            _ => {}
        }
    }
}

impl BattleVehicle {
    /// Advance the shared descent clock, braking when power, rotor and fuel provide lift.
    /// Recovery clears the cursor; impact leaves state unchanged until host damage commits.
    pub fn advance_vtol_fall(
        &mut self,
        surface: i32,
        free_fusion_fuel: bool,
    ) -> Result<super::BattleFreeFallStep> {
        let mut flight = self
            .vtol_flight
            .ok_or_else(|| anyhow::anyhow!("Falling requires a VTOL"))?;
        ensure!(
            flight.phase == BattleVtolFlightPhase::Falling,
            "Aircraft is not falling"
        );
        let mut fall = flight
            .fall
            .ok_or_else(|| anyhow::anyhow!("Aircraft fall cursor is unavailable"))?;
        let lift = !self.is_destroyed()
            && self.power() == super::BattlePower::Running
            && !self.rotor_destroyed()
            && self.has_vtol_fuel(free_fusion_fuel);
        let step = fall.advance_with_lift(surface, lift)?;
        if step == super::BattleFreeFallStep::Recovered {
            flight.phase = BattleVtolFlightPhase::Airborne;
            flight.fall = None;
            self.vtol_flight = Some(flight);
            return Ok(step);
        }
        if matches!(step, super::BattleFreeFallStep::Impact { .. }) {
            return Ok(step);
        }
        if step == super::BattleFreeFallStep::Descending {
            flight.altitude = f64::from(fall.elevation());
        }
        flight.fall = Some(fall);
        self.vtol_flight = Some(flight);
        Ok(step)
    }
}

impl BattleVehicle {
    /// Validate flight against the shared vehicle lifecycle before publishing live state.
    pub(super) fn validate_flight_state(&self) -> Result<()> {
        self.validate_ground_descent()?;
        let Some(flight) = self.vtol_flight else {
            return Ok(());
        };
        flight.validate()?;
        ensure!(
            !self.under_bridge() && self.ground_elevation.is_none(),
            "Aircraft retains ground-vehicle elevation state"
        );
        if flight.phase == BattleVtolFlightPhase::Landed {
            return Ok(());
        }
        ensure!(
            (self.position().is_some() || self.detached) && self.motion().is_some(),
            "Active aircraft flight requires placement"
        );
        ensure!(
            self.definition().movement == BattleVehicleMovement::Vtol,
            "Stationary aircraft cannot fly"
        );
        if flight.phase == BattleVtolFlightPhase::Falling {
            ensure!(
                flight.vertical_speed == 0.0,
                "Falling aircraft retains powered vertical motion"
            );
            return Ok(());
        }
        if flight.phase == BattleVtolFlightPhase::Airborne && self.idle_flight_controls() {
            return Ok(());
        }
        ensure!(
            self.power() == BattlePower::Running && !self.is_destroyed() && !self.rotor_destroyed(),
            "Powered aircraft flight requires power and an intact rotor"
        );
        Ok(())
    }
}

/// An unpowered aircraft can retain requested controls while its physical pose waits for startup.
/// Scenario insertion and subsequent coordinate edits share this geometric, non-propelled state.
pub(super) fn idle_controls(
    power: BattlePower,
    flight: Option<BattleVtolFlight>,
    motion: Option<super::BattleMotion>,
) -> bool {
    power != BattlePower::Running
        && flight.is_some_and(|flight| {
            matches!(
                flight.phase,
                BattleVtolFlightPhase::Landed | BattleVtolFlightPhase::Airborne
            ) && flight.vertical_speed == 0.0
        })
        && motion.is_some_and(|motion| motion.speed == 0.0)
}

impl BattleVehicle {
    /// Requested throttle and facing are inert until the aircraft has power.
    pub(super) fn idle_flight_controls(&self) -> bool {
        idle_controls(self.power, self.vtol_flight, self.motion)
    }
}
