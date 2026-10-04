//! Shared ammunition draw planning and expenditure after damage to a firing unit.
use super::{Mech, Vehicle};
use anyhow::{Context, Result};
use serde::Serialize;

/// A bounded draw from one live bin, in feed-priority order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AmmunitionDraw {
    pub bin_index: usize,
    pub rounds: u16,
}

/// Spend an admitted feed plan after damage, capping each draw at the bin's surviving rounds.
/// The plan must refer to this inventory; damage can empty bins but cannot change their indices.
/// Preserve feed order and omit empty draws from the attack's expenditure report.
pub(super) fn spend_surviving_draws(
    inventory: &mut [u16],
    draws: impl IntoIterator<Item = AmmunitionDraw>,
) -> Vec<AmmunitionDraw> {
    let mut spent = Vec::new();
    for draw in draws {
        let available = &mut inventory[draw.bin_index];
        let rounds = draw.rounds.min(*available);
        if rounds == 0 {
            continue;
        }
        *available -= rounds;
        spent.push(AmmunitionDraw { rounds, ..draw });
    }
    spent
}

impl Mech {
    /// Plan up to `rounds` compatible rounds without changing inventory, mode, heat or dice.
    /// Prefer the selected section, then the mount and canonical section/slot order; empty or unavailable bins are skipped.
    /// A short plan exposes shortage so a firing mode can choose its specified fallback atomically.
    pub fn ammunition_feed(&self, index: usize, rounds: u16) -> Result<Vec<AmmunitionDraw>> {
        let loadout = self.loadout()?;
        self.ammunition_feed_with_loadout(&loadout, index, rounds)
    }

    /// Resolve the live feed using an equipment projection from this immutable unit.
    pub(crate) fn ammunition_feed_with_loadout(
        &self,
        loadout: &super::MechLoadout,
        index: usize,
        rounds: u16,
    ) -> Result<Vec<AmmunitionDraw>> {
        let mount = loadout
            .weapons
            .get(index)
            .context("Weapon index out of bounds")?;
        let mode = self
            .ammunition_modes
            .get(&index)
            .copied()
            .unwrap_or_default();
        let bins = loadout
            .ammunition
            .iter()
            .enumerate()
            .filter(|(i, bin)| {
                bin.weapon == mount.weapon
                    && bin.mode == mode
                    && self.ammunition[*i] > 0
                    && !self.critical_unavailable(bin.location)
            })
            .map(|(bin_index, bin)| {
                (
                    bin_index,
                    (
                        super::ammunition_preference::priority(
                            self.ammunition_section(index),
                            mount.criticals[0].section,
                            bin.location.section,
                        ),
                        bin.location,
                    ),
                    self.ammunition[bin_index],
                )
            });
        Ok(plan_draws(bins, rounds))
    }
}

impl Vehicle {
    /// Plan up to `rounds` compatible rounds without changing inventory, mode, heat or dice.
    /// Prefer the selected section, then the mount and canonical section/slot order; empty or unavailable bins are skipped.
    /// A short plan exposes shortage so a firing mode can choose its specified fallback atomically.
    pub fn ammunition_feed(&self, index: usize, rounds: u16) -> Result<Vec<AmmunitionDraw>> {
        let loadout = self.loadout()?;
        self.ammunition_feed_with_loadout(&loadout, index, rounds)
    }

    /// Resolve the live feed using an equipment projection from this immutable unit.
    pub(crate) fn ammunition_feed_with_loadout(
        &self,
        loadout: &super::VehicleLoadout,
        index: usize,
        rounds: u16,
    ) -> Result<Vec<AmmunitionDraw>> {
        let mount = loadout
            .weapons
            .get(index)
            .context("Weapon index out of bounds")?;
        let mode = self
            .ammunition_modes
            .get(&index)
            .copied()
            .unwrap_or_default();
        let bins = loadout
            .ammunition
            .iter()
            .enumerate()
            .filter(|(i, bin)| {
                bin.weapon == mount.weapon
                    && bin.mode == mode
                    && self.ammunition()[*i] > 0
                    && !self.critical_unavailable(bin.location)
            })
            .map(|(bin_index, bin)| {
                (
                    bin_index,
                    (
                        super::ammunition_preference::priority(
                            self.ammunition_section(index),
                            mount.criticals[0].section,
                            bin.location.section,
                        ),
                        bin.location,
                    ),
                    self.ammunition()[bin_index],
                )
            });
        Ok(plan_draws(bins, rounds))
    }
}

/// Order eligible bins by mount locality and canonical location, then cap draws by requested supply.
fn plan_draws<L: Ord>(
    bins: impl Iterator<Item = (usize, (u8, L), u16)>,
    rounds: u16,
) -> Vec<AmmunitionDraw> {
    let mut bins: Vec<_> = bins.collect();
    bins.sort_by(|left, right| left.1.cmp(&right.1));
    let mut remaining = rounds;
    let mut draws = Vec::new();
    for (bin_index, _, available) in bins {
        if remaining == 0 {
            break;
        }
        let rounds = available.min(remaining);
        draws.push(AmmunitionDraw { bin_index, rounds });
        remaining -= rounds;
    }
    draws
}
