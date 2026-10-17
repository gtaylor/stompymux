//! Terrain line-of-sight reports at live unit eye heights, independent of sensor acquisition.
//!
//! Terrain between two units follows Total Warfare's intervening terrain rule (p. 100): a
//! feature with a height, such as a hill, building, stand of woods or cloud of smoke,
//! intervenes when it is at least as high as both units' line-of-sight heights, or when it is
//! adjacent to one unit and at least as high as that unit. A unit's line-of-sight height is its
//! ground plus its eye height: one and a half levels for a standing 'Mech, half a level for a
//! vehicle or a prone 'Mech, so an integer feature height intervenes exactly when it reaches the
//! level the rulebook gives the unit. Intervening hills and buildings block the line; woods,
//! jungle, smoke and planted fields add up until they block it (see the perception module).
use super::{HexCoordinate, StoredMap};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Terrain observations; perception applies range, lighting and equipment to decide visibility.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TerrainLos {
    /// An intervening hill, building or wall, or water the line cannot pass, blocks the line.
    pub blocked: bool,
    /// Intervening woods or jungle the line passes through below the canopy: light counts once,
    /// heavy twice and ultra-heavy three times. Target woods are separate.
    pub woods: u8,
    /// Intervening smoke the line passes through: light smoke counts once, heavy smoke twice.
    /// Smoke obscures like woods but radar sees through it.
    pub smoke: u8,
    /// Intervening hexes of planted fields the line passes through at crop height: every two
    /// add one to hit, and six block the line.
    pub fields: u8,
    /// Woods or jungle in the target's own hex, when the target stands within the canopy.
    pub target_woods: u8,
    /// Smoke over the target's own hex, when the target stands within it.
    pub target_smoke: u8,
    /// Water the line crosses between the two hexes: hexes an underwater line runs through,
    /// or surface water the line passes close over. Neither end hex counts.
    pub water: u8,
    /// A standing 'Mech behind a hex one level above its own ground, or standing in water one
    /// level deep, so that only its upper body can be hit (Total Warfare p. 102).
    pub partial_cover: bool,
    /// Intervening ground (a hill) blocks the line; infrared and radar sensors cannot pass it.
    pub hills: bool,
    /// Building or wall hexes along the line that are not hardened; they block radar and
    /// magscans and shorten infrared brackets by a hex each.
    pub buildings: u8,
    /// Hardened building hexes along the line, which block radar and magscans like any other
    /// building and shorten infrared brackets by two hexes each.
    pub hardened_buildings: u8,
    /// Heavy woods or jungle hexes along the line, which shorten infrared and radar brackets.
    pub heavy_woods: u8,
    /// Ultra-heavy woods or jungle hexes along the line, which shorten them twice as much.
    pub ultra_woods: u8,
}

impl TerrainLos {
    /// Woods and smoke points together; three or more block the line.
    pub fn obscuring_points(self) -> u8 {
        self.woods.saturating_add(self.smoke)
    }

    /// Building hexes of any class along the line, hardened ones included.
    pub fn building_hexes(self) -> u8 {
        self.buildings.saturating_add(self.hardened_buildings)
    }

    /// The to-hit cost of the terrain between attacker and target: a point per light woods or
    /// light smoke hex, two per heavy (Total Warfare p. 107), three per ultra-heavy woods, and
    /// one for every two hexes of planted fields (Tactical Operations). The target's own hex
    /// and partial cover are not part of it.
    pub fn intervening_modifier(self) -> i16 {
        i16::from(self.obscuring_points()) + i16::from(self.fields / 2)
    }
}

/// Trace hex centers at standing-mech eye height, excluding the observer's own hex.
/// Intact ice endpoints use zero; bridge endpoints use deck height.
pub fn ground_terrain_los(
    map: &StoredMap,
    observer: HexCoordinate,
    target: HexCoordinate,
) -> Result<TerrainLos> {
    ground_posture_los(map, observer, target, false, false, (None, None))
}

/// Shared terrain trace at the units' current standing or prone eye heights.
fn ground_posture_los(
    map: &StoredMap,
    observer: HexCoordinate,
    target: HexCoordinate,
    observer_prone: bool,
    target_prone: bool,
    airborne: (Option<f64>, Option<f64>),
) -> Result<TerrainLos> {
    terrain_los_at_heights(
        map,
        observer,
        target,
        (
            if observer_prone { 0.5 } else { 1.5 },
            if target_prone { 0.5 } else { 1.5 },
        ),
        airborne,
        TraceOptions::default(),
    )
}

/// Whether a feature `height` levels high at the `index`th of `steps` cells intervenes between
/// line-of-sight heights `start` and `end`: it reaches both, or it reaches the unit it is
/// adjacent to.
fn intervenes(height: f64, index: usize, steps: usize, start: f64, end: f64) -> bool {
    (height >= start && height >= end)
        || (index == 1 && height >= start)
        || (index + 1 == steps && height >= end)
}

/// Endpoint rules a trace applies beyond its heights.
#[derive(Debug, Clone, Copy, Default)]
struct TraceOptions {
    /// A hull-down biped at either end: one level tall, it still uses a one-level rise beside
    /// it as partial cover instead of being hidden by it (Tactical Operations p. 19).
    kneeling: (bool, bool),
    /// The target is a hex, not a unit: the hex next to it along the line counts one level
    /// lower (Total Warfare p. 100).
    hex_target: bool,
}

/// Trace with explicit eye offsets so empty hexes do not acquire a standing unit's height.
/// Airborne heights replace the ground under either endpoint. The walk continues past the
/// first blocker so sensors learn what lies along the whole line.
fn terrain_los_at_heights(
    map: &StoredMap,
    observer: HexCoordinate,
    target: HexCoordinate,
    eyes: (f64, f64),
    airborne: (Option<f64>, Option<f64>),
    options: TraceOptions,
) -> Result<TerrainLos> {
    let kneeling = options.kneeling;
    let hex = |point: HexCoordinate| map.hex(i64::from(point.x), i64::from(point.y));
    let base = |point: HexCoordinate| map.base_hex(i64::from(point.x), i64::from(point.y));
    let source = base(observer)?;
    let destination = base(target)?;
    let visible_destination = hex(target)?;
    let start_ground = f64::from(source.standing_height());
    let end_ground = f64::from(destination.standing_height());
    let start_height = airborne.0.unwrap_or(start_ground) + eyes.0;
    let end_height = airborne.1.unwrap_or(end_ground) + eyes.1;
    // Water surfaces sit at each hex's level; "both worlds" is a unit one level below one.
    let surface = |hex: super::Hex| f64::from(hex.level());
    let underwater = source.holds_water() && start_height < surface(source);
    let target_underwater = destination.holds_water() && end_height < surface(destination);
    let target_both_worlds = destination.holds_water() && end_ground == surface(destination) - 1.0;
    let mut report = TerrainLos {
        target_woods: visible_destination.woods_density(),
        target_smoke: visible_destination.smoke_density(),
        ..TerrainLos::default()
    };
    if underwater != target_underwater {
        report.blocked = true;
        return Ok(report);
    }
    if observer == target {
        return Ok(report);
    }
    let cells = super::los_trace::center_trace(observer, target)?;
    let steps = cells.len() - 1;
    // The one-level rise a kneeling unit shelters behind is cover, not a wall.
    let shelters = |index: usize, ground: super::Hex| {
        (kneeling.0
            && index == 1
            && airborne.0.is_none()
            && f64::from(ground.surface_height()) == start_ground + 1.0)
            || (kneeling.1
                && index + 1 == steps
                && airborne.1.is_none()
                && f64::from(ground.surface_height()) == end_ground + 1.0)
    };
    for (index, &point) in cells.iter().enumerate().skip(1) {
        if index == steps {
            // The target's own hex never intervenes.
            break;
        }
        // Sensors care what the line crosses even where sight is already blocked, but once a
        // hill and a building both lie on it no sensor's answer can change: hills stop infrared
        // and radar, buildings stop magscans, and seismic sensors and probes ignore terrain.
        if report.hills && report.building_hexes() > 0 {
            break;
        }
        let tile = hex(point)?;
        let ground = base(point)?;
        if let Some(structure) = ground.structure()
            && structure.is_standing()
        {
            if structure.class == super::ConstructionClass::Hardened {
                report.hardened_buildings = report.hardened_buildings.saturating_add(1);
            } else {
                report.buildings = report.buildings.saturating_add(1);
            }
        }
        match tile.woods_density() {
            2 => report.heavy_woods = report.heavy_woods.saturating_add(1),
            3 => report.ultra_woods = report.ultra_woods.saturating_add(1),
            _ => {}
        }
        // Targeting a hex, the hex before it along the line sits one level lower.
        let relief = if options.hex_target && index + 1 == steps {
            1.0
        } else {
            0.0
        };
        if underwater {
            // An underwater line follows the water, under any bed and below the surface.
            let height = f64::from(ground.surface_height());
            let sight_height =
                start_height + (end_height - start_height) * index as f64 / steps as f64;
            if !ground.holds_water()
                || (!ground.has_bridge() && height >= sight_height)
                || (!target_both_worlds && sight_height > surface(ground))
            {
                report.blocked = true;
                return Ok(report);
            }
            report.water = report.water.saturating_add(1).min(7);
            continue;
        }
        let level = f64::from(ground.level()) - relief;
        if ground.is_water_surface() {
            if intervenes(level, index, steps, start_height, end_height) {
                report.blocked = true;
                report.hills = true;
            }
            if level + 2.0 > start_height.min(end_height) {
                report.water = report.water.saturating_add(1).min(7);
            }
        } else if !ground.has_bridge()
            && !shelters(index, ground)
            && intervenes(
                f64::from(ground.surface_height()) - relief,
                index,
                steps,
                start_height,
                end_height,
            )
        {
            report.blocked = true;
            if !ground.has_standing_structure() {
                report.hills = true;
            }
        }
        let canopy = level + f64::from(tile.foliage_height());
        if tile.foliage().is_some() && intervenes(canopy, index, steps, start_height, end_height) {
            if tile.is_woods() {
                report.woods = report.woods.saturating_add(tile.woods_density()).min(15);
            } else {
                report.fields = report.fields.saturating_add(1).min(15);
            }
        }
        if tile.is_smoky() && intervenes(level + 2.0, index, steps, start_height, end_height) {
            report.smoke = report.smoke.saturating_add(tile.smoke_density()).min(15);
        }
    }
    if steps >= 2 && (eyes.1 == 1.5 || kneeling.1) && airborne.1.is_none() {
        let preceding = base(cells[cells.len() - 2])?;
        // A hill or building one level up gives cover only to a target the attacker does not
        // look down on; a kneeling biped and water around the legs are covered from any height.
        let ridge = (start_height <= end_height || kneeling.1)
            && !preceding.has_bridge()
            && f64::from(preceding.surface_height()) == end_ground + 1.0;
        report.partial_cover =
            ridge || (destination.is_open_water() && end_ground == surface(destination) - 1.0);
    }
    // A standing 'Mech inside a building one level below its roof shows only its upper body,
    // whatever height the attacker fires from (Total Warfare p. 171).
    if steps >= 1
        && eyes.1 == 1.5
        && !kneeling.1
        && destination.has_building()
        && airborne.1 == Some(end_ground - 1.0)
    {
        report.partial_cover = true;
    }
    Ok(report)
}

/// Shared placed-unit geometry for terrain and perception queries.
pub(super) struct UnitSightPoint {
    pub position: super::Position,
    pub point: super::Point,
    /// A hull-down biped: one level tall, sheltering behind a one-level rise.
    pub kneeling: bool,
    pub eye: f64,
    /// Explicit altitude preserves flight precision and vehicle bridge/water position.
    pub height: Option<f64>,
    pub level: i32,
    /// Height of the water surface in the unit's hex, if the hex holds water.
    pub water_surface: Option<i32>,
}

impl UnitSightPoint {
    /// A low vehicle or prone unit becomes submerged one level before a standing Mech.
    pub fn below_waterline(&self) -> bool {
        let reach = if self.eye > 1.0 { 1 } else { 0 };
        self.water_surface
            .is_some_and(|surface| self.level < surface - reach)
    }
}

/// The water surface a unit in `tile` can be below.
fn water_surface(tile: super::Hex) -> Option<i32> {
    tile.holds_water().then(|| i32::from(tile.level()))
}

/// Sample either supported unit class without conflating terrain depth with its actual elevation.
pub(super) fn unit_sight_point(world: &World, id: ObjectId) -> Result<UnitSightPoint> {
    if let Some(vehicle) = world.btech.vehicles().get(&id) {
        let position = vehicle.position().context("Unit is not on a battlefield")?;
        let map = world
            .btech
            .maps()
            .get(&position.map)
            .context("Map not found")?;
        let tile = map.base_hex(i64::from(position.x), i64::from(position.y))?;
        let level = vehicle.elevation_level(tile);
        return Ok(UnitSightPoint {
            position,
            point: vehicle.motion().context("Unit has no motion")?.point,
            kneeling: false,
            eye: if vehicle.definition().movement == super::VehicleMovement::Stationary {
                1.5
            } else {
                0.5
            },
            height: Some(vehicle.altitude(tile)),
            level,
            water_surface: water_surface(tile),
        });
    }
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit construction state is unavailable")?;
    let position = unit.position().context("Unit is not on a battlefield")?;
    let map = world
        .btech
        .maps()
        .get(&position.map)
        .context("Map not found")?;
    let tile = map.base_hex(i64::from(position.x), i64::from(position.y))?;
    // A hull-down biped kneels to one level (Tactical Operations p. 19); a squatting quad keeps
    // its height.
    let kneeling = unit.hull_down().active && unit.chassis() != super::MechChassis::Quad;
    Ok(UnitSightPoint {
        position,
        point: unit.motion().context("Unit has no motion")?.point,
        kneeling,
        eye: if unit.posture() == super::Posture::Prone || kneeling {
            0.5
        } else {
            1.5
        },
        height: unit.retained_altitude(),
        level: unit.elevation_level(tile),
        water_surface: water_surface(tile),
    })
}

/// The worst-case LOS range precedes terrain tracing and is symmetric for unit endpoints.
/// Whether two points are too far apart for any line of sight: nothing on the ground sees
/// past daylight's sixty hexes (Tactical Operations p. 189).
fn beyond_maximum_range(distance: f64) -> bool {
    distance > f64::from(super::MAX_SIGHT)
}

/// Inspect terrain and the map's LOS distance ceiling without changing contact state.
pub fn unit_terrain_los(world: &World, observer: ObjectId, target: ObjectId) -> Result<TerrainLos> {
    unit_terrain_geometry(world, observer, target).map(|(terrain, _)| terrain)
}

/// Share the range already required by LOS with synchronous sensor callers.
/// No result survives this immutable query or bypasses live admission.
pub(super) fn unit_terrain_geometry(
    world: &World,
    observer: ObjectId,
    target: ObjectId,
) -> Result<(TerrainLos, super::Range)> {
    let _measurement = crate::btech::autopilot::diagnostics::measure(
        crate::btech::autopilot::diagnostics::Category::Geometry,
    );
    let range = super::unit_range(world, observer, target)?;
    let distance = range.spatial;
    let observer = unit_sight_point(world, observer)?;
    let target = unit_sight_point(world, target)?;
    ensure!(
        observer.position.map == target.position.map,
        "Units are on different battlefields"
    );
    let map = world
        .btech
        .maps()
        .get(&observer.position.map)
        .context("Map not found")?;
    if beyond_maximum_range(distance) {
        return Ok((
            TerrainLos {
                blocked: true,
                ..Default::default()
            },
            range,
        ));
    }
    let mut report = terrain_los_at_heights(
        map,
        HexCoordinate {
            x: i32::from(observer.position.x),
            y: i32::from(observer.position.y),
        },
        HexCoordinate {
            x: i32::from(target.position.x),
            y: i32::from(target.position.y),
        },
        (observer.eye, target.eye),
        (observer.height, target.height),
        TraceOptions {
            kneeling: (observer.kneeling, target.kneeling),
            hex_target: false,
        },
    )?;
    let tile = map.base_hex(i64::from(target.position.x), i64::from(target.position.y))?;
    if target.level > i32::from(tile.level()) + i32::from(tile.foliage_height()) {
        report.target_woods = 0;
    }
    if target.level > i32::from(tile.level()) + 2 {
        report.target_smoke = 0;
    }
    Ok((report, range))
}

/// Trace a terrain hex from either supported unit class and report spatial range.
pub(super) fn unit_hex_los(
    world: &World,
    observer: ObjectId,
    target: HexCoordinate,
) -> Result<(TerrainLos, f64)> {
    let unit = unit_sight_point(world, observer)?;
    let map = &world.btech.maps()[&unit.position.map];
    let source = HexCoordinate {
        x: i32::from(unit.position.x),
        y: i32::from(unit.position.y),
    };
    let tile = map.base_hex(i64::from(target.x), i64::from(target.y))?;
    let source_tile = map.base_hex(i64::from(source.x), i64::from(source.y))?;
    let altitude = unit
        .height
        .unwrap_or(f64::from(source_tile.standing_height()));
    // A water hex is targeted at its surface (Total Warfare p. 100), anything else where a
    // unit would stand. A submerged observer's line runs under the water, so it targets the
    // bed of a water hex instead.
    let target_height = f64::from(if tile.is_open_water() && !unit.below_waterline() {
        tile.water_line()
    } else {
        tile.standing_height()
    });
    let horizontal = unit.point.range(target.center())?;
    let distance = horizontal.hypot((altitude - target_height) / 5.0);
    if beyond_maximum_range(distance) {
        return Ok((
            TerrainLos {
                blocked: true,
                ..Default::default()
            },
            distance,
        ));
    }
    // A hex is sighted just above its surface, so its own ground never intervenes.
    let report = terrain_los_at_heights(
        map,
        source,
        target,
        (unit.eye, 0.1),
        (Some(altitude), Some(target_height)),
        TraceOptions {
            kneeling: (unit.kneeling, false),
            hex_target: true,
        },
    )?;
    Ok((report, distance))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Hex, Terrain};
    use std::sync::Arc;

    /// Woods and smoke on the line cost their points and crops one per two hexes; the
    /// target's own hex and partial cover stay out of the intervening term.
    #[test]
    fn intervening_modifier_counts_only_the_line() {
        let terrain = TerrainLos {
            woods: 2,
            smoke: 1,
            fields: 3,
            target_woods: 2,
            target_smoke: 1,
            partial_cover: true,
            ..TerrainLos::default()
        };
        assert_eq!(terrain.intervening_modifier(), 4);
        assert_eq!(TerrainLos::default().intervening_modifier(), 0);
    }

    /// Empty-hex LOS stops at daylight's sixty hexes and still reports the range beyond it.
    #[test]
    fn coordinate_los_uses_the_same_cutoff_and_retains_range_when_blocked() {
        let mut world = World::default();
        world
            .btech
            .maps
            .insert(ObjectId(99), lane(&[(Terrain::Clear, 0); 201]));
        let template = crate::MechTemplate::parse(
            "JR7-D",
            include_str!("../../tests/fixtures/btech/units/JR7-D.toml"),
        )
        .unwrap();
        let mut unit = crate::Mech::from_template(template).unwrap();
        unit.position = Some(crate::Position {
            map: ObjectId(99),
            x: 0,
            y: 0,
        });
        unit.motion = Some(crate::Motion::stationary(
            HexCoordinate { x: 0, y: 0 }.center(),
        ));
        world.btech.constructed.insert(ObjectId(1), unit);
        let before = world.btech.clone();
        for y in [10, 60, 61, 180, 200] {
            let (los, distance) =
                unit_hex_los(&world, ObjectId(1), HexCoordinate { x: 0, y }).unwrap();
            assert_eq!(distance, f64::from(y));
            assert_eq!(los.blocked, y > i32::from(crate::btech::MAX_SIGHT));
        }
        assert_eq!(world.btech, before);
    }

    /// A north/south lane avoids ambiguous hex-edge crossings in terrain rule fixtures.
    fn lane(tiles: &[(Terrain, u8)]) -> StoredMap {
        let mut map = StoredMap {
            membership_extent: 0,
            building_parent: 0,
            cargo_transfer_point: None,
            artillery_shots: Default::default(),
            name: "LOS lane".into(),
            width: 3,
            height: tiles.len() as i64,
            gravity: 100,
            temperature: 20,
            flags: 0,
            building: Default::default(),
            building_repair: None,
            landing_exclusions: Default::default(),
            landing_exclusion_order: Default::default(),
            minefields: Default::default(),
            minefield_order: Default::default(),
            building_entrances: Default::default(),
            building_entry_points: Default::default(),
            building_exits: Default::default(),
            authored_link: None,
            light: 2,
            weather: 0,
            cloud_base: 200,
            wind_direction: 0,
            wind_speed: 0,
            fire_dice: None,
            sensor_dice: None,
            terrain_damage: Default::default(),
            decorations: Default::default(),
            static_decorations: Default::default(),
            points_of_interest: Default::default(),
            regions: Default::default(),
            terrain: None,
        };
        map.establish_terrain(Arc::new(
            tiles
                .iter()
                .flat_map(|&(terrain, elevation)| [Hex::new(terrain, elevation); 3])
                .collect(),
        ))
        .unwrap();
        map
    }

    fn sight(tiles: &[(Terrain, u8)]) -> TerrainLos {
        ground_terrain_los(
            &lane(tiles),
            HexCoordinate { x: 1, y: 0 },
            HexCoordinate {
                x: 1,
                y: tiles.len() as i32 - 1,
            },
        )
        .unwrap()
    }

    #[test]
    fn cached_topology_still_reads_current_terrain_and_eye_heights() {
        use Terrain::*;
        let from = HexCoordinate { x: 1, y: 0 };
        let to = HexCoordinate { x: 1, y: 2 };
        super::super::los_trace::clear();
        assert!(!sight(&[(Clear, 0); 3]).blocked);
        assert!(sight(&[(Clear, 0), (Clear, 2), (Clear, 0)]).blocked);
        assert!(!sight(&[(Clear, 0); 3]).blocked);
        let map = lane(&[(Clear, 0), (Clear, 1), (Clear, 0)]);
        assert!(
            !ground_posture_los(&map, from, to, false, false, (None, None))
                .unwrap()
                .blocked
        );
        assert!(
            ground_posture_los(&map, from, to, true, true, (None, None))
                .unwrap()
                .blocked
        );
        assert!(
            !ground_posture_los(&map, from, to, false, false, (None, None))
                .unwrap()
                .blocked
        );
    }

    /// Woods, jungle and smoke on the way count their density; the endpoints' own hexes count
    /// only for the target, and fire itself hides nothing.
    #[test]
    fn woods_and_smoke_exclude_endpoints() {
        use Terrain::*;
        let report = sight(&[
            (HeavyWoods, 0),
            (LightWoods, 0),
            (HeavyWoods, 0),
            (Smoke, 0),
            (HeavySmoke, 0),
            (Fire, 0),
            (UltraRough, 0),
            (HeavyWoods, 0),
        ]);
        assert_eq!(report.woods, 3);
        assert_eq!(report.smoke, 3);
        assert_eq!(report.obscuring_points(), 6);
        assert_eq!(report.target_woods, 2);
        assert_eq!(report.target_smoke, 0);
        assert!(!report.blocked);
        let report = sight(&[(Fire, 0), (Clear, 0), (HeavySmoke, 0)]);
        assert_eq!(report.smoke, 0);
        assert_eq!(report.target_smoke, 2);
        assert_eq!(report.woods, 0);
        assert_eq!(
            sight(&[(Clear, 0), (Fire, 0), (Clear, 0)]).obscuring_points(),
            0
        );
    }

    /// Planted fields rise one level and are counted by the hex: they intervene between low
    /// units such as vehicles and prone 'Mechs, never between standing 'Mechs on the same
    /// ground.
    #[test]
    fn planted_fields_count_by_the_hex_at_crop_height() {
        use Terrain::*;
        let map = lane(&[
            (Clear, 0),
            (PlantedFields, 0),
            (PlantedFields, 0),
            (Clear, 0),
        ]);
        let from = HexCoordinate { x: 1, y: 0 };
        let to = HexCoordinate { x: 1, y: 3 };
        let low = ground_posture_los(&map, from, to, true, true, (None, None)).unwrap();
        assert_eq!(low.fields, 2);
        assert_eq!(low.woods, 0);
        assert_eq!(low.target_woods, 0);
        assert!(!low.blocked);
        let standing = ground_posture_los(&map, from, to, false, false, (None, None)).unwrap();
        assert_eq!(standing.fields, 0);
        // Crops next to a low target reach it even when the attacker stands tall.
        let mixed = ground_posture_los(&map, from, to, false, true, (None, None)).unwrap();
        assert_eq!(mixed.fields, 1);
    }

    /// Terrain intervenes when it reaches both units, or the unit it stands next to.
    #[test]
    fn terrain_intervenes_by_the_rulebook_level_test() {
        use Terrain::*;
        assert!(sight(&[(Clear, 0), (Clear, 2), (Clear, 0)]).blocked);
        let report = sight(&[(Clear, 0), (Clear, 1), (Clear, 0)]);
        assert!(!report.blocked);
        assert!(report.partial_cover);
        // Woods below both units never intervene.
        assert_eq!(sight(&[(Clear, 3), (HeavyWoods, 0), (Clear, 3)]).woods, 0);
        // A hill next to the attacker intervenes at the attacker's level even when the target
        // stands higher; the same hill in the middle does not.
        assert!(sight(&[(Clear, 0), (Clear, 2), (Clear, 3), (Clear, 3)]).blocked);
        assert!(!sight(&[(Clear, 0), (Clear, 0), (Clear, 2), (Clear, 3), (Clear, 3)]).blocked);
        // A hill next to the target intervenes at the target's level.
        assert!(sight(&[(Clear, 3), (Clear, 3), (Clear, 2), (Clear, 0)]).blocked);
        // Woods next to the target at the target's level intervene; two levels below neither.
        assert_eq!(
            sight(&[(Clear, 3), (Clear, 3), (LightWoods, 1), (Clear, 1)]).woods,
            1
        );
        assert_eq!(
            sight(&[(Clear, 3), (Clear, 3), (LightWoods, 0), (Clear, 3)]).woods,
            0
        );
    }

    /// A one-level ridge midway hides vehicles from each other but not from 'Mechs, and a
    /// vehicle next to the ridge is hidden from a 'Mech beyond it.
    #[test]
    fn low_units_lose_sight_over_a_ridge_tall_units_see_across() {
        use Terrain::*;
        let map = lane(&[(Clear, 0), (Clear, 0), (Clear, 1), (Clear, 0), (Clear, 0)]);
        let from = HexCoordinate { x: 1, y: 0 };
        let to = HexCoordinate { x: 1, y: 4 };
        let los = |eyes: (f64, f64)| {
            terrain_los_at_heights(&map, from, to, eyes, (None, None), TraceOptions::default())
                .unwrap()
                .blocked
        };
        assert!(!los((1.5, 1.5)));
        assert!(los((0.5, 0.5)));
        // A ridge in the middle is lower than the 'Mech, so it does not intervene for either.
        assert!(!los((1.5, 0.5)));
        assert!(!los((0.5, 1.5)));
        // Next to the vehicle it reaches the vehicle's level and hides it from the 'Mech.
        let near = lane(&[(Clear, 0), (Clear, 0), (Clear, 0), (Clear, 1), (Clear, 0)]);
        assert!(
            terrain_los_at_heights(
                &near,
                from,
                to,
                (1.5, 0.5),
                (None, None),
                TraceOptions::default()
            )
            .unwrap()
            .blocked
        );
        assert!(
            terrain_los_at_heights(
                &map,
                from,
                to,
                (0.5, 0.5),
                (Some(0.0), Some(0.0)),
                TraceOptions::default()
            )
            .unwrap()
            .blocked
        );
    }

    /// Intervening terrain is checked at any height (Total Warfare pp. 99-100): a level-20
    /// mountain still blocks two 'Mechs standing on level-11 hills, woods between them still
    /// count, and two VTOLs flying above level 10 still lose sight across the peak.
    #[test]
    fn terrain_intervenes_between_units_above_level_ten() {
        use Terrain::*;
        let ridge = [
            (Clear, 11),
            (Clear, 11),
            (Clear, 20),
            (Clear, 11),
            (Clear, 11),
        ];
        let report = sight(&ridge);
        assert!(report.blocked);
        assert!(report.hills);
        let woods = sight(&[(Clear, 11), (HeavyWoods, 11), (LightWoods, 11), (Clear, 11)]);
        assert_eq!(woods.woods, 3);
        assert!(!woods.blocked);
        assert!(!sight(&[(Clear, 11); 5]).blocked);
        let map = lane(&ridge);
        let flying = |altitude: f64| {
            terrain_los_at_heights(
                &map,
                HexCoordinate { x: 1, y: 0 },
                HexCoordinate { x: 1, y: 4 },
                (0.5, 0.5),
                (Some(altitude), Some(altitude)),
                TraceOptions::default(),
            )
            .unwrap()
            .blocked
        };
        assert!(flying(15.0));
        assert!(!flying(21.0));
    }

    /// Partial cover needs an attacker no higher than the target, or water around the legs.
    #[test]
    fn partial_cover_is_negated_from_above_except_in_water() {
        use Terrain::*;
        assert!(sight(&[(Clear, 0), (Clear, 0), (Clear, 1), (Clear, 0)]).partial_cover);
        assert!(!sight(&[(Clear, 2), (Clear, 1), (Clear, 1), (Clear, 0)]).partial_cover);
        assert!(sight(&[(Clear, 0), (Clear, 1), (Clear, 1), (Clear, 0)]).partial_cover);
        // A bridge deck one level up gives no cover; a building does.
        assert!(!sight(&[(Clear, 0), (Clear, 0), (Bridge, 1), (Clear, 0)]).partial_cover);
        assert!(sight(&[(Clear, 0), (Clear, 0), (Building, 1), (Clear, 0)]).partial_cover);
        let wading = sight(&[(Clear, 5), (Clear, 5), (Water, 1), (Water, 1)]);
        assert!(!wading.blocked);
        assert!(wading.partial_cover);
    }

    #[test]
    fn underwater_paths_count_depth_and_reject_air_or_seafloor() {
        use Terrain::*;
        // Six hexes lie between the ends of an eight-hex lane; neither end counts.
        let report = sight(&[(Water, 3); 8]);
        assert!(!report.blocked);
        assert_eq!(report.water, 6);
        assert!(!sight(&[(Water, 3), (Ice, 3), (Water, 3)]).blocked);
        assert!(sight(&[(Water, 3), (Clear, 0), (Water, 3)]).blocked);
        assert!(sight(&[(Water, 3), (Water, 1), (Water, 3)]).blocked);
        assert!(sight(&[(Water, 3), (Water, 3), (Clear, 0)]).blocked);
        let report = sight(&[(Clear, 0), (Water, 1), (Water, 1)]);
        assert!(!report.blocked);
        assert!(report.partial_cover);
    }

    /// A hex on an ice sheet is seen just above the ice; intervening ground still blocks.
    #[test]
    fn coordinate_ice_endpoint_is_visible_without_ignoring_intervening_ground() {
        let from = HexCoordinate { x: 1, y: 0 };
        let to = HexCoordinate { x: 1, y: 2 };
        for eye in [0.5, 1.5] {
            let clear = lane(&[(Terrain::Clear, 0), (Terrain::Clear, 0), (Terrain::Ice, 0)]);
            assert!(
                !terrain_los_at_heights(
                    &clear,
                    from,
                    to,
                    (eye, 0.1),
                    (None, None),
                    TraceOptions::default()
                )
                .unwrap()
                .blocked
            );
            let ridge = lane(&[(Terrain::Clear, 0), (Terrain::Clear, 2), (Terrain::Ice, 0)]);
            assert!(
                terrain_los_at_heights(
                    &ridge,
                    from,
                    to,
                    (eye, 0.1),
                    (None, None),
                    TraceOptions::default()
                )
                .unwrap()
                .blocked
            );
        }
    }

    #[test]
    fn ice_surface_endpoints_use_unit_height_instead_of_water_depth() {
        use Terrain::*;
        for depth in [0, 1, 3, 9] {
            let report = sight(&[(Ice, depth); 3]);
            assert!(!report.blocked);
            assert!(!report.partial_cover);
            assert!(!sight(&[(Clear, 0), (Ice, depth), (Ice, depth)]).blocked);
            assert!(!sight(&[(Ice, depth), (Ice, depth), (Clear, 0)]).blocked);
            assert!(sight(&[(Ice, depth), (Clear, 2), (Ice, depth)]).blocked);
            assert!(sight(&[(Ice, depth), (Water, 3), (Water, 3)]).blocked);
            assert!(sight(&[(Water, 3), (Water, 3), (Ice, depth)]).blocked);
        }
        let map = lane(&[(Ice, 9); 3]);
        let point = HexCoordinate { x: 1, y: 0 };
        assert!(!ground_terrain_los(&map, point, point).unwrap().blocked);
    }

    #[test]
    fn bridge_decks_and_underwater_spans_use_endpoint_altitudes() {
        use Terrain::*;
        for height in [0, 1, 3, 9] {
            assert!(!sight(&[(Bridge, height); 3]).blocked);
            assert!(!sight(&[(Clear, height), (Bridge, height), (Bridge, height)]).blocked);
            assert!(!sight(&[(Bridge, height), (Bridge, height), (Clear, height)]).blocked);
        }
        assert!(sight(&[(Bridge, 0), (Clear, 2), (Bridge, 0)]).blocked);
        assert!(!sight(&[(Bridge, 0), (Bridge, 9), (Bridge, 0)]).blocked);
        assert_eq!(sight(&[(Bridge, 0), (HeavyWoods, 0), (Bridge, 0)]).woods, 2);
        let map = lane(&[(Bridge, 9); 3]);
        let source = HexCoordinate { x: 1, y: 0 };
        let target = HexCoordinate { x: 1, y: 2 };
        let submerged =
            ground_posture_los(&map, source, target, false, false, (Some(-3.0), Some(-3.0)))
                .unwrap();
        assert!(!submerged.blocked);
        assert_eq!(submerged.water, 1);
        assert!(
            ground_posture_los(&map, source, target, false, false, (Some(-3.0), None))
                .unwrap()
                .blocked
        );
    }

    #[test]
    fn same_hex_is_clear_and_out_of_map_endpoints_return_errors() {
        use Terrain::*;
        let map = lane(&[(HeavyWoods, 0)]);
        let point = HexCoordinate { x: 1, y: 0 };
        let report = ground_terrain_los(&map, point, point).unwrap();
        assert!(!report.blocked);
        assert_eq!(report.woods, 0);
        assert_eq!(report.target_woods, 2);
        assert!(ground_terrain_los(&map, point, HexCoordinate { x: 1, y: 1 }).is_err());
        assert!(
            !ground_terrain_los(&lane(&[(Bridge, 1)]), point, point)
                .unwrap()
                .blocked
        );
    }
}
