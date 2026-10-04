//! Rotor material transitions share vehicle section damage and speed-loss accounting.
use super::{DamagePhase, RotorHit, Vehicle, VehicleMotiveHit, VehicleSection};
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
    pub repeated: bool,
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

    /// Tail rotor damage is retained separately from main rotor material.
    pub fn tail_rotor_destroyed(&self) -> bool {
        self.tail_rotor_destroyed
    }

    /// Apply rotor material effects; fallen aircraft do not lose further speed from minor hits.
    /// The enclosing flight action owns notices, tail-rotor control limits and crash scheduling.
    pub fn apply_rotor_hit(&mut self, effect: RotorHit, fallen: bool) -> Result<RotorDamage> {
        ensure!(self.definition().is_vtol(), "Rotor damage requires a VTOL");
        let speed_before = self.maximum_speed();
        let was_destroyed = self.rotor_destroyed();
        let repeated = match effect {
            RotorHit::TailRotor => self.tail_rotor_destroyed,
            _ => was_destroyed,
        };
        let effect = if effect == RotorHit::Damage && speed_before <= 10.75 {
            RotorHit::Destroy
        } else {
            effect
        };
        match effect {
            RotorHit::TailRotor => self.tail_rotor_destroyed = true,
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
            repeated,
        })
    }
}

impl RotorDamage {
    /// Shared occupant feedback for hit-table and critical-table rotor consequences.
    pub(super) fn message(self) -> &'static str {
        match self.effect {
            RotorHit::Damage => "Your rotor takes damage!",
            RotorHit::TailRotor if self.repeated => "Your damaged tail rotor suffers more damage!",
            RotorHit::TailRotor => {
                "[fg=red bold]Your tail rotor is damaged, slowing you down![reset]"
            }
            RotorHit::Destroy => "Your rotor is destroyed!",
        }
    }
}

/// Share rotor material changes and first-hit control consequences across both damage tables.
/// The enclosing damage transaction owns validation, crash scheduling and publication.
pub(super) fn apply_in_world(
    world: &mut crate::World,
    id: crate::ObjectId,
    effect: RotorHit,
) -> Result<RotorDamage> {
    use anyhow::Context;
    let unit = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle is unavailable")?;
    let fallen = unit
        .vtol_flight()
        .is_some_and(|flight| flight.phase == super::VtolFlightPhase::Falling);
    let vertical = unit
        .vtol_flight()
        .map_or(0.0, |flight| flight.vertical_speed);
    let maximum = if effect == RotorHit::TailRotor && !unit.tail_rotor_destroyed() {
        Some(super::motion_controls::throttle_maximum(world, id, true)?)
    } else {
        None
    };
    let unit = world.btech.vehicles.get_mut(&id).unwrap();
    let report = unit.apply_rotor_hit(effect, fallen)?;
    if let Some(maximum) = maximum {
        unit.constrain_vtol_cruise_at(vertical, maximum)?;
    }
    Ok(report)
}
