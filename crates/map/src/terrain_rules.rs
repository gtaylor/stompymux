//! Shared terrain rule lookups on a hex's layers: open ground, foliage density, water and
//! ground movement cost.
//!
//! Combat, movement and sensor code ask these questions of a [`Hex`] instead of matching on
//! layers locally, so a rule changes in exactly one place. Movement costs follow Tactical
//! Operations' expanded movement cost table: each hex costs one point plus what its terrain
//! adds, and the realtime game divides a unit's speed by that total.
use crate::{Condition, Density, Flow, Foliage, Ground, Hex, Route};
use serde::{Deserialize, Serialize};

/// How a ground unit moves, which decides what terrain slows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroundMovement {
    /// ’Mechs, ProtoMechs and battle armor walking on legs.
    Legged,
    Tracked,
    Wheeled,
    Hover,
}

impl Hex {
    /// Whether nothing stands on or covers the ground: no foliage, water or structure.
    pub fn is_bare(self) -> bool {
        self.foliage().is_none() && self.water().is_none() && self.structure().is_none()
    }

    /// Bare clear ground, pavement, sand or tundra, or a road over one, with no weather
    /// condition, fire or smoke: firm, open ground a unit can set down on.
    pub fn is_open_ground(self) -> bool {
        self.is_bare()
            && self.overlay().is_none()
            && self.condition().is_none()
            && self.route() != Some(Route::Rail)
            && matches!(
                self.ground(),
                Ground::Clear | Ground::Pavement | Ground::Sand | Ground::Tundra
            )
    }

    /// Whether woods or jungle cover this hex; planted fields do not count.
    pub fn is_woods(self) -> bool {
        self.foliage().and_then(Foliage::density).is_some()
    }

    /// Woods or jungle density used by to-hit modifiers and line of sight: light 1, heavy 2,
    /// ultra-heavy 3, otherwise 0.
    pub fn woods_density(self) -> u8 {
        self.foliage()
            .and_then(Foliage::density)
            .map_or(0, Density::points)
    }

    /// Levels the hex's foliage rises above its ground, or zero without foliage.
    pub fn foliage_height(self) -> u8 {
        self.foliage().map_or(0, Foliage::height)
    }

    /// Whether this hex holds water, including under ice or a bridge.
    pub fn holds_water(self) -> bool {
        self.water().is_some()
    }

    /// Whether rough footing makes boulders and rocks to hide behind or crash into: rough,
    /// ultra rough or rubble ground with nothing standing on it.
    pub fn is_rocky(self) -> bool {
        self.is_bare()
            && matches!(
                self.ground(),
                Ground::Rough | Ground::UltraRough | Ground::Rubble | Ground::UltraRubble
            )
    }

    /// What this hex's ground, foliage and route add to a ground unit's cost to enter it. A
    /// road replaces the terrain it runs through, and a rail line counts as rough ground for
    /// everything but hovercraft.
    fn terrain_cost(self, movement: GroundMovement) -> u8 {
        let hover = movement == GroundMovement::Hover;
        match self.route() {
            Some(route) if route.is_road() => return 0,
            Some(Route::Rail) if !hover => return 1,
            _ => {}
        }
        let ground = match (self.ground(), movement) {
            (Ground::Rough | Ground::Rubble, _) => 1,
            (Ground::UltraRough | Ground::UltraRubble, _) => 2,
            (Ground::Sand, GroundMovement::Wheeled) => 1,
            (Ground::Swamp, GroundMovement::Legged) => 1,
            (Ground::Swamp, GroundMovement::Hover) => 0,
            (Ground::Swamp, _) => 2,
            (Ground::Magma | Ground::HeavyIndustrial, GroundMovement::Legged) => 1,
            (Ground::Magma, _) => 1,
            _ => 0,
        };
        let foliage = match self.foliage() {
            Some(Foliage::LightWoods) => 1,
            Some(Foliage::HeavyWoods | Foliage::LightJungle) => 2,
            Some(Foliage::UltraHeavyWoods | Foliage::HeavyJungle) => 3,
            Some(Foliage::UltraHeavyJungle) => 4,
            Some(Foliage::PlantedFields) | None => 0,
        };
        ground.max(foliage)
    }

    /// What this hex's ice, snow or mud adds to a ground unit's cost; hovercraft skim over all
    /// of them.
    fn condition_cost(self, movement: GroundMovement) -> u8 {
        if movement == GroundMovement::Hover {
            return 0;
        }
        match self.condition() {
            // Frozen water is crossed on the ice sheet or walked beneath; only ice-coated
            // ground slows a unit.
            Some(Condition::Ice) if self.holds_water() => 0,
            Some(Condition::Ice | Condition::DeepSnow | Condition::Mud) => 1,
            Some(Condition::ThinSnow) if movement == GroundMovement::Wheeled => 1,
            _ => 0,
        }
    }

    /// Movement points a ground unit spends to enter this hex, before any change of level:
    /// one, plus what the terrain and its weather condition add. Water costs only what its
    /// current adds; wading and swimming are governed by the water rules. Structures are
    /// crossed on their roofs or decks like roads. Fire and smoke make any hex cost at least two.
    pub fn movement_cost(self, movement: GroundMovement) -> u8 {
        let terrain = match (self.structure(), self.water()) {
            (Some(_), _) => 0,
            (None, Some(water)) if movement != GroundMovement::Hover => match water.flow {
                Flow::Still => 0,
                Flow::Rapids => 1,
                Flow::Torrent => 2,
            },
            (None, Some(_)) => 0,
            (None, None) => self.terrain_cost(movement),
        };
        let cost = 1 + terrain + self.condition_cost(movement);
        if self.overlay().is_some() {
            return cost.max(2);
        }
        cost
    }

    /// Divisor this hex applies to a ground unit's desired speed: its movement cost.
    pub fn ground_speed_divisor(self, movement: GroundMovement) -> f64 {
        f64::from(self.movement_cost(movement))
    }

    /// Whether a unit standing at `level` relative to this hex is in water: below the
    /// surface datum of a water column.
    pub fn immerses(self, level: i32) -> bool {
        self.holds_water() && level < i32::from(self.level())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DecorationKind, Terrain, Water};
    use GroundMovement::{Hover, Legged, Tracked, Wheeled};

    #[test]
    fn woods_density_matches_foliage_layer() {
        let density = |terrain| Hex::new(terrain, 0).woods_density();
        for terrain in [
            Terrain::Clear,
            Terrain::Smoke,
            Terrain::Rough,
            Terrain::PlantedFields,
        ] {
            assert_eq!(density(terrain), 0);
            assert!(!Hex::new(terrain, 0).is_woods());
        }
        assert_eq!(density(Terrain::LightWoods), 1);
        assert_eq!(density(Terrain::HeavyJungle), 2);
        assert_eq!(density(Terrain::UltraHeavyWoods), 3);
        let burning = Hex::new(Terrain::HeavyWoods, 0).with_overlay(Some(DecorationKind::Fire));
        assert_eq!(burning.woods_density(), 2);
        assert_eq!(Hex::new(Terrain::PlantedFields, 0).foliage_height(), 1);
    }

    /// Costs follow the expanded movement cost table.
    #[test]
    fn movement_costs_follow_the_expanded_table() {
        let cost = |terrain, movement| Hex::new(terrain, 0).movement_cost(movement);
        assert_eq!(cost(Terrain::Sand, Wheeled), 2);
        assert_eq!(cost(Terrain::Sand, Legged), 1);
        for movement in [Legged, Tracked, Wheeled] {
            assert_eq!(cost(Terrain::Clear, movement), 1);
            assert_eq!(cost(Terrain::Rough, movement), 2);
            assert_eq!(cost(Terrain::UltraRough, movement), 3);
            assert_eq!(cost(Terrain::Rubble, movement), 2);
            assert_eq!(cost(Terrain::UltraRubble, movement), 3);
            assert_eq!(cost(Terrain::LightWoods, movement), 2);
            assert_eq!(cost(Terrain::HeavyWoods, movement), 3);
            assert_eq!(cost(Terrain::UltraHeavyWoods, movement), 4);
            assert_eq!(cost(Terrain::LightJungle, movement), 3);
            assert_eq!(cost(Terrain::HeavyJungle, movement), 4);
            assert_eq!(cost(Terrain::UltraHeavyJungle, movement), 5);
            assert_eq!(cost(Terrain::PlantedFields, movement), 1);
            assert_eq!(cost(Terrain::Road, movement), 1);
            assert_eq!(cost(Terrain::Rail, movement), 2);
            assert_eq!(cost(Terrain::Bridge, movement), 1);
            assert_eq!(cost(Terrain::DeepSnow, movement), 2);
            assert_eq!(cost(Terrain::Mud, movement), 2);
            assert_eq!(cost(Terrain::Ice, movement), 1);
            let icy_field = Hex::new(Terrain::Clear, 0).with_condition(Some(Condition::Ice));
            assert_eq!(icy_field.movement_cost(movement), 2);
        }
        assert_eq!(cost(Terrain::Swamp, Legged), 2);
        assert_eq!(cost(Terrain::Swamp, Tracked), 3);
        assert_eq!(cost(Terrain::Swamp, Hover), 1);
        assert_eq!(cost(Terrain::ThinSnow, Wheeled), 2);
        assert_eq!(cost(Terrain::ThinSnow, Tracked), 1);
        assert_eq!(cost(Terrain::HeavyIndustrial, Legged), 2);
        assert_eq!(cost(Terrain::HeavyIndustrial, Tracked), 1);
        for terrain in [Terrain::DeepSnow, Terrain::Mud, Terrain::Ice] {
            assert_eq!(cost(terrain, Hover), 1);
        }
    }

    /// A road through woods costs what the road does; snow on the road still counts.
    #[test]
    fn roads_replace_the_terrain_they_cross() {
        let forest_road = Hex::new(Terrain::HeavyJungle, 1).with_route(Some(Route::DirtRoad));
        assert_eq!(forest_road.movement_cost(Tracked), 1);
        let snowy = forest_road.with_condition(Some(Condition::DeepSnow));
        assert_eq!(snowy.movement_cost(Tracked), 2);
        assert_eq!(snowy.movement_cost(Hover), 1);
        let rail = Hex::new(Terrain::LightWoods, 0).with_route(Some(Route::Rail));
        assert_eq!(rail.movement_cost(Legged), 2);
        assert_eq!(rail.movement_cost(Hover), 2);
    }

    #[test]
    fn running_water_costs_more_except_for_hovercraft() {
        let river = |flow| Hex::at_level(0).with_water(Some(Water { depth: 2, flow }));
        assert_eq!(river(Flow::Still).movement_cost(Legged), 1);
        assert_eq!(river(Flow::Rapids).movement_cost(Legged), 2);
        assert_eq!(river(Flow::Torrent).movement_cost(Tracked), 3);
        assert_eq!(river(Flow::Torrent).movement_cost(Hover), 1);
    }

    #[test]
    fn fire_and_smoke_cost_at_least_two() {
        for kind in [DecorationKind::Fire, DecorationKind::Smoke] {
            let covered = |terrain| Hex::new(terrain, 1).with_overlay(Some(kind));
            for movement in [Legged, Wheeled] {
                assert_eq!(covered(Terrain::Clear).ground_speed_divisor(movement), 2.0);
                assert_eq!(covered(Terrain::Road).ground_speed_divisor(movement), 2.0);
                assert_eq!(covered(Terrain::Water).ground_speed_divisor(movement), 2.0);
                assert_eq!(covered(Terrain::Bridge).ground_speed_divisor(movement), 2.0);
                assert_eq!(
                    covered(Terrain::HeavyWoods).ground_speed_divisor(movement),
                    3.0
                );
            }
        }
    }

    #[test]
    fn open_ground_is_bare_firm_and_dry() {
        for terrain in Terrain::ALL {
            assert_eq!(
                Hex::new(terrain, 0).is_open_ground(),
                matches!(
                    terrain,
                    Terrain::Clear
                        | Terrain::Pavement
                        | Terrain::Road
                        | Terrain::Sand
                        | Terrain::Tundra
                ),
                "{terrain:?}"
            );
        }
    }

    #[test]
    fn water_columns_include_ice_and_bridges() {
        let holds = |terrain| Hex::new(terrain, 1).holds_water();
        assert!(holds(Terrain::Water));
        assert!(holds(Terrain::Ice));
        assert!(holds(Terrain::Bridge));
        assert!(!holds(Terrain::Clear));
        assert!(!holds(Terrain::Swamp));
    }

    #[test]
    fn immersion_depends_on_level() {
        let hex = |terrain| Hex::new(terrain, 2);
        assert!(!hex(Terrain::Water).immerses(3));
        assert!(hex(Terrain::Water).immerses(-1));
        assert!(!hex(Terrain::Water).immerses(0));
        assert!(hex(Terrain::Bridge).immerses(-2));
        assert!(!hex(Terrain::Clear).immerses(-1));
    }
}
