//! Active probe families and their installed-equipment checks.
use crate::btech::{BattleLoadout, BattleSystem, BattleUnit, BattleVehicle, BattleVehicleLoadout};
use anyhow::Result;
use serde::Serialize;

/// Installed active-probe family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleActiveProbe {
    Beagle,
    Light,
    Bloodhound,
}

impl BattleActiveProbe {
    /// Every family, longest reach first.
    pub const BY_REACH: [Self; 3] = [Self::Bloodhound, Self::Beagle, Self::Light];

    /// Hardware reach before the stationary-installation bonus.
    pub fn range(self) -> u8 {
        match self {
            Self::Beagle => 6,
            Self::Light => 3,
            Self::Bloodhound => 8,
        }
    }

    /// Only the Bloodhound sees through stealth armor and null signature systems.
    pub fn sees_concealed(self) -> bool {
        self == Self::Bloodhound
    }

    /// Player-facing equipment name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Beagle => "Beagle Active Probe",
            Self::Light => "Light Active Probe",
            Self::Bloodhound => "Bloodhound Active Probe",
        }
    }

    /// Probe critical identity and minimum installation size.
    pub(crate) fn equipment(self) -> (BattleSystem, usize) {
        match self {
            Self::Beagle => (BattleSystem::BeagleProbe, 1),
            Self::Light => (BattleSystem::LightProbe, 1),
            Self::Bloodhound => (BattleSystem::BloodhoundProbe, 3),
        }
    }
}

/// Whether and how well one probe family is fitted, read from an already built loadout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProbeFitting {
    pub installed: bool,
    pub available: bool,
}

impl BattleUnit {
    /// Presence follows installed parts; Bloodhound requires three slots on a biped.
    pub fn has_active_probe(&self, probe: BattleActiveProbe) -> Result<bool> {
        Ok(self.probe_fitting(&self.loadout()?, probe).installed)
    }

    /// Damage or flooding to any installed component disables that probe family.
    pub fn active_probe_available(&self, probe: BattleActiveProbe) -> Result<bool> {
        Ok(self.probe_fitting(&self.loadout()?, probe).available)
    }

    /// Check one family against a loadout the caller already built, so several families
    /// share a single equipment projection.
    pub(crate) fn probe_fitting(
        &self,
        loadout: &BattleLoadout,
        probe: BattleActiveProbe,
    ) -> ProbeFitting {
        let (system, minimum) = probe.equipment();
        let parts = || loadout.systems.iter().filter(|part| part.system == system);
        if parts().count() < minimum {
            return ProbeFitting {
                installed: false,
                available: false,
            };
        }
        let available = if probe == BattleActiveProbe::Light
            && let Some(failed) = self.critical_conditions.light_probe_failure
        {
            !failed
                && parts().all(|part| {
                    self.sections()[&part.location.section].internal > 0
                        && !self.section_disabled(part.location.section)
                })
        } else {
            parts().all(|part| !self.critical_unavailable(part.location))
        };
        ProbeFitting {
            installed: true,
            available,
        }
    }
}

impl BattleVehicle {
    /// Vehicles install each probe as one equipment slot, including Bloodhound probes.
    pub fn has_active_probe(&self, probe: BattleActiveProbe) -> Result<bool> {
        Ok(self.probe_fitting(&self.loadout()?, probe).installed)
    }

    /// At least one surviving installed probe of the requested family is usable.
    pub fn active_probe_available(&self, probe: BattleActiveProbe) -> Result<bool> {
        Ok(self.probe_fitting(&self.loadout()?, probe).available)
    }

    /// Check one family against a loadout the caller already built, so several families
    /// share a single equipment projection.
    pub(crate) fn probe_fitting(
        &self,
        loadout: &BattleVehicleLoadout,
        probe: BattleActiveProbe,
    ) -> ProbeFitting {
        let system = probe.equipment().0;
        let mut parts = loadout.systems.iter().filter(|part| part.system == system);
        let installed = loadout.systems.iter().any(|part| part.system == system);
        let available = if probe == BattleActiveProbe::Light
            && let Some(failed) = self.critical_conditions.light_probe_failure
        {
            !failed
                && parts.any(|part| {
                    self.sections()[&part.location.section].internal > 0
                        && !self.breached_sections().contains(&part.location.section)
                })
        } else {
            parts.any(|part| !self.critical_unavailable(part.location))
        };
        ProbeFitting {
            installed,
            available,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reach ordering drives profile selection; only the Bloodhound sees concealed units.
    #[test]
    fn families_order_by_reach_and_concealment() {
        let ranges = BattleActiveProbe::BY_REACH.map(BattleActiveProbe::range);
        assert_eq!(ranges, [8, 6, 3]);
        assert_eq!(
            BattleActiveProbe::BY_REACH.map(BattleActiveProbe::sees_concealed),
            [true, false, false]
        );
        assert_eq!(
            serde_json::to_value(BattleActiveProbe::Bloodhound).unwrap(),
            "bloodhound"
        );
    }
}
