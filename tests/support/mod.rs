//! Shared integration-test infrastructure; scenarios own their world edits and fault injection.
// Each integration-test binary imports this module independently and uses a subset.
#![allow(dead_code, unused_imports)]
pub mod btech_firing;
mod client;
mod commands;
mod database;
mod fixtures;
mod reuse;
mod server;
pub use client::Client;
pub use commands::{run_text, run_text_for_player};
pub use database::stable_world;
pub use fixtures::{copy, isolated_scripts, isolated_world};
pub use reuse::{attempt_heartbeat, install, restore_database, snapshot_database};
pub use server::start;

/// Make a later neighboring-wood ignition fail after preceding mine effects have run.
/// A map without active fire may lack its random stream; ignition requires it.
pub fn fail_mine_ignition(world: &mut stompymux_rs::World, map: stompymux_rs::ObjectId, y: usize) {
    let mut state = serde_json::to_value(&world.btech).unwrap();
    let record = &mut state["maps"][map.0.to_string()];
    let width = record["width"].as_u64().unwrap() as usize;
    record["terrain"][y * width]["terrain"] =
        serde_json::to_value(stompymux_rs::Terrain::LightForest).unwrap();
    record["fire_dice"] = serde_json::Value::Null;
    world.btech = serde_json::from_value(state).unwrap();
}
