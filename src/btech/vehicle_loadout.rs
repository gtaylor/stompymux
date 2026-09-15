//! Vehicle slots resolve complete weapons and independent bins using shared equipment validation.
use super::*;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// A zero-based equipment slot in a vehicle hull face or turret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct VehicleCriticalLocation {
    pub section: BattleVehicleSection,
    pub slot: u8,
}

/// Resolved equipment only; construction, mass, systems and live vehicle simulation require further validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleVehicleLoadout {
    pub weapons: Vec<WeaponMount<VehicleCriticalLocation>>,
    pub ammunition: Vec<AmmunitionBin<VehicleCriticalLocation>>,
    pub systems: Vec<SystemCritical<VehicleCriticalLocation>>,
}

impl BattleVehicleLoadout {
    /// Resolve each vehicle slot independently in hull-face and slot order.
    pub fn resolve(template: &BattleVehicleTemplate) -> Result<Self> {
        let mut loadout = Self {
            weapons: Vec::new(),
            ammunition: Vec::new(),
            systems: Vec::new(),
        };
        for (&section, layout) in &template.sections {
            for (&slot, critical) in &layout.criticals {
                let location = VehicleCriticalLocation { section, slot };
                (|| -> Result<()> {
                    ensure!(slot < 12, "Vehicle equipment slot is out of bounds");
                    if let Some(name) =
                        super::equipment::strip_name_prefix(&critical.equipment, "Ammo_")
                    {
                        loadout
                            .ammunition
                            .push(AmmunitionBin::from_critical(name, critical, location)?);
                        return Ok(());
                    }
                    ensure!(
                        critical.data == "-"
                            || (critical.equipment.eq_ignore_ascii_case("ArtemisIV")
                                && critical.data.parse::<u8>().is_ok()),
                        "Unsupported critical data {}",
                        critical.data
                    );
                    if super::equipment::strip_name_prefix(&critical.equipment, "IS.").is_some()
                        || super::equipment::strip_name_prefix(&critical.equipment, "CL.").is_some()
                    {
                        let weapon = BattleWeapon::parse(&critical.equipment)?;
                        loadout.weapons.push(WeaponMount::from_critical(
                            weapon,
                            critical,
                            vec![location],
                        )?);
                        return Ok(());
                    }
                    ensure!(critical.modes.is_empty(), "Unsupported system mode");
                    loadout.systems.push(SystemCritical {
                        location,
                        system: BattleSystem::parse(&critical.equipment)?,
                        brand: critical.brand,
                    });
                    Ok(())
                })()
                .with_context(|| {
                    format!(
                        "{} slot {} ({})",
                        section.name(),
                        slot + 1,
                        critical.equipment
                    )
                })?;
            }
        }
        Ok(loadout)
    }
}
