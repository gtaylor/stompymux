//! C connection-report semantics and real TCP operational commands.
use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::Rc,
    time::Duration,
};
use stompymux_rs::{
    Config, Flag, ObjectId, Scripts, ShutdownRequest, accounts,
    commands::{self, Action, ExecutionContext, InputOrigin},
    operations, persistence,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
mod support;
use support::{Client, copy, stable_world, start};
async fn fixture() -> (tempfile::TempDir, Config) {
    let d = tempfile::tempdir().unwrap();
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"),
        d.path(),
    );
    let p = d.path().join("stompymux.toml");
    let mut v: toml::Value = toml::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    v["security"]["password_hash_opslimit"] = 1.into();
    v["security"]["password_hash_memlimit"] = 1048576.into();
    // Permit the four connections exercised by this accounting test.
    v["security"]["login_attempt_burst"] = 10.into();
    v.as_table_mut()
        .unwrap()
        .entry("runtime")
        .or_insert(toml::Value::Table(Default::default()))
        .as_table_mut()
        .unwrap()
        .insert("output_message_limit".into(), 256.into());
    std::fs::write(
        p,
        toml::to_string(&v).unwrap() + "\n[aliases.commands]\n\"@aw\"=\"@who\"\nver=\"version\"\n",
    )
    .unwrap();
    let c = Config::load(d.path()).unwrap();
    credentials(&c).await;
    (d, c)
}
/// Seed known credentials and a non-Wizard exclusively in the temporary database.
async fn credentials(c: &Config) {
    let mut world = persistence::load(&c.database()).await.unwrap();
    for id in [ObjectId(1), ObjectId(2)] {
        world.accounts.get_mut(&id).unwrap().hash = Some(accounts::hash("secret", c).unwrap());
    }
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    persistence::save(&c.database(), &world).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn permissions_aliases_switches_and_background() {
    let (_d, mut c) = fixture().await;
    let mut s = Scripts::new(
        &c,
        Rc::new(RefCell::new(
            persistence::load(&c.database()).await.unwrap(),
        )),
    )
    .unwrap();
    for (line, expected) in [
        ("version", operations::VERSION),
        ("ver", operations::VERSION),
        ("version x", "Usage: version"),
        ("version/no", "Unsupported command switch."),
    ] {
        assert!(
            matches!(commands::run(&s,&c,ObjectId(2),1,line).unwrap(),Action::Report(commands::Report::Reply(ref x)) if x==expected)
        );
    }
    assert!(
        matches!(commands::run(&s,&c,ObjectId(2),1,"@aw").unwrap(),Action::Report(commands::Report::Reply(ref x)) if x=="Permission denied.")
    );
    assert!(
        matches!(commands::run(&s,&c,ObjectId(1),1,"@aw wizard").unwrap(),Action::Server(commands::ServerRequest::Who(ref x)) if x=="wizard")
    );
    assert!(
        matches!(commands::run(&s,&c,ObjectId(1),1,"@who/no").unwrap(),Action::Report(commands::Report::Reply(ref x)) if x.contains("switch"))
    );
    assert!(matches!(
        commands::run(&s, &c, ObjectId(1), 1, "@list pr").unwrap(),
        Action::Server(commands::ServerRequest::ProcessReport)
    ));
    let ctx = ExecutionContext {
        executor: ObjectId(1),
        cause: ObjectId(1),
        session: None,
        origin: InputOrigin::Queued,
    };
    assert!(
        matches!(commands::execute(&s,&c,ctx,"@who").unwrap(),Action::Report(commands::Report::Reply(ref x)) if x=="@who is only available from an active connection.")
    );
    assert!(
        matches!(commands::execute(&s,&c,ctx,"version").unwrap(),Action::Report(commands::Report::Reply(ref x)) if x==operations::VERSION)
    );
    assert!(matches!(
        commands::execute(&s, &c, ctx, "@list process").unwrap(),
        Action::Server(commands::ServerRequest::ProcessReport)
    ));
    s.world_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(Flag::Wizard);
    assert!(matches!(
        commands::run(&s, &c, ObjectId(2), 1, "@aw").unwrap(),
        Action::Server(commands::ServerRequest::Who(_))
    ));
    for (directive, value) in [
        ("access", "@who !wizard god"),
        ("list_access", "process !wizard god"),
    ] {
        let candidate = c
            .administer(
                &stompymux_rs::config::administration::Request {
                    directive: directive.into(),
                    value: value.into(),
                },
                &s.world(),
                ObjectId(1),
                s.commands(),
            )
            .unwrap();
        s.configure(&candidate.config).unwrap();
        c = candidate.config;
    }
    for line in ["@aw", "@list process"] {
        assert!(
            matches!(commands::run(&s,&c,ObjectId(2),1,line).unwrap(),Action::Report(commands::Report::Reply(ref x)) if x=="Permission denied.")
        );
    }
    c.logger.shutdown(&c).await.unwrap();
}

/// Reference: C connection_commands.c dump_users; columns use visible Unicode widths.
#[test]
fn who_fields_unicode_markers_and_hosts() {
    let rows = vec![
        operations::WhoRow {
            name: "界é long name beyond column budget".into(),
            connected: 3601,
            idle: 5,
            markers: "D+".into(),
            location: 42,
            commands: 12,
            host: "::1".into(),
        },
        operations::WhoRow {
            name: "Other".into(),
            connected: 0,
            idle: 0,
            markers: String::new(),
            location: 1,
            commands: 0,
            host: "127.0.0.1".into(),
        },
    ];
    let text = operations::who_report(&rows, 5, -1);
    assert!(
        text.contains("#42")
            && text.contains("D+")
            && text.contains("::1")
            && text.contains("127.0.0.1")
    );
    assert!(text.ends_with("2 Players logged in, 5 record, no maximum."));
    assert!(
        operations::who_report(&[], 5, 20).ends_with("0 Players logged in, 5 record, 20 maximum.")
    );
    assert!(
        operations::who_report(&[], 5, -2).ends_with("0 Players logged in, 5 record, -2 maximum.")
    );
    let lines: Vec<_> = text.lines().collect();
    let a = lines[1].find("::1").unwrap();
    let b = lines[2].find("127.0.0.1").unwrap();
    use unicode_width::UnicodeWidthStr;
    assert_eq!(lines[1][..a].width(), lines[2][..b].width());
}

/// C hides short idle intervals only in the privileged session report.
#[test]
fn session_idle_hides_ten_minutes_or_less() {
    assert_eq!(operations::session_idle(0), "0s");
    assert_eq!(operations::session_idle(600), "0s");
    assert_ne!(operations::session_idle(601), "0s");
}

/// Extract command counts without depending on display padding or connection time.
fn counts(text: &str) -> Vec<u64> {
    text.lines()
        .filter(|line| line.trim_end().ends_with("127.0.0.1"))
        .map(|line| {
            let cells: Vec<_> = line.split_whitespace().collect();
            cells[cells.len() - 2].parse().unwrap()
        })
        .collect()
}
#[tokio::test(flavor = "current_thread")]
async fn tcp_private_reports_counts_idle_and_queue() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (_d, c) = fixture().await;
            let mut world = persistence::load(&c.database()).await.unwrap();
            world
                .objects
                .get_mut(&ObjectId(1))
                .unwrap()
                .flags
                .insert(Flag::Dark);
            use stompymux_rs::macros::{MacroEntry, MacroSet, MacroSlots};
            world.macros.sets.push(MacroSet {
                origin: Default::default(),
                owner: ObjectId(1),
                modes: Default::default(),
                description: "operational test".into(),
                entries: vec![MacroEntry {
                    origin: Default::default(),
                    alias: "ver".into(),
                    expansion: "version".into(),
                }],
            });
            let index = world.macros.sets.len() - 1;
            world.macros.players.insert(
                ObjectId(1),
                MacroSlots {
                    current: Some(index),
                    slots: [Some(index), None, None, None, None],
                },
            );
            persistence::save(&c.database(), &world).await.unwrap();
            let (address, shutdown, task, _) = start(&c, Rc::new(Cell::new(0))).await;
            let mut first = Client::connect(address, 1).await;
            let mut second = Client::connect(address, 1).await;
            let mut player = Client::connect(address, 2).await;
            let before = stable_world(&c.database()).await;
            first.send("@aw").await;
            let text = first.until("maximum.").await;
            assert_eq!(counts(&text), [1, 0, 0]);
            assert!(text.split_whitespace().any(|token| token == "D"));
            assert!(text.contains("3 Players logged in") && text.contains("D"));
            first.socket.write_all(b"iD").await.unwrap();
            first
                .socket
                .write_all(b"lE\r\nversion\r\n@who GOD\r\n")
                .await
                .unwrap();
            first.until(operations::VERSION).await;
            let text = first.until("maximum.").await;
            assert_eq!(counts(&text), [3, 0]);
            first.send("IDLE argument").await;
            first.until("Huh?").await;
            first.send("@who GOD").await;
            assert_eq!(counts(&first.until("maximum.").await), [5, 0]);
            player.send("@who").await;
            player.until("Permission denied.").await;
            first.send("@who Wizard").await;
            assert_eq!(counts(&first.until("maximum.").await), [1]);
            first.send("@list process").await;
            first.until("Descriptor limits:").await;
            second.send("ver").await;
            let own = second.until(operations::VERSION).await;
            assert!(!own.contains("Player Name") && !own.contains("Process ID:"));
            assert_eq!(before, stable_world(&c.database()).await);
            first.send("@wait 0={version;@list process;@who}").await;
            first
                .until("@who is only available from an active connection.")
                .await;
            first.send("@who GOD").await;
            assert_eq!(counts(&first.until("maximum.").await), [9, 1]);
            first.send("flow-demo confirm").await;
            first.until("Really do the thing?").await;
            first.send("IDLE").await;
            first.until("Please answer y or n:").await;
            first.send("n").await;
            first.until("Cancelled.").await;
            first.send("@who GOD").await;
            assert_eq!(counts(&first.until("maximum.").await), [11, 1]);
            first.send(".ver").await;
            first.until(operations::VERSION).await;
            first.send("@who GOD").await;
            assert_eq!(counts(&first.until("maximum.").await), [13, 1]);
            second.send("quit").await;
            tokio::time::timeout(
                Duration::from_secs(5),
                second.socket.read_to_end(&mut Vec::new()),
            )
            .await
            .unwrap()
            .unwrap();
            let _replacement = Client::connect(address, 1).await;
            first.send("@who GOD").await;
            assert_eq!(counts(&first.until("maximum.").await), [14, 0]);
            shutdown.send(ShutdownRequest::Sigterm).unwrap();
            task.await.unwrap().unwrap();
        })
        .await;
}
