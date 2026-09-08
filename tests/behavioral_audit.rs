//! C/Rust audit evidence: unresolved characterizations and resolved parity regressions.
//! C results are captured by the optional TCP probe; cargo test needs no C build.
use std::{cell::RefCell, path::Path, rc::Rc};
use stompymux_rs::{
    Config, Flag, Kind, ObjectId, Scripts, StateValue as Scalar,
    commands::{self, Action},
};

mod support;
use support::isolated_world;

/// A populated, isolated world with both accounts connected and no production writes.
async fn fixture() -> (tempfile::TempDir, Config, Scripts) {
    let (d, c, mut w) = isolated_world().await;
    std::fs::write(
        d.path().join("lua/object_logic/audit.lua"),
        r#"return {
      messages={describe=function(ctx)
        mux.world.object(ctx.object):state('audit'):set('provider',true)
        return {enactor_message='PROVIDER DESCRIPTION'}
      end}
    }"#,
    )
    .unwrap();
    std::fs::write(
        d.path().join("lua/object_logic/audit_room.lua"),
        r#"return {
      internal_appearance=function() return 'INTERNAL ROOM VIEW' end,
      external_appearance=function() return 'EXTERNAL ROOM VIEW' end
    }"#,
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
    let action = commands::run(s, c, ObjectId(1), 1, command).unwrap();
    let mut output = s
        .drain_outbox()
        .into_iter()
        .map(|(id, doc)| (id, doc.source().to_owned()))
        .collect::<Vec<_>>();
    if let Action::Report(commands::Report::Reply(text))
    | Action::CommitReply(text)
    | Action::Report(commands::Report::Inspection(text)) = action
    {
        output.push((ObjectId(1), text));
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

/// D06: ordinary matching follows C match_list word-prefix rules.
#[tokio::test(flavor = "current_thread")]
async fn ordinary_word_prefix_matches_c() {
    let (_d, c, s) = fixture().await;
    run(&s, &c, "@create Red Sword");
    assert!(text(&run(&s, &c, "look sWoRd"), 1).contains("Red Sword"));
    assert!(text(&run(&s, &c, "look red"), 1).contains("Red Sword"));
    assert!(text(&run(&s, &c, "look Red Sword"), 1).contains("Red Sword"));
    assert!(text(&run(&s, &c, "look *Sword*"), 1).contains("I don't see that here."));
    c.logger.shutdown(&c).await.unwrap();
}

/// D07: provider side effects happen in both servers, but C displays the stored description first.
#[tokio::test(flavor = "current_thread")]
async fn stored_description_overrides_provider_content() {
    let (_d, c, s) = fixture().await;
    run(&s, &c, "@create Red Sword");
    run(&s, &c, "@description Red Sword=STORED DESCRIPTION");
    run(&s, &c, "@lua/parent Red Sword=audit.lua");
    let output = text(&run(&s, &c, "look Red Sword"), 1);
    assert!(output.contains("STORED DESCRIPTION") && !output.contains("PROVIDER DESCRIPTION"));
    assert_eq!(
        s.world().objects[&ObjectId(16)].state["audit"]["provider"],
        Scalar::Boolean(true)
    );
    c.logger.shutdown(&c).await.unwrap();
}

/// D08: C chooses internal room appearance even when looking through an exit from elsewhere.
#[tokio::test(flavor = "current_thread")]
async fn remote_room_uses_internal_appearance() {
    let (_d, c, s) = fixture().await;
    {
        let mut w = s.world_mut();
        let room = w.create(&c, "Audit Room".into(), Kind::Room);
        w.objects.get_mut(&room).unwrap().lua_parent = "audit_room.lua".into();
        let exit = w.create(&c, "audit-window".into(), Kind::Exit);
        let e = w.objects.get_mut(&exit).unwrap();
        e.location = Some(ObjectId(0));
        e.destination = Some(room);
        e.flags.insert(Flag::Transparent);
    }
    let output = text(&run(&s, &c, "look audit-window"), 1);
    assert!(output.contains("INTERNAL ROOM VIEW") && !output.contains("EXTERNAL ROOM VIEW"));
    c.logger.shutdown(&c).await.unwrap();
}

/// D09 resolved: private pages never expand recipients through containment.
#[tokio::test(flavor = "current_thread")]
async fn private_page_excludes_contained_bystander() {
    let (_d, c, s) = fixture().await;
    s.world_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .location = Some(ObjectId(1));
    let output = run(&s, &c, "page GOD=PRIVATE-PAGE");
    assert!(text(&output, 1).contains("GOD pages: PRIVATE-PAGE"));
    assert!(text(&output, 2).is_empty());
    assert_eq!(s.world().last_pages[&ObjectId(1)], vec![ObjectId(1)]);
    c.logger.shutdown(&c).await.unwrap();
}

/// D10: recipient group formatting survives failed name resolution; history records deliveries.
#[tokio::test(flavor = "current_thread")]
async fn mixed_recipient_page_preserves_group_annotation() {
    let (_d, c, s) = fixture().await;
    for targets in [
        "Wizard MissingPlayer",
        "MissingPlayer Wizard",
        "#2 MissingPlayer",
    ] {
        for (message, expected, confirmation) in [
            (
                "hello",
                "To (Wizard), GOD pages you: hello",
                "You paged Wizard with 'hello'.",
            ),
            (
                "\"hello",
                "To (Wizard), GOD pages you: hello",
                "You paged Wizard with '\"hello'.",
            ),
            (
                ":waves",
                "From afar, to (Wizard): GOD waves",
                "Long distance to Wizard: GOD waves",
            ),
            (
                ";waves",
                "From afar, to (Wizard): GODwaves",
                "Long distance to Wizard: GODwaves",
            ),
        ] {
            let output = run(&s, &c, &format!("page {targets}={message}"));
            let sender = text(&output, 1);
            assert!(sender.contains("I don't recognize \"MissingPlayer\"."));
            assert!(sender.contains(confirmation), "{sender}");
            assert_eq!(text(&output, 2), expected);
            assert_eq!(s.world().last_pages[&ObjectId(1)], vec![ObjectId(2)]);
        }
    }
    // Reusing the successful list is a single-recipient page, not the previous request's mode.
    assert_eq!(text(&run(&s, &c, "page again"), 2), "GOD pages: again");
    s.world_mut().objects.get_mut(&ObjectId(2)).unwrap().name = "Long Player Name".into();
    s.world_mut().accounts.get_mut(&ObjectId(2)).unwrap().alias = Some("LP".into());
    for target in ["Long Player Name", "LP", "#2"] {
        let output = run(&s, &c, &format!("page {target}=single"));
        assert_eq!(text(&output, 2), "GOD pages: single");
        assert!(text(&output, 1).contains("You paged Long Player Name with 'single'."));
    }
    let output = run(&s, &c, "page LP MissingPlayer=alias");
    assert_eq!(
        text(&output, 2),
        "To (Long Player Name), GOD pages you: alias"
    );
    let output = run(&s, &c, "page LP #2=repeat");
    assert_eq!(
        text(&output, 2)
            .matches("To (Long Player Name, Long Player Name), GOD pages you: repeat")
            .count(),
        2
    );
    assert_eq!(
        s.world().last_pages[&ObjectId(1)],
        vec![ObjectId(2), ObjectId(2)]
    );
    assert_eq!(
        text(&run(&s, &c, "page again"), 2)
            .matches("To (Long Player Name, Long Player Name), GOD pages you: again")
            .count(),
        2
    );
    s.world_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Connected);
    let output = run(&s, &c, "page LP #1=offline");
    let sender = text(&output, 1);
    assert!(sender.contains("not connected"));
    assert!(sender.contains("To (Long Player Name, GOD), GOD pages you: offline"));
    assert!(sender.contains("You paged (Long Player Name, GOD) with 'offline'."));
    assert!(text(&output, 2).is_empty());
    assert_eq!(s.world().last_pages[&ObjectId(1)], vec![ObjectId(1)]);
    let output = run(&s, &c, "page MissingPlayer NobodyHere=none");
    assert!(!text(&output, 1).contains("You paged"));
    assert_eq!(s.world().last_pages[&ObjectId(1)], vec![ObjectId(1)]);
    c.logger.shutdown(&c).await.unwrap();
}

/// D11: the predicate requires a string and examines every byte without UTF-8 conversion.
#[tokio::test(flavor = "current_thread")]
async fn printable_ascii_requires_strings() {
    let (_d, c, s) = fixture().await;
    s.inspect_lua()
        .load(
            r#"
        for _, value in ipairs({123, 1.5, true, {}, function() end, mux.world.object(1)}) do
            local ok, problem = pcall(mux.text.is_printable_ascii, value)
            assert(not ok)
            assert(tostring(problem):find('string'))
        end
        assert(not pcall(mux.text.is_printable_ascii, nil))
        assert(not pcall(mux.text.is_printable_ascii))
        assert(mux.text.is_printable_ascii(''))
        assert(mux.text.is_printable_ascii('ASCII ~'))
        assert(not mux.text.is_printable_ascii('a\0b'))
        assert(not mux.text.is_printable_ascii('line\nfeed'))
        assert(not mux.text.is_printable_ascii('é'))
    "#,
        )
        .exec()
        .unwrap();
    let predicate = s
        .inspect_lua()
        .load("return mux.text.is_printable_ascii")
        .eval::<mlua::Function>()
        .unwrap();
    for byte in 0..=255u8 {
        let value = s.inspect_lua().create_string([byte]).unwrap();
        assert_eq!(
            predicate.call::<bool>(value).unwrap(),
            (0x20..=0x7e).contains(&byte)
        );
    }
    c.logger.shutdown(&c).await.unwrap();
}

/// D12 resolved: explicit movement and every configured alias share bare exit travel.
#[tokio::test(flavor = "current_thread")]
async fn goto_and_aliases_dispatch_exit_travel() {
    let (_d, c, s) = fixture().await;
    {
        let mut w = s.world_mut();
        let exit = w.create(&c, "audit-window".into(), Kind::Exit);
        let o = w.objects.get_mut(&exit).unwrap();
        o.location = Some(ObjectId(0));
        o.destination = Some(ObjectId(4));
        o.lua_parent.clear();
    }
    for verb in ["goto", "go", "got", "m", "mo", "mov", "move", "GoTo"] {
        s.world_mut()
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .location = Some(ObjectId(0));
        assert!(!text(&run(&s, &c, &format!("{verb} AUDIT-WINDOW")), 1).contains("Huh?"));
        assert_eq!(s.world().objects[&ObjectId(1)].location, Some(ObjectId(4)));
    }
    s.world_mut()
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .location = Some(ObjectId(0));
    run(&s, &c, "audit-window");
    assert_eq!(s.world().objects[&ObjectId(1)].location, Some(ObjectId(4)));
    c.logger.shutdown(&c).await.unwrap();
}

/// Keep captured evidence, stable finding IDs and source/test references consistent.
#[test]
fn audit_fixture_references_and_transcripts_are_consistent() {
    use serde_json::Value;
    use std::collections::BTreeSet;

    let audit: Value =
        serde_json::from_str(include_str!("fixtures/behavioral-audit-round2.json")).unwrap();
    let matrix: Value =
        serde_json::from_str(include_str!("fixtures/behavioral-parity.json")).unwrap();
    let wire: Value = serde_json::from_str(include_str!("fixtures/behavioral-wire.json")).unwrap();
    let rows = matrix["behaviors"].as_array().unwrap();
    let mut ids = BTreeSet::new();
    for row in rows {
        assert!(
            ids.insert(row["id"].as_str().unwrap()),
            "duplicate matrix ID"
        );
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tests = include_str!("behavioral_audit.rs");
    let findings = audit["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 7);
    for finding in findings {
        let id = finding["id"].as_str().unwrap();
        let row = rows.iter().find(|row| row["id"] == id).unwrap();
        assert_eq!(row["status"], finding["status"]);
        for field in [
            "severity",
            "setup",
            "command",
            "c_result",
            "rust_result",
            "effect",
            "recommendation",
            "evidence",
        ] {
            assert!(
                !finding[field].as_str().unwrap().is_empty(),
                "{id}: {field}"
            );
        }
        assert!(
            root.join(finding["rust_source"].as_str().unwrap())
                .is_file()
        );
        // The standalone Rust checkout need not contain the sibling C repository.
        if root.parent().unwrap().join("btmux-khi").is_dir() {
            assert!(
                root.parent()
                    .unwrap()
                    .join(finding["c_source"].as_str().unwrap())
                    .is_file()
            );
        }
        assert!(tests.contains(&format!(
                "async fn {}(",
                finding
                    .get("regression_test")
                    .unwrap_or(&finding["test"])
                    .as_str()
                    .unwrap()
            )));
        for engine in ["c", "rust"] {
            assert!(
                wire[engine]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|entry| entry["command"] == finding["command"])
            );
        }
    }
    let output = |engine: &str, command: &str, recipient: &str| -> &str {
        wire[engine]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["command"] == command)
            .unwrap()[recipient]
            .as_str()
            .unwrap()
    };
    for (engine, marker) in [("c", "Red Sword(#16)"), ("rust", "I don't see that here.")] {
        assert!(output(engine, "look sword", "sender").contains(marker));
    }
    for (engine, marker) in [
        ("c", "STORED DESCRIPTION"),
        ("rust", "PROVIDER DESCRIPTION"),
    ] {
        assert!(output(engine, "look Red Sword", "sender").contains(marker));
        assert!(output(engine, "@state/examine Red Sword/audit", "sender").contains("true"));
    }
    assert!(output("c", "look audit-window", "sender").contains("INTERNAL ROOM VIEW"));
    assert!(output("rust", "look audit-window", "sender").contains("EXTERNAL ROOM VIEW"));
    assert!(output("c", "page GOD=PRIVATE-PAGE", "observer").is_empty());
    assert!(
        output("rust", "page GOD=PRIVATE-PAGE", "observer").contains("GOD pages: PRIVATE-PAGE")
    );
    assert_eq!(
        output("c", "page Wizard MissingPlayer=hello", "observer"),
        "To (Wizard), GOD pages you: hello\n"
    );
    assert_eq!(
        output("rust", "page Wizard MissingPlayer=hello", "observer"),
        "GOD pages: hello\n"
    );
    assert!(output("c", "audit-ascii", "sender").contains("ASCII number accepted=false"));
    assert!(
        output("rust", "audit-ascii", "sender").contains("ASCII number accepted=true result=true")
    );
    assert!(output("c", "goto audit-window", "sender").contains("INTERNAL ROOM VIEW"));
    assert!(output("rust", "goto audit-window", "sender").contains("Huh?"));
    assert_eq!(
        output("c", "page Wizard=hello", "observer"),
        output("rust", "page Wizard=hello", "observer")
    );
    assert!(output("c", "zonegate", "sender").contains("Huh?"));
    assert!(output("rust", "zonegate", "sender").contains("Huh?"));
}

/// Preserve the original D09 failure while verifying the corrected paired result.
#[test]
fn d09_corrected_wire_matches_c_private_recipients() {
    let wire: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/behavioral-wire-d09-resolved.json")).unwrap();
    let page = |engine: &str| {
        wire[engine]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["command"] == "page GOD=PRIVATE-PAGE")
            .unwrap()
    };
    assert_eq!(page("c"), page("rust"));
    assert_eq!(page("rust")["observer"], "");
    assert!(
        page("rust")["sender"]
            .as_str()
            .unwrap()
            .contains("GOD pages: PRIVATE-PAGE")
    );
}

/// The correction transcript retains the original audit and demonstrates native/alias parity.
#[test]
fn resolved_goto_wire_matches_c() {
    let wire: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/behavioral-wire-d12-resolved.json")).unwrap();
    for command in ["goto audit-window", "go audit-window", "move audit-window"] {
        for engine in ["c", "rust"] {
            let entry = wire[engine]
                .as_array()
                .unwrap()
                .iter()
                .find(|entry| entry["command"] == command)
                .unwrap();
            assert_eq!(entry["sender"], "INTERNAL ROOM VIEW\n");
            assert_eq!(entry["observer"], "GOD has left.\n");
        }
    }
}

/// C notify_action reads live content after provider evaluation and before the event.
#[tokio::test(flavor = "current_thread")]
async fn description_provider_live_content_order_and_rollback() {
    let (_d, c, s) = fixture().await;
    run(&s, &c, "@create Red Sword");
    run(&s, &c, "@lua/parent Red Sword=audit.lua");
    let parents: mlua::Table = s.inspect_lua().named_registry_value("mux.parents").unwrap();
    s.inspect_lua()
        .globals()
        .set(
            "audit_parent",
            parents.get::<mlua::Table>("audit.lua").unwrap(),
        )
        .unwrap();
    for (stored, returned, expected) in [
        (None, None, "You see nothing special."),
        (Some(""), Some("PROVIDER"), "PROVIDER"),
        (Some("STORED"), Some("PROVIDER"), "STORED"),
        (Some("STORED"), None, "STORED"),
    ] {
        s.world_mut()
            .objects
            .get_mut(&ObjectId(16))
            .unwrap()
            .description = stored.map(str::to_owned);
        s.inspect_lua().globals().set("returned", returned).unwrap();
        s.inspect_lua()
            .load(
                r#"audit_parent.messages.describe=function(ctx)
          assert(ctx.object==16 and ctx.enactor==1 and ctx.descriptor==1)
          mux.world.pemit(ctx.enactor,'BEGIN CALLBACK')
          return {enactor_message=returned,other_message='NEIGHBOR'}
        end
        audit_parent.events={on_describe=function(ctx)
          mux.world.pemit(ctx.enactor,'EVENT END')
        end}"#,
            )
            .exec()
            .unwrap();
        let output = run(&s, &c, "look Red Sword");
        let actor = text(&output, 1);
        assert!(actor.find("BEGIN CALLBACK").unwrap() < actor.find(expected).unwrap());
        assert!(actor.find(expected).unwrap() < actor.find("EVENT END").unwrap());
        assert_eq!(actor.matches("EVENT END").count(), 1);
        assert!(text(&output, 2).contains("GOD NEIGHBOR"));
    }
    // Use Lua-facing description setters so mutations take the normal transaction path.
    for value in ["CHANGED", ""] {
        s.world_mut()
            .objects
            .get_mut(&ObjectId(16))
            .unwrap()
            .description = Some("BEFORE".into());
        s.inspect_lua().globals().set("changed", value).unwrap();
        s.inspect_lua()
            .load(
                r#"audit_parent.messages.describe=function(ctx)
          mux.world.object(ctx.object):set_description(changed)
          return {enactor_message='PROVIDER'}
        end"#,
            )
            .exec()
            .unwrap();
        let output = text(&run(&s, &c, "look Red Sword"), 1);
        assert!(output.contains(if value.is_empty() {
            "PROVIDER"
        } else {
            "CHANGED"
        }));
        assert!(!output.contains("BEFORE"));
    }
    s.inspect_lua()
        .load(r#"audit_parent.events.on_describe=function(ctx) error('DESCRIBE FAIL') end"#)
        .exec()
        .unwrap();
    let before = s.world().objects[&ObjectId(16)].description.clone();
    s.inspect_lua()
        .globals()
        .set("changed", "ROLLBACK")
        .unwrap();
    let output = text(&run(&s, &c, "look Red Sword"), 1);
    assert!(output.contains("DESCRIBE FAIL") && !output.contains("ROLLBACK"));
    assert_eq!(s.world().objects[&ObjectId(16)].description, before);
    c.logger.shutdown(&c).await.unwrap();
}

/// Remote-room fallback and defined-empty renderers differ; container mode remains local.
#[tokio::test(flavor = "current_thread")]
async fn room_fallback_and_container_internal_content() {
    let (_d, c, s) = fixture().await;
    let (room, container) = {
        let mut w = s.world_mut();
        let room = w.create(&c, "Remote".into(), Kind::Room);
        w.objects.get_mut(&room).unwrap().lua_parent = "audit_room.lua".into();
        w.objects.get_mut(&room).unwrap().description = Some("REMOTE DESCRIPTION".into());
        let exit = w.create(&c, "window".into(), Kind::Exit);
        let e = w.objects.get_mut(&exit).unwrap();
        e.location = Some(ObjectId(0));
        e.destination = Some(room);
        e.lua_parent.clear();
        e.flags.insert(Flag::Transparent);
        let container = w.create(&c, "Cabinet".into(), Kind::Thing);
        let o = w.objects.get_mut(&container).unwrap();
        o.location = Some(ObjectId(0));
        o.home = Some(ObjectId(0));
        o.lua_parent = "audit.lua".into();
        o.description = Some("OUTSIDE".into());
        o.internal_description = Some("INSIDE".into());
        (room, container)
    };
    let parents: mlua::Table = s.inspect_lua().named_registry_value("mux.parents").unwrap();
    s.inspect_lua()
        .globals()
        .set(
            "room_parent",
            parents.get::<mlua::Table>("audit_room.lua").unwrap(),
        )
        .unwrap();
    s.inspect_lua()
        .globals()
        .set(
            "container_parent",
            parents.get::<mlua::Table>("audit.lua").unwrap(),
        )
        .unwrap();
    s.inspect_lua()
        .load("room_parent.internal_appearance=nil")
        .exec()
        .unwrap();
    assert!(text(&run(&s, &c, "look window"), 1).contains("REMOTE DESCRIPTION"));
    s.inspect_lua()
        .load("room_parent.internal_appearance=function() return '' end")
        .exec()
        .unwrap();
    let output = text(&run(&s, &c, "look window"), 1);
    assert!(!output.contains("REMOTE DESCRIPTION") && !output.contains("EXTERNAL ROOM VIEW"));
    s.world_mut().objects.get_mut(&room).unwrap().lua_parent = "default_room.lua".into();
    assert!(text(&run(&s, &c, "look window"), 1).contains("REMOTE DESCRIPTION"));
    s.world_mut()
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .location = Some(container);
    s.inspect_lua()
        .load(
            r#"container_parent.messages.describe=function(ctx)
      assert(ctx.operation=='inside_describe')
      mux.world.object(ctx.object):set_internal_description('LIVE INSIDE')
      return {enactor_message='PROVIDER'}
    end"#,
        )
        .exec()
        .unwrap();
    assert!(text(&run(&s, &c, "look"), 1).contains("LIVE INSIDE"));
    s.world_mut()
        .objects
        .get_mut(&container)
        .unwrap()
        .internal_description = Some("".into());
    s.inspect_lua()
        .load(
            r#"container_parent.messages.describe=function(ctx)
      assert(ctx.operation=='describe')
      return {enactor_message='PROVIDER'}
    end"#,
        )
        .exec()
        .unwrap();
    assert!(text(&run(&s, &c, "look"), 1).contains("OUTSIDE"));
    c.logger.shutdown(&c).await.unwrap();
}

/// Preserve the original audit while checking the corrected paired TCP evidence.
#[test]
fn corrected_description_and_room_wire_evidence() {
    let wire: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/behavioral-wire-d07-d08-resolved.json"
    ))
    .unwrap();
    for engine in ["c", "rust"] {
        let entries = wire[engine].as_array().unwrap();
        for (command, expected, excluded) in [
            (
                "look Red Sword",
                "STORED DESCRIPTION",
                "PROVIDER DESCRIPTION",
            ),
            (
                "look audit-window",
                "INTERNAL ROOM VIEW",
                "EXTERNAL ROOM VIEW",
            ),
        ] {
            let row = entries.iter().find(|r| r["command"] == command).unwrap();
            let output = row["sender"].as_str().unwrap();
            assert!(output.contains(expected) && !output.contains(excluded));
        }
    }
}

/// Corrected paired evidence is separate from the immutable initial audit transcript.
#[test]
fn d10_d11_corrected_tcp_transcript_matches_c() {
    let wire: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/behavioral-wire-d10-d11-resolved.json"
    ))
    .unwrap();
    for command in [
        "audit-ascii",
        "page Wizard MissingPlayer=hello",
        "page Wizard=hello",
    ] {
        let response = |engine: &str| {
            wire[engine]
                .as_array()
                .unwrap()
                .iter()
                .find(|entry| entry["command"] == command)
                .unwrap()
        };
        assert_eq!(response("c"), response("rust"), "{command}");
    }
}
