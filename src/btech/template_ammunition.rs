//! Construction-only ammunition sizing and filling; live state loading never uses this normalization.
use super::{AmmunitionBin, BattleTemplate, BattleWeapon};
use anyhow::{Context, Result};

/// Infer small bins and normalize initial salvo counts using the biped template-loader rules.
pub(super) fn normalize(template: &mut BattleTemplate) -> Result<()> {
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
            let weapon = BattleWeapon::parse(name).with_context(context)?;
            let (mut capacity, half_ton, mode) =
                AmmunitionBin::configuration(weapon, &part.modes).with_context(context)?;
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
