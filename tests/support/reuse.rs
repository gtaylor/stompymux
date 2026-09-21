//! Sandbox and clock reuse so scenario matrices share one fixture copy and VM pair.
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

/// Advance the shared runtime clock past one heartbeat instead of waiting out a
/// real second. Denied commits leave no observable database change, so the
/// settle only bounds the attempt; the following retry polls stay the oracle.
pub async fn attempt_heartbeat() {
    tokio::time::pause();
    tokio::time::advance(std::time::Duration::from_secs(1)).await;
    tokio::time::resume();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
}
