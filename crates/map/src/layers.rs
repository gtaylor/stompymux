//! The layers a battlefield hex is built from, named after the categories of Tactical
//! Operations' planetary conditions rules.
//!
//! - [`Ground`] is the hex's base terrain type: clear, pavement, rough, sand and so on. Water
//!   is its own layer ([`Water`]) since it has a depth and can be spanned by a bridge.
//! - [`Foliage`] is the vegetation standing on dry ground: woods, jungle or planted fields.
//! - [`Route`] is a road or rail line running through the hex, which units travelling along
//!   it use instead of the terrain underneath.
//! - [`Structure`] is something built in the hex: a building, wall or bridge, with its own
//!   construction factor.
//! - [`Condition`] is a terrain modification that comes and goes with the weather: ice, snow
//!   or mud lying over whatever the hex holds.
//! - [`DecorationKind`] is fire or smoke, which the live map tracks as burning effects.
use serde::{Deserialize, Serialize};

/// The base terrain type: what the ground of a dry hex is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ground {
    /// Open, firm ground: grass, scrub or bare earth.
    Clear,
    /// Paved ground such as a parking lot, plaza or runway.
    Pavement,
    /// Broken ground, boulders and gullies.
    Rough,
    /// Shattered ground that is very hard to cross.
    UltraRough,
    /// Wreckage of light and medium structures.
    Rubble,
    /// Wreckage of heavy and hardened structures.
    UltraRubble,
    /// Loose sand that slows wheels.
    Sand,
    /// Boggy, half-frozen ground.
    Tundra,
    /// Waterlogged ground that slows everything but hovercraft.
    Swamp,
    /// Cooled magma that can still crack.
    MagmaCrust,
    /// Molten rock.
    Magma,
    /// A dense tangle of factories, pipework and storage.
    HeavyIndustrial,
}

impl Ground {
    /// Every ground, in palette order.
    pub const ALL: [Self; 12] = [
        Self::Clear,
        Self::Pavement,
        Self::Rough,
        Self::UltraRough,
        Self::Rubble,
        Self::UltraRubble,
        Self::Sand,
        Self::Tundra,
        Self::Swamp,
        Self::MagmaCrust,
        Self::Magma,
        Self::HeavyIndustrial,
    ];

    /// Snake_case name shared by serialization and Lua.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::Pavement => "pavement",
            Self::Rough => "rough",
            Self::UltraRough => "ultra_rough",
            Self::Rubble => "rubble",
            Self::UltraRubble => "ultra_rubble",
            Self::Sand => "sand",
            Self::Tundra => "tundra",
            Self::Swamp => "swamp",
            Self::MagmaCrust => "magma_crust",
            Self::Magma => "magma",
            Self::HeavyIndustrial => "heavy_industrial",
        }
    }

    /// Display name for editors and reports.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Clear => "Clear",
            Self::Pavement => "Pavement",
            Self::Rough => "Rough",
            Self::UltraRough => "Ultra rough",
            Self::Rubble => "Rubble",
            Self::UltraRubble => "Ultra rubble",
            Self::Sand => "Sand",
            Self::Tundra => "Tundra",
            Self::Swamp => "Swamp",
            Self::MagmaCrust => "Magma crust",
            Self::Magma => "Magma",
            Self::HeavyIndustrial => "Heavy industrial",
        }
    }

    /// Whether vegetation can grow here: anything but molten rock and paving.
    pub const fn supports_foliage(self) -> bool {
        !matches!(self, Self::Magma | Self::Pavement | Self::HeavyIndustrial)
    }
}

/// How dense a stand of woods or jungle is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Density {
    Light,
    Heavy,
    UltraHeavy,
}

impl Density {
    /// Every density, thinnest first.
    pub const ALL: [Self; 3] = [Self::Light, Self::Heavy, Self::UltraHeavy];

    /// The next thinner density, or `None` for light.
    pub const fn thinner(self) -> Option<Self> {
        match self {
            Self::Light => None,
            Self::Heavy => Some(Self::Light),
            Self::UltraHeavy => Some(Self::Heavy),
        }
    }

    /// To-hit and line-of-sight points: light 1, heavy 2, ultra-heavy 3.
    pub const fn points(self) -> u8 {
        match self {
            Self::Light => 1,
            Self::Heavy => 2,
            Self::UltraHeavy => 3,
        }
    }
}

/// Vegetation standing on dry ground.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Foliage {
    LightWoods,
    HeavyWoods,
    UltraHeavyWoods,
    /// Tropical forest, slower to move through than woods of the same density.
    LightJungle,
    HeavyJungle,
    UltraHeavyJungle,
    /// Crops a level high: they hide what is in them but do not slow anything down.
    PlantedFields,
}

impl Foliage {
    /// Every foliage, in palette order.
    pub const ALL: [Self; 7] = [
        Self::LightWoods,
        Self::HeavyWoods,
        Self::UltraHeavyWoods,
        Self::LightJungle,
        Self::HeavyJungle,
        Self::UltraHeavyJungle,
        Self::PlantedFields,
    ];

    /// Woods of `density`.
    pub const fn woods(density: Density) -> Self {
        match density {
            Density::Light => Self::LightWoods,
            Density::Heavy => Self::HeavyWoods,
            Density::UltraHeavy => Self::UltraHeavyWoods,
        }
    }

    /// Jungle of `density`.
    pub const fn jungle(density: Density) -> Self {
        match density {
            Density::Light => Self::LightJungle,
            Density::Heavy => Self::HeavyJungle,
            Density::UltraHeavy => Self::UltraHeavyJungle,
        }
    }

    /// Snake_case name shared by serialization and Lua.
    pub const fn name(self) -> &'static str {
        match self {
            Self::LightWoods => "light_woods",
            Self::HeavyWoods => "heavy_woods",
            Self::UltraHeavyWoods => "ultra_heavy_woods",
            Self::LightJungle => "light_jungle",
            Self::HeavyJungle => "heavy_jungle",
            Self::UltraHeavyJungle => "ultra_heavy_jungle",
            Self::PlantedFields => "planted_fields",
        }
    }

    /// Display name for editors and reports.
    pub const fn label(self) -> &'static str {
        match self {
            Self::LightWoods => "Light woods",
            Self::HeavyWoods => "Heavy woods",
            Self::UltraHeavyWoods => "Ultra-heavy woods",
            Self::LightJungle => "Light jungle",
            Self::HeavyJungle => "Heavy jungle",
            Self::UltraHeavyJungle => "Ultra-heavy jungle",
            Self::PlantedFields => "Planted fields",
        }
    }

    /// Density of woods or jungle; planted fields have none.
    pub const fn density(self) -> Option<Density> {
        match self {
            Self::LightWoods | Self::LightJungle => Some(Density::Light),
            Self::HeavyWoods | Self::HeavyJungle => Some(Density::Heavy),
            Self::UltraHeavyWoods | Self::UltraHeavyJungle => Some(Density::UltraHeavy),
            Self::PlantedFields => None,
        }
    }

    /// Whether this is jungle rather than woods or fields.
    pub const fn is_jungle(self) -> bool {
        matches!(
            self,
            Self::LightJungle | Self::HeavyJungle | Self::UltraHeavyJungle
        )
    }

    /// Levels the foliage rises above the ground for line of sight: two for light and heavy
    /// woods or jungle, three for ultra-heavy, one for planted fields.
    pub const fn height(self) -> u8 {
        match self.density() {
            Some(Density::UltraHeavy) => 3,
            Some(_) => 2,
            None => 1,
        }
    }

    /// This foliage cut back one density, or `None` when it is cleared entirely.
    pub const fn thinned(self) -> Option<Self> {
        let Some(density) = self.density() else {
            return None;
        };
        let Some(thinner) = density.thinner() else {
            return None;
        };
        Some(if self.is_jungle() {
            Self::jungle(thinner)
        } else {
            Self::woods(thinner)
        })
    }
}

/// How fast water runs through a hex.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Flow {
    /// Still water, or a lazy river.
    #[default]
    Still,
    /// Fast water that makes footing harder.
    Rapids,
    /// A torrent that can sweep units off their feet.
    Torrent,
}

impl Flow {
    /// Every flow, slowest first.
    pub const ALL: [Self; 3] = [Self::Still, Self::Rapids, Self::Torrent];

    /// Whether the water is still.
    pub const fn is_still(&self) -> bool {
        matches!(self, Self::Still)
    }

    /// Display name for editors and reports.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Still => "Still",
            Self::Rapids => "Rapids",
            Self::Torrent => "Torrent",
        }
    }
}

/// Standing water whose surface is at the hex's ground level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Water {
    /// Levels from the surface down to the bed.
    pub depth: u8,
    /// How fast the water runs; rapids and torrents need at least one level of depth.
    #[serde(default, skip_serializing_if = "Flow::is_still")]
    pub flow: Flow,
}

impl Water {
    /// Still water `depth` levels deep.
    pub const fn still(depth: u8) -> Self {
        Self {
            depth,
            flow: Flow::Still,
        }
    }
}

/// A road or rail line running through a hex. Units travelling along it pay for the route
/// rather than for the terrain it passes through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    PavedRoad,
    GravelRoad,
    DirtRoad,
    /// Rail track, which ground units cross like rough ground.
    Rail,
}

impl Route {
    /// Every route, in palette order.
    pub const ALL: [Self; 4] = [
        Self::PavedRoad,
        Self::GravelRoad,
        Self::DirtRoad,
        Self::Rail,
    ];

    /// Whether this is a road of any surface.
    pub const fn is_road(self) -> bool {
        !matches!(self, Self::Rail)
    }

    /// Display name for editors and reports.
    pub const fn label(self) -> &'static str {
        match self {
            Self::PavedRoad => "Paved road",
            Self::GravelRoad => "Gravel road",
            Self::DirtRoad => "Dirt road",
            Self::Rail => "Rail",
        }
    }
}

/// A terrain modification that comes and goes with the weather, lying over whatever the hex
/// holds. Ice can coat dry ground as well as freeze water; snow and mud lie only on dry ground.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Condition {
    /// An ice coating, or the frozen surface of water.
    Ice,
    /// A dusting of snow that slows only wheels.
    ThinSnow,
    /// Snow deep enough to slow everything but hovercraft.
    DeepSnow,
    /// Mud left by rain or melted snow.
    Mud,
}

impl Condition {
    /// Every condition, in palette order.
    pub const ALL: [Self; 4] = [Self::Ice, Self::ThinSnow, Self::DeepSnow, Self::Mud];

    /// Display name for editors and reports.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ice => "Ice",
            Self::ThinSnow => "Thin snow",
            Self::DeepSnow => "Deep snow",
            Self::Mud => "Mud",
        }
    }
}

/// Visible fire or smoke laid over a hex.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecorationKind {
    Fire,
    Smoke,
}

impl DecorationKind {
    /// Terrain identity used by movement, visibility and map inspection.
    pub fn terrain(self) -> crate::Terrain {
        match self {
            Self::Fire => crate::Terrain::Fire,
            Self::Smoke => crate::Terrain::Smoke,
        }
    }
}

/// What a structure is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StructureKind {
    /// A building standing on the ground; units can stand on its roof.
    Building,
    /// A wall standing on the ground; units can stand on top of it.
    Wall,
    /// A bridge deck spanning water.
    Bridge,
}

impl StructureKind {
    /// Every kind.
    pub const ALL: [Self; 3] = [Self::Building, Self::Wall, Self::Bridge];

    /// Display name for editors and reports.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Building => "Building",
            Self::Wall => "Wall",
            Self::Bridge => "Bridge",
        }
    }
}

/// A structure's construction class, which the rules read from its construction factor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstructionClass {
    /// Construction factor 1 to 15.
    Light,
    /// Construction factor 16 to 40.
    Medium,
    /// Construction factor 41 to 90.
    Heavy,
    /// Construction factor 91 to 150.
    Hardened,
}

/// The greatest construction factor any structure hex may have.
pub const MAX_CONSTRUCTION_FACTOR: u16 = 150;

impl ConstructionClass {
    /// Every class, weakest first.
    pub const ALL: [Self; 4] = [Self::Light, Self::Medium, Self::Heavy, Self::Hardened];

    /// The class a construction factor falls in.
    pub const fn of(cf: u16) -> Self {
        match cf {
            0..=15 => Self::Light,
            16..=40 => Self::Medium,
            41..=90 => Self::Heavy,
            _ => Self::Hardened,
        }
    }

    /// The construction factor a new structure of this class starts with: the top of its range.
    pub const fn construction_factor(self) -> u16 {
        match self {
            Self::Light => 15,
            Self::Medium => 40,
            Self::Heavy => 90,
            Self::Hardened => MAX_CONSTRUCTION_FACTOR,
        }
    }

    /// Display name for editors and reports.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Medium => "Medium",
            Self::Heavy => "Heavy",
            Self::Hardened => "Hardened",
        }
    }

    /// Ground a collapsed structure of this class leaves behind.
    pub const fn rubble(self) -> Ground {
        match self {
            Self::Light | Self::Medium => Ground::Rubble,
            Self::Heavy | Self::Hardened => Ground::UltraRubble,
        }
    }
}

/// One hex of something built: a building, wall or bridge. Each hex of a structure has its
/// own construction factor, which weapon fire wears down until the hex collapses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Structure {
    pub kind: StructureKind,
    /// How it was built, which decides what it collapses into.
    pub class: ConstructionClass,
    /// Levels above the hex's ground: the roof of a building, the top of a wall, or the
    /// surface of a bridge deck.
    pub height: u8,
    /// Construction factor remaining, 1 up to its class's full construction factor.
    pub cf: u16,
}

impl Structure {
    /// A structure of `kind` and `class`, at full construction factor.
    pub const fn new(kind: StructureKind, height: u8, class: ConstructionClass) -> Self {
        Self {
            kind,
            class,
            height,
            cf: class.construction_factor(),
        }
    }

    /// A medium building `height` levels tall.
    pub const fn building(height: u8) -> Self {
        Self::new(StructureKind::Building, height, ConstructionClass::Medium)
    }

    /// A medium wall `height` levels tall.
    pub const fn wall(height: u8) -> Self {
        Self::new(StructureKind::Wall, height, ConstructionClass::Medium)
    }

    /// A medium bridge whose deck is `deck` levels above the water surface.
    pub const fn bridge(deck: u8) -> Self {
        Self::new(StructureKind::Bridge, deck, ConstructionClass::Medium)
    }

    /// Whether the structure is undamaged.
    pub const fn is_intact(self) -> bool {
        self.cf >= self.class.construction_factor()
    }

    /// Whether this is a bridge.
    pub const fn is_bridge(self) -> bool {
        matches!(self.kind, StructureKind::Bridge)
    }

    /// Whether this structure stands on the ground rather than spanning it: a building or wall.
    pub const fn is_standing(self) -> bool {
        !self.is_bridge()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construction classes cover every construction factor, and each class starts at the top
    /// of its own range.
    #[test]
    fn construction_classes_partition_construction_factors() {
        for class in ConstructionClass::ALL {
            assert_eq!(ConstructionClass::of(class.construction_factor()), class);
        }
        assert_eq!(ConstructionClass::of(1), ConstructionClass::Light);
        assert_eq!(ConstructionClass::of(16), ConstructionClass::Medium);
        assert_eq!(ConstructionClass::of(41), ConstructionClass::Heavy);
        assert_eq!(ConstructionClass::of(91), ConstructionClass::Hardened);
        assert_eq!(Structure::wall(2).class, ConstructionClass::Medium);
        assert!(Structure::wall(2).is_intact());
    }

    /// Foliage thins one density at a time and planted fields clear in one step.
    #[test]
    fn foliage_thins_one_density_at_a_time() {
        let ultra = Foliage::UltraHeavyJungle;
        assert_eq!(ultra.thinned(), Some(Foliage::HeavyJungle));
        assert_eq!(Foliage::LightWoods.thinned(), None, "light woods clear");
        assert_eq!(Foliage::PlantedFields.thinned(), None);
        assert_eq!(ultra.height(), 3);
        assert_eq!(Foliage::HeavyWoods.height(), 2);
        assert_eq!(Foliage::PlantedFields.height(), 1);
    }

    /// Layers serialize with the snake_case names scripts use.
    #[test]
    fn layers_serialize_by_name() {
        let name = |value: serde_json::Value| value.as_str().unwrap().to_owned();
        for ground in Ground::ALL {
            assert_eq!(name(serde_json::to_value(ground).unwrap()), ground.name());
        }
        for foliage in Foliage::ALL {
            assert_eq!(name(serde_json::to_value(foliage).unwrap()), foliage.name());
        }
        assert_eq!(
            serde_json::to_value(Water::still(2)).unwrap(),
            serde_json::json!({"depth": 2})
        );
        assert_eq!(
            serde_json::to_value(Structure::bridge(3)).unwrap(),
            serde_json::json!({"kind": "bridge", "class": "medium", "height": 3, "cf": 40})
        );
    }
}
