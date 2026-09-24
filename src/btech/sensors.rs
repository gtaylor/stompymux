//! Sensor eligibility, shared range policy and aim contributions, separate from acquisition rolls.
use super::BattleTerrainLos;
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Optical modes with full-circle coverage on conventional BattleMechs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleSensorMode {
    Visual,
    LightAmplification,
    Infrared,
    Seismic,
    Electromagnetic,
    Radar,
    BeagleProbe,
    LightProbe,
    BloodhoundProbe,
}

/// Battlefield illumination levels used by optical range and aim rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleLight {
    Night,
    Twilight,
    Day,
}

impl std::str::FromStr for BattleLight {
    type Err = anyhow::Error;

    /// Parse named light levels used by operator commands and Lua.
    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "night" => Ok(Self::Night),
            "twilight" => Ok(Self::Twilight),
            "day" => Ok(Self::Day),
            _ => anyhow::bail!("Expected night, twilight or day"),
        }
    }
}

/// Explicit environment and illumination inputs until map conditions and lights are owned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleSensorConditions {
    pub light: BattleLight,
    /// Weather visibility in hexes, from zero through sixty.
    pub visibility: u8,
    pub disabled: bool,
    pub target_lit: bool,
}

/// A potential contact, not a detection result or a complete attack target number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleSensorReport {
    pub eligible: bool,
    /// Sensor-specific acquisition factor before pilot, arc and primary/secondary weighting.
    pub acquisition_factor: u8,
    /// Lighting, intervening/target woods and partial-cover contribution to weapon aim.
    pub aim_modifier: i16,
}

impl BattleSensorMode {
    /// Evaluate terrain and range without acquiring a contact; infrared assumes a cold target.
    /// World queries adjust this baseline using the target's continuous heat rates.
    pub fn evaluate(
        self,
        terrain: BattleTerrainLos,
        distance: f64,
        target_underwater: bool,
        conditions: BattleSensorConditions,
    ) -> Result<BattleSensorReport> {
        ensure!(
            !matches!(
                self,
                Self::Seismic
                    | Self::Electromagnetic
                    | Self::Radar
                    | Self::BeagleProbe
                    | Self::LightProbe
                    | Self::BloodhoundProbe
            ),
            "This sensor evaluation requires physical target facts"
        );
        self.evaluate_with_ceiling(
            terrain,
            distance,
            target_underwater,
            conditions,
            (u16::from(conditions.visibility) * 3).clamp(24, 60),
            15,
        )
    }

    /// Apply a persisted range ceiling while retaining the sixty-hex optical hardware limit.
    pub(super) fn evaluate_with_ceiling(
        self,
        terrain: BattleTerrainLos,
        distance: f64,
        target_underwater: bool,
        conditions: BattleSensorConditions,
        ceiling: u16,
        infrared_maximum: u16,
    ) -> Result<BattleSensorReport> {
        ensure!(
            distance.is_finite() && distance >= 0.0,
            "Invalid sensor distance"
        );
        ensure!(
            conditions.visibility <= 60,
            "Invalid battlefield visibility"
        );
        ensure!(
            terrain.woods <= 15 && terrain.target_woods <= 2 && terrain.water <= 7,
            "Invalid terrain LOS counts"
        );
        let darkness = match conditions.light {
            BattleLight::Night => 2,
            BattleLight::Twilight => 1,
            BattleLight::Day => 0,
        };
        let night = conditions.light == BattleLight::Night;
        let woods = terrain.woods + terrain.target_woods;
        let cover = if terrain.partial_cover { 3 } else { 0 };
        let aim_modifier = match self {
            Self::Visual => (if conditions.target_lit { 0 } else { darkness }) + woods + cover,
            Self::LightAmplification => darkness / 2 + woods * 3 / 2 + cover,
            Self::Infrared => woods * 4 / 3 + cover + 2,
            Self::Seismic
            | Self::Electromagnetic
            | Self::Radar
            | Self::BeagleProbe
            | Self::LightProbe
            | Self::BloodhoundProbe => {
                unreachable!("physical sensor queries use target facts")
            }
        };
        let visibility = u16::from(conditions.visibility);
        let range_limit = match self {
            Self::Visual => visibility * if night && conditions.target_lit { 3 } else { 1 },
            Self::LightAmplification => visibility * if night { 2 } else { 1 },
            Self::Infrared => infrared_maximum,
            Self::Seismic => 8,
            Self::Electromagnetic => 24,
            Self::Radar => 180,
            Self::BeagleProbe => 6,
            Self::LightProbe => 3,
            Self::BloodhoundProbe => 8,
        };
        let map_limit = ceiling.min(60);
        let eligible = !conditions.disabled
            && !terrain.blocked
            && !terrain.fire
            && (!terrain.smoke || self == Self::Infrared)
            && distance <= f64::from(range_limit.min(map_limit))
            && match self {
                Self::Visual => terrain.woods < 3 && (!target_underwater || terrain.water < 6),
                Self::LightAmplification => {
                    terrain.woods < 2 && terrain.water == 0 && !conditions.target_lit
                }
                Self::Infrared => terrain.woods < 6,
                Self::Seismic
                | Self::Electromagnetic
                | Self::Radar
                | Self::BeagleProbe
                | Self::LightProbe
                | Self::BloodhoundProbe => true,
            };
        let acquisition_factor = if !eligible {
            0
        } else {
            match self {
                Self::Visual => (100.0 - distance / 3.0) as u8,
                Self::LightAmplification => (70.0 - distance) as u8,
                Self::Infrared => (80.0 - distance) as u8,
                Self::Seismic => (50.0 - distance * 4.0) as u8,
                Self::Electromagnetic => (30.0 - distance) as u8,
                Self::Radar => (180.0 - distance).clamp(10.0, 90.0) as u8,
                Self::BeagleProbe | Self::LightProbe | Self::BloodhoundProbe => 101,
            }
        };
        Ok(BattleSensorReport {
            eligible,
            acquisition_factor,
            aim_modifier: i16::from(aim_modifier),
        })
    }
}

/// Compose current placement, terrain and range for a read-only optical sensor query.
/// Target illumination and map conditions are explicit; this does not choose installed sensors.
pub fn optical_contact(
    world: &World,
    observer: ObjectId,
    target: ObjectId,
    sensor: BattleSensorMode,
    conditions: BattleSensorConditions,
) -> Result<BattleSensorReport> {
    let report = evaluate_optical_contact(world, observer, target, sensor, conditions)?;
    Ok(super::visibility::sensor_report(world, target, report))
}

/// Evaluate equipment and geometry before applying the common target visibility policy.
fn evaluate_optical_contact(
    world: &World,
    observer: ObjectId,
    target: ObjectId,
    sensor: BattleSensorMode,
    conditions: BattleSensorConditions,
) -> Result<BattleSensorReport> {
    if super::clouds::blocks_contact(world, observer, target, sensor)? {
        return Ok(BattleSensorReport {
            eligible: false,
            acquisition_factor: 0,
            aim_modifier: 0,
        });
    }
    if let Some(probe) = sensor.active_probe() {
        let mut report = super::active_probe_contact(world, observer, target, probe)?;
        if conditions.disabled {
            report.eligible = false;
            report.acquisition_factor = 0;
        }
        return Ok(report);
    }

    if sensor == BattleSensorMode::Radar {
        let mut report = super::radar_contact(world, observer, target)?;
        if conditions.disabled {
            report.eligible = false;
            report.acquisition_factor = 0;
        }
        return Ok(report);
    }

    if sensor == BattleSensorMode::Electromagnetic {
        let mut report = super::electromagnetic_contact(
            world,
            observer,
            target,
            super::BattleElectromagneticRules {
                signal_strength: super::scanner::scanner_unit(world, observer)
                    .context("Observer is unavailable")?
                    .sensor_signal,
                aim_adjustment: 0,
            },
        )?;
        if conditions.disabled {
            report.eligible = false;
            report.acquisition_factor = 0;
        }
        return Ok(report);
    }

    if sensor == BattleSensorMode::Seismic {
        let mut report = super::seismic_contact(
            world,
            observer,
            target,
            super::BattleSeismicRules {
                detect_stopped: world.btech.seismic_detect_stopped,
                signal_strength: super::scanner::scanner_unit(world, observer)
                    .context("Observer is unavailable")?
                    .sensor_signal,
                aim_adjustment: 0,
            },
        )?;
        if conditions.disabled {
            report.eligible = false;
            report.acquisition_factor = 0;
        }
        return Ok(report);
    }

    let (terrain, range) = super::los::unit_terrain_geometry(world, observer, target)?;
    let sight = super::los::unit_sight_point(world, target)?;
    let underwater = sight.below_waterline();
    let mut report = sensor.evaluate_with_ceiling(
        terrain,
        range.spatial,
        underwater,
        conditions,
        (u16::from(conditions.visibility) * 3).clamp(24, 60),
        super::sensors::sensor_maximum(world, observer, 15),
    )?;
    if matches!(
        sensor,
        BattleSensorMode::Visual | BattleSensorMode::LightAmplification
    ) {
        report.aim_modifier +=
            super::hull_down::cover_modifier(world, target, terrain.partial_cover);
    }
    if sensor == BattleSensorMode::Infrared {
        report.aim_modifier += infrared_heat_modifier(infrared_target_rates(world, target)?)? - 2;
    }
    Ok(report)
}

/// Query saved battlefield weather and light; illumination and disabled status are explicit.
pub fn map_optical_contact(
    world: &World,
    observer: ObjectId,
    target: ObjectId,
    sensor: BattleSensorMode,
    target_lit: bool,
    disabled: bool,
) -> Result<BattleSensorReport> {
    map_optical_contact_with_geometry(world, observer, target, sensor, target_lit, disabled, None)
}

/// Reuse immutable pair geometry within one observation. Physical sensor modes
/// still use their ordinary specialized admission, and visibility is always live.
pub(super) fn map_optical_contact_with_geometry(
    world: &World,
    observer: ObjectId,
    target: ObjectId,
    sensor: BattleSensorMode,
    target_lit: bool,
    disabled: bool,
    geometry: Option<(super::BattleTerrainLos, super::BattleRange)>,
) -> Result<BattleSensorReport> {
    map_optical_contact_prepared(
        world, observer, target, sensor, target_lit, disabled, geometry, None,
    )
}

/// Observation-local preparation shares lighting while retaining every live sensor gate.
pub(super) fn map_optical_contact_prepared(
    world: &World,
    observer: ObjectId,
    target: ObjectId,
    sensor: BattleSensorMode,
    target_lit: bool,
    disabled: bool,
    geometry: Option<(super::BattleTerrainLos, super::BattleRange)>,
    illumination: Option<&super::searchlight::IlluminationContext<'_>>,
) -> Result<BattleSensorReport> {
    let _measurement = crate::btech::autopilot::diagnostics::measure(
        crate::btech::autopilot::diagnostics::Category::Sensors,
    );
    let report = evaluate_map_optical_contact(
        world,
        observer,
        target,
        sensor,
        target_lit,
        disabled,
        geometry,
        illumination,
    )?;
    Ok(super::visibility::sensor_report(world, target, report))
}

/// Evaluate equipment and geometry before applying the common target visibility policy.
fn evaluate_map_optical_contact(
    world: &World,
    observer: ObjectId,
    target: ObjectId,
    sensor: BattleSensorMode,
    target_lit: bool,
    disabled: bool,
    geometry: Option<(super::BattleTerrainLos, super::BattleRange)>,
    illumination: Option<&super::searchlight::IlluminationContext<'_>>,
) -> Result<BattleSensorReport> {
    if super::clouds::blocks_contact(world, observer, target, sensor)? {
        return Ok(BattleSensorReport {
            eligible: false,
            acquisition_factor: 0,
            aim_modifier: 0,
        });
    }
    if let Some(probe) = sensor.active_probe() {
        let mut report = super::active_probe_contact(world, observer, target, probe)?;
        if disabled {
            report.eligible = false;
            report.acquisition_factor = 0;
        }
        return Ok(report);
    }

    if sensor == BattleSensorMode::Radar {
        let mut report = super::radar_contact(world, observer, target)?;
        if disabled {
            report.eligible = false;
            report.acquisition_factor = 0;
        }
        return Ok(report);
    }

    if sensor == BattleSensorMode::Electromagnetic {
        let mut report = super::electromagnetic_contact(
            world,
            observer,
            target,
            super::BattleElectromagneticRules {
                signal_strength: super::scanner::scanner_unit(world, observer)
                    .context("Observer is unavailable")?
                    .sensor_signal,
                aim_adjustment: 0,
            },
        )?;
        if disabled {
            report.eligible = false;
            report.acquisition_factor = 0;
        }
        return Ok(report);
    }

    if sensor == BattleSensorMode::Seismic {
        let mut report = super::seismic_contact(
            world,
            observer,
            target,
            super::BattleSeismicRules {
                detect_stopped: world.btech.seismic_detect_stopped,
                signal_strength: super::scanner::scanner_unit(world, observer)
                    .context("Observer is unavailable")?
                    .sensor_signal,
                aim_adjustment: 0,
            },
        )?;
        if disabled {
            report.eligible = false;
            report.acquisition_factor = 0;
        }
        return Ok(report);
    }

    let (terrain, range) = match geometry {
        Some(geometry) => geometry,
        None => super::los::unit_terrain_geometry(world, observer, target)?,
    };
    let sight = super::los::unit_sight_point(world, target)?;
    let position = sight.position;
    let map = &world.btech.maps()[&position.map];
    let light = match map.light {
        0 => BattleLight::Night,
        1 => BattleLight::Twilight,
        2 => BattleLight::Day,
        _ => anyhow::bail!("Invalid battlefield light"),
    };
    let mut report = sensor.evaluate_with_ceiling(
        terrain,
        range.spatial,
        sight.below_waterline(),
        BattleSensorConditions {
            light,
            visibility: u8::try_from(map.visibility)?,
            target_lit: target_lit
                || illumination.map_or_else(
                    || super::unit_illuminated(world, target),
                    |context| context.illuminated(target),
                ),
            disabled: disabled || map.optical_sensor_disabled(sensor),
        },
        u16::try_from(map.maximum_visibility)?,
        super::sensors::sensor_maximum(world, observer, 15),
    )?;
    if matches!(
        sensor,
        BattleSensorMode::Visual | BattleSensorMode::LightAmplification
    ) {
        report.aim_modifier +=
            super::hull_down::cover_modifier(world, target, terrain.partial_cover);
    }
    if sensor == BattleSensorMode::Infrared {
        report.aim_modifier += infrared_heat_modifier(infrared_target_rates(world, target)?)? - 2;
    }
    Ok(report)
}

/// Vehicles do not run the thermal-production cycle; weapon expenditure is a separate field.
fn infrared_target_rates(world: &World, target: ObjectId) -> Result<super::BattleHeatRates> {
    if world.btech.vehicles().contains_key(&target) {
        return Ok(super::BattleHeatRates {
            production: 0.0,
            dissipation: 0.0,
        });
    }
    Ok(world
        .btech
        .constructed_units()
        .get(&target)
        .context("Target is not constructed")?
        .infrared_heat_rates(world))
}

/// Heat contrast uses total production, including weapon heat, and current cooling.
pub fn infrared_heat_modifier(rates: super::BattleHeatRates) -> Result<i16> {
    ensure!(
        rates.production.is_finite() && rates.dissipation.is_finite(),
        "Invalid thermal signature"
    );
    let signature =
        2.0 * (rates.production - rates.dissipation) + rates.production.min(rates.dissipation);
    Ok(if signature <= 0.0 {
        2
    } else if signature > 50.0 {
        -2
    } else if signature > 35.0 {
        -1
    } else if signature > 20.0 {
        0
    } else {
        1
    })
}

impl super::BattleUnit {
    /// Infrared includes weapon heat in production before comparing it with cooling.
    pub fn infrared_heat_rates(&self, world: &World) -> super::BattleHeatRates {
        let mut rates = self.heat_rates(world);
        rates.production += self.heat().stored;
        rates
    }
}

/// Fixed installations extend non-optical sensor reach by forty percent before fluctuation.
pub(super) fn sensor_maximum(world: &World, observer: ObjectId, ordinary: u16) -> u16 {
    if world
        .btech
        .vehicles()
        .get(&observer)
        .is_some_and(|unit| unit.definition().movement == super::BattleVehicleMovement::Stationary)
    {
        return ordinary * 140 / 100;
    }
    ordinary
}

/// The signal expands the guaranteed band, clamped to the installed hardware maximum.
pub(super) fn signal_reach(maximum: u16, variation: u16, signal: u8) -> u16 {
    (maximum.saturating_sub(variation) + u16::from(signal) * (variation + 1) / 100).min(maximum)
}

#[cfg(test)]
mod tests {
    use super::*;
    const NIGHT: BattleSensorConditions = BattleSensorConditions {
        light: BattleLight::Night,
        visibility: 10,
        disabled: false,
        target_lit: false,
    };

    #[test]
    fn darkness_illumination_and_weather_bound_ranges() {
        let clear = BattleTerrainLos::default();
        let evaluate = |sensor: BattleSensorMode, distance, conditions| {
            sensor.evaluate(clear, distance, false, conditions).unwrap()
        };
        assert!(evaluate(BattleSensorMode::Visual, 10.0, NIGHT).eligible);
        assert!(!evaluate(BattleSensorMode::Visual, 10.001, NIGHT).eligible);
        assert!(evaluate(BattleSensorMode::LightAmplification, 20.0, NIGHT).eligible);
        let lit = BattleSensorConditions {
            target_lit: true,
            ..NIGHT
        };
        assert!(evaluate(BattleSensorMode::Visual, 30.0, lit).eligible);
        assert!(!evaluate(BattleSensorMode::Visual, 30.001, lit).eligible);
        assert!(!evaluate(BattleSensorMode::LightAmplification, 1.0, lit).eligible);
        assert_eq!(
            evaluate(BattleSensorMode::Visual, 3.0, NIGHT).acquisition_factor,
            99
        );
        assert_eq!(
            evaluate(BattleSensorMode::LightAmplification, 3.0, NIGHT).acquisition_factor,
            67
        );
        let day = BattleSensorConditions {
            light: BattleLight::Day,
            ..lit
        };
        assert!(!evaluate(BattleSensorMode::Visual, 11.0, day).eligible);
        assert!(
            !evaluate(
                BattleSensorMode::Visual,
                61.0,
                BattleSensorConditions {
                    visibility: 60,
                    ..lit
                }
            )
            .eligible
        );
    }

    #[test]
    fn obscurants_and_water_have_sensor_specific_thresholds() {
        let mut terrain = BattleTerrainLos {
            woods: 2,
            ..BattleTerrainLos::default()
        };
        assert!(
            BattleSensorMode::Visual
                .evaluate(terrain, 1.0, false, NIGHT)
                .unwrap()
                .eligible
        );
        assert!(
            !BattleSensorMode::LightAmplification
                .evaluate(terrain, 1.0, false, NIGHT)
                .unwrap()
                .eligible
        );
        terrain.woods = 3;
        assert!(
            !BattleSensorMode::Visual
                .evaluate(terrain, 1.0, false, NIGHT)
                .unwrap()
                .eligible
        );
        terrain.woods = 0;
        terrain.water = 5;
        assert!(
            BattleSensorMode::Visual
                .evaluate(terrain, 1.0, true, NIGHT)
                .unwrap()
                .eligible
        );
        terrain.water = 6;
        assert!(
            !BattleSensorMode::Visual
                .evaluate(terrain, 1.0, true, NIGHT)
                .unwrap()
                .eligible
        );
        assert!(
            BattleSensorMode::Visual
                .evaluate(terrain, 1.0, false, NIGHT)
                .unwrap()
                .eligible
        );
        for terrain in [
            BattleTerrainLos {
                smoke: true,
                ..Default::default()
            },
            BattleTerrainLos {
                fire: true,
                ..Default::default()
            },
            BattleTerrainLos {
                blocked: true,
                ..Default::default()
            },
        ] {
            for sensor in [
                BattleSensorMode::Visual,
                BattleSensorMode::LightAmplification,
            ] {
                assert_eq!(
                    sensor
                        .evaluate(terrain, 0.0, false, NIGHT)
                        .unwrap()
                        .acquisition_factor,
                    0
                );
            }
        }
    }

    #[test]
    fn aim_counts_target_woods_cover_and_integer_light_amplification_penalties() {
        let terrain = BattleTerrainLos {
            woods: 1,
            target_woods: 2,
            partial_cover: true,
            ..Default::default()
        };
        let visual = BattleSensorMode::Visual
            .evaluate(terrain, 1.0, false, NIGHT)
            .unwrap();
        assert!(visual.eligible);
        assert_eq!(visual.aim_modifier, 8);
        assert_eq!(
            BattleSensorMode::LightAmplification
                .evaluate(terrain, 1.0, false, NIGHT)
                .unwrap()
                .aim_modifier,
            8
        );
        let twilight = BattleSensorConditions {
            light: BattleLight::Twilight,
            ..NIGHT
        };
        assert_eq!(
            BattleSensorMode::LightAmplification
                .evaluate(terrain, 1.0, false, twilight)
                .unwrap()
                .aim_modifier,
            7
        );
        assert!(
            BattleSensorMode::Visual
                .evaluate(terrain, f64::NAN, false, NIGHT)
                .is_err()
        );
        assert!(
            BattleSensorMode::Visual
                .evaluate(
                    terrain,
                    1.0,
                    false,
                    BattleSensorConditions {
                        visibility: 61,
                        ..NIGHT
                    }
                )
                .is_err()
        );
    }
}
