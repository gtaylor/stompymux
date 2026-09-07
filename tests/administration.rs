//! Runtime directives, effective Lua snapshots and C permission/parser compatibility.
use std::{cell::RefCell, path::Path, rc::Rc};
use stompymux_rs::{
    commands::{self, Action},
    config::{
        Config,
        administration::{Request, Support, support},
        directives::DIRECTIVES,
    },
    flags::Flag,
    lua::Scripts,
    persistence,
    world::ObjectId,
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
fn edit(
    c: &mut Config,
    s: &mut Scripts,
    who: i64,
    name: &str,
    value: &str,
) -> anyhow::Result<Vec<String>> {
    let candidate = c.administer(
        &Request {
            directive: name.into(),
            value: value.into(),
        },
        &s.world.borrow(),
        ObjectId(who),
        &s.commands,
    )?;
    s.configure(&candidate.config)?;
    *c = candidate.config;
    Ok(candidate.diagnostics)
}
fn run(s: &Scripts, c: &Config, who: i64, line: &str) -> String {
    let action = commands::run(s, c, ObjectId(who), 1, line).unwrap();
    let output = s
        .outbox
        .borrow_mut()
        .drain(..)
        .map(|(_, d)| d.source().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    match action {
        Action::Reply(v) | Action::LiteralReport(v) | Action::Report(v) => v,
        _ => output,
    }
}
#[tokio::test(flavor = "current_thread")]
async fn live_settings_permissions_reports_and_restart() {
    let (d, mut c, mut s) = fixture().await;
    let disk = std::fs::read(c.database()).unwrap();
    let file = std::fs::read(d.path().join("stompymux.toml")).unwrap();
    assert!(edit(&mut c, &mut s, 2, "max_players", "7").is_err());
    edit(
        &mut c,
        &mut s,
        1,
        "config_access",
        "max_players !god wizard",
    )
    .unwrap();
    edit(&mut c, &mut s, 2, "max_players", "7").unwrap();
    assert_eq!(c.mux.max_players, 7);
    assert_eq!(
        edit(&mut c, &mut s, 1, "mud_name", &"é".repeat(40)).unwrap(),
        ["String truncated"]
    );
    assert_eq!(c.server.mud_name.len(), 30);
    assert_eq!(
        s.eval_callback::<i64>("return mux.config.get('max_players')")
            .unwrap(),
        7
    );
    assert_eq!(
        s.eval_callback::<i64>("return mux.config.get('mux.max_players')")
            .unwrap(),
        7
    );
    assert!(run(&s, &c, 1, "@list config_permissions").contains("max_players: wizard (live)"));
    assert!(run(&s, &c, 1, "@list options").contains("sessions: 7"));
    assert!(edit(&mut c, &mut s, 1, "max_players", "3x").is_err());
    assert_eq!(c.mux.max_players, 7);
    edit(&mut c, &mut s, 1, "config_access", "port !disabled god").unwrap();
    assert!(
        edit(&mut c, &mut s, 1, "port", "1234")
            .unwrap_err()
            .to_string()
            .contains("restart-only")
    );
    assert!(
        edit(&mut c, &mut s, 1, "all_commands", "yes")
            .unwrap_err()
            .to_string()
            .contains("unsupported")
    );
    assert!(edit(&mut c, &mut s, 1, "MAX_PLAYERS", "1").is_err());
    edit(&mut c, &mut s, 1, "config_access", "max_players disabled").unwrap();
    assert!(edit(&mut c, &mut s, 1, "max_players", "1").is_err());
    assert_eq!(
        file,
        std::fs::read(d.path().join("stompymux.toml")).unwrap()
    );
    assert_eq!(disk, std::fs::read(c.database()).unwrap());
    assert_ne!(Config::load(d.path()).unwrap().mux.max_players, 7);
}
#[tokio::test(flavor = "current_thread")]
async fn edits_partial_success_aliases_and_live_handles() {
    let (_d, mut c, mut s) = fixture().await;
    let warnings = edit(&mut c, &mut s, 1, "access", "say wizard unknown").unwrap();
    assert_eq!(warnings.len(), 1);
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    assert_eq!(run(&s, &c, 2, "say nope"), "Permission denied.");
    edit(&mut c, &mut s, 1, "alias", "talk say").unwrap();
    assert_eq!(run(&s, &c, 2, "talk nope"), "Permission denied.");
    edit(&mut c, &mut s, 1, "access", "talk !wizard").unwrap();
    assert!(run(&s, &c, 2, "talk okay").contains("okay"));
    assert!(edit(&mut c, &mut s, 1, "alias", "talk look").is_err());
    edit(&mut c, &mut s, 1, "alias", "peek look/outside").unwrap();
    assert!(edit(&mut c, &mut s, 1, "alias", "bad look/nosuchswitch").is_err());
    edit(&mut c, &mut s, 1, "flag_alias", "shiny ansi").unwrap();
    assert!(edit(&mut c, &mut s, 1, "flag_alias", "ansi dark").is_err());
    assert_eq!(
        edit(&mut c, &mut s, 1, "default_thing_flags", "shiny missing")
            .unwrap()
            .len(),
        1
    );
    assert_eq!(c.mux.default_thing_flags, [Flag::Ansi]);
    let id = s.eval_callback::<i64>("return mux.world.create_object{type=mux.world.types.THING,name='Live defaults'}:dbref()").unwrap();
    assert!(
        s.world.borrow().objects[&ObjectId(id)]
            .flags
            .contains(Flag::Ansi)
    );
    assert!(edit(&mut c, &mut s, 1, "alias", "l say").is_err());
    edit(&mut c, &mut s, 1, "alias", "view l").unwrap();
    assert_eq!(c.aliases.commands["view"], "look");
    assert!(edit(&mut c, &mut s, 1, "default_thing_flags", "missing").is_err());
    assert!(run(&s, &c, 1, "@list default_flags").contains("Things: ANSI"));
    edit(&mut c, &mut s, 1, "bad_name", "Bad*").unwrap();
    assert!(stompymux_rs::accounts::validate_name("Baduser", &c).is_err());
    edit(&mut c, &mut s, 1, "good_name", "bad*").unwrap();
    assert!(stompymux_rs::accounts::validate_name("Baduser", &c).is_ok());
    s.eval_callback::<()>("held_state = mux.world.object(2):state('live_config')")
        .unwrap();
    edit(&mut c, &mut s, 1, "lua_state_value_limit", "2").unwrap();
    assert!(
        s.eval_callback::<()>("held_state:set('key','long')")
            .is_err()
    );
    edit(&mut c, &mut s, 1, "lua_state_value_limit", "100").unwrap();
    s.eval_callback::<()>("held_state:set('key','long')")
        .unwrap();
    edit(&mut c, &mut s, 1, "lua_state_value_limit", "2").unwrap();
    assert!(s.world.borrow().validate(&c).is_ok());
    assert!(
        s.eval_callback::<()>("held_state:set('key','longer')")
            .is_err()
    );
    assert!(s.world.borrow().validate(&c).is_ok());
    assert!(edit(&mut c, &mut s, 1, "default_thing_lua_parent", "missing.lua").is_err());
    let commands = s.commands.definitions().count();
    assert!(edit(&mut c, &mut s, 1, "access", "missing wizard").is_err());
    assert_eq!(commands, s.commands.definitions().count());
}
#[tokio::test(flavor = "current_thread")]
async fn sites_prepend_and_live_destinations() {
    let (_d, mut c, mut s) = fixture().await;
    edit(&mut c, &mut s, 1, "forbid_site", "127.0.0.0 255.0.0.0").unwrap();
    assert!(
        c.site_policy
            .classify("127.0.0.1".parse().unwrap())
            .forbidden
    );
    edit(
        &mut c,
        &mut s,
        1,
        "permit_site",
        "127.0.0.1,255.255.255.255",
    )
    .unwrap();
    edit(&mut c, &mut s, 1, "suspect_site", "127.0.0.0=255.0.0.0").unwrap();
    let state = c.site_policy.classify("127.0.0.1".parse().unwrap());
    assert!(!state.forbidden && state.suspect);
    edit(&mut c, &mut s, 1, "trust_site", "127.0.0.1 255.255.255.255").unwrap();
    assert!(!c.site_policy.classify("127.0.0.1".parse().unwrap()).suspect);
    assert!(edit(&mut c, &mut s, 1, "forbid_site", "::1 ::1").is_err());
    assert!(edit(&mut c, &mut s, 1, "default_home", "999999").is_err());
    let room =
        s.world
            .borrow_mut()
            .create(&c, "Runtime room".into(), stompymux_rs::world::Kind::Room);
    edit(
        &mut c,
        &mut s,
        1,
        "player_starting_room",
        &room.0.to_string(),
    )
    .unwrap();
    assert_eq!(c.start(), room.0);
}
#[test]
fn compiled_directive_catalog_and_file_access() {
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/config-directives.json")).unwrap();
    assert_eq!(rows.len(), DIRECTIVES.len());
    for (row, d) in rows.iter().zip(DIRECTIVES) {
        assert_eq!(row["name"], d.name);
        assert_eq!(row["parser"], d.parser);
        assert_eq!(
            row["permission"]
                .as_str()
                .unwrap()
                .trim_start_matches("CA_")
                .to_ascii_lowercase(),
            d.permission.name()
        );
        if support(d) == Support::Live {
            assert!(!d.name.starts_with("btech_"));
        }
    }
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("stompymux.toml");
    std::fs::write(&p, "[access.config]\nmax_players='!god wizard'\n").unwrap();
    let c = Config::load(d.path()).unwrap();
    c.validate_for_serve().unwrap();
    assert_eq!(c.directive_permissions["max_players"].name(), "wizard");
    std::fs::write(&p, "[access.config]\nnothing='god'\n").unwrap();
    let e = Config::load(d.path()).unwrap_err().to_string();
    assert!(
        e.contains("stompymux.toml") && e.contains("access.config.nothing"),
        "{e}"
    );
}
