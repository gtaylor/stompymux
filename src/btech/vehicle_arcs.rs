//! Ground-vehicle firing geometry follows hull faces and the saved turret bearing
//! (Total Warfare p. 105).
use super::{Vehicle, VehicleCriticalLocation, VehicleSection, WeaponMount};
use anyhow::{Context, Result, ensure};

/// Firing-arc geometry for vehicle weapon mounts.
pub trait VehicleMountArcs {
    /// Test vehicle mounting geometry; rear-mount flags do not override a vehicle's hull face.
    /// Bearings use nearest whole degrees and facing uses whole degrees. Hull faces fire only
    /// into their own 60-degree side arcs or the 120-degree front and rear arcs; turret
    /// weapons use the forward arc centred on the turret's own facing.
    fn bears_on(&self, heading: f64, bearing: f64, turret_heading: Option<f64>) -> Result<bool>;
}

impl VehicleMountArcs for WeaponMount<VehicleCriticalLocation> {
    fn bears_on(&self, heading: f64, bearing: f64, turret_heading: Option<f64>) -> Result<bool> {
        ensure!(
            heading.is_finite() && bearing.is_finite() && turret_heading.is_none_or(f64::is_finite),
            "Invalid firing direction"
        );
        let section = self
            .criticals
            .first()
            .context("Weapon mount has no criticals")?
            .section;
        let facing = if section == VehicleSection::Turret {
            let Some(turret) = turret_heading else {
                return Ok(false);
            };
            turret
        } else {
            heading
        };
        if section == VehicleSection::Turret {
            return Ok(turret_arc(facing, bearing));
        }
        let angle = (bearing.rem_euclid(360.0).round() - facing.rem_euclid(360.0).trunc())
            .rem_euclid(360.0);
        Ok(match section {
            VehicleSection::Rotor => {
                anyhow::bail!("Rotor-mounted weapons require flight firing geometry")
            }
            VehicleSection::Front => angle <= 60.0 || angle >= 300.0,
            VehicleSection::Rear => angle > 120.0 && angle < 240.0,
            VehicleSection::Left => (240.0..300.0).contains(&angle),
            VehicleSection::Right => angle > 60.0 && angle <= 120.0,
            VehicleSection::Turret => unreachable!("turret handled above"),
        })
    }
}

impl Vehicle {
    /// Test a zero-based weapon index against current placement, section survival and facing.
    /// This is geometry only: combat must separately validate power, recycle, targeting and ammunition.
    pub fn weapon_bears_on(&self, weapon: usize, bearing: f64) -> Result<bool> {
        ensure!(bearing.is_finite(), "Invalid firing direction");
        let loadout = self.loadout()?;
        let mount = loadout
            .weapons
            .get(weapon)
            .context("Invalid weapon number")?;
        let motion = self.motion().context("Vehicle is not placed")?;
        let location = *mount
            .criticals
            .first()
            .context("Weapon mount has no criticals")?;
        if self.is_destroyed() || self.critical_unavailable(location) {
            return Ok(false);
        }
        mount.bears_on(motion.heading, bearing, self.turret_heading())
    }
}

/// Shared whole-degree turret geometry, independent of whether a weapon is installed.
/// A turret fires into the forward arc (three hexsides, 120 degrees) defined by the hexside
/// it points at rather than the hull's facing (Total Warfare p. 105).
pub(super) fn turret_arc(heading: f64, bearing: f64) -> bool {
    let angle =
        (bearing.rem_euclid(360.0).round() - heading.rem_euclid(360.0).trunc()).rem_euclid(360.0);
    angle <= 60.0 || angle >= 300.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The turret arc is the forward arc rotated to the turret's facing.
    #[test]
    fn turret_arc_spans_three_hexsides_around_the_turret_facing() {
        for inside in [0.0, 60.0, 300.0, 359.0, 60.49, 299.5] {
            assert!(turret_arc(0.0, inside), "{inside}");
        }
        for outside in [60.5, 61.0, 120.0, 180.0, 240.0, 299.49] {
            assert!(!turret_arc(0.0, outside), "{outside}");
        }
        assert!(turret_arc(90.0, 150.0));
        assert!(turret_arc(90.0, 30.0));
        assert!(!turret_arc(90.0, 29.0));
        assert!(!turret_arc(90.0, 151.0));
        assert!(turret_arc(1.99, 61.0));
    }
}
