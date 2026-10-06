//! Terrain identities: the one feature a map shows for a hex, with the names scripts use and
//! the glyphs of the in-game map and the compact cell notation.
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

/// Terrain identity: the most important thing in a hex, as maps display it. A hex holds
/// several layers at once; [`Hex::terrain`](crate::Hex::terrain) picks the one to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Terrain {
    Clear,
    Pavement,
    Road,
    Rail,
    Rough,
    UltraRough,
    Rubble,
    UltraRubble,
    Sand,
    Tundra,
    Swamp,
    MagmaCrust,
    Magma,
    HeavyIndustrial,
    LightWoods,
    HeavyWoods,
    UltraHeavyWoods,
    LightJungle,
    HeavyJungle,
    UltraHeavyJungle,
    PlantedFields,
    Water,
    Ice,
    ThinSnow,
    DeepSnow,
    Mud,
    Bridge,
    Building,
    Wall,
    Fire,
    Smoke,
}

impl Terrain {
    /// Every terrain, in glyph-table order.
    pub const ALL: [Self; 31] = [
        Self::Clear,
        Self::Pavement,
        Self::Road,
        Self::Rail,
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
        Self::LightWoods,
        Self::HeavyWoods,
        Self::UltraHeavyWoods,
        Self::LightJungle,
        Self::HeavyJungle,
        Self::UltraHeavyJungle,
        Self::PlantedFields,
        Self::Water,
        Self::Ice,
        Self::ThinSnow,
        Self::DeepSnow,
        Self::Mud,
        Self::Bridge,
        Self::Building,
        Self::Wall,
        Self::Fire,
        Self::Smoke,
    ];

    /// Snake_case name shared by serialization, Lua reports and Lua arguments.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::Pavement => "pavement",
            Self::Road => "road",
            Self::Rail => "rail",
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
            Self::LightWoods => "light_woods",
            Self::HeavyWoods => "heavy_woods",
            Self::UltraHeavyWoods => "ultra_heavy_woods",
            Self::LightJungle => "light_jungle",
            Self::HeavyJungle => "heavy_jungle",
            Self::UltraHeavyJungle => "ultra_heavy_jungle",
            Self::PlantedFields => "planted_fields",
            Self::Water => "water",
            Self::Ice => "ice",
            Self::ThinSnow => "thin_snow",
            Self::DeepSnow => "deep_snow",
            Self::Mud => "mud",
            Self::Bridge => "bridge",
            Self::Building => "building",
            Self::Wall => "wall",
            Self::Fire => "fire",
            Self::Smoke => "smoke",
        }
    }

    /// Display name for maps, editors and reports.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Clear => "Clear",
            Self::Pavement => "Pavement",
            Self::Road => "Road",
            Self::Rail => "Rail",
            Self::Rough => "Rough",
            Self::UltraRough => "Ultra Rough",
            Self::Rubble => "Rubble",
            Self::UltraRubble => "Ultra Rubble",
            Self::Sand => "Sand",
            Self::Tundra => "Tundra",
            Self::Swamp => "Swamp",
            Self::MagmaCrust => "Magma Crust",
            Self::Magma => "Magma",
            Self::HeavyIndustrial => "Heavy Industrial",
            Self::LightWoods => "Light Woods",
            Self::HeavyWoods => "Heavy Woods",
            Self::UltraHeavyWoods => "Ultra Woods",
            Self::LightJungle => "Light Jungle",
            Self::HeavyJungle => "Heavy Jungle",
            Self::UltraHeavyJungle => "Ultra Jungle",
            Self::PlantedFields => "Planted Fields",
            Self::Water => "Water",
            Self::Ice => "Ice",
            Self::ThinSnow => "Thin Snow",
            Self::DeepSnow => "Deep Snow",
            Self::Mud => "Mud",
            Self::Bridge => "Bridge",
            Self::Building => "Building",
            Self::Wall => "Wall",
            Self::Fire => "Fire",
            Self::Smoke => "Smoke",
        }
    }

    /// Decode a snake_case terrain name.
    pub fn from_name(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|terrain| terrain.name() == name)
            .with_context(|| format!("unknown terrain {name:?}"))
    }

    /// Decode a terrain glyph.
    pub fn from_symbol(symbol: char) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|terrain| terrain.symbol() == symbol)
            .map_or_else(|| bail!("unknown terrain symbol {symbol:?}"), Ok)
    }

    /// The glyph the in-game map and the compact cell notation show for this terrain.
    pub const fn symbol(self) -> char {
        match self {
            Self::Clear => ' ',
            Self::Pavement => '_',
            Self::Road => '#',
            Self::Rail => '|',
            Self::Rough => '%',
            Self::UltraRough => '^',
            Self::Rubble => ';',
            Self::UltraRubble => '!',
            Self::Sand => '}',
            Self::Tundra => '{',
            Self::Swamp => 'w',
            Self::MagmaCrust => 'm',
            Self::Magma => 'M',
            Self::HeavyIndustrial => '$',
            Self::LightWoods => '`',
            Self::HeavyWoods => '"',
            Self::UltraHeavyWoods => 'W',
            Self::LightJungle => 'j',
            Self::HeavyJungle => 'J',
            Self::UltraHeavyJungle => 'U',
            Self::PlantedFields => 'f',
            Self::Water => '~',
            Self::Ice => '-',
            Self::ThinSnow => '*',
            Self::DeepSnow => '+',
            Self::Mud => ',',
            Self::Bridge => '/',
            Self::Building => '@',
            Self::Wall => '=',
            Self::Fire => '&',
            Self::Smoke => ':',
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Names and symbols are distinct for every terrain and agree with serialization.
    #[test]
    fn terrain_names_and_symbols_round_trip() {
        let mut symbols = std::collections::BTreeSet::new();
        for terrain in Terrain::ALL {
            assert!(symbols.insert(terrain.symbol()), "{terrain:?}");
            assert!(terrain.symbol() != '.', "'.' means clear in grids");
            assert_eq!(Terrain::from_symbol(terrain.symbol()).unwrap(), terrain);
            assert_eq!(Terrain::from_name(terrain.name()).unwrap(), terrain);
            assert_eq!(
                serde_json::to_value(terrain).unwrap(),
                serde_json::Value::from(terrain.name())
            );
        }
        assert!(Terrain::from_name("Heavy_Woods").is_err());
        assert!(Terrain::from_name("\"").is_err());
    }
}
