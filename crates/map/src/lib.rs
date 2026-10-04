//! Battlefield map data shared by the stompymux server, the map generator and the Mappy
//! editor: layered hexes, terrain identities, map rule flags, hex geometry and the TOML map
//! file format.
//!
//! A [`BattleMapAsset`] is a map as it is stored on disk: dimensions, environment and one
//! [`BattleHex`] per cell. [`BattleMapAsset::parse`] reads a map file and
//! [`BattleMapAsset::to_file`] writes one.
//!
//! ```
//! use stompymux_map::{BattleMapAsset, Terrain};
//!
//! let map = BattleMapAsset::parse("terrain = '.\"'\nlevel = '02'\n").unwrap();
//! assert_eq!(map.hex(1, 0).unwrap().terrain(), Terrain::HeavyForest);
//! assert_eq!(BattleMapAsset::parse(&map.to_file().unwrap()).unwrap(), map);
//! ```
//!
//! This crate knows nothing about units, objects or the game world; the server layers its
//! live map state and rules on top of these types.
mod asset;
mod file;
mod flags;
mod geometry;
mod hex;
mod terrain;
mod terrain_rules;

pub use asset::{BattleMapAsset, MapPointOfInterest};
pub use flags::{BattleMapFlag, format_map_flags, parse_map_flags};
pub use geometry::{BattleHexCoordinate, BattlePoint};
pub use hex::{
    BattleDecorationKind, BattleHex, Ground, MAX_DEPTH, MAX_HEIGHT, Structure, Water, Woods,
    height_glyph,
};
pub use terrain::Terrain;
