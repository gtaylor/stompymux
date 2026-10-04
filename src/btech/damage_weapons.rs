//! Weapon replacement rules shared by Mechs, ground vehicles and VTOLs.
use super::{DamageReplacement, DamageSlot, EquipmentFailure, Unjam, WeaponMount};
use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet};

/// Borrow only the operational state affected by compact weapon replacement.
pub(super) struct Weapons<'a> {
    pub failures: &'a mut BTreeMap<usize, EquipmentFailure>,
    pub manual_jams: &'a mut BTreeSet<usize>,
    pub powered_down: &'a mut BTreeSet<usize>,
    pub spent: &'a mut BTreeSet<usize>,
    pub recycle: &'a mut BTreeMap<usize, u16>,
    pub unjam: &'a mut Option<Unjam>,
}

/// Assign failures, retire unavailable timers and replenish explicitly restored launchers.
pub(super) fn replace<L: Copy + Ord>(
    replacement: &DamageReplacement,
    mounts: &[WeaponMount<L>],
    slot: impl Fn(L) -> DamageSlot,
    available: &BTreeSet<usize>,
    restored: &BTreeSet<L>,
    state: Weapons<'_>,
) -> Result<()> {
    state.failures.clear();
    state.manual_jams.clear();
    state.powered_down.clear();
    for (index, mount) in mounts.iter().enumerate() {
        if available.contains(&index) {
            if let Some(&code) = replacement.failures.get(&slot(mount.criticals[0]))
                && let Some(failure) = EquipmentFailure::from_code(code)?
            {
                state.failures.insert(index, failure);
            }
        } else {
            state.recycle.remove(&index);
        }
        if mount
            .criticals
            .iter()
            .any(|location| restored.contains(location))
        {
            state.spent.remove(&index);
        }
    }
    if state.unjam.is_some_and(|attempt| {
        !super::weapon_failure::feed_jammed(state.failures, attempt.weapon_index)
    }) {
        *state.unjam = None;
    }
    Ok(())
}
