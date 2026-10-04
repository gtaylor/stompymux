//! Construction-only ammunition sizing and filling; live state loading never uses this normalization.
use super::{AmmunitionBin, BattleTemplate, BattleWeapon};
use anyhow::{Context, Result};

impl BattleTemplate {
    /// Infer small bins and normalize initial salvo counts using the biped template-loader rules.
    pub fn normalize_ammunition(&mut self) -> Result<()> {
        normalize_with(self, false)
    }

    /// Apply the native administrator's broader ammunition flag admission while
    /// preserving the strict constructor's compatibility checks.
    pub fn normalize_contract_ammunition(&mut self) -> Result<()> {
        normalize_with(self, true)
    }
}

/// Size and fill every ammunition critical, admitting administrator flags when `contract`.
fn normalize_with(template: &mut BattleTemplate, contract: bool) -> Result<()> {
    for (location, section) in &mut template.sections {
        for (slot, part) in &mut section.criticals {
            let Some(name) = super::equipment::strip_name_prefix(&part.equipment, "Ammo_") else {
                continue;
            };
            let context = || {
                format!(
                    "{} critical {} ({})",
                    location.name(),
                    slot + 1,
                    part.equipment
                )
            };
            let weapon = match BattleWeapon::parse(name) {
                Ok(weapon) => weapon,
                Err(_)
                    if contract
                        && super::BattlePart::parse(&part.equipment)
                            .is_ok_and(|part| part.kind == super::BattlePartKind::Ammunition) =>
                {
                    continue;
                }
                Err(error) => return Err(error).with_context(context),
            };
            let (mut capacity, half_ton, mode) = if contract {
                AmmunitionBin::configuration_contract(weapon, &part.modes)
            } else {
                AmmunitionBin::configuration(weapon, &part.modes)
            }
            .with_context(context)?;
            let rounds: u16 = part
                .data
                .parse()
                .context("Invalid ammunition count")
                .with_context(context)?;
            if rounds < capacity {
                let half_capacity =
                    u16::from(weapon.profile_for_ammunition(mode).ammunition_per_ton / 2);
                if rounds <= half_capacity {
                    capacity = half_capacity;
                    if !half_ton {
                        part.modes.push("Halfton".into());
                    }
                }
            }
            part.data = capacity.to_string();
        }
    }
    Ok(())
}
