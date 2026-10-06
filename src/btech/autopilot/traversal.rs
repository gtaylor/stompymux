//! Read-only ground traversal admission for the autopilot.
//!
//! This module deliberately sits below planning and above the movement engine.  It
//! describes whether one adjacent hex may be entered and gives the planner a
//! deterministic positive cost.  It does not roll a control check, inspect mine
//! records, or mutate a unit.  The heartbeat must still revalidate the selected
//! segment through the ordinary movement transaction before committing it.

use super::super::{
    GroundMovement, Hex, HexCoordinate, MapFlag, Mech, Position, StructureKind, VehicleMovement,
};
use crate::{ObjectId, World};
use std::collections::BTreeMap;

/// Per-hex `(occupants, friendly occupants)` counts keyed by `(x, y)`.
type OccupancyCounts = BTreeMap<(u16, u16), (usize, usize)>;

/// A reason for an assessment.  Reasons on an eligible transition describe the
/// principal terrain or occupancy penalty; `Eligible` means no special penalty
/// was needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TraversalReason {
    Eligible,
    OutOfBounds,
    WrongMap,
    MapUnavailable,
    UnitUnavailable,
    UnitNotPlaced,
    NotAdjacent,
    ImpassableTerrain,
    ElevationTooSteep,
    OccupancyUnknown,
    Congested,
    Occupied,
    WaterRisk,
    IceRisk,
    BridgeRisk,
    KnownHazard,
}

/// The result of assessing a single adjacent transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraversalAssessment {
    /// Whether the movement executor may attempt this transition.
    pub eligible: bool,
    /// Positive deterministic route cost when eligible, or zero when blocked.
    pub cost: u32,
    /// Why the transition was admitted or rejected.
    pub reason: TraversalReason,
}

impl TraversalAssessment {
    const fn blocked(reason: TraversalReason) -> Self {
        Self {
            eligible: false,
            cost: 0,
            reason,
        }
    }

    const fn allowed(cost: u32, reason: TraversalReason) -> Self {
        Self {
            eligible: true,
            cost: if cost == 0 { 1 } else { cost },
            reason,
        }
    }
}

/// Assess one ground transition using only currently known world state.
///
/// `from` may be a planner cursor rather than the unit's currently committed
/// coordinate, which allows a search to assess its discovered frontier.  The
/// unit must nevertheless be placed on the same map, and `from` and `to` must
/// be adjacent.  A caller that has found a route must revalidate its next
/// segment immediately before driving it.
pub fn assess(
    world: &World,
    unit_id: ObjectId,
    from: Position,
    to: Position,
) -> TraversalAssessment {
    assess_with_occupancy(world, unit_id, from, to, None)
}

fn assess_with_occupancy(
    world: &World,
    unit_id: ObjectId,
    from: Position,
    to: Position,
    cached_occupancy: Option<&OccupancyCounts>,
) -> TraversalAssessment {
    if from.map != to.map {
        return TraversalAssessment::blocked(TraversalReason::WrongMap);
    }
    if from.x == to.x && from.y == to.y {
        return TraversalAssessment::blocked(TraversalReason::NotAdjacent);
    }
    let from_coordinate = HexCoordinate {
        x: i32::from(from.x),
        y: i32::from(from.y),
    };
    let to_coordinate = HexCoordinate {
        x: i32::from(to.x),
        y: i32::from(to.y),
    };
    if from_coordinate.distance(to_coordinate) != 1 {
        return TraversalAssessment::blocked(TraversalReason::NotAdjacent);
    }

    let Some(map) = world.btech.maps().get(&from.map) else {
        return TraversalAssessment::blocked(TraversalReason::MapUnavailable);
    };
    let Some(unit_kind) = unit_kind(world, unit_id) else {
        return TraversalAssessment::blocked(TraversalReason::UnitUnavailable);
    };
    let Some(current_position) = unit_position(world, unit_id) else {
        return TraversalAssessment::blocked(TraversalReason::UnitNotPlaced);
    };
    if current_position.map != from.map {
        return TraversalAssessment::blocked(TraversalReason::WrongMap);
    }

    let Ok(from_tile) = map.hex(i64::from(from.x), i64::from(from.y)) else {
        return if in_bounds(map.width, map.height, from.x, from.y) {
            TraversalAssessment::blocked(TraversalReason::MapUnavailable)
        } else {
            TraversalAssessment::blocked(TraversalReason::OutOfBounds)
        };
    };
    let Ok(to_tile) = map.hex(i64::from(to.x), i64::from(to.y)) else {
        return if in_bounds(map.width, map.height, to.x, to.y) {
            TraversalAssessment::blocked(TraversalReason::MapUnavailable)
        } else {
            TraversalAssessment::blocked(TraversalReason::OutOfBounds)
        };
    };

    let Some(mut cost) = terrain_cost(to_tile, unit_kind) else {
        return TraversalAssessment::blocked(TraversalReason::ImpassableTerrain);
    };
    let (from_height, to_height) = support_heights(world, unit_id, unit_kind, from_tile, to_tile);
    let change = to_height - from_height;
    let maximum_change = match unit_kind {
        GroundUnitKind::Mech => 2,
        GroundUnitKind::Tracked | GroundUnitKind::Wheeled | GroundUnitKind::Hover => 1,
    };
    if change.abs() > maximum_change {
        return TraversalAssessment::blocked(TraversalReason::ElevationTooSteep);
    }
    cost = cost.saturating_add(change.unsigned_abs().saturating_mul(10));

    let (reason, hazard_cost) = transition_reason(unit_kind, from_tile, to_tile, to_height);
    cost = cost.saturating_add(hazard_cost);
    // Without stacking collisions, other units neither block nor slow a route.
    if map.has_flag(MapFlag::NoStacking) {
        return TraversalAssessment::allowed(cost, reason);
    }

    let (occupants, friendly) = if let Some(cached) = cached_occupancy {
        cached.get(&(to.x, to.y)).copied().unwrap_or_default()
    } else {
        match occupancy(world, from.map, to_coordinate, unit_id) {
            Ok(value) => value,
            Err(()) => {
                return TraversalAssessment::blocked(TraversalReason::OccupancyUnknown);
            }
        }
    };
    // Native stacking resolves crowded hexes rather than treating every unit as
    // an immutable wall.  Admit light congestion with a penalty, but do not
    // ask an autonomous unit to create a known collision crowd.
    if crowded((occupants, friendly)) {
        return TraversalAssessment::blocked(TraversalReason::Congested);
    }
    if occupants == 0 {
        return TraversalAssessment::allowed(cost, reason);
    }
    cost = cost.saturating_add(occupants.saturating_mul(4) as u32);
    TraversalAssessment::allowed(cost, TraversalReason::Occupied)
}

/// Adapter for [`crate::btech::autopilot::navigation::Traversal`].
///
/// Keeping this small wrapper in the integration layer means the planner can
/// continue to depend on its generic trait while this assessment remains a
/// typed world-facing API.
pub struct GroundTraversal<'a> {
    pub world: &'a World,
    pub unit_id: ObjectId,
    pub map: ObjectId,
    occupancy: OccupancyCounts,
    congested: std::cell::RefCell<super::congestion::Cells>,
}

impl<'a> GroundTraversal<'a> {
    /// Whether this search step actually encountered a temporarily crowded edge.
    pub(crate) fn congested_cells(&self) -> super::congestion::Cells {
        self.congested.borrow().clone()
    }

    /// Build a traversal provider with one stable occupancy snapshot.  A* calls
    /// this provider once per edge, so rebuilding map membership or sorting all
    /// battlefield slots in that hot path would violate the heartbeat budget.
    /// Maps without stacking collisions need no snapshot.
    pub fn new(world: &'a World, unit_id: ObjectId, map: ObjectId) -> Self {
        let stacking = world
            .btech
            .maps()
            .get(&map)
            .is_some_and(|stored| !stored.has_flag(MapFlag::NoStacking));
        Self {
            world,
            unit_id,
            map,
            occupancy: if stacking {
                known_occupancy(world, unit_id, map)
            } else {
                OccupancyCounts::new()
            },
            congested: Default::default(),
        }
    }
}

impl super::navigation::Traversal for GroundTraversal<'_> {
    fn traversal_cost(
        &self,
        from: super::navigation::GridHex,
        to: super::navigation::GridHex,
    ) -> Option<u32> {
        let assessment = assess_with_occupancy(
            self.world,
            self.unit_id,
            Position {
                map: self.map,
                x: from.x,
                y: from.y,
            },
            Position {
                map: self.map,
                x: to.x,
                y: to.y,
            },
            Some(&self.occupancy),
        );
        if assessment.reason == TraversalReason::Congested {
            super::congestion::remember(&mut self.congested.borrow_mut(), (to.x, to.y));
        }
        assessment.eligible.then_some(assessment.cost)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroundUnitKind {
    Mech,
    Tracked,
    Wheeled,
    Hover,
}

impl GroundUnitKind {
    /// How the map's terrain rules see this kind of unit move.
    fn ground_movement(self) -> GroundMovement {
        match self {
            Self::Mech => GroundMovement::Legged,
            Self::Tracked => GroundMovement::Tracked,
            Self::Wheeled => GroundMovement::Wheeled,
            Self::Hover => GroundMovement::Hover,
        }
    }
}

fn unit_kind(world: &World, id: ObjectId) -> Option<GroundUnitKind> {
    if let Some(unit) = world.btech.constructed_units().get(&id) {
        return (!unit.is_destroyed() && !unit.airborne()).then_some(GroundUnitKind::Mech);
    }
    let vehicle = world.btech.vehicles().get(&id)?;
    if vehicle.is_destroyed() || vehicle.free_fall().is_some() {
        return None;
    }
    match vehicle.definition().movement {
        VehicleMovement::Tracked => Some(GroundUnitKind::Tracked),
        VehicleMovement::Wheeled => Some(GroundUnitKind::Wheeled),
        VehicleMovement::Hover => Some(GroundUnitKind::Hover),
        VehicleMovement::Stationary | VehicleMovement::Vtol => None,
    }
}

fn unit_position(world: &World, id: ObjectId) -> Option<Position> {
    world
        .btech
        .constructed_units()
        .get(&id)
        .and_then(Mech::position)
        .or_else(|| {
            world
                .btech
                .vehicles()
                .get(&id)
                .and_then(|unit| unit.position())
        })
}

fn in_bounds(width: i64, height: i64, x: u16, y: u16) -> bool {
    i64::from(x) < width && i64::from(y) < height
}

/// Relative route cost of entering a tile, or `None` when the tile is impassable. Liquid
/// magma destroys any vehicle that enters it, so vehicles never route through it.
fn terrain_cost(hex: Hex, kind: GroundUnitKind) -> Option<u32> {
    if hex
        .structure()
        .is_some_and(|structure| structure.kind == StructureKind::Wall)
    {
        return None;
    }
    if kind != GroundUnitKind::Mech && hex.in_liquid_magma(i32::from(hex.level())) {
        return None;
    }
    Some(if hex.is_water_surface() {
        3
    } else {
        // Elsewhere a route prefers terrain in proportion to the speed it costs, which counts
        // any fire or smoke over it.
        hex.ground_speed_divisor(kind.ground_movement()) as u32
    })
}

fn support_heights(
    world: &World,
    id: ObjectId,
    kind: GroundUnitKind,
    from: Hex,
    to: Hex,
) -> (i32, i32) {
    let from_height = match kind {
        GroundUnitKind::Mech => world.btech.constructed_units()[&id].elevation_level(from),
        GroundUnitKind::Tracked | GroundUnitKind::Wheeled | GroundUnitKind::Hover => {
            world.btech.vehicles()[&id].elevation_level(from)
        }
    };
    let to_water_line = i32::from(to.water_line());
    let mut to_height = i32::from(to.standing_height());
    if to.is_ice() && from_height < to_water_line {
        to_height = i32::from(to.surface_height());
    }
    if to.has_bridge() && from_height < i32::from(to.standing_height()) - 2 {
        to_height = to_water_line - 1;
    }
    if kind == GroundUnitKind::Hover && to.is_water_surface() {
        to_height = to_water_line;
    }
    if kind == GroundUnitKind::Hover
        && to.deck_clearance().is_some_and(|deck| deck >= 2)
        && from_height == i32::from(from.water_line())
        && (from.is_open_water() || from.is_ice() || world.btech.vehicles()[&id].under_bridge())
    {
        to_height = to_water_line;
    }
    (from_height, to_height)
}

fn transition_reason(
    kind: GroundUnitKind,
    from: Hex,
    to: Hex,
    to_height: i32,
) -> (TraversalReason, u32) {
    if to.overlay().is_some() {
        return (TraversalReason::KnownHazard, 8);
    }
    if to.is_ice() {
        return (TraversalReason::IceRisk, 8);
    }
    if to.in_liquid_magma(to_height) {
        return (TraversalReason::KnownHazard, 12);
    }
    if to.on_magma_crust(to_height) {
        return (TraversalReason::KnownHazard, 4);
    }
    if to.has_bridge() {
        let under_bridge = to_height < i32::from(to.water_line())
            || (kind == GroundUnitKind::Hover && from.has_bridge());
        return (
            if under_bridge {
                TraversalReason::BridgeRisk
            } else {
                TraversalReason::Eligible
            },
            if under_bridge { 8 } else { 1 },
        );
    }
    if to.is_open_water() {
        return (TraversalReason::WaterRisk, 12);
    }
    if from.is_water_surface() && matches!(kind, GroundUnitKind::Tracked | GroundUnitKind::Wheeled)
    {
        return (TraversalReason::WaterRisk, 4);
    }
    (TraversalReason::Eligible, 0)
}

fn occupancy(
    world: &World,
    map: ObjectId,
    coordinate: HexCoordinate,
    moving: ObjectId,
) -> Result<(usize, usize), ()> {
    let occupants =
        super::super::map_slots::hex_occupants(world, map, coordinate).map_err(|_| ())?;
    let moving_team = team(world, moving).ok_or(())?;
    let mut count = 0;
    let mut friendly = 0;
    for occupant in occupants {
        if occupant == moving || !known_occupant(world, moving, occupant) {
            continue;
        }
        count += 1;
        if team(world, occupant) == Some(moving_team) {
            friendly += 1;
        }
    }
    Ok((count, friendly))
}

/// The exact occupancy predicate shared by planning and clearance polling.
pub(crate) fn crowded((occupants, friendly): (usize, usize)) -> bool {
    occupants > 6 || friendly > 2
}

/// Check only relevant cells with the same observation boundary as navigation.
pub(crate) fn watched_clearance(
    world: &World,
    moving: ObjectId,
    map: ObjectId,
    cells: &super::congestion::Cells,
) -> bool {
    let occupancy = filtered_occupancy(world, moving, map, Some(cells));
    cells
        .iter()
        .any(|cell| !crowded(occupancy.get(cell).copied().unwrap_or_default()))
}

/// Snapshot only occupancy that the moving unit is allowed to know.  Friendly
/// units are known from their team membership; enemy placement affects routing
/// only after the moving unit has an acquired sensor contact for that unit.
fn known_occupancy(world: &World, moving: ObjectId, map: ObjectId) -> OccupancyCounts {
    filtered_occupancy(world, moving, map, None)
}

fn filtered_occupancy(
    world: &World,
    moving: ObjectId,
    map: ObjectId,
    cells: Option<&super::congestion::Cells>,
) -> OccupancyCounts {
    let Some(moving_team) = team(world, moving) else {
        return BTreeMap::new();
    };
    let reader = std::cell::OnceCell::new();
    let mut result = BTreeMap::new();
    for (id, position) in world
        .btech
        .constructed_units()
        .iter()
        .filter_map(|(&id, unit)| unit.position().map(|position| (id, position)))
        .chain(
            world
                .btech
                .vehicles()
                .iter()
                .filter_map(|(&id, unit)| unit.position().map(|position| (id, position))),
        )
    {
        if id == moving
            || position.map != map
            || cells.is_some_and(|cells| !cells.contains(&(position.x, position.y)))
            || !known_occupant_with_team(world, &reader, moving, moving_team, id)
        {
            continue;
        }
        let entry = result.entry((position.x, position.y)).or_insert((0, 0));
        entry.0 += 1;
        if team(world, id) == Some(moving_team) {
            entry.1 += 1;
        }
    }
    result
}

pub(super) fn known_occupant(world: &World, moving: ObjectId, occupant: ObjectId) -> bool {
    let Some(moving_team) = team(world, moving) else {
        return false;
    };
    known_occupant_with_team(
        world,
        &std::cell::OnceCell::new(),
        moving,
        moving_team,
        occupant,
    )
}

/// Enemy occupants are checked through one lazily built contact reader, so an occupancy
/// snapshot shares a single perception profile for the moving unit.
fn known_occupant_with_team<'w>(
    world: &'w World,
    reader: &std::cell::OnceCell<Option<super::super::contacts::ContactReader<'w>>>,
    moving: ObjectId,
    moving_team: i32,
    occupant: ObjectId,
) -> bool {
    if team(world, occupant) == Some(moving_team) {
        return active_occupant(world, occupant);
    }
    // Acquisition flags can outlive visibility until the next scanner update.
    // Use the same live facts as observations, including only observable
    // destruction, rather than reading an unseen enemy's current condition.
    reader
        .get_or_init(|| super::super::contacts::ContactReader::new(world, moving).ok())
        .as_ref()
        .and_then(|reader| reader.facts(occupant).ok().flatten())
        .is_some_and(|contact| !contact.known_destroyed)
}

fn active_occupant(world: &World, id: ObjectId) -> bool {
    if let Some(unit) = world.btech.constructed_units().get(&id) {
        return !unit.is_destroyed() && !unit.airborne();
    }
    world
        .btech
        .vehicles()
        .get(&id)
        .is_some_and(|unit| !unit.is_destroyed() && unit.free_fall().is_none())
}

fn team(world: &World, id: ObjectId) -> Option<i32> {
    world
        .btech
        .constructed_units()
        .get(&id)
        .map(|unit| unit.signature().team)
        .or_else(|| {
            world
                .btech
                .vehicles()
                .get(&id)
                .map(|unit| unit.signature().team)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::Terrain;
    use crate::btech::UnitTemplateExt;

    #[test]
    fn terrain_costs_are_positive_and_walls_are_blocked() {
        for terrain in [
            Terrain::Clear,
            Terrain::Road,
            Terrain::LightWoods,
            Terrain::HeavyWoods,
            Terrain::Water,
            Terrain::Ice,
            Terrain::Bridge,
            Terrain::Rough,
            Terrain::UltraRough,
            Terrain::Fire,
            Terrain::Smoke,
            Terrain::DeepSnow,
            Terrain::Building,
            Terrain::Sand,
        ] {
            assert!(
                terrain_cost(Hex::new(terrain, 0), GroundUnitKind::Mech)
                    .is_some_and(|cost| cost > 0)
            );
        }
        assert_eq!(
            terrain_cost(Hex::new(Terrain::Wall, 0), GroundUnitKind::Mech),
            None
        );
    }

    /// Wheeled routes avoid sand; other ground units price it like clear terrain.
    #[test]
    fn sand_costs_extra_only_for_wheeled_units() {
        assert_eq!(
            terrain_cost(Hex::new(Terrain::Sand, 0), GroundUnitKind::Wheeled),
            Some(2)
        );
        for kind in [
            GroundUnitKind::Mech,
            GroundUnitKind::Tracked,
            GroundUnitKind::Hover,
        ] {
            assert_eq!(
                terrain_cost(Hex::new(Terrain::Sand, 0), kind),
                terrain_cost(Hex::new(Terrain::Clear, 0), kind)
            );
        }
    }

    #[test]
    fn transition_risks_are_explicit_and_do_not_query_mines() {
        let (reason, _) = transition_reason(
            GroundUnitKind::Tracked,
            Hex::new(Terrain::Clear, 0),
            Hex::new(Terrain::Water, 1),
            -1,
        );
        assert_eq!(reason, TraversalReason::WaterRisk);

        let (reason, _) = transition_reason(
            GroundUnitKind::Hover,
            Hex::new(Terrain::Water, 1),
            Hex::new(Terrain::Ice, 1),
            0,
        );
        assert_eq!(reason, TraversalReason::IceRisk);
    }

    #[test]
    fn stale_acquisition_cannot_reveal_hidden_movement_or_destruction() {
        use crate::{Config, Contact, Kind, MechSection, Power, UnitTemplate};
        let config = Config::load("tests/fixtures/game").unwrap();
        let mut world = World::default();
        let map = world.create(&config, "Occupancy map".into(), Kind::Room);
        crate::create_battle_map(
            &mut world,
            map,
            "occupancy",
            crate::MapAsset::from_cells("1 5\n.0\n.0\n.0\n.0\n.0\n").unwrap(),
        )
        .unwrap();
        let observer = world.create(&config, "Observer".into(), Kind::Thing);
        let target = world.create(&config, "Enemy".into(), Kind::Thing);
        for (id, y) in [(observer, 0), (target, 2)] {
            world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
            UnitTemplate::parse(
                "JR7-D",
                include_str!("../../../tests/fixtures/btech/units/JR7-D.toml"),
            )
            .unwrap()
            .create(&mut world, id)
            .unwrap();
            crate::place_battle_unit(&mut world, id, map, 0, y).unwrap();
            world.btech.constructed.get_mut(&id).unwrap().power = Power::Running;
        }
        crate::set_battle_unit_signature(
            &mut world,
            target,
            crate::UnitSignature {
                team: 1,
                ..Default::default()
            },
        )
        .unwrap();
        world
            .btech
            .constructed
            .get_mut(&observer)
            .unwrap()
            .contacts
            .insert(target, Contact { identified: true });
        assert_eq!(
            known_occupancy(&world, observer, map).get(&(0, 2)),
            Some(&(1, 0))
        );
        world
            .btech
            .constructed
            .get_mut(&target)
            .unwrap()
            .visibility
            .invisible = true;
        assert!(known_occupancy(&world, observer, map).is_empty());
        let watched = [(0, 2), (0, 3)].into_iter().collect();
        assert!(watched_clearance(&world, observer, map, &watched));
        assert!(filtered_occupancy(&world, observer, map, Some(&watched)).is_empty());
        assert!(!known_occupant(&world, observer, target));
        world.btech.constructed.get_mut(&target).unwrap().power = Power::Off;
        crate::place_battle_unit(&mut world, target, map, 0, 3).unwrap();
        world.btech.constructed.get_mut(&target).unwrap().power = Power::Running;
        assert!(known_occupancy(&world, observer, map).is_empty());
        let watched = [(0, 2), (0, 3)].into_iter().collect();
        assert!(watched_clearance(&world, observer, map, &watched));
        assert!(filtered_occupancy(&world, observer, map, Some(&watched)).is_empty());
        world
            .btech
            .constructed
            .get_mut(&target)
            .unwrap()
            .sections
            .get_mut(&MechSection::CenterTorso)
            .unwrap()
            .internal = 0;
        assert!(known_occupancy(&world, observer, map).is_empty());
        let watched = [(0, 2), (0, 3)].into_iter().collect();
        assert!(watched_clearance(&world, observer, map, &watched));
        assert!(filtered_occupancy(&world, observer, map, Some(&watched)).is_empty());
        assert!(!known_occupant(&world, observer, target));
    }

    /// A friendly crowd blocks a route until the map switches stacking off, after which
    /// the crowded hex costs the same as an empty one.
    #[test]
    fn no_stacking_maps_route_through_crowds() {
        use crate::btech::autopilot::navigation::{GridHex, Traversal};
        use crate::{Config, Kind, Power, UnitTemplate};
        let config = Config::load("tests/fixtures/game").unwrap();
        let mut world = World::default();
        let map = world.create(&config, "Crowded map".into(), Kind::Room);
        crate::create_battle_map(
            &mut world,
            map,
            "crowded",
            crate::MapAsset::from_cells("1 3\n.0\n.0\n.0\n").unwrap(),
        )
        .unwrap();
        let mut units = Vec::new();
        for y in [0, 1, 1, 1] {
            let id = world.create(&config, "Jenner".into(), Kind::Thing);
            world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
            UnitTemplate::parse(
                "JR7-D",
                include_str!("../../../tests/fixtures/btech/units/JR7-D.toml"),
            )
            .unwrap()
            .create(&mut world, id)
            .unwrap();
            crate::place_battle_unit(&mut world, id, map, 0, y).unwrap();
            world.btech.constructed.get_mut(&id).unwrap().power = Power::Running;
            units.push(id);
        }
        let mover = units[0];
        let at = |y| Position { map, x: 0, y };
        let (from, crowd) = (GridHex::new(0, 0), GridHex::new(0, 1));
        let crowded = assess(&world, mover, at(0), at(1));
        assert!(!crowded.eligible);
        assert_eq!(crowded.reason, TraversalReason::Congested);
        assert_eq!(
            GroundTraversal::new(&world, mover, map).traversal_cost(from, crowd),
            None
        );

        world
            .btech
            .maps
            .get_mut(&map)
            .unwrap()
            .set_flag(MapFlag::NoStacking, true);
        let open = assess(&world, mover, at(0), at(1));
        assert!(open.eligible);
        assert_eq!(open.reason, TraversalReason::Eligible);
        assert_eq!(open, assess(&world, mover, at(1), at(2)));
        let traversal = GroundTraversal::new(&world, mover, map);
        assert!(traversal.occupancy.is_empty());
        assert_eq!(traversal.traversal_cost(from, crowd), Some(open.cost));
        assert!(traversal.congested_cells().is_empty());
    }

    #[test]
    fn support_height_respects_ice_and_bridge_surfaces() {
        let source = Hex::new(Terrain::Water, 2);
        let ice = Hex::new(Terrain::Ice, 2);
        let bridge = Hex::new(Terrain::Bridge, 4);
        assert_eq!(ice.surface_height(), -2);
        assert_eq!(bridge.standing_height(), 4);
        // The pure map values establish the same surfaces used by the
        // authoritative movement adapters; world-dependent heights are tested
        // through assess() by the integration suite.
        assert_ne!(source.surface_height(), ice.standing_height());
    }
}
