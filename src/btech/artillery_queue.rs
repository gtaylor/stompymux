//! Map-owned artillery queues retain admitted shots through shooter loss and server restart.
use super::*;
use crate::{Config, Flag, ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};

/// An admitted launch. Shooter identity is historical and does not keep that object alive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtilleryShot {
    pub shooter: ObjectId,
    pub flight: ArtilleryFlight,
}

impl StoredMap {
    /// Stable launch order; shot identifiers remain fixed until their arrival.
    pub fn artillery_shots(&self) -> &BTreeMap<u32, ArtilleryShot> {
        &self.artillery_shots
    }

    /// Validate saved queue bounds without requiring historical shooters to remain present.
    pub(crate) fn validate_artillery(&self) -> Result<()> {
        ensure!(
            self.artillery_shots.len() <= 4096,
            "Too many artillery shots"
        );
        ensure!(
            self.artillery_shots.is_empty() || self.terrain_ready(),
            "Artillery requires decoded terrain"
        );
        for shot in self.artillery_shots.values() {
            ensure!(
                shot.shooter.0 >= 0 && shot.flight.remaining() > 0,
                "Invalid queued artillery shot"
            );
            for coordinate in [shot.flight.origin(), shot.flight.target()] {
                ensure!(
                    coordinate.x >= 0
                        && coordinate.y >= 0
                        && i64::from(coordinate.x) < self.width
                        && i64::from(coordinate.y) < self.height,
                    "Artillery launch is outside its map"
                );
            }
        }
        ensure!(
            self.artillery_shots.is_empty() || self.fire_dice.is_some(),
            "Artillery requires a saved map random stream"
        );
        Ok(())
    }
}

/// Store a launch already authorized and paid for by the enclosing firing transaction.
/// This domain API checks ownership/geometry; it is not a player-facing firing command.
pub fn enqueue_artillery(
    world: &mut World,
    map: ObjectId,
    shooter: ObjectId,
    flight: ArtilleryFlight,
) -> Result<u32> {
    ensure!(
        world
            .objects
            .get(&shooter)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Artillery shooter is unavailable"
    );
    ensure!(
        world
            .objects
            .get(&map)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Map is unavailable"
    );
    let unit =
        super::scanner::scanner_unit(world, shooter).context("Artillery shooter is unavailable")?;
    let position = unit.position.context("Artillery shooter is not placed")?;
    ensure!(
        position.map == map
            && flight.origin()
                == (HexCoordinate {
                    x: i32::from(position.x),
                    y: i32::from(position.y)
                }),
        "Artillery launch does not match shooter position"
    );
    let record = world.btech.maps().get(&map).context("Map not found")?;
    record.validate()?;
    ensure!(record.terrain_ready(), "Map terrain is unavailable");
    ensure!(
        record.artillery_shots.len() < 4096,
        "Too many artillery shots"
    );
    let ordinal = record
        .artillery_shots
        .last_key_value()
        .map(|(&id, _)| id.checked_add(1).context("Artillery sequence exhausted"))
        .transpose()?
        .unwrap_or(0);
    world.attempt(|world| {
        let record = world.btech.maps.get_mut(&map).unwrap();
        Arc::make_mut(&mut record.artillery_shots)
            .insert(ordinal, ArtilleryShot { shooter, flight });
        record.validate_artillery()?;
        Ok(ordinal)
    })
}

/// Queued rounds keep otherwise idle battlefields in the committed-second simulation.
pub fn artillery_pending(world: &World) -> bool {
    world.btech.maps().iter().any(|(id, map)| {
        !map.artillery_shots.is_empty()
            && world
                .objects
                .get(id)
                .is_some_and(|object| !object.flags.contains(Flag::Going))
    })
}

/// Advance the launch-order snapshot once and publish arrivals, restoring every shot on any failure.
pub fn advance_artillery_action(
    scripts: &Scripts,
    config: &Config,
    rules: FallRules,
) -> Result<Vec<ArtilleryImpactReport>> {
    if !artillery_pending(&scripts.world.borrow()) {
        return Ok(Vec::new());
    }
    scripts.atomic(|before| {
        let shots: Vec<_> = before
            .btech
            .maps()
            .iter()
            .filter(|(id, _)| {
                before
                    .objects
                    .get(id)
                    .is_some_and(|object| !object.flags.contains(Flag::Going))
            })
            .flat_map(|(&map, record)| {
                record.artillery_shots.iter().map(move |(&ordinal, shot)| {
                    (
                        map,
                        ordinal,
                        super::fire_target::TargetSource::from(shot.shooter),
                        shot.flight.clone(),
                    )
                })
            })
            .collect();
        let mut reports = Vec::new();
        for (map, ordinal, source, mut flight) in shots {
            let mut report =
                advance_artillery_flight_action(scripts, config, map, &mut flight, rules)?;
            if let Some(report) = &mut report {
                let notices = super::artillery_adjustment::observe_miss(
                    &mut scripts.world.borrow_mut(),
                    map,
                    source,
                    &report.pattern,
                )?;
                for notice in &notices {
                    super::notify_unit(scripts, notice.clone())?;
                }
                report.notices.extend(notices);
            }
            let mut world = scripts.world.borrow_mut();
            let record = world
                .btech
                .maps
                .get_mut(&map)
                .context("Artillery map disappeared during arrival")?;
            let shots = Arc::make_mut(&mut record.artillery_shots);
            if let Some(report) = report {
                shots.remove(&ordinal);
                reports.push(report);
            } else {
                shots
                    .get_mut(&ordinal)
                    .context("Artillery shot disappeared during arrival")?
                    .flight = flight;
            }
        }
        scripts.world.borrow().validate_action(config)?;
        Ok(reports)
    })
}
