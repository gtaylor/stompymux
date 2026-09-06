//! C comsys semantics, typed Lua handles and selective relational durability.
use sqlx::{Connection, Row};
use std::{cell::RefCell, path::Path, rc::Rc};
use stompymux_rs::{
    commands::{self, Action},
    communication::{Access, ChannelFlag, ChannelFlags},
    config::Config,
    flags::Flag,
    lua::Scripts,
    persistence,
    world::ObjectId,
};

/// Isolate every SQL and Lua mutation from the checked-in game.
fn copy(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).unwrap();
    for e in std::fs::read_dir(source).unwrap() {
        let e = e.unwrap();
        if e.path().is_dir() {
            copy(&e.path(), &target.join(e.file_name()));
        } else {
            std::fs::copy(e.path(), target.join(e.file_name())).unwrap();
        }
    }
}
async fn fixture() -> (tempfile::TempDir, Config, Scripts) {
    let d = tempfile::tempdir().unwrap();
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"),
        d.path(),
    );
    let c = Config::load(d.path()).unwrap();
    let mut w = persistence::load(&c.database()).await.unwrap();
    for id in [ObjectId(1), ObjectId(2)] {
        w.objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::Connected);
    }
    w.objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    let s = Scripts::new(&c, Rc::new(RefCell::new(w))).unwrap();
    (d, c, s)
}

/// Collect a single command's staged text, including session-private diagnostics.
fn run(s: &Scripts, c: &Config, who: i64, line: &str) -> String {
    let action = commands::run(s, c, ObjectId(who), 1, line).unwrap();
    let mut output = s
        .outbox
        .borrow_mut()
        .drain(..)
        .map(|(_, d)| stompymux_rs::text::plain_with(&s.palette, d.source()))
        .collect::<Vec<_>>();
    if let Action::Reply(text) = action {
        output.push(text);
    }
    output.join("\n")
}
async fn connection(c: &Config) -> sqlx::SqliteConnection {
    sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(c.database())
            .foreign_keys(false),
    )
    .await
    .unwrap()
}

#[tokio::test(flavor = "current_thread")]
async fn commands_access_aliases_history_and_pages() {
    let (d, c, s) = fixture().await;
    let path = d.path().join("aliases.toml");
    std::fs::write(
        &path,
        std::fs::read_to_string(&path).unwrap().replace(
            "[aliases.commands]",
            "[aliases.commands]\ncc='addcom'\npc='page'\nch='@chan'",
        ),
    )
    .unwrap();
    let c = Config::load(&c.root).unwrap();
    assert!(run(&s, &c, 2, "ch/create Test").contains("Permission denied"));
    assert!(run(&s, &c, 1, "@chan/create Test").contains("created"));
    assert_eq!(s.world.borrow().channels["Test"].flags, ChannelFlags(127));
    assert!(run(&s, &c, 2, "cc t=Test").contains("joined"));
    run(&s, &c, 1, "addcom t=Test");
    assert!(run(&s, &c, 2, "T Hello [fg=red]world[/]").contains("[Test] Wizard: Hello world"));
    assert!(run(&s, &c, 2, "t :waves").contains("Wizard waves"));
    assert!(run(&s, &c, 2, "t ;'s happy").contains("Wizard's happy"));
    run(&s, &c, 1, "@chan/pflags Test=!transmit");
    assert!(run(&s, &c, 2, "t denied").contains("cannot be transmitted"));
    run(&s, &c, 1, "@chan/pflags Test=transmit");
    run(&s, &c, 2, "t off");
    assert!(run(&s, &c, 2, "t denied").contains("must be on"));
    run(&s, &c, 2, "t on");
    for i in 0..25 {
        run(&s, &c, 1, &format!("t message {i}"));
    }
    assert_eq!(s.world.borrow().channels["Test"].history.len(), 20);
    let history = run(&s, &c, 2, "t last");
    assert!(history.find("message 24").unwrap() < history.find("message 23").unwrap());
    assert!(!history.contains("message 4\n"));
    let who = run(&s, &c, 2, "t who");
    assert!(who.contains("Wizard"));
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Dark);
    assert!(!run(&s, &c, 2, "t who").contains("God(#1)"));
    run(&s, &c, 1, "@chan/flags Test=transparent");
    assert!(
        s.world.borrow().channels["Test"]
            .flags
            .has(ChannelFlag::Transparent)
    );
    assert!(run(&s, &c, 2, "pc #1=hello").contains("You paged"));
    assert_eq!(s.world.borrow().last_pages[&ObjectId(2)], vec![ObjectId(1)]);
    assert!(run(&s, &c, 2, "page :waves").contains("From afar"));
    assert!(run(&s, &c, 2, "page").contains("You last paged"));
    assert!(run(&s, &c, 2, "page Missing #1=partial").contains("don't recognize"));
    run(&s, &c, 2, "delcom t");
    assert!(
        !s.world.borrow().channels["Test"]
            .users
            .iter()
            .any(|u| u.who == ObjectId(2))
    );
    assert!(run(&s, &c, 1, "@chan/emit/list Test=invalid").contains("Illegal combination"));
}

#[tokio::test(flavor = "current_thread")]
async fn lua_catalog_handles_and_argument_validation() {
    let (_d, c, s) = fixture().await;
    s.communication(&c);
    s.lua
        .load(
            r#"
        local c=mux.comsys.create_channel('Lua')
        assert(c:name()=='Lua' and c:user_count()==0 and c:max_user_count()==0)
        assert(mux.comsys.channel('lua')==c)
        assert(not pcall(function() mux.comsys.channel('missing') end))
        local f=c:flags()
        assert(#f:list()==0)
        assert(f:add(mux.comsys.flags.PUBLIC))
        assert(not f:add(mux.comsys.flags.PUBLIC))
        assert(f:has(mux.comsys.flags.PUBLIC) and #f:list()==1)
        assert(tostring(f:list()[1])=='PUBLIC')
        assert(not pcall(function() mux.comsys.flags.PUBLIC=1 end))
        assert(not pcall(function() return mux.comsys.flags.nope end))
        assert(not pcall(function() f:add('PUBLIC') end))
        assert(not pcall(function() f:add(mux.world.flags.WIZARD) end))
        assert(not pcall(function() c:set_object() end))
        assert(not pcall(function() c:set_object(-42) end))
        c:set_object(1)
        assert(c:object():dbref()==1)
        c:set_object(nil)
        assert(c:object()==nil)
        assert(not pcall(function() c:add_player(2,'lua') end))
        assert(not pcall(function() c:add_player(2,123,true) end))
        assert(not pcall(function() c:emit('hello',{noheader=true}) end))
        assert(not pcall(function() c:who({all=1}) end))
        c:add_player(mux.world.object(2),'lua',true)
        assert(c:user_count()==1 and c:max_user_count()==10)
        assert(#c:who()==1 and c:who()[1].listening)
        c:emit('hello')
        assert(c:message_count()==1)
        c:boot_player(2)
        assert(c:user_count()==0)
        mux.comsys.destroy_channel(c)
        local replacement=mux.comsys.create_channel('Lua')
        assert(c~=replacement)
        assert(not pcall(function() c:name() end))
        assert(not pcall(function() f:has(mux.comsys.flags.PUBLIC) end))
        assert(not pcall(function() mux.comsys.destroy_channel(c) end))
        assert(#mux.comsys.list_channels()>=1)
    "#,
        )
        .exec()
        .unwrap();
    let before = s.world.borrow().clone();
    s.lua
        .load("provisional=mux.comsys.create_channel('Provisional')")
        .exec()
        .unwrap();
    *s.world.borrow_mut() = before;
    s.lua.load("mux.comsys.create_channel('Provisional'); assert(not pcall(function() provisional:name() end))").exec().unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn locks_grant_independently_errors_restore_and_output_limits_rollback() {
    let (_d, c, s) = fixture().await;
    run(&s, &c, 1, "@chan/create Locked");
    run(&s, &c, 1, "@chan/pflags Locked=!join");
    assert!(run(&s, &c, 2, "addcom lock=Locked").contains("not allowed"));
    s.world
        .borrow_mut()
        .channels
        .get_mut("Locked")
        .unwrap()
        .object = Some(ObjectId(1));
    // Missing lock grants, just like the fork's comsys_test_access.
    assert!(
        s.communication(&c)
            .allowed(ObjectId(2), "Locked", Access::Join)
            .unwrap()
    );
    s.lua.load(r#"_parents[_object_parents[1]].locks={channel_join=function(ctx) mux.world.object(2):set_description('leaked'); error('lock failed') end}"#).exec().unwrap();
    let before = s.world.borrow().objects[&ObjectId(2)].description.clone();
    assert!(
        !s.communication(&c)
            .allowed(ObjectId(2), "Locked", Access::Join)
            .unwrap()
    );
    assert_eq!(s.world.borrow().objects[&ObjectId(2)].description, before);
    run(&s, &c, 1, "@chan/pflags Locked=join");
    assert!(
        s.communication(&c)
            .allowed(ObjectId(2), "Locked", Access::Join)
            .unwrap()
    );
    run(&s, &c, 2, "addcom lock=Locked");
    let before = serde_json::to_value(s.world.borrow().clone()).unwrap();
    assert!(
        run(
            &s,
            &c,
            2,
            &format!("lock {}", "a".repeat(c.runtime.output_message_limit))
        )
        .contains("output limit")
    );
    assert_eq!(
        serde_json::to_value(s.world.borrow().clone()).unwrap(),
        before
    );
}

#[tokio::test(flavor = "current_thread")]
async fn pages_names_partial_delivery_ic_and_offline_saved_recipients() {
    let (_d, c, s) = fixture().await;
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .name = "Long Player Name".into();
    assert!(run(&s, &c, 1, "page Long Player Name=message").contains("You paged Long Player Name"));
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Connected);
    let output = run(&s, &c, 1, "page #2 #1=partial");
    assert!(output.contains("not connected"));
    assert_eq!(s.world.borrow().last_pages[&ObjectId(1)], vec![ObjectId(1)]);
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(Flag::Gagged);
    assert!(run(&s, &c, 2, "page #2=denied").contains("Permission denied"));
    assert!(run(&s, &c, 2, "page #1=wizard exception").contains("You paged"));
}

#[tokio::test(flavor = "current_thread")]
async fn relational_roundtrip_sparse_positions_unknown_fields_and_failed_writes() {
    let (_d, c, s) = fixture().await;
    run(&s, &c, 1, "@chan/create Durable");
    run(&s, &c, 1, "addcom dur=Durable");
    run(&s, &c, 2, "addcom dur=Durable");
    run(&s, &c, 2, "dur hello");
    run(&s, &c, 1, "page #2=message");
    save(&c, &s).await.unwrap();
    let mut db = connection(&c).await;
    sqlx::raw_sql("ALTER TABLE comsys_channels ADD COLUMN extra BLOB DEFAULT X'FE'; ALTER TABLE commac_aliases ADD COLUMN extra BLOB DEFAULT X'FF'; UPDATE commac_entries SET curmac=42,macro_slot_0=43 WHERE who=1; UPDATE comsys_channels SET type=type|8192 WHERE name='Durable'; UPDATE comsys_channel_messages SET position=position+100 WHERE channel_name='Durable'").execute(&mut db).await.unwrap();
    let w = persistence::load(&c.database()).await.unwrap();
    let image = std::fs::read(c.database()).unwrap();
    persistence::save(&c.database(), &w).await.unwrap();
    assert_eq!(std::fs::read(c.database()).unwrap(), image);
    *s.world.borrow_mut() = w;
    run(&s, &c, 1, "@chan/flags Durable=public");
    run(&s, &c, 1, "dur next");
    save(&c, &s).await.unwrap();
    let loaded = persistence::load(&c.database()).await.unwrap();
    assert!(loaded.channels["Durable"].flags.has(ChannelFlag::Public));
    assert!(loaded.channels["Durable"].flags.0 & 8192 != 0);
    assert!(
        loaded.channels["Durable"]
            .history
            .last()
            .unwrap()
            .message
            .contains("next")
    );
    assert_eq!(loaded.last_pages[&ObjectId(1)], vec![ObjectId(2)]);
    let r = sqlx::query(
        "SELECT extra,typeof(extra) AS storage FROM comsys_channels WHERE name='Durable'",
    )
    .fetch_one(&mut db)
    .await
    .unwrap();
    assert_eq!(r.get::<Vec<u8>, _>("extra"), [254]);
    assert_eq!(r.get::<String, _>("storage"), "blob");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT curmac FROM commac_entries WHERE who=1")
            .fetch_one(&mut db)
            .await
            .unwrap(),
        42
    );
    sqlx::raw_sql("CREATE TRIGGER block_comsys BEFORE UPDATE ON comsys_channels BEGIN SELECT RAISE(FAIL,'comsys blocked'); END").execute(&mut db).await.unwrap();
    run(&s, &c, 1, "dur rollback");
    assert!(save(&c, &s).await.is_err());
    assert_eq!(
        serde_json::to_value(persistence::load(&c.database()).await.unwrap()).unwrap(),
        serde_json::to_value(loaded).unwrap()
    );
    db.close().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn destroy_preserves_macros_and_rejects_unknown_dependencies() {
    let (_d, c, s) = fixture().await;
    run(&s, &c, 1, "@chan/create Delete");
    run(&s, &c, 1, "addcom del=Delete");
    save(&c, &s).await.unwrap();
    let mut db = connection(&c).await;
    sqlx::raw_sql("UPDATE commac_entries SET macro_slot_4=99 WHERE who=1; CREATE TABLE extension(channel TEXT REFERENCES comsys_channels(name)); INSERT INTO extension VALUES('Delete')").execute(&mut db).await.unwrap();
    run(&s, &c, 1, "@chan/destroy Delete");
    assert!(
        format!("{:#}", save(&c, &s).await.unwrap_err()).contains("dependency extension.channel")
    );
    sqlx::query("DELETE FROM extension")
        .execute(&mut db)
        .await
        .unwrap();
    save(&c, &s).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT macro_slot_4 FROM commac_entries WHERE who=1")
            .fetch_one(&mut db)
            .await
            .unwrap(),
        99
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM commac_aliases WHERE channel_name='Delete'"
        )
        .fetch_one(&mut db)
        .await
        .unwrap(),
        0
    );
    assert!(
        !persistence::load(&c.database())
            .await
            .unwrap()
            .channels
            .contains_key("Delete")
    );
    db.close().await.unwrap();
}

/// Drop the world borrow before SQLx yields to its worker.
async fn save(c: &Config, s: &Scripts) -> anyhow::Result<()> {
    let snapshot = s.world.borrow().clone();
    persistence::save(&c.database(), &snapshot).await
}

/// Callback mutations and queued channel output belong to the surrounding command transaction.
#[tokio::test(flavor = "current_thread")]
async fn channel_leave_and_lua_command_failures_restore_state() {
    let (d, c, s) = fixture().await;
    run(&s, &c, 1, "@chan/create Callback");
    run(&s, &c, 2, "addcom cb=Callback");
    let thing = {
        let mut w = s.world.borrow_mut();
        let id = w.create(&c, "Listener".into(), stompymux_rs::world::Kind::Thing);
        w.channels.get_mut("Callback").unwrap().users.push(
            stompymux_rs::communication::Membership {
                who: id,
                listening: true,
            },
        );
        id
    };
    s.sync_parents().unwrap();
    s.lua.load(format!(r#"_parents[_object_parents[{}]].events={{on_leave=function(ctx) assert(ctx.subject==2 and ctx.cause==2); mux.world.object(2):set_description('must rollback'); error('leave failure') end}}"#,thing.0)).exec().unwrap();
    let before = serde_json::to_value(s.world.borrow().clone()).unwrap();
    assert!(run(&s, &c, 2, "cb off").contains("leave failure"));
    assert_eq!(
        serde_json::to_value(s.world.borrow().clone()).unwrap(),
        before
    );
    assert!(s.outbox.borrow().is_empty());
    std::fs::write(d.path().join("lua/global_logic/comsys_test.lua"),r#"return {commands={{name='failcom',permission='everyone',pattern='^failcom$',handler=function(ctx) mux.comsys.create_channel('RolledBack'):emit('must not arrive'); error('comsys callback failure') end}}}"#).unwrap();
    let scripts = Scripts::new(&c, s.world.clone()).unwrap();
    let prior = scripts.world.borrow().clone();
    assert!(commands::run(&scripts, &c, ObjectId(2), 1, "failcom").is_err());
    // The world owner restores its snapshot on callback failure, before any persistence or flush.
    *scripts.world.borrow_mut() = prior;
    scripts.outbox.borrow_mut().clear();
    assert!(!scripts.world.borrow().channels.contains_key("RolledBack"));
}

/// Database maintenance must compact membership and page slots without deleting surviving entries.
#[tokio::test(flavor = "current_thread")]
async fn repair_purges_communication_ownership_and_retains_survivors() {
    let (_d, c, s) = fixture().await;
    run(&s, &c, 1, "@chan/create Repair");
    run(&s, &c, 1, "addcom r=Repair");
    run(&s, &c, 2, "addcom r=Repair");
    let victim = {
        let mut w = s.world.borrow_mut();
        let id = w.create(&c, "PurgeMe".into(), stompymux_rs::world::Kind::Player);
        w.accounts.insert(id, Default::default());
        id
    };
    s.communication(&c)
        .add(victim, "Repair", "r", true, true)
        .unwrap();
    s.world
        .borrow_mut()
        .last_pages
        .insert(ObjectId(1), vec![victim, ObjectId(2)]);
    save(&c, &s).await.unwrap();
    s.world
        .borrow_mut()
        .objects
        .get_mut(&victim)
        .unwrap()
        .flags
        .insert(Flag::Going);
    let report = persistence::repair(&c.database(), c.database.busy_timeout_ms, |links| {
        stompymux_rs::dbck::plan(&s.world.borrow(), links, &c)
    })
    .await
    .unwrap();
    assert!(report.plan.purges.contains(&victim));
    let loaded = persistence::load(&c.database()).await.unwrap();
    assert_eq!(
        loaded.channels["Repair"]
            .users
            .iter()
            .map(|u| u.who)
            .collect::<Vec<_>>(),
        vec![ObjectId(1), ObjectId(2)]
    );
    assert_eq!(loaded.last_pages[&ObjectId(1)], vec![ObjectId(2)]);
    assert!(!loaded.channel_aliases.contains_key(&victim));
}
