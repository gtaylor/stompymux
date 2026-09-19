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
        Self::resolve_with(template, false)
    }

    pub(crate) fn resolve_contract(template: &BattleVehicleTemplate) -> Result<Self> {
        Self::resolve_with(template, true)
    }

    fn resolve_with(template: &BattleVehicleTemplate, contract: bool) -> Result<Self> {
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
                        let bin = if contract {
                            match AmmunitionBin::from_critical_contract(name, critical, location) {
                                Ok(bin) => Some(bin),
                                Err(_)
                                    if BattlePart::parse(&critical.equipment).is_ok_and(
                                        |part| part.kind == BattlePartKind::Ammunition,
                                    ) =>
                                {
                                    None
                                }
                                Err(error) => return Err(error),
                            }
                        } else {
                            Some(AmmunitionBin::from_critical(name, critical, location)?)
                        };
                        loadout.ammunition.extend(bin);
                        return Ok(());
                    }
                    ensure!(
                        if contract {
                            critical.data == "-" || critical.data.parse::<i32>().is_ok()
                        } else {
                            critical.data == "-"
                                || (critical.equipment.eq_ignore_ascii_case("ArtemisIV")
                                    && critical.data.parse::<u8>().is_ok())
                        },
                        "Unsupported critical data {}",
                        critical.data
                    );
                    // Manufacturer-qualified spellings (for example "Agra.IS.PPC") name the
                    // same weapon as their technology-prefixed form; the brand column
                    // already carries the manufacturer identity.
                    let equipment = super::loadout::unbranded_weapon_name(&critical.equipment)
                        .map(str::to_owned)
                        .unwrap_or_else(|| critical.equipment.clone());
                    if super::equipment::strip_name_prefix(&equipment, "IS.").is_some()
                        || super::equipment::strip_name_prefix(&equipment, "CL.").is_some()
                    {
                        let weapon = match BattleWeapon::parse(&equipment) {
                            Ok(weapon) => weapon,
                            Err(_)
                                if contract
                                    && (super::loadout::contract_raw_weapon(
                                        &critical.equipment,
                                    ) || BattlePart::parse(&critical.equipment)
                                        .is_ok_and(|part| part.kind == BattlePartKind::Weapon)) =>
                            {
                                return Ok(());
                            }
                            Err(error) => return Err(error),
                        };
                        loadout.weapons.push(if contract {
                            WeaponMount::from_critical_contract(weapon, critical, vec![location])?
                        } else {
                            WeaponMount::from_critical(weapon, critical, vec![location])?
                        });
                        return Ok(());
                    }
                    let system = match BattleSystem::parse(&critical.equipment) {
                        Ok(system) => system,
                        Err(_)
                            if contract
                                && BattlePart::parse(&critical.equipment).is_ok_and(|part| {
                                    matches!(
                                        part.kind,
                                        BattlePartKind::Component | BattlePartKind::Bomb
                                    )
                                }) =>
                        {
                            return Ok(());
                        }
                        Err(error) => return Err(error),
                    };
                    ensure!(critical.modes.is_empty(), "Unsupported system mode");
                    loadout.systems.push(SystemCritical {
                        location,
                        system,
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
