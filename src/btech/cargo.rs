//! Shared cockpit manifests and atomic loose-stock transfers with operation-specific admission.
use super::BattlePower;
use super::stock_selection::{TransferSelector, name, selected};
use crate::{Config, Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Cargo operations retain distinct power, pilot, hangar and movement requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleCargoOperation {
    Manifest,
    Stores,
    Load,
    Unload,
}

/// Detached stock row used by both cockpit and scripted reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleCargoRow {
    pub part_id: i32,
    pub quantity: i32,
    pub name: String,
}

/// Common cargo access applies before inventory inspection, even for Wizards.
fn enabled(config: &Config, pattern: &str) -> Result<()> {
    ensure!(config.battletech.allow_cargo_commands, "Permission denied.");
    ensure!(pattern.len() <= 1024, "Cargo pattern is too long");
    Ok(())
}

/// Match reference cockpit exceptions without imposing combat's stricter assigned-pilot gate.
fn admit(
    world: &World,
    actor: ObjectId,
    operation: BattleCargoOperation,
) -> Result<(ObjectId, ObjectId)> {
    let player = world
        .objects
        .get(&actor)
        .filter(|p| p.kind == crate::Kind::Player && !p.flags.contains(Flag::Going))
        .context("Player is unavailable")?;
    let id = player.location.context("Enter a unit first")?;
    let object = world
        .objects
        .get(&id)
        .filter(|o| !o.flags.contains(Flag::Going))
        .context("Unit is unavailable")?;
    let unit = super::scanner::scanner_unit(world, id).context("Enter a constructed unit first")?;
    let (pilot, recovery, cargo) =
        super::with_unit!(world.btech.unit(id).expect("scanned unit"), |unit| (
            unit.pilot(),
            unit.crew_recovery().remaining,
            unit.definition().has_special("CargoTech"),
        ));
    let running = unit.power == BattlePower::Running;
    if operation != BattleCargoOperation::Unload {
        ensure!(!unit.destroyed, "You are destroyed!");
        ensure!(running, "Reactor is not online!");
    }
    let unconscious = pilot.map_or(recovery > 0, |pilot| world.btech.unconscious(pilot));
    ensure!(
        !unconscious || (running && pilot != Some(actor)),
        "You are unconscious....zzzzzzz"
    );
    if operation != BattleCargoOperation::Stores
        && object.flags.contains(Flag::InCharacter)
        && !crate::authority::is_wizard(world, actor)
    {
        ensure!(
            pilot == Some(actor),
            "Now now, only the pilot can push that button."
        );
    }
    let position = unit.position.context("You are on no map!")?;
    let map = world
        .objects
        .get(&position.map)
        .context("You are on an invalid map!")?;
    ensure!(
        object.location == Some(position.map),
        "You ain't in hangar!"
    );
    if operation != BattleCargoOperation::Stores {
        ensure!(cargo, "This unit cannot haul cargo!");
    }
    if operation == BattleCargoOperation::Load {
        ensure!(unit.speed == 0.0, "You're moving too fast!");
    }
    if operation != BattleCargoOperation::Unload {
        ensure!(
            !map.flags.contains(Flag::InCharacter),
            "You aren't inside a hangar!"
        );
        super::check_cargo_transfer_point(
            world,
            position.map,
            i32::from(position.x),
            i32::from(position.y),
        )?;
    }
    Ok((id, position.map))
}

/// List carried or bay stock; MANIFEST can list any live object containing the actor.
pub fn cargo_manifest(
    world: &World,
    config: &Config,
    actor: ObjectId,
    stores: bool,
    pattern: &str,
) -> Result<Vec<BattleCargoRow>> {
    enabled(config, pattern)?;
    let holder = if stores {
        admit(world, actor, BattleCargoOperation::Stores)?.1
    } else {
        world
            .objects
            .get(&actor)
            .filter(|p| p.kind == crate::Kind::Player && !p.flags.contains(Flag::Going))
            .and_then(|p| p.location)
            .context("Player has no location")?
    };
    Ok(super::inventory(world, holder)?
        .iter()
        .filter(|entry| selected(entry, pattern))
        .map(|entry| BattleCargoRow {
            part_id: entry.part_id,
            quantity: entry.quantity,
            name: name(entry),
        })
        .collect())
}

/// Move bounded quantities in a candidate world; every stock edit and throttle correction commits together.
pub fn transfer_cargo(
    world: &mut World,
    config: &Config,
    actor: ObjectId,
    load: bool,
    pattern: &str,
    quantity: i32,
) -> Result<Vec<BattleCargoRow>> {
    enabled(config, pattern)?;
    ensure!(
        !pattern.trim().is_empty() && quantity > 0,
        "Invalid cargo name or amount!"
    );
    let quantity = quantity.min(50_000);
    let (unit, map) = admit(
        world,
        actor,
        if load {
            BattleCargoOperation::Load
        } else {
            BattleCargoOperation::Unload
        },
    )?;
    let (source, destination) = if load { (map, unit) } else { (unit, map) };
    let selector = TransferSelector::new(pattern);
    let entries: Vec<_> = super::inventory(world, source)?
        .iter()
        .copied()
        .filter(|entry| selector.contains(entry))
        .collect();
    ensure!(
        !entries.is_empty(),
        "Nothing matching that criteria was found!"
    );
    world.attempt(|world| {
        let mut moved = Vec::new();
        for entry in entries {
            let count = quantity.min(entry.quantity);
            let existing = super::inventory(world, destination)?
                .iter()
                .find(|row| row.key() == entry.key())
                .map_or(0, |row| row.quantity);
            let next = existing
                .checked_add(count)
                .context("Destination inventory quantity overflow")?;
            super::inventory::edit_quantity(world, source, entry.part_id, entry.quantity - count)?;
            super::inventory::edit_quantity(world, destination, entry.part_id, next)?;
            moved.push(BattleCargoRow {
                part_id: entry.part_id,
                quantity: count,
                name: name(&entry),
            });
        }
        super::load::reconcile(world, unit, config.battletech.tsm_tow_bonus != 0)?;
        world.btech.validate(world)?;
        Ok(moved)
    })
}

/// Transfer stock and publish economy diagnostics under one world/effects checkpoint.
/// Missing diagnostic channels are ignored; existing channels retain normal delivery and history.
pub fn transfer_cargo_action(
    scripts: &crate::Scripts,
    config: &Config,
    actor: ObjectId,
    load: bool,
    pattern: &str,
    quantity: i32,
) -> Result<Vec<BattleCargoRow>> {
    scripts.atomic(|before| {
        let rows = transfer_cargo(
            &mut scripts.world_mut(),
            config,
            actor,
            load,
            pattern,
            quantity,
        )?;
        let unit = before
            .objects
            .get(&actor)
            .and_then(|player| player.location)
            .context("Player has no unit")?;
        let map = before
            .objects
            .get(&unit)
            .and_then(|unit| unit.location)
            .context("Unit has no map")?;
        let messages = rows
            .iter()
            .flat_map(|row| {
                [(unit, load), (map, !load)].map(|(holder, added)| {
                    super::channels::stock_message(
                        actor,
                        holder,
                        &row.name,
                        if added { row.quantity } else { -row.quantity },
                    )
                })
            })
            .collect::<Vec<_>>();
        super::channels::publish(scripts, config, &messages)?;
        scripts.effects.validate()?;
        Ok(rows)
    })
}

/// Plain cockpit output keeps the same rows and transfer counts as Lua.
fn rows_text(rows: &[BattleCargoRow], verb: Option<&str>) -> String {
    if rows.is_empty() {
        return "No cargo found.".into();
    }
    rows.iter()
        .map(|row| match verb {
            Some(verb) => format!(
                "You {verb} {} {}{}.",
                row.quantity,
                row.name,
                if row.quantity == 1 { "" } else { "s" }
            ),
            None => format!("{} {}", row.quantity, row.name),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Native adapters use the same domain operations and enclosing command transaction.
fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
    operation: BattleCargoOperation,
) -> Result<crate::CommandAction> {
    let result = (|| -> Result<String> {
        if matches!(
            operation,
            BattleCargoOperation::Manifest | BattleCargoOperation::Stores
        ) {
            return Ok(rows_text(
                &cargo_manifest(
                    &ctx.scripts.world(),
                    ctx.config,
                    ctx.player,
                    operation == BattleCargoOperation::Stores,
                    input.args.trim(),
                )?,
                None,
            ));
        }
        let (pattern, count) = input
            .args
            .trim()
            .rsplit_once(char::is_whitespace)
            .context("Usage: loadcargo/unloadcargo <part pattern> <quantity>")?;
        let load = operation == BattleCargoOperation::Load;
        let rows = transfer_cargo_action(
            ctx.scripts,
            ctx.config,
            ctx.player,
            load,
            pattern.trim(),
            count.trim().parse()?,
        )?;
        Ok(rows_text(&rows, Some(if load { "load" } else { "unload" })))
    })();
    Ok(match result {
        Ok(text) => crate::CommandAction::CommitReply(text),
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

/// List stock at the actor's current location.
pub(crate) fn manifest_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command(ctx, input, BattleCargoOperation::Manifest)
}
/// List stock at the unit's admitted loading bay.
pub(crate) fn stores_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command(ctx, input, BattleCargoOperation::Stores)
}
/// Load available stock through the bay and cockpit gates.
pub(crate) fn load_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command(ctx, input, BattleCargoOperation::Load)
}
/// Unload stock without the load-only power and bay-location restrictions.
pub(crate) fn unload_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command(ctx, input, BattleCargoOperation::Unload)
}
