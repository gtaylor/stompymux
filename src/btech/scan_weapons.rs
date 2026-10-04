//! Adversarial weapon rows share one fixed-column renderer across supported unit anatomies.
use super::{EquipmentFailure, WeaponMount};
use crate::{ObjectId, World, text};
use anyhow::{Context, Result};

/// Inspect only public scan condition: first slot, section survival and the current recycle clock.
pub(super) fn render(world: &World, id: ObjectId) -> Result<String> {
    crate::btech::with_unit!(
        world.btech.unit(id).context("Unit is unavailable")?,
        |unit| {
            Ok(rows(&unit.loadout()?.weapons, |index, slot| {
                (unit.sections()[&slot.section].internal > 0).then(|| {
                    (
                        unit.section_name(slot.section),
                        unit.critical_unavailable(slot)
                            || unit.weapon_failures().get(&index)
                                == Some(&EquipmentFailure::Disabled),
                        unit.weapon_recycle().get(&index).copied().unwrap_or(0),
                    )
                })
            }))
        }
    )
}

/// Scan numbering counts only displayed installations, while state lookup keeps the real mount index.
fn rows<L: Copy>(
    mounts: &[WeaponMount<L>],
    state: impl Fn(usize, L) -> Option<(&'static str, bool, u16)>,
) -> String {
    let mut lines = vec![
        "================WEAPON SYSTEMS================".to_owned(),
        "----- Weapon ------ [##]  Location ---- Status".to_owned(),
    ];
    for (index, mount) in mounts.iter().enumerate() {
        let Some((location, broken, remaining)) = state(index, mount.criticals[0]) else {
            continue;
        };
        let name = mount.weapon.name();
        let name: String = name
            .split_once('.')
            .map_or(name, |(_, short)| short)
            .chars()
            .take(18)
            .collect();
        let location: String = location.replace('_', " ").chars().take(14).collect();
        let number = lines.len() - 2;
        let status = if broken {
            "[fg=black bold]*****[reset]"
        } else if remaining > 0 {
            "-----"
        } else {
            "[fg=green]Ready[reset]"
        };
        lines.push(format!(
            "{}{status}",
            text::escape(&format!(" {name:<18} [{number:>2}]  {location:<14}"))
        ));
    }
    lines.join("\r\n")
}
