//! Shared endpoint admission for cockpit coordinate and target measurements.
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Source of an endpoint's live elevation.
pub(super) enum EndpointSource {
    Unit(ObjectId),
    Hex(super::HexCoordinate),
}

/// Continuous location plus the data needed for vertical measurements.
pub(super) struct Endpoint {
    pub point: super::Point,
    pub source: EndpointSource,
}

/// Validated navigation request and its player-facing description.
pub(super) struct Segment {
    pub origin: Endpoint,
    pub destination: Endpoint,
    pub prefix: String,
    pub map: ObjectId,
    /// Physical observing unit.
    pub observer: ObjectId,
}

/// Resolve shared target visibility and coordinate syntax without changing the world.
pub(super) fn resolve(
    world: &World,
    unit: ObjectId,
    viewer: ObjectId,
    arguments: &str,
    operation: &str,
) -> Result<Segment> {
    let source = super::brief::display_source(world, unit, viewer)?;
    let unit = source.unit;
    let record = super::scanner::scanner_unit(world, unit).context("Unit is unavailable")?;
    ensure!(
        record.power == super::BattlePower::Running && !record.destroyed,
        "Start the unit first"
    );
    let position = record.position.context("You are not on a map!")?;
    let map = world
        .btech
        .maps()
        .get(&position.map)
        .context("Map not found")?;
    let mut origin = Endpoint {
        point: record.point.context("Unit has no motion state")?,
        source: EndpointSource::Unit(unit),
    };
    let words: Vec<_> = arguments.split_whitespace().collect();
    ensure!(
        matches!(words.len(), 0 | 2 | 4),
        "Invalid number of attributes to {operation} function!"
    );
    let values: Vec<i32> = words
        .iter()
        .map(|word| word.parse().context("Invalid map coordinates!"))
        .collect::<Result<_>>()?;
    let checked = |x: i32, y: i32, origin: bool| -> Result<Endpoint> {
        // Bearing's explicit origin admits the east border because no terrain is read there.
        ensure!(
            x >= 0
                && i64::from(x) < map.width + i64::from(origin)
                && y >= 0
                && i64::from(y) < map.height,
            "Invalid map coordinates!"
        );
        Ok(Endpoint {
            point: super::HexCoordinate { x, y }.center(),
            source: EndpointSource::Hex(super::HexCoordinate { x, y }),
        })
    };
    let (destination, prefix) = match values.as_slice() {
        [] => {
            let point = match source
                .selection(world)
                .context("There is no default target!")?
            {
                super::BattleTargetSelection::Unit(lock) => {
                    ensure!(
                        super::visible_contact(world, unit, lock.target)?.is_some(),
                        "Target is not in line of sight!"
                    );
                    Endpoint {
                        point: super::scanner::scanner_unit(world, lock.target)
                            .context("Target is unavailable")?
                            .point
                            .context("Target has no motion state")?,
                        source: EndpointSource::Unit(lock.target),
                    }
                }
                super::BattleTargetSelection::Hex(lock) => checked(lock.hex.x, lock.hex.y, false)?,
            };
            (point, format!("{operation} to default target is: "))
        }
        [x, y] => (
            checked(*x, *y, false)?,
            format!("{operation} to  {x},{y} is: "),
        ),
        [x0, y0, x1, y1] => {
            origin = checked(*x0, *y0, true)?;
            (
                checked(*x1, *y1, false)?,
                format!("{operation} to {x1},{y1} from {x0},{y0} is: "),
            )
        }
        _ => unreachable!("argument count checked"),
    };
    Ok(Segment {
        origin,
        destination,
        prefix,
        map: position.map,
        observer: unit,
    })
}

/// Live height in map levels; water and ice coordinate endpoints use signed bed elevation.
pub(super) fn elevation(world: &World, map: ObjectId, endpoint: &Endpoint) -> Result<f64> {
    match endpoint.source {
        EndpointSource::Unit(id) => {
            super::unit_altitude(world, id)?.context("Unit has no elevation")
        }
        EndpointSource::Hex(hex) => {
            let tile = world.btech.maps()[&map].base_hex(i64::from(hex.x), i64::from(hex.y))?;
            Ok(f64::from(tile.surface_height()))
        }
    }
}

/// Compare displayed precision before adding a separate horizontal distance.
pub(super) fn range_text(spatial: f64, horizontal: f64) -> String {
    let distance = format!("{spatial:.1}");
    let ground = format!("{horizontal:.1}");
    if distance == ground {
        return format!("{distance} hexes");
    }
    format!("{distance} hexes ({ground} ground hexes)")
}
