//! Battlefield map data shared by the stompymux server, the map generator and the Mappy
//! editor: layered hexes, terrain identities, map rule flags and conditions, hex geometry and
//! the TOML map file format.
//!
//! A [`MapAsset`] is a map as it is stored on disk: dimensions, environment and one
//! [`Hex`] per cell. [`MapAsset::parse`] reads a map file and
//! [`MapAsset::to_file`] writes one.
//!
//! ```
//! use stompymux_map::{MapAsset, Terrain};
//!
//! let map = MapAsset::parse("terrain = '..'\nfoliage = '.\"'\nlevel = '02'\n").unwrap();
//! assert_eq!(map.hex(1, 0).unwrap().terrain(), Terrain::HeavyWoods);
//! assert_eq!(MapAsset::parse(&map.to_file().unwrap()).unwrap(), map);
//! ```
//!
//! This crate knows nothing about units, objects or the game world; the server layers its
//! live map state and rules on top of these types.
//!
//! A [`Hex`] is built from layers named after Tactical Operations' planetary conditions:
//! [`Ground`] (the base terrain type) or [`Water`], [`Foliage`], a [`Route`], a
//! [`Structure`] with its own construction factor, a weather [`Condition`] such as ice or snow,
//! and a fire or smoke [`DecorationKind`] overlay.
mod asset;
mod environment;
mod file;
mod flags;
mod geometry;
mod hex;
mod layers;
mod terrain;
mod terrain_rules;

pub use asset::{MapAsset, MapPointOfInterest};
pub use environment::{Light, MAX_VISIBILITY, Wind};
pub use flags::{MapFlag, format_map_flags, parse_map_flags};
pub use geometry::{HexCoordinate, Point};
pub use hex::{Hex, MAX_DEPTH, MAX_HEIGHT, height_glyph};
pub use layers::{
    Condition, ConstructionClass, DecorationKind, Density, Flow, Foliage, Ground,
    MAX_CONSTRUCTION_FACTOR, Route, Structure, StructureKind, Water,
};
pub use terrain::Terrain;
pub use terrain_rules::GroundMovement;
