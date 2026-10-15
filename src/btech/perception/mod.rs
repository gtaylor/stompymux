//! Automatic perception, after Total Warfare's line of sight and the double-blind rules of
//! Tactical Operations (pp. 188–191), adapted to realtime play.
//!
//! * **Sight** needs a clear line and reaches as far as the light and weather allow: sixty
//!   hexes in daylight, three in pitch black. At night a unit's own searchlight stretches its
//!   sight to thirty hexes and a target lit by anything else is seen at forty-five. Smoke
//!   along the line shortens sight by its density. A sighted unit is identified.
//! * **Sensors** are one selected system (infrared, magscan, radar, seismic or an active
//!   probe) swept with 2d6 once a turn; the sweep's result picks the range band that turn's
//!   sweep finds targets in. Sensors do not need a clear line, though hills stop infrared and
//!   radar, buildings stop radar and magscan, and nothing but magscan crosses the water's
//!   surface. A sensor contact is a blip: a position and height, not an identity.
//! * A **hidden** unit is perceived by no enemy until something reveals it.
//!
//! Darkness and weather cost to hit through the map's conditions, not here; cover along the
//! line and in the target's hex is reported as the perception's aim modifier.

mod probe;
mod report;
mod sensors;
mod sight;

pub use probe::ActiveProbe;
pub use report::{PerceptionReport, perception_report};
pub use sensors::{
    SensorBand, SensorBands, SensorMode, SweepResult, SweepTarget, TargetSystems, ecm_modifier,
    sweep,
};

use crate::btech::{
    Ground, HexCoordinate, Light, Power, Range, StoredMap, System, TerrainLos, VehicleMovement,
    Weather,
};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sight::SightFacts;

/// How a target is perceived. Declaration order breaks ties: sight identifies, so it wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionChannel {
    Sight,
    Sensors,
    Probe,
}

impl DetectionChannel {
    /// Every channel, in tie-break order.
    pub const ALL: [Self; 3] = [Self::Sight, Self::Sensors, Self::Probe];

    /// Stable name shared by serialization and Lua constants.
    pub fn name(self) -> &'static str {
        match self {
            Self::Sight => "sight",
            Self::Sensors => "sensors",
            Self::Probe => "probe",
        }
    }

    /// One-letter contact-row code; a blip that is not identified uses lowercase.
    pub fn code(self, identified: bool) -> char {
        let code = match self {
            Self::Sight => 'V',
            Self::Sensors => 'S',
            Self::Probe => 'P',
        };
        if identified {
            code
        } else {
            code.to_ascii_lowercase()
        }
    }
}

/// A current detection of one target by one observer.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Perception {
    pub channel: DetectionChannel,
    /// The target is seen, so its identity and condition are known; sensor blips are not.
    pub identified: bool,
    /// Terrain leaves a line of fire; a blip behind a hill can only be engaged indirectly.
    pub line_of_fire: bool,
    /// Perception's contribution to weapon aim: woods, smoke and crops along the line and in
    /// the target's hex, partial cover, and hull-down posture behind it.
    pub aim_modifier: i16,
    pub range: Range,
    /// What this turn's sensor sweep made of the target, when sensors were tried.
    pub sweep: Option<SweepResult>,
}

/// Condition of one perception system on the observing unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PerceptionStatus {
    /// Working.
    Ready,
    /// Working while hostile ECM adds to its sweeps.
    Jammed,
    /// Destroyed or otherwise failed equipment.
    Damaged,
    /// Switched off for this battlefield by its operators.
    Disabled,
    /// The unit carries no such equipment.
    Absent,
}

/// The selected sensor system and its current sweep.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SensorProfile {
    /// Every sensor system the unit sweeps with this turn and its brackets on this chassis,
    /// in the order they are tried: a working active probe, then the built-in infrared,
    /// magscan, radar and seismic sensors.
    pub systems: Vec<SensorSystem>,
    /// Condition of the built-in sensors.
    pub status: PerceptionStatus,
    /// Hostile ECM covers the unit, so its radar and probe sweeps take the jamming modifier.
    pub jammed: bool,
    /// This turn's 2d6 sweep, once rolled.
    pub sweep: Option<u8>,
    /// The band the unmodified sweep detects in.
    pub band: Option<SensorBand>,
    /// The installed probe the probe sweep uses.
    pub probe: Option<ActiveProbe>,
}

/// One sensor system of a unit's suite and the brackets it detects in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SensorSystem {
    pub mode: SensorMode,
    pub bands: SensorBands,
}

/// The best installed active probe and its standard-rules reach for finding hidden units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ProbeProfile {
    pub kind: ActiveProbe,
    /// Hexes within which the probe reveals a hidden unit it has line of sight to (Total
    /// Warfare p. 129).
    pub range: u16,
    pub status: PerceptionStatus,
}

/// Everything an observer can currently perceive with, computed once per observation.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PerceptionProfile {
    pub light: Light,
    pub weather: Weather,
    /// How far the observer sees under the light and weather, stretched to thirty hexes by
    /// its own searchlight at night.
    pub sight_range: u16,
    /// How far the observer sees a target lit by another source at night.
    pub lit_sight_range: u16,
    pub sensor: SensorProfile,
    pub probe: Option<ProbeProfile>,
    #[serde(skip)]
    pub(crate) clairvoyant: bool,
    #[serde(skip)]
    cloud_base: i16,
    #[serde(skip)]
    level: i32,
    #[serde(skip)]
    team: i32,
    #[serde(skip)]
    vehicle: bool,
    #[serde(skip)]
    underwater: bool,
    /// Hostile ECM covers the observer: any suite, and an Angel suite.
    #[serde(skip)]
    disturbed: (bool, bool),
}

impl PerceptionProfile {
    /// The probe that can currently reveal hidden units, if any.
    pub(super) fn ready_probe(&self) -> Option<ProbeProfile> {
        self.probe
            .filter(|probe| probe.status == PerceptionStatus::Ready)
    }

    /// The farthest hex any channel could reach this turn: sight of a lit target, or the
    /// sensor's long band stretched by the most heat and mass can add. Nothing beyond it needs
    /// its terrain traced.
    pub(super) fn reach(&self) -> u64 {
        let sensors = if self.sensor.sweep.is_some() {
            self.sensor
                .systems
                .iter()
                .map(|system| u64::from(system.bands.long) + 8)
                .max()
                .unwrap_or(0)
        } else {
            0
        };
        u64::from(self.sight_range.max(self.lit_sight_range)).max(sensors)
    }
}

/// Battlefield switches that turn off one perception channel for every unit on the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MapPerceptionFlag {
    Sensors,
    Radar,
    Probes,
}

impl MapPerceptionFlag {
    /// Every perception switch, in bit order.
    pub const ALL: [Self; 3] = [Self::Sensors, Self::Radar, Self::Probes];

    /// Persisted `sensor_flags` bit. Positions match the maps shared with the reference server.
    pub fn bit(self) -> i64 {
        match self {
            Self::Sensors => 1,
            Self::Radar => 32,
            Self::Probes => 64,
        }
    }

    /// Operator-facing spelling used by `@SETMAP sensorflags` and `@VIEWMAP`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Sensors => "sensors",
            Self::Radar => "radar",
            Self::Probes => "probes",
        }
    }
}

/// Parse a whitespace- or comma-separated list of disabled perception channels; `-` means none.
pub fn parse_perception_flags(value: &str) -> Result<i64> {
    let value = value.trim();
    if value == "-" {
        return Ok(0);
    }
    value
        .split([' ', '\t', ','])
        .filter(|name| !name.is_empty())
        .try_fold(0, |flags, name| {
            let flag = MapPerceptionFlag::ALL
                .into_iter()
                .find(|flag| flag.name().eq_ignore_ascii_case(name))
                .with_context(|| format!("Unknown sensor flag {name:?}"))?;
            Ok(flags | flag.bit())
        })
}

/// Display the disabled perception channels in `flags`; `-` when none are disabled.
pub fn format_perception_flags(flags: i64) -> String {
    let names: Vec<_> = MapPerceptionFlag::ALL
        .into_iter()
        .filter(|flag| flags & flag.bit() != 0)
        .map(MapPerceptionFlag::name)
        .collect();
    if names.is_empty() {
        return "-".into();
    }
    names.join(" ")
}

impl StoredMap {
    /// Whether operators switched this perception channel off for the battlefield.
    pub fn perception_disabled(&self, flag: MapPerceptionFlag) -> bool {
        self.sensor_flags & flag.bit() != 0
    }
}

/// Enable or disable one perception channel on a battlefield without touching other flags.
pub fn set_map_perception(
    world: &mut World,
    map: ObjectId,
    flag: MapPerceptionFlag,
    enabled: bool,
) -> Result<()> {
    ensure!(
        world
            .objects
            .get(&map)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Map is unavailable"
    );
    let stored = world.btech.maps.get_mut(&map).context("Map not found")?;
    if enabled {
        stored.sensor_flags &= !flag.bit();
    } else {
        stored.sensor_flags |= flag.bit();
    }
    Ok(())
}

/// Summarize an observer's current reach without tracing any target.
pub fn perception_profile(world: &World, observer: ObjectId) -> Result<PerceptionProfile> {
    let unit = crate::btech::scanner::scanner_unit(world, observer)
        .context("Observer is not constructed")?;
    let position = unit.position.context("Observer is not placed")?;
    let map = world
        .btech
        .maps()
        .get(&position.map)
        .context("Map not found")?;
    let light = map.light_level()?;
    let weather = map.weather_condition()?;
    let visual = u16::from(map.visual_range()?);
    let searchlight_on = crate::btech::searchlight::lamp_on(world, observer);
    let sight_range = if light.searchlights_help_sight() && searchlight_on {
        visual.max(u16::from(crate::btech::SEARCHLIGHT_SIGHT))
    } else {
        visual
    };
    let lit_sight_range = if light.searchlights_help_sight() {
        sight_range.max(u16::from(crate::btech::LIT_TARGET_SIGHT))
    } else {
        sight_range
    };
    let field = crate::btech::electronic_field(world, observer)?;
    let probe = probe_profile(world, observer, map)?;
    let sensor = sensor_profile(world, observer, &unit, map, probe, field.disturbed)?;
    let point = crate::btech::los::unit_sight_point(world, observer)?;
    Ok(PerceptionProfile {
        light,
        weather,
        sight_range,
        lit_sight_range,
        sensor,
        probe,
        clairvoyant: unit.visibility.clairvoyant,
        cloud_base: map.cloud_base,
        level: point.level,
        team: unit.signature.team,
        vehicle: unit.vehicle,
        underwater: point.below_waterline(),
        disturbed: (field.disturbed, field.angel_disturbed),
    })
}

/// Resolve the selected sensor from equipment, damage, jamming and map switches.
fn sensor_profile(
    world: &World,
    observer: ObjectId,
    unit: &crate::btech::scanner::ScannerUnit<'_>,
    map: &StoredMap,
    probe: Option<ProbeProfile>,
    jammed: bool,
) -> Result<SensorProfile> {
    let (clan, hits) = match world.btech.constructed_units().get(&observer) {
        Some(mech) => (
            mech.definition().has_special("Clan"),
            mech.system_hits(System::Sensors),
        ),
        None => (
            world.btech.vehicles()[&observer]
                .definition()
                .has_special("Clan"),
            0,
        ),
    };
    let status = if map.perception_disabled(MapPerceptionFlag::Sensors) {
        PerceptionStatus::Disabled
    } else if hits >= 2 {
        PerceptionStatus::Damaged
    } else {
        PerceptionStatus::Ready
    };
    let probe_kind = probe.map(|probe| probe.kind);
    let mut systems = Vec::new();
    // A working probe sweeps first, with its own brackets, so a target it finds shows as a
    // probe contact; the built-in sensors follow.
    if probe.is_some_and(|probe| probe.status == PerceptionStatus::Ready) {
        systems.push(SensorSystem {
            mode: SensorMode::Probe,
            bands: SensorBands::of(SensorMode::Probe, unit.vehicle, probe_kind, clan),
        });
    }
    if status == PerceptionStatus::Ready {
        for mode in [
            SensorMode::Infrared,
            SensorMode::Magscan,
            SensorMode::Radar,
            SensorMode::Seismic,
        ] {
            if mode == SensorMode::Radar && map.perception_disabled(MapPerceptionFlag::Radar) {
                continue;
            }
            systems.push(SensorSystem {
                mode,
                bands: SensorBands::of(mode, unit.vehicle, None, clan),
            });
        }
    }
    Ok(SensorProfile {
        systems,
        status,
        jammed,
        sweep: unit.sensor_sweep,
        band: unit
            .sensor_sweep
            .and_then(|roll| SensorBand::of_result(i16::from(roll))),
        probe: probe_kind,
    })
}

/// Pick the working probe with the longest reach, or else the best installed one as damaged.
fn probe_profile(
    world: &World,
    observer: ObjectId,
    map: &StoredMap,
) -> Result<Option<ProbeProfile>> {
    // Build the equipment projection once and check every family against it.
    let (clan, fittings) = match world.btech.vehicles().get(&observer) {
        Some(vehicle) => {
            let loadout = vehicle.loadout()?;
            (
                vehicle.definition().has_special("Clan"),
                ActiveProbe::BY_REACH.map(|kind| (kind, vehicle.probe_fitting(&loadout, kind))),
            )
        }
        None => {
            let unit = world
                .btech
                .constructed_units()
                .get(&observer)
                .context("Observer is not constructed")?;
            let loadout = unit.loadout()?;
            (
                unit.definition().has_special("Clan"),
                ActiveProbe::BY_REACH.map(|kind| (kind, unit.probe_fitting(&loadout, kind))),
            )
        }
    };
    let mut damaged = None;
    for (kind, fitting) in fittings {
        if !fitting.installed {
            continue;
        }
        let range = u16::from(kind.range(clan));
        if !fitting.available {
            damaged.get_or_insert(ProbeProfile {
                kind,
                range,
                status: PerceptionStatus::Damaged,
            });
            continue;
        }
        let status = if map.perception_disabled(MapPerceptionFlag::Probes) {
            PerceptionStatus::Disabled
        } else {
            PerceptionStatus::Ready
        };
        return Ok(Some(ProbeProfile {
            kind,
            range,
            status,
        }));
    }
    Ok(damaged)
}

/// Perceive one target with a freshly computed observer profile.
pub fn perceive(world: &World, observer: ObjectId, target: ObjectId) -> Result<Option<Perception>> {
    let profile = perception_profile(world, observer)?;
    perceive_prepared(world, observer, &profile, target, None, None)
}

/// Cover an attack on `target` must clear, from the terrain already traced between the
/// units: woods, smoke and crops along the line and in the target's hex, partial cover and
/// hull-down posture behind it, less the woods a probe within reach lets the shooter see
/// through (Tactical Operations p. 97).
pub(crate) fn cover_aim(
    world: &World,
    profile: &PerceptionProfile,
    target: ObjectId,
    terrain: TerrainLos,
    range: Range,
) -> i16 {
    let hull_down = crate::btech::hull_down::cover_modifier(world, target, terrain.partial_cover);
    let cover = sight::cover(terrain, hull_down);
    let woods = i16::from(terrain.woods) + i16::from(terrain.target_woods);
    let probe_helps = profile
        .ready_probe()
        .is_some_and(|probe| !terrain.blocked && range.hex_distance <= u64::from(probe.range));
    if probe_helps && woods > 0 {
        cover - 1
    } else {
        cover
    }
}

/// Perceive one target, reusing an observation's profile, pair geometry and lighting cache.
///
/// Nothing here survives the immutable borrow: callers must not reuse the result after a
/// world mutation such as movement or a shot.
pub(crate) fn perceive_prepared(
    world: &World,
    observer: ObjectId,
    profile: &PerceptionProfile,
    target: ObjectId,
    geometry: Option<(TerrainLos, Range)>,
    illumination: Option<&crate::btech::searchlight::IlluminationContext<'_>>,
) -> Result<Option<Perception>> {
    let _measurement = crate::btech::autopilot::diagnostics::measure(
        crate::btech::autopilot::diagnostics::Category::Sensors,
    );
    ensure!(observer != target, "A unit cannot perceive itself");
    let other =
        crate::btech::scanner::scanner_unit(world, target).context("Target is not constructed")?;
    if other.visibility.invisible {
        return Ok(None);
    }
    // A hidden unit stays unseen by the enemy until something reveals it (Total Warfare
    // p. 259); its own side always knows where it is.
    if other.signature.hidden && other.signature.team != profile.team {
        return Ok(None);
    }
    let (terrain, range) = match geometry {
        Some(geometry) => geometry,
        None => {
            // Too far for every channel: skip the terrain trace, the costly part of a sweep
            // over a crowded battlefield.
            if crate::btech::unit_range(world, observer, target)?.hex_distance > profile.reach() {
                return Ok(None);
            }
            crate::btech::los::unit_terrain_geometry(world, observer, target)?
        }
    };
    let point = crate::btech::los::unit_sight_point(world, target)?;
    let target_underwater = point.below_waterline();
    let lit = || -> Result<bool> {
        Ok(illumination.map_or_else(
            || crate::btech::unit_illuminated(world, target),
            |context| context.illuminated(target),
        ))
    };
    let facts = SightFacts {
        terrain,
        distance: range.spatial,
        target_underwater,
        crosses_clouds: crate::btech::clouds::crosses(
            profile.cloud_base,
            profile.level,
            point.level,
        ),
        light: profile.light,
        sight_range: profile.sight_range,
        lit_sight_range: profile.lit_sight_range,
    };
    let aim_modifier = cover_aim(world, profile, target, terrain, range);
    let mut channel = None;
    let mut sweep = None;
    if sight::sees(&facts, lit)? {
        channel = Some(DetectionChannel::Sight);
    } else if let Some(roll) = profile.sensor.sweep {
        sweep = sweep_suite(
            world,
            profile,
            target,
            &other,
            &terrain,
            range,
            target_underwater,
            roll,
        )?;
        if let Some(result) = sweep.filter(|result| result.detected) {
            channel = Some(if result.mode == SensorMode::Probe {
                DetectionChannel::Probe
            } else {
                DetectionChannel::Sensors
            });
        }
    }
    Ok(channel.map(|channel| Perception {
        channel,
        identified: channel == DetectionChannel::Sight,
        line_of_fire: !terrain.blocked,
        aim_modifier,
        range,
        sweep,
    }))
}

/// Try this turn's roll against every system of the observer's suite. The first system to
/// find the target wins; otherwise a jammed sweep is reported over a plain miss, and `None`
/// means the suite has no system to sweep with.
#[allow(clippy::too_many_arguments)]
fn sweep_suite(
    world: &World,
    profile: &PerceptionProfile,
    target: ObjectId,
    other: &crate::btech::scanner::ScannerUnit<'_>,
    terrain: &TerrainLos,
    range: Range,
    target_underwater: bool,
    roll: u8,
) -> Result<Option<SweepResult>> {
    if profile.sensor.systems.is_empty() {
        return Ok(None);
    }
    let position = other.position.context("Target is not placed")?;
    let map = &world.btech.maps()[&position.map];
    let tile = map.base_hex(i64::from(position.x), i64::from(position.y))?;
    let burning_hex = map
        .decoration(HexCoordinate {
            x: i32::from(position.x),
            y: i32::from(position.y),
        })?
        .is_some_and(|effect| effect.kind == crate::btech::DecorationKind::Fire);
    let field = crate::btech::electronic_field(world, target)?;
    let (heat_penalties, tons, airborne, inferno, stealth, null_signature) =
        match world.btech.constructed_units().get(&target) {
            Some(mech) => (
                (mech.heat().excess / 5.0).floor().clamp(0.0, 5.0) as u8,
                u32::from(mech.definition().tons),
                mech.airborne(),
                false,
                mech.stealth().enabled,
                mech.null_signature().enabled,
            ),
            None => {
                let vehicle = &world.btech.vehicles()[&target];
                (
                    0,
                    u32::from(vehicle.definition().tons),
                    vehicle.definition().movement == VehicleMovement::Vtol
                        && vehicle.vtol_flight().is_some_and(|flight| {
                            flight.phase != crate::btech::VtolFlightPhase::Landed
                        }),
                    vehicle.inferno_remaining() > 0,
                    false,
                    false,
                )
            }
        };
    let systems = TargetSystems {
        angel: profile.disturbed.1 || field.angel_protected,
        guardian: (profile.disturbed.0 && !profile.disturbed.1)
            || (field.protected && !field.angel_protected),
        null_signature,
        stealth,
    };
    let sweep_target = SweepTarget {
        distance: range.hex_distance,
        terrain,
        across_water_surface: profile.underwater != target_underwater,
        heat_penalties,
        burning: inferno || burning_hex,
        tons,
        heavy_industrial: tile.ground() == Ground::HeavyIndustrial,
        moving_on_ground: other.speed.abs() > 0.0 && !airborne,
        systems,
    };
    let mut reported = None;
    for system in &profile.sensor.systems {
        let result = sweep(
            system.mode,
            system.bands,
            profile.vehicle,
            (system.mode == SensorMode::Probe)
                .then_some(profile.sensor.probe)
                .flatten(),
            roll,
            sweep_target,
        );
        if result.detected {
            return Ok(Some(result));
        }
        if reported.is_none_or(|previous: SweepResult| !previous.jammed) {
            reported = Some(result);
        }
    }
    Ok(reported)
}

/// Perceive an empty battlefield hex with a freshly computed observer profile.
pub fn hex_perception(
    world: &World,
    observer: ObjectId,
    target: HexCoordinate,
) -> Result<Option<DetectionChannel>> {
    let profile = perception_profile(world, observer)?;
    hex_perception_prepared(world, observer, &profile, target, true)
}

/// Resolve whether sight reaches a hex; sensors need a unit to find.
///
/// Clairvoyant observers see every hex. Stopped units see nothing unless `require_running`
/// is false, which lets callers inspect a parked unit's view.
pub(crate) fn hex_perception_prepared(
    world: &World,
    observer: ObjectId,
    profile: &PerceptionProfile,
    target: HexCoordinate,
    require_running: bool,
) -> Result<Option<DetectionChannel>> {
    ensure!(
        world
            .objects
            .get(&observer)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Observer is unavailable"
    );
    let unit = crate::btech::scanner::scanner_unit(world, observer)
        .context("Observer is not constructed")?;
    let position = unit.position.context("Observer is not placed")?;
    let (terrain, distance) = crate::btech::los::unit_hex_los(world, observer, target)?;
    if require_running && unit.power != Power::Running {
        return Ok(None);
    }
    if profile.clairvoyant {
        return Ok(Some(DetectionChannel::Sight));
    }
    let map = &world.btech.maps()[&position.map];
    let tile = map.base_hex(i64::from(target.x), i64::from(target.y))?;
    let facts = SightFacts {
        terrain,
        distance,
        target_underwater: false,
        crosses_clouds: crate::btech::clouds::crosses(
            profile.cloud_base,
            profile.level,
            i32::from(tile.standing_height()),
        ),
        light: profile.light,
        sight_range: profile.sight_range,
        lit_sight_range: profile.lit_sight_range,
    };
    let lit = || crate::btech::hex_illuminated(world, position.map, target);
    Ok(sight::sees(&facts, lit)?.then_some(DetectionChannel::Sight))
}

/// Roll one unit's sweep from its map's sensor stream.
pub(crate) fn sweep_unit(world: &mut World, id: ObjectId) -> Result<()> {
    let position = crate::btech::scanner::scanner_unit(world, id)
        .and_then(|unit| unit.position)
        .context("Unit is not placed")?;
    let map = world
        .btech
        .maps
        .get_mut(&position.map)
        .context("Map not found")?;
    let roll = map
        .sensor_dice
        .get_or_insert_with(crate::btech::Dice::fresh)
        .two_d6();
    crate::btech::with_unit_mut!(
        world.btech.unit_mut(id).context("Unit is unavailable")?,
        |unit| unit.sensor_sweep = Some(roll)
    );
    Ok(())
}

/// Sweep every running, placed unit at the start of a turn, as Tactical Operations rolls
/// sensors once a turn.
pub fn advance_sensor_sweeps(world: &mut World) -> Result<()> {
    if !world.btech.turn_clock.starts_turn() {
        return Ok(());
    }
    let ids: Vec<_> = crate::btech::scanner::scanner_ids(world)
        .into_iter()
        .filter(|&id| {
            crate::btech::scanner::scanner_unit(world, id)
                .is_some_and(|unit| unit.power == Power::Running && unit.position.is_some())
                && world
                    .objects
                    .get(&id)
                    .is_some_and(|object| !object.flags.contains(Flag::Going))
        })
        .collect();
    for id in ids {
        sweep_unit(world, id)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Channel codes, names and serialization stay aligned for rows and Lua.
    #[test]
    fn channel_codes_names_and_serialization_agree() {
        for channel in DetectionChannel::ALL {
            assert_eq!(
                serde_json::to_value(channel).unwrap(),
                serde_json::json!(channel.name())
            );
            assert!(channel.code(true).is_ascii_uppercase());
            assert_eq!(channel.code(false), channel.code(true).to_ascii_lowercase());
        }
        assert_eq!(
            DetectionChannel::ALL
                .map(|channel| channel.code(true))
                .iter()
                .collect::<String>(),
            "VSP"
        );
        let mut sorted = DetectionChannel::ALL;
        sorted.sort();
        assert_eq!(sorted, DetectionChannel::ALL);
    }

    /// Map switches use the reference bit positions and leave other bits alone.
    #[test]
    fn map_flags_use_reference_bits() {
        let mut map: StoredMap = serde_json::from_value(serde_json::json!({
            "name": "flags", "width": 1, "height": 1, "gravity": 100, "temperature": 20,
            "flags": 0, "light": 2, "weather": 0,
            "cloud_base": 0, "sensor_flags": 2
        }))
        .unwrap();
        assert!(!map.perception_disabled(MapPerceptionFlag::Sensors));
        map.sensor_flags |= MapPerceptionFlag::Probes.bit();
        assert!(map.perception_disabled(MapPerceptionFlag::Probes));
        assert!(!map.perception_disabled(MapPerceptionFlag::Radar));
        assert_eq!(map.sensor_flags, 66);
        assert_eq!(
            [
                MapPerceptionFlag::Sensors,
                MapPerceptionFlag::Radar,
                MapPerceptionFlag::Probes
            ]
            .map(MapPerceptionFlag::bit),
            [1, 32, 64]
        );
        assert_eq!(parse_perception_flags("radar, probes").unwrap(), 96);
        assert_eq!(format_perception_flags(97), "sensors radar probes");
        assert!(parse_perception_flags("sonar").is_err());
    }
}
