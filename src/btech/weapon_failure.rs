//! Temporary weapon conditions and recycle policy shared by Mechs and vehicles.
use super::{BattleEquipmentCondition, BattleWeapon};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A named failure code, separate from material loss; weapon mounts also enforce it during firing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleEquipmentFailure {
    Jammed,
    Shorted,
    Dud,
    Empty,
    Disabled,
    AmmunitionJam,
    CriticalAmmunitionJam,
}

impl BattleEquipmentFailure {
    /// Stable compact damage-field code.
    pub fn code(self) -> u8 {
        match self {
            Self::Jammed => 1,
            Self::Shorted => 2,
            Self::Dud => 3,
            Self::Empty => 4,
            Self::Disabled => 5,
            Self::AmmunitionJam => 6,
            Self::CriticalAmmunitionJam => 7,
        }
    }

    /// Zero clears a condition; other values must identify a known failure.
    pub fn from_code(code: u8) -> Result<Option<Self>> {
        Ok(match code {
            0 => None,
            1 => Some(Self::Jammed),
            2 => Some(Self::Shorted),
            3 => Some(Self::Dud),
            4 => Some(Self::Empty),
            5 => Some(Self::Disabled),
            6 => Some(Self::AmmunitionJam),
            7 => Some(Self::CriticalAmmunitionJam),
            _ => anyhow::bail!("Unknown temporary weapon failure"),
        })
    }

    /// Ballistic criticals jam mechanisms; other weapon families short out.
    pub(super) fn for_weapon(weapon: BattleWeapon) -> Self {
        if weapon.gunnery_skill(true) == "Gunnery-Ballistic" {
            Self::Jammed
        } else {
            Self::Shorted
        }
    }

    /// Diagnostic condition shared by whole-weapon reports.
    pub(super) fn condition(self) -> BattleEquipmentCondition {
        match self {
            Self::Jammed => BattleEquipmentCondition::Jammed,
            Self::Shorted => BattleEquipmentCondition::Shorted,
            Self::Dud => BattleEquipmentCondition::Broken,
            Self::Empty => BattleEquipmentCondition::Empty,
            Self::Disabled => BattleEquipmentCondition::Destroyed,
            Self::AmmunitionJam | Self::CriticalAmmunitionJam => BattleEquipmentCondition::AmmoJam,
        }
    }
}

/// Advance an existing recycle event; disabled weapons finish on its next powered tick.
pub(super) fn remaining(failure: Option<BattleEquipmentFailure>, remaining: u16) -> u16 {
    if failure == Some(BattleEquipmentFailure::Disabled) {
        0
    } else {
        remaining.saturating_sub(1)
    }
}

/// Recovery feedback uses the same spelling for every unit type.
pub(super) fn recovery_notice(weapon: BattleWeapon) -> String {
    format!(
        "[fg=green]{} is operational again.[reset]",
        weapon.name().split_once('.').unwrap().1
    )
}

/// Stored conditions reference available mounts; they need not have a recovery event.
pub(super) fn validate(
    failures: &BTreeMap<usize, BattleEquipmentFailure>,
    weapons: usize,
    available: impl Fn(usize) -> bool,
) -> Result<()> {
    ensure!(
        failures
            .keys()
            .all(|index| *index < weapons && available(*index)),
        "Invalid weapon failure mount"
    );
    Ok(())
}

/// Set a temporary condition without changing material, dice or existing recovery timers.
/// This trusted domain operation leaves authorization and notice publication to its caller.
pub fn set_weapon_failure(
    world: &mut World,
    id: ObjectId,
    index: usize,
    failure: Option<BattleEquipmentFailure>,
) -> Result<()> {
    let failures = super::with_unit_mut!(
        world.btech.unit_mut(id).context("Unit is unavailable")?,
        |unit| {
            ensure!(
                index < unit.loadout()?.weapons.len(),
                "Weapon index out of bounds"
            );
            ensure!(
                failure.is_none() || unit.weapon_intact(index)?,
                "That weapon has been destroyed"
            );
            &mut unit.weapon_failures
        }
    );
    if let Some(failure) = failure {
        failures.insert(index, failure);
    } else {
        failures.remove(&index);
    }
    Ok(())
}

impl super::BattleUnit {
    /// Temporary conditions by mount index; existing recycle clocks govern recovery.
    pub fn weapon_failures(&self) -> &BTreeMap<usize, BattleEquipmentFailure> {
        &self.weapon_failures
    }
}

/// Only ordinary ammunition jams can be shaken loose by the crew.
pub(super) fn feed_jammed(
    failures: &BTreeMap<usize, BattleEquipmentFailure>,
    index: usize,
) -> bool {
    failures.get(&index) == Some(&BattleEquipmentFailure::AmmunitionJam)
}

/// Clear a recoverable feed failure without clearing unrelated operational conditions.
pub(super) fn clear_feed(
    failures: &mut BTreeMap<usize, BattleEquipmentFailure>,
    index: usize,
) -> bool {
    if !feed_jammed(failures, index) {
        return false;
    }
    failures.remove(&index);
    true
}

/// A powered-down or temporarily disabled mount cannot initiate a weapon explosion.
pub(super) fn explosion_disabled(
    index: usize,
    powered_down: &std::collections::BTreeSet<usize>,
    failures: &BTreeMap<usize, BattleEquipmentFailure>,
) -> bool {
    powered_down.contains(&index) || failures.get(&index) == Some(&BattleEquipmentFailure::Disabled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_codes_and_recovery_policy_cover_every_condition() {
        assert_eq!(BattleEquipmentFailure::from_code(0).unwrap(), None);
        for code in 1..=7 {
            let failure = BattleEquipmentFailure::from_code(code).unwrap().unwrap();
            assert_eq!(failure.code(), code);
            assert_eq!(remaining(Some(failure), 2), if code == 5 { 0 } else { 1 });
            assert_eq!(remaining(Some(failure), 0), 0);
            let mut failures = [(0, failure)].into_iter().collect();
            assert_eq!(clear_feed(&mut failures, 0), code == 6);
            assert_eq!(failures.is_empty(), code == 6);
        }
        assert!(BattleEquipmentFailure::from_code(8).is_err());
    }
}
