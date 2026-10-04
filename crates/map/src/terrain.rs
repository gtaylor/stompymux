//! Terrain identities: the one feature a map shows for a hex, with the names scripts use and
//! the symbols of the compact cell notation.
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

/// Terrain identity; its spelling belongs to the map-file codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Terrain {
    Grassland,
    Road,
    LightForest,
    HeavyForest,
    Water,
    Ice,
    Bridge,
    Rough,
    Mountains,
    Fire,
    Smoke,
    Snow,
    Building,
    Wall,
    Sand,
}

impl Terrain {
    /// Every terrain, in symbol-table order.
    pub const ALL: [Self; 15] = [
        Self::Grassland,
        Self::Road,
        Self::LightForest,
        Self::HeavyForest,
        Self::Water,
        Self::Ice,
        Self::Bridge,
        Self::Rough,
        Self::Mountains,
        Self::Fire,
        Self::Smoke,
        Self::Snow,
        Self::Building,
        Self::Wall,
        Self::Sand,
    ];

    /// Snake_case name shared by serialization, Lua reports and Lua arguments.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Grassland => "grassland",
            Self::Road => "road",
            Self::LightForest => "light_forest",
            Self::HeavyForest => "heavy_forest",
            Self::Water => "water",
            Self::Ice => "ice",
            Self::Bridge => "bridge",
            Self::Rough => "rough",
            Self::Mountains => "mountains",
            Self::Fire => "fire",
            Self::Smoke => "smoke",
            Self::Snow => "snow",
            Self::Building => "building",
            Self::Wall => "wall",
            Self::Sand => "sand",
        }
    }

    /// Decode a snake_case terrain name.
    pub fn from_name(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|terrain| terrain.name() == name)
            .with_context(|| format!("unknown terrain {name:?}"))
    }

    /// Decode a canonical terrain symbol without applying asset-file normalization.
    pub fn from_symbol(symbol: char) -> Result<Self> {
        Ok(match symbol {
            ' ' => Self::Grassland,
            '#' => Self::Road,
            '`' => Self::LightForest,
            '"' => Self::HeavyForest,
            '~' => Self::Water,
            '-' => Self::Ice,
            '/' => Self::Bridge,
            '%' => Self::Rough,
            '^' => Self::Mountains,
            '&' => Self::Fire,
            ':' => Self::Smoke,
            '+' => Self::Snow,
            '@' => Self::Building,
            '=' => Self::Wall,
            '}' => Self::Sand,
            _ => bail!("unknown terrain symbol {symbol:?}"),
        })
    }

    /// Encode the canonical symbol used by map assets.
    pub fn symbol(self) -> char {
        match self {
            Self::Grassland => ' ',
            Self::Road => '#',
            Self::LightForest => '`',
            Self::HeavyForest => '"',
            Self::Water => '~',
            Self::Ice => '-',
            Self::Bridge => '/',
            Self::Rough => '%',
            Self::Mountains => '^',
            Self::Fire => '&',
            Self::Smoke => ':',
            Self::Snow => '+',
            Self::Building => '@',
            Self::Wall => '=',
            Self::Sand => '}',
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
            assert_eq!(Terrain::from_symbol(terrain.symbol()).unwrap(), terrain);
            assert_eq!(Terrain::from_name(terrain.name()).unwrap(), terrain);
            assert_eq!(
                serde_json::to_value(terrain).unwrap(),
                serde_json::Value::from(terrain.name())
            );
        }
        assert!(Terrain::from_name("Heavy_Forest").is_err());
        assert!(Terrain::from_name("\"").is_err());
    }
}
