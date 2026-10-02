//! Terrain line-of-sight reports at live unit eye heights, independent of sensor acquisition.
use super::{BattleHexCoordinate, StoredBattleMap};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Terrain observations; perception applies range, lighting and equipment to decide visibility.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleTerrainLos {
    pub blocked: bool,
    /// Intervening light woods count once and heavy woods twice; target woods are separate.
    pub woods: u8,
    pub target_woods: u8,
    pub water: u8,
    pub smoke: bool,
    pub fire: bool,
    pub partial_cover: bool,
}

/// Trace hex centers at standing-mech eye height, excluding the observer's own hex.
/// Intact ice endpoints use zero; bridge endpoints use deck height.
pub fn ground_terrain_los(
    map: &StoredBattleMap,
    observer: BattleHexCoordinate,
    target: BattleHexCoordinate,
) -> Result<BattleTerrainLos> {
    ground_posture_los(map, observer, target, false, false, (None, None))
}

/// Shared terrain trace at the units' current standing or prone eye heights.
fn ground_posture_los(
    map: &StoredBattleMap,
    observer: BattleHexCoordinate,
    target: BattleHexCoordinate,
    observer_prone: bool,
    target_prone: bool,
    airborne: (Option<f64>, Option<f64>),
) -> Result<BattleTerrainLos> {
    terrain_los_at_heights(
        map,
        observer,
        target,
        (
            if observer_prone { 0.5 } else { 1.5 },
            if target_prone { 0.5 } else { 1.5 },
        ),
        airborne,
    )
}

/// Trace with explicit eye offsets so empty hexes do not acquire a standing unit's height.
fn terrain_los_at_heights(
    map: &StoredBattleMap,
    observer: BattleHexCoordinate,
    target: BattleHexCoordinate,
    eyes: (f64, f64),
    airborne: (Option<f64>, Option<f64>),
) -> Result<BattleTerrainLos> {
    terrain_los_with_endpoint(map, observer, target, eyes, airborne, false)
}

/// Coordinate fire at the ice surface permits the final cell to meet the sightline.
fn terrain_los_with_endpoint(
    map: &StoredBattleMap,
    observer: BattleHexCoordinate,
    target: BattleHexCoordinate,
    eyes: (f64, f64),
    airborne: (Option<f64>, Option<f64>),
    ice_surface: bool,
) -> Result<BattleTerrainLos> {
    let hex = |point: BattleHexCoordinate| map.hex(i64::from(point.x), i64::from(point.y));
    let base = |point: BattleHexCoordinate| map.base_hex(i64::from(point.x), i64::from(point.y));
    let source = base(observer)?;
    let destination = base(target)?;
    let visible_destination = hex(target)?;
    let start_ground = f64::from(source.standing_height());
    let end_ground = f64::from(destination.standing_height());
    let start_height = airborne.0.unwrap_or(start_ground) + eyes.0;
    let end_height = airborne.1.unwrap_or(end_ground) + eyes.1;
    let underwater = source.holds_water() && start_height < 0.0;
    let target_underwater = destination.holds_water() && end_height < 0.0;
    let both_worlds = source.holds_water() && start_ground == -1.0;
    let target_both_worlds = destination.holds_water() && end_ground == -1.0;
    let mut report = BattleTerrainLos {
        target_woods: visible_destination.woods_density(),
        ..BattleTerrainLos::default()
    };
    if start_height > 10.0 && end_height > 10.0 {
        return Ok(report);
    }
    if underwater != target_underwater {
        report.blocked = true;
        return Ok(report);
    }
    if observer == target {
        return Ok(report);
    }
    let cells = super::los_trace::center_trace(observer, target)?;
    let steps = cells.len() - 1;
    let mut submerged = 0;
    for (index, &point) in cells.iter().enumerate().skip(1) {
        let tile = hex(point)?;
        let ground = base(point)?;
        let height = f64::from(ground.surface_height());
        let sight_height = start_height + (end_height - start_height) * index as f64 / steps as f64;
        let intervening = index < steps;
        if underwater {
            if !ground.holds_water()
                || (!ground.has_bridge() && height >= sight_height)
                || (!target_both_worlds && sight_height > 0.0)
            {
                report.blocked = true;
                return Ok(report);
            }
            submerged += usize::from(sight_height <= 0.0);
            report.water = report.water.saturating_add(1).min(7);
            continue;
        }
        if sight_height < height + 2.0 {
            if ground.is_water_surface() {
                if sight_height < 0.0
                    && (ground.is_ice() || (ground.is_open_water() && !both_worlds))
                {
                    report.blocked = true;
                    return Ok(report);
                }
                if ground.is_open_water() && sight_height < 0.0 {
                    submerged += 1;
                }
                report.water = report.water.saturating_add(1).min(7);
            }
            if intervening {
                report.woods = (report.woods + tile.woods_density()).min(15);
                match tile.overlay() {
                    Some(super::BattleDecorationKind::Smoke) => report.smoke = true,
                    Some(super::BattleDecorationKind::Fire) => report.fire = true,
                    None => {}
                }
            }
        }
        if height >= sight_height && !ground.has_bridge() && !(ice_surface && !intervening) {
            report.blocked = true;
            return Ok(report);
        }
    }
    if steps >= 2 && eyes.1 == 1.5 && airborne.1.is_none() {
        let preceding = base(cells[cells.len() - 2])?;
        report.partial_cover = (end_ground >= start_ground
            && f64::from(preceding.surface_height()) == end_ground + 1.0)
            || (destination.is_open_water() && end_ground == -1.0);
    }
    report.fire |= submerged > 6;
    Ok(report)
}

/// Shared placed-unit geometry for terrain and perception queries.
pub(super) struct UnitSightPoint {
    pub position: super::BattlePosition,
    pub point: super::BattlePoint,
    pub eye: f64,
    /// Explicit altitude preserves flight precision and vehicle bridge/water position.
    pub height: Option<f64>,
    pub level: i32,
}

impl UnitSightPoint {
    /// A low vehicle or prone unit becomes submerged one level before a standing Mech.
    pub fn below_waterline(&self) -> bool {
        self.level < if self.eye > 1.0 { -1 } else { 0 }
    }
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
            eye: if vehicle.definition().movement == super::BattleVehicleMovement::Stationary {
                1.5
            } else if vehicle.dig_state().dug_in {
                0.1
            } else {
                0.5
            },
            height: Some(vehicle.altitude(tile)),
            level,
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
    Ok(UnitSightPoint {
        position,
        point: unit.motion().context("Unit has no motion")?.point,
        eye: if unit.posture() == super::BattlePosture::Prone {
            0.5
        } else {
            1.5
        },
        height: unit.retained_altitude(),
        level: unit.elevation_level(tile),
    })
}

/// The worst-case LOS range precedes terrain tracing and is symmetric for unit endpoints.
fn beyond_maximum_range(
    world: &World,
    map: &StoredBattleMap,
    observer: ObjectId,
    target: Option<ObjectId>,
    distance: f64,
) -> bool {
    // Either endpoint's radar can select only one of these two ceilings.
    // Outside their overlap, hardware inspection cannot change the answer.
    let ordinary = map.maximum_visibility as f64;
    if distance <= ordinary.min(180.0) {
        return false;
    }
    if distance > ordinary.max(180.0) {
        return true;
    }
    let radar = std::iter::once(observer)
        .chain(target)
        .any(|id| super::scanner::scanner_unit(world, id).is_some_and(|unit| unit.radar));
    distance
        > if radar {
            180.0
        } else {
            map.maximum_visibility as f64
        }
}

/// Inspect terrain and the map's LOS distance ceiling without changing contact state.
pub fn unit_terrain_los(
    world: &World,
    observer: ObjectId,
    target: ObjectId,
) -> Result<BattleTerrainLos> {
    unit_terrain_geometry(world, observer, target).map(|(terrain, _)| terrain)
}

/// Share the range already required by LOS with synchronous sensor callers.
/// No result survives this immutable query or bypasses live admission.
pub(super) fn unit_terrain_geometry(
    world: &World,
    observer: ObjectId,
    target: ObjectId,
) -> Result<(BattleTerrainLos, super::BattleRange)> {
    let _measurement = crate::btech::autopilot::diagnostics::measure(
        crate::btech::autopilot::diagnostics::Category::Geometry,
    );
    let observer_id = observer;
    let target_id = target;
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
    if beyond_maximum_range(world, map, observer_id, Some(target_id), distance) {
        return Ok((
            BattleTerrainLos {
                blocked: true,
                ..Default::default()
            },
            range,
        ));
    }
    let mut report = terrain_los_at_heights(
        map,
        BattleHexCoordinate {
            x: i32::from(observer.position.x),
            y: i32::from(observer.position.y),
        },
        BattleHexCoordinate {
            x: i32::from(target.position.x),
            y: i32::from(target.position.y),
        },
        (observer.eye, target.eye),
        (observer.height, target.height),
    )?;
    let tile = map.base_hex(i64::from(target.position.x), i64::from(target.position.y))?;
    if target.level > i32::from(tile.level()) + 2 {
        report.target_woods = 0;
    }
    Ok((report, range))
}

/// Trace a terrain hex from either supported unit class and report spatial range.
pub(super) fn unit_hex_los(
    world: &World,
    observer: ObjectId,
    target: BattleHexCoordinate,
) -> Result<(BattleTerrainLos, f64)> {
    let unit = unit_sight_point(world, observer)?;
    let map = &world.btech.maps()[&unit.position.map];
    let source = BattleHexCoordinate {
        x: i32::from(unit.position.x),
        y: i32::from(unit.position.y),
    };
    let tile = map.base_hex(i64::from(target.x), i64::from(target.y))?;
    let source_tile = map.base_hex(i64::from(source.x), i64::from(source.y))?;
    let altitude = unit
        .height
        .unwrap_or(f64::from(source_tile.standing_height()));
    let target_height = f64::from(tile.standing_height());
    let horizontal = unit.point.range(target.center())?;
    let distance = horizontal.hypot((altitude - target_height) / 5.0);
    if beyond_maximum_range(world, map, observer, None, distance) {
        return Ok((
            BattleTerrainLos {
                blocked: true,
                ..Default::default()
            },
            distance,
        ));
    }
    let target_eye = if tile.is_ice() && altitude + unit.eye >= 0.0 {
        0.0
    } else {
        0.1
    };
    let report = terrain_los_with_endpoint(
        map,
        source,
        target,
        (unit.eye, target_eye),
        (Some(altitude), Some(target_height)),
        tile.is_ice() && altitude + unit.eye >= 0.0,
    )?;
    Ok((report, distance))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BattleHex, Terrain};
    use std::sync::Arc;

    /// Empty-hex LOS has only an observer hardware exception, even on maps with a larger ceiling.
    #[test]
    fn coordinate_los_uses_the_same_cutoff_and_retains_range_when_blocked() {
        for radar in [false, true] {
            for maximum in [10, 200] {
                let mut world = World::default();
                let mut map = lane(&[(Terrain::Grassland, 0); 201]);
                map.maximum_visibility = maximum;
                world.btech.maps.insert(ObjectId(99), map);
                let mut template = crate::BattleTemplate::parse(
                    "JR7-D",
                    include_str!("../../tests/fixtures/btech/mechs/JR7-D.toml"),
                )
                .unwrap();
                if radar {
                    template
                        .attributes
                        .insert("specials".into(), "AntiAircraft".into());
                }
                let mut unit = crate::BattleUnit::from_template(template).unwrap();
                unit.position = Some(crate::BattlePosition {
                    map: ObjectId(99),
                    x: 0,
                    y: 0,
                });
                unit.motion = Some(crate::BattleMotion::stationary(
                    BattleHexCoordinate { x: 0, y: 0 }.center(),
                ));
                world.btech.constructed.insert(ObjectId(1), unit);
                let before = world.btech.clone();
                for y in [10, 11, 180, 181, 200] {
                    let (los, distance) =
                        unit_hex_los(&world, ObjectId(1), BattleHexCoordinate { x: 0, y }).unwrap();
                    assert_eq!(distance, f64::from(y));
                    assert_eq!(
                        los.blocked,
                        i64::from(y) > if radar { 180 } else { maximum }
                    );
                }
                assert_eq!(world.btech, before);
            }
        }
    }

    /// A north/south lane avoids ambiguous hex-edge crossings in terrain rule fixtures.
    fn lane(tiles: &[(Terrain, u8)]) -> StoredBattleMap {
        let mut map = StoredBattleMap {
            membership_extent: 0,
            building_parent: 0,
            cargo_transfer_point: None,
            linked_markers: Default::default(),
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
            movement_modifier: 100,
            light: 2,
            visibility: 30,
            maximum_visibility: 60,
            cloud_base: 200,
            sensor_flags: 0,
            wind_direction: 0,
            wind_speed: 0,
            fire_dice: None,
            decorations: Default::default(),
            static_decorations: Default::default(),
            terrain: None,
        };
        map.establish_terrain(Arc::new(
            tiles
                .iter()
                .flat_map(|&(terrain, elevation)| [BattleHex::new(terrain, elevation); 3])
                .collect(),
        ))
        .unwrap();
        map
    }

    fn sight(tiles: &[(Terrain, u8)]) -> BattleTerrainLos {
        ground_terrain_los(
            &lane(tiles),
            BattleHexCoordinate { x: 1, y: 0 },
            BattleHexCoordinate {
                x: 1,
                y: tiles.len() as i32 - 1,
            },
        )
        .unwrap()
    }

    #[test]
    fn cached_topology_still_reads_current_terrain_and_eye_heights() {
        use Terrain::*;
        let from = BattleHexCoordinate { x: 1, y: 0 };
        let to = BattleHexCoordinate { x: 1, y: 2 };
        super::super::los_trace::clear();
        assert!(!sight(&[(Grassland, 0); 3]).blocked);
        assert!(sight(&[(Grassland, 0), (Grassland, 2), (Grassland, 0)]).blocked);
        assert!(!sight(&[(Grassland, 0); 3]).blocked);
        let map = lane(&[(Grassland, 0), (Grassland, 1), (Grassland, 0)]);
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

    #[test]
    fn woods_and_obscurants_exclude_endpoints() {
        use Terrain::*;
        let report = sight(&[
            (HeavyForest, 0),
            (LightForest, 0),
            (HeavyForest, 0),
            (Smoke, 0),
            (Fire, 0),
            (Mountains, 0),
            (HeavyForest, 0),
        ]);
        assert_eq!(report.woods, 3);
        assert_eq!(report.target_woods, 2);
        assert!(report.smoke && report.fire);
        assert!(!report.blocked);
        let report = sight(&[(Fire, 0), (Grassland, 0), (Smoke, 0)]);
        assert!(!report.fire && !report.smoke);
        assert_eq!(report.woods, 0);
    }

    #[test]
    fn ridges_block_or_cover_and_high_sight_clears_woods() {
        use Terrain::*;
        assert!(sight(&[(Grassland, 0), (Grassland, 2), (Grassland, 0)]).blocked);
        let report = sight(&[(Grassland, 0), (Grassland, 1), (Grassland, 0)]);
        assert!(!report.blocked);
        assert!(report.partial_cover);
        assert_eq!(
            sight(&[(Grassland, 3), (HeavyForest, 0), (Grassland, 3)]).woods,
            0
        );
        // Equality with terrain blocks: the halfway eye height is exactly three.
        assert!(sight(&[(Grassland, 0), (Grassland, 3), (Grassland, 3)]).blocked);
    }

    #[test]
    fn underwater_paths_count_depth_and_reject_air_or_seafloor() {
        use Terrain::*;
        let report = sight(&[(Water, 3); 8]);
        assert!(!report.blocked);
        assert_eq!(report.water, 7);
        assert!(report.fire);
        assert!(!sight(&[(Water, 3), (Ice, 3), (Water, 3)]).blocked);
        assert!(sight(&[(Water, 3), (Grassland, 0), (Water, 3)]).blocked);
        assert!(sight(&[(Water, 3), (Water, 1), (Water, 3)]).blocked);
        assert!(sight(&[(Water, 3), (Water, 3), (Grassland, 0)]).blocked);
        let report = sight(&[(Grassland, 0), (Water, 1), (Water, 1)]);
        assert!(!report.blocked);
        assert!(report.partial_cover);
    }

    #[test]
    fn coordinate_ice_endpoint_is_visible_without_ignoring_intervening_ground() {
        let from = BattleHexCoordinate { x: 1, y: 0 };
        let to = BattleHexCoordinate { x: 1, y: 2 };
        for eye in [0.5, 1.5] {
            let clear = lane(&[
                (Terrain::Grassland, 0),
                (Terrain::Grassland, 0),
                (Terrain::Ice, 0),
            ]);
            assert!(
                !terrain_los_with_endpoint(&clear, from, to, (eye, 0.0), (None, None), true)
                    .unwrap()
                    .blocked
            );
            assert!(
                terrain_los_at_heights(&clear, from, to, (eye, 0.0), (None, None))
                    .unwrap()
                    .blocked
            );
            let ridge = lane(&[
                (Terrain::Grassland, 0),
                (Terrain::Grassland, 2),
                (Terrain::Ice, 0),
            ]);
            assert!(
                terrain_los_with_endpoint(&ridge, from, to, (eye, 0.0), (None, None), true)
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
            assert!(!sight(&[(Grassland, 0), (Ice, depth), (Ice, depth)]).blocked);
            assert!(!sight(&[(Ice, depth), (Ice, depth), (Grassland, 0)]).blocked);
            assert!(sight(&[(Ice, depth), (Grassland, 2), (Ice, depth)]).blocked);
            assert!(sight(&[(Ice, depth), (Water, 3), (Water, 3)]).blocked);
            assert!(sight(&[(Water, 3), (Water, 3), (Ice, depth)]).blocked);
        }
        let map = lane(&[(Ice, 9); 3]);
        let point = BattleHexCoordinate { x: 1, y: 0 };
        assert!(!ground_terrain_los(&map, point, point).unwrap().blocked);
    }

    #[test]
    fn bridge_decks_and_underwater_spans_use_endpoint_altitudes() {
        use Terrain::*;
        for height in [0, 1, 3, 9] {
            assert!(!sight(&[(Bridge, height); 3]).blocked);
            assert!(!sight(&[(Grassland, height), (Bridge, height), (Bridge, height)]).blocked);
            assert!(!sight(&[(Bridge, height), (Bridge, height), (Grassland, height)]).blocked);
        }
        assert!(sight(&[(Bridge, 0), (Grassland, 2), (Bridge, 0)]).blocked);
        assert!(!sight(&[(Bridge, 0), (Bridge, 9), (Bridge, 0)]).blocked);
        assert_eq!(
            sight(&[(Bridge, 0), (HeavyForest, 0), (Bridge, 0)]).woods,
            2
        );
        let map = lane(&[(Bridge, 9); 3]);
        let source = BattleHexCoordinate { x: 1, y: 0 };
        let target = BattleHexCoordinate { x: 1, y: 2 };
        let submerged =
            ground_posture_los(&map, source, target, false, false, (Some(-3.0), Some(-3.0)))
                .unwrap();
        assert!(!submerged.blocked);
        assert_eq!(submerged.water, 2);
        assert!(
            ground_posture_los(&map, source, target, false, false, (Some(-3.0), None))
                .unwrap()
                .blocked
        );
    }

    #[test]
    fn same_hex_is_clear_and_out_of_map_endpoints_return_errors() {
        use Terrain::*;
        let map = lane(&[(HeavyForest, 0)]);
        let point = BattleHexCoordinate { x: 1, y: 0 };
        let report = ground_terrain_los(&map, point, point).unwrap();
        assert!(!report.blocked);
        assert_eq!(report.woods, 0);
        assert_eq!(report.target_woods, 2);
        assert!(ground_terrain_los(&map, point, BattleHexCoordinate { x: 1, y: 1 }).is_err());
        assert!(
            !ground_terrain_los(&lane(&[(Bridge, 1)]), point, point)
                .unwrap()
                .blocked
        );
    }
}
