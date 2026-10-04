//! Ground-vehicle firing geometry follows hull faces and the saved turret bearing.
use super::{Vehicle, VehicleCriticalLocation, VehicleSection, WeaponMount};
use anyhow::{Context, Result, ensure};

/// Firing-arc geometry for vehicle weapon mounts.
pub trait VehicleMountArcs {
    /// Test vehicle mounting geometry; rear-mount flags do not override a vehicle's hull face.
    /// Bearings use nearest whole degrees, facing uses whole degrees, and turret arcs span 60 degrees.
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
pub(super) fn turret_arc(heading: f64, bearing: f64) -> bool {
    let angle =
        (bearing.rem_euclid(360.0).round() - heading.rem_euclid(360.0).trunc()).rem_euclid(360.0);
    angle <= 30.0 || angle >= 330.0
}
