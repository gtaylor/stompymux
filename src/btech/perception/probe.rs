//! Active probe families, their installed-equipment checks, and the technology base each
//! fitted probe is built from.
use crate::btech::{Mech, MechLoadout, System, Vehicle, VehicleLoadout};
use anyhow::Result;
use serde::Serialize;

/// Installed active-probe family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActiveProbe {
    Beagle,
    Light,
    Bloodhound,
    /// A Watchdog CEWS: a Clan ECM suite that doubles as an active probe.
    Watchdog,
}

impl ActiveProbe {
    /// Every family, longest reach first.
    pub const BY_REACH: [Self; 4] = [Self::Bloodhound, Self::Beagle, Self::Watchdog, Self::Light];

    /// Hardware reach before the stationary-installation bonus. A Beagle slot built from Clan
    /// technology (`clan`: one stated `CL.BeagleProbe`, or an unprefixed one on a Clan
    /// chassis) is the Clan active probe, which reaches five hexes. The Watchdog CEWS carries a
    /// light active probe and reaches three, like the Light Active Probe (Tactical Operations: Advanced Units & Equipment, Watchdog CEWS rules and
    /// equipment table, range —/—/—/3).
    pub fn range(self, clan: bool) -> u8 {
        match self {
            Self::Beagle if clan => 5,
            Self::Beagle => 4,
            Self::Watchdog | Self::Light => 3,
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
            Self::Watchdog => "Watchdog CEWS",
        }
    }

    /// Player-facing name of an installed probe: a Beagle slot built from Clan technology is
    /// the Clan active probe.
    pub fn installed_name(self, clan: bool) -> &'static str {
        if self == Self::Beagle && clan {
            return "Clan Active Probe";
        }
        self.name()
    }

    /// Probe critical identity and minimum installation size.
    pub(crate) fn equipment(self) -> (System, usize) {
        match self {
            Self::Beagle => (System::BeagleProbe, 1),
            Self::Light => (System::LightProbe, 1),
            Self::Bloodhound => (System::BloodhoundProbe, 3),
            Self::Watchdog => (System::Ecm, 2),
        }
    }
}

/// Whether and how well one probe family is fitted, read from an already built loadout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProbeFitting {
    pub installed: bool,
    pub available: bool,
    /// The fitted probe is Clan technology by its own slots (see
    /// [`SystemCritical::is_clan`](crate::btech::SystemCritical::is_clan)):
    /// the Beagle and the Clan active probe share one slot name, so this tells them apart.
    pub clan: bool,
}

impl Mech {
    /// Presence follows installed parts; Bloodhound requires three slots on a biped.
    pub fn has_active_probe(&self, probe: ActiveProbe) -> Result<bool> {
        Ok(self.probe_fitting(&self.loadout()?, probe).installed)
    }

    /// Damage or flooding to any installed component disables that probe family.
    pub fn active_probe_available(&self, probe: ActiveProbe) -> Result<bool> {
        Ok(self.probe_fitting(&self.loadout()?, probe).available)
    }

    /// Check one family against a loadout the caller already built, so several families
    /// share a single equipment projection.
    pub(crate) fn probe_fitting(&self, loadout: &MechLoadout, probe: ActiveProbe) -> ProbeFitting {
        let (system, minimum) = probe.equipment();
        let parts = || loadout.systems.iter().filter(|part| part.system == system);
        if parts().count() < minimum
            || (probe == ActiveProbe::Watchdog
                && !self
                    .definition()
                    .has_technology(crate::btech::Technology::Watchdog))
        {
            return ProbeFitting {
                installed: false,
                available: false,
                clan: false,
            };
        }
        let available = if probe == ActiveProbe::Light
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
        let clan_chassis = self.definition().has_special("Clan");
        ProbeFitting {
            installed: true,
            available,
            clan: parts().any(|part| part.is_clan(clan_chassis)),
        }
    }
}

impl Vehicle {
    /// Vehicles install each probe as one equipment slot, including Bloodhound probes.
    pub fn has_active_probe(&self, probe: ActiveProbe) -> Result<bool> {
        Ok(self.probe_fitting(&self.loadout()?, probe).installed)
    }

    /// At least one surviving installed probe of the requested family is usable.
    pub fn active_probe_available(&self, probe: ActiveProbe) -> Result<bool> {
        Ok(self.probe_fitting(&self.loadout()?, probe).available)
    }

    /// Check one family against a loadout the caller already built, so several families
    /// share a single equipment projection.
    pub(crate) fn probe_fitting(
        &self,
        loadout: &VehicleLoadout,
        probe: ActiveProbe,
    ) -> ProbeFitting {
        let system = probe.equipment().0;
        let parts = || loadout.systems.iter().filter(|part| part.system == system);
        let installed = loadout.systems.iter().any(|part| part.system == system)
            && (probe != ActiveProbe::Watchdog
                || self
                    .definition()
                    .has_technology(crate::btech::Technology::Watchdog));
        let available = if probe == ActiveProbe::Light
            && let Some(failed) = self.critical_conditions.light_probe_failure
        {
            !failed
                && parts().any(|part| {
                    self.sections()[&part.location.section].internal > 0
                        && !self.breached_sections().contains(&part.location.section)
                })
        } else {
            parts().any(|part| !self.critical_unavailable(part.location))
        };
        let clan_chassis = self.definition().has_special("Clan");
        ProbeFitting {
            installed,
            available,
            clan: installed && parts().any(|part| part.is_clan(clan_chassis)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reach ordering drives profile selection; only the Bloodhound sees concealed units. The
    /// Watchdog's light probe reaches three hexes (TO:AUE equipment table, —/—/—/3) and the
    /// Clan active probe five.
    #[test]
    fn families_order_by_reach_and_concealment() {
        let ranges = |clan| ActiveProbe::BY_REACH.map(|probe| probe.range(clan));
        assert_eq!(ranges(false), [8, 4, 3, 3]);
        assert_eq!(ranges(true), [8, 5, 3, 3]);
        assert_eq!(ActiveProbe::Watchdog.range(true), 3);
        assert_eq!(
            ActiveProbe::BY_REACH.map(ActiveProbe::sees_concealed),
            [true, false, false, false]
        );
        assert_eq!(
            serde_json::to_value(ActiveProbe::Bloodhound).unwrap(),
            "bloodhound"
        );
    }

    /// Only a Clan-built Beagle is the Clan active probe.
    #[test]
    fn only_a_clan_built_beagle_is_named_the_clan_active_probe() {
        assert_eq!(
            ActiveProbe::Beagle.installed_name(true),
            "Clan Active Probe"
        );
        assert_eq!(
            ActiveProbe::Beagle.installed_name(false),
            "Beagle Active Probe"
        );
        assert_eq!(
            ActiveProbe::Light.installed_name(true),
            "Light Active Probe"
        );
    }
}
