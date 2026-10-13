//! Rotor material transitions (Total Warfare p. 197): speed loss per rotor hit, the flight
//! stabilizer, rotor destruction, and the engine-damage landing a VTOL makes when rotor hits
//! leave it no movement.
use super::{DamagePhase, PilotingCheck, RotorHit, Vehicle, VehicleMotiveHit, VehicleSection};
use anyhow::{Result, ensure};
use serde::Serialize;

/// Material outcome; the host must start a crash when rotor loss occurs above the surface.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[must_use = "Apply new rotor loss to the flight lifecycle inside the same transaction"]
pub struct RotorDamage {
    pub effect: RotorHit,
    pub speed_before: f64,
    pub speed_after: f64,
    pub lost_rotor: bool,
    /// Rotor hits that left an airborne VTOL without movement forced an engine-damage landing roll.
    pub landing: Option<PilotingCheck>,
    /// Rotor hits left the VTOL without movement; it is immobile for the rest of the game.
    pub grounded: bool,
}

impl Vehicle {
    /// Rotor loss prevents lift, independently of hull destruction and crew casualties.
    pub fn rotor_destroyed(&self) -> bool {
        self.definition().is_vtol()
            && self
                .sections()
                .get(&VehicleSection::Rotor)
                .is_some_and(|section| section.internal == 0)
    }

    /// Flight Stabilizer Hit (tail rotor): cruising speed only, +3 driving and +1 to-hit.
    pub fn tail_rotor_destroyed(&self) -> bool {
        self.tail_rotor_destroyed
    }

    /// Apply rotor material effects; fallen aircraft do not lose further speed from minor hits.
    /// The enclosing flight action owns notices, control limits and crash scheduling.
    pub fn apply_rotor_hit(&mut self, effect: RotorHit, fallen: bool) -> Result<RotorDamage> {
        ensure!(self.definition().is_vtol(), "Rotor damage requires a VTOL");
        let speed_before = self.maximum_speed();
        let was_destroyed = self.rotor_destroyed();
        match effect {
            RotorHit::FlightStabilizer => {
                if !self.tail_rotor_destroyed {
                    self.tail_rotor_destroyed = true;
                    self.piloting_damage = self.piloting_damage.saturating_add(3).min(127);
                    self.gunnery_damage = self.gunnery_damage.saturating_add(1).min(127);
                }
            }
            RotorHit::Damage if !fallen => {
                self.apply_motive_hit(VehicleMotiveHit::SpeedLoss { movement_points: 1 })
            }
            RotorHit::Damage => {}
            RotorHit::Destroy => {
                let _ =
                    self.damage_phase(VehicleSection::Rotor, u16::MAX, DamagePhase::Internal)?;
            }
        }
        Ok(RotorDamage {
            effect,
            speed_before,
            speed_after: self.maximum_speed(),
            lost_rotor: !was_destroyed && self.rotor_destroyed(),
            landing: None,
            grounded: false,
        })
    }
}

impl RotorDamage {
    /// Shared occupant feedback for hit-table and critical-table rotor consequences.
    pub(super) fn message(self) -> &'static str {
        match self.effect {
            RotorHit::Damage if self.grounded => {
                "[fg=red bold]Your rotor takes damage and can no longer keep you moving![reset]"
            }
            RotorHit::Damage => "Your rotor takes damage!",
            RotorHit::FlightStabilizer => {
                "[fg=red bold]Your flight stabilizer is hit! You can only fly at cruising speed and the VTOL is harder to control and aim![reset]"
            }
            RotorHit::Destroy => "Your rotor is destroyed!",
        }
    }
}

/// Share rotor material changes and first-hit control consequences across both damage tables.
/// Rotor hits that reduce an airborne VTOL to no movement are treated as Engine Damage: a
/// landing roll over suitable terrain, otherwise a crash (TW p. 197). The enclosing damage
/// transaction owns validation, crash scheduling and publication.
pub(super) fn apply_in_world(
    world: &mut crate::World,
    id: crate::ObjectId,
    effect: RotorHit,
    extended_piloting: bool,
) -> Result<RotorDamage> {
    use anyhow::Context;
    let unit = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle is unavailable")?;
    let phase = unit.vtol_flight().map(|flight| flight.phase);
    let fallen = phase == Some(super::VtolFlightPhase::Falling);
    let vertical = unit
        .vtol_flight()
        .map_or(0.0, |flight| flight.vertical_speed);
    let maximum = if effect == RotorHit::FlightStabilizer && !unit.tail_rotor_destroyed() {
        Some(super::motion_controls::throttle_maximum(world, id, true)?)
    } else {
        None
    };
    let unit = world.btech.vehicles.get_mut(&id).unwrap();
    let mut report = unit.apply_rotor_hit(effect, fallen)?;
    if let Some(maximum) = maximum {
        unit.constrain_vtol_cruise_at(vertical, maximum)?;
    }
    let grounded = effect == RotorHit::Damage
        && !fallen
        && !unit.rotor_destroyed()
        && unit.maximum_speed() <= 0.0;
    if !grounded {
        return Ok(report);
    }
    report.grounded = true;
    if phase == Some(super::VtolFlightPhase::Airborne) {
        report.landing = super::vtol_emergency::engine_landing(world, id, extended_piloting, true)?;
        return Ok(report);
    }
    unit.apply_motive_hit(VehicleMotiveHit::Immobilize);
    Ok(report)
}
