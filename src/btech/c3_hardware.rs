//! Shared command-computer accounting with chassis-specific slot sizes and live damage inputs.
use super::{BattleSystem, BattleUnit, BattleVehicle};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Physical computer counts and current eligibility, independent of power and network membership.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleC3Hardware {
    /// Complete master computers: five slots per Mech computer, one per vehicle computer.
    pub masters: usize,
    /// Master computers whose component slots are all functional.
    pub working_masters: usize,
    /// At least one slave slot is installed.
    pub slave_installed: bool,
    /// At least one slave slot is functional.
    pub slave_operational: bool,
    /// The chassis has enough C3i slots: two for a Mech, one for a vehicle.
    pub c3i_installed: bool,
    /// Enough C3i slots remain functional for this chassis.
    pub c3i_operational: bool,
}

/// Installation facts needed to distinguish absent masters from incomplete or destroyed computers.
struct ComputerInventory {
    hardware: BattleC3Hardware,
    master_installed: bool,
}

impl ComputerInventory {
    /// Installed masters must retain a complete working computer before a slave can provide C3.
    fn operational(&self) -> bool {
        if self.master_installed && self.hardware.working_masters == 0 {
            return false;
        }
        self.hardware.working_masters > 0 || self.hardware.slave_operational
    }
}

/// Group computers within sections in slot order; intervening equipment does not split a group.
/// Adapters supply live slots, including whole-unit destruction, so damage is never cached.
fn inventory<S: Ord, const MASTER: usize, const C3I: usize>(
    parts: impl IntoIterator<Item = (S, u8, BattleSystem, bool)>,
) -> ComputerInventory {
    let mut hardware = BattleC3Hardware::default();
    let mut masters = BTreeMap::<_, Vec<_>>::new();
    let mut c3i_slots = 0;
    let mut working_c3i_slots = 0;
    for (section, slot, system, working) in parts {
        match system {
            BattleSystem::C3Master => masters.entry(section).or_default().push((slot, working)),
            BattleSystem::C3Slave => {
                hardware.slave_installed = true;
                hardware.slave_operational |= working;
            }
            BattleSystem::C3i => {
                c3i_slots += 1;
                working_c3i_slots += usize::from(working);
            }
            _ => {}
        }
    }
    for slots in masters.values_mut() {
        slots.sort_unstable_by_key(|(slot, _)| *slot);
        for computer in slots.as_chunks::<MASTER>().0 {
            hardware.masters += 1;
            hardware.working_masters += usize::from(computer.iter().all(|(_, live)| *live));
        }
    }
    hardware.c3i_installed = c3i_slots >= C3I;
    hardware.c3i_operational = working_c3i_slots >= C3I;
    ComputerInventory {
        hardware,
        master_installed: !masters.is_empty(),
    }
}

impl BattleUnit {
    /// Supply Mech slot availability, including flooding and whole-unit destruction.
    fn command_computers(&self) -> Result<ComputerInventory> {
        let systems = self.loadout()?.systems;
        // Whole-unit destruction is the same for every slot; scanner and network
        // queries ask this of every unit each tick, so evaluate it once.
        let destroyed = self.is_destroyed();
        Ok(inventory::<_, 5, 2>(systems.into_iter().map(|part| {
            (
                part.location.section,
                part.location.slot,
                part.system,
                !destroyed && !self.critical_unavailable(part.location),
            )
        })))
    }

    /// Derive computer capabilities without trusting template flags or caching damage.
    pub fn c3_hardware(&self) -> Result<BattleC3Hardware> {
        Ok(self.command_computers()?.hardware)
    }

    /// Classic C3 requires a live computer; losing every installed master disables its unit's C3.
    pub fn c3_operational(&self) -> Result<bool> {
        Ok(self.command_computers()?.operational())
    }
}

impl BattleVehicle {
    /// Supply independent vehicle slots, including destroyed sections and whole-unit destruction.
    fn command_computers(&self) -> Result<ComputerInventory> {
        let systems = self.loadout()?.systems;
        // Whole-unit destruction is the same for every slot; scanner and network
        // queries ask this of every unit each tick, so evaluate it once.
        let destroyed = self.is_destroyed();
        Ok(inventory::<_, 1, 1>(systems.into_iter().map(|part| {
            (
                part.location.section,
                part.location.slot,
                part.system,
                !destroyed && !self.critical_unavailable(part.location),
            )
        })))
    }

    /// Derive vehicle command-computer capabilities from installed equipment and live damage.
    pub fn c3_hardware(&self) -> Result<BattleC3Hardware> {
        Ok(self.command_computers()?.hardware)
    }

    /// Apply the same master-loss rule as Mechs, independently of power or network membership.
    pub fn c3_operational(&self) -> Result<bool> {
        Ok(self.command_computers()?.operational())
    }
}
