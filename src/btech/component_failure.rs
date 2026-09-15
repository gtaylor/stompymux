//! Inspectable nonweapon failure codes retain slot identity without changing physical system damage.
use super::{BattleDamageSlot, BattleEquipmentFailure, SectionDefinition};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// A nonweapon component's diagnostic condition, independent of its material availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleComponentFailure<L> {
    pub location: L,
    pub failure: BattleEquipmentFailure,
}

/// Validate unique, real nonweapon and nonammunition slots against their owned construction.
pub(super) fn validate<L: Copy + Ord>(
    failures: &[BattleComponentFailure<L>],
    installed: impl Fn(L) -> bool,
) -> Result<()> {
    let mut seen = BTreeSet::new();
    ensure!(
        failures
            .iter()
            .all(|failure| seen.insert(failure.location) && installed(failure.location)),
        "Invalid component failure slot"
    );
    Ok(())
}

/// Build diagnostic assignments for real system slots; weapons have their separate operational owner.
pub(super) fn replacement<S: Copy + Ord, L: Copy>(
    definitions: &std::collections::BTreeMap<S, SectionDefinition>,
    failures: &std::collections::BTreeMap<BattleDamageSlot, u8>,
    number: impl Fn(S) -> u8,
    location: impl Fn(S, u8) -> L,
    is_component: impl Fn(L) -> bool,
) -> Result<Vec<BattleComponentFailure<L>>> {
    let mut result = Vec::new();
    for (&section, definition) in definitions {
        for &critical in definition.criticals.keys() {
            let place = location(section, critical);
            if is_component(place)
                && let Some(&code) = failures.get(&BattleDamageSlot {
                    section: number(section),
                    slot: critical,
                })
                && let Some(failure) = BattleEquipmentFailure::from_code(code)?
            {
                result.push(BattleComponentFailure {
                    location: place,
                    failure,
                });
            }
        }
    }
    Ok(result)
}

/// Find the condition of one installed component without interpreting it as material loss.
pub(super) fn at<L: Copy + PartialEq>(
    failures: &[BattleComponentFailure<L>],
    location: L,
) -> Option<BattleEquipmentFailure> {
    failures
        .iter()
        .find(|failure| failure.location == location)
        .map(|failure| failure.failure)
}

impl super::BattleUnit {
    /// Nonweapon diagnostic failures, independent of system operation and material loss.
    pub fn component_failures(&self) -> &[BattleComponentFailure<super::CriticalLocation>] {
        &self.component_failures
    }
}

impl super::BattleVehicle {
    /// Nonweapon diagnostic failures, independent of system operation and material loss.
    pub fn component_failures(&self) -> &[BattleComponentFailure<super::VehicleCriticalLocation>] {
        &self.component_failures
    }
}
