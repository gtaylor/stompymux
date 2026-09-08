//! Live-runtime characterizations of unresolved lock/message parity findings.
use std::{cell::RefCell, path::Path, rc::Rc};
use stompymux_rs::{
    Config, Flag, Kind, ObjectId, Scripts,
    commands::{self, Action},
};

mod support;
use support::isolated_world;

/// A populated, isolated world with both accounts connected and no production writes.
async fn fixture() -> (tempfile::TempDir, Config, Scripts) {
    let (d, c, mut w) = isolated_world().await;
    std::fs::write(
        d.path().join("lua/object_logic/lock_message.lua"),
        r#"return {
        locks={take=function() return false end,
               use=function() return {passes=false,enactor_message='DENY USE'} end,
               give=function() return false end,receive=function() return false end},
        events={on_leave=function(ctx)
          assert(ctx.enactor==2 and ctx.subject==2 and ctx.cause==2 and ctx.scope=='object')
          mux.world.object(ctx.object):state('audit'):set('left',true)
        end}
    }"#,
    )
    .unwrap();
    std::fs::write(
        d.path().join("lua/object_logic/lock_message_order.lua"),
        r#"return {events={on_leave=function(ctx)
          local state=mux.world.object(1):state('leave_order')
          state:set('ids',state:get('ids','')..','..ctx.object)
        end}}"#,
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
        .map(|(id, doc)| (id, doc.source().to_owned()))
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

/// LM01: object members use AUDIBLE exits without leaking to their contents.
#[tokio::test(flavor = "current_thread")]
async fn channel_thing_routes_only_through_audible_exits() {
    let (_d, c, s) = fixture().await;
    run(&s, &c, "@create LockBox");
    run(&s, &c, "@chan/create Probe");
    s.communication(&c)
        .add(ObjectId(16), "Probe", "box", true, true)
        .unwrap();
    s.world_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .location = Some(ObjectId(16));
    {
        let mut w = s.world_mut();
        let id = w.create(&c, "ear".into(), Kind::Exit);
        let exit = w.objects.get_mut(&id).unwrap();
        exit.location = Some(ObjectId(16));
        exit.destination = Some(ObjectId(4));
        exit.flags.insert(Flag::Audible);
    }
    s.drain_outbox();
    s.communication(&c)
        .emit("Probe", "CHANNEL LEAK", false)
        .unwrap();
    let output = s
        .outbox()
        .iter()
        .map(|(id, d)| (*id, d.source().to_owned()))
        .collect::<Vec<_>>();
    assert!(text(&output, 2).is_empty());
    assert!(
        !s.world().channels["Probe"]
            .users
            .iter()
            .any(|u| u.who == ObjectId(2))
    );
    s.world_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .location = Some(ObjectId(4));
    s.drain_outbox();
    s.communication(&c)
        .emit("Probe", "AUDIBLE ROUTE", false)
        .unwrap();
    let output = s
        .outbox()
        .iter()
        .map(|(id, d)| (*id, d.source().to_owned()))
        .collect::<Vec<_>>();
    assert_eq!(text(&output, 2), "From a distance, [[Probe] AUDIBLE ROUTE");
    s.communication(&c)
        .add(ObjectId(2), "Probe", "p", true, true)
        .unwrap();
    s.world_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .location = Some(ObjectId(16));
    s.drain_outbox();
    s.communication(&c)
        .emit("Probe", "ONE COPY", false)
        .unwrap();
    assert_eq!(
        s.outbox()
            .iter()
            .filter(|(id, d)| *id == ObjectId(2) && d.source().contains("ONE COPY"))
            .count(),
        1
    );
    c.logger.shutdown(&c).await.unwrap();
}

/// LM02: every membership-removal command invokes loaded Live callbacks transactionally.
#[tokio::test(flavor = "current_thread")]
async fn channel_leave_callbacks_work_in_live_runtime() {
    let (_d, c, s) = fixture().await;
    run(&s, &c, "@create LockBox");
    run(&s, &c, "@lua/parent #16=lock_message.lua");
    run(&s, &c, "@chan/create Probe");
    s.communication(&c)
        .add(ObjectId(16), "Probe", "box", true, true)
        .unwrap();
    s.communication(&c)
        .add(ObjectId(2), "Probe", "p", true, true)
        .unwrap();
    let ordered = {
        let mut w = s.world_mut();
        (0..2)
            .map(|i| {
                let id = w.create(&c, format!("Ordered{i}"), Kind::Thing);
                w.objects.get_mut(&id).unwrap().lua_parent = "lock_message_order.lua".into();
                id
            })
            .collect::<Vec<_>>()
    };
    for object in &ordered {
        s.communication(&c)
            .add(*object, "Probe", &format!("o{}", object.0), true, true)
            .unwrap();
    }
    let assert_callback = |s: &Scripts| {
        assert!(s.world().objects[&ObjectId(16)].state.contains_key("audit"));
        s.world_mut()
            .objects
            .get_mut(&ObjectId(16))
            .unwrap()
            .state
            .remove("audit");
    };
    assert!(text(&run_as(&s, &c, ObjectId(2), "p off"), 2).contains("left channel Probe"));
    assert_callback(&s);
    assert_eq!(
        s.world().objects[&ObjectId(1)].state["leave_order"]["ids"],
        stompymux_rs::StateValue::String(
            format!(",{},{}", ordered[1].0, ordered[0].0).into_bytes()
        )
    );
    run_as(&s, &c, ObjectId(2), "p on");
    assert!(text(&run_as(&s, &c, ObjectId(2), "delcom p"), 2).contains("Channel Probe deleted"));
    assert_callback(&s);
    run_as(&s, &c, ObjectId(2), "addcom p=Probe");
    assert!(text(&run_as(&s, &c, ObjectId(2), "allcom off"), 2).contains("left channel Probe"));
    assert_callback(&s);
    run_as(&s, &c, ObjectId(2), "p on");
    run(&s, &c, "addcom g=Probe");
    assert!(
        text(&run(&s, &c, "@chan/boot Probe=Wizard"), 2).contains("You have left channel Probe")
    );
    assert_callback(&s);
    assert!(
        !s.world().channels["Probe"]
            .users
            .iter()
            .any(|u| u.who == ObjectId(2))
    );
    s.world_mut()
        .objects
        .get_mut(&ObjectId(16))
        .unwrap()
        .state
        .remove("audit");
    s.communication(&c).create("IndexZero").unwrap();
    s.communication(&c)
        .add(ObjectId(16), "IndexZero", "z", true, true)
        .unwrap();
    let newcomer = {
        let mut w = s.world_mut();
        let id = w.create(&c, "Newcomer".into(), Kind::Player);
        w.objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::Connected);
        id
    };
    s.communication(&c)
        .add(newcomer, "IndexZero", "n", true, true)
        .unwrap();
    s.communication(&c).leave(newcomer, "IndexZero").unwrap();
    assert!(!s.world().objects[&ObjectId(16)].state.contains_key("audit"));
    c.logger.shutdown(&c).await.unwrap();
}

/// LM03: lock denial and page paths forward through AUDIBLE exits only.
#[tokio::test(flavor = "current_thread")]
async fn direct_lock_messages_and_pages_use_audible_exits() {
    let (_d, c, s) = fixture().await;
    run(&s, &c, "@create LockBox");
    run(&s, &c, "@lua/parent #16=lock_message.lua");
    {
        let mut w = s.world_mut();
        w.objects.get_mut(&ObjectId(2)).unwrap().location = Some(ObjectId(4));
        let id = w.create(&c, "ear".into(), Kind::Exit);
        let exit = w.objects.get_mut(&id).unwrap();
        exit.location = Some(ObjectId(1));
        exit.destination = Some(ObjectId(4));
        exit.flags.insert(Flag::Audible);
        let bystander = w.create(&c, "Bystander".into(), Kind::Player);
        let bystander = w.objects.get_mut(&bystander).unwrap();
        bystander.location = Some(ObjectId(1));
        bystander.flags.insert(Flag::Connected);
    }
    assert!(text(&run(&s, &c, "@pemit GOD=CONTROL"), 2).contains("From a distance, CONTROL"));
    let output = run(&s, &c, "use LockBox");
    assert!(text(&output, 1).contains("DENY USE"));
    assert!(text(&output, 2).contains("From a distance, DENY USE"));
    assert!(text(&output, 18).is_empty());
    let output = run(&s, &c, "page GOD=ROUTED PAGE");
    assert!(text(&output, 1).contains("ROUTED PAGE"));
    assert!(text(&output, 2).contains("From a distance, GOD pages: ROUTED PAGE"));
    assert!(text(&output, 18).is_empty());
    c.logger.shutdown(&c).await.unwrap();
}

/// LM04: boolean denials and successful native confirmations retain C text.
#[tokio::test(flavor = "current_thread")]
async fn native_defaults_and_confirmations_match_c() {
    let (_d, c, s) = fixture().await;
    run(&s, &c, "@create LockBox");
    run(&s, &c, "@lua/parent #16=lock_message.lua");
    assert_eq!(
        text(&run(&s, &c, "give Wizard=LockBox"), 1),
        "You can't give LockBox away."
    );
    run(&s, &c, "@lua/parent #16=default_thing.lua");
    run(&s, &c, "@lua/parent #2=lock_message.lua");
    assert_eq!(
        text(&run(&s, &c, "give Wizard=LockBox"), 1),
        "Wizard doesn't want LockBox."
    );
    run(&s, &c, "@lua/parent #16=default_thing.lua");
    let output = run(&s, &c, "drop LockBox");
    assert!(text(&output, 2).contains("GOD dropped LockBox."));
    run(&s, &c, "@lua/parent #2=default_player.lua");
    run(&s, &c, "@lua/parent #16=lock_message.lua");
    assert_eq!(
        text(&run(&s, &c, "get LockBox"), 1),
        "You can't pick that up."
    );
    run(&s, &c, "@chan/create Probe");
    assert!(text(&run(&s, &c, "addcom p=Probe"), 1).contains("Channel Probe added with alias p."));
    run(&s, &c, "@flag LockBox=audible");
    assert_eq!(
        text(&run(&s, &c, "@chan/object Probe=#16"), 1),
        "Channel Probe is now using LockBox(#16:a) as channel object."
    );
    c.logger.shutdown(&c).await.unwrap();
}

/// Captured C evidence is optional to reproduce and required to stay consistent with this report.
#[test]
fn paired_wire_supports_findings_and_controls() {
    let audit: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/lock-message-audit.json")).unwrap();
    let matrix: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/behavioral-parity.json")).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(audit["findings"].as_array().unwrap().len(), 4);
    for finding in audit["findings"].as_array().unwrap() {
        assert!(
            root.join(finding["rust_source"].as_str().unwrap())
                .is_file()
        );
        assert!(matrix["behaviors"].as_array().unwrap().iter().any(|entry| {
            entry["id"] == finding["id"] && entry["status"] == "verified equivalent"
        }));
        assert!(
            include_str!("lock_message_audit.rs")
                .contains(&format!("async fn {}(", finding["test"].as_str().unwrap()))
        );
    }

    let wire: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/lock-message-wire.json")).unwrap();
    let entries = |engine: &str, command: &str| -> Vec<&serde_json::Value> {
        wire[engine]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["command"] == command)
            .collect()
    };
    let c = entries("c", "@chan/emit Probe=CHANNEL LEAK");
    let r = entries("rust", "@chan/emit Probe=CHANNEL LEAK");
    assert_eq!(c[0]["wizard"], "");
    assert_eq!(r[0]["wizard"], "[Probe] CHANNEL LEAK\n");
    assert!(
        entries("c", "p off")[0]["god"]
            .as_str()
            .unwrap()
            .contains("EVENT on_leave")
    );
    assert!(
        entries("rust", "p off")[0]["wizard"]
            .as_str()
            .unwrap()
            .contains("_parents")
    );
    assert!(
        entries("rust", "p on")[0]["wizard"]
            .as_str()
            .unwrap()
            .contains("already on")
    );
    assert!(entries("c", "use LockBox").iter().any(|e| {
        e["wizard"]
            .as_str()
            .unwrap()
            .contains("From a distance, DENY use")
    }));
    assert!(!entries("rust", "use LockBox").iter().any(|e| {
        e["wizard"]
            .as_str()
            .unwrap()
            .contains("From a distance, DENY use")
    }));
    assert_eq!(
        entries("c", "@pemit GOD=CONTROL")[0]["wizard"],
        entries("rust", "@pemit GOD=CONTROL")[0]["wizard"]
    );
    assert!(
        entries("c", "page GOD=ROUTED PAGE")[0]["wizard"]
            .as_str()
            .unwrap()
            .contains("From a distance, GOD pages")
    );
    assert_eq!(entries("rust", "page GOD=ROUTED PAGE")[0]["wizard"], "");
    for message in [
        "You can't give LockBox away.",
        "Wizard doesn't want LockBox.",
    ] {
        assert!(
            entries("c", "give Wizard=LockBox")
                .iter()
                .any(|e| e["god"].as_str().unwrap().contains(message))
        );
    }
    for command in [
        "p TEST",
        "p TEST DENIED LOCK OPEN FLAGS",
        "@chan/emit Probe=RECEIVE DENIED",
    ] {
        for who in ["god", "wizard"] {
            assert_eq!(
                entries("c", command)[0][who],
                entries("rust", command)[0][who]
            );
        }
    }

    let corrected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/lock-message-wire-corrected.json")).unwrap();
    let corrected_entries = |engine: &str, command: &str| -> Vec<&serde_json::Value> {
        corrected[engine]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["command"] == command)
            .collect()
    };
    assert_eq!(
        corrected_entries("c", "@chan/emit Probe=CHANNEL LEAK")[0]["wizard"],
        corrected_entries("rust", "@chan/emit Probe=CHANNEL LEAK")[0]["wizard"]
    );
    assert!(
        !corrected_entries("rust", "p off")[0]["wizard"]
            .as_str()
            .unwrap()
            .contains("_parents")
    );
    assert!(
        !corrected_entries("rust", "p on")[0]["wizard"]
            .as_str()
            .unwrap()
            .contains("already on")
    );
    for command in ["use LockBox", "page GOD=ROUTED PAGE"] {
        assert!(corrected_entries("rust", command).iter().any(|entry| {
            entry["wizard"]
                .as_str()
                .unwrap()
                .contains("From a distance,")
        }));
    }
    for message in [
        "You can't pick that up.",
        "You can't give LockBox away.",
        "Wizard doesn't want LockBox.",
        "GOD dropped LockBox.",
        "Channel Probe added with alias p.",
        "Channel Probe is now using LockBox(#16) as channel object.",
    ] {
        assert!(corrected["rust"].as_array().unwrap().iter().any(|entry| {
            entry["god"].as_str().unwrap().contains(message)
                || entry["wizard"].as_str().unwrap().contains(message)
        }));
    }
}
