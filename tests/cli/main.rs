//! Integration tests that run the server package's own executables, `stompymux-rs` and
//! `megamek-convert`, as child processes.
//!
//! Cargo only defines `CARGO_BIN_EXE_<name>` for integration tests in the package that owns
//! those binaries, so these tests live in the server package. Every other scenario runs from
//! the `stompymux-suites` package in `tests/suites`. This target must not use
//! `stompymux-test-support`: as a dev-dependency of the server package it would hold the
//! server library's unit-test build until the library itself finished compiling.

mod btech_megamek_convert;
mod configuration;
mod foundation;
mod persistence;

use std::path::{Path, PathBuf};
use stompymux_rs::persistence as store;

/// The repository root, which holds `game/` and `tests/fixtures/`.
pub fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Recursively copy a fixture directory into `target`, creating it if needed.
pub fn copy(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let destination = target.join(entry.file_name());
        if entry.path().is_dir() {
            copy(&entry.path(), &destination);
        } else {
            std::fs::copy(entry.path(), destination).unwrap();
        }
    }
}

/// Load a saved world as JSON with its BattleTech tick counters zeroed. A live server
/// advances and commits the simulation clock every second, so read-only scenarios compare
/// this snapshot rather than raw database bytes.
pub async fn stable_world(database: &Path) -> serde_json::Value {
    let world = store::load(database).await.unwrap();
    let mut value = serde_json::to_value(&world).unwrap();
    let btech = value
        .get_mut("btech")
        .expect("serialized btech state")
        .as_object_mut()
        .expect("btech state object");
    btech.insert("turn_clock".into(), 0.into());
    btech.insert("simulation_seconds".into(), 0.into());
    if let Some(reactor) = btech.get_mut("reactor").and_then(|r| r.as_object_mut()) {
        reactor.insert("startup_remaining".into(), 0.into());
    }
    value
}
