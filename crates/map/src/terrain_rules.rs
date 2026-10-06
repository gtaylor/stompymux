//! Shared terrain rule lookups on a hex's layers: open ground, foliage density, water, ground
//! movement cost, piloting modifiers and the heat the ground gives off.
//!
//! Combat, movement and sensor code ask these questions of a [`Hex`] instead of matching on
//! layers locally, so a rule changes in exactly one place. Movement costs and piloting
//! modifiers follow Tactical Operations' expanded movement costs and planetary conditions
//! tables: each hex costs one point plus what its terrain adds, and the realtime game divides a
//! unit's speed by that total.
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

    /// Whether a unit at `level` stands on this hex's bare ground: dry, with no structure
    /// to stand on, and not in the air. Ground hazards such as magma and deep snow reach it.
    pub fn touches_ground(self, level: i32) -> bool {
        self.water().is_none() && self.structure().is_none() && level <= i32::from(self.level())
    }

    /// Whether a unit at `level` stands in molten magma.
    pub fn in_liquid_magma(self, level: i32) -> bool {
        self.ground() == Ground::Magma && self.touches_ground(level)
    }

    /// Whether a unit at `level` stands on magma crust that may break beneath it.
    pub fn on_magma_crust(self, level: i32) -> bool {
        self.ground() == Ground::MagmaCrust && self.touches_ground(level)
    }

    /// Heat a 'Mech at `level` picks up each turn from the ground it stands on: five on magma
    /// crust and ten in liquid magma. Unlike other outside heat this is never capped.
    pub fn ground_heat(self, level: i32) -> u8 {
        if !self.touches_ground(level) {
            return 0;
        }
        match self.ground() {
            Ground::MagmaCrust => 5,
            Ground::Magma => 10,
            _ => 0,
        }
    }

    /// Whether a unit at `level` stands in deep snow, which draws a point of heat a turn from
    /// a 'Mech with heat sinks in its legs.
    pub fn chills(self, level: i32) -> bool {
        self.condition() == Some(Condition::DeepSnow) && self.touches_ground(level)
    }

    /// Modifier the terrain under a ground unit at `level` adds to every piloting or driving
    /// roll it makes there, from the PSR column of Tactical Operations' expanded movement
    /// costs table. Units in the air, or on a roof, wall or bridge deck, take none. Hovercraft
    /// skim over ice, snow, mud, swamp and fast water; a road replaces the ground and foliage it
    /// runs through, as it does for movement.
    pub fn piloting_modifier(self, movement: GroundMovement, level: i32) -> i16 {
        let hover = movement == GroundMovement::Hover;
        if level > i32::from(self.standing_height()) {
            return 0;
        }
        if let Some(water) = self.water() {
            if self.is_ice() && level >= i32::from(self.water_line()) {
                return if hover { 0 } else { 4 };
            }
            if hover || !self.immerses(level) {
                return 0;
            }
            return match water.flow {
                Flow::Still => 0,
                Flow::Rapids => 2,
                Flow::Torrent => 3,
            };
        }
        if self.structure().is_some() {
            return 0;
        }
        let terrain = if self.is_road() {
            0
        } else {
            let ground = match (self.ground(), movement) {
                (Ground::Sand | Ground::Tundra | Ground::MagmaCrust, _) => 1,
                (Ground::HeavyIndustrial, _) => 1,
                (Ground::Magma, _) => 4,
                (Ground::Swamp, GroundMovement::Legged) => 1,
                (Ground::Swamp, GroundMovement::Hover) => 0,
                (Ground::Swamp, _) => 2,
                _ => 0,
            };
            let foliage = match self.foliage() {
                Some(foliage) if foliage.is_jungle() => {
                    foliage.density().map_or(0, Density::points)
                }
                _ => 0,
            };
            ground + i16::from(foliage)
        };
        let condition = match self.condition() {
            _ if hover => 0,
            Some(Condition::Ice) => 4,
            Some(Condition::DeepSnow | Condition::Mud) => 1,
            Some(Condition::ThinSnow) if movement == GroundMovement::Wheeled => 1,
            _ => 0,
        };
        terrain + condition
    }

    /// Modifier for the piloting roll a ground unit must make just to enter this hex, or
    /// `None` when entering takes no roll. Ultra rubble is the only such terrain: shattered
    /// hardened construction takes a +1 roll to cross unless a road has been cleared through.
    pub fn entry_piloting_modifier(self) -> Option<i16> {
        (self.ground() == Ground::UltraRubble && self.is_bare() && !self.is_road()).then_some(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DecorationKind, Structure, Terrain, Water};
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

    /// The PSR column of the expanded movement costs tables, for a unit standing on the hex.
    #[test]
    fn piloting_modifiers_follow_the_expanded_table() {
        let psr = |terrain, movement| Hex::new(terrain, 0).piloting_modifier(movement, 0);
        for movement in [Legged, Tracked, Wheeled, Hover] {
            assert_eq!(psr(Terrain::Clear, movement), 0);
            assert_eq!(psr(Terrain::Rough, movement), 0);
            assert_eq!(psr(Terrain::UltraRubble, movement), 0);
            assert_eq!(psr(Terrain::LightWoods, movement), 0);
            assert_eq!(psr(Terrain::Sand, movement), 1);
            assert_eq!(psr(Terrain::Tundra, movement), 1);
            assert_eq!(psr(Terrain::MagmaCrust, movement), 1);
            assert_eq!(psr(Terrain::Magma, movement), 4);
            assert_eq!(psr(Terrain::HeavyIndustrial, movement), 1);
            assert_eq!(psr(Terrain::LightJungle, movement), 1);
            assert_eq!(psr(Terrain::HeavyJungle, movement), 2);
            assert_eq!(psr(Terrain::UltraHeavyJungle, movement), 3);
        }
        assert_eq!(psr(Terrain::Swamp, Legged), 1);
        assert_eq!(psr(Terrain::Swamp, Tracked), 2);
        assert_eq!(psr(Terrain::Swamp, Hover), 0);
        assert_eq!(psr(Terrain::DeepSnow, Legged), 1);
        assert_eq!(psr(Terrain::Mud, Tracked), 1);
        assert_eq!(psr(Terrain::ThinSnow, Wheeled), 1);
        assert_eq!(psr(Terrain::ThinSnow, Legged), 0);
        let icy_field = Hex::new(Terrain::Clear, 0).with_condition(Some(Condition::Ice));
        assert_eq!(icy_field.piloting_modifier(Legged, 0), 4);
        assert_eq!(icy_field.piloting_modifier(Hover, 0), 0);
        for terrain in [Terrain::DeepSnow, Terrain::Mud] {
            assert_eq!(psr(terrain, Hover), 0);
        }
    }

    /// Fast water troubles units wading in it, ice units standing on it, and nothing in the
    /// air or on a structure takes a ground modifier.
    #[test]
    fn piloting_modifiers_depend_on_where_the_unit_is() {
        let river = |flow| Hex::at_level(2).with_water(Some(Water { depth: 2, flow }));
        assert_eq!(river(Flow::Rapids).piloting_modifier(Legged, 0), 2);
        assert_eq!(river(Flow::Torrent).piloting_modifier(Legged, 0), 3);
        assert_eq!(river(Flow::Torrent).piloting_modifier(Hover, 2), 0);
        assert_eq!(river(Flow::Still).piloting_modifier(Legged, 0), 0);
        let ice = Hex::new(Terrain::Ice, 2);
        assert_eq!(ice.piloting_modifier(Legged, i32::from(ice.level())), 4);
        assert_eq!(ice.piloting_modifier(Legged, -1), 0, "beneath the ice");
        let magma = Hex::new(Terrain::Magma, 1);
        assert_eq!(magma.piloting_modifier(Legged, 3), 0, "jumping overhead");
        let roof = Hex::new(Terrain::Magma, 1).with_structure(Some(Structure::building(2)));
        assert_eq!(roof.piloting_modifier(Legged, 3), 0);
        let road = Hex::new(Terrain::HeavyJungle, 0).with_route(Some(Route::DirtRoad));
        assert_eq!(road.piloting_modifier(Tracked, 0), 0);
        let snowy_road = road.with_condition(Some(Condition::DeepSnow));
        assert_eq!(snowy_road.piloting_modifier(Tracked, 0), 1);
    }

    /// Ultra rubble alone takes a roll to enter, unless a road crosses it.
    #[test]
    fn only_ultra_rubble_takes_an_entry_roll() {
        for terrain in Terrain::ALL {
            let expected = (terrain == Terrain::UltraRubble).then_some(1);
            assert_eq!(
                Hex::new(terrain, 0).entry_piloting_modifier(),
                expected,
                "{terrain:?}"
            );
        }
        let cleared = Hex::new(Terrain::UltraRubble, 0).with_route(Some(Route::PavedRoad));
        assert_eq!(cleared.entry_piloting_modifier(), None);
    }

    /// Magma heats only units standing on it; deep snow chills them.
    #[test]
    fn magma_heat_and_snow_chill_reach_units_on_the_ground() {
        let crust = Hex::new(Terrain::MagmaCrust, 2);
        let magma = Hex::new(Terrain::Magma, 2);
        assert_eq!(crust.ground_heat(2), 5);
        assert_eq!(magma.ground_heat(2), 10);
        assert_eq!(magma.ground_heat(3), 0, "airborne");
        assert_eq!(Hex::new(Terrain::Clear, 2).ground_heat(2), 0);
        assert!(magma.in_liquid_magma(2) && !crust.in_liquid_magma(2));
        assert!(crust.on_magma_crust(2) && !crust.on_magma_crust(4));
        assert!(Hex::new(Terrain::DeepSnow, 1).chills(1));
        assert!(!Hex::new(Terrain::ThinSnow, 1).chills(1));
        assert!(!Hex::new(Terrain::DeepSnow, 1).chills(2));
    }

    /// Broken crust turns to liquid magma and swallows whatever lay on it.
    #[test]
    fn broken_crust_becomes_liquid_magma() {
        let crust = Hex::new(Terrain::MagmaCrust, 3)
            .with_foliage(Some(Foliage::LightWoods))
            .with_condition(Some(Condition::ThinSnow));
        let broken = crust.with_crust_broken();
        assert_eq!(broken.terrain(), Terrain::Magma);
        assert_eq!(broken.level(), 3);
        broken.validate().unwrap();
        assert_eq!(
            Hex::new(Terrain::Clear, 1).with_crust_broken().terrain(),
            Terrain::Clear
        );
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
