//! Vehicle slots resolve complete weapons and independent bins using shared equipment validation.
use super::*;
use anyhow::{Context, Result, bail, ensure};

/// A zero-based equipment slot in a vehicle hull face or turret.
pub type VehicleCriticalLocation = CriticalLocation<VehicleSection>;

/// Resolved equipment only; construction, mass, systems and live vehicle simulation require further validation.
pub type VehicleLoadout = ResolvedLoadout<VehicleCriticalLocation>;

impl VehicleLoadout {
    /// Resolve each vehicle slot independently in hull-face and slot order.
    pub fn resolve(template: &VehicleTemplate) -> Result<Self> {
        Self::resolve_with(template, false)
    }

    pub fn resolve_contract(template: &VehicleTemplate) -> Result<Self> {
        Self::resolve_with(template, true)
    }

    fn resolve_with(template: &VehicleTemplate, contract: bool) -> Result<Self> {
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
                                    if Part::parse(&critical.equipment)
                                        .is_ok_and(|part| part.kind == PartKind::Ammunition) =>
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
                    let equipment = critical.equipment.clone();
                    if super::equipment::strip_name_prefix(&equipment, "IS.").is_some()
                        || super::equipment::strip_name_prefix(&equipment, "CL.").is_some()
                    {
                        let weapon = match Weapon::parse(&equipment) {
                            Ok(weapon) => weapon,
                            Err(_)
                                if contract
                                    && (super::loadout::contract_raw_weapon(
                                        &critical.equipment,
                                    ) || Part::parse(&critical.equipment)
                                        .is_ok_and(|part| part.kind == PartKind::Weapon)) =>
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
                    let system = match System::named(&critical.equipment) {
                        Some(system) => system,
                        None if contract
                            && Part::parse(&critical.equipment).is_ok_and(|part| {
                                matches!(part.kind, PartKind::Component | PartKind::Bomb)
                            }) =>
                        {
                            return Ok(());
                        }
                        None => bail!("Unsupported equipment {}", critical.equipment),
                    };
                    ensure!(critical.modes.is_empty(), "Unsupported system mode");
                    loadout.systems.push(SystemCritical { location, system });
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
