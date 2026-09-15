//! Sensor visibility of terrain coordinates without acquiring or identifying their occupants.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Inspect an empty terrain target on the observer's map without contacts, randomness or mutation.
pub fn hex_visible(world: &World, observer: ObjectId, target: BattleHexCoordinate) -> Result<bool> {
    Ok(observation(world, observer, target)?.visible)
}

/// Report eligible terrain sensor roles without acquiring or identifying occupants.
pub fn hex_sensor_visibility(
    world: &World,
    observer: ObjectId,
    target: BattleHexCoordinate,
) -> Result<BattleContactSensors> {
    Ok(observation(world, observer, target)?.sensors)
}

/// Artillery selects an observer by sight before applying its separate power-state check.
pub(super) fn observation_visible(
    world: &World,
    observer: ObjectId,
    target: BattleHexCoordinate,
) -> Result<bool> {
    Ok(sensor_visibility(world, observer, target, false)?.visible)
}

/// Visibility can be granted without assigning a sensor role to an operator observation.
#[derive(Default)]
pub(super) struct HexObservation {
    pub visible: bool,
    pub sensors: BattleContactSensors,
}

/// Shared terrain observation for displays that also report primary and secondary roles.
pub(super) fn observation(
    world: &World,
    observer: ObjectId,
    target: BattleHexCoordinate,
) -> Result<HexObservation> {
    sensor_visibility(world, observer, target, true)
}

/// Evaluate the same optics while leaving observer-selection order to the caller.
fn sensor_visibility(
    world: &World,
    observer: ObjectId,
    target: BattleHexCoordinate,
    require_running: bool,
) -> Result<HexObservation> {
    ensure!(
        world
            .objects
            .get(&observer)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Observer is unavailable"
    );
    let unit =
        super::scanner::scanner_unit(world, observer).context("Observer is not constructed")?;
    let position = unit.position.context("Observer is not placed")?;
    let map = &world.btech.maps()[&position.map];
    let (terrain, distance) = super::los::unit_hex_los(world, observer, target)?;
    if require_running && unit.power != BattlePower::Running {
        return Ok(HexObservation::default());
    }
    if unit.visibility.clairvoyant {
        return Ok(HexObservation {
            visible: true,
            sensors: BattleContactSensors::default(),
        });
    }
    if distance > map.maximum_visibility as f64 {
        return Ok(HexObservation::default());
    }
    if super::clouds::blocks_terrain(
        map.cloud_base,
        super::los::unit_sight_point(world, observer)?.level,
        unit.pair,
    ) {
        return Ok(HexObservation::default());
    }
    let light = match map.light {
        0 => BattleLight::Night,
        1 => BattleLight::Twilight,
        2 => BattleLight::Day,
        _ => anyhow::bail!("Invalid battlefield light"),
    };
    let target_lit = super::hex_illuminated(world, position.map, target)?;
    let pair = unit.pair;
    let mut sensors = BattleContactSensors::default();
    for (index, sensor) in [pair.primary, pair.secondary].into_iter().enumerate() {
        if index == 1 && pair.primary == pair.secondary {
            sensors.secondary = sensors.primary;
            continue;
        }
        let disabled = map.optical_sensor_disabled(sensor);
        let eligible = match sensor {
            BattleSensorMode::Visual
            | BattleSensorMode::LightAmplification
            | BattleSensorMode::Infrared => {
                sensor
                    .evaluate_with_ceiling(
                        terrain,
                        distance,
                        false,
                        BattleSensorConditions {
                            light,
                            visibility: u8::try_from(map.visibility)?,
                            target_lit,
                            disabled,
                        },
                        u16::try_from(map.maximum_visibility)?,
                        super::sensors::sensor_maximum(world, observer, 15),
                    )?
                    .eligible
            }
            BattleSensorMode::Electromagnetic => BattleElectromagneticRules {
                signal_strength: super::scanner::scanner_unit(world, observer)
                    .context("Observer is unavailable")?
                    .sensor_signal,
                aim_adjustment: 0,
            }
            .eligible(
                terrain,
                distance,
                electronic_field(world, observer)?.disturbed,
                disabled,
                super::sensors::sensor_maximum(world, observer, 24),
            ),
            // These devices require a unit signature and cannot observe an empty terrain target.
            _ => false,
        };
        if index == 0 {
            sensors.primary = eligible;
        } else {
            sensors.secondary = eligible;
        }
    }
    Ok(HexObservation {
        visible: sensors.primary || sensors.secondary,
        sensors,
    })
}
