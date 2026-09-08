//! Effective access policy, reload compilation, source diagnostics and C catalog compatibility.
use std::{cell::RefCell, path::Path, rc::Rc};
use stompymux_rs::{
    access::Permissions as P,
    commands::{self, Action, CommandRegistry},
    config::Config,
    flags::Flag,
    lua::Scripts,
    persistence, server,
    world::{Kind, ObjectId},
};

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.path().is_dir() {
            copy(&entry.path(), &to.join(entry.file_name()));
        } else {
            std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
        }
    }
}
/// Add an ordered, isolated overlay to the existing game configuration.
fn game(policy: &str) -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"),
        d.path(),
    );
    std::fs::rename(d.path().join("stompymux.toml"), d.path().join("base.toml")).unwrap();
    std::fs::write(
        d.path().join("stompymux.toml"),
        format!("include=['base.toml']\n{policy}"),
    )
    .unwrap();
    d
}
async fn scripts(c: &Config) -> Scripts {
    let mut w = persistence::load(&c.database()).await.unwrap();
    w.objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    Scripts::new(c, Rc::new(RefCell::new(w))).unwrap()
}
fn run(s: &Scripts, c: &Config, who: i64, line: &str) -> String {
    let action = commands::run(s, c, ObjectId(who), 1, line).unwrap();
    let mut output = s
        .outbox
        .borrow_mut()
        .drain(..)
        .map(|(_, d)| d.source().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    match action {
        Action::Reply(t)
        | Action::LiteralReport(t)
        | Action::Report(t)
        | Action::CommitReply(t) => output.push_str(&t),
        _ => {}
    }
    output
}

#[tokio::test(flavor = "current_thread")]
async fn edits_alias_order_switches_lists_and_live_flags() {
    let d = game(
        r#"
[aliases.commands]
inspect="@list"
[access.commands]
"@list"="!wizard"
"say"="wizard"
"pose/nospace"="god"
"l"="dark"
"@find"="disabled"
";"="!dark"
[access.lists]
flags="disabled"
permissions="!wizard"
"site_information"="god !wizard"
"#,
    );
    let c = Config::load(d.path()).unwrap();
    let mut s = scripts(&c).await;
    assert!(run(&s, &c, 2, "say secret").contains("Permission denied"));
    assert!(run(&s, &c, 2, "pose/nospace").contains("Permission denied"));
    assert!(run(&s, &c, 2, "inspect flags").contains("Permission denied"));
    assert!(run(&s, &c, 1, "@list flags").contains("Permission denied"));
    assert!(run(&s, &c, 2, "@list si").contains("Permission denied"));
    let listing = run(&s, &c, 2, "inspect permissions");
    assert!(!listing.contains("  look:") && !listing.contains("  @find:"));
    assert!(
        listing.contains("  ;:") && listing.contains("  @list: everyone"),
        "{listing}"
    );
    assert!(run(&s, &c, 1, "@find").contains("Permission denied"));
    assert!(!run(&s, &c, 2, "look").contains("Permission denied"));
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(Flag::Wizard);
    assert!(!run(&s, &c, 2, "say allowed").contains("Permission denied"));
    assert!(run(&s, &c, 2, "@list si").contains("Permission denied"));
    assert!(run(&s, &c, 1, "@list si").contains("Site Access"));
    let before = s
        .commands
        .definitions()
        .map(|d| d.permission.name())
        .collect::<Vec<_>>();
    s.commands.configure_access(&c).unwrap();
    assert_eq!(
        before,
        s.commands
            .definitions()
            .map(|d| d.permission.name())
            .collect::<Vec<_>>()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn lua_scopes_and_runtime_prerequisites_are_checked_before_patterns() {
    let d = game(
        r#"[access.commands]
probe="no_suspect need_player queue_enabled"
"@flag"="!wizard"
"#,
    );
    for (path, answer) in [
        ("global_logic/access.lua", "global"),
        ("object_logic/access.lua", "local"),
    ] {
        std::fs::write(d.path().join("lua").join(path), format!(r#"return {{commands={{{{name='probe',permission='everyone',pattern='^probe$',handler=function(ctx) mux.world.pemit(ctx.enactor,'{answer}');return true end}}}}}}"#)).unwrap();
    }
    let c = Config::load(d.path()).unwrap();
    let s = scripts(&c).await;
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .lua_parent = "access.lua".into();
    assert_eq!(run(&s, &c, 2, "probe"), "local\nlocal");
    s.queue_enabled.set(false);
    assert!(run(&s, &c, 2, "probe").contains("Huh?"));
    s.queue_enabled.set(true);
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(Flag::Suspect);
    assert!(run(&s, &c, 2, "probe").contains("Huh?"));
    assert!(run(&s, &c, 2, "@flag #1=dark").contains("Permission denied"));
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(Flag::Wizard);
    assert_eq!(run(&s, &c, 2, "probe"), "local\nlocal");
    assert!(
        s.commands
            .definitions()
            .filter(|d| d.name == "probe")
            .all(|d| d.permission.contains(P::NO_SUSPECT))
    );
    assert_eq!(
        s.commands
            .definitions()
            .filter(|d| d.name == "probe")
            .count(),
        2
    );
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .kind = Kind::Thing;
    assert!(run(&s, &c, 2, "probe").contains("Huh?"));
}

#[tokio::test(flavor = "current_thread")]
async fn unresolved_policies_fail_before_startup_writes_and_invalid_tokens_are_contextual() {
    for policy in [
        "[access.commands]\nabsent='disabled'",
        "[access.commands]\n'look/missing'='god'",
        "[access.lists]\nmissing='god'",
    ] {
        let d = game(policy);
        let c = Config::load(d.path()).unwrap();
        let before = std::fs::read(c.database()).unwrap();
        let error = match server::prepare(&c).await {
            Ok(_) => panic!("invalid ACL accepted"),
            Err(e) => format!("{e:#}"),
        };
        assert!(
            error.contains("stompymux.toml") && error.contains("access."),
            "{error}"
        );
        assert_eq!(before, std::fs::read(c.database()).unwrap());
        std::fs::remove_file(c.database()).unwrap();
        assert!(server::prepare(&c).await.is_err());
        assert!(!c.database().exists() && !c.path(&c.database.bootstrap.credentials_file).exists());
    }
    let d = game("[access.commands]\nlook='not_a_permission'");
    let error = format!("{:#}", Config::load(d.path()).unwrap_err());
    assert!(error.contains("stompymux.toml") && error.contains("access.commands.look"));
}

/// Independent checked-in C defaults cover every mapped native and list registration.
#[test]
fn c_catalog_defaults() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/access-catalog.json")).unwrap();
    fn mask(raw: &str) -> P {
        raw.split('|').fold(P::EVERYONE, |value, part| {
            value
                | match part.trim() {
                    "CA_PUBLIC" | "0" | "CA_NO_IC" => P::EVERYONE,
                    "CA_GOD" => P::GOD,
                    "CA_WIZARD" | "CA_ADMIN" => P::WIZARD,
                    "CA_LOCATION" => P::LOCATION,
                    "CA_CONTENTS" => P::CONTENTS,
                    "CA_QUEUE" => P::QUEUE,
                    "CF_DARK" => P::DARK,
                    other => panic!("unknown C bit {other}"),
                }
        })
    }
    let registry = CommandRegistry::new();
    for d in registry.definitions() {
        if let Some(entry) = fixture["commands"].get(&d.name) {
            assert_eq!(
                d.permission,
                mask(entry["c_permissions"].as_str().unwrap()),
                "{}",
                d.name
            );
            for switch in &d.switch_definitions {
                let expected =
                    &fixture["switches"][entry["switch_table"].as_str().unwrap()][switch.name];
                assert_eq!(
                    switch.permission,
                    mask(expected["c_permissions"].as_str().unwrap()),
                    "{}/{}",
                    d.name,
                    switch.name
                );
                assert_eq!(
                    switch.minimum,
                    expected["minimum"].as_u64().unwrap() as usize
                );
            }
        } else {
            assert!(
                d.name == "home" || d.name == "color" || d.name.starts_with('.'),
                "unrecorded native {}",
                d.name
            );
        }
    }
    for list in &registry.lists {
        let expected = &fixture["lists"][list.name];
        assert_eq!(
            list.permission,
            mask(expected["c_permissions"].as_str().unwrap())
        );
        assert_eq!(list.minimum, expected["minimum"].as_u64().unwrap() as usize);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn acl_include_and_alias_edits_keep_declaration_order() {
    let d = game(
        r#"[aliases.commands]
z="look"
[access.commands]
z="dark"
look="!dark"
"#,
    );
    let c = Config::load(d.path()).unwrap();
    let s = scripts(&c).await;
    assert!(
        s.commands
            .definitions()
            .find(|d| d.name == "look")
            .unwrap()
            .listed
    );
    // Replacing the existing z edit retains its position before look through includes.
    std::fs::rename(
        d.path().join("stompymux.toml"),
        d.path().join("policy.toml"),
    )
    .unwrap();
    std::fs::write(
        d.path().join("stompymux.toml"),
        "include=['policy.toml']\n[access.commands]\nz='dark wizard'\n",
    )
    .unwrap();
    let c = Config::load(d.path()).unwrap();
    let s = scripts(&c).await;
    let entry = s.commands.definitions().find(|d| d.name == "look").unwrap();
    assert!(entry.listed && entry.permission.contains(P::WIZARD));
}
