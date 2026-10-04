//! Sandbox reuse so scenario matrices share one fixture copy and VM pair.
use stompymux_rs::{Config, Scripts, World};

/// Replace a reused VM's scenario world and drop notices left by earlier bodies.
pub fn install(scripts: &Scripts, world: World) {
    *scripts.world_mut() = world;
    scripts.drain_outbox();
}

/// Snapshot the fixture database before scenarios begin overwriting it.
pub fn snapshot_database(config: &Config) -> std::path::PathBuf {
    let database = config.database();
    let pristine = database.with_file_name("pristine.db");
    std::fs::copy(&database, &pristine).unwrap();
    pristine
}

/// Restore the snapshotted database so scenarios sharing one sandbox and server
/// directory keep the restart isolation a fresh fixture copy used to provide.
pub fn restore_database(config: &Config, pristine: &std::path::Path) {
    let database = config.database();
    let name = database.file_name().unwrap().to_string_lossy();
    for suffix in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(database.with_file_name(format!("{name}{suffix}")));
    }
    std::fs::copy(pristine, &database).unwrap();
}
