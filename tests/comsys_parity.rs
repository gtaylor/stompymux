//! Executed C comsys wire fixtures, lifecycle ordering and relational preservation.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::{
    Config, Flag, Kind, ObjectId, Scripts,
    commands::{self, Action},
    persistence,
};

use crate::support;
use support::isolated_world;

/// A populated, isolated world with both accounts connected and no production writes.
async fn fixture() -> (tempfile::TempDir, Config, Scripts) {
    let (d, c, mut w) = isolated_world().await;
    std::fs::write(
        d.path().join("lua/global_logic/comsys_probe.lua"),
        r#"return {commands={
        {name='lua-add',permission='wizard',pattern='^lua%-add$',handler=function(ctx)
          mux.comsys.channel('Probe'):add_player(2,'lua',true); return true end},
        {name='lua-boot',permission='wizard',pattern='^lua%-boot$',handler=function(ctx)
          mux.comsys.channel('Probe'):boot_player(2); return true end}
    }}"#,
    )
    .unwrap();
    for id in [ObjectId(1), ObjectId(2)] {
        w.objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::Connected);
        w.objects.get_mut(&id).unwrap().location = Some(ObjectId(0));
    }
    let s = Scripts::new(&c, Rc::new(RefCell::new(w))).unwrap();
    (d, c, s)
}

fn run(s: &Scripts, c: &Config, command: &str) -> Vec<(ObjectId, String)> {
    run_as(s, c, ObjectId(1), command)
}
fn run_as(s: &Scripts, c: &Config, who: ObjectId, command: &str) -> Vec<(ObjectId, String)> {
    let action = commands::run(s, c, who, 1, command).unwrap();
    let mut output = s
        .drain_outbox()
        .into_iter()
        .map(|(id, doc)| {
            (
                id,
                stompymux_rs::text::plain_with(s.palette(), doc.source()),
            )
        })
        .collect::<Vec<_>>();
    if let Action::Report(commands::Report::Reply(text))
    | Action::CommitReply(text)
    | Action::Report(commands::Report::Inspection(text)) = action
    {
        output.push((who, text));
    }
    output
}
fn text(output: &[(ObjectId, String)], recipient: i64) -> String {
    output
        .iter()
        .filter(|(id, _)| id.0 == recipient)
        .map(|(_, s)| s.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Replay an executed C trace without requiring the C binary during normal Cargo tests.
#[tokio::test(flavor = "current_thread")]
async fn c_command_and_lua_transcript_matches() {
    let (_d, c, s) = fixture().await;
    let trace: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/comsys-wire.json")).unwrap();
    for row in trace["c"].as_array().unwrap() {
        let command = row["command"].as_str().unwrap();
        let output = run_as(&s, &c, ObjectId(row["actor"].as_i64().unwrap()), command);
        for (id, key) in [(1, "god"), (2, "wizard")] {
            let rendered = text(&output, id);
            let rendered = if rendered.is_empty() {
                rendered
            } else {
                rendered + "\n"
            };
            assert_eq!(rendered, row[key].as_str().unwrap(), "{command}: {key}");
        }
    }
    c.logger.shutdown(&c).await.unwrap();
}

/// Aliases are player-owned even after boot/destruction; their stale state survives restart.
#[tokio::test(flavor = "current_thread")]
async fn removal_and_destroy_preserve_other_aliases_across_restart() {
    let (_d, c, s) = fixture().await;
    run(&s, &c, "@chan/create Probe");
    run(&s, &c, "addcom a=Probe");
    run(&s, &c, "addcom b=Probe");
    run_as(&s, &c, ObjectId(2), "addcom p=Probe");
    let output = run(&s, &c, "delcom A");
    assert_eq!(
        text(&output, 1),
        "You have left channel Probe.\nChannel Probe deleted."
    );
    assert_eq!(text(&output, 2), "[Probe] GOD has left this channel.");
    assert_eq!(s.world().channel_aliases[&ObjectId(1)][0].alias, "b");
    assert_eq!(s.world().channels["Probe"].users.len(), 1);
    run(&s, &c, "@chan/destroy Probe");
    let snapshot = s.world().clone();
    persistence::save(&c.database(), &snapshot).await.unwrap();
    let restored = persistence::load(&c.database()).await.unwrap();
    assert!(!restored.channels.contains_key("Probe"));
    assert_eq!(restored.channel_aliases, snapshot.channel_aliases);
    *s.world_mut() = restored;
    s.communication(&c)
        .presence(ObjectId(1), true, Some("test"))
        .unwrap();
    assert!(s.outbox().iter().any(|(id, d)| {
        *id == ObjectId(1)
            && d.source()
                .contains("Bad Comsys Alias: b for Channel: Probe")
    }));
    s.drain_outbox();
    run(&s, &c, "@chan/create Probe");
    assert!(text(&run(&s, &c, "b hello"), 1).contains("not listed"));
    assert_eq!(text(&run(&s, &c, "delcom b"), 1), "Channel Probe deleted.");
    c.logger.shutdown(&c).await.unwrap();
}

/// Native allcom visits alias slots (not distinct channels), and presence shares that ordering.
#[tokio::test(flavor = "current_thread")]
async fn repeated_alias_presence_and_off_callbacks() {
    let (d, c, s) = fixture().await;
    std::fs::write(
        d.path().join("lua/object_logic/comsys_leave.lua"),
        r#"return {events={on_leave=function(ctx)
        local state=mux.world.object(1):state('comsys')
        state:set('calls',state:get('calls',0)+1)
    end}}"#,
    )
    .unwrap();
    let thing = s.world_mut().create(&c, "Listener".into(), Kind::Thing);
    s.world_mut().objects.get_mut(&thing).unwrap().lua_parent = "comsys_leave.lua".into();
    let s = s.rebuild_for_inspection(&c).unwrap();
    run(&s, &c, "@chan/create Probe");
    run(&s, &c, "addcom a=Probe");
    run(&s, &c, "addcom b=Probe");
    s.communication(&c)
        .add(thing, "Probe", "obj", true, true)
        .unwrap();
    run(&s, &c, "@chan/flags Probe=loud");
    s.drain_outbox();
    s.communication(&c)
        .presence(ObjectId(1), true, Some("test"))
        .unwrap();
    assert_eq!(
        s.outbox()
            .iter()
            .filter(|(id, d)| *id == ObjectId(1) && d.source().contains("GOD has connected."))
            .count(),
        2
    );
    s.drain_outbox();
    run(&s, &c, "allcom off");
    assert_eq!(
        s.inspect_lua()
            .load("return mux.world.object(1):state('comsys'):get('calls')")
            .eval::<i64>()
            .unwrap(),
        2
    );
    run(&s, &c, "clearcom");
    assert_eq!(
        s.inspect_lua()
            .load("return mux.world.object(1):state('comsys'):get('calls')")
            .eval::<i64>()
            .unwrap(),
        3
    );
    assert!(s.world().channel_aliases[&ObjectId(1)].is_empty());
    assert_eq!(s.world().channels["Probe"].users.len(), 1);
    c.logger.shutdown(&c).await.unwrap();
}

/// Receive locks see the incremented count, online insertion order and IC members before filtering.
#[tokio::test(flavor = "current_thread")]
async fn receive_lock_order_and_message_count() {
    let (d, c, s) = fixture().await;
    std::fs::write(d.path().join("lua/object_logic/comsys_receive.lua"), r#"return {locks={channel_receive=function(ctx)
      local state=mux.world.object(1):state('comsys')
      state:set('order',state:get('order','')..ctx.enactor..':'..mux.comsys.channel('Probe'):message_count()..',')
      return true
    end}}"#).unwrap();
    let thing = s.world_mut().create(&c, "Listener".into(), Kind::Thing);
    let policy = s.world_mut().create(&c, "Policy".into(), Kind::Thing);
    {
        let mut w = s.world_mut();
        w.objects.get_mut(&policy).unwrap().lua_parent = "comsys_receive.lua".into();
        w.objects
            .get_mut(&ObjectId(2))
            .unwrap()
            .flags
            .remove(Flag::Wizard);
        w.objects
            .get_mut(&ObjectId(2))
            .unwrap()
            .flags
            .insert(Flag::Gagged);
    }
    let s = s.rebuild_for_inspection(&c).unwrap();
    let service = s.communication(&c);
    service.create("Probe").unwrap();
    service.add(ObjectId(1), "Probe", "p", true, true).unwrap();
    service.add(thing, "Probe", "p", true, true).unwrap();
    service.add(ObjectId(2), "Probe", "p", true, true).unwrap();
    s.world_mut().channels.get_mut("Probe").unwrap().object = Some(policy);
    s.drain_outbox();
    run(&s, &c, "@chan/emit Probe=counter");
    assert_eq!(
        s.inspect_lua()
            .load("return mux.world.object(1):state('comsys'):get('order')")
            .eval::<String>()
            .unwrap(),
        format!("2:1,{}:1,", thing.0)
    );
    assert_eq!(s.world().channels["Probe"].history.len(), 1);
    // @chan/who follows membership slots, including interleaved player/object types.
    s.world_mut()
        .channels
        .get_mut("Probe")
        .unwrap()
        .users
        .swap(1, 2);
    let output = text(&run(&s, &c, "@chan/who Probe/all"), 1);
    assert!(output.find("Listener").unwrap() < output.find("Wizard").unwrap());
    c.logger.shutdown(&c).await.unwrap();
}

/// A Lua pcall must not retain half an alias join when confirmation exceeds the output budget.
#[tokio::test(flavor = "current_thread")]
async fn caught_lua_join_and_emit_failures_are_atomic() {
    let (_d, c, s) = fixture().await;
    run(&s, &c, "@chan/create Probe");
    run(&s, &c, "addcom p=Probe");
    s.drain_outbox();
    for _ in 0..c.lua.output_entry_limit - 1 {
        s.stage_message_for_inspection(ObjectId(1), "retained");
    }
    let before = serde_json::to_value(s.world().clone()).unwrap();
    s.inspect_lua().load("local ok=pcall(function() mux.comsys.channel('Probe'):add_player(2,'p',true) end); assert(not ok)").exec().unwrap();
    assert_eq!(serde_json::to_value(s.world().clone()).unwrap(), before);
    assert_eq!(s.outbox().len(), c.lua.output_entry_limit - 1);
    s.drain_outbox();
    let message = "x".repeat(c.runtime.output_message_limit);
    s.inspect_lua().globals().set("oversized", message).unwrap();
    s.inspect_lua()
        .load("assert(not pcall(function() mux.comsys.channel('Probe'):emit(oversized) end))")
        .exec()
        .unwrap();
    assert_eq!(serde_json::to_value(s.world().clone()).unwrap(), before);
    assert!(s.outbox().is_empty());
    c.logger.shutdown(&c).await.unwrap();
}

/// Administrative headers retain the invoked channel spelling; comlist retains description styles.
#[tokio::test(flavor = "current_thread")]
async fn styled_descriptions_and_emit_spelling() {
    let (_d, c, s) = fixture().await;
    run(&s, &c, "@chan/create Probe");
    run(&s, &c, "addcom p=Probe");
    assert_eq!(
        text(&run(&s, &c, "@chan/emit probe=message"), 1),
        "[probe] message"
    );
    let count = s.world().channels["Probe"].messages;
    assert!(run(&s, &c, "@chan/emit/noheader Probe=").is_empty());
    assert_eq!(s.world().channels["Probe"].messages, count + 1);
    s.world_mut().channels.get_mut("Probe").unwrap().object = Some(ObjectId(1));
    s.world_mut()
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .description = Some("[fg=red]Styled[/] description".into());
    commands::run(&s, &c, ObjectId(1), 1, "comlist").unwrap();
    {
        let output = s.outbox();
        let row = output
            .iter()
            .find(|(_, d)| d.source().contains("Styled"))
            .unwrap();
        assert_ne!(
            row.1.source(),
            stompymux_rs::text::plain_with(s.palette(), row.1.source())
        );
        assert!(
            stompymux_rs::text::plain_with(s.palette(), row.1.source())
                .contains("Styled description")
        );
    }
    c.logger.shutdown(&c).await.unwrap();
}

/// C history uses notify_checked, unlike live player channel delivery's raw_notify.
#[tokio::test(flavor = "current_thread")]
async fn history_replays_follow_audible_exits_without_contents_leaks() {
    let (_d, c, s) = fixture().await;
    run(&s, &c, "@chan/create Probe");
    run(&s, &c, "addcom p=Probe");
    {
        let mut w = s.world_mut();
        w.objects.get_mut(&ObjectId(2)).unwrap().location = Some(ObjectId(4));
        let exit = w.create(&c, "ear".into(), Kind::Exit);
        let exit = w.objects.get_mut(&exit).unwrap();
        exit.location = Some(ObjectId(1));
        exit.destination = Some(ObjectId(4));
        exit.flags.insert(Flag::Audible);
    }
    assert!(text(&run(&s, &c, "p history-route"), 2).is_empty());
    assert!(text(&run(&s, &c, "p last"), 2).contains("history-route"));
    s.world_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .location = Some(ObjectId(1));
    assert!(text(&run(&s, &c, "p last"), 2).is_empty());
    c.logger.shutdown(&c).await.unwrap();
}
