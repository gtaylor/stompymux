//! Shared integration-test infrastructure; scenarios own their world edits and fault injection.
//!
//! Built as a dev-dependency library so every integration suite links one copy and uses
//! whichever subset of helpers its scenarios need.

pub mod autopilot;
pub mod btech_defense;
pub mod btech_firing;
pub mod btech_map_objects;
mod client;
mod commands;
mod database;
mod fixtures;
mod logging;
mod reuse;
mod server;
pub mod templates;
pub use client::Client;
pub use commands::{run_text, run_text_for_player};
pub use database::{stable_world, store_unit_record, unit_record};
pub use fixtures::{copy, isolated_scripts, isolated_world, with_clock_save_interval};
pub use logging::init_logging;
pub use reuse::{attempt_heartbeat, install, restore_database, snapshot_database};
pub use server::start;

/// Write the map file `<name>.toml` into `dir`, built from the compact cell notation.
pub fn write_map(dir: &std::path::Path, name: &str, cells: &str) {
    std::fs::create_dir_all(dir).unwrap();
    let text = stompymux_rs::BattleMapAsset::from_cells(cells)
        .unwrap()
        .to_file()
        .unwrap();
    std::fs::write(dir.join(format!("{name}.toml")), text).unwrap();
}

/// Rewrite one hex of serialized world state through [`stompymux_rs::BattleHex`].
pub fn edit_hex(
    tile: &mut serde_json::Value,
    change: impl FnOnce(stompymux_rs::BattleHex) -> stompymux_rs::BattleHex,
) {
    let hex = serde_json::from_value(tile.take()).expect("serialized hex");
    *tile = serde_json::to_value(change(hex)).unwrap();
}

/// Replace one serialized hex outright with the hex a terrain and elevation digit describe.
pub fn set_hex(tile: &mut serde_json::Value, terrain: stompymux_rs::Terrain, elevation: u8) {
    *tile = serde_json::to_value(stompymux_rs::BattleHex::new(terrain, elevation)).unwrap();
}

/// The digit the compact notation of [`stompymux_rs::BattleHex::new`] gives `hex`: water
/// depth, structure top or bridge deck, or ground height.
pub fn notation_digit(hex: stompymux_rs::BattleHex) -> u8 {
    if hex.is_water_surface() {
        return hex.water_depth();
    }
    u8::try_from(hex.top_height()).unwrap()
}

/// Rebuild one serialized hex from the compact notation with a new terrain, keeping its digit.
pub fn set_hex_terrain(tile: &mut serde_json::Value, terrain: stompymux_rs::Terrain) {
    edit_hex(tile, |hex| {
        stompymux_rs::BattleHex::new(terrain, notation_digit(hex))
    });
}

/// Rebuild one serialized hex from the compact notation with a new digit, keeping its terrain.
pub fn set_hex_elevation(tile: &mut serde_json::Value, elevation: u8) {
    edit_hex(tile, |hex| {
        stompymux_rs::BattleHex::new(hex.terrain(), elevation)
    });
}

/// Make a later neighboring-wood ignition fail after preceding mine effects have run.
/// A map without active fire may lack its random stream; ignition requires it.
pub fn fail_mine_ignition(world: &mut stompymux_rs::World, map: stompymux_rs::ObjectId, y: usize) {
    let mut state = serde_json::to_value(&world.btech).unwrap();
    let record = &mut state["maps"][map.0.to_string()];
    let width = record["width"].as_u64().unwrap() as usize;
    set_hex_terrain(
        &mut record["terrain"][y * width],
        stompymux_rs::Terrain::LightForest,
    );
    record["fire_dice"] = serde_json::Value::Null;
    world.btech = serde_json::from_value(state).unwrap();
}
