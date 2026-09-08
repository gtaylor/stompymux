//! C-grounded movement action order, contexts, suppression and nested rollback.
use std::{cell::RefCell, path::Path, rc::Rc};
use stompymux_rs::{Config, Flag, Kind, ObjectId, Scripts, commands};

mod support;
use support::isolated_world;

/// Install tracing providers on both rooms and the traveler.
async fn fixture() -> (tempfile::TempDir, Config, Scripts) {
    let (dir, config, mut world) = isolated_world().await;
    std::fs::write(dir.path().join("lua/object_logic/parity.lua"),r#"
      local function record(ctx,key)
        trace=trace or {};table.insert(trace,key..(ctx.silent and ':silent' or ''))
        assert(ctx.enactor==traveler and ctx.descriptor==expected_descriptor)
        assert(ctx.source==source and ctx.destination==destination)
        assert(ctx.cause==((key=='teleport_source' or key=='teleport' or key=='on_teleport' or key=='move' or key=='on_move') and object_cause or -1))
        if ctx.object==traveler then assert(ctx.operation==operation) else assert(ctx.operation=='move') end
        if key==fail_at then
          mux.world.object(traveler):state('parity'):set('leak',1)
          mux.world.pemit(traveler,mux.text.markdown('**DISCARDED**'))
          assert(mux.log('movement.log','DISCARDED'))
          error('movement failure')
        end
      end
      local t={messages={},events={}}
      for _,key in ipairs({'teleport_source','leave','enter_source','teleport','move','enter','leave_destination'}) do
        t.messages[key]=function(ctx)
          record(ctx,key)
          if key=='teleport_source' or key=='enter_source' or key=='leave_destination' then return {other_message=key} end
          return {enactor_message=key,other_message=key}
        end
      end
      for _,key in ipairs({'on_leave','on_enter','on_teleport','on_move'}) do
        t.events[key]=function(ctx) record(ctx,key) end
      end
      return t
    "#).unwrap();
    std::fs::create_dir(dir.path().join("logs")).unwrap();
    std::fs::write(dir.path().join("logs/movement.log"), "").unwrap();
    for id in [0, 4, 2] {
        world.objects.get_mut(&ObjectId(id)).unwrap().lua_parent = "parity.lua".into();
    }
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(ObjectId(4));
    world.objects.get_mut(&ObjectId(2)).unwrap().home = Some(ObjectId(0));
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(ObjectId(4));
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .inspect_lua()
        .load("traveler=2;source=4;destination=0;object_cause=1;operation='teleport';trace={}")
        .exec()
        .unwrap();
    (dir, config, scripts)
}

/// Compare the actual invocation sequence with the checked-in C trace.
fn trace(s: &Scripts, key: &str) {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/movement-parity.json")).unwrap();
    let expected = fixture[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(
        s.inspect_lua()
            .load("return table.concat(trace,',')")
            .eval::<String>()
            .unwrap(),
        expected
    );
}

#[tokio::test(flavor = "current_thread")]
async fn native_teleport_home_dark_and_noop_match_c_actions() {
    for (command, key, dark, descriptor, cause, operation) in [
        ("@teleport #2=#0", "teleport", false, None, 1, "teleport"),
        (
            "@teleport #2=#0",
            "dark_teleport",
            true,
            None,
            1,
            "teleport",
        ),
        ("home", "home", false, Some(7), -1, "move"),
    ] {
        let (_d, c, s) = fixture().await;
        if dark {
            s.world_mut()
                .objects
                .get_mut(&ObjectId(2))
                .unwrap()
                .flags
                .insert(Flag::Dark);
        }
        s.inspect_lua()
            .globals()
            .set("expected_descriptor", descriptor)
            .unwrap();
        s.inspect_lua()
            .globals()
            .set("object_cause", cause)
            .unwrap();
        s.inspect_lua()
            .globals()
            .set("operation", operation)
            .unwrap();
        let actor = if command == "home" { 2 } else { 1 };
        commands::run(&s, &c, ObjectId(actor), 7, command).unwrap();
        trace(&s, key);
        assert_eq!(s.world().objects[&ObjectId(2)].location, Some(ObjectId(0)));
        let messages = s
            .outbox()
            .iter()
            .map(|(id, t)| (id.0, t.source().to_string()))
            .collect::<Vec<_>>();
        if dark {
            assert!(messages.contains(&(2, "leave".into())));
            assert!(!messages.iter().any(|(_, t)| t.contains("has left.")));
        } else {
            assert!(
                messages
                    .iter()
                    .any(|(id, t)| *id == 1 && t.contains("has left."))
            );
        }
        if command == "home" {
            assert_eq!(
                messages
                    .iter()
                    .filter(|(_, t)| t == "There's no place like home...")
                    .count(),
                3
            );
        }
        s.inspect_lua().load("trace={}").exec().unwrap();
        commands::run(&s, &c, ObjectId(actor), 7, command).unwrap();
        assert_eq!(
            s.inspect_lua()
                .load("return #trace")
                .eval::<usize>()
                .unwrap(),
            0
        );
        c.logger.shutdown(&c).await.unwrap();
    }
}

#[tokio::test(flavor = "current_thread")]
async fn lua_caught_failure_discards_every_movement_phase() {
    let (d, c, s) = fixture().await;
    for phase in [
        "teleport_source",
        "leave",
        "on_leave",
        "teleport",
        "on_teleport",
        "move",
        "on_move",
        "enter",
        "on_enter",
    ] {
        s.inspect_lua().globals().set("fail_at", phase).unwrap();
        let before = serde_json::to_value(&*s.world()).unwrap();
        s.eval_callback::<()>(
            "local ok=pcall(mux.world.teleport_object,{object=2,destination=0});assert(not ok)",
        )
        .unwrap();
        assert_eq!(
            before,
            serde_json::to_value(&*s.world()).unwrap(),
            "{phase}"
        );
        assert!(s.outbox().is_empty());
        assert!(s.drain_logs_for_inspection().is_empty());
    }
    assert_eq!(
        std::fs::read_to_string(d.path().join("logs/movement.log")).unwrap(),
        ""
    );
    s.inspect_lua()
        .globals()
        .set("fail_at", mlua::Value::Nil)
        .unwrap();
    s.inspect_lua().load("trace={}").exec().unwrap();
    s.eval_callback::<()>("mux.world.teleport_object{object=2,destination=0}")
        .unwrap();
    trace(&s, "teleport");
    c.logger.shutdown(&c).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn nested_container_thing_and_exit_relocation() {
    let (_d, c, s) = fixture().await;
    let thing = {
        let mut w = s.world_mut();
        let id = w.create(&c, "Container".into(), Kind::Thing);
        let o = w.objects.get_mut(&id).unwrap();
        o.location = Some(ObjectId(4));
        o.lua_parent = "parity.lua".into();
        w.objects.get_mut(&ObjectId(2)).unwrap().location = Some(id);
        id
    };
    s.inspect_lua().globals().set("traveler", thing.0).unwrap();
    commands::run(
        &s,
        &c,
        ObjectId(1),
        7,
        &format!("@teleport #{}=#0", thing.0),
    )
    .unwrap();
    trace(&s, "teleport");
    assert_eq!(s.world().objects[&ObjectId(2)].location, Some(thing));
    s.inspect_lua().load("trace={}").exec().unwrap();
    let linked = s.world().objects[&ObjectId(13)].destination;
    commands::run(&s, &c, ObjectId(1), 7, "@teleport #13=#0").unwrap();
    assert_eq!(s.world().objects[&ObjectId(13)].destination, linked);
    assert_eq!(
        s.inspect_lua()
            .load("return #trace")
            .eval::<usize>()
            .unwrap(),
        0
    );
    for command in [
        "goto/quiet x",
        "@teleport/quiet #4",
        "@teleport/loud #4",
        "@teleport #13",
    ] {
        let before = serde_json::to_value(&*s.world()).unwrap();
        commands::run(&s, &c, ObjectId(1), 7, command).unwrap();
        assert_eq!(before, serde_json::to_value(&*s.world()).unwrap());
    }
    c.logger.shutdown(&c).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn appearance_precedes_arrival_and_callback_containment_is_rechecked() {
    let (_d, c, s) = fixture().await;
    let parents = s
        .inspect_lua()
        .named_registry_value::<mlua::Table>("mux.parents")
        .unwrap();
    s.inspect_lua()
        .globals()
        .set("parent", parents.get::<mlua::Table>("parity.lua").unwrap())
        .unwrap();
    s.inspect_lua().load("parent.internal_appearance=function(ctx) table.insert(trace,'appearance');return 'ARRIVAL APPEARANCE' end").exec().unwrap();
    commands::run(&s, &c, ObjectId(1), 7, "@teleport #2=#0").unwrap();
    assert!(
        s.inspect_lua()
            .load("return table.concat(trace,',')")
            .eval::<String>()
            .unwrap()
            .contains("enter_source,appearance,teleport,on_teleport,move,on_move,enter")
    );
    s.world_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .location = Some(ObjectId(4));
    s.drain_outbox();
    s.inspect_lua().load("parent.internal_appearance=function() assert(mux.log('movement.log','discarded appearance'));error('bad appearance') end").exec().unwrap();
    s.eval_callback::<()>("assert(not pcall(mux.world.teleport_object,{object=2,destination=0}))")
        .unwrap();
    assert_eq!(s.world().objects[&ObjectId(2)].location, Some(ObjectId(4)));
    assert!(s.outbox().is_empty());
    assert!(s.drain_logs_for_inspection().is_empty());
    let target = {
        let mut w = s.world_mut();
        let id = w.create(&c, "Changing destination".into(), Kind::Thing);
        w.objects.get_mut(&id).unwrap().location = Some(ObjectId(0));
        id
    };
    s.inspect_lua().globals().set("target", target.0).unwrap();
    s.inspect_lua().load("parent.internal_appearance=nil;parent.messages={};parent.events={on_leave=function(ctx) if ctx.enactor==2 then mux.world.teleport_object{object=target,destination=2} end end}").exec().unwrap();
    let before = serde_json::to_value(&*s.world()).unwrap();
    s.eval_callback::<()>(
        "assert(not pcall(mux.world.teleport_object,{object=2,destination=target}))",
    )
    .unwrap();
    assert_eq!(before, serde_json::to_value(&*s.world()).unwrap());
    c.logger.shutdown(&c).await.unwrap();
}

/// C silent providers, immediate appearance and routed neighbor action messages.
#[tokio::test(flavor = "current_thread")]
async fn generic_actions_match_c_silence_order_and_routing() {
    let (_d, c, s) = fixture().await;
    let parents = s
        .inspect_lua()
        .named_registry_value::<mlua::Table>("mux.parents")
        .unwrap();
    s.inspect_lua()
        .globals()
        .set("parent", parents.get::<mlua::Table>("parity.lua").unwrap())
        .unwrap();
    s.inspect_lua().load(r#"
      trace={};parent.messages={leave=function(ctx) table.insert(trace,'leave');return {enactor_message='silent direct'} end,
      enter=function(ctx) table.insert(trace,'enter');return {} end,
      move=function(ctx) table.insert(trace,'move');return {} end};parent.events={}
      parent.internal_appearance=function(ctx) table.insert(trace,'appearance');return '' end
    "#).exec().unwrap();
    s.world_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(Flag::Dark);
    stompymux_rs::movement::perform(
        &s,
        stompymux_rs::movement::Request {
            actor: ObjectId(2),
            object: ObjectId(2),
            cause: ObjectId(2),
            destination: ObjectId(0),
            session: Some(7),
            route: stompymux_rs::movement::Route::Generic,
        },
    )
    .unwrap();
    // C invokes silent leave/enter providers and renders before move.
    assert_eq!(
        s.inspect_lua()
            .load("return table.concat(trace,',')")
            .eval::<String>()
            .unwrap(),
        "leave,appearance,move,enter"
    );
    assert!(
        s.outbox()
            .iter()
            .any(|(_, d)| d.source() == "silent direct")
    );
    s.drain_outbox();
    // An audible exit leads from the actor's room to the remote observer's room.
    {
        let mut w = s.world_mut();
        w.objects.get_mut(&ObjectId(1)).unwrap().location = Some(ObjectId(4));
        let e = w.objects.get_mut(&ObjectId(13)).unwrap();
        e.location = Some(ObjectId(0));
        e.destination = Some(ObjectId(4));
        e.flags.insert(Flag::Audible);
    }
    s.action_text(ObjectId(2), ObjectId(2), None, Some("ordinary action"))
        .unwrap();
    assert!(
        s.outbox()
            .iter()
            .any(|(id, d)| *id == ObjectId(1) && d.source().contains("ordinary action"))
    );
    assert!(!s.outbox().iter().any(|(id, _)| *id == ObjectId(2)));
    c.logger.shutdown(&c).await.unwrap();
}

/// Evidence references and statuses remain reviewable as the sources evolve.
#[test]
fn audit_matrix_has_evidence_and_all_requested_areas() {
    let matrix: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/behavioral-parity.json")).unwrap();
    let rows = matrix["behaviors"].as_array().unwrap();
    let mut ids = std::collections::BTreeSet::new();
    for row in rows {
        assert!(ids.insert(row["id"].as_str().unwrap()));
        assert!(
            [
                "verified equivalent",
                "intentional difference",
                "confirmed discrepancy",
                "unverified"
            ]
            .contains(&row["status"].as_str().unwrap())
        );
        assert!(!row["note"].as_str().unwrap().is_empty());
        assert!(
            row["c_source"]
                .as_str()
                .unwrap()
                .starts_with("btmux-khi/src/mux/")
        );
        for path in std::iter::once(&row["rust_source"]).chain(row["tests"].as_array().unwrap()) {
            assert!(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(path.as_str().unwrap())
                    .is_file(),
                "{path}"
            );
        }
    }
    for area in [
        "commands/switches",
        "targeting/permissions",
        "movement/locks",
        "object lifecycle",
        "communication",
        "macros/queues",
        "Lua APIs/callbacks",
        "accounts/admission",
        "configuration",
        "Telnet/rendering",
        "help",
        "logging",
        "operational reports",
    ] {
        assert!(rows.iter().any(|r| r["area"] == area), "{area}");
    }
}

/// C exit travel moves the executor and runs exit/location/traveler actions.
#[tokio::test(flavor = "current_thread")]
async fn exit_executor_and_action_sequence_match_c() {
    let (_d, c, s) = fixture().await;
    {
        let mut w = s.world_mut();
        for id in [0, 1, 2, 4, 13] {
            w.objects.get_mut(&ObjectId(id)).unwrap().lua_parent = String::new();
        }
        let e = w.objects.get_mut(&ObjectId(13)).unwrap();
        e.name = "parityexit".into();
        e.location = Some(ObjectId(4));
        e.destination = Some(ObjectId(0));
    }
    commands::execute(
        &s,
        &c,
        commands::ExecutionContext {
            executor: ObjectId(2),
            cause: ObjectId(1),
            session: None,
            origin: commands::InputOrigin::Queued,
        },
        "parityexit",
    )
    .unwrap();
    assert_eq!(s.world().objects[&ObjectId(1)].location, Some(ObjectId(4)));
    assert_eq!(s.world().objects[&ObjectId(2)].location, Some(ObjectId(0)));
    s.world_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .location = Some(ObjectId(4));
    let parents = s
        .inspect_lua()
        .named_registry_value::<mlua::Table>("mux.parents")
        .unwrap();
    s.inspect_lua()
        .globals()
        .set("parent", parents.get::<mlua::Table>("parity.lua").unwrap())
        .unwrap();
    s.inspect_lua().load("trace={};parent.messages={};parent.events={};for _,key in ipairs({'on_success','on_leave','on_exit','on_drop','on_move','on_enter'}) do parent.events[key]=function(ctx) table.insert(trace,key) end end").exec().unwrap();
    {
        let mut w = s.world_mut();
        for id in [0, 2, 4, 13] {
            w.objects.get_mut(&ObjectId(id)).unwrap().lua_parent = "parity.lua".into();
        }
    }
    commands::run(&s, &c, ObjectId(2), 7, "parityexit").unwrap();
    // No obsolete on_exit callback fires alongside the C action sequence.
    assert_eq!(
        s.inspect_lua()
            .load("return table.concat(trace,',')")
            .eval::<String>()
            .unwrap(),
        "on_success,on_leave,on_drop,on_move,on_enter"
    );
    c.logger.shutdown(&c).await.unwrap();
}

/// The C exit trace includes providers, contexts, immediate look and per-phase visibility.
#[tokio::test(flavor = "current_thread")]
async fn exit_contexts_suppression_and_callback_rollback() {
    let (_d, c, s) = fixture().await;
    let parents = s
        .inspect_lua()
        .named_registry_value::<mlua::Table>("mux.parents")
        .unwrap();
    s.inspect_lua()
        .globals()
        .set("parent", parents.get::<mlua::Table>("parity.lua").unwrap())
        .unwrap();
    s.inspect_lua().load(r#"
      local function record(ctx,key)
        table.insert(trace,key..(ctx.silent and ':silent' or ''))
        assert(ctx.enactor==2 and ctx.cause==expected_cause and ctx.descriptor==nil)
        assert(ctx.source==4 and ctx.destination==0 and #ctx.args==0)
        local id=({success=13,on_success=13,drop=13,on_drop=13,leave=4,on_leave=4,
          enter_source=0,move=2,on_move=2,enter=0,on_enter=0,leave_destination=4})[key]
        assert(ctx.object==id)
        assert(ctx.operation==(id==13 and 'traverse' or 'move'))
        mux.world.object(2):state('parity'):set('last',key)
        if fail_at==key then
          mux.world.pemit(2,mux.text.markdown('**DISCARDED EXIT**'))
          assert(mux.log('movement.log','DISCARDED EXIT'))
          error('exit callback failed')
        end
      end
      parent.messages={};parent.events={}
      for _,key in ipairs({'success','leave','enter_source','drop','move','enter','leave_destination'}) do
        parent.messages[key]=function(ctx)
          record(ctx,key)
          if key=='enter_source' or key=='leave_destination' then return {other_message=key} end
          return {enactor_message='direct '..key,other_message='neighbor '..key}
        end
      end
      for _,key in ipairs({'on_success','on_leave','on_drop','on_move','on_enter'}) do
        parent.events[key]=function(ctx) record(ctx,key) end
      end
      parent.events.on_exit=function() error('obsolete on_exit') end
      parent.internal_appearance=function(ctx)
        table.insert(trace,'appearance')
        if fail_at=='appearance' then error('render failed') end
        return 'EXIT APPEARANCE'
      end
      parent.locks={traverse=function(ctx)
        assert(ctx.object==13 and ctx.enactor==2 and ctx.subject==2)
        assert(ctx.cause==expected_cause and ctx.descriptor==nil)
        return {passes=not deny, enactor_message='LOCK DENIED'}
      end}
    "#).exec().unwrap();
    {
        let mut w = s.world_mut();
        w.objects.get_mut(&ObjectId(1)).unwrap().lua_parent.clear();
        let exit = w.objects.get_mut(&ObjectId(13)).unwrap();
        exit.name = "parityexit".into();
        exit.location = Some(ObjectId(4));
        exit.destination = Some(ObjectId(0));
        exit.lua_parent = "parity.lua".into();
    }
    let execution = |cause| commands::ExecutionContext {
        executor: ObjectId(2),
        cause: ObjectId(cause),
        session: None,
        origin: commands::InputOrigin::Queued,
    };
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/movement-parity.json")).unwrap();
    for (wizard, dark, cause, expected) in [
        (true, false, 1, "exit"),
        (true, true, 1, "dark_exit"),
        (false, true, 2, "exit"),
    ] {
        {
            let mut w = s.world_mut();
            let traveler = w.objects.get_mut(&ObjectId(2)).unwrap();
            traveler.location = Some(ObjectId(4));
            if wizard {
                traveler.flags.insert(Flag::Wizard);
            } else {
                traveler.flags.remove(Flag::Wizard);
            }
            if dark {
                traveler.flags.insert(Flag::Dark);
            } else {
                traveler.flags.remove(Flag::Dark);
            }
        }
        s.drain_outbox();
        s.inspect_lua()
            .load(format!(
                "trace={{}}; expected_cause={cause};fail_at=nil;deny=false"
            ))
            .exec()
            .unwrap();
        commands::execute(&s, &c, execution(cause), "parityexit").unwrap();
        let actual: Vec<String> = s.inspect_lua().load("return trace").eval().unwrap();
        let expected: Vec<String> = serde_json::from_value(fixture[expected].clone()).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(s.world().objects[&ObjectId(1)].location, Some(ObjectId(4)));
        assert_eq!(s.world().objects[&ObjectId(2)].location, Some(ObjectId(0)));
        if wizard && dark {
            assert!(
                s.outbox()
                    .iter()
                    .any(|(id, d)| *id == ObjectId(2) && d.source() == "direct success")
            );
            assert!(
                !s.outbox()
                    .iter()
                    .any(|(_, d)| d.source().contains("neighbor success"))
            );
        }
    }
    // Every phase restores durable mutations and staged output on failure.
    for phase in fixture["exit"].as_array().unwrap() {
        let phase = phase.as_str().unwrap();
        {
            let mut w = s.world_mut();
            let traveler = w.objects.get_mut(&ObjectId(2)).unwrap();
            traveler.location = Some(ObjectId(4));
            traveler.flags.remove(Flag::Dark);
        }
        s.drain_outbox();
        let before = serde_json::to_value(&*s.world()).unwrap();
        s.inspect_lua()
            .load(format!("trace={{}};expected_cause=1;fail_at='{phase}'"))
            .exec()
            .unwrap();
        assert!(
            commands::execute(&s, &c, execution(1), "parityexit").is_err(),
            "{phase}"
        );
        assert_eq!(
            before,
            serde_json::to_value(&*s.world()).unwrap(),
            "{phase}"
        );
        assert!(s.outbox().is_empty(), "{phase}");
        assert!(s.drain_logs_for_inspection().is_empty(), "{phase}");
    }
    // Changing the matched exit from a provider invalidates the pending move atomically.
    s.inspect_lua().load("trace={};fail_at=nil;parent.messages.success=function(ctx) mux.world.object(13):set_destination(4);return {} end").exec().unwrap();
    let before = serde_json::to_value(&*s.world()).unwrap();
    let error = commands::execute(&s, &c, execution(1), "parityexit")
        .err()
        .unwrap();
    assert!(error.to_string().contains("Exit changed"), "{error}");
    assert_eq!(before, serde_json::to_value(&*s.world()).unwrap());
    assert!(s.outbox().is_empty());
    s.inspect_lua()
        .load("trace={};fail_at=nil;deny=true")
        .exec()
        .unwrap();
    commands::execute(&s, &c, execution(1), "parityexit").unwrap();
    assert!(
        s.inspect_lua()
            .load("return trace")
            .eval::<Vec<String>>()
            .unwrap()
            .is_empty()
    );
    assert_eq!(s.world().objects[&ObjectId(2)].location, Some(ObjectId(4)));
    c.logger.shutdown(&c).await.unwrap();
    assert!(
        std::fs::read(_d.path().join("logs/movement.log"))
            .unwrap()
            .is_empty()
    );
}

/// Generic relocation keeps occupied containers intact and rolls back bounded notification failures.
#[tokio::test(flavor = "current_thread")]
async fn generic_container_contexts_and_notification_limits() {
    let (d, c, mut s) = fixture().await;
    let cargo = {
        let mut w = s.world_mut();
        let id = w.create(&c, "Cargo".into(), Kind::Thing);
        let o = w.objects.get_mut(&id).unwrap();
        o.location = Some(ObjectId(4));
        o.lua_parent = "parity.lua".into();
        w.objects.get_mut(&ObjectId(2)).unwrap().location = Some(id);
        id
    };
    let parents = s
        .inspect_lua()
        .named_registry_value::<mlua::Table>("mux.parents")
        .unwrap();
    s.inspect_lua()
        .globals()
        .set("parent", parents.get::<mlua::Table>("parity.lua").unwrap())
        .unwrap();
    s.inspect_lua().globals().set("cargo", cargo.0).unwrap();
    s.inspect_lua()
        .load(
            r#"
      trace={};parent.events={};parent.messages={}
      for _,key in ipairs({'leave','enter_source','move','enter','leave_destination'}) do
        parent.messages[key]=function(ctx)
          assert(ctx.enactor==cargo and ctx.cause==1 and ctx.descriptor==nil)
          assert(ctx.source==4 and ctx.destination==0 and ctx.operation=='move')
          table.insert(trace,key)
          mux.world.object(cargo):state('parity'):set('last',key)
          return {other_message=key}
        end
      end
      parent.internal_appearance=function() error('things do not look') end
    "#,
        )
        .exec()
        .unwrap();
    let request = stompymux_rs::movement::Request {
        actor: ObjectId(1),
        object: cargo,
        cause: ObjectId(1),
        destination: ObjectId(0),
        session: Some(77),
        route: stompymux_rs::movement::Route::Generic,
    };
    stompymux_rs::movement::perform(&s, request).unwrap();
    assert_eq!(s.world().objects[&ObjectId(2)].location, Some(cargo));
    assert_eq!(s.world().objects[&cargo].location, Some(ObjectId(0)));
    assert_eq!(
        s.inspect_lua()
            .load("return table.concat(trace,',')")
            .eval::<String>()
            .unwrap(),
        "leave,enter_source,move,enter,leave_destination"
    );
    // A second observer makes neighbor routing exceed the configured traversal budget.
    {
        let mut w = s.world_mut();
        w.objects.get_mut(&cargo).unwrap().location = Some(ObjectId(4));
        w.objects.get_mut(&ObjectId(2)).unwrap().location = Some(ObjectId(4));
    }
    s.drain_outbox();
    let path = d.path().join("stompymux.toml");
    let source = std::fs::read_to_string(&path)
        .unwrap()
        .replace("[lua]", "[lua]\noutput_entry_limit = 1");
    std::fs::write(path, source).unwrap();
    let limited = Config::load(d.path()).unwrap();
    s.configure(&limited).unwrap();
    let before = serde_json::to_value(&*s.world()).unwrap();
    assert!(stompymux_rs::movement::perform(&s, request).is_err());
    assert_eq!(before, serde_json::to_value(&*s.world()).unwrap());
    assert!(s.outbox().is_empty());
    c.logger.shutdown(&c).await.unwrap();
}
