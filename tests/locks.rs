//! Complete lock catalog, command ordering, callback identities and durable object operations.
use sqlx::Connection;
use std::{cell::RefCell, path::Path, rc::Rc};
use stompymux_rs::{
    LockInvocation, LockType,
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
    if let Action::Reply(t) | Action::CommitReply(t) | Action::Report(t) | Action::StyledReport(t) =
        action
    {
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
async fn typed_catalog_public_validation_silence_and_failure_rollback() {
    let (_d, c, w) = fixture().await;
    let s = scripts(&c, w);
    let catalog = include_str!("fixtures/lock_catalog.tsv");
    assert_eq!(
        catalog
            .lines()
            .map(|l| l.split('\t').next().unwrap())
            .collect::<Vec<_>>(),
        stompymux_rs::locks::LOCKS
            .iter()
            .map(|l| l.name())
            .collect::<Vec<_>>()
    );
    for lock in stompymux_rs::locks::LOCKS {
        lua(
            &s,
            &format!(
                r#"
            local key={key:?};local constant=mux.world.locks[{name:?}]
            assert(tostring(constant)=={name:?} and constant==mux.world.locks[{name:?}])
            local p=_parents['default_room.lua'];p.locks={{[key]=function(ctx)
              assert(ctx.lock==key and ctx.silent==true and ctx.args and #ctx.args==0)
              assert(ctx.enactor==2 and ctx.subject==2 and ctx.cause==2)
              return {{passes=true,enactor_message='not emitted'}}
            end}}
            assert(mux.world.lock_passes{{object={room},enactor=2,lock=constant}})
            assert(not pcall(function() constant.name='bad' end))
        "#,
                key = lock.key(),
                name = lock.name(),
                room = c.start()
            ),
        );
    }
    lua(
        &s,
        r#"
        assert(not pcall(function() return mux.world.locks.NOPE end))
        assert(not pcall(function() return mux.world.locks[2] end))
        assert(not pcall(function() mux.world.locks.TRAVERSE='bad' end))
        assert(not pcall(function() rawset(mux.world.locks,'TAKE',true) end))
        for _,bad in ipairs({'traverse',mux.world.flags.WIZARD,mux.world.powers.IDLE}) do
          assert(not pcall(function() mux.world.lock_passes{object=13,enactor=2,lock=bad} end))
        end
        assert(not pcall(function() mux.world.lock_passes{object=9999,enactor=2,lock=mux.world.locks.TAKE} end))
        assert(not pcall(function() mux.world.lock_passes{object=13,enactor=2,lock=mux.world.locks.TAKE,silent=false} end))
        _parents['default_exit.lua'].locks.traverse=function(ctx)
          mux.world.object(13):state('test'):set('leak',true)
          mux.world.pemit(2,'leaked output')
          return {passes='wrong'}
        end
        assert(mux.world.lock_passes{object=13,enactor=2,lock=mux.world.locks.TRAVERSE}==false)
    "#,
    );
    assert!(s.outbox.borrow().is_empty());
    assert!(
        !s.world.borrow().objects[&ObjectId(13)]
            .state
            .contains_key("test")
    );
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(13))
        .unwrap()
        .lua_parent
        .clear();
    lua(
        &s,
        "assert(mux.world.lock_passes{object=13,enactor=2,lock=mux.world.locks.TAKE})",
    );
}

#[tokio::test(flavor = "current_thread")]
async fn malformed_declarations_reject_modules_contextually() {
    for (root, source, expected) in [
        (
            "object_logic",
            "return {locks={typo=function() end}}",
            "typo",
        ),
        ("object_logic", "return {locks={take=true}}", "take"),
        (
            "object_logic",
            "return {locks={ [3]=function()end }}",
            "strings",
        ),
        ("object_logic", "return {locks=false}", "table"),
        ("global_logic", "return {locks={}}", "only valid"),
    ] {
        let (d, c, w) = fixture().await;
        std::fs::write(d.path().join(format!("lua/{root}/bad.lua")), source).unwrap();
        let error = Scripts::new(&c, Rc::new(RefCell::new(w)))
            .err()
            .unwrap()
            .to_string();
        assert!(
            error.contains("bad.lua") && error.contains(expected),
            "{error}"
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn transfers_locks_subjects_containment_and_use_messages() {
    let (_d, c, mut w) = fixture().await;
    let room = ObjectId(c.start());
    let item = object(&mut w, &c, "Widget", Kind::Thing, room);
    let bag = object(&mut w, &c, "Bag", Kind::Thing, room);
    let s = scripts(&c, w);
    lua(
        &s,
        &format!(
            r#"
        order={{}};local p=_parents['default_thing.lua'];p.locks={{
          take=function(ctx) table.insert(order,'take');assert(ctx.enactor==2 and ctx.subject==2 and ctx.descriptor==71);return true end,
          give=function(ctx) table.insert(order,'give');assert(ctx.object=={item});return true end,
          receive=function(ctx) table.insert(order,'receive');assert(ctx.object=={bag} and ctx.subject=={item});return true end,
          drop=function(ctx) table.insert(order,'drop');return true end,
          use=function(ctx) return false end
        }};p.messages={{use=function(ctx) return {{enactor_message='activated',other_message='activates it'}} end}};
        p.events={{on_use=function(ctx) mux.world.object(ctx.object):state('use'):set('done',true) end}}
    "#,
            item = item.0,
            bag = bag.0
        ),
    );
    assert!(run(&s, &c, 2, "get Widget").contains("Taken."));
    assert!(run(&s, &c, 2, "inventory").contains("Widget"));
    assert!(run(&s, &c, 2, "give Bag=Widget").contains("Given."));
    assert_eq!(s.world.borrow().objects[&item].location, Some(bag));
    assert!(run(&s, &c, 2, "get Bag's Widget").contains("Taken."));
    assert!(run(&s, &c, 2, "drop Widget").contains("Dropped."));
    lua(
        &s,
        "assert(table.concat(order,',')=='take,give,receive,take,drop')",
    );
    assert!(run(&s, &c, 2, "use Widget").contains("can't figure"));
    lua(
        &s,
        "_parents['default_thing.lua'].locks.use=function() return true end",
    );
    assert!(run(&s, &c, 2, "use Widget").contains("activated"));
    assert!(s.world.borrow().objects[&item].state.contains_key("use"));
    run(&s, &c, 2, "get Bag");
    assert!(
        run(&s, &c, 2, "give Bag=Bag").contains("itself")
            || s.world.borrow().objects[&bag].location == Some(ObjectId(2))
    );
    save(&s, &c).await.unwrap();
    let loaded = persistence::load(&c.database()).await.unwrap();
    assert_eq!(loaded.objects[&item].location, Some(room));
}

#[tokio::test(flavor = "current_thread")]
async fn enter_leave_speak_matching_and_callback_rollback() {
    let (_d, c, mut w) = fixture().await;
    let room = ObjectId(c.start());
    let bag = object(&mut w, &c, "Cabin", Kind::Thing, room);
    let first = object(&mut w, &c, "Twin", Kind::Thing, room);
    let second = object(&mut w, &c, "Twin", Kind::Thing, room);
    let s = scripts(&c, w);
    lua(
        &s,
        &format!(
            r#"
      trace={{}};local p=_parents['default_thing.lua'];p.locks={{match=function(ctx) assert(ctx.silent);return ctx.object=={second} end,enter=function(ctx) table.insert(trace,'enter');return true end,leave=function(ctx) table.insert(trace,'leave');return true end}};
      _parents['default_room.lua'].locks={{leave=function(ctx) table.insert(trace,'room-leave');return true end,enter=function(ctx) table.insert(trace,'room-enter');return true end,speak=function(ctx) return {{passes=false,enactor_message='speech denied'}} end}}
    "#,
            second = second.0
        ),
    );
    run(&s, &c, 2, "get Twin");
    assert_eq!(
        s.world.borrow().objects[&second].location,
        Some(ObjectId(2))
    );
    assert_eq!(s.world.borrow().objects[&first].location, Some(room));
    run(&s, &c, 2, "enter Cabin");
    assert_eq!(s.world.borrow().objects[&ObjectId(2)].location, Some(bag));
    run(&s, &c, 2, "leave");
    assert_eq!(s.world.borrow().objects[&ObjectId(2)].location, Some(room));
    lua(
        &s,
        "assert(table.concat(trace,',')=='enter,room-leave,leave,room-enter')",
    );
    assert!(run(&s, &c, 2, "say outside").contains("outside"));
    s.world
        .borrow_mut()
        .objects
        .get_mut(&room)
        .unwrap()
        .flags
        .insert(Flag::Auditorium);
    assert!(run(&s, &c, 2, "\"blocked").contains("speech denied"));
    lua(&s, "_parents['default_room.lua'].locks.speak=nil");
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Gagged);
    assert!(run(&s, &c, 1, "say wizard").contains("wizard"));
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(Flag::Gagged);
    assert!(run(&s, &c, 2, "say denied").contains("Gagged"));
    lua(
        &s,
        "_parents['default_thing.lua'].events={on_enter=function(ctx) mux.world.object(ctx.object):state('bad'):set('leak',true);error('movement failed') end}",
    );
    assert!(run(&s, &c, 2, "enter Cabin").contains("movement failed"));
    assert_eq!(s.world.borrow().objects[&ObjectId(2)].location, Some(room));
    assert!(!s.world.borrow().objects[&bag].state.contains_key("bad"));
}

#[tokio::test(flavor = "current_thread")]
async fn builder_creation_link_home_clone_and_dropto_roundtrip() {
    let (_d, c, mut w) = fixture().await;
    let room = ObjectId(c.start());
    let target = object(&mut w, &c, "Workshop", Kind::Room, room);
    let template = object(&mut w, &c, "Template", Kind::Thing, room);
    w.objects.get_mut(&template).unwrap().description = Some("copied description".into());
    w.objects
        .get_mut(&template)
        .unwrap()
        .flags
        .insert(Flag::Wizard);
    let s = scripts(&c, w);
    lua(
        &s,
        &format!(
            r#"
        seen={{}};_parents['default_room.lua'].locks={{link=function(ctx) assert(ctx.subject==1 and ctx.enactor==1);table.insert(seen,'link');return true end,set_home=function(ctx) table.insert(seen,'home');return true end}}
        mux.world.object({template}):state('copy'):set('value','content')
        _parents['default_thing.lua'].events={{on_clone=function(ctx) assert(ctx.event=='on_clone' and ctx.enactor==1);mux.world.object(ctx.object):state('copy'):set('cloned',true) end}}
    "#,
            template = template.0
        ),
    );
    assert!(run(&s, &c, 2, "@open forbidden").contains("Permission denied"));
    let exit = ObjectId(s.world.borrow().next_id);
    assert!(
        run(
            &s,
            &c,
            1,
            &format!("@op out={target},back", target = target.0)
        )
        .contains("Linked.")
    );
    assert_eq!(s.world.borrow().objects[&exit].destination, Some(target));
    let back = ObjectId(exit.0 + 1);
    assert_eq!(s.world.borrow().objects[&back].location, Some(target));
    assert_eq!(s.world.borrow().objects[&back].destination, Some(room));
    run(&s, &c, 1, &format!("@unlink #{}", exit.0));
    assert_eq!(s.world.borrow().objects[&exit].destination, None);
    run(&s, &c, 1, &format!("@link #{}=#{}", exit.0, target.0));
    assert!(run(&s, &c, 1, &format!("@link #{}=#{}", template.0, target.0)).contains("Home set"));
    lua(&s, "assert(table.concat(seen,',')=='link,link,link,home')");
    let clone = ObjectId(s.world.borrow().next_id);
    assert!(run(&s, &c, 1, &format!("@clone/inv #{}=Copy", template.0)).contains("cloned"));
    let cloned = s.world.borrow().objects[&clone].clone();
    assert_eq!(cloned.location, Some(ObjectId(1)));
    assert_eq!(cloned.description.as_deref(), Some("copied description"));
    assert!(cloned.state["copy"].contains_key("cloned"));
    assert!(!cloned.flags.contains(Flag::Wizard));
    run(&s, &c, 1, &format!("@link here=#{}", target.0));
    run(&s, &c, 1, "drop Copy");
    assert_eq!(s.world.borrow().objects[&clone].location, Some(target));
    let room_clone = ObjectId(s.world.borrow().next_id);
    run(&s, &c, 1, &format!("@clone #{}=Room Copy", room.0));
    assert_eq!(s.world.borrow().objects[&room_clone].dropto, Some(target));
    lua(
        &s,
        "_parents['default_room.lua'].locks.link=function()return false end",
    );
    let denied = ObjectId(s.world.borrow().next_id);
    run(&s, &c, 1, &format!("@clone #{}=Blocked copy", exit.0));
    assert_eq!(s.world.borrow().objects[&denied].destination, None);
    assert!(run(&s, &c, 1, "@clone #2").contains("cannot clone"));
    run(&s, &c, 1, "@unlink here");
    assert_eq!(s.world.borrow().objects[&room].dropto, None);
    save(&s, &c).await.unwrap();
    let loaded = persistence::load(&c.database()).await.unwrap();
    loaded.validate(&c).unwrap();
    assert_eq!(loaded.objects[&clone].location, Some(target));
    assert_eq!(loaded.objects[&back].destination, Some(room));
    persistence::validate_lists(&c.database(), &loaded, c.database.busy_timeout_ms)
        .await
        .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn link_failure_partial_success_callback_rollback_and_write_failure() {
    let (_d, c, mut w) = fixture().await;
    let room = ObjectId(c.start());
    let target = object(&mut w, &c, "Target", Kind::Room, room);
    let s = scripts(&c, w);
    lua(
        &s,
        "_parents['default_room.lua'].locks={link=function() return false end}",
    );
    let opened = ObjectId(s.world.borrow().next_id);
    assert!(run(&s, &c, 1, &format!("@open door=#{}", target.0)).contains("can't link"));
    assert_eq!(s.world.borrow().objects[&opened].destination, None);
    save(&s, &c).await.unwrap();
    let before = s.world.borrow().clone();
    lua(
        &s,
        "_parents['default_room.lua'].locks.link=function(ctx) mux.world.object(ctx.object):state('bad'):set('leak',1);error('link failed') end",
    );
    assert!(run(&s, &c, 1, &format!("@open aborted=#{}", target.0)).contains("link failed"));
    assert_eq!(s.world.borrow().next_id, before.next_id);
    assert!(!s.world.borrow().objects[&target].state.contains_key("bad"));
    lua(
        &s,
        "_parents['default_room.lua'].locks.link=function()return true end",
    );
    let mut db = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(c.database()),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TRIGGER deny_new BEFORE INSERT ON objects BEGIN SELECT RAISE(FAIL,'new object blocked'); END").execute(&mut db).await.unwrap();
    run(&s, &c, 1, "@clone here=Failed Room");
    assert!(save(&s, &c).await.is_err());
    let durable = persistence::load(&c.database()).await.unwrap();
    assert_eq!(durable.next_id, before.next_id);
    db.close().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn native_results_nested_policies_and_channel_descriptor() {
    let (_d, c, mut w) = fixture().await;
    let room = ObjectId(c.start());
    let item = object(&mut w, &c, "Policy", Kind::Thing, room);
    let s = scripts(&c, w);
    lua(
        &s,
        &format!(
            r#"
        _parents['default_thing.lua'].locks={{take=function(ctx)
            assert(ctx.subject==1 and ctx.cause==2 and not ctx.silent)
            return mux.world.lock_passes{{object={room},enactor=2,lock=mux.world.locks.SPEAK}}
        end}}
        _parents['default_room.lua'].locks={{speak=function(ctx)assert(ctx.silent and ctx.descriptor==71);return true end}}
    "#,
            room = room.0
        ),
    );
    assert!(
        s.evaluate_lock(LockInvocation {
            kind: LockType::Take,
            object: item,
            enactor: ObjectId(2),
            subject: ObjectId(1),
            cause: ObjectId(2),
            descriptor: Some(71),
            silent: false
        })
        .unwrap()
        .passes
    );
    run(&s, &c, 1, "@chan/create PolicyChan");
    run(&s, &c, 1, &format!("@chan/object PolicyChan=#{}", item.0));
    lua(
        &s,
        "_parents['default_thing.lua'].locks={channel_join=function(ctx) assert(ctx.silent and ctx.descriptor==71 and #ctx.args==0);mux.world.object(ctx.object):state('channel'):set('checked',true);return true end}",
    );
    run(&s, &c, 2, "addcom pol=PolicyChan");
    assert!(
        s.world.borrow().objects[&item]
            .state
            .contains_key("channel")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn movement_exit_inventory_and_lock_resource_failure() {
    let (d, c, mut w) = fixture().await;
    let room = ObjectId(c.start());
    let cabin = object(&mut w, &c, "Quiet Cabin", Kind::Thing, room);
    let exit = object(&mut w, &c, "Loose Exit", Kind::Exit, room);
    w.objects.get_mut(&exit).unwrap().destination = Some(room);
    let s = scripts(&c, w);
    lua(
        &s,
        r#"
      events={};_parents['default_room.lua'].events={on_leave=function()table.insert(events,'room-leave')end,on_enter=function()table.insert(events,'room-enter')end}
      _parents['default_thing.lua'].events={on_leave=function()table.insert(events,'cabin-leave')end,on_enter=function()table.insert(events,'cabin-enter')end}
      _parents['default_thing.lua'].locks={enter=function(ctx) assert(not ctx.silent);return true end,leave=function(ctx) assert(not ctx.silent);return true end}
    "#,
    );
    run(&s, &c, 1, "enter Quiet Cabin");
    assert_eq!(s.world.borrow().objects[&ObjectId(1)].location, Some(cabin));
    run(&s, &c, 1, "leave");
    lua(
        &s,
        "assert(table.concat(events,',')=='room-leave,cabin-enter,cabin-leave,room-enter')",
    );
    assert!(run(&s, &c, 2, "get Loose Exit").contains("Permission denied"));
    assert!(run(&s, &c, 1, "get Loose Exit").contains("Exit taken"));
    assert_eq!(s.world.borrow().objects[&exit].destination, Some(room));
    assert!(run(&s, &c, 1, "drop Loose Exit").contains("Exit dropped"));
    assert!(run(&s, &c, 2, "give/q #1=anything").contains("Unsupported command switch"));
    assert!(run(&s, &c, 1, "enter/invalid Quiet Cabin").contains("Unsupported"));
    let path = d.path().join("stompymux.toml");
    let source = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        source.replace("[lua]", "[lua]\ninstruction_limit=2000"),
    )
    .unwrap();
    let limited = Config::load(d.path()).unwrap();
    let s = scripts(&limited, s.world.borrow().clone());
    lua(
        &s,
        "_parents['default_thing.lua'].locks={use=function(ctx) mux.world.object(ctx.object):state('bad'):set('value',1);while true do end end}",
    );
    assert!(run(&s, &limited, 1, "use Quiet Cabin").contains("budget"));
    assert!(!s.world.borrow().objects[&cabin].state.contains_key("bad"));
}
