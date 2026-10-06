//! One battlefield hex as independent layers: elevation, ground, water, foliage, route,
//! structure, condition, and a fire or smoke overlay.
//!
//! Each layer answers one question, so a hex can hold jungle on a hill, ice over a torrent, a
//! dirt road through heavy woods or snow on a building's roof without one value standing in for
//! another. Fire and smoke never replace what they burn or cover: they are an overlay the map
//! applies from its decorations. Rules ask the layers. [`Hex::terrain`] names the one feature a
//! map shows for a hex, and [`Hex::new`] reads the compact symbol-and-digit notation used by
//! [`MapAsset::from_cells`](crate::MapAsset::from_cells). [`Hex::validate`] says which
//! combinations make sense.
use crate::{
    Condition, ConstructionClass, DecorationKind, Foliage, Ground, Route, Structure, StructureKind,
    Terrain, Water,
};
use anyhow::ensure;
use serde::{Deserialize, Serialize};

/// The largest ground level, structure height or bridge deck a hex may have.
pub const MAX_HEIGHT: u8 = 35;

/// The deepest water a hex may hold.
pub const MAX_DEPTH: u8 = 9;

/// One-character height for maps and map files: `0`-`9`, then `a`-`z` for 10 through 35.
/// Anything higher shows as `?`.
pub fn height_glyph(height: u8) -> char {
    char::from_digit(u32::from(height), 36).unwrap_or('?')
}

/// A battlefield hex. Build one with [`Hex::new`] from its single-terrain description, or with
/// [`Hex::at_level`] and the `with_` setters from its layers. Saved and scripted as its
/// layers; absent layers are omitted. A live map's terrain grid never holds an overlay: the
/// server adds it from the map's fire and smoke decorations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hex {
    level: u8,
    ground: Ground,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    water: Option<Water>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    foliage: Option<Foliage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    route: Option<Route>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    structure: Option<Structure>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    condition: Option<Condition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    overlay: Option<DecorationKind>,
}

impl Hex {
    /// Build the hex the compact notation describes: a terrain symbol and one digit. The digit
    /// is the ground level, except for water and ice (depth), bridges (deck height) and
    /// buildings and walls (their height). Roads are paved; bridges span still water one level
    /// deep; structures are medium; snow, mud, fire and smoke lie on clear ground. Rules build
    /// and change hexes through their layers instead.
    pub const fn new(terrain: Terrain, digit: u8) -> Self {
        let dry = Self::at_level(digit);
        let water = Self::at_level(0).with_water(Some(Water::still(digit)));
        match terrain {
            Terrain::Clear => dry,
            Terrain::Pavement => dry.with_ground(Ground::Pavement),
            Terrain::Road => dry.with_route(Some(Route::PavedRoad)),
            Terrain::Rail => dry.with_route(Some(Route::Rail)),
            Terrain::Rough => dry.with_ground(Ground::Rough),
            Terrain::UltraRough => dry.with_ground(Ground::UltraRough),
            Terrain::Rubble => dry.with_ground(Ground::Rubble),
            Terrain::UltraRubble => dry.with_ground(Ground::UltraRubble),
            Terrain::Sand => dry.with_ground(Ground::Sand),
            Terrain::Tundra => dry.with_ground(Ground::Tundra),
            Terrain::Swamp => dry.with_ground(Ground::Swamp),
            Terrain::MagmaCrust => dry.with_ground(Ground::MagmaCrust),
            Terrain::Magma => dry.with_ground(Ground::Magma),
            Terrain::HeavyIndustrial => dry.with_ground(Ground::HeavyIndustrial),
            Terrain::LightWoods => dry.with_foliage(Some(Foliage::LightWoods)),
            Terrain::HeavyWoods => dry.with_foliage(Some(Foliage::HeavyWoods)),
            Terrain::UltraHeavyWoods => dry.with_foliage(Some(Foliage::UltraHeavyWoods)),
            Terrain::LightJungle => dry.with_foliage(Some(Foliage::LightJungle)),
            Terrain::HeavyJungle => dry.with_foliage(Some(Foliage::HeavyJungle)),
            Terrain::UltraHeavyJungle => dry.with_foliage(Some(Foliage::UltraHeavyJungle)),
            Terrain::PlantedFields => dry.with_foliage(Some(Foliage::PlantedFields)),
            Terrain::Water => water,
            Terrain::Ice => water.with_condition(Some(Condition::Ice)),
            Terrain::ThinSnow => dry.with_condition(Some(Condition::ThinSnow)),
            Terrain::DeepSnow => dry.with_condition(Some(Condition::DeepSnow)),
            Terrain::Mud => dry.with_condition(Some(Condition::Mud)),
            Terrain::Bridge => Self::at_level(0)
                .with_water(Some(Water::still(1)))
                .with_structure(Some(Structure::bridge(digit))),
            Terrain::Building => Self::at_level(0).with_structure(Some(Structure::building(digit))),
            Terrain::Wall => Self::at_level(0).with_structure(Some(Structure::wall(digit))),
            Terrain::Fire => dry.with_overlay(Some(DecorationKind::Fire)),
            Terrain::Smoke => dry.with_overlay(Some(DecorationKind::Smoke)),
        }
    }

    /// Clear ground at `level`, with nothing on it.
    pub const fn at_level(level: u8) -> Self {
        Self {
            level,
            ground: Ground::Clear,
            water: None,
            foliage: None,
            route: None,
            structure: None,
            condition: None,
            overlay: None,
        }
    }

    /// This hex with its ground height changed.
    pub const fn with_level(self, level: u8) -> Self {
        Self { level, ..self }
    }

    /// This hex with its ground made of `ground`.
    pub const fn with_ground(self, ground: Ground) -> Self {
        Self { ground, ..self }
    }

    /// This hex with its water replaced or drained.
    pub const fn with_water(self, water: Option<Water>) -> Self {
        Self { water, ..self }
    }

    /// This hex with its foliage replaced or cleared.
    pub const fn with_foliage(self, foliage: Option<Foliage>) -> Self {
        Self { foliage, ..self }
    }

    /// This hex with its road or rail replaced or removed.
    pub const fn with_route(self, route: Option<Route>) -> Self {
        Self { route, ..self }
    }

    /// This hex with its structure replaced or removed.
    pub const fn with_structure(self, structure: Option<Structure>) -> Self {
        Self { structure, ..self }
    }

    /// This hex with its weather condition replaced or cleared.
    pub const fn with_condition(self, condition: Option<Condition>) -> Self {
        Self { condition, ..self }
    }

    /// This hex with fire or smoke laid over it, or with its overlay removed.
    pub const fn with_overlay(self, overlay: Option<DecorationKind>) -> Self {
        Self { overlay, ..self }
    }

    /// Check every height and depth is within the battlefield's limits and that the layers
    /// make sense together:
    ///
    /// - water lies over clear ground, with no foliage or route, and only ice for a condition;
    ///   rapids and torrents need at least one level of depth;
    /// - foliage grows only on ground that supports it, and not where a building or wall
    ///   stands;
    /// - a road or rail line does not run through a building or wall;
    /// - a bridge spans water, while buildings and walls stand on dry ground;
    /// - nothing but a structure lies on molten magma, and snow and mud need dry ground;
    /// - every structure hex has a construction factor from 1 up to its class's full one.
    pub fn validate(self) -> anyhow::Result<()> {
        let structure = self.structure.map_or(0, |structure| structure.height);
        ensure!(
            self.level <= MAX_HEIGHT && structure <= MAX_HEIGHT && self.water_depth() <= MAX_DEPTH,
            "Hex heights exceed map limits"
        );
        if let Some(water) = self.water {
            ensure!(
                self.ground == Ground::Clear,
                "Water must lie over clear ground"
            );
            ensure!(
                self.foliage.is_none() && self.route.is_none(),
                "Foliage, roads and rails cannot be under water; use a bridge"
            );
            ensure!(
                matches!(self.condition, None | Some(Condition::Ice)),
                "Only ice can lie over water"
            );
            ensure!(
                water.flow.is_still() || water.depth >= 1,
                "Rapids and torrents need water at least one level deep"
            );
        }
        if let Some(foliage) = self.foliage {
            ensure!(
                self.ground.supports_foliage(),
                "{} cannot grow on {}",
                foliage.label(),
                self.ground.label().to_ascii_lowercase()
            );
        }
        if self.ground == Ground::Magma {
            ensure!(
                self.condition.is_none() && self.route.is_none(),
                "Nothing but a structure can lie on magma"
            );
        }
        if let Some(structure) = self.structure {
            let full = structure.class.construction_factor();
            ensure!(
                (1..=full).contains(&structure.cf),
                "A {} structure's construction factor must be between 1 and {full}",
                structure.class.label().to_ascii_lowercase()
            );
            match structure.kind {
                StructureKind::Bridge => {
                    ensure!(self.water.is_some(), "A bridge must span water");
                }
                StructureKind::Building | StructureKind::Wall => {
                    ensure!(
                        self.water.is_none() && self.foliage.is_none() && self.route.is_none(),
                        "Buildings and walls stand on dry ground without foliage or routes"
                    );
                }
            }
        }
        Ok(())
    }

    /// Ground height in levels; water surfaces sit at this height.
    pub const fn level(self) -> u8 {
        self.level
    }

    /// What the ground is made of.
    pub const fn ground(self) -> Ground {
        self.ground
    }

    /// Standing water, if any.
    pub const fn water(self) -> Option<Water> {
        self.water
    }

    /// Vegetation on the ground, if any.
    pub const fn foliage(self) -> Option<Foliage> {
        self.foliage
    }

    /// The road or rail line through this hex, if any.
    pub const fn route(self) -> Option<Route> {
        self.route
    }

    /// The built feature in this hex, if any.
    pub const fn structure(self) -> Option<Structure> {
        self.structure
    }

    /// The ice, snow or mud lying over this hex, if any.
    pub const fn condition(self) -> Option<Condition> {
        self.condition
    }

    /// Fire or smoke over this hex, if any.
    pub const fn overlay(self) -> Option<DecorationKind> {
        self.overlay
    }

    /// The one feature a map shows for this hex: fire or smoke, then a structure, then water
    /// or ice over it, then a road or rail line (which sets the hex's movement cost), then
    /// foliage, then ice, snow or mud, then the ground. Rules ask the layers instead.
    pub const fn terrain(self) -> Terrain {
        match self.overlay {
            Some(DecorationKind::Fire) => return Terrain::Fire,
            Some(DecorationKind::Smoke) => return Terrain::Smoke,
            None => {}
        }
        if let Some(structure) = self.structure {
            return match structure.kind {
                StructureKind::Building => Terrain::Building,
                StructureKind::Wall => Terrain::Wall,
                StructureKind::Bridge => Terrain::Bridge,
            };
        }
        if self.water.is_some() {
            return match self.condition {
                Some(Condition::Ice) => Terrain::Ice,
                _ => Terrain::Water,
            };
        }
        match self.route {
            Some(Route::Rail) => return Terrain::Rail,
            Some(_) => return Terrain::Road,
            None => {}
        }
        if let Some(foliage) = self.foliage {
            return match foliage {
                Foliage::LightWoods => Terrain::LightWoods,
                Foliage::HeavyWoods => Terrain::HeavyWoods,
                Foliage::UltraHeavyWoods => Terrain::UltraHeavyWoods,
                Foliage::LightJungle => Terrain::LightJungle,
                Foliage::HeavyJungle => Terrain::HeavyJungle,
                Foliage::UltraHeavyJungle => Terrain::UltraHeavyJungle,
                Foliage::PlantedFields => Terrain::PlantedFields,
            };
        }
        match self.condition {
            Some(Condition::Ice) => return Terrain::Ice,
            Some(Condition::ThinSnow) => return Terrain::ThinSnow,
            Some(Condition::DeepSnow) => return Terrain::DeepSnow,
            Some(Condition::Mud) => return Terrain::Mud,
            None => {}
        }
        match self.ground {
            Ground::Clear => Terrain::Clear,
            Ground::Pavement => Terrain::Pavement,
            Ground::Rough => Terrain::Rough,
            Ground::UltraRough => Terrain::UltraRough,
            Ground::Rubble => Terrain::Rubble,
            Ground::UltraRubble => Terrain::UltraRubble,
            Ground::Sand => Terrain::Sand,
            Ground::Tundra => Terrain::Tundra,
            Ground::Swamp => Terrain::Swamp,
            Ground::MagmaCrust => Terrain::MagmaCrust,
            Ground::Magma => Terrain::Magma,
            Ground::HeavyIndustrial => Terrain::HeavyIndustrial,
        }
    }

    /// Height of this hex's water surface, which sits at its ground level. Heights below it
    /// are underwater when the hex holds water.
    pub const fn water_line(self) -> i16 {
        self.level as i16
    }

    /// Depth of the standing water in this hex, or zero when there is none.
    pub const fn water_depth(self) -> u8 {
        match self.water {
            Some(Water { depth, .. }) => depth,
            None => 0,
        }
    }

    /// Whether this hex's water is frozen over.
    pub const fn is_frozen(self) -> bool {
        self.water.is_some() && matches!(self.condition, Some(Condition::Ice))
    }

    /// Whether this hex is open water: unfrozen, with no bridge or other structure over it.
    pub const fn is_open_water(self) -> bool {
        self.water.is_some() && !self.is_frozen() && self.structure.is_none()
    }

    /// Whether fire is burning over this hex.
    pub const fn is_burning(self) -> bool {
        matches!(self.overlay, Some(DecorationKind::Fire))
    }

    /// Whether a road of any surface runs through this hex with nothing built over it.
    pub const fn is_road(self) -> bool {
        matches!(self.route, Some(route) if route.is_road())
            && self.water.is_none()
            && self.structure.is_none()
    }

    /// Whether a bridge spans this hex.
    pub const fn has_bridge(self) -> bool {
        matches!(self.structure, Some(structure) if structure.is_bridge())
    }

    /// Whether a building or wall stands in this hex.
    pub const fn has_standing_structure(self) -> bool {
        matches!(self.structure, Some(structure) if structure.is_standing())
    }

    /// Whether this hex's surface is water or ice with nothing built over it.
    pub const fn is_water_surface(self) -> bool {
        self.water.is_some() && self.structure.is_none()
    }

    /// Whether this hex is frozen water with no structure over it: ice that can break.
    pub const fn is_ice(self) -> bool {
        self.is_frozen() && self.structure.is_none()
    }

    /// Whether ice coats this hex, over water or dry ground, making it slippery.
    pub const fn is_icy(self) -> bool {
        matches!(self.condition, Some(Condition::Ice))
    }

    /// Height of the bridge deck in this hex, if it has one.
    pub fn deck_height(self) -> Option<i16> {
        self.deck_clearance()
            .map(|deck| i16::from(self.level) + i16::from(deck))
    }

    /// Height of the bridge deck above the hex's ground level, if it has a bridge.
    pub const fn deck_clearance(self) -> Option<u8> {
        match self.structure {
            Some(structure) if structure.is_bridge() => Some(structure.height),
            _ => None,
        }
    }

    /// Height of the topmost surface: a structure's top or bridge deck, otherwise the ground
    /// or water surface.
    pub fn top_height(self) -> i16 {
        match self.structure {
            Some(_) => self.surface_height(),
            None => i16::from(self.level),
        }
    }

    /// Height of the bottom of the hex: the ground, or the bed beneath any water.
    pub fn bottom_height(self) -> i16 {
        i16::from(self.level) - i16::from(self.water_depth())
    }

    /// This hex with its water frozen over; dry hexes are unchanged.
    pub const fn frozen(self) -> Self {
        if self.water.is_none() {
            return self;
        }
        self.with_condition(Some(Condition::Ice))
    }

    /// This hex with any ice on it melted.
    pub const fn thawed(self) -> Self {
        match self.condition {
            Some(Condition::Ice) => self.with_condition(None),
            _ => self,
        }
    }

    /// This hex after its ice cracks or its bridge collapses: the same water, now open.
    pub const fn with_surface_broken(self) -> Self {
        let mut hex = self;
        if hex.has_bridge() {
            hex.structure = None;
        }
        if hex.water.is_some() {
            hex = hex.thawed();
        }
        hex
    }

    /// This hex after its magma crust breaks open: liquid magma at the same height, with
    /// anything that lay on the crust swallowed. Other hexes are unchanged.
    pub const fn with_crust_broken(self) -> Self {
        if !matches!(self.ground, Ground::MagmaCrust) {
            return self;
        }
        self.with_ground(Ground::Magma)
            .with_foliage(None)
            .with_route(None)
            .with_condition(None)
    }

    /// This hex after its building or wall collapses: rubble at ground level, ultra rubble for
    /// heavy and hardened construction. Bridges collapse into their water instead; see
    /// [`Hex::with_surface_broken`].
    pub const fn collapsed(self) -> Self {
        match self.structure {
            Some(structure) if structure.is_standing() => self
                .with_structure(None)
                .with_ground(structure.class.rubble()),
            Some(_) => self.with_surface_broken(),
            None => self,
        }
    }

    /// This hex with its structure's construction factor reduced by `damage`, or collapsed
    /// when that leaves none. Hexes without a structure are unchanged.
    pub const fn with_structure_damage(self, damage: u16) -> Self {
        let Some(structure) = self.structure else {
            return self;
        };
        if damage >= structure.cf {
            return self.collapsed();
        }
        self.with_structure(Some(Structure {
            cf: structure.cf - damage,
            ..structure
        }))
    }

    /// The construction class of this hex's structure, if it has one.
    pub fn construction_class(self) -> Option<ConstructionClass> {
        self.structure.map(|structure| structure.class)
    }

    /// Supported standing surface: ice holds units at the water surface, a bridge or other
    /// structure at its top, and anything else at its bottom.
    pub fn standing_height(self) -> i16 {
        if self.is_ice() {
            return i16::from(self.level);
        }
        self.surface_height()
    }

    /// Whether entering this hex at a rounded jump altitude hits an obstacle.
    /// Water entry uses immersion; bridge spans permit passage below their underside.
    /// This predicate does not authorize a route or move the unit.
    pub fn blocks_jump_entry(self, altitude: i32) -> bool {
        if self.is_open_water() {
            return false;
        }
        if let Some(deck) = self.deck_height() {
            return altitude < i32::from(self.level) || altitude == i32::from(deck) - 1;
        }
        altitude < i32::from(self.surface_height())
    }

    /// Bridge contact checked during vertical integration, before the hex transition.
    /// Unlike entry checks, this stage only collides above the water surface.
    pub fn strikes_bridge_during_jump(self, altitude: i32) -> bool {
        self.deck_height().is_some()
            && altitude > i32::from(self.level)
            && self.blocks_jump_entry(altitude)
    }

    /// Height ground movement treats as this hex's surface: a structure's top, a bridge deck,
    /// the bed under water, or the ground.
    pub fn surface_height(self) -> i16 {
        match self.structure {
            Some(structure) => i16::from(self.level) + i16::from(structure.height),
            None => self.bottom_height(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The layer the notation's digit sets: water depth, structure top or bridge deck, or
    /// ground height.
    fn digit(hex: Hex) -> u8 {
        if hex.is_water_surface() {
            return hex.water_depth();
        }
        hex.top_height() as u8
    }

    /// Every terrain and digit of the compact notation survives the layers unchanged and
    /// makes a valid hex.
    #[test]
    fn notation_round_trips_through_layers() {
        for terrain in Terrain::ALL {
            for elevation in 0..=9 {
                let hex = Hex::new(terrain, elevation);
                hex.validate().unwrap();
                assert_eq!((hex.terrain(), digit(hex)), (terrain, elevation));
                let saved = serde_json::to_value(hex).unwrap();
                assert_eq!(serde_json::from_value::<Hex>(saved).unwrap(), hex);
            }
        }
    }

    /// Heights follow what the compact notation describes for every terrain and digit.
    #[test]
    fn heights_match_the_single_symbol_rules() {
        for terrain in Terrain::ALL {
            for elevation in 0..=9 {
                let hex = Hex::new(terrain, elevation);
                let digit = i16::from(elevation);
                let surface = if matches!(terrain, Terrain::Water | Terrain::Ice) {
                    -digit
                } else {
                    digit
                };
                assert_eq!(hex.surface_height(), surface, "{terrain:?} {elevation}");
                assert_eq!(
                    hex.standing_height(),
                    if terrain == Terrain::Ice { 0 } else { surface }
                );
                for altitude in -12..=12 {
                    let blocks = match terrain {
                        Terrain::Water => false,
                        Terrain::Bridge => altitude < 0 || altitude == i32::from(elevation) - 1,
                        _ => altitude < i32::from(surface),
                    };
                    assert_eq!(hex.blocks_jump_entry(altitude), blocks);
                    assert_eq!(
                        hex.strikes_bridge_during_jump(altitude),
                        terrain == Terrain::Bridge && altitude > 0 && blocks
                    );
                }
            }
        }
    }

    #[test]
    fn layers_separate_what_the_digit_used_to_mean() {
        let forest = Hex::new(Terrain::HeavyWoods, 3);
        assert_eq!(
            (forest.level(), forest.foliage(), forest.water()),
            (3, Some(Foliage::HeavyWoods), None)
        );
        let ice = Hex::new(Terrain::Ice, 4);
        assert_eq!(ice.level(), 0);
        assert_eq!(ice.water(), Some(Water::still(4)));
        assert!(ice.is_frozen() && ice.is_ice() && ice.is_icy());
        let bridge = Hex::new(Terrain::Bridge, 2);
        assert_eq!(bridge.structure(), Some(Structure::bridge(2)));
        assert_eq!(bridge.water().map(|water| water.depth), Some(1));
        assert_eq!(
            Hex::new(Terrain::Ice, 4).with_surface_broken(),
            Hex::new(Terrain::Water, 4)
        );
        assert_eq!(
            Hex::new(Terrain::Bridge, 3).with_surface_broken(),
            Hex::new(Terrain::Water, 1)
        );
        let building = Hex::new(Terrain::Building, 5);
        assert_eq!(building.structure(), Some(Structure::building(5)));
        assert_eq!(building.level(), 0);
    }

    /// Ice is a condition: it can coat dry ground as well as freeze water, but only water
    /// ice can break.
    #[test]
    fn ice_coats_dry_ground_or_freezes_water() {
        let icy_road = Hex::new(Terrain::Road, 2).with_condition(Some(Condition::Ice));
        icy_road.validate().unwrap();
        assert!(icy_road.is_icy() && !icy_road.is_ice() && !icy_road.is_frozen());
        assert_eq!(
            icy_road.terrain(),
            Terrain::Road,
            "the road sets the movement cost"
        );
        let icy_field = Hex::new(Terrain::Clear, 2).with_condition(Some(Condition::Ice));
        assert_eq!(icy_field.terrain(), Terrain::Ice);
        assert_eq!(icy_road.thawed(), Hex::new(Terrain::Road, 2));
        assert_eq!(
            Hex::new(Terrain::Road, 2).frozen(),
            Hex::new(Terrain::Road, 2)
        );
        let frozen_bridge = Hex::new(Terrain::Bridge, 2).frozen();
        assert!(frozen_bridge.is_frozen() && !frozen_bridge.is_ice());
        assert!(!frozen_bridge.is_open_water());
        assert_eq!(frozen_bridge.terrain(), Terrain::Bridge);
    }

    /// The validator rejects combinations the rules do not allow.
    #[test]
    fn validation_rejects_impossible_combinations() {
        let water = Hex::new(Terrain::Water, 2);
        let road = Hex::new(Terrain::Road, 0);
        let building = Hex::new(Terrain::Building, 2);
        for hex in [
            water.with_foliage(Some(Foliage::LightWoods)),
            water.with_route(Some(Route::DirtRoad)),
            water.with_condition(Some(Condition::DeepSnow)),
            water.with_ground(Ground::Rough),
            Hex::new(Terrain::Water, 0).with_water(Some(Water {
                depth: 0,
                flow: crate::Flow::Torrent,
            })),
            Hex::new(Terrain::Magma, 0).with_foliage(Some(Foliage::LightJungle)),
            Hex::new(Terrain::Magma, 0).with_condition(Some(Condition::Ice)),
            Hex::new(Terrain::Pavement, 0).with_foliage(Some(Foliage::HeavyWoods)),
            building.with_foliage(Some(Foliage::LightWoods)),
            building.with_route(Some(Route::PavedRoad)),
            road.with_structure(Some(Structure::bridge(2))),
            road.with_structure(Some(Structure {
                cf: 0,
                ..Structure::wall(1)
            })),
            Hex::at_level(36),
        ] {
            assert!(hex.validate().is_err(), "{hex:?}");
        }
        for hex in [
            Hex::new(Terrain::Rough, 1).with_foliage(Some(Foliage::UltraHeavyJungle)),
            Hex::new(Terrain::HeavyWoods, 1).with_route(Some(Route::DirtRoad)),
            Hex::new(Terrain::HeavyWoods, 1).with_condition(Some(Condition::DeepSnow)),
            building.with_condition(Some(Condition::ThinSnow)),
            Hex::new(Terrain::Bridge, 3).with_water(Some(Water {
                depth: 2,
                flow: crate::Flow::Rapids,
            })),
            Hex::new(Terrain::Swamp, 0).with_foliage(Some(Foliage::LightWoods)),
        ] {
            hex.validate().unwrap();
        }
    }

    /// A structure stands on its ground, so its top can rise past what one digit could say.
    #[test]
    fn structures_stand_on_raised_ground() {
        let tower = Hex::at_level(8).with_structure(Some(Structure::building(7)));
        assert_eq!(tower.surface_height(), 15);
        assert_eq!(tower.top_height(), 15);
        assert!(tower.blocks_jump_entry(14));
        assert!(!tower.blocks_jump_entry(15));
        tower.validate().unwrap();
        assert_eq!(height_glyph(15), 'f');
        assert_eq!(height_glyph(36), '?');
        assert!(Hex::at_level(36).validate().is_err());
        let too_deep = Hex::at_level(0).with_water(Some(Water::still(10)));
        assert!(too_deep.validate().is_err());
    }

    /// Damage wears down a structure's construction factor and collapses it at zero.
    #[test]
    fn structures_collapse_into_rubble() {
        let heavy = Structure::new(StructureKind::Building, 4, ConstructionClass::Heavy);
        let tower = Hex::at_level(2).with_structure(Some(heavy));
        let damaged = tower.with_structure_damage(50);
        assert_eq!(damaged.structure().map(|s| s.cf), Some(40));
        assert_eq!(damaged.construction_class(), Some(ConstructionClass::Heavy));
        let fallen = damaged.with_structure_damage(40);
        assert_eq!(fallen, Hex::at_level(2).with_ground(Ground::UltraRubble));
        assert_eq!(
            Hex::new(Terrain::Wall, 2)
                .with_structure_damage(99)
                .ground(),
            Ground::Rubble
        );
        let bridge = Hex::new(Terrain::Bridge, 2).frozen();
        assert_eq!(
            bridge.with_structure_damage(40),
            Hex::new(Terrain::Water, 1)
        );
        assert_eq!(
            Hex::new(Terrain::Clear, 1).with_structure_damage(5),
            Hex::new(Terrain::Clear, 1)
        );
    }

    /// Fire and smoke lie over a hex's layers without replacing them.
    #[test]
    fn overlays_keep_the_layers_they_cover() {
        let woods = Hex::new(Terrain::HeavyWoods, 3);
        let burning = woods.with_overlay(Some(DecorationKind::Fire));
        assert_eq!((burning.terrain(), burning.level()), (Terrain::Fire, 3));
        assert_eq!(burning.foliage(), Some(Foliage::HeavyWoods));
        assert_eq!(burning.with_overlay(None), woods);
        let smoky = Hex::new(Terrain::Building, 4).with_overlay(Some(DecorationKind::Smoke));
        assert_eq!(smoky.structure(), Some(Structure::building(4)));
        assert_eq!(smoky.surface_height(), 4);
        assert_eq!(
            serde_json::to_value(Hex::new(Terrain::Fire, 1)).unwrap(),
            serde_json::json!({"level": 1, "ground": "clear", "overlay": "fire"})
        );
    }

    /// Hexes are saved as their layers, leaving out the ones that are absent.
    #[test]
    fn saved_shape_names_each_layer() {
        assert_eq!(
            serde_json::to_value(Hex::new(Terrain::Clear, 2)).unwrap(),
            serde_json::json!({"level": 2, "ground": "clear"})
        );
        assert_eq!(
            serde_json::to_value(Hex::new(Terrain::HeavyWoods, 1)).unwrap(),
            serde_json::json!({"level": 1, "ground": "clear", "foliage": "heavy_woods"})
        );
        assert_eq!(
            serde_json::to_value(Hex::new(Terrain::Ice, 2)).unwrap(),
            serde_json::json!({"level": 0, "ground": "clear", "water": {"depth": 2}, "condition": "ice"})
        );
        assert_eq!(
            serde_json::to_value(Hex::new(Terrain::Bridge, 3)).unwrap(),
            serde_json::json!({
                "level": 0,
                "ground": "clear",
                "water": {"depth": 1},
                "structure": {"kind": "bridge", "class": "medium", "height": 3, "cf": 40}
            })
        );
        assert!(
            serde_json::from_value::<Hex>(serde_json::json!({"terrain": "road", "elevation": 1}))
                .is_err()
        );
    }
}
