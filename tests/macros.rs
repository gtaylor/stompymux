//! Player macro commands, selective row moves, rollback and schema-32 compatibility.
use sqlx::{Connection, SqliteConnection};
use std::{cell::RefCell, path::Path, rc::Rc};
use stompymux_rs::{
    Config, Flag, ObjectId, Scripts,
    commands::{self, Action},
    persistence,
};

use crate::support;
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

/// Open the database as the server would leave it, in write-ahead-log mode.
async fn sql(c: &Config) -> SqliteConnection {
    SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(c.database())
            .foreign_keys(false)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal),
    )
    .await
    .unwrap()
}

/// Database file bytes with every committed write folded in from the write-ahead log.
async fn settled_image(db: &mut SqliteConnection, c: &Config) -> Vec<u8> {
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(&mut *db)
        .await
        .unwrap();
    std::fs::read(c.database()).unwrap()
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
    let image = settled_image(&mut db, &c).await;
    save(&c, &s).await.unwrap();
    assert_eq!(settled_image(&mut db, &c).await, image);
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
        let image = settled_image(&mut db, &c).await;
        let error = persistence::load(&c.database()).await.unwrap_err();
        assert!(format!("{error:#}").contains(expected), "{error:#}");
        assert_eq!(settled_image(&mut db, &c).await, image);
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
    let image = settled_image(&mut db, &c).await;
    run(&s, &c, 2, ".create rejected");
    let error = save(&c, &s).await.unwrap_err();
    assert!(format!("{error:#}").contains("writing macro set 0"));
    assert_eq!(settled_image(&mut db, &c).await, image);
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

/// Trusted Lua CRUD, typed flags, detached records and player attachment semantics.
#[tokio::test(flavor = "current_thread")]
async fn lua_macro_api_contract() {
    let (_d, c, s) = fixture().await;
    s.eval_callback::<()>(r#"
        local m=mux.macro
        local function fails(code,fn)
            local ok,e=mux.error.pcall(fn)
            assert(not ok and e.code==code,tostring(e))
        end
        assert(#m.list_sets()==0 and m.set(0)==nil)
        local a=m.create_set(2,'  Personal é  ')
        assert(a==m.set(0) and a==m.list_sets()[1])
        assert(a:number()==0 and a:description()=='  Personal é  ')
        assert(a:owner()==mux.world.object(2))
        assert(#m.list_player_sets(2)==0 and #a:list_macros()==0)
        assert(select('#',a:set_owner(mux.world.object(1)))==0)
        assert(a:owner()==mux.world.object(1))
        a:set_owner(0) -- rooms are valid owners
        a:set_owner(2)
        local f=a:flags()
        for _,k in ipairs({'LOCKED','READ','WRITE'}) do
            assert(select('#',f:add(m.flags[k]))==0)
            assert(f:has(m.flags[k]))
        end
        local flags=f:list()
        assert(#flags==3 and flags[1]==m.flags.LOCKED and flags[2]==m.flags.READ and flags[3]==m.flags.WRITE)
        f:remove(m.flags.READ)
        assert(not f:has(m.flags.READ))
        fails('mux.arg.invalid',function() m.flags.READ=2 end)
        fails('mux.arg.invalid',function() m.flags.READ.x=2 end)
        fails('mux.arg.invalid',function() return m.flags.UNKNOWN end)
        fails('mux.arg.invalid',function() f:add(mux.comsys.flags.PUBLIC) end)
        fails('mux.arg.invalid',function() f:add(1) end)
        fails('mux.arg.invalid',function() a.owner=1 end)
        assert(select('#',a:add_macro('Hi',' say é * '))==0) -- trusted even while locked
        a:add_macro('aa','look')
        fails('mux.macro.exists',function() a:add_macro('hI','bad') end)
        fails('mux.macro.not_found',function() a:update_macro('none','bad') end)
        fails('mux.macro.not_found',function() a:delete_macro('none') end)
        assert(select('#',a:update_macro('hI','  say updated  '))==0)
        local entries=a:list_macros()
        assert(entries[1].alias=='aa' and entries[2].alias=='Hi' and entries[2].expansion=='  say updated  ')
        entries[2].expansion='tampered'
        assert(a:list_macros()[2].expansion=='  say updated  ')
        assert(select('#',a:delete_macro('AA'))==0)
        for slot=0,4 do assert(m.attach(2,a)==slot) end
        fails('mux.macro.slots_full',function() m.attach(2,a) end)
        local rows=m.list_player_sets(2)
        assert(#rows==5)
        for i,row in ipairs(rows) do assert(row.slot==i-1 and row.set==a and row.selected==false) end
        rows[1].slot=99
        assert(m.list_player_sets(2)[1].slot==0)
        assert(m.detach(2,2) and not m.detach(2,2) and m.attach(2,a)==2)
        fails('mux.arg.invalid',function() m.detach(2,5) end)
        fails('mux.arg.invalid',function() m.set(-1) end)
        fails('mux.arg.invalid',function() m.set(0.5) end)
        fails('mux.object.invalid',function() m.attach(0,a) end)
        fails('mux.object.invalid',function() m.list_player_sets(0) end)
        fails('mux.object.invalid',function() a:set_owner(999999) end)
        fails('mux.object.invalid',function() m.create_set(999999,'bad') end)
        fails('mux.arg.invalid',function() m.destroy_set(0) end)
        for _,alias in ipairs({'','abcde','a b','é','a\0','a\t'}) do
            fails('mux.arg.invalid',function() a:add_macro(alias,'look') end)
        end
        for _,expansion in ipairs({'','a\0',string.rep('x',8192),string.char(255)}) do
            fails('mux.arg.invalid',function() a:update_macro('hi',expansion) end)
        end
        fails('mux.arg.invalid',function() m.create_set(1,'a\0') end)
        fails('mux.arg.invalid',function() m.create_set(1,string.rep('x',8192)) end)
        a:add_macro('1234',string.rep('x',8191))
        assert(#a:list_macros()[1].expansion==8191)
        a:delete_macro('1234')
        assert(#a:list_macros()==1 and #m.list_sets()==1)
        local b=m.create_set(1,'second') -- full attachment list does not prevent creation
        assert(b:number()==1)
        assert(m.attach(1,a)==0 and m.attach(1,b)==1)
        assert(select('#',m.destroy_set(a))==0)
        assert(b:number()==0 and b==m.set(0))
        assert(#m.list_player_sets(2)==0)
        assert(#m.list_player_sets(1)==1 and m.list_player_sets(1)[1].slot==1)
        fails('mux.macro.invalid',function() a:number() end)
        fails('mux.macro.invalid',function() f:list() end)
        fails('mux.macro.invalid',function() m.attach(1,a) end)
        assert(m.create_set(1,'replacement')~=a)
    "#).unwrap();
    save(&c, &s).await.unwrap();
    let loaded = persistence::load(&c.database()).await.unwrap();
    assert_eq!(loaded.macros.sets.len(), 2);
    assert_eq!(
        loaded.macros.players[&ObjectId(1)].slots,
        [None, Some(0), None, None, None]
    );
}

/// Handles retained by Lua cannot revive after rollback, and restored handles remain usable.
#[tokio::test(flavor = "current_thread")]
async fn lua_macro_handles_follow_commit_and_rollback() {
    let (_d, c, s) = fixture().await;
    s.eval_callback::<()>(
        "kept=mux.macro.create_set(2,'kept'); kept:add_macro('Hi','look'); kept_flags=kept:flags()",
    )
    .unwrap();
    save(&c, &s).await.unwrap();
    assert!(
        s.eval_callback::<()>(
            "rolled=mux.macro.create_set(1,'rolled'); mux.macro.destroy_set(kept); error('abort')"
        )
        .is_err()
    );
    s.eval_callback::<()>(
        r#"
        assert(kept==mux.macro.set(0) and kept:number()==0)
        assert(#kept_flags:list()==0)
        local replacement=mux.macro.create_set(1,'replacement')
        assert(replacement~=rolled)
        local ok,e=mux.error.pcall(function() rolled:number() end)
        assert(not ok and e.code=='mux.macro.invalid')
        kept:update_macro('hi','say changed')
    "#,
    )
    .unwrap();
    save(&c, &s).await.unwrap();
    s.eval_callback::<()>(
        "assert(kept:number()==0 and kept:list_macros()[1].expansion=='say changed')",
    )
    .unwrap();
    assert_eq!(
        persistence::load(&c.database()).await.unwrap().macros.sets[0].entries[0].expansion,
        "say changed"
    );
}

/// Lua attaches privately and edits locked sets, while commands retain their authority policy.
#[tokio::test(flavor = "current_thread")]
async fn lua_macro_trusted_access_and_selection() {
    let (_d, c, s) = fixture().await;
    run(&s, &c, 2, ".create player set");
    s.eval_callback::<()>(
        r#"
        local m=mux.macro
        local a=m.set(0)
        assert(m.list_player_sets(2)[1].selected)
        assert(m.detach(2,0))
        assert(m.attach(2,a)==0 and not m.list_player_sets(2)[1].selected)
        a:flags():add(m.flags.LOCKED)
        a:add_macro('Hi','say trusted')
        m.attach(1,a)
    "#,
    )
    .unwrap();
    run(&s, &c, 2, ".chslot 0");
    assert!(run(&s, &c, 2, ".def no=look").contains("Permission denied"));
    assert!(run(&s, &c, 2, ".Hi").contains("trusted"));
    s.eval_callback::<()>(
        "mux.macro.destroy_set(mux.macro.set(0)); assert(#mux.macro.list_player_sets(2)==0)",
    )
    .unwrap();
    assert_eq!(s.world().macros.players[&ObjectId(2)].current, None);
}

/// Live functions are unavailable during loading/checking; typed constants stay available.
#[tokio::test(flavor = "current_thread")]
async fn lua_macro_availability() {
    let (_d, c, s) = fixture().await;
    let calls = r#"
        assert(type(mux.macro.flags.LOCKED)=='userdata')
        for _,call in ipairs({
            function() mux.macro.list_sets() end,
            function() mux.macro.set(0) end,
            function() mux.macro.create_set(1,'x') end,
            function() mux.macro.destroy_set(nil) end,
            function() mux.macro.attach(1,nil) end,
            function() mux.macro.detach(1,0) end,
            function() mux.macro.list_player_sets(1) end
        }) do
            local ok,e=mux.error.pcall(call)
            assert(not ok and e.code==expected,tostring(e))
        end
    "#;
    s.inspect_lua()
        .load(format!("local expected='mux.state.unavailable'\n{calls}"))
        .exec()
        .unwrap();
    let checking = s
        .from_sources_for_inspection(
            &c,
            stompymux_rs::help::HelpIndex::load(&c).unwrap(),
            std::sync::Arc::new(stompymux_rs::lua::sources::Sources::read(&c).unwrap()),
            stompymux_rs::lua::RuntimeMode::Checking,
        )
        .unwrap();
    checking
        .inspect_lua()
        .load(format!(
            "local expected='mux.unavailable.checking'\n{calls}"
        ))
        .exec()
        .unwrap();
}

/// Lua updates preserve extension data through entry and set compaction; failed saves are atomic.
#[tokio::test(flavor = "current_thread")]
async fn lua_macro_persistence_extensions_and_failure() {
    let (_d, c, s) = fixture().await;
    s.eval_callback::<()>(
        r#"
        mux.macro.create_set(1,'first')
        local s=mux.macro.create_set(2,'survivor')
        s:add_macro('a','look'); s:add_macro('Hi','say initial')
        mux.macro.attach(2,s)
    "#,
    )
    .unwrap();
    save(&c, &s).await.unwrap();
    let mut db = sql(&c).await;
    sqlx::raw_sql("ALTER TABLE macro_sets ADD COLUMN opaque TEXT; ALTER TABLE macro_entries ADD COLUMN opaque TEXT; UPDATE macro_sets SET opaque=description,status=128; UPDATE macro_entries SET opaque=alias;").execute(&mut db).await.unwrap();
    let world = persistence::load(&c.database()).await.unwrap();
    let s = Scripts::new(&c, Rc::new(RefCell::new(world))).unwrap();
    s.eval_callback::<()>(
        r#"
        survivor=mux.macro.set(1)
        survivor:update_macro('hI','say updated')
        survivor:delete_macro('a')
        survivor:flags():add(mux.macro.flags.READ)
        survivor:flags():remove(mux.macro.flags.WRITE)
        mux.macro.destroy_set(mux.macro.set(0))
    "#,
    )
    .unwrap();
    save(&c, &s).await.unwrap();
    let row: (String, i64) =
        sqlx::query_as("SELECT opaque,status FROM macro_sets WHERE set_index=0")
            .fetch_one(&mut db)
            .await
            .unwrap();
    assert_eq!(row, ("survivor".into(), 130));
    let row: (String, String, String) = sqlx::query_as(
        "SELECT alias,expansion,opaque FROM macro_entries WHERE set_index=0 AND position=0",
    )
    .fetch_one(&mut db)
    .await
    .unwrap();
    assert_eq!(row, ("Hi".into(), "say updated".into(), "Hi".into()));
    sqlx::raw_sql("CREATE TRIGGER deny_macro_update BEFORE UPDATE ON macro_entries BEGIN SELECT RAISE(FAIL,'blocked macro update'); END;").execute(&mut db).await.unwrap();
    s.eval_callback::<()>("survivor:update_macro('hi','must not persist')")
        .unwrap();
    assert!(save(&c, &s).await.is_err());
    let loaded = persistence::load(&c.database()).await.unwrap();
    assert_eq!(loaded.macros.sets[0].entries[0].expansion, "say updated");
    assert_eq!(loaded.macros.players[&ObjectId(2)].slots[0], Some(0));
    db.close().await.unwrap();
}

/// Creation resolves the current catalog, including sets installed later by Lua bootstrapping.
#[tokio::test(flavor = "current_thread")]
async fn default_player_macros_resolve_at_creation_and_persist() {
    let (d, _, s) = fixture().await;
    let path = d.path().join("stompymux.toml");
    let mut config: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    config["mux"]["default_player_macros"] = toml::Value::Array(vec![0.into()]);
    std::fs::write(&path, toml::to_string(&config).unwrap()).unwrap();
    let c = Config::load(d.path()).unwrap();
    let early = s
        .world_mut()
        .create_with(
            &c,
            "Before bootstrap".into(),
            stompymux_rs::Kind::Player,
            stompymux_rs::CreationContext::Player,
        )
        .unwrap();
    assert!(!s.world().macros.players.contains_key(&early));
    s.eval_callback::<()>(
        r#"
        local a=mux.macro.create_set(1,'bootstrap default')
        assert(a:number()==0)
        a:add_macro('hi','say hello')
        a:flags():add(mux.macro.flags.LOCKED)
        mux.macro.create_set(1,'second')
        mux.macro.create_set(1,'third')
    "#,
    )
    .unwrap();
    let later = s
        .world_mut()
        .create_with(
            &c,
            "After bootstrap".into(),
            stompymux_rs::Kind::Player,
            stompymux_rs::CreationContext::Player,
        )
        .unwrap();
    assert_eq!(
        s.world().macros.players[&later].slots,
        [Some(0), None, None, None, None]
    );
    assert_eq!(s.world().macros.players[&later].current, None);
    assert_eq!(
        s.world().macros.expand(later, ".hi", 8191).unwrap(),
        Some("say hello".into())
    );
    assert!(!s.world().macros.players.contains_key(&early));
    config["mux"]["default_player_macros"] =
        toml::Value::Array(vec![2.into(), 999.into(), 0.into(), 2.into()]);
    std::fs::write(&path, toml::to_string(&config).unwrap()).unwrap();
    let c = Config::load(d.path()).unwrap();
    let multi = s
        .world_mut()
        .create(&c, "Multiple defaults".into(), stompymux_rs::Kind::Player);
    assert_eq!(
        s.world().macros.players[&multi].slots,
        [Some(2), Some(0), Some(2), None, None]
    );
    let room = s
        .world_mut()
        .create(&c, "No room macros".into(), stompymux_rs::Kind::Room);
    assert!(!s.world().macros.players.contains_key(&room));
    config["mux"]["default_player_macros"] = toml::Value::Array(vec![]);
    std::fs::write(&path, toml::to_string(&config).unwrap()).unwrap();
    let disabled = Config::load(d.path()).unwrap();
    let bare = s.world_mut().create(
        &disabled,
        "Defaults disabled".into(),
        stompymux_rs::Kind::Player,
    );
    assert!(!s.world().macros.players.contains_key(&bare));
    save(&c, &s).await.unwrap();
    let loaded = persistence::load(&c.database()).await.unwrap();
    assert_eq!(
        loaded.macros.players[&multi].slots,
        [Some(2), Some(0), Some(2), None, None]
    );
    assert_eq!(loaded.macros.players[&later].slots[0], Some(0));
}
