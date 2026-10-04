//! Operator cleanup of loose stock records, independent of unit repair and installed equipment.
use super::{InventoryEntry, Part};
use crate::{Config, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::collections::BTreeMap;

/// Detached cleanup totals; quantities are wide enough to count multiple full stock rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InventoryCleanup {
    pub original_entries: usize,
    pub new_entries: usize,
    pub items: u64,
}

impl InventoryCleanup {
    /// Operator feedback retains the two inventory-consistency summary lines.
    pub fn text(&self) -> String {
        format!(
            "Fixing done. Original entries: {}. New entries: {}.\nItems in new: {}. Unique items in new: {}.",
            self.original_entries, self.new_entries, self.items, self.new_entries
        )
    }
}

/// Structural critical placeholders are not loose repair parts and are removed by FIXSTUFF.
fn structural_placeholder(id: i32) -> bool {
    matches!(id, 406 | 407 | 408 | 428 | 432 | 433 | 443 | 444 | 454)
}

/// Rebuild ordered positive stock, rejecting quantity overflow.
fn normalized(entries: &[InventoryEntry]) -> Result<Vec<InventoryEntry>> {
    let mut quantities = BTreeMap::<i32, i64>::new();
    for entry in entries {
        if structural_placeholder(entry.part_id) || Part::from_id(entry.part_id).is_none() {
            continue;
        }
        let quantity = quantities.entry(entry.key()).or_default();
        *quantity = quantity
            .checked_add(i64::from(entry.quantity))
            .context("Inventory quantity overflow")?;
    }
    quantities
        .into_iter()
        .filter(|(_, quantity)| *quantity > 0)
        .map(|(part_id, quantity)| {
            Ok(InventoryEntry {
                part_id,
                quantity: i32::try_from(quantity).context("Inventory quantity overflow")?,
            })
        })
        .collect()
}

/// Wizard cleanup applies to any inventory holder; unit load is reconciled before atomic commit.
/// This never changes installed equipment, armor, ammunition bins or repair state.
pub fn clean_inventory(
    world: &mut World,
    config: &Config,
    actor: ObjectId,
    object: ObjectId,
) -> Result<InventoryCleanup> {
    ensure!(
        crate::authority::is_wizard(world, actor),
        "Permission denied."
    );
    let entries = super::inventory(world, object)?;
    let cleaned = normalized(entries)?;
    let report = InventoryCleanup {
        original_entries: entries.len(),
        new_entries: cleaned.len(),
        items: cleaned.iter().try_fold(0u64, |sum, row| {
            sum.checked_add(row.quantity as u64)
                .context("Inventory item count overflow")
        })?,
    };
    world.attempt(|world| {
        let inventories = &mut world.btech.inventories;
        if cleaned.is_empty() {
            inventories.remove(&object);
        } else {
            inventories.insert(object, cleaned);
        }
        if world.btech.constructed_units().contains_key(&object)
            || world.btech.vehicles().contains_key(&object)
        {
            super::load::reconcile(world, object, config.battletech.tsm_tow_bonus != 0)?;
        }
        world.btech.validate_action(world)?;
        Ok(report)
    })
}

/// Stage the private summary with cleanup so output and callback failures restore both.
pub fn clean_inventory_action(
    scripts: &crate::Scripts,
    config: &Config,
    actor: ObjectId,
    object: ObjectId,
) -> Result<InventoryCleanup> {
    scripts.atomic(|_| {
        let report = clean_inventory(&mut scripts.world_mut(), config, actor, object)?;
        super::notify_message(scripts, super::MessageTarget::Player(actor), &report.text())?;
        scripts.effects.validate()?;
        Ok(report)
    })
}

/// FIXSTUFF operates on the operator's location and ignores arguments, as does the reference command.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    _input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let object = ctx
            .scripts
            .world()
            .objects
            .get(&ctx.player)
            .and_then(|player| player.location)
            .context("Player has no location")?;
        clean_inventory_action(ctx.scripts, ctx.config, ctx.player, object)
    })();
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cleanup sums signed duplicate rows and discards placeholders and unknowns.
    #[test]
    fn normalizes_stock_without_changing_part_identity() {
        let row = |part_id, quantity| InventoryEntry { part_id, quantity };
        let mut entries = vec![
            row(528, 9),
            row(528, -4),
            row(528, 3),
            row(-1, 1),
            row(i32::MAX, 1),
            row(529, -1),
            row(530, 0),
        ];
        entries.extend(
            [406, 407, 408, 428, 432, 433, 443, 444]
                .into_iter()
                .map(|id| row(id, 5)),
        );
        assert_eq!(normalized(&entries).unwrap(), vec![row(528, 8)]);
        assert!(normalized(&[row(528, i32::MAX), row(528, 1)]).is_err());
    }
}
