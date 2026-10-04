//! Inspectable nonweapon failure codes retain slot identity without changing physical system damage.
use super::{DamageSlot, EquipmentFailure, SectionDefinition};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// A nonweapon component's diagnostic condition, independent of its material availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentFailure<L> {
    pub location: L,
    pub failure: EquipmentFailure,
}

/// Validate unique, real nonweapon and nonammunition slots against their owned construction.
pub(super) fn validate<L: Copy + Ord>(
    failures: &[ComponentFailure<L>],
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
    failures: &std::collections::BTreeMap<DamageSlot, u8>,
    number: impl Fn(S) -> u8,
    location: impl Fn(S, u8) -> L,
    is_component: impl Fn(L) -> bool,
) -> Result<Vec<ComponentFailure<L>>> {
    let mut result = Vec::new();
    for (&section, definition) in definitions {
        for &critical in definition.criticals.keys() {
            let place = location(section, critical);
            if is_component(place)
                && let Some(&code) = failures.get(&DamageSlot {
                    section: number(section),
                    slot: critical,
                })
                && let Some(failure) = EquipmentFailure::from_code(code)?
            {
                result.push(ComponentFailure {
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
    failures: &[ComponentFailure<L>],
    location: L,
) -> Option<EquipmentFailure> {
    failures
        .iter()
        .find(|failure| failure.location == location)
        .map(|failure| failure.failure)
}

impl super::Mech {
    /// Nonweapon diagnostic failures, independent of system operation and material loss.
    pub fn component_failures(&self) -> &[ComponentFailure<super::CriticalLocation>] {
        &self.component_failures
    }
}

impl super::Vehicle {
    /// Nonweapon diagnostic failures, independent of system operation and material loss.
    pub fn component_failures(&self) -> &[ComponentFailure<super::VehicleCriticalLocation>] {
        &self.component_failures
    }
}
