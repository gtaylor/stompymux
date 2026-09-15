//! Command-network status as a pure typed report and private cockpit display.
use super::BattleHexCoordinate;
use super::network_unit::unit as network_unit;
use crate::{BattleCommandNetwork, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Live status supplied by one running, unjammed peer, without requiring visual contact.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleNetworkStatusRow {
    pub unit: ObjectId,
    pub label: String,
    pub name: String,
    pub coordinate: BattleHexCoordinate,
    pub elevation: i32,
    pub range: f64,
    pub bearing: u16,
    pub speed: f64,
    pub heading: u16,
    pub armor_percent: u8,
    pub internal_percent: u8,
}

/// Network members available for automatic status reporting, excluding the requesting unit.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleNetworkStatusReport {
    pub rows: Vec<BattleNetworkStatusRow>,
    pub text: String,
}

/// Inspect C3i status without acquiring contacts, changing membership, or consuming dice.
pub fn status(world: &World, id: ObjectId, pilot: ObjectId) -> Result<BattleNetworkStatusReport> {
    report(world, id, pilot, BattleCommandNetwork::C3i)
}

/// Inspect classic C3 using the same report and its active master capacity.
pub fn c3_status(
    world: &World,
    id: ObjectId,
    pilot: ObjectId,
) -> Result<BattleNetworkStatusReport> {
    report(world, id, pilot, BattleCommandNetwork::C3)
}

/// Build a private report for the selected command-network family.
fn report(
    world: &World,
    id: ObjectId,
    pilot: ObjectId,
    kind: BattleCommandNetwork,
) -> Result<BattleNetworkStatusReport> {
    super::command_network::ready_for(world, id, pilot, kind)?;
    ensure!(
        !super::command_network::members_for(world, id, kind)?.is_empty(),
        "There are no other units in your {} network!",
        kind.name()
    );
    let members = super::command_network::active_members(world, id, kind, false)?;
    let mut rows = Vec::new();
    for peer in members {
        if peer == id {
            continue;
        }
        let unit = network_unit(world, peer)?;
        let position = unit.position().context("Peer is not placed")?;
        let motion = unit.motion().context("Peer has no motion state")?;
        let range = super::unit_range(world, id, peer)?;
        let (armor, original_armor, internal, original_internal) = unit.protection;
        rows.push(BattleNetworkStatusRow {
            unit: peer,
            label: unit
                .battlefield_id()
                .context("Peer has no battlefield ID")?
                .to_ascii_lowercase(),
            name: crate::text::plain(unit.name()),
            coordinate: BattleHexCoordinate {
                x: i32::from(position.x),
                y: i32::from(position.y),
            },
            elevation: super::unit_elevation(world, peer)?.context("Peer has no elevation")?,
            range: range.spatial,
            bearing: range.bearing.unwrap_or(180.0).round().rem_euclid(360.0) as u16,
            speed: motion.speed,
            heading: motion.heading.trunc().rem_euclid(360.0) as u16,
            armor_percent: percent(armor, original_armor),
            internal_percent: percent(internal, original_internal),
        });
    }
    let mut lines = vec![format!("{} Network Status:", kind.name())];
    for row in &rows {
        let name: String = row.name.chars().take(12).collect();
        let details = format!(
            "{} {name:<12} x:{:>3} y:{:>3} z:{:>3} r:{:>4.1} b:{:>3} s:{:>5.1} h:{:>3} a: {:>3} i: {:>3}",
            super::contacts::movement_type(world, row.unit)
                .chars()
                .next()
                .unwrap_or('U'),
            row.coordinate.x,
            row.coordinate.y,
            row.elevation,
            row.range,
            row.bearing,
            row.speed,
            row.heading,
            row.armor_percent,
            row.internal_percent
        );
        lines.push(format!(
            "[fg=yellow bold]{}[reset]{}[reset]",
            crate::text::escape(&format!("[{}]", row.label)),
            crate::text::escape(&details)
        ));
    }
    lines.push(format!("End {} Network Status", kind.name()));
    Ok(BattleNetworkStatusReport {
        rows,
        text: lines.join("\r\n"),
    })
}

/// Remaining protection uses the construction totals, including rear armor and destroyed sections.
fn percent(remaining: u32, original: u32) -> u8 {
    if original == 0 {
        return 0;
    }
    (remaining as f32 / original as f32 * 100.0) as u8
}

/// The native status report is private to the requesting pilot, not broadcast to the cockpit.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, input, BattleCommandNetwork::C3i)
}

/// Classic C3 private cockpit display.
pub(crate) fn c3_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, input, BattleCommandNetwork::C3)
}

/// Dispatch a report without broadcasting to other cockpit occupants.
fn command_for(
    ctx: &crate::CommandContext<'_>,
    _: &crate::CommandInput,
    kind: BattleCommandNetwork,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let world = ctx.scripts.world();
        let id = world
            .objects
            .get(&ctx.player)
            .and_then(|p| p.location)
            .context("Enter a unit first")?;
        report(&world, id, ctx.player, kind)
    })();
    Ok(crate::CommandAction::Report(crate::CommandReport::Reply(
        match result {
            Ok(report) => report.text,
            Err(error) => format!("{error:#}"),
        },
    )))
}
