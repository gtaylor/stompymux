//! Removed `stompymux-rs` subcommands are refused without touching the game database, and
//! saves into empty or missing database files fail without creating or changing them.
use crate::repository_root;
use stompymux_rs::{World, persistence};

/// Use an isolated copy of the populated relational fixture, normalized by one save.
async fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("stompymux.db");
    std::fs::copy(
        repository_root().join("tests/fixtures/game/data/stompymux.db"),
        &path,
    )
    .unwrap();
    let world = persistence::load(&path).await.unwrap();
    persistence::save(&path, &world).await.unwrap();
    (dir, path)
}

#[tokio::test(flavor = "current_thread")]
async fn removed_cli_commands_and_invalid_files_are_nonmutating() {
    let (dir, path) = fixture().await;
    std::fs::write(
        dir.path().join("stompymux.toml"),
        "database.game_database='stompymux.db'",
    )
    .unwrap();
    let before = std::fs::read(&path).unwrap();
    for command in ["check", "import-legacy"] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_stompymux-rs"))
            .arg(command)
            .arg("--game-dir")
            .arg(dir.path())
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("unrecognized subcommand"));
        assert_eq!(before, std::fs::read(&path).unwrap());
    }
    let empty = dir.path().join("empty.db");
    std::fs::write(&empty, []).unwrap();
    assert!(persistence::save(&empty, &World::default()).await.is_err());
    assert!(std::fs::read(empty).unwrap().is_empty());
    let missing = dir.path().join("missing.db");
    assert!(
        persistence::save(&missing, &World::default())
            .await
            .is_err()
    );
    assert!(!missing.exists());
}
