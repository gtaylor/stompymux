//! Shared tactical and long-range display centering, independent of text rendering or map overlays.
use super::HexCoordinate;
use crate::{ObjectId, World};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};

/// Hardware radius used to admit a display center.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleViewKind {
    Tactical,
    LongRange,
}

/// A view can follow the cockpit, an acquired contact or a relative compass projection.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BattleViewCenter {
    OwnUnit,
    Contact { target: ObjectId },
    Projection { bearing: i32, distance: f64 },
}

/// Resolved display center; projected coordinates may lie outside the map before viewport clipping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleViewPosition {
    /// Battlefield containing the scanner.
    pub map: ObjectId,
    /// Requested center before the renderer clips the viewport to map bounds.
    pub center: HexCoordinate,
    /// Damage-adjusted hardware radius for the chosen display.
    pub maximum_range: u8,
}

/// Resolve display centering without changing targets, contacts, dice or simulation state.
/// Projection admission uses truncated absolute distance and allows observer exemptions;
/// contact admission uses spatial distance and retains its range limit for observers.
pub fn resolve_view_center(
    world: &World,
    observer: ObjectId,
    pilot: ObjectId,
    kind: BattleViewKind,
    center: BattleViewCenter,
) -> Result<BattleViewPosition> {
    resolve_center(world, observer, pilot, kind, |_| Ok(center), true)
}

/// Admit the cockpit and display hardware before parsing or resolving a requested center.
/// Navigation can show the current hex with failed scanners; other displays require hardware.
fn resolve_center(
    world: &World,
    observer: ObjectId,
    pilot: ObjectId,
    kind: BattleViewKind,
    center: impl FnOnce(ObjectId) -> Result<BattleViewCenter>,
    require_hardware: bool,
) -> Result<BattleViewPosition> {
    let observer = super::combat_operator::for_owner(world, observer, pilot)?
        .source
        .unit;
    let unit = super::scanner::scanner_unit(world, observer).context("Scanner is unavailable")?;
    ensure!(
        unit.power == super::BattlePower::Running && !unit.destroyed,
        "Start the unit first"
    );
    let ranges = world.btech.vehicles().get(&observer).map_or_else(
        || world.btech.constructed_units()[&observer].sensor_ranges(),
        super::BattleVehicle::sensor_ranges,
    );
    let maximum_range = match kind {
        BattleViewKind::Tactical => ranges.tactical,
        BattleViewKind::LongRange => ranges.long_range,
    };
    ensure!(
        !require_hardware || maximum_range > 0,
        "Your system seems to be inoperational."
    );
    let position = unit.position.context("Unit is not on a battlefield")?;
    let center = match center(observer)? {
        BattleViewCenter::OwnUnit => HexCoordinate {
            x: i32::from(position.x),
            y: i32::from(position.y),
        },
        BattleViewCenter::Contact { target } => {
            let view =
                super::visible_contact(world, observer, target)?.context("No such target.")?;
            ensure!(
                view.range.spatial.trunc() <= f64::from(maximum_range),
                "Target is out of scanner range."
            );
            let target = super::scanner::scanner_unit(world, target)
                .context("Target is unavailable")?
                .position
                .context("Target is not placed")?;
            HexCoordinate {
                x: i32::from(target.x),
                y: i32::from(target.y),
            }
        }
        BattleViewCenter::Projection { bearing, distance } => {
            ensure!(distance.is_finite(), "Invalid bearing or range.");
            ensure!(
                unit.observer || distance.trunc().abs() <= f64::from(maximum_range),
                "Those coordinates are out of sensor range!"
            );
            // Project supports nonnegative distances; reverse the compass for negative requests.
            let heading =
                (f64::from(bearing) + if distance < 0.0 { 180.0 } else { 0.0 }).rem_euclid(360.0);
            unit.point
                .context("Unit has no motion state")?
                .project(heading, distance.abs())?
                .containing_hex()?
        }
    };
    Ok(BattleViewPosition {
        map: position.map,
        center,
        maximum_range,
    })
}

/// Parse the common display argument grammar using labels/dbrefs or integer bearing and finite range.
pub fn parse_view_center(
    world: &World,
    observer: ObjectId,
    pilot: ObjectId,
    kind: BattleViewKind,
    arguments: &str,
) -> Result<BattleViewPosition> {
    resolve_center(
        world,
        observer,
        pilot,
        kind,
        |observer| parse_center(world, observer, arguments),
        true,
    )
}

/// Resolve native navigation arguments without the display hardware admission gate.
pub(super) fn navigation_center(
    world: &World,
    observer: ObjectId,
    pilot: ObjectId,
    arguments: &str,
) -> Result<BattleViewPosition> {
    resolve_center(
        world,
        observer,
        pilot,
        BattleViewKind::Tactical,
        |observer| parse_center(world, observer, arguments),
        false,
    )
}

/// Decode a center request independently of display admission.
fn parse_center(world: &World, observer: ObjectId, arguments: &str) -> Result<BattleViewCenter> {
    let args: Vec<_> = arguments.split_whitespace().collect();
    let center = match args.as_slice() {
        [] => BattleViewCenter::OwnUnit,
        [target] => BattleViewCenter::Contact {
            target: super::radio_targeted::target(world, observer, target)?,
        },
        [bearing, distance] => BattleViewCenter::Projection {
            bearing: bearing.parse().context("Invalid bearing or range.")?,
            distance: distance.parse().context("Invalid bearing or range.")?,
        },
        _ => bail!("Invalid number of parameters!"),
    };
    Ok(center)
}
