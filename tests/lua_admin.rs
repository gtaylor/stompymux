//! Typed Lua objects, isolated validation and snapshot-based module administration.
use std::{cell::RefCell, path::Path, rc::Rc};
use stompymux_rs::{
    commands::{self, Action},
    config::Config,
    flags::Flag,
    lua::Scripts,
    persistence,
    world::{Kind, ObjectId, World},
};

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        if e.path().is_dir() {
            copy(&e.path(), &to.join(e.file_name()));
        } else {
            std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}
async fn fixture() -> (tempfile::TempDir, Config, World) {
    let d = tempfile::tempdir().unwrap();
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"),
        d.path(),
    );
    let c = Config::load(d.path()).unwrap();
    let mut w = persistence::load(&c.database()).await.unwrap();
    for id in [ObjectId(1), ObjectId(2)] {
        w.objects.get_mut(&id).unwrap().location = Some(ObjectId(c.start()));
    }
    w.objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    (d, c, w)
}
fn object(w: &mut World, c: &Config, name: &str, kind: Kind, loc: ObjectId) -> ObjectId {
    let id = w.create(c, name.into(), kind);
    let o = w.objects.get_mut(&id).unwrap();
    if kind != Kind::Room {
        o.location = Some(loc);
    }
    if kind == Kind::Thing {
        o.home = Some(ObjectId(c.home()));
    }
    id
}
fn scripts(c: &Config, w: World) -> Scripts {
    Scripts::new(c, Rc::new(RefCell::new(w))).unwrap()
}
fn run(s: &Scripts, c: &Config, who: i64, line: &str) -> String {
    let action = commands::run(s, c, ObjectId(who), 71, line).unwrap();
    let mut messages = s
        .outbox
        .borrow_mut()
        .drain(..)
        .map(|(_, m)| m.source().to_string())
        .collect::<Vec<_>>();
    if let Action::Reply(t) | Action::CommitReply(t) | Action::Report(t) = action {
        messages.push(t);
    }
    messages.join("\n")
}
fn lua(s: &Scripts, source: &str) {
    s.eval_callback::<()>(source).unwrap();
}
async fn save(s: &Scripts, c: &Config) -> anyhow::Result<()> {
    let w = s.world.borrow().clone();
    persistence::save(&c.database(), &w).await
}

#[tokio::test(flavor = "current_thread")]
async fn typed_objects_relationships_and_enumeration() {
    let (_d, c, mut w) = fixture().await;
    let room = ObjectId(c.start());
    let bag = object(&mut w, &c, "Bag", Kind::Thing, room);
    let child = object(&mut w, &c, "Child", Kind::Thing, bag);
    let exit = object(&mut w, &c, "Exit", Kind::Exit, room);
    let s = scripts(&c, w);
    lua(
        &s,
        &format!(
            r#"
      local world=mux.world; local types=world.types
      local room=world.object({room}); local bag=world.object({bag}); local child=world.object({child}); local exit=world.object({exit})
      assert(bag:type()==types.THING and tostring(bag:type())=='THING')
      assert(not pcall(function() types.THING=3 end))
      assert(not pcall(function() return types.GARBAGE end))
      assert(not pcall(function() return types.thing end))
      assert(not pcall(world.object, '1'))
      assert(not pcall(world.create_object, {{type=1,name='invalid'}}))
      assert(not pcall(world.list_objects, {{types={{1}}}}))
      assert(not pcall(world.list_objects, {{types={{[2]=types.THING}}}}))
      assert(not pcall(world.list_objects, {{bad=true}}))
      assert(#world.list_objects({{types={{}}}})==0)
      assert(#bag:contents({{types={{}}}})==0)
      assert(bag:contents()[1]==child)
      assert(not pcall(function() bag:contents({{other=true}}) end))
      assert(bag:location()==room)
      exit:set_destination(bag); assert(exit:destination()==bag)
      exit:set_destination(nil); assert(exit:destination()==nil)
      assert(not pcall(function() exit:set_destination() end))
      assert(not pcall(function() bag:destination() end))
      assert(not pcall(function() room:location() end))
      bag:set_home(room); assert(bag:home()==room)
      for _,value in ipairs({{bag,child,exit}}) do assert(not pcall(function() bag:set_home(value) end)) end
      assert(not pcall(function() bag:set_home(nil) end))
      assert(bag:home()==room)
      bag:flags():add(world.flags.WIZARD);bag:powers():add(world.powers.IDLE)
      bag:set_zone(room);assert(bag:zone()==room)
      assert(bag:flags():has(world.flags.WIZARD) and bag:powers():has(world.powers.IDLE))
      assert(bag:powers():list()[1]==world.powers.IDLE)
      local has=false;for _,flag in ipairs(bag:flags():list()) do if flag==world.flags.WIZARD then has=true end end;assert(has)
      local found=world.list_objects({{types={{types.THING}},in_zone=room}}); assert(#found==1 and found[1]==bag)
      local last=-1;for _,o in ipairs(world.list_objects()) do assert(o:dbref()>last);last=o:dbref() end
      bag:set_affiliation(exit);assert(bag:affiliation()==exit)
      bag:set_affiliation(nil);bag:set_zone(nil);assert(bag:zone()==nil)
      bag:set_lua_parent(nil);assert(bag:lua_parent()==nil)
      assert(not pcall(function() bag:set_lua_parent('') end))
      assert(not pcall(function() bag:set_lua_parent() end))
      assert(not pcall(function() bag:set_lua_parent('../default_exit.lua') end))
      bag:set_lua_parent('default_exit.lua');assert(bag:lua_parent()=='default_exit.lua')
    "#,
            room = room.0,
            bag = bag.0,
            child = child.0,
            exit = exit.0
        ),
    );
    save(&s, &c).await.unwrap();
    let loaded = persistence::load(&c.database()).await.unwrap();
    assert_eq!(loaded.objects[&bag].lua_parent, "default_exit.lua");
    assert!(
        loaded.objects[&bag]
            .powers
            .contains(stompymux_rs::powers::Power::Idle)
    );
    s.world
        .borrow_mut()
        .objects
        .get_mut(&child)
        .unwrap()
        .flags
        .insert(Flag::Going);
    lua(
        &s,
        &format!(
            "local c=mux.world.object({});assert(not pcall(function() c:set_zone(0) end)); local found=false;for _,o in ipairs(mux.world.list_objects()) do if o==c then found=true end end;assert(found)",
            child.0
        ),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn parent_commands_use_active_catalog_and_preserve_c_authority() {
    let (d, c, w) = fixture().await;
    let s = scripts(&c, w);
    assert!(run(&s, &c, 2, "@lua/parent #1=default_exit.lua").contains("Permission denied"));
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(Flag::Wizard);
    assert!(run(&s, &c, 2, "@lua/parent #1=default_exit.lua").contains("parent set"));
    assert_eq!(
        s.world.borrow().objects[&ObjectId(1)].lua_parent,
        "default_exit.lua"
    );
    assert!(run(&s, &c, 2, "@lua/parent #1").contains("parent cleared"));
    std::fs::write(d.path().join("lua/object_logic/new.lua"), "return {}").unwrap();
    assert!(run(&s, &c, 1, "@lua/parent #1=new.lua").contains("not loaded"));
    for path in [
        "../new.lua",
        "/tmp/new.lua",
        "object_logic/new.lua",
        "packages/new.lua",
        "a/./new.lua",
        "a\\new.lua",
    ] {
        assert!(run(&s, &c, 1, &format!("@lua/parent #1={path}")).contains("relative"));
    }
    for command in [
        "@lua/check argument",
        "@lua/reload argument",
        "@lua/check/reload",
        "@lua/test/reload",
    ] {
        assert!(matches!(
            commands::run(&s, &c, ObjectId(1), 1, command).unwrap(),
            Action::Reply(_)
        ));
    }
    assert!(matches!(
        commands::run(&s, &c, ObjectId(1), 1, "@lua/viewparent new.lua").unwrap(),
        Action::LuaAdmin(stompymux_rs::lua::AdminRequest::View { .. })
    ));
    save(&s, &c).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn snapshots_checking_limits_and_mutation_isolation() {
    use stompymux_rs::lua::{RuntimeMode, sources::Sources};
    let (d, c, w) = fixture().await;
    let active = scripts(&c, w);
    let source_path = d.path().join("lua/global_logic/new.lua");
    std::fs::write(
        &source_path,
        "mux.world.object(1):state('reload'):set('value',7); return {}",
    )
    .unwrap();
    let sources = std::sync::Arc::new(Sources::read(&c).unwrap());
    let build = |mode| {
        Scripts::from_sources(
            &c,
            std::rc::Rc::new(std::cell::RefCell::new(active.world.borrow().clone())),
            active.help.clone(),
            sources.clone(),
            mode,
        )
    };
    let check = build(RuntimeMode::Checking).err().unwrap().to_string();
    assert!(check.to_string().contains("global_logic/new.lua"));
    let candidate = build(RuntimeMode::Live).unwrap();
    assert!(
        candidate.world.borrow().objects[&ObjectId(1)]
            .state
            .contains_key("reload")
    );
    assert!(
        !active.world.borrow().objects[&ObjectId(1)]
            .state
            .contains_key("reload")
    );
    std::fs::write(&source_path, "error('changed on disk')").unwrap();
    assert!(build(RuntimeMode::Live).is_ok()); // captured source is immutable
    let snapshot = Sources::read(&c).unwrap();
    assert!(
        Scripts::from_sources(
            &c,
            active.world.clone(),
            active.help.clone(),
            std::sync::Arc::new(snapshot),
            RuntimeMode::Checking
        )
        .is_err()
    );
    std::fs::write(&source_path, "return {events={on_connect=3}}").unwrap();
    assert!(Scripts::new(&c, active.world.clone()).is_err());
    std::fs::remove_file(&source_path).unwrap();
    std::fs::write(d.path().join("lua/packages/invalid.lua"), "local =").unwrap();
    assert!(
        Scripts::new(&c, active.world.clone())
            .err()
            .unwrap()
            .to_string()
            .contains("invalid.lua")
    );
    std::fs::remove_file(d.path().join("lua/packages/invalid.lua")).unwrap();
    #[cfg(unix)]
    {
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("escape.lua"), "return {}").unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("escape.lua"),
            d.path().join("lua/object_logic/escape.lua"),
        )
        .unwrap();
        assert!(Sources::read(&c).is_err());
        assert!(Sources::view(&c, "escape.lua").is_err());
    }
}

#[tokio::test(flavor = "current_thread")]
async fn caught_invalid_creations_and_callback_errors_leave_no_partial_changes() {
    let (_d, c, w) = fixture().await;
    let s = scripts(&c, w);
    let next = s.world.borrow().next_id;
    lua(
        &s,
        "local w=mux.world;assert(not pcall(w.create_object,{type=w.types.THING,name='bad',location=999999})); assert(not pcall(w.create_object,{type=w.types.ROOM,name='bad',bogus=true}))",
    );
    assert_eq!(s.world.borrow().next_id, next);
    assert!(s.eval_callback::<()>("mux.world.object(1):set_zone(0); mux.world.object(1):set_lua_parent(nil); error('abort')").is_err());
    assert!(!s.world.borrow().objects[&ObjectId(1)].lua_parent.is_empty());
}

async fn testing_vm(source: &str) -> (tempfile::TempDir, Config, Scripts) {
    use stompymux_rs::{LuaSources, RuntimeMode};
    let (d, c, w) = fixture().await;
    persistence::save(&c.database(), &w).await.unwrap();
    std::fs::create_dir_all(d.path().join("lua/tests/unit")).unwrap();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("game/lua/packages/testing.lua"),
        d.path().join("lua/packages/testing.lua"),
    )
    .unwrap();
    std::fs::write(d.path().join("lua/tests/unit/probe.lua"), source).unwrap();
    let sources = LuaSources::read(&c).unwrap().with_tests(&c).unwrap();
    let s = Scripts::from_sources(
        &c,
        Rc::new(RefCell::new(w)),
        stompymux_rs::help::HelpIndex::load(&c).unwrap(),
        std::sync::Arc::new(sources),
        RuntimeMode::Testing,
    )
    .unwrap();
    (d, c, s)
}

#[tokio::test(flavor = "current_thread")]
async fn test_runner_retains_failed_mutations_and_runs_teardown() {
    use stompymux_rs::lua::testing::{self, Request};
    let (_d, c, s) = testing_vm(
        r#"
local t=require('testing')
local state=mux.world.object(1):state('runner')
state:set('loaded',true)
return t.suite('live',{
 before_all=function(ctx) ctx.count=0;state:set('before',true) end,
 before_each=function(ctx) ctx.count=ctx.count+1 end,
 after_each=function(ctx) state:set('teardown',ctx.count) end,
 after_all=function(ctx) state:set('done',true) end,
 tests={
 t.test('assertion',function(ctx,e) state:set('assertion',true);e.equal(1,2) end),
 t.test('runtime',function(ctx,e) state:set('runtime',true);error('ordinary error') end),
 t.test('helpers',function(ctx,e)
 e.truthy(mux.world.lock_passes({object=13,enactor=1,lock=mux.world.locks.TRAVERSE}))
 e.equal(ctx.count,3);e.not_equal(1,2);e.truthy(true);e.falsy(false);e.is_nil(nil)
 e.contains('hello','ell');e.contains({1,2},2);e.near(1,1.001,.01)
 e.error_matches(function()error('needle')end,'needle');e.no_error(function()end)
 local codes=t.error.codes
 e.equal(tostring(codes.assertion),'testing.assertion')
 e.falsy(pcall(function()codes.assertion=3 end));e.falsy(pcall(function()return codes.unknown end))
 e.raises_code(function()e.equal(1,2)end,codes.assertion)
 e.is_error(mux.error.wrap('cause',codes.runtime,'wrapped'),'testing')
 e.raises(function()error('plain')end)
 end),
 }})
"#,
    )
    .await;
    let request = Request::parse("test/verbose", "").unwrap();
    let report = testing::run(&s, &c, &request, || {}).await.unwrap();
    assert_eq!(
        (report.passed, report.failed, report.errored, report.skipped),
        (1, 1, 1, 0),
        "{}",
        report.render(true)
    );
    assert!(report.render(true).contains("expected: 2"));
    assert!(report.render(true).contains("stack traceback:"));
    assert!(report.render(true).contains("unit/probe.lua:helpers"));
    let loaded = persistence::load(&c.database()).await.unwrap();
    let restarted = scripts(&c, loaded);
    lua(
        &restarted,
        "local s=mux.world.object(1):state('runner');assert(s:get('loaded'));assert(s:get('assertion'));assert(s:get('runtime'));assert(s:get('done'));assert(s:get('teardown')==3)",
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_runner_filter_hooks_checks_and_limits() {
    use stompymux_rs::lua::testing::{self, Request};
    let (d, c, s) = testing_vm(
        r#"
local t=require('testing'); return t.suite('hooks',{
 before_all=function(ctx) error('setup failed') end,
 after_all=function(ctx) mux.world.object(1):state('runner'):set('cleaned',true) end,
 tests={t.test('Alpha',function()error('must not run')end), t.test('Beta',function()end)}})
"#,
    )
    .await;
    testing::check(&s).unwrap();
    let report = testing::run(
        &s,
        &c,
        &Request::parse("test/unit", "Alpha").unwrap(),
        || {},
    )
    .await
    .unwrap();
    assert_eq!((report.errored, report.skipped), (1, 1));
    lua(
        &s,
        "assert(mux.world.object(1):state('runner'):get('cleaned'))",
    );
    let bytes = std::fs::read(c.database()).unwrap();
    let report = testing::run(
        &s,
        &c,
        &Request::parse("test/unit", "alpha").unwrap(),
        || {},
    )
    .await
    .unwrap();
    assert_eq!(report.skipped, 2);
    assert_eq!(std::fs::read(c.database()).unwrap(), bytes);
    assert!(Request::parse("test/reload", "").is_err());
    let report = testing::run(
        &s,
        &c,
        &Request::parse("test/integration", "").unwrap(),
        || {},
    )
    .await
    .unwrap();
    assert_eq!((report.passed, report.skipped, report.errored), (0, 0, 0));
    drop(d);
    let (_d,c,s)=testing_vm("local t=require('testing');return t.suite('limit',{tests={t.test('loop',function()while true do end end),t.test('later',function()end)}})").await;
    let limited = Scripts::from_sources(
        &c,
        s.world.clone(),
        s.help.clone(),
        s.sources.clone(),
        stompymux_rs::RuntimeMode::Testing,
    )
    .unwrap();
    let report = testing::run(&limited, &c, &Request::parse("test", "").unwrap(), || {})
        .await
        .unwrap();
    assert_eq!(
        (report.errored, report.passed),
        (1, 1),
        "{}",
        report.render(true)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_runner_write_failure_discards_output_and_world_changes() {
    use sqlx::Connection;
    use stompymux_rs::lua::testing::{self, Request};
    let (_d,c,s)=testing_vm("local t=require('testing');return t.suite('write',{tests={t.test('write',function()mux.world.object(1):state('runner'):set('bad',1);mux.world.pemit(1,'false success')end),t.test('next',function()end)}})").await;
    let mut db = sqlx::SqliteConnection::connect(c.database().to_str().unwrap())
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER reject_runner BEFORE INSERT ON object_state BEGIN SELECT RAISE(FAIL,'injected runner failure'); END").execute(&mut db).await.unwrap();
    db.close().await.unwrap();
    let report = testing::run(&s, &c, &Request::parse("test", "").unwrap(), || {})
        .await
        .unwrap();
    assert_eq!(
        (report.errored, report.passed),
        (1, 1),
        "{}",
        report.render(true)
    );
    assert!(report.render(false).contains("injected runner failure"));
    assert!(s.outbox.borrow().is_empty());
    lua(
        &s,
        "assert(mux.world.object(1):state('runner'):get('bad')==nil)",
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_runner_hook_errors_and_bounded_reporting() {
    use stompymux_rs::lua::testing::{self, Request};
    let (_d,c,s)=testing_vm(r#"
local t=require('testing');local n=0
return t.suite('hooks',{
 before_each=function(ctx)n=n+1;if n==1 then error('pre')end end,
 after_each=function(ctx)if n==2 then error('post')end end,
 after_all=function()error('last')end,
 tests={t.test('a',function()error('must skip')end),t.test('b',function()end),t.test('c',function(ctx,e)e.equal(1,2)end),false}
})
"#).await;
    let report = testing::run(&s, &c, &Request::parse("test", "").unwrap(), || {})
        .await
        .unwrap();
    assert_eq!(
        (report.passed, report.failed, report.errored),
        (0, 1, 4),
        "{}",
        report.render(true)
    );
    assert!(!report.render(true).contains("must skip"));
    let (_d,c,s)=testing_vm("local t=require('testing');local tests={};for i=1,70 do tests[#tests+1]=t.test('pass'..i,function()end);tests[#tests+1]=t.test('fail'..i,function(ctx,e)e.equal(1,2)end)end;return t.suite('many',{tests=tests})").await;
    let report = testing::run(&s, &c, &Request::parse("test", "").unwrap(), || {})
        .await
        .unwrap();
    assert_eq!((report.passed, report.failed), (70, 70));
    assert_eq!((report.passes.len(), report.failures.len()), (64, 64));
    assert!(report.passes_truncated && report.failures_truncated);
    assert!(report.render(true).contains("omitted"));
}
