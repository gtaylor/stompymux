//! C-compatible state syntax, immutable Lua handles, atomic quotas and relational round trips.
use sqlx::{Connection, Row};
use std::{cell::RefCell, path::Path, rc::Rc};
use stompymux_rs::{
    commands::{self, Action},
    config::Config,
    lua::Scripts,
    persistence,
    state::{self, Value},
    world::{Kind, ObjectId},
};

/// Copy the game before modifying either modules or relational fixtures.
fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let target = to.join(e.file_name());
        if e.path().is_dir() {
            copy(&e.path(), &target)
        } else {
            std::fs::copy(e.path(), target).unwrap();
        }
    }
}
/// Shared isolated populated fixture with the copied, unchanged access-policy modules.
async fn fixture() -> (tempfile::TempDir, Config, Scripts) {
    let d = tempfile::tempdir().unwrap();
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"),
        d.path(),
    );
    let c = Config::load(d.path()).unwrap();
    let w = persistence::load(&c.database()).await.unwrap();
    let s = Scripts::new(&c, Rc::new(RefCell::new(w))).unwrap();
    (d, c, s)
}
/// Collect both private inspection replies and transaction-staged mutation acknowledgements.
fn command(s: &Scripts, c: &Config, p: i64, line: &str) -> String {
    let action = commands::run(s, c, ObjectId(p), 1, line).unwrap();
    let mut text = if let Action::Reply(t) | Action::Report(t) = action {
        t
    } else {
        String::new()
    };
    for (_, out) in s.outbox.borrow_mut().drain(..) {
        text.push_str(out.source());
    }
    text
}

#[test]
fn grammar_scalar_types_and_c_byte_quotas() {
    for name in ["a", "A.b/c-d_e"] {
        assert!(state::valid_name(name, 127));
    }
    for name in ["", "1a", "a b", "é", "a\0"] {
        assert!(!state::valid_name(name, 127));
    }
    assert!(state::valid_name(&"a".repeat(127), 127));
    assert!(!state::valid_name(&"a".repeat(128), 127));
    for (input, expected) in [
        ("true", Value::Boolean(true)),
        ("42", Value::Integer(42)),
        ("1.25", Value::Number(1.25)),
        ("0x1.8p+2", Value::Number(6.0)),
        ("1e-9999", Value::String(b"1e-9999".to_vec())),
        ("NaN", Value::String(b"NaN".to_vec())),
        (
            "\"\\x00\\xFF\\n\\r\\t\\\\\\\"\"",
            Value::String(vec![0, 255, 10, 13, 9, 92, 34]),
        ),
    ] {
        assert_eq!(state::commands::parse_value(input).unwrap(), Some(expected));
    }
    for input in ["\"open", "\"\\x0\"", "\"\\q\""] {
        assert!(state::commands::parse_value(input).is_err());
    }
    assert_eq!(Value::Number(0.1).display(), "0.10000000000000001");
    assert_eq!(Value::Number(1e20).display(), "1e+20");
    assert_eq!(state::commands::parse_value("").unwrap(), None);
    assert_eq!(
        state::commands::parse_value("\"\"").unwrap(),
        Some(Value::String(vec![]))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn commands_cover_inspection_types_copy_move_wipe_permissions_and_aliases() {
    let (_d, c, s) = fixture().await;
    let path = c.root.join("stompymux.toml");
    let doc =
        std::fs::read_to_string(&path).unwrap() + "\n[aliases.commands]\n\"@st\"=\"@state\"\n";
    std::fs::write(path, doc).unwrap();
    let c = Config::load(&c.root).unwrap();
    assert!(command(&s, &c, 1, "@state").contains("/copy"));
    assert!(
        command(&s, &c, 1, "@st/set #2/test value=\"\\x00\\xFF[red]\"")
            .contains("State value set.")
    );
    assert!(
        command(&s, &c, 1, "@state/examine #2/test")
            .contains("value (string): \"\\x00\\xFF[red]\"")
    );
    assert!(command(&s, &c, 1, "@state/copy #2/test value=other copy").contains("copied"));
    assert!(command(&s, &c, 1, "@state/move #2/other copy=other copy").contains("moved"));
    assert!(command(&s, &c, 1, "@state/move #2/test value=other moved").contains("moved"));
    assert!(command(&s, &c, 1, "@examine #2").contains("other: 2 values"));
    assert!(command(&s, &c, 1, "@state/wipe #2/other").contains("2 state values wiped."));
    assert!(command(&s, &c, 1, "@state/set #2/test k=true").contains("set"));
    assert!(command(&s, &c, 1, "@state/set #2/test k=").contains("cleared"));
    assert!(
        !s.world.borrow().objects[&ObjectId(2)]
            .state
            .contains_key("test")
    );
    assert!(command(&s, &c, 1, "@state/copy #2/test missing=other new").contains("not found"));
    assert!(command(&s, &c, 1, "@state/set #2/test 0bad=true").contains("invalid state key"));
    assert!(command(&s, &c, 1, "@state/wrong").contains("Invalid @state"));
    // Wizards can modify GOD's state, unlike flag control.
    assert!(command(&s, &c, 2, "@state/set #1/test k=1").contains("State value set"));
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(stompymux_rs::flags::Flag::Wizard);
    assert_eq!(
        command(&s, &c, 2, "@state/set #1/test k=2"),
        "Permission denied."
    );
}

#[tokio::test(flavor = "current_thread")]
async fn lua_complete_api_binary_defaults_atomic_batches_and_incarnations() {
    let (_d, c, s) = fixture().await;
    s.lua.load("handle=mux.world.object(1):state('test'); assert(handle:get('absent')==nil); assert(not pcall(function() handle:set('a',1) end)); assert(not pcall(function() handle:keys() end))").exec().unwrap();
    s.eval_callback::<()>(r#"
      local t=handle
      assert(tostring(t)=='state(#1, test)')
      local default={};assert(rawequal(t:get('absent',default),default))
      t:set_many({z=3,a=false,bytes=string.char(0,255),decimal=1.25})
      assert(t:has('a') and t:get('a',true)==false)
      assert(t:get('bytes')==string.char(0,255))
      assert(table.concat(t:keys(),',')=='a,bytes,decimal,z')
      assert(t:entries()[1].value==false)
      local some=t:get_many({'z','absent','a'});assert(some.z==3 and some.a==false and some.absent==nil)
      assert(not pcall(function() t:set_many({ok=2,bad={}}) end));assert(not t:has('ok'))
      for _,v in ipairs({{},function()end,0/0,1/0}) do assert(not pcall(function() t:set('invalid',v) end)) end
      assert(not pcall(function() t.fake=1 end))
      assert(t:delete('z'));assert(not t:delete('z'))
      t:set('a',nil);assert(not t:has('a'))
      assert(not pcall(function() t:get('1invalid') end))
    "#).unwrap();
    let before = s.world.borrow().clone();
    assert!(
        s.eval_callback::<()>(
            "handle:set('rolled_back',true); mux.world.pemit(1,'must disappear'); error('stop')"
        )
        .is_err()
    );
    assert_eq!(
        before.objects[&ObjectId(1)].state,
        s.world.borrow().objects[&ObjectId(1)].state
    );
    assert!(s.outbox.borrow().is_empty());
    let id = s.world.borrow().next_id;
    assert!(s.eval_callback::<()>("local o=mux.world.create_object{type=mux.world.types.THING,name='provisional'}; stale=o:state('test'); error('rollback')").is_err());
    assert_eq!(
        s.world
            .borrow_mut()
            .create(&c, "replacement".into(), Kind::Thing)
            .0,
        id
    );
    assert!(s.eval_callback::<()>("stale:get('a')").is_err());
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .kind = Kind::Garbage;
    assert!(
        s.lua
            .load("return tostring(handle)")
            .eval::<String>()
            .is_err()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn final_batch_quota_is_atomic_even_when_lua_catches_the_error() {
    let (_d, c, s) = fixture().await;
    let path = c.root.join("stompymux.toml");
    let mut doc: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    for (key, value) in [
        ("state_entry_limit", 2),
        ("state_value_limit", 8),
        ("state_object_limit", 16),
    ] {
        doc["lua"][key] = toml::Value::Integer(value);
    }
    std::fs::write(path, toml::to_string(&doc).unwrap()).unwrap();
    let c = Config::load(&c.root).unwrap();
    let s = Scripts::new(&c, s.world.clone()).unwrap();
    s.eval_callback::<()>(
        r#"
       local s=mux.world.object(1):state('n')
       s:set_many({a='123456',b='12'}) -- 6+2 payload + two four-byte addresses =16
       assert(not pcall(function() s:set_many({a='12345678',b='123'}) end))
       assert(s:get('a')=='123456' and s:get('b')=='12')
       s:set_many({a='1',b='1234567'}) -- final accounting fits regardless of iteration order
       assert(not pcall(function() s:set('c',false) end))
       assert(not s:has('c'))
       assert(not pcall(function() s:set('a','123456789') end))
    "#,
    )
    .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn binary_storage_preserves_types_unchanged_storage_classes_and_unknown_columns() {
    let (_d, c, s) = fixture().await;
    let mut db = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(c.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    sqlx::raw_sql("ALTER TABLE object_state ADD COLUMN extension BLOB DEFAULT X'FF'; INSERT INTO object_state(object_dbref,namespace,key,value_type,value) VALUES(1,'test','text',1,'original'),(1,'test','rawtext',1,CAST(X'00FF' AS TEXT)),(1,'test','blob',1,X'00FF'),(1,'test','large',3,9223372036854775807),(1,'test','float',4,7.0)").execute(&mut db).await.unwrap();
    *s.world.borrow_mut() = persistence::load(&c.database()).await.unwrap();
    s.eval_callback::<()>("local s=mux.world.object(1):state('test');assert(s:get('blob')==string.char(0,255));s:set('added',string.char(255,0,128))").unwrap();
    let world = s.world.borrow().clone();
    persistence::save(&c.database(), &world).await.unwrap();
    let rows=sqlx::query("SELECT key,typeof(value) AS storage,hex(extension) AS extension FROM object_state WHERE namespace='test' ORDER BY key").fetch_all(&mut db).await.unwrap();
    assert_eq!(
        rows.iter()
            .map(|r| (r.get::<String, _>("key"), r.get::<String, _>("storage")))
            .collect::<Vec<_>>(),
        vec![
            ("added".into(), "blob".into()),
            ("blob".into(), "blob".into()),
            ("float".into(), "real".into()),
            ("large".into(), "integer".into()),
            ("rawtext".into(), "text".into()),
            ("text".into(), "text".into())
        ]
    );
    assert!(rows.iter().all(|r| r.get::<String, _>("extension") == "FF"));
    let reloaded = persistence::load(&c.database()).await.unwrap();
    assert_eq!(
        world.objects[&ObjectId(1)].state,
        reloaded.objects[&ObjectId(1)].state
    );
    assert_eq!(
        reloaded.objects[&ObjectId(1)].state["test"]["large"],
        Value::Integer(i64::MAX)
    );
    sqlx::raw_sql("CREATE TRIGGER fail_state BEFORE UPDATE ON object_state BEGIN SELECT RAISE(ABORT,'state write blocked'); END").execute(&mut db).await.unwrap();
    command(&s, &c, 1, "@state/set #1/test text=changed");
    let changed = s.world.borrow().clone();
    assert!(persistence::save(&c.database(), &changed).await.is_err());
    assert_eq!(
        persistence::load(&c.database()).await.unwrap().objects[&ObjectId(1)].state,
        reloaded.objects[&ObjectId(1)].state
    );
    db.close().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn unchanged_default_exit_policy_handles_all_predicates_and_fails_closed() {
    let (_d, c, s) = fixture().await;
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("game/lua");
    for file in [
        "object_logic/default_exit.lua",
        "packages/access_policy.lua",
    ] {
        assert_eq!(
            std::fs::read(source.join(file)).unwrap(),
            std::fs::read(c.lua_dir().join(file)).unwrap()
        );
    }
    assert!(s.lock(ObjectId(2), ObjectId(13)).unwrap());
    command(
        &s,
        &c,
        1,
        "@state/set #13/locks.traverse state/access/key=42",
    );
    assert!(!s.lock(ObjectId(2), ObjectId(13)).unwrap());
    command(&s, &c, 1, "@state/set #2/access key=\"42\"");
    assert!(!s.lock(ObjectId(2), ObjectId(13)).unwrap());
    command(&s, &c, 1, "@state/set #2/access key=42");
    assert!(s.lock(ObjectId(2), ObjectId(13)).unwrap());
    command(&s, &c, 1, "@state/set #13/locks.traverse flag/DARK=false");
    assert!(s.lock(ObjectId(2), ObjectId(13)).unwrap());
    command(&s, &c, 1, "@state/set #13/locks.traverse affiliation=1");
    assert!(!s.lock(ObjectId(2), ObjectId(13)).unwrap());
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .affiliation = Some(ObjectId(1));
    assert!(s.lock(ObjectId(2), ObjectId(13)).unwrap());
    command(
        &s,
        &c,
        1,
        "@state/set #13/locks.traverse affiliation=999999",
    );
    assert!(s.lock(ObjectId(2), ObjectId(13)).is_err());
    command(&s, &c, 1, "@state/set #13/locks.traverse affiliation=");
    for (key, value) in [
        ("flag/MISSING", "true"),
        ("flag/WIZARD", "1"),
        ("unknown", "true"),
        ("message/enactor", "true"),
    ] {
        command(
            &s,
            &c,
            1,
            &format!("@state/set #13/locks.traverse {key}={value}"),
        );
        assert!(s.lock(ObjectId(2), ObjectId(13)).is_err());
        command(&s, &c, 1, &format!("@state/set #13/locks.traverse {key}="));
    }
    command(&s, &c, 1, "@state/wipe #13/locks.traverse");
    command(
        &s,
        &c,
        1,
        "@state/set #13/locks.traverse message/enactor=message only",
    );
    assert!(s.lock(ObjectId(2), ObjectId(13)).unwrap());
}

#[tokio::test(flavor = "current_thread")]
async fn malformed_lock_returns_rollback_and_denial_hooks_receive_context() {
    let (_d, _c, s) = fixture().await;
    s.lua
        .load(
            r#"
        local parent=_parents['default_exit.lua']
        parent.locks.traverse=function(ctx)
            mux.world.object(ctx.object):state('test'):set('bad',true)
            mux.world.pemit(ctx.enactor,'LEAK')
            return {passes=false,unknown=true}
        end
    "#,
        )
        .exec()
        .unwrap();
    assert!(s.lock(ObjectId(2), ObjectId(13)).is_err());
    assert!(
        !s.world.borrow().objects[&ObjectId(13)]
            .state
            .contains_key("test")
    );
    assert!(s.outbox.borrow().is_empty());
    s.lua
        .load(
            r#"
        local parent=_parents['default_exit.lua']
        parent.locks.traverse=function(ctx)
            assert(ctx.lock=='traverse' and ctx.descriptor==7 and ctx.subject==2)
            return {passes=false,enactor_message='Denied specifically.',other_message='is denied.'}
        end
        parent.events={on_fail=function(ctx)
            assert(ctx.object==13 and ctx.enactor==2 and ctx.cause==2 and ctx.descriptor==7)
            mux.world.object(13):state('test'):set('failed',true)
        end}
    "#,
        )
        .exec()
        .unwrap();
    assert!(!s.traversal(ObjectId(2), ObjectId(13), 7).unwrap());
    assert_eq!(
        s.world.borrow().objects[&ObjectId(13)].state["test"]["failed"],
        Value::Boolean(true)
    );
    assert!(
        s.outbox
            .borrow()
            .iter()
            .any(|(_, v)| v.source() == "Denied specifically.")
    );
    s.outbox.borrow_mut().clear();
    s.lua.load("_parents['default_exit.lua'].locks.traverse=function() return {passes=false,enactor_message='',other_message=''} end").exec().unwrap();
    assert!(!s.traversal(ObjectId(2), ObjectId(13), 7).unwrap());
    assert!(s.outbox.borrow().is_empty());
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(stompymux_rs::flags::Flag::Dark);
    assert!(!s.traversal(ObjectId(2), ObjectId(13), 7).unwrap());
    assert!(s.outbox.borrow().is_empty());
    // A failed failure-hook rolls back its own state mutations too.
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(stompymux_rs::flags::Flag::Dark);
    s.lua.load("_parents['default_exit.lua'].events.on_fail=function(ctx) mux.world.object(13):state('test'):set('leak',true); error('failed hook') end").exec().unwrap();
    assert!(s.traversal(ObjectId(2), ObjectId(13), 7).is_err());
    assert!(!s.world.borrow().objects[&ObjectId(13)].state["test"].contains_key("leak"));
}
