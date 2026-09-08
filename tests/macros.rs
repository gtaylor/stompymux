//! Player macro commands, selective row moves, rollback and schema-32 compatibility.
use sqlx::{Connection, SqliteConnection};
use std::{cell::RefCell, path::Path, rc::Rc};
use stompymux_rs::{
    Config, Flag, ObjectId, Scripts,
    commands::{self, Action},
    persistence,
};

mod support;
use support::copy;

async fn fixture() -> (tempfile::TempDir, Config, Scripts) {
    let d = tempfile::tempdir().unwrap();
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"),
        d.path(),
    );
    let c = Config::load(d.path()).unwrap();
    let mut w = persistence::load(&c.database()).await.unwrap();
    w.objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    let s = Scripts::new(&c, Rc::new(RefCell::new(w))).unwrap();
    (d, c, s)
}

fn run(s: &Scripts, c: &Config, who: i64, line: &str) -> String {
    let action = commands::run(s, c, ObjectId(who), 1, line).unwrap();
    let mut output = s
        .drain_outbox()
        .into_iter()
        .map(|(_, d)| d.source().to_string())
        .collect::<Vec<_>>();
    if let Action::Report(commands::Report::Reply(text)) | Action::CommitReply(text) = action {
        output.push(text);
    }
    output.join("\n")
}

async fn save(c: &Config, s: &Scripts) -> anyhow::Result<()> {
    let snapshot = s.world().clone();
    persistence::save(&c.database(), &snapshot).await
}

async fn sql(c: &Config) -> SqliteConnection {
    SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(c.database())
            .foreign_keys(false),
    )
    .await
    .unwrap()
}

#[tokio::test(flavor = "current_thread")]
async fn commands_sharing_modes_slots_and_case_insensitive_expansion() {
    let (_d, c, s) = fixture().await;
    assert!(run(&s, &c, 2, ".create Personal").contains("set 0"));
    assert!(run(&s, &c, 2, ".def Hi=say hello * %*").contains("defined"));
    assert!(run(&s, &c, 2, ".def hI=say duplicate").contains("already defined"));
    assert!(run(&s, &c, 2, ".HI world").contains("hello world *"));
    assert!(run(&s, &c, 2, ".undef HI").contains("deleted"));
    run(&s, &c, 2, ".def lua=global-hello");
    assert!(run(&s, &c, 2, ".LUA ignored").contains("Hello, world"));
    run(&s, &c, 2, ".def god=@shutdown");
    assert!(run(&s, &c, 2, ".god").contains("Permission denied"));
    assert!(run(&s, &c, 1, ".add 0").contains("added"));
    assert!(run(&s, &c, 1, ".list").contains("Current slot: none"));
    run(&s, &c, 1, ".chslot 0");
    assert!(run(&s, &c, 1, ".name no").contains("Permission denied"));
    assert!(run(&s, &c, 1, ".def x=look").contains("Permission denied"));
    run(&s, &c, 2, ".chmod W");
    assert!(run(&s, &c, 1, ".name Shared").contains("Shared"));
    run(&s, &c, 2, ".chmod L");
    assert!(run(&s, &c, 1, ".name no").contains("Permission denied"));
    assert!(run(&s, &c, 2, ".clear").contains("locked"));
    run(&s, &c, 1, ".chmod !L");
    assert!(run(&s, &c, 2, ".chown #1").contains("Permission denied"));
    run(&s, &c, 1, ".chown #1");
    run(&s, &c, 1, ".chmod L");
    assert!(run(&s, &c, 1, ".clear").contains("locked"));
    run(&s, &c, 1, ".chmod !L");
    run(&s, &c, 1, ".chmod !W");
    assert!(run(&s, &c, 2, ".gex 0").contains("Permission denied"));
    assert!(run(&s, &c, 2, ".ex").contains("global-hello"));
    assert!(run(&s, &c, 2, ".lua").contains("Hello, world"));
    for slot in 1..5 {
        assert!(run(&s, &c, 1, ".add 0").contains(&format!("{slot} slot")));
    }
    assert!(run(&s, &c, 1, ".create full").contains("already have 5"));
    run(&s, &c, 1, ".clear");
    assert!(s.world().macros.sets.is_empty());
    assert!(
        s.world()
            .macros
            .players
            .values()
            .all(|s| s.current.is_none() && s.slots.iter().all(Option::is_none))
    );
    save(&c, &s).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn slot_precedence_single_pass_limits_and_private_inspection() {
    let (d, _, s) = fixture().await;
    let path = d.path().join("stompymux.toml");
    let base =
        std::fs::read_to_string(&path).unwrap() + "\n[aliases.commands]\ngreet='global-hello'\n";
    std::fs::write(&path, &base).unwrap();
    let c = Config::load(d.path()).unwrap();
    run(&s, &c, 2, ".create first");
    run(&s, &c, 2, ".def x=say first");
    run(&s, &c, 2, ".create second");
    run(&s, &c, 2, ".def x=say second");
    assert!(run(&s, &c, 2, ".x").contains("first"));
    run(&s, &c, 2, ".def no=.create forbidden");
    run(&s, &c, 2, ".no");
    assert_eq!(s.world().macros.sets.len(), 2);
    run(&s, &c, 2, ".def re=.x");
    assert!(!run(&s, &c, 2, ".re").contains("first"));
    run(&s, &c, 2, ".def lua=greet");
    assert!(run(&s, &c, 2, ".lua").contains("Hello, world"));
    run(&s, &c, 2, ".def huge=say **");
    std::fs::write(&path, format!("{base}\n[runtime]\ninput_line_limit=10\n")).unwrap();
    let c = Config::load(d.path()).unwrap();
    assert!(run(&s, &c, 2, ".huge abcdef").contains("exceeds"));
    run(&s, &c, 2, ".del 0");
    assert!(run(&s, &c, 2, ".x").contains("second"));
    assert!(matches!(
        commands::run(&s, &c, ObjectId(2), 99, ".list").unwrap(),
        Action::Report(commands::Report::Reply(_))
    ));
    assert!(matches!(
        commands::run(&s, &c, ObjectId(2), 99, ".list/bad").unwrap(),
        Action::Report(commands::Report::Reply(_))
    ));
    assert!(run(&s, &c, 2, ".def toooo=look").contains("1–4"));
    assert!(run(&s, &c, 2, ".def é=look").contains("ASCII"));
    assert!(run(&s, &c, 2, ".def x=").contains("substitute"));
    std::fs::write(
        &path,
        format!("{base}\n[runtime]\noutput_message_limit=64\n"),
    )
    .unwrap();
    let c = Config::load(d.path()).unwrap();
    assert!(run(&s, &c, 2, ".glist").contains("[truncated]"));
}

#[tokio::test(flavor = "current_thread")]
async fn existing_rows_compact_with_extension_identity_and_restart() {
    let (_d, c, s) = fixture().await;
    let mut db = sql(&c).await;
    sqlx::raw_sql("ALTER TABLE macro_sets ADD COLUMN opaque BLOB; ALTER TABLE macro_entries ADD COLUMN opaque BLOB; INSERT INTO macro_sets VALUES(0,1,128,'same',X'00'),(1,1,128,'same',X'01'),(2,1,128,'same',X'02'); INSERT INTO macro_entries VALUES(2,0,'b','look',X'62'),(2,1,'d','global-hello',X'64'); INSERT INTO commac_entries VALUES(1,0,1,2,2,-1,-1),(2,1,2,1,-1,-1,-1);").execute(&mut db).await.unwrap();
    *s.world_mut() = persistence::load(&c.database()).await.unwrap();
    let image = std::fs::read(c.database()).unwrap();
    save(&c, &s).await.unwrap();
    assert_eq!(std::fs::read(c.database()).unwrap(), image);
    run(&s, &c, 1, ".clear");
    save(&c, &s).await.unwrap();
    let opaque: Vec<(i64, Vec<u8>)> =
        sqlx::query_as("SELECT set_index,opaque FROM macro_sets ORDER BY set_index")
            .fetch_all(&mut db)
            .await
            .unwrap();
    assert_eq!(opaque, vec![(0, vec![0]), (1, vec![2])]);
    let w = persistence::load(&c.database()).await.unwrap();
    assert_eq!(
        w.macros.players[&ObjectId(2)].slots,
        [Some(1), None, None, None, None]
    );
    assert_eq!(w.macros.players[&ObjectId(2)].current, None);
    run(&s, &c, 1, ".chslot 1");
    run(&s, &c, 1, ".def a=say a");
    save(&c, &s).await.unwrap();
    run(&s, &c, 1, ".undef B");
    save(&c, &s).await.unwrap();
    let entries: Vec<(i64, String, Option<Vec<u8>>)> = sqlx::query_as(
        "SELECT position,alias,opaque FROM macro_entries WHERE set_index=1 ORDER BY position",
    )
    .fetch_all(&mut db)
    .await
    .unwrap();
    assert_eq!(
        entries,
        vec![(0, "a".into(), None), (1, "d".into(), Some(vec![100]))]
    );
    run(&s, &c, 1, ".chmod R");
    save(&c, &s).await.unwrap();
    assert_eq!(
        persistence::load(&c.database()).await.unwrap().macros.sets[1]
            .modes
            .0,
        130
    );
    let loaded = persistence::load(&c.database()).await.unwrap();
    assert_eq!(
        loaded.macros.expand(ObjectId(2), ".D", 8191).unwrap(),
        Some("global-hello".into())
    );
    db.close().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn failed_reindex_rolls_back_keys_and_does_not_advance_row_origins() {
    let (_d, c, s) = fixture().await;
    run(&s, &c, 2, ".create first");
    run(&s, &c, 2, ".create second");
    run(&s, &c, 2, ".def b=look");
    save(&c, &s).await.unwrap();
    let before = s.world().clone();
    let mut db = sql(&c).await;
    sqlx::raw_sql("CREATE TRIGGER deny_macro BEFORE UPDATE ON commac_entries BEGIN SELECT RAISE(FAIL,'blocked macro slots'); END;").execute(&mut db).await.unwrap();
    run(&s, &c, 2, ".chslot 0");
    run(&s, &c, 2, ".clear");
    assert!(save(&c, &s).await.is_err());
    let unchanged = persistence::load(&c.database()).await.unwrap();
    assert_eq!(
        serde_json::to_value(&unchanged.macros).unwrap(),
        serde_json::to_value(&before.macros).unwrap()
    );
    *s.world_mut() = before;
    sqlx::query("DROP TRIGGER deny_macro")
        .execute(&mut db)
        .await
        .unwrap();
    run(&s, &c, 2, ".chslot 0");
    run(&s, &c, 2, ".clear");
    save(&c, &s).await.unwrap();
    assert_eq!(
        persistence::load(&c.database()).await.unwrap().macros.sets[0].description,
        "second"
    );
    sqlx::raw_sql("CREATE TABLE extension(set_id INTEGER REFERENCES macro_sets(set_index)); INSERT INTO extension VALUES(0)").execute(&mut db).await.unwrap();
    run(&s, &c, 2, ".chslot 1");
    run(&s, &c, 2, ".clear");
    assert!(format!("{:#}", save(&c, &s).await.unwrap_err()).contains("unknown dependency"));
    assert_eq!(
        persistence::load(&c.database())
            .await
            .unwrap()
            .macros
            .sets
            .len(),
        1
    );
    db.close().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn malformed_storage_fails_contextually_and_is_never_normalized() {
    for (sql_text, expected) in [
        ("INSERT INTO macro_sets VALUES(1,1,0,'gap')", "contiguous"),
        ("INSERT INTO macro_sets VALUES(0,999,0,'bad')", "owner #999"),
        (
            "INSERT INTO macro_sets VALUES(0,1,0,'bad'); INSERT INTO macro_entries VALUES(0,1,'x','look')",
            "position",
        ),
        (
            "INSERT INTO macro_sets VALUES(0,1,0,'bad'); INSERT INTO macro_entries VALUES(0,0,'x','look'),(0,1,'X','look')",
            "duplicate alias",
        ),
        (
            "INSERT INTO commac_entries VALUES(1,0,99,-1,-1,-1,-1)",
            "slot 0",
        ),
        (
            "INSERT INTO commac_entries VALUES(1,8,-1,-1,-1,-1,-1)",
            "current slot",
        ),
    ] {
        let (_d, c, _s) = fixture().await;
        let mut db = sql(&c).await;
        sqlx::raw_sql(sql_text).execute(&mut db).await.unwrap();
        let image = std::fs::read(c.database()).unwrap();
        let error = persistence::load(&c.database()).await.unwrap_err();
        assert!(format!("{error:#}").contains(expected), "{error:#}");
        assert_eq!(std::fs::read(c.database()).unwrap(), image);
        db.close().await.unwrap();
    }
}

#[tokio::test(flavor = "current_thread")]
async fn purge_compacts_surviving_sets_and_resets_affected_selections() {
    let (_d, c, s) = fixture().await;
    run(&s, &c, 1, ".create survivor-first");
    run(&s, &c, 2, ".create doomed");
    run(&s, &c, 1, ".create survivor-last");
    run(&s, &c, 1, ".def hi=say survived");
    run(&s, &c, 1, ".add 1");
    run(&s, &c, 1, ".chslot 2");
    save(&c, &s).await.unwrap();
    let mut db = sql(&c).await;
    sqlx::raw_sql("ALTER TABLE macro_sets ADD COLUMN extension TEXT; UPDATE macro_sets SET extension=description;").execute(&mut db).await.unwrap();
    let mut w = s.world().clone();
    w.objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(Flag::Going);
    let report = persistence::repair(&c.database(), c.database.busy_timeout_ms, |raw| {
        stompymux_rs::dbck::plan(&w, raw, &c)
    })
    .await
    .unwrap();
    assert!(report.plan.purges.contains(&ObjectId(2)));
    let loaded = persistence::load(&c.database()).await.unwrap();
    assert_eq!(loaded.macros.sets.len(), 2);
    assert!(!loaded.macros.players.contains_key(&ObjectId(2)));
    assert_eq!(loaded.macros.players[&ObjectId(1)].current, None);
    assert_eq!(
        loaded.macros.players[&ObjectId(1)].slots,
        [Some(0), Some(1), None, None, None]
    );
    assert_eq!(
        loaded.macros.expand(ObjectId(1), ".hi", 8191).unwrap(),
        Some("say survived".into())
    );
    let values: Vec<String> =
        sqlx::query_scalar("SELECT extension FROM macro_sets ORDER BY set_index")
            .fetch_all(&mut db)
            .await
            .unwrap();
    assert_eq!(values, ["survivor-first", "survivor-last"]);
    persistence::save(&c.database(), &loaded).await.unwrap();
    db.close().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn unknown_required_columns_refuse_insert_without_losing_existing_rows() {
    let (_d, c, s) = fixture().await;
    let mut db = sql(&c).await;
    sqlx::query("ALTER TABLE macro_sets ADD COLUMN required TEXT NOT NULL")
        .execute(&mut db)
        .await
        .unwrap();
    let image = std::fs::read(c.database()).unwrap();
    run(&s, &c, 2, ".create rejected");
    let error = save(&c, &s).await.unwrap_err();
    assert!(format!("{error:#}").contains("writing macro set 0"));
    assert_eq!(std::fs::read(c.database()).unwrap(), image);
    assert!(
        persistence::load(&c.database())
            .await
            .unwrap()
            .macros
            .sets
            .is_empty()
    );
    db.close().await.unwrap();
}
