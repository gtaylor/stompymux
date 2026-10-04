//! Automatic perception: how one unit detects another.
//!
//! Every observer/target pair is traced once, and up to four channels may reach the target:
//!
//! * **Sensors**: every unit detects anything with a clear line inside a short band
//!   ([`DEFAULT_SENSOR_RANGE`] hexes unless configured), whatever the darkness or weather.
//! * **Sight**: beyond the band, weather visibility sets the reach. At night an unlit target
//!   costs +1 to hit, and an illuminated one can be seen three times as far.
//! * **Probe**: an active probe sees through terrain, woods, smoke and darkness within its
//!   radius and reveals hidden units. Behind blocking terrain the contact can be locked,
//!   spotted and shared, but not engaged with direct fire.
//! * **Radar**: anti-aircraft radar tracks airborne targets at long range.
//!
//! A clear line means no blocking terrain, fire, smoke, dense woods or cloud boundary between
//! the two units. Hostile ECM silences the sensor band and probes; stealth armor and null
//! signature systems hide a unit from enemy sensors and all probes but the Bloodhound. When
//! several channels reach a target, the one with the lowest aim modifier wins.

mod acquisition;
mod probe;
mod radar;
mod report;
mod sight;

pub(crate) use acquisition::acquire;
pub use acquisition::{
    AUTOMATIC_DETECTION_RANGE, BattleAcquisitionRules, BattleDetection, BattleSensorArc,
    HIDDEN_DETECTION_RANGE, perception_factor,
};
pub use probe::BattleActiveProbe;
pub use radar::{BattleRadarTarget, RADAR_RANGE};
pub use report::{BattlePerceptionReport, perception_report};

use crate::btech::{
    BattleLight, BattlePower, BattleRange, BattleSystem, BattleTerrainLos, BattleVehicleMovement,
    HexCoordinate, StoredMap,
};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sight::SightFacts;

/// Default reach of the all-conditions sensor band, in hexes.
pub const DEFAULT_SENSOR_RANGE: u16 = 15;

/// Host-configured sensor band reach, defaulting to [`DEFAULT_SENSOR_RANGE`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleSensorRange(pub u16);

impl Default for BattleSensorRange {
    fn default() -> Self {
        Self(DEFAULT_SENSOR_RANGE)
    }
}

/// Apply the host's sensor band reach; negative or oversized values are clamped to the map ceiling.
pub fn configure_perception(world: &mut World, sensor_range: i64) {
    world.btech.sensor_range = BattleSensorRange(sensor_range.clamp(0, 60) as u16);
}

/// How a target is perceived. Declaration order breaks ties between equal aim modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleDetectionChannel {
    Sensors,
    Sight,
    Radar,
    Probe,
}

impl BattleDetectionChannel {
    /// Every channel, in tie-break order.
    pub const ALL: [Self; 4] = [Self::Sensors, Self::Sight, Self::Radar, Self::Probe];

    /// Stable name shared by serialization and Lua constants.
    pub fn name(self) -> &'static str {
        match self {
            Self::Sensors => "sensors",
            Self::Sight => "sight",
            Self::Radar => "radar",
            Self::Probe => "probe",
        }
    }

    /// One-letter contact-row code. A contact behind blocking terrain uses lowercase.
    pub fn code(self, identified: bool) -> char {
        let code = match self {
            Self::Sensors => 'S',
            Self::Sight => 'V',
            Self::Radar => 'R',
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
pub struct BattlePerception {
    pub channel: BattleDetectionChannel,
    /// Terrain leaves a line of fire. Only probe contacts behind blocking terrain are unidentified.
    pub identified: bool,
    /// Perception's contribution to weapon aim: concealment, cover, darkness or radar tracking.
    pub aim_modifier: i16,
    /// An active probe reaches the target, which reveals hidden units.
    pub probed: bool,
    pub range: BattleRange,
}

/// Condition of one perception system on the observing unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattlePerceptionStatus {
    /// Working at full reach.
    Ready,
    /// Working at reduced reach after damage.
    Degraded,
    /// Silenced by hostile electronic countermeasures or the unit's own stealth systems.
    Jammed,
    /// Destroyed or otherwise failed equipment.
    Damaged,
    /// Switched off for this battlefield by its operators.
    Disabled,
    /// The unit carries no such equipment.
    Absent,
}

/// The best installed active probe and its current condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleProbeProfile {
    pub kind: BattleActiveProbe,
    /// Reach in hexes, including the stationary-installation bonus.
    pub range: u16,
    pub status: BattlePerceptionStatus,
}

/// Installed anti-aircraft radar and its current condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleRadarProfile {
    /// Reach in hexes; line of sight caps radar here on every chassis, fixed or mobile.
    pub range: u16,
    pub status: BattlePerceptionStatus,
}

/// Everything an observer can currently perceive with, computed once per observation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BattlePerceptionProfile {
    pub light: BattleLight,
    /// Weather visibility, capped by the battlefield ceiling.
    pub sight_range: u16,
    /// Reach to an illuminated target; three times sight at night, otherwise equal to it.
    pub lit_sight_range: u16,
    /// Effective all-conditions band; zero while the sensors are unavailable.
    pub sensor_range: u16,
    pub sensors: BattlePerceptionStatus,
    pub probe: Option<BattleProbeProfile>,
    pub radar: Option<BattleRadarProfile>,
    #[serde(skip)]
    pub(crate) clairvoyant: bool,
    #[serde(skip)]
    ceiling: u16,
    #[serde(skip)]
    cloud_base: i16,
    #[serde(skip)]
    level: i32,
}

impl BattlePerceptionProfile {
    /// The probe that can currently reach targets, if any.
    fn ready_probe(&self) -> Option<BattleProbeProfile> {
        self.probe
            .filter(|probe| probe.status == BattlePerceptionStatus::Ready)
    }

    /// Installed radar that can currently track targets, if any.
    fn ready_radar(&self) -> Option<BattleRadarProfile> {
        self.radar
            .filter(|radar| radar.status == BattlePerceptionStatus::Ready)
    }
}

/// Fixed installations extend sensor band and probe reach by forty percent.
pub(crate) fn installation_reach(world: &World, observer: ObjectId, ordinary: u16) -> u16 {
    if world
        .btech
        .vehicles()
        .get(&observer)
        .is_some_and(|unit| unit.definition().movement == BattleVehicleMovement::Stationary)
    {
        return ordinary * 140 / 100;
    }
    ordinary
}

/// Radar's aim term when it reaches an airborne target, excluding hull-down cover.
///
/// Low-flying targets stay within the ordinary map ceiling until either end climbs to
/// altitude eleven.
fn radar_aim(
    world: &World,
    profile: &BattlePerceptionProfile,
    target: ObjectId,
    point: &crate::btech::los::UnitSightPoint,
    terrain: BattleTerrainLos,
    range: BattleRange,
) -> Result<Option<i16>> {
    let Some(radar) = profile.ready_radar() else {
        return Ok(None);
    };
    let map = world
        .btech
        .maps()
        .get(&point.position.map)
        .context("Map not found")?;
    if range.spatial > map.maximum_visibility as f64 && point.level < 11 && profile.level < 11 {
        return Ok(None);
    }
    let tile = map.base_hex(i64::from(point.position.x), i64::from(point.position.y))?;
    let flying = world
        .btech
        .vehicles()
        .get(&target)
        .is_some_and(|unit| unit.definition().is_vtol());
    BattleRadarTarget::above_tile(point.level, tile, flying).evaluate(
        terrain,
        range.spatial,
        radar.range,
    )
}

/// Battlefield switches that turn off one perception channel for every unit on the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleMapPerceptionFlag {
    Sensors,
    Radar,
    Probes,
}

impl BattleMapPerceptionFlag {
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
            let flag = BattleMapPerceptionFlag::ALL
                .into_iter()
                .find(|flag| flag.name().eq_ignore_ascii_case(name))
                .with_context(|| format!("Unknown sensor flag {name:?}"))?;
            Ok(flags | flag.bit())
        })
}

/// Display the disabled perception channels in `flags`; `-` when none are disabled.
pub fn format_perception_flags(flags: i64) -> String {
    let names: Vec<_> = BattleMapPerceptionFlag::ALL
        .into_iter()
        .filter(|flag| flags & flag.bit() != 0)
        .map(BattleMapPerceptionFlag::name)
        .collect();
    if names.is_empty() {
        return "-".into();
    }
    names.join(" ")
}

impl StoredMap {
    /// Whether operators switched this perception channel off for the battlefield.
    pub fn perception_disabled(&self, flag: BattleMapPerceptionFlag) -> bool {
        self.sensor_flags & flag.bit() != 0
    }
}

/// Enable or disable one perception channel on a battlefield without touching other flags.
pub fn set_map_perception(
    world: &mut World,
    map: ObjectId,
    flag: BattleMapPerceptionFlag,
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
pub fn perception_profile(world: &World, observer: ObjectId) -> Result<BattlePerceptionProfile> {
    let unit = crate::btech::scanner::scanner_unit(world, observer)
        .context("Observer is not constructed")?;
    let position = unit.position.context("Observer is not placed")?;
    let map = world
        .btech
        .maps()
        .get(&position.map)
        .context("Map not found")?;
    let light = map.light_level()?;
    let ceiling = u16::try_from(map.maximum_visibility.clamp(0, 60))?;
    let visibility = u16::try_from(map.visibility.clamp(0, 60))?;
    let sight_range = visibility.min(ceiling);
    let lit_sight_range = if light == BattleLight::Night {
        (visibility * 3).min(ceiling)
    } else {
        sight_range
    };
    let jammed = crate::btech::electronic_field(world, observer)?.blocks_outgoing_guidance();
    let (sensors, sensor_range) = sensor_band(world, observer, map, jammed)?;
    let radar = unit.radar.then(|| BattleRadarProfile {
        range: RADAR_RANGE,
        status: if map.perception_disabled(BattleMapPerceptionFlag::Radar) {
            BattlePerceptionStatus::Disabled
        } else {
            BattlePerceptionStatus::Ready
        },
    });
    Ok(BattlePerceptionProfile {
        light,
        sight_range,
        lit_sight_range,
        sensor_range,
        sensors,
        probe: probe_profile(world, observer, map, jammed)?,
        radar,
        clairvoyant: unit.visibility.clairvoyant,
        ceiling,
        cloud_base: map.cloud_base,
        level: crate::btech::los::unit_sight_point(world, observer)?.level,
    })
}

/// Resolve the all-conditions band from equipment, damage, jamming and map switches.
fn sensor_band(
    world: &World,
    observer: ObjectId,
    map: &StoredMap,
    jammed: bool,
) -> Result<(BattlePerceptionStatus, u16)> {
    let mech = world.btech.constructed_units().get(&observer);
    let absent = match (mech, world.btech.vehicles().get(&observer)) {
        (Some(unit), _) => unit.definition().has_special("NoSensors"),
        (None, Some(vehicle)) => vehicle.definition().has_special("NoSensors"),
        (None, None) => anyhow::bail!("Observer is not constructed"),
    };
    if absent {
        return Ok((BattlePerceptionStatus::Absent, 0));
    }
    if map.perception_disabled(BattleMapPerceptionFlag::Sensors) {
        return Ok((BattlePerceptionStatus::Disabled, 0));
    }
    let hits = mech.map_or(0, |unit| unit.system_hits(BattleSystem::Sensors));
    if hits >= 2 {
        return Ok((BattlePerceptionStatus::Damaged, 0));
    }
    if jammed {
        return Ok((BattlePerceptionStatus::Jammed, 0));
    }
    let range = installation_reach(world, observer, world.btech.sensor_range.0);
    if hits == 1 {
        return Ok((BattlePerceptionStatus::Degraded, range / 2));
    }
    Ok((BattlePerceptionStatus::Ready, range))
}

/// Pick the working probe with the longest reach, or else the best installed one as damaged.
fn probe_profile(
    world: &World,
    observer: ObjectId,
    map: &StoredMap,
    jammed: bool,
) -> Result<Option<BattleProbeProfile>> {
    // Build the equipment projection once and check every family against it.
    let (clan, fittings) = match world.btech.vehicles().get(&observer) {
        Some(vehicle) => {
            let loadout = vehicle.loadout()?;
            (
                vehicle.definition().has_special("Clan"),
                BattleActiveProbe::BY_REACH
                    .map(|kind| (kind, vehicle.probe_fitting(&loadout, kind))),
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
                BattleActiveProbe::BY_REACH.map(|kind| (kind, unit.probe_fitting(&loadout, kind))),
            )
        }
    };
    let mut damaged = None;
    for (kind, fitting) in fittings {
        if !fitting.installed {
            continue;
        }
        let range = installation_reach(world, observer, u16::from(kind.range(clan)));
        if !fitting.available {
            damaged.get_or_insert(BattleProbeProfile {
                kind,
                range,
                status: BattlePerceptionStatus::Damaged,
            });
            continue;
        }
        let status = if map.perception_disabled(BattleMapPerceptionFlag::Probes) {
            BattlePerceptionStatus::Disabled
        } else if jammed {
            BattlePerceptionStatus::Jammed
        } else {
            BattlePerceptionStatus::Ready
        };
        return Ok(Some(BattleProbeProfile {
            kind,
            range,
            status,
        }));
    }
    Ok(damaged)
}

/// Perceive one target with a freshly computed observer profile.
pub fn perceive(
    world: &World,
    observer: ObjectId,
    target: ObjectId,
) -> Result<Option<BattlePerception>> {
    let profile = perception_profile(world, observer)?;
    perceive_prepared(world, observer, &profile, target, None, None)
}

/// Perceive one target, reusing an observation's profile, pair geometry and lighting cache.
///
/// Nothing here survives the immutable borrow: callers must not reuse the result after a
/// world mutation such as movement or a shot.
pub(crate) fn perceive_prepared(
    world: &World,
    observer: ObjectId,
    profile: &BattlePerceptionProfile,
    target: ObjectId,
    geometry: Option<(BattleTerrainLos, BattleRange)>,
    illumination: Option<&crate::btech::searchlight::IlluminationContext<'_>>,
) -> Result<Option<BattlePerception>> {
    let _measurement = crate::btech::autopilot::diagnostics::measure(
        crate::btech::autopilot::diagnostics::Category::Sensors,
    );
    ensure!(observer != target, "A unit cannot perceive itself");
    let other =
        crate::btech::scanner::scanner_unit(world, target).context("Target is not constructed")?;
    if other.visibility.invisible {
        return Ok(None);
    }
    let (terrain, range) = match geometry {
        Some(geometry) => geometry,
        None => crate::btech::los::unit_terrain_geometry(world, observer, target)?,
    };
    let point = crate::btech::los::unit_sight_point(world, target)?;
    let hull_down = crate::btech::hull_down::cover_modifier(world, target, terrain.partial_cover);
    let facts = SightFacts {
        terrain,
        distance: range.spatial,
        target_underwater: point.below_waterline(),
        crosses_clouds: crate::btech::clouds::crosses(
            profile.cloud_base,
            profile.level,
            point.level,
        ),
        light: profile.light,
        sight_range: profile.sight_range,
        lit_sight_range: profile.lit_sight_range,
        ceiling: profile.ceiling,
        sensor_range: if other.concealed {
            0
        } else {
            profile.sensor_range
        },
        hull_down,
    };
    let mut best: Option<(i16, BattleDetectionChannel)> = None;
    let mut offer = |channel: BattleDetectionChannel, aim: i16| {
        if best.is_none_or(|current| (aim, channel) < current) {
            best = Some((aim, channel));
        }
    };
    let lit = || {
        Ok(illumination.map_or_else(
            || crate::btech::unit_illuminated(world, target),
            |context| context.illuminated(target),
        ))
    };
    if let Some((channel, aim)) = sight::perceive(&facts, lit)? {
        offer(channel, aim);
    }
    let mut probed = false;
    if let Some(probe) = profile.ready_probe()
        && range.spatial <= f64::from(probe.range)
        && range.spatial <= f64::from(profile.ceiling)
        && (!other.concealed || probe.kind.sees_concealed())
        && !crate::btech::electronic_field(world, target)?.angel_protected
    {
        probed = true;
        offer(
            BattleDetectionChannel::Probe,
            sight::partial_cover(terrain, hull_down),
        );
    }
    if let Some(aim) = radar_aim(world, profile, target, &point, terrain, range)? {
        offer(BattleDetectionChannel::Radar, aim + hull_down);
    }
    Ok(best.map(|(aim_modifier, channel)| BattlePerception {
        channel,
        identified: !terrain.blocked,
        aim_modifier,
        probed,
        range,
    }))
}

/// Perceive an empty battlefield hex with a freshly computed observer profile.
pub fn hex_perception(
    world: &World,
    observer: ObjectId,
    target: HexCoordinate,
) -> Result<Option<BattleDetectionChannel>> {
    let profile = perception_profile(world, observer)?;
    hex_perception_prepared(world, observer, &profile, target, true)
}

/// Resolve whether the sensor band or sight reaches a hex; probes and radar need a unit to find.
///
/// Clairvoyant observers see every hex. Stopped units see nothing unless `require_running`
/// is false, which lets callers inspect a parked unit's view.
pub(crate) fn hex_perception_prepared(
    world: &World,
    observer: ObjectId,
    profile: &BattlePerceptionProfile,
    target: HexCoordinate,
    require_running: bool,
) -> Result<Option<BattleDetectionChannel>> {
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
    if require_running && unit.power != BattlePower::Running {
        return Ok(None);
    }
    if profile.clairvoyant {
        return Ok(Some(BattleDetectionChannel::Sight));
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
        ceiling: profile.ceiling,
        sensor_range: profile.sensor_range,
        hull_down: 0,
    };
    let lit = || crate::btech::hex_illuminated(world, position.map, target);
    Ok(sight::perceive(&facts, lit)?.map(|(channel, _)| channel))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Channel codes, names and serialization stay aligned for rows and Lua.
    #[test]
    fn channel_codes_names_and_serialization_agree() {
        for channel in BattleDetectionChannel::ALL {
            assert_eq!(
                serde_json::to_value(channel).unwrap(),
                serde_json::json!(channel.name())
            );
            assert!(channel.code(true).is_ascii_uppercase());
            assert_eq!(channel.code(false), channel.code(true).to_ascii_lowercase());
        }
        assert_eq!(
            BattleDetectionChannel::ALL
                .map(|channel| channel.code(true))
                .iter()
                .collect::<String>(),
            "SVRP"
        );
    }

    /// Declaration order is the tie-break when two channels give the same aim.
    #[test]
    fn channel_order_prefers_everyday_channels_on_ties() {
        let mut sorted = BattleDetectionChannel::ALL;
        sorted.sort();
        assert_eq!(sorted, BattleDetectionChannel::ALL);
        assert!(
            (0, BattleDetectionChannel::Sensors) < (0, BattleDetectionChannel::Probe)
                && (-3, BattleDetectionChannel::Radar) < (0, BattleDetectionChannel::Sensors)
        );
    }

    /// Map switches use the reference bit positions and leave other bits alone.
    #[test]
    fn map_flags_use_reference_bits() {
        let mut map: StoredMap = serde_json::from_value(serde_json::json!({
            "name": "flags", "width": 1, "height": 1, "gravity": 100, "temperature": 20,
            "flags": 0, "light": 2, "visibility": 30, "maximum_visibility": 60,
            "cloud_base": 0, "sensor_flags": 2
        }))
        .unwrap();
        assert!(!map.perception_disabled(BattleMapPerceptionFlag::Sensors));
        map.sensor_flags |= BattleMapPerceptionFlag::Probes.bit();
        assert!(map.perception_disabled(BattleMapPerceptionFlag::Probes));
        assert!(!map.perception_disabled(BattleMapPerceptionFlag::Radar));
        assert_eq!(map.sensor_flags, 66);
        assert_eq!(
            [
                BattleMapPerceptionFlag::Sensors,
                BattleMapPerceptionFlag::Radar,
                BattleMapPerceptionFlag::Probes
            ]
            .map(BattleMapPerceptionFlag::bit),
            [1, 32, 64]
        );
    }

    /// Unconfigured worlds use the default band and configuration clamps to the map ceiling.
    #[test]
    fn sensor_range_defaults_and_clamps() {
        let mut world = World::default();
        assert_eq!(
            world.btech.sensor_range,
            BattleSensorRange(DEFAULT_SENSOR_RANGE)
        );
        configure_perception(&mut world, 90);
        assert_eq!(world.btech.sensor_range, BattleSensorRange(60));
        configure_perception(&mut world, -1);
        assert_eq!(world.btech.sensor_range, BattleSensorRange(0));
    }
}
