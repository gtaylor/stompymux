//! Read-only contact-position indexing within one full validation call.
//!
//! Compact identities use bounded direct addressing. Sparse or extreme ranges
//! keep the ordinary tree lookup; neither path persists beyond the immutable read.
use super::{BattlePosition, BtechState};
use crate::ObjectId;

const MAX_DENSE_SPAN: usize = 65_536;

pub(super) struct Positions {
    base: i64,
    /// Outer None means absent; Some(None) means a present but unplaced unit.
    entries: Vec<Option<Option<BattlePosition>>>,
}
impl Positions {
    /// Prefer vehicle records on malformed duplicate identities, as the native
    /// contact validator does. Range checks precede every allocation.
    pub(super) fn prepare(state: &BtechState) -> Option<Self> {
        #[cfg(test)]
        if super::shot_transaction::reference_enabled() {
            return None;
        }
        let first = [
            state.constructed.first_key_value().map(|(id, _)| id.0),
            state.vehicles.first_key_value().map(|(id, _)| id.0),
        ]
        .into_iter()
        .flatten()
        .min()?;
        let last = [
            state.constructed.last_key_value().map(|(id, _)| id.0),
            state.vehicles.last_key_value().map(|(id, _)| id.0),
        ]
        .into_iter()
        .flatten()
        .max()?;
        let span = usize::try_from(last.checked_sub(first)?.checked_add(1)?).ok()?;
        let count = state.constructed.len().saturating_add(state.vehicles.len());
        if span > MAX_DENSE_SPAN || span > count.saturating_mul(4) {
            return None;
        }
        let mut entries = vec![None; span];
        for (id, position) in state
            .constructed
            .iter()
            .map(|(id, unit)| (id, unit.position()))
            .chain(
                state
                    .vehicles
                    .iter()
                    .map(|(id, unit)| (id, unit.position())),
            )
        {
            entries[(id.0 - first) as usize] = Some(position);
        }
        Some(Self {
            base: first,
            entries,
        })
    }

    /// Preserve the distinction between missing records and unplaced units.
    pub(super) fn get(&self, id: ObjectId) -> Option<Option<BattlePosition>> {
        let index = usize::try_from(id.0.checked_sub(self.base)?).ok()?;
        self.entries.get(index).copied().flatten()
    }
}
