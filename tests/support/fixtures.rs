//! Isolated fixture copying for tests that edit game configuration, Lua, or SQLite.
use std::path::{Path, PathBuf};
use stompymux_rs::{Config, Scripts, World, persistence};

/// The repository root, which holds `game/` and `tests/fixtures/`.
///
/// Integration suites build in their own package, so their `CARGO_MANIFEST_DIR` is not the
/// repository root; resolve every repository path from here instead.
pub fn repository_root() -> PathBuf {
    let support = Path::new(env!("CARGO_MANIFEST_DIR"));
    support
        .parent()
        .and_then(Path::parent)
        .expect("tests/support sits two levels below the repository root")
        .to_path_buf()
}

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

/// Copy and load the unchanged relational game fixture, with every saved dice stream reseeded
/// from [`crate::FIXTURE_DICE_SEED`].
pub async fn isolated_world() -> (tempfile::TempDir, Config, World) {
    crate::init_logging();
    let directory = tempfile::tempdir().unwrap();
    copy(
        &repository_root().join("tests/fixtures/game"),
        directory.path(),
    );
    let config = Config::load(directory.path()).unwrap();
    let mut world = persistence::load(&config.database()).await.unwrap();
    crate::seed_world_dice(&mut world, crate::FIXTURE_DICE_SEED);
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

/// Copy the unchanged relational game fixture and initialize its Lua runtime, reseeding any
/// dice stream its startup scripts created.
pub async fn isolated_scripts() -> (tempfile::TempDir, Config, Scripts) {
    let (directory, config, world) = isolated_world().await;
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    crate::seed_world_dice(&mut scripts.world_mut(), crate::FIXTURE_DICE_SEED);
    (directory, config, scripts)
}
