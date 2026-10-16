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
                    ensure!(
                        slot < VEHICLE_SECTION_SLOT_LIMIT,
                        "Vehicle equipment slot is out of bounds"
                    );
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
                            critical.data == "-"
                                || critical.data.parse::<i32>().is_ok()
                                || super::loadout::system_data_supported(critical)
                        } else {
                            super::loadout::system_data_supported(critical)
                        },
                        "Unsupported critical data {}",
                        critical.data
                    );
                    let equipment = critical.equipment.clone();
                    if (super::equipment::strip_name_prefix(&equipment, "IS.").is_some()
                        || super::equipment::strip_name_prefix(&equipment, "CL.").is_some())
                        && System::named(&equipment).is_none()
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
                    let (system, tech_base) = match System::installation(&critical.equipment) {
                        Some(installation) => installation,
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
                    loadout.systems.push(SystemCritical::resolve(
                        location, system, tech_base, critical,
                    )?);
                    Ok(())
                })()
                .with_context(|| {
                    format!(
                        "{} slot {} ({})",
                        section.name(),
                        u16::from(slot) + 1,
                        critical.equipment
                    )
                })?;
            }
        }
        Ok(loadout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 40-ton tank whose turret holds sixteen entries: Artemis takes no item slot, so the
    /// twelve item slots of a 40-ton ICE tank cover them.
    const CROWDED_TURRET: &str = r#"
name = "Crowded"
class = "vehicle"
movement = "track"
tons = 40
walk_mp = 4

[construction]
engine = "ice"

[sections.left_side]
armor = 10

[sections.right_side]
armor = 10

[sections.front_side]
armor = 10

[sections.aft_side]
armor = 10

[sections.turret]
armor = 10
slots = [
    { at = "1-8", item = "IS.MachineGun" },
    { at = 9, item = "IS.SRM-2" },
    { at = 10, item = "ArtemisIV", link = 9 },
    { at = 11, item = "IS.SRM-2" },
    { at = 12, item = "ArtemisIV", link = 11 },
    { at = 13, item = "IS.SRM-2" },
    { at = 14, item = "ArtemisIV", link = 13 },
    { at = 15, item = "Ammo_IS.MachineGun", rounds = 200 },
    { at = 16, item = "Ammo_IS.SRM-2", rounds = 50, modes = ["Artemis/Mine"] },
]
"#;

    /// Slots past twelve resolve in place; the item-slot budget remains the limit.
    #[test]
    fn vehicle_sections_resolve_past_twelve_slots() {
        let template = VehicleTemplate::parse("Crowded", CROWDED_TURRET).unwrap();
        let loadout = VehicleLoadout::resolve(&template).unwrap();
        assert_eq!(loadout.weapons.len(), 11);
        let artemis: Vec<_> = loadout
            .systems
            .iter()
            .map(|part| part.location.slot)
            .collect();
        assert_eq!(artemis, [9, 11, 13]);
        assert_eq!(loadout.ammunition[1].location.slot, 15);
        let report = template.construction_rules().unwrap();
        assert_eq!((report.item_slots, report.item_slot_limit), (13, 13));

        let crowded = VehicleTemplate::parse(
            "Crowded",
            &CROWDED_TURRET.replace(
                "    { at = 15,",
                "    { at = 17, item = \"IS.MachineGun\" },\n    { at = 15,",
            ),
        )
        .unwrap();
        assert!(VehicleLoadout::resolve(&crowded).is_ok());
        let rejection = format!("{:#}", crowded.construction_rules().unwrap_err());
        assert!(
            rejection.contains("14 items exceed the 13 item slots"),
            "{rejection}"
        );

        let mut overflow = template;
        overflow
            .sections
            .get_mut(&VehicleSection::Turret)
            .unwrap()
            .criticals
            .insert(
                u8::MAX,
                CriticalDefinition {
                    equipment: "IS.MachineGun".into(),
                    data: "-".into(),
                    modes: Vec::new(),
                },
            );
        assert!(VehicleLoadout::resolve(&overflow).is_err());
    }
}
