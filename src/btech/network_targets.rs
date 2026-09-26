//! Network contact reports combine sightings without granting the requester acquired contacts.
use super::network_unit::unit as network_unit;
use super::{BattleContactArc, BattleDetectionChannel, BattleHexCoordinate, BattleNetworkRange};
use crate::{BattleCommandNetwork, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// One directly or remotely detected target, with physical and network distance kept separate.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleNetworkTargetRow {
    pub unit: ObjectId,
    pub label: String,
    pub name: String,
    pub identified: bool,
    /// How this unit itself perceives the target; absent for network-only sightings.
    pub detection: Option<BattleDetectionChannel>,
    pub weapon_arc: BattleContactArc,
    pub coordinate: BattleHexCoordinate,
    pub elevation: i32,
    pub range: f64,
    pub network_range: BattleNetworkRange,
    pub bearing: u16,
    pub speed: f64,
    pub heading: u16,
    pub status: String,
    pub destroyed: bool,
    pub selected: bool,
    pub friendly: bool,
}

/// Pure network target report, with private native presentation and detached Lua rows.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleNetworkTargetReport {
    pub rows: Vec<BattleNetworkTargetRow>,
    pub text: String,
}

/// Inspect the union of direct contacts and usable network sightings without acquiring any contact.
pub fn targets(world: &World, id: ObjectId, pilot: ObjectId) -> Result<BattleNetworkTargetReport> {
    report(world, id, pilot, BattleCommandNetwork::C3i)
}

/// Inspect classic C3 using the same report and its active master capacity.
pub fn c3_targets(
    world: &World,
    id: ObjectId,
    pilot: ObjectId,
) -> Result<BattleNetworkTargetReport> {
    report(world, id, pilot, BattleCommandNetwork::C3)
}

/// Build a private report for the selected command-network family.
fn report(
    world: &World,
    id: ObjectId,
    pilot: ObjectId,
    kind: BattleCommandNetwork,
) -> Result<BattleNetworkTargetReport> {
    super::command_network::ready_for(world, id, pilot, kind)?;
    ensure!(
        !super::command_network::members_for(world, id, kind)?.is_empty(),
        "There are no other units in your {} network!",
        kind.name()
    );
    let members = super::command_network::active_members(world, id, kind, false)?;
    let observer = network_unit(world, id)?;
    let map = observer.position().context("Unit is not placed")?.map;
    let peers: Vec<_> = members.into_iter().filter(|peer| *peer != id).collect();
    let mut rows = Vec::new();
    // One lazily built reader per observer shares its perception profile across every target.
    let mut readers = std::collections::BTreeMap::new();
    for target in super::map_slots::all_unit_order(world, map)? {
        if target == id {
            continue;
        }
        let direct = contact_view(world, &mut readers, id, target)?;
        let mut seen = direct.is_some();
        let mut identified = direct.as_ref().is_some_and(|c| c.identified);
        for &peer in &peers {
            if peer == target {
                continue;
            }
            if let Some(contact) = contact_view(world, &mut readers, peer, target)? {
                seen = true;
                identified |= contact.identified;
            }
        }
        if !seen {
            continue;
        }
        let unit = network_unit(world, target)?;
        let position = unit.position().context("Target is not placed")?;
        let motion = unit.motion().context("Target has no motion")?;
        let range = super::unit_range(world, id, target)?;
        let friendly = unit.signature().team == observer.signature().team;
        let mut label = unit
            .battlefield_id()
            .context("Target has no battlefield ID")?;
        if friendly || !identified {
            label.make_ascii_lowercase();
        }
        rows.push(BattleNetworkTargetRow {
            unit: target,
            label,
            name: if identified {
                crate::text::plain(unit.name())
            } else {
                "something".into()
            },
            identified,
            friendly,
            detection: direct.as_ref().and_then(|contact| contact.detection),
            weapon_arc: observer.facing().contact_arc(
                observer.motion().context("Observer has no motion")?.heading,
                range.bearing.unwrap_or(180.0),
            )?,
            coordinate: BattleHexCoordinate {
                x: i32::from(position.x),
                y: i32::from(position.y),
            },
            elevation: super::unit_elevation(world, target)?.context("Target has no elevation")?,
            range: range.spatial,
            network_range: super::network_range::select(
                world,
                id,
                super::network_range::NetworkTarget::Unit(target),
                range.spatial,
                &peers,
                kind,
            )?,
            bearing: range.bearing.unwrap_or(180.0).round().rem_euclid(360.0) as u16,
            speed: motion.speed,
            heading: motion.heading.trunc().rem_euclid(360.0) as u16,
            status: if identified {
                super::contact_status::known_status(world, id, target)?
            } else {
                "     ".into()
            },
            destroyed: unit.is_destroyed(),
            selected: observer.selected() == Some(target),
        });
    }
    // Destroyed contacts sort first, then descending physical range; equal keys retain map order.
    let order =
        |row: &BattleNetworkTargetRow| row.range + if row.destroyed { 10000.0 } else { 0.0 };
    rows.sort_by(|a, b| order(b).total_cmp(&order(a)));
    let mut lines = vec![format!("{} Contacts:", kind.name())];
    for row in &rows {
        let name: String = row.name.chars().take(11).collect();
        let line = format!(
            "{}{}{}[{}]{} {name:<11} x:{:>3} y:{:>3} z:{:>3} r:{:>4.1} c:{:>4.1} b:{:>3} s:{:>5.1} h:{:>3} S:{}",
            super::contacts::detection_code(row.detection, row.identified),
            ' ',
            row.weapon_arc.symbol(),
            row.label,
            super::contacts::movement_type(world, row.unit)
                .chars()
                .next()
                .unwrap_or('U'),
            row.coordinate.x,
            row.coordinate.y,
            row.elevation,
            row.range,
            row.network_range.distance,
            row.bearing,
            row.speed,
            row.heading,
            row.status
        );
        let line = crate::text::escape(&line);
        lines.push(if row.selected {
            format!("[fg=red bold]{line}[reset]")
        } else if row.identified && !row.friendly {
            format!("[fg=yellow bold]{line}[reset]")
        } else {
            line
        });
    }
    lines.push(format!("End {} Contact List", kind.name()));
    Ok(BattleNetworkTargetReport {
        rows,
        text: lines.join("\r\n"),
    })
}

/// Native target reports are private to the requesting pilot.
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

/// Read `observer`'s view of `target`, building that observer's contact reader on first use.
fn contact_view<'w>(
    world: &'w World,
    readers: &mut std::collections::BTreeMap<ObjectId, super::contacts::ContactReader<'w>>,
    observer: ObjectId,
    target: ObjectId,
) -> Result<Option<super::BattleContactView>> {
    let reader = match readers.entry(observer) {
        std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(super::contacts::ContactReader::new(world, observer)?)
        }
    };
    reader.view(target)
}
