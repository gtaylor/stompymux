//! Isolated fixture copying for tests that edit game configuration, Lua, or SQLite.
use std::path::Path;
use stompymux_rs::{Config, Scripts, World, persistence};

/// Recursively copy fixture contents into an isolated directory.
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

/// Copy and load the unchanged relational game fixture.
pub async fn isolated_world() -> (tempfile::TempDir, Config, World) {
    crate::init_logging();
    let directory = tempfile::tempdir().unwrap();
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/game"),
        directory.path(),
    );
    let config = Config::load(directory.path()).unwrap();
    let world = persistence::load(&config.database()).await.unwrap();
    (directory, config, world)
}

/// Reload a copied fixture's configuration with a different clock save interval; the
/// fixture itself saves the clock every second so tests can observe each heartbeat.
pub fn with_clock_save_interval(directory: &Path, seconds: u64) -> Config {
    let path = directory.join("stompymux.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    let edited = text.replace(
        "clock_save_interval = 1\n",
        &format!("clock_save_interval = {seconds}\n"),
    );
    assert_ne!(text, edited, "fixture clock save interval not found");
    std::fs::write(&path, edited).unwrap();
    Config::load(directory).unwrap()
}

/// Copy the unchanged relational game fixture and initialize its Lua runtime.
pub async fn isolated_scripts() -> (tempfile::TempDir, Config, Scripts) {
    let (directory, config, world) = isolated_world().await;
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    (directory, config, scripts)
}
