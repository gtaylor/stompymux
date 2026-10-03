//! The `stompymux-rs serve` command reads its listener and text paths from the game's TOML
//! configuration and accepts command-line listener overrides.
use crate::{copy, repository_root};
use std::{path::Path, time::Duration};
use stompymux_rs::{Config, accounts, persistence};

/// Copy the relational game fixture into a fresh temporary directory.
fn game() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    copy(&repository_root().join("tests/fixtures/game"), d.path());
    d
}

/// Set one dotted `key` in the copied game's `stompymux.toml`.
fn put(dir: &Path, key: &str, value: toml::Value) {
    let path = dir.join("stompymux.toml");
    let mut document: toml::Value =
        toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut table = document.as_table_mut().unwrap();
    let parts: Vec<_> = key.split('.').collect();
    for part in &parts[..parts.len() - 1] {
        table = table
            .entry((*part).to_owned())
            .or_insert_with(|| toml::Value::Table(Default::default()))
            .as_table_mut()
            .unwrap();
    }
    table.insert(parts[parts.len() - 1].into(), value);
    std::fs::write(path, toml::to_string(&document).unwrap()).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn cli_uses_toml_listener_paths_and_optional_overrides() {
    use tokio::{
        io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
        net::TcpStream,
        process::Command,
    };
    let d = game();
    put(d.path(), "server.port", 0.into());
    put(d.path(), "server.listen_address", "127.0.0.2".into());
    put(
        d.path(),
        "mux.connect_file",
        "text/custom-connect.txt".into(),
    );
    put(d.path(), "mux.quit_file", "text/custom-quit.txt".into());
    put(d.path(), "runtime.input_line_limit", 128.into());
    put(d.path(), "security.login_history_limit", 1.into());
    std::fs::write(
        d.path().join("text/custom-connect.txt"),
        "Configured banner\r\n",
    )
    .unwrap();
    std::fs::write(
        d.path().join("text/custom-quit.txt"),
        "Configured goodbye\r\n",
    )
    .unwrap();
    let c = Config::load(d.path()).unwrap();
    persistence::load(&c.database()).await.unwrap();
    let mut w = persistence::load(&c.database()).await.unwrap();
    w.accounts.get_mut(&stompymux_rs::ObjectId(2)).unwrap().hash =
        Some(accounts::hash("secret", &c).unwrap());
    w.accounts
        .get_mut(&stompymux_rs::ObjectId(2))
        .unwrap()
        .history = (0..10)
        .map(|_| stompymux_rs::Login {
            success: true,
            at: 0,
            host: "old".into(),
        })
        .collect();
    persistence::save(&c.database(), &w).await.unwrap();
    async fn until(stream: &mut TcpStream, needle: &str) {
        tokio::time::timeout(Duration::from_secs(5), async {
            let mut collected = Vec::new();
            loop {
                let mut b = [0; 4096];
                let n = stream.read(&mut b).await.unwrap();
                assert!(n > 0, "closed waiting for {needle}");
                collected.extend_from_slice(&b[..n]);
                if String::from_utf8_lossy(&collected).contains(needle) {
                    break;
                }
            }
        })
        .await
        .unwrap();
    }
    for args in [vec![], vec!["--listen-address", "127.0.0.1", "--port", "0"]] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_stompymux-rs"))
            .args(["serve", "--game-dir", d.path().to_str().unwrap()])
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut reader = BufReader::new(child.stdout.take().unwrap());
        let mut ready = String::new();
        tokio::time::timeout(Duration::from_secs(10), reader.read_line(&mut ready))
            .await
            .unwrap()
            .unwrap();
        let address = ready
            .trim()
            .strip_prefix("Listening on ")
            .expect("server ready");
        assert!(address.starts_with(if args.is_empty() {
            "127.0.0.2:"
        } else {
            "127.0.0.1:"
        }));
        let mut stream = TcpStream::connect(address).await.unwrap();
        until(&mut stream, "Configured banner").await;
        stream.write_all(b"#2\r\n").await.unwrap();
        until(&mut stream, "Password: ").await;
        stream.write_all(b"secret\r\n").await.unwrap();
        until(&mut stream, "Staff Nexus").await;
        let persisted = persistence::load(&c.database()).await.unwrap();
        assert_eq!(
            persisted.accounts[&stompymux_rs::ObjectId(2)].history.len(),
            1
        );
        stream.write_all(b"quit\r\n").await.unwrap();
        until(&mut stream, "Configured goodbye").await;
        child.kill().await.unwrap();
        child.wait().await.unwrap();
    }
}
