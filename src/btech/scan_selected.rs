//! Saved target selection dispatch for scans without replacing locks or advancing their countdowns.
use super::{BuildingScan, HexScan, HexTargetMode, TargetSelection};
use crate::{Config, ObjectId, Scripts};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// A scan of the selected target; structure results have already been delivered to the cockpit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "report", rename_all = "snake_case")]
pub enum SelectedScan {
    /// Unit report returned for the pilot, with any target warning already staged.
    Unit(String),
    /// Building-only scan with cockpit delivery.
    Building(BuildingScan),
    /// Combined building and mine scan with recipient-specific delivery.
    Hex(HexScan),
}

/// Scan the saved target regardless of its settling countdown, rechecking current scan admission.
/// Ignition and clearing selections inspect occupants, as does a unit-at-hex lock.
pub fn scan_selected_action(
    scripts: &Scripts,
    config: &Config,
    observer: ObjectId,
    pilot: ObjectId,
    selection: &str,
) -> Result<SelectedScan> {
    super::scan::options(selection)?;
    let (observer, target) = {
        let world = scripts.world.borrow();
        let source = super::combat_operator::admit_running(&world, observer, pilot)?.source;
        (
            source.unit,
            source.selection(&world).context("No default target set!")?,
        )
    };
    if let TargetSelection::Hex(lock) = target {
        ensure!(
            super::visibility::hex_unblocked(&scripts.world.borrow(), observer, lock.hex)?,
            "Target hex is not in line of sight!"
        );
    }
    match target {
        TargetSelection::Unit(lock) => {
            super::scan_unit_action(scripts, observer, pilot, lock.target, selection)
                .map(SelectedScan::Unit)
        }
        TargetSelection::Hex(lock) => match lock.mode {
            HexTargetMode::Building => super::scan_building::action_with_range(
                scripts, config, observer, pilot, lock.hex, true,
            )
            .map(SelectedScan::Building),
            HexTargetMode::Hex => super::scan_mines::action_with_range(
                scripts, config, observer, pilot, lock.hex, true,
            )
            .map(SelectedScan::Hex),
            _ => super::scan_hex_unit_action(scripts, observer, pilot, lock.hex, selection)
                .map(SelectedScan::Unit),
        },
    }
}
