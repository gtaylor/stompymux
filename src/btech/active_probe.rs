//! Active probes share equipment checks and ECM rejection while retaining family-specific reach and concealment rules.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Installed active-probe family; each family can serve either sensor slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleActiveProbe {
    Beagle,
    Light,
    Bloodhound,
}

/// Common electronic and equipment gates, independent of the observing chassis.
struct ProbeConditions {
    disturbed: bool,
    angel_protected: bool,
    concealed: bool,
    disabled: bool,
}

impl BattleActiveProbe {
    /// Hardware reach before the stationary-installation bonus.
    pub fn range(self) -> u8 {
        match self {
            Self::Beagle => 6,
            Self::Light => 3,
            Self::Bloodhound => 8,
        }
    }

    /// Probe critical identity and minimum installation size.
    pub(super) fn equipment(self) -> (BattleSystem, usize) {
        match self {
            Self::Beagle => (BattleSystem::BeagleProbe, 1),
            Self::Light => (BattleSystem::LightProbe, 1),
            Self::Bloodhound => (BattleSystem::BloodhoundProbe, 3),
        }
    }

    /// Evaluate without terrain penalties; acquisition 101 bypasses hidden-target penalties, not arc weighting.
    pub fn evaluate(
        self,
        distance: f64,
        disturbed: bool,
        angel_protected: bool,
        concealed: bool,
        disabled: bool,
        aim_adjustment: u8,
    ) -> Result<BattleSensorReport> {
        self.evaluate_with_maximum(
            distance,
            u16::from(self.range()),
            ProbeConditions {
                disturbed,
                angel_protected,
                concealed,
                disabled,
            },
            aim_adjustment,
        )
    }

    /// Apply the shared range and interference rules after chassis reach is selected.
    fn evaluate_with_maximum(
        self,
        distance: f64,
        maximum: u16,
        conditions: ProbeConditions,
        aim_adjustment: u8,
    ) -> Result<BattleSensorReport> {
        let ProbeConditions {
            disturbed,
            angel_protected,
            concealed,
            disabled,
        } = conditions;
        ensure!(
            distance.is_finite() && distance >= 0.0,
            "Invalid active-probe distance"
        );
        ensure!(aim_adjustment <= 2, "Invalid active-probe aim adjustment");
        let eligible = distance <= f64::from(maximum)
            && !disabled
            && !disturbed
            && !angel_protected
            && (!concealed || self == Self::Bloodhound);
        Ok(BattleSensorReport {
            eligible,
            acquisition_factor: if eligible { 101 } else { 0 },
            aim_modifier: i16::from(aim_adjustment),
        })
    }
}

impl BattleSensorMode {
    /// Resolve active probes without duplicating their rules in the contact and aiming pipelines.
    pub fn active_probe(self) -> Option<BattleActiveProbe> {
        match self {
            Self::BeagleProbe => Some(BattleActiveProbe::Beagle),
            Self::LightProbe => Some(BattleActiveProbe::Light),
            Self::BloodhoundProbe => Some(BattleActiveProbe::Bloodhound),
            _ => None,
        }
    }
}

impl BattleUnit {
    /// Presence follows installed parts; Bloodhound requires three slots on a biped.
    pub fn has_active_probe(&self, probe: BattleActiveProbe) -> Result<bool> {
        let (system, minimum) = probe.equipment();
        Ok(self
            .loadout()?
            .systems
            .iter()
            .filter(|part| part.system == system)
            .count()
            >= minimum)
    }

    /// Damage or flooding to any installed component disables that probe family.
    pub fn active_probe_available(&self, probe: BattleActiveProbe) -> Result<bool> {
        if !self.has_active_probe(probe)? {
            return Ok(false);
        }
        let (system, _) = probe.equipment();
        if probe == BattleActiveProbe::Light
            && let Some(failed) = self.critical_conditions.light_probe_failure
        {
            return Ok(!failed
                && self
                    .loadout()?
                    .systems
                    .iter()
                    .filter(|part| part.system == system)
                    .all(|part| {
                        self.sections()[&part.location.section].internal > 0
                            && !self.section_disabled(part.location.section)
                    }));
        }
        Ok(self
            .loadout()?
            .systems
            .iter()
            .filter(|part| part.system == system)
            .all(|part| !self.critical_unavailable(part.location)))
    }

    /// Losing a probe returns its active sensor slots to visual and invalidates the target lock.
    pub(super) fn reconcile_active_probes(&mut self) {
        let fallback = |sensor: BattleSensorMode| {
            if sensor
                .active_probe()
                .is_some_and(|probe| !self.active_probe_available(probe).unwrap_or(false))
            {
                return BattleSensorMode::Visual;
            }
            sensor
        };
        let pair = self.sensor_selection.active;
        let updated = BattleSensorPair {
            primary: fallback(pair.primary),
            secondary: fallback(pair.secondary),
        };
        if updated != pair {
            self.sensor_selection.active = updated;
            self.target_lock = None;
        }
    }
}

/// Inspect live probe equipment, concealment and electronic fields without consuming acquisition or aiming dice.
pub fn active_probe_contact(
    world: &World,
    observer: ObjectId,
    target: ObjectId,
    probe: BattleActiveProbe,
) -> Result<BattleSensorReport> {
    let report = evaluate_contact(world, observer, target, probe)?;
    Ok(super::visibility::sensor_report(world, target, report))
}

/// Evaluate this sensor's hardware and signature without applying operator visibility.
fn evaluate_contact(
    world: &World,
    observer: ObjectId,
    target: ObjectId,
    probe: BattleActiveProbe,
) -> Result<BattleSensorReport> {
    for id in [observer, target] {
        ensure!(
            world
                .objects
                .get(&id)
                .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
            "Unit is unavailable"
        );
    }
    let unit =
        super::scanner::scanner_unit(world, observer).context("Observer is not constructed")?;
    let target_unit =
        super::scanner::scanner_unit(world, target).context("Target is not constructed")?;
    let available = if let Some(vehicle) = world.btech.vehicles().get(&observer) {
        vehicle.active_probe_available(probe)?
    } else {
        world.btech.constructed_units()[&observer].active_probe_available(probe)?
    };
    let distance = unit_range(world, observer, target)?.spatial;
    let map = &world.btech.maps()[&unit.position.context("Observer is not placed")?.map];
    // Both Beagle variants use the Beagle map-disable switch.
    let flag = if probe == BattleActiveProbe::Bloodhound {
        256
    } else {
        64
    };
    probe.evaluate_with_maximum(
        distance,
        super::sensors::sensor_maximum(world, observer, u16::from(probe.range())),
        ProbeConditions {
            disturbed: electronic_field(world, observer)?.blocks_outgoing_guidance(),
            angel_protected: electronic_field(world, target)?.angel_protected,
            concealed: target_unit.concealed,
            disabled: !available
                || map.sensor_flags & flag != 0
                || distance > f64::from(u16::try_from(map.maximum_visibility)?),
        },
        0,
    )
}

impl BattleVehicle {
    /// Vehicles install each probe as one equipment slot, including Bloodhound probes.
    pub fn has_active_probe(&self, probe: BattleActiveProbe) -> Result<bool> {
        Ok(self
            .loadout()?
            .systems
            .iter()
            .any(|part| part.system == probe.equipment().0))
    }

    /// At least one surviving installed probe of the requested family is usable.
    pub fn active_probe_available(&self, probe: BattleActiveProbe) -> Result<bool> {
        if probe == BattleActiveProbe::Light
            && let Some(failed) = self.critical_conditions.light_probe_failure
        {
            return Ok(!failed
                && self.loadout()?.systems.iter().any(|part| {
                    part.system == probe.equipment().0
                        && self.sections()[&part.location.section].internal > 0
                        && !self.breached_sections().contains(&part.location.section)
                }));
        }
        Ok(self.loadout()?.systems.iter().any(|part| {
            part.system == probe.equipment().0 && !self.critical_unavailable(part.location)
        }))
    }

    /// Losing the last working probe returns only the affected active sensor slots to visual.
    pub(super) fn reconcile_active_probes(&mut self) {
        let fallback = |sensor: BattleSensorMode| {
            if sensor
                .active_probe()
                .is_some_and(|probe| !self.active_probe_available(probe).unwrap_or(false))
            {
                BattleSensorMode::Visual
            } else {
                sensor
            }
        };
        let active = BattleSensorPair {
            primary: fallback(self.sensor_selection.active.primary),
            secondary: fallback(self.sensor_selection.active.secondary),
        };
        if active != self.sensor_selection.active {
            self.target_lock = None;
        }
        self.sensor_selection.active = active;
    }
}
