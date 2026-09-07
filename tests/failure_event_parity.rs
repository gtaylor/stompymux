//! C-grounded native lock-failure event identities, suppression and rollback.
use std::{cell::RefCell, path::Path, rc::Rc};
use stompymux_rs::{
    commands::{self, Action, ExecutionContext, InputOrigin},
    config::Config,
    flags::Flag,
    lua::Scripts,
    movement::{self, Request, Route},
    persistence,
    world::{Kind, ObjectId, World},
};

/// Copy only the checked-in integration world into an isolated temporary game.
fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Load the fixture world with a reusable object module for denial probes.
async fn fixture() -> (tempfile::TempDir, Config, World) {
    let dir = tempfile::tempdir().unwrap();
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"),
        dir.path(),
    );
    std::fs::write(
        dir.path().join("lua/object_logic/failure_parity.lua"),
        "return {locks={},events={}}",
    )
    .unwrap();
    let config = Config::load(dir.path()).unwrap();
    let mut world = persistence::load(&config.database()).await.unwrap();
    for id in [ObjectId(1), ObjectId(2)] {
        world.objects.get_mut(&id).unwrap().location = Some(ObjectId(4));
    }
    (dir, config, world)
}

/// Create a normal thing at an explicit location.
fn thing(world: &mut World, config: &Config, name: &str, location: ObjectId) -> ObjectId {
    let id = world.create(config, name.into(), Kind::Thing);
    let object = world.objects.get_mut(&id).unwrap();
    object.location = Some(location);
    object.home = Some(ObjectId(config.home()));
    object.lua_parent = "failure_parity.lua".into();
    id
}

/// Start Lua after all probe attachments have been installed.
fn scripts(config: &Config, world: World) -> Scripts {
    Scripts::new(config, Rc::new(RefCell::new(world))).unwrap()
}

/// Execute a queued command with a distinct retained cause and collect its response.
fn execute(scripts: &Scripts, config: &Config, command: &str) -> String {
    let action = commands::execute(
        scripts,
        config,
        ExecutionContext {
            executor: ObjectId(2),
            cause: ObjectId(1),
            session: None,
            origin: InputOrigin::Queued,
        },
        command,
    )
    .unwrap();
    let mut output = scripts
        .outbox
        .borrow_mut()
        .drain(..)
        .map(|(_, message)| message.source().to_string())
        .collect::<Vec<_>>();
    if let Action::Reply(message)
    | Action::CommitReply(message)
    | Action::Report(message)
    | Action::StyledReport(message) = action
    {
        output.push(message);
    }
    output.join("\n")
}

#[tokio::test(flavor = "current_thread")]
async fn ordinary_and_receive_failures_keep_lock_subject_out_of_events() {
    let (_dir, config, mut world) = fixture().await;
    let item = thing(&mut world, &config, "Parcel", ObjectId(2));
    world.objects.get_mut(&ObjectId(1)).unwrap().lua_parent = "failure_parity.lua".into();
    let scripts = scripts(&config, world);
    let parent = scripts
        .lua
        .named_registry_value::<mlua::Table>("mux.parents")
        .unwrap()
        .get::<mlua::Table>("failure_parity.lua")
        .unwrap();
    scripts.lua.globals().set("parent", parent).unwrap();
    scripts
        .lua
        .load(format!(
            r#"
            trace={{}}
            parent.locks={{
              give=function(ctx)
                assert(ctx.object=={item} and ctx.enactor==2 and ctx.subject==2)
                assert(ctx.cause==1 and ctx.descriptor==nil)
                table.insert(trace,'give-lock')
                return true
              end,
              receive=function(ctx)
                assert(ctx.object==1 and ctx.enactor==2 and ctx.subject=={item})
                assert(ctx.cause==1 and ctx.descriptor==nil)
                table.insert(trace,'receive-lock')
                return false
              end,
              use=function(ctx)
                assert(ctx.object=={item} and ctx.enactor==2 and ctx.subject==2)
                assert(ctx.cause==1 and ctx.descriptor==nil)
                table.insert(trace,'use-lock')
                return false
              end
            }}
            parent.events={{
              on_give_receive_fail=function(ctx)
                assert(ctx.object==1 and ctx.enactor==2 and ctx.cause==1)
                assert(ctx.subject==nil and ctx.descriptor==nil)
                assert(ctx.operation=='receive' and ctx.silent==false and #ctx.args==0)
                table.insert(trace,'receive-fail')
              end,
              on_use_fail=function(ctx)
                assert(ctx.object=={item} and ctx.enactor==2 and ctx.cause==1)
                assert(ctx.subject==nil and ctx.descriptor==nil)
                assert(ctx.operation=='use' and ctx.silent==false and #ctx.args==0)
                table.insert(trace,'use-fail')
              end
            }}
        "#,
            item = item.0
        ))
        .exec()
        .unwrap();

    assert!(execute(&scripts, &config, "give GOD=Parcel").contains("doesn't want Parcel"));
    assert_eq!(
        scripts.world.borrow().objects[&item].location,
        Some(ObjectId(2))
    );
    assert!(execute(&scripts, &config, "use Parcel").contains("can't figure"));
    assert_eq!(
        scripts
            .lua
            .load("return table.concat(trace,',')")
            .eval::<String>()
            .unwrap(),
        "give-lock,receive-lock,receive-fail,use-lock,use-fail"
    );
    config.logger.shutdown(&config).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn traversal_failure_omits_subject_and_dark_silence_suppresses_event() {
    let (_dir, config, mut world) = fixture().await;
    let exit = ObjectId(13);
    {
        let object = world.objects.get_mut(&exit).unwrap();
        object.name = "blocked".into();
        object.location = Some(ObjectId(4));
        object.destination = Some(ObjectId(0));
        object.lua_parent = "failure_parity.lua".into();
    }
    let scripts = scripts(&config, world);
    let parent = scripts
        .lua
        .named_registry_value::<mlua::Table>("mux.parents")
        .unwrap()
        .get::<mlua::Table>("failure_parity.lua")
        .unwrap();
    scripts.lua.globals().set("parent", parent).unwrap();
    scripts
        .lua
        .load(
            r#"
      locks=0;events=0
      parent.locks={traverse=function(ctx)
        assert(ctx.object==13 and ctx.enactor==2 and ctx.subject==2)
        assert(ctx.cause==1 and ctx.descriptor==nil)
        locks=locks+1
        return {passes=false,enactor_message='TRAVERSE DENIED'}
      end}
      parent.events={on_fail=function(ctx)
        assert(ctx.object==13 and ctx.enactor==2 and ctx.cause==1)
        assert(ctx.subject==nil and ctx.descriptor==nil and ctx.lock=='traverse')
        events=events+1
      end}
    "#,
        )
        .exec()
        .unwrap();

    assert!(execute(&scripts, &config, "blocked").contains("TRAVERSE DENIED"));
    assert_eq!(
        scripts
            .lua
            .load("return locks..':'..events")
            .eval::<String>()
            .unwrap(),
        "1:1"
    );
    {
        let mut world = scripts.world.borrow_mut();
        let player = world.objects.get_mut(&ObjectId(2)).unwrap();
        player.flags.insert(Flag::Wizard);
        player.flags.insert(Flag::Dark);
    }
    assert!(!execute(&scripts, &config, "blocked").contains("TRAVERSE DENIED"));
    assert_eq!(
        scripts
            .lua
            .load("return locks..':'..events")
            .eval::<String>()
            .unwrap(),
        "2:1"
    );
    config.logger.shutdown(&config).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn teleport_failure_keeps_movement_identity_and_omits_event_subject() {
    let (_dir, config, mut world) = fixture().await;
    world.objects.get_mut(&ObjectId(0)).unwrap().lua_parent = "failure_parity.lua".into();
    let scripts = scripts(&config, world);
    let parent = scripts
        .lua
        .named_registry_value::<mlua::Table>("mux.parents")
        .unwrap()
        .get::<mlua::Table>("failure_parity.lua")
        .unwrap();
    scripts.lua.globals().set("parent", parent).unwrap();
    scripts
        .lua
        .load(
            r#"
      trace={}
      parent.locks={teleport=function(ctx)
        assert(ctx.object==0 and ctx.enactor==2 and ctx.subject==1 and ctx.cause==1)
        assert(ctx.source==4 and ctx.destination==0 and ctx.descriptor==nil)
        table.insert(trace,'teleport-lock')
        return false
      end}
      parent.events={on_teleport_destination_fail=function(ctx)
        assert(ctx.object==0 and ctx.enactor==2 and ctx.cause==1)
        assert(ctx.subject==nil and ctx.descriptor==nil and ctx.lock=='teleport')
        table.insert(trace,'teleport-fail')
      end}
    "#,
        )
        .exec()
        .unwrap();

    movement::perform(
        &scripts,
        Request {
            actor: ObjectId(1),
            object: ObjectId(2),
            cause: ObjectId(1),
            destination: ObjectId(0),
            session: Some(71),
            route: Route::Teleport,
        },
    )
    .unwrap();
    assert_eq!(
        scripts.world.borrow().objects[&ObjectId(2)].location,
        Some(ObjectId(4))
    );
    assert_eq!(
        scripts
            .lua
            .load("return table.concat(trace,',')")
            .eval::<String>()
            .unwrap(),
        "teleport-lock,teleport-fail"
    );
    config.logger.shutdown(&config).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn failure_callback_error_rolls_back_world_and_staged_output() {
    let (_dir, config, mut world) = fixture().await;
    let item = thing(&mut world, &config, "Faulty", ObjectId(4));
    let scripts = scripts(&config, world);
    let parent = scripts
        .lua
        .named_registry_value::<mlua::Table>("mux.parents")
        .unwrap()
        .get::<mlua::Table>("failure_parity.lua")
        .unwrap();
    scripts.lua.globals().set("parent", parent).unwrap();
    scripts
        .lua
        .load(format!(
            r#"
            parent.locks={{use=function(ctx)
              assert(ctx.subject==2 and ctx.cause==1)
              return {{passes=false,enactor_message='STAGED DENIAL'}}
            end}}
            parent.events={{on_use_fail=function(ctx)
              assert(ctx.subject==nil and ctx.enactor==2 and ctx.cause==1)
              mux.world.object({item}):state('failure_event'):set('leak',true)
              mux.world.pemit(2,'STAGED EVENT OUTPUT')
              error('failure event rollback probe')
            end}}
        "#,
            item = item.0
        ))
        .exec()
        .unwrap();

    let output = execute(&scripts, &config, "use Faulty");
    assert!(output.contains("failure event rollback probe"));
    assert!(!output.contains("STAGED DENIAL") && !output.contains("STAGED EVENT OUTPUT"));
    assert!(
        !scripts.world.borrow().objects[&item]
            .state
            .contains_key("failure_event")
    );
    config.logger.shutdown(&config).await.unwrap();
}
