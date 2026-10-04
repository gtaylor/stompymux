//! Catalogue-based Wizard stock additions, removals and clearing share atomic inventory edits.
use super::{CargoRow, DiagnosticChannel, DiagnosticMessage};
use crate::{Config, ObjectId, Scripts};
use anyhow::{Context, Result, ensure};

/// A complete operator stock request; quantities apply independently to each catalogue match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryChange {
    Add { pattern: String, quantity: i32 },
    Remove { pattern: String, quantity: i32 },
    Clear,
}

/// Prepare all inventory changes and diagnostics before committing; publication errors restore both.
pub fn change_inventory_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    object: ObjectId,
    change: InventoryChange,
) -> Result<Vec<CargoRow>> {
    ensure!(
        crate::authority::is_wizard(&scripts.world(), actor),
        "Permission denied."
    );
    super::inventory(&scripts.world(), object)?;
    scripts.atomic(|before| {
        let mut candidate = before.clone();
        let mut rows = Vec::new();
        let mut messages = Vec::new();
        if change == InventoryChange::Clear {
            candidate.btech.inventories.remove(&object);
            messages.push(DiagnosticMessage::new(
                DiagnosticChannel::Economy,
                format!("#{} reset #{}'s stuff.", actor.0, object.0),
            ));
        } else {
            let (pattern, quantity, add) = match &change {
                InventoryChange::Add { pattern, quantity } => (pattern, *quantity, true),
                InventoryChange::Remove { pattern, quantity } => (pattern, *quantity, false),
                InventoryChange::Clear => unreachable!(),
            };
            ensure!(
                !pattern.is_empty() && pattern.len() <= 1024,
                "Invalid part pattern"
            );
            ensure!(quantity > 0, "Invalid amount!");
            let quantity = quantity.min(50_000);
            let entries = super::stock_selection::TransferSelector::new(pattern).catalogue();
            ensure!(!entries.is_empty(), "Nothing matches '{pattern}'!");
            ensure!(
                actor == ObjectId(1) || entries.len() <= 20,
                "Wizards cannot change more than 20 different part types at a time"
            );
            for entry in entries {
                super::inventory::change_quantity(
                    &mut candidate,
                    object,
                    entry,
                    if add { quantity } else { -quantity },
                )?;
                let name = super::stock_selection::name(&entry);
                messages.push(super::channels::stock_message(
                    actor,
                    object,
                    &name,
                    if add { quantity } else { -quantity },
                ));
                rows.push(CargoRow {
                    quantity,
                    name,
                    part_id: entry.part_id,
                });
            }
        }
        if candidate.btech.constructed_units().contains_key(&object)
            || candidate.btech.vehicles().contains_key(&object)
        {
            super::load::reconcile(&mut candidate, object, config.battletech.tsm_tow_bonus != 0)?;
        }
        candidate.btech.validate(&candidate)?;
        *scripts.world_mut() = candidate;
        super::channels::publish(scripts, config, &messages)?;
        scripts.effects.validate()?;
        Ok(rows)
    })
}

/// Native stock tools operate on the Wizard's current location, without cockpit or cargo-power gates.
fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
    add: Option<bool>,
) -> Result<crate::CommandAction> {
    let result = (|| -> Result<String> {
        let object = ctx
            .scripts
            .world()
            .objects
            .get(&ctx.player)
            .and_then(|player| player.location)
            .context("Player has no location")?;
        let change = if let Some(add) = add {
            let (pattern, quantity) = input
                .args
                .trim()
                .rsplit_once(char::is_whitespace)
                .context("Expected part pattern and quantity")?;
            let pattern = pattern.trim().to_owned();
            let quantity = quantity.trim().parse()?;
            if add {
                InventoryChange::Add { pattern, quantity }
            } else {
                InventoryChange::Remove { pattern, quantity }
            }
        } else {
            InventoryChange::Clear
        };
        let rows = change_inventory_action(ctx.scripts, ctx.config, ctx.player, object, change)?;
        let Some(add) = add else {
            return Ok("Inventory cleaned!".to_owned());
        };
        Ok(rows
            .iter()
            .map(|row| {
                format!(
                    "You {} {} {}{}.",
                    if add { "add" } else { "remove" },
                    row.quantity,
                    row.name,
                    if row.quantity == 1 { "" } else { "s" }
                )
            })
            .collect::<Vec<_>>()
            .join("\n"))
    })();
    Ok(match result {
        Ok(text) => crate::CommandAction::CommitReply(text),
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

/// Add catalogue-selected stock to the current holder.
pub(crate) fn add_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command(ctx, input, Some(true))
}
/// Remove selected stock, flooring quantities at zero.
pub(crate) fn remove_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command(ctx, input, Some(false))
}
/// Clear all stored rows, including unknown imported identifiers.
pub(crate) fn clear_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command(ctx, input, None)
}

/// Scripted single-match stock adjustment retains signed counts and boolean no-match results.
pub fn add_stores_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    object: ObjectId,
    pattern: &str,
    quantity: i32,
) -> Result<bool> {
    ensure!(
        crate::authority::is_wizard(&scripts.world(), actor),
        "Permission denied."
    );
    super::inventory(&scripts.world(), object)?;
    ensure!(pattern.len() < 2048, "Part name is too long");
    let quantity = quantity.min(50_000);
    if quantity == 0 {
        return Ok(true);
    }
    if pattern.is_empty() {
        return Ok(false);
    }
    let Some(entry) = super::stock_selection::TransferSelector::new(pattern).first() else {
        return Ok(false);
    };
    scripts.atomic(|before| {
        let mut candidate = before.clone();
        super::inventory::change_quantity(&mut candidate, object, entry, quantity)?;
        if candidate.btech.constructed_units().contains_key(&object)
            || candidate.btech.vehicles().contains_key(&object)
        {
            super::load::reconcile(&mut candidate, object, config.battletech.tsm_tow_bonus != 0)?;
        }
        candidate.btech.validate(&candidate)?;
        *scripts.world_mut() = candidate;
        let message = DiagnosticMessage::new(
            DiagnosticChannel::Economy,
            format!(
                "#{} added {} {} to #{}",
                actor.0,
                quantity,
                super::stock_selection::name(&entry),
                object.0
            ),
        );
        super::channels::publish(scripts, config, &[message])?;
        scripts.effects.validate()?;
        Ok(true)
    })
}
