//! Basic building, appearance, inspection and selective persistence regressions.
use sqlx::Connection;
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
async fn create_dig_edit_zone_and_restart() {
    let (_d, c, w) = fixture().await;
    let s = scripts(&c, w);
    let id = ObjectId(s.world.borrow().next_id);
    assert!(run(&s, &c, 2, "@create denied").contains("Permission denied"));
    assert!(run(&s, &c, 1, "@create Toolkit").contains("created as object"));
    assert_eq!(s.world.borrow().objects[&id].location, Some(ObjectId(1)));
    assert!(s.world.borrow().objects[&id].home.is_some());
    assert!(run(&s, &c, 1, "@name Toolkit=Equipment").contains("Name set"));
    assert!(run(&s, &c, 1, "@description Equipment=[bold]Tools[/]").contains("Set."));
    assert!(run(&s, &c, 1, "@internal-description Equipment=Inside the box").contains("Set."));
    let room = ObjectId(s.world.borrow().next_id);
    assert!(run(&s, &c, 1, "@dig Workshop=workshop;ws,out;o").contains("created with room number"));
    {
        let w = s.world.borrow();
        assert_eq!(w.objects[&ObjectId(room.0 + 1)].destination, Some(room));
        assert_eq!(
            w.objects[&ObjectId(room.0 + 2)].destination,
            Some(ObjectId(c.start()))
        );
    }
    s.world
        .borrow_mut()
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::Wizard);
    s.world
        .borrow_mut()
        .objects
        .get_mut(&id)
        .unwrap()
        .powers
        .insert(stompymux_rs::powers::Power::Idle);
    assert!(run(&s, &c, 1, &format!("@chzone Equipment=#{}", room.0)).contains("Zone changed"));
    assert!(!s.world.borrow().objects[&id].flags.contains(Flag::Wizard));
    assert!(
        s.world.borrow().objects[&id]
            .powers
            .description()
            .is_empty()
    );
    let report = run(&s, &c, 1, "@examine Equipment");
    assert!(report.contains("[bold]Tools[/]"), "{report}");
    assert!(report.contains("InternalDescription: Inside the box"));
    assert!(report.contains("Zone: Workshop"));
    save(&s, &c).await.unwrap();
    let loaded = persistence::load(&c.database()).await.unwrap();
    persistence::validate_lists(&c.database(), &loaded, c.database.busy_timeout_ms)
        .await
        .unwrap();
    assert_eq!(loaded.objects[&id].zone, Some(room));
    assert_eq!(loaded.objects[&id].name, "Equipment");
    assert_eq!(
        loaded.objects[&id].description.as_deref(),
        Some("[bold]Tools[/]")
    );
    assert!(run(&s, &c, 1, "@description Equipment=").contains("Cleared."));
    assert!(s.world.borrow().objects[&id].description.is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn player_names_aliases_and_control_are_atomic() {
    let (_d, c, mut w) = fixture().await;
    w.objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(Flag::Wizard);
    let s = scripts(&c, w);
    assert!(run(&s, &c, 2, "@name #1=Forbidden").contains("Permission denied"));
    assert!(run(&s, &c, 1, "@name #2=BuilderAlice").contains("Name set"));
    assert!(run(&s, &c, 1, "@name #2=builderalice").contains("Name set"));
    assert!(run(&s, &c, 1, "@alias #2=BuildAlias").contains("Alias set"));
    assert!(run(&s, &c, 1, "@name #1=buildalias").contains("already in use"));
    assert!(run(&s, &c, 1, "@alias #1=builderalice").contains("already in use"));
    assert!(run(&s, &c, 1, "@alias #2=BuildAlias").contains("already in use"));
    assert!(run(&s, &c, 1, "@alias here=ZoneAlias").contains("Only players"));
    assert!(run(&s, &c, 1, "@chzone #2=here").contains("Zone changed"));
    assert!(
        s.world.borrow().objects[&ObjectId(2)]
            .flags
            .contains(Flag::Wizard)
    );
    save(&s, &c).await.unwrap();
    let loaded = persistence::load(&c.database()).await.unwrap();
    assert_eq!(loaded.find_player("BUILDALIAS"), Some(ObjectId(2)));
    assert_eq!(loaded.find_player("BuilderAlice"), Some(ObjectId(2)));
    assert_eq!(loaded.find_player("#2"), Some(ObjectId(2)));
    assert!(run(&s, &c, 1, "@alias #2=").contains("Alias removed"));
    assert!(s.world.borrow().find_player("BuildAlias").is_none());
    for command in [
        "@create/x Thing",
        "@name/x #2=Name",
        "@dig/x Room",
        "@description/x here=text",
        "@chzone/x here=none",
    ] {
        assert!(run(&s, &c, 1, command).contains("Unsupported"), "{command}");
    }
    for name in ["me", "here", "home", "bad&name", "bad|name", "#123"] {
        assert!(run(&s, &c, 1, &format!("@create {name}")).contains("reasonable"));
    }
}

#[tokio::test(flavor = "current_thread")]
async fn look_modes_possessions_transparency_and_callback_rollback() {
    let (_d, c, mut w) = fixture().await;
    let room = ObjectId(c.start());
    let box_id = object(&mut w, &c, "Cabinet", Kind::Thing, room);
    let item = object(&mut w, &c, "Badge", Kind::Thing, box_id);
    w.objects.get_mut(&box_id).unwrap().description = Some("Outside cabinet".into());
    w.objects.get_mut(&box_id).unwrap().internal_description = Some("Inside cabinet".into());
    w.objects.get_mut(&item).unwrap().description = Some("Golden badge".into());
    let s = scripts(&c, w);
    assert!(run(&s, &c, 2, "look Cabinet").contains("Outside cabinet"));
    assert!(run(&s, &c, 2, "look Cabinet's Badge").contains("Golden badge"));
    assert!(run(&s, &c, 2, "look/outside").contains("can't look outside"));
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .location = Some(box_id);
    assert!(run(&s, &c, 2, "look").contains("Inside cabinet"));
    assert!(run(&s, &c, 2, "look/outside").contains("Starter Room"));
    lua(
        &s,
        "_parents['default_thing.lua'].external_appearance=function(ctx) assert(ctx.descriptor==71); return 'EXTERNAL' end; _parents['default_thing.lua'].internal_appearance=function(ctx) return 'INTERNAL' end",
    );
    assert!(run(&s, &c, 2, "look").contains("INTERNAL"));
    assert!(run(&s, &c, 2, "look Badge").contains("EXTERNAL"));
    lua(
        &s,
        "_parents['default_thing.lua'].events={on_describe=function(ctx) mux.world.object(ctx.object):state('seen'):set('count',1); error('describe failed') end}",
    );
    assert!(run(&s, &c, 2, "look Badge").contains("describe failed"));
    assert!(!s.world.borrow().objects[&item].state.contains_key("seen"));
    lua(
        &s,
        "_parents['default_thing.lua'].events=nil; _parents['default_thing.lua'].external_appearance=nil; _parents['default_thing.lua'].internal_appearance=nil",
    );
    let exit = object(
        &mut s.world.borrow_mut(),
        &c,
        "Window;win",
        Kind::Exit,
        box_id,
    );
    {
        let mut w = s.world.borrow_mut();
        let o = w.objects.get_mut(&exit).unwrap();
        o.flags.insert(Flag::Transparent);
        o.destination = Some(room);
    }
    assert!(run(&s, &c, 2, "look win").contains("Starter Room"));
    object(&mut s.world.borrow_mut(), &c, "Badge", Kind::Thing, box_id);
    assert!(run(&s, &c, 2, "look Badge").contains("which object"));
    assert!(run(&s, &c, 2, "look #1").contains("don't see"));
}

#[tokio::test(flavor = "current_thread")]
async fn dig_partial_denial_and_callback_failure() {
    let (_d, c, w) = fixture().await;
    let s = scripts(&c, w);
    lua(
        &s,
        "_parents['default_room.lua'].locks={link=function(ctx) return false end, teleport=function(ctx) return false end}",
    );
    let id = ObjectId(s.world.borrow().next_id);
    let before = s.world.borrow().objects[&ObjectId(1)].location;
    run(&s, &c, 1, "@dig/t Locked=gate,back");
    assert_eq!(s.world.borrow().objects[&id].kind, Kind::Room);
    assert_eq!(
        s.world.borrow().objects[&ObjectId(id.0 + 1)].destination,
        None
    );
    assert_eq!(s.world.borrow().objects[&ObjectId(1)].location, before);
    lua(
        &s,
        "_parents['default_room.lua'].locks.link=function(ctx) mux.world.object(ctx.object):state('failed'):set('x',1); error('link exploded') end",
    );
    let next = s.world.borrow().next_id;
    assert!(run(&s, &c, 1, "@dig Abort=gate").contains("link exploded"));
    assert_eq!(s.world.borrow().next_id, next);
    assert!(!s.world.borrow().objects.values().any(|o| o.name == "Abort"));
    lua(&s, "_parents['default_room.lua'].locks=nil");
    assert!(run(&s, &c, 1, "@dig Partial=me,out").contains("reasonable"));
    assert!(
        s.world
            .borrow()
            .objects
            .values()
            .any(|o| o.name == "Partial")
    );
    assert!(run(&s, &c, 1, "@dig/t Destination").contains("Destination"));
    let w = s.world.borrow();
    assert_eq!(
        w.objects[&w.objects[&ObjectId(1)].location.unwrap()].name,
        "Destination"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn inspection_ranges_brief_debug_and_unknown_column_preservation() {
    let (_d, c, w) = fixture().await;
    let s = scripts(&c, w);
    run(&s, &c, 1, "@dig Study=study,out");
    let room = s
        .world
        .borrow()
        .objects
        .values()
        .find(|o| o.name == "Study")
        .unwrap()
        .id;
    run(&s, &c, 1, "@create Book");
    run(&s, &c, 1, &format!("@link Book=#{}", room.0));
    run(&s, &c, 1, &format!("@link here=#{}", room.0));
    let report = run(&s, &c, 1, &format!("@entrances #{}", room.0));
    assert!(
        report.contains("[home]") && report.contains("[dropto]") && report.contains("(study)"),
        "{report}"
    );
    assert!(report.contains("3 entrances found"));
    assert!(run(&s, &c, 1, &format!("@entrances #{},99,0", room.0)).contains("0 entrances"));
    lua(&s, "mux.world.object(1):state('notes'):set('x',1)");
    assert!(run(&s, &c, 1, "@examine me").contains("State namespaces"));
    assert!(!run(&s, &c, 1, "@examine/b me").contains("State namespaces"));
    assert!(run(&s, &c, 2, "@examine").contains("Permission denied"));
    assert!(matches!(
        commands::run(&s, &c, ObjectId(1), 71, "@examine/d me").unwrap(),
        Action::ExamineDebug(ObjectId(1))
    ));
    save(&s, &c).await.unwrap();
    let mut db = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(c.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    sqlx::raw_sql("ALTER TABLE objects ADD COLUMN extra_marker BLOB; UPDATE objects SET extra_marker=x'FF0001' WHERE dbref=1;").execute(&mut db).await.unwrap();
    let expected: (i64, i64, i64) =
        sqlx::query_as("SELECT contents,exits,next FROM objects WHERE dbref=1")
            .fetch_one(&mut db)
            .await
            .unwrap();
    assert_eq!(
        persistence::inspect_links(&c.database(), ObjectId(1), 100)
            .await
            .unwrap(),
        [expected.0, expected.1, expected.2]
    );
    run(&s, &c, 1, "@description me=Changed");
    save(&s, &c).await.unwrap();
    let marker: Vec<u8> = sqlx::query_scalar("SELECT extra_marker FROM objects WHERE dbref=1")
        .fetch_one(&mut db)
        .await
        .unwrap();
    assert_eq!(marker, [255, 0, 1]);
    db.close().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn appearance_validation_visibility_and_read_only_metadata() {
    let (_d, c, mut w) = fixture().await;
    let room = ObjectId(c.start());
    let item = object(&mut w, &c, "Silver Medal", Kind::Thing, room);
    let dark = object(&mut w, &c, "Hidden", Kind::Thing, room);
    w.objects.get_mut(&dark).unwrap().flags.insert(Flag::Dark);
    let s = scripts(&c, w);
    assert!(run(&s, &c, 2, "look Hidden").contains("don't see"));
    assert!(run(&s, &c, 1, "@name Medal=Gold Medal").contains("Name set"));
    assert!(run(&s, &c, 1, "@alias *Wizard=RemoteAlias").contains("Alias set"));
    lua(
        &s,
        "_parents['default_thing.lua'].external_appearance=function(ctx) mux.world.object(ctx.object):state('bad'):set('x',1); return 42 end",
    );
    assert!(run(&s, &c, 2, "look Gold Medal").contains("must return a string"));
    assert!(!s.world.borrow().objects[&item].state.contains_key("bad"));
    lua(
        &s,
        "_parents['default_thing.lua'].external_appearance=function(ctx) while true do end end",
    );
    let failure = run(&s, &c, 2, "look Gold Medal");
    assert!(failure.contains("instruction"), "{failure}");
    lua(
        &s,
        "_parents['default_thing.lua'].external_appearance=nil; setmetatable(_parents['default_thing.lua'], {__index=function() error('metadata invoked a function') end})",
    );
    let report = run(&s, &c, 1, "@examine Gold Medal");
    assert!(
        report.contains("Lua parent: object_logic/default_thing.lua"),
        "{report}"
    );
    assert!(!report.contains("metadata invoked"));
    assert!(s.outbox.borrow().is_empty());
}
