//! Shared terrain rule lookups on a hex's layers: open ground, foliage density, water, ground
//! movement cost, piloting modifiers, prohibited terrain, bog-down and the heat the ground
//! gives off.
//!
//! Combat, movement and sensor code ask these questions of a [`Hex`] instead of matching on
//! layers locally, so a rule changes in exactly one place. Movement costs, prohibitions and
//! piloting modifiers follow Total Warfare's movement cost table and Tactical Operations'
//! expanded movement costs and planetary conditions tables: each hex costs one point plus what
//! its terrain adds, and the realtime game divides a unit's speed by that total.
use crate::{Condition, Density, Flow, Foliage, Ground, Hex, Route, Water};
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
    /// of them. Ice costs one more wherever it lies, over water or dry ground.
    fn condition_cost(self, movement: GroundMovement) -> u8 {
        if movement == GroundMovement::Hover {
            return 0;
        }
        match self.condition() {
            Some(Condition::Ice | Condition::DeepSnow | Condition::Mud) => 1,
            Some(Condition::ThinSnow) if movement == GroundMovement::Wheeled => 1,
            _ => 0,
        }
    }

    /// What wading or walking the bed of this hex's water adds, from Total Warfare's movement
    /// cost table: one point at depth 1 and three at depth 2 or more, plus what rapids or a
    /// torrent add. Hovercraft skim the surface for nothing.
    fn water_cost(self, water: Water, movement: GroundMovement) -> u8 {
        if movement == GroundMovement::Hover {
            return 0;
        }
        let depth = match water.depth {
            0 => 0,
            1 => 1,
            _ => 3,
        };
        depth
            + match water.flow {
                Flow::Still => 0,
                Flow::Rapids => 1,
                Flow::Torrent => 2,
            }
    }

    /// Movement points a ground unit spends to enter this hex, before any change of level:
    /// one, plus what the terrain and its weather condition add. Water costs by its depth and
    /// current. Structures are crossed on their roofs or decks like roads. Fire costs nothing
    /// to move through (Tactical Operations p. 41); its dangers are heat and damage.
    pub fn movement_cost(self, movement: GroundMovement) -> u8 {
        let terrain = match (self.structure(), self.water()) {
            (Some(_), _) => 0,
            (None, Some(water)) => self.water_cost(water, movement),
            (None, None) => self.terrain_cost(movement),
        };
        1 + terrain + self.condition_cost(movement)
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

    /// Modifier for the piloting or driving roll a ground unit must make just to enter this
    /// hex, or `None` when entering takes no roll. 'Mechs roll at +0 to cross rubble and at +1
    /// to cross ultra rubble (Total Warfare p. 52, Tactical Operations p. 32). Ice takes no
    /// entry roll: its extra movement point is the price of Careful Movement, which crosses
    /// it without mishap (Tactical Operations: Advanced Rules pp. 48, 63). The realtime game
    /// has no announcement of full-speed movement, so every unit moves carefully. A road
    /// cleared through the terrain takes no roll, and neither does a structure crossed on its
    /// roof or deck.
    pub fn entry_piloting_modifier(self, movement: GroundMovement) -> Option<i16> {
        if self.is_road() || self.structure().is_some() || self.is_icy() {
            return None;
        }
        if movement != GroundMovement::Legged || !self.is_bare() {
            return None;
        }
        match self.ground() {
            Ground::Rubble => Some(0),
            Ground::UltraRubble => Some(1),
            _ => None,
        }
    }

    /// Why a ground unit may not enter this hex at all, or `None` when it may. From the
    /// prohibited-units columns of Total Warfare's and Tactical Operations' movement cost
    /// tables: wheels stay out of rough ground, rubble and ultra rubble (which inherits
    /// rubble's restrictions, Tactical Operations p. 32), woods, deep snow and magma crust; no
    /// vehicle enters heavy or ultra-heavy woods, jungle, ultra rough ground or liquid magma;
    /// hovercraft also avoid light woods; and nothing but a hovercraft crosses water a level or
    /// more deep (callers exempt amphibious vehicles). 'Mechs cannot push through ultra-heavy
    /// woods or jungle. A road or bridge carries a unit through terrain it could not otherwise
    /// enter, and a building or wall is climbed onto rather than entered.
    pub fn prohibition(self, movement: GroundMovement) -> Option<Prohibition> {
        if self.is_road() || self.structure().is_some() {
            return None;
        }
        let vehicle = movement != GroundMovement::Legged;
        if let Some(water) = self.water() {
            // Frozen water is crossed on the ice sheet, which may break (Tactical Operations
            // p. 48); open water a level or more deep sinks tracks and wheels.
            return (!self.is_frozen()
                && vehicle
                && movement != GroundMovement::Hover
                && water.depth >= 1)
                .then_some(Prohibition::Water);
        }
        if let Some(foliage) = self.foliage()
            && let Some(density) = foliage.density()
        {
            let banned = match density {
                Density::UltraHeavy => true,
                Density::Heavy => vehicle,
                Density::Light => {
                    foliage.is_jungle() && vehicle
                        || matches!(movement, GroundMovement::Wheeled | GroundMovement::Hover)
                }
            };
            if banned {
                return Some(Prohibition::Terrain(foliage.label()));
            }
        }
        let ground = match (self.ground(), movement) {
            (Ground::Magma, _) if vehicle => true,
            (Ground::UltraRough, _) if vehicle => true,
            (
                Ground::Rough | Ground::Rubble | Ground::UltraRubble | Ground::MagmaCrust,
                GroundMovement::Wheeled,
            ) => true,
            _ => false,
        };
        if ground {
            return Some(Prohibition::Terrain(self.ground().label()));
        }
        if self.condition() == Some(Condition::DeepSnow) && movement == GroundMovement::Wheeled {
            return Some(Prohibition::Terrain(Condition::DeepSnow.label()));
        }
        None
    }

    /// Modifier for the piloting or driving roll a ground unit makes against getting stuck as
    /// it enters this hex, or `None` when the ground cannot bog it down. Swamp, deep snow and
    /// tundra can trap any ground unit, and mud any but a 'Mech, which takes mud's other
    /// penalties but never sticks in it (Tactical Operations pp. 37, 48, 60); tundra, deep
    /// snow and mud make the roll one easier. Sand traps only Large Vehicles (p. 37), which the
    /// game does not have. Hovercraft never bog down, and a road, a structure or water keeps a
    /// unit off the soft ground.
    pub fn bog_down_modifier(self, movement: GroundMovement) -> Option<i16> {
        if movement == GroundMovement::Hover
            || self.is_road()
            || self.structure().is_some()
            || self.holds_water()
        {
            return None;
        }
        match self.condition() {
            Some(Condition::DeepSnow) => return Some(-1),
            Some(Condition::Mud) if movement != GroundMovement::Legged => return Some(-1),
            Some(Condition::Mud) | None => {}
            Some(_) => return None,
        }
        match self.ground() {
            Ground::Swamp => Some(0),
            Ground::Tundra => Some(-1),
            _ => None,
        }
    }

    /// Whether the terrain under a ground unit at `level` in this hex can bog it down: soft
    /// ground it stands on with nothing built or frozen over it.
    pub fn can_bog_down(self, movement: GroundMovement, level: i32) -> bool {
        self.bog_down_modifier(movement).is_some() && self.touches_ground(level)
    }

    /// Whether running or turning on this hex risks a skid: pavement, a road or a bridge deck
    /// (Total Warfare p. 62), or ice over anything (Tactical Operations p. 48).
    pub fn is_slick(self) -> bool {
        self.is_icy() || self.is_road() || self.has_bridge() || self.ground() == Ground::Pavement
    }
}

/// Why a ground unit cannot enter a hex.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Prohibition {
    /// Water a level or more deep, which only hovercraft and amphibious vehicles cross.
    Water,
    /// Terrain the unit's movement type cannot push through, named for the report.
    Terrain(&'static str),
}

impl Prohibition {
    /// What the unit is told when it stops at the hex's edge.
    pub fn describe(self) -> String {
        match self {
            Self::Water => "Your vehicle cannot cross water that deep.".into(),
            Self::Terrain(label) => {
                format!("You cannot enter {}.", label.to_ascii_lowercase())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DecorationKind, Prohibition, Structure, Terrain, Water};
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
            // Ice costs one more over water as on land, and frozen water is crossed on top.
            assert_eq!(cost(Terrain::Ice, movement), 2);
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

    /// Water costs by depth as in Total Warfare, plus what its current adds.
    #[test]
    fn water_costs_by_depth_and_current_except_for_hovercraft() {
        let water = |depth, flow| Hex::at_level(0).with_water(Some(Water { depth, flow }));
        assert_eq!(water(0, Flow::Still).movement_cost(Legged), 1);
        assert_eq!(water(1, Flow::Still).movement_cost(Legged), 2);
        assert_eq!(water(2, Flow::Still).movement_cost(Legged), 4);
        assert_eq!(water(5, Flow::Still).movement_cost(Tracked), 4);
        assert_eq!(water(1, Flow::Rapids).movement_cost(Legged), 3);
        assert_eq!(water(2, Flow::Torrent).movement_cost(Tracked), 6);
        assert_eq!(water(2, Flow::Torrent).movement_cost(Hover), 1);
        assert_eq!(Hex::new(Terrain::Bridge, 2).movement_cost(Wheeled), 1);
    }

    /// Fire and smoke cost nothing to move through (Tactical Operations pp. 41, 45): a burning
    /// or smoky hex costs what the hex under it does.
    #[test]
    fn fire_and_smoke_cost_no_movement() {
        let burning = |terrain| Hex::new(terrain, 1).with_overlay(Some(DecorationKind::Fire));
        for movement in [Legged, Wheeled] {
            for terrain in [
                Terrain::Clear,
                Terrain::Road,
                Terrain::Water,
                Terrain::Bridge,
                Terrain::HeavyWoods,
            ] {
                assert_eq!(
                    burning(terrain).ground_speed_divisor(movement),
                    Hex::new(terrain, 1).ground_speed_divisor(movement),
                    "{terrain:?}"
                );
            }
            for kind in [DecorationKind::Smoke, DecorationKind::HeavySmoke] {
                let smoky = Hex::new(Terrain::Clear, 1).with_overlay(Some(kind));
                assert_eq!(smoky.ground_speed_divisor(movement), 1.0);
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

    /// 'Mechs roll to cross rubble and ultra rubble; ice, crossed carefully, takes no roll,
    /// and neither does a road or structure.
    #[test]
    fn rubble_takes_entry_rolls_and_ice_does_not() {
        for terrain in Terrain::ALL {
            let expected = match terrain {
                Terrain::Rubble => Some(0),
                Terrain::UltraRubble => Some(1),
                _ => None,
            };
            assert_eq!(
                Hex::new(terrain, 0).entry_piloting_modifier(Legged),
                expected,
                "{terrain:?}"
            );
            assert_eq!(
                Hex::new(terrain, 0).entry_piloting_modifier(Tracked),
                None,
                "{terrain:?}"
            );
            assert_eq!(
                Hex::new(terrain, 0).entry_piloting_modifier(Hover),
                None,
                "{terrain:?}"
            );
        }
        let cleared = Hex::new(Terrain::UltraRubble, 0).with_route(Some(Route::PavedRoad));
        assert_eq!(cleared.entry_piloting_modifier(Legged), None);
        let icy_field = Hex::new(Terrain::Clear, 0).with_condition(Some(Condition::Ice));
        assert_eq!(icy_field.entry_piloting_modifier(Wheeled), None);
        assert_eq!(icy_field.entry_piloting_modifier(Legged), None);
        let icy_road = icy_field.with_route(Some(Route::DirtRoad));
        assert_eq!(icy_road.entry_piloting_modifier(Wheeled), None);
    }

    /// The prohibited-units columns of the movement cost tables.
    #[test]
    fn prohibitions_follow_the_movement_cost_tables() {
        let banned = |terrain, movement| Hex::new(terrain, 0).prohibition(movement).is_some();
        for terrain in Terrain::ALL {
            let mech = matches!(
                terrain,
                Terrain::UltraHeavyWoods | Terrain::UltraHeavyJungle
            );
            assert_eq!(banned(terrain, Legged), mech, "{terrain:?}");
            let wheeled = matches!(
                terrain,
                Terrain::Rough
                    | Terrain::UltraRough
                    | Terrain::Rubble
                    | Terrain::UltraRubble
                    | Terrain::LightWoods
                    | Terrain::HeavyWoods
                    | Terrain::UltraHeavyWoods
                    | Terrain::LightJungle
                    | Terrain::HeavyJungle
                    | Terrain::UltraHeavyJungle
                    | Terrain::DeepSnow
                    | Terrain::MagmaCrust
                    | Terrain::Magma
            );
            // `Hex::new` gives water and ice no depth, so they are open to everything here.
            assert_eq!(banned(terrain, Wheeled), wheeled, "{terrain:?}");
            let tracked = matches!(
                terrain,
                Terrain::UltraRough
                    | Terrain::HeavyWoods
                    | Terrain::UltraHeavyWoods
                    | Terrain::LightJungle
                    | Terrain::HeavyJungle
                    | Terrain::UltraHeavyJungle
                    | Terrain::Magma
            );
            assert_eq!(banned(terrain, Tracked), tracked, "{terrain:?}");
            let hover = matches!(
                terrain,
                Terrain::UltraRough
                    | Terrain::LightWoods
                    | Terrain::HeavyWoods
                    | Terrain::UltraHeavyWoods
                    | Terrain::LightJungle
                    | Terrain::HeavyJungle
                    | Terrain::UltraHeavyJungle
                    | Terrain::Magma
            );
            assert_eq!(banned(terrain, Hover), hover, "{terrain:?}");
        }
        // The notation's digit is water depth: ankle-deep water is open to everything, and an
        // ice sheet carries vehicles over any depth.
        for movement in [Tracked, Wheeled] {
            assert_eq!(
                Hex::new(Terrain::Water, 1).prohibition(movement),
                Some(Prohibition::Water)
            );
            assert_eq!(Hex::new(Terrain::Ice, 3).prohibition(movement), None);
            assert_eq!(Hex::new(Terrain::Water, 0).prohibition(movement), None);
        }
        assert_eq!(Hex::new(Terrain::Water, 4).prohibition(Hover), None);
        assert_eq!(Hex::new(Terrain::Bridge, 1).prohibition(Wheeled), None);
        let forest_road = Hex::new(Terrain::HeavyJungle, 0).with_route(Some(Route::DirtRoad));
        assert_eq!(forest_road.prohibition(Wheeled), None);
        assert_eq!(
            Hex::new(Terrain::HeavyWoods, 0).prohibition(Tracked),
            Some(Prohibition::Terrain("Heavy woods"))
        );
    }

    /// Swamp, tundra and deep snow bog any ground unit down and mud any but a 'Mech; sand
    /// traps only Large Vehicles, which the game lacks (Tactical Operations pp. 37, 48, 60);
    /// hovercraft, roads and structures are exempt.
    #[test]
    fn soft_ground_bogs_down_the_units_the_book_lists() {
        for terrain in Terrain::ALL {
            let expected = match terrain {
                Terrain::Swamp => Some(0),
                Terrain::Tundra | Terrain::DeepSnow | Terrain::Mud => Some(-1),
                _ => None,
            };
            let mech = if terrain == Terrain::Mud {
                None
            } else {
                expected
            };
            let hex = Hex::new(terrain, 0);
            assert_eq!(hex.bog_down_modifier(Legged), mech, "{terrain:?}");
            assert_eq!(hex.bog_down_modifier(Tracked), expected, "{terrain:?}");
            assert_eq!(hex.bog_down_modifier(Wheeled), expected, "{terrain:?}");
            assert_eq!(hex.bog_down_modifier(Hover), None, "{terrain:?}");
            assert_eq!(hex.can_bog_down(Tracked, 0), expected.is_some());
            assert!(!hex.can_bog_down(Tracked, 1), "{terrain:?}");
        }
        let snowy_swamp = Hex::new(Terrain::Swamp, 0).with_condition(Some(Condition::DeepSnow));
        assert_eq!(snowy_swamp.bog_down_modifier(Wheeled), Some(-1));
        let muddy_swamp = Hex::new(Terrain::Swamp, 0).with_condition(Some(Condition::Mud));
        assert_eq!(muddy_swamp.bog_down_modifier(Legged), Some(0));
        assert_eq!(muddy_swamp.bog_down_modifier(Tracked), Some(-1));
        let road = Hex::new(Terrain::Swamp, 0).with_route(Some(Route::GravelRoad));
        assert_eq!(road.bog_down_modifier(Legged), None);
    }

    /// Pavement, roads, bridge decks and ice are where units skid.
    #[test]
    fn slick_surfaces_are_pavement_roads_decks_and_ice() {
        for terrain in Terrain::ALL {
            let expected = matches!(
                terrain,
                Terrain::Pavement | Terrain::Road | Terrain::Bridge | Terrain::Ice
            );
            assert_eq!(Hex::new(terrain, 0).is_slick(), expected, "{terrain:?}");
        }
        let icy_field = Hex::new(Terrain::Clear, 0).with_condition(Some(Condition::Ice));
        assert!(icy_field.is_slick());
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
