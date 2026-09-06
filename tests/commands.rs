//! Registry metadata, access checks, aliases and scoped Lua dispatch regressions.
use std::{cell::RefCell, path::Path, rc::Rc};
use stompymux_rs::{
    commands::{
        self, Action, CommandContext, CommandDefinition, CommandInput, CommandPermissions as P,
        CommandRegistry, CommandScope,
    },
    config::Config,
    flags::Flag,
    persistence,
    scripting::Scripts,
    world::{ObjectId, World},
};

/// Copy fixtures so metadata validation and script edits never touch production data.
fn copy(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        if entry.path().is_dir() {
            copy(&entry.path(), &target.join(entry.file_name()));
        } else {
            std::fs::copy(entry.path(), target.join(entry.file_name())).unwrap();
        }
    }
}
/// Load an isolated legacy world without starting a server.
async fn fixture() -> (tempfile::TempDir, Config, World) {
    let d = tempfile::tempdir().unwrap();
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"),
        d.path(),
    );
    let c = Config::load(d.path()).unwrap();
    let w = persistence::read_legacy(&c.legacy_database(), &c)
        .await
        .unwrap();
    (d, c, w)
}
/// Execute through native matching and collect ordinary player-directed output.
fn run(s: &Scripts, c: &Config, player: i64, line: &str) -> String {
    commands::run(s, c, ObjectId(player), 1, line).unwrap();
    s.outbox
        .borrow_mut()
        .drain(..)
        .map(|(_, s)| s)
        .collect::<Vec<_>>()
        .join("\n")
}
#[tokio::test(flavor = "current_thread")]
async fn native_catalog_permissions_and_aliases() {
    let (_d, c, w) = fixture().await;
    let s = Scripts::new(&c, Rc::new(RefCell::new(w))).unwrap();
    let definitions: Vec<_> = s
        .commands
        .definitions()
        .filter(|d| d.scope == CommandScope::Native)
        .collect();
    assert_eq!(
        definitions
            .iter()
            .map(|d| d.name.as_str())
            .collect::<Vec<_>>(),
        [
            "look",
            "say",
            "quit",
            "home",
            "@teleport",
            "@flag",
            "@power",
            "@list",
            "@examine",
            "@find"
        ]
    );
    for d in &definitions {
        assert_eq!(
            d.permission,
            if ["look", "say", "quit"].contains(&d.name.as_str()) {
                P::EVERYONE
            } else {
                P::WIZARD
            }
        );
    }
    for permission in [P::EVERYONE, P::WIZARD, P::GOD, P::GOD | P::WIZARD] {
        assert!(permission.allows(&s.world.borrow(), ObjectId(1)));
        assert_eq!(
            permission.allows(&s.world.borrow(), ObjectId(2)),
            permission == P::EVERYONE || permission == P::WIZARD
        );
        assert_eq!(
            permission.allows(&s.world.borrow(), ObjectId(4)),
            permission == P::EVERYONE
        );
    }
    let god = CommandDefinition::native("god-test", P::GOD, |_, _| Ok(Action::Quit));
    let input = CommandInput::parse(&c, "god-test");
    for (player, allowed) in [(1, true), (2, false), (4, false)] {
        let action = god
            .invoke_native(
                &CommandContext {
                    scripts: &s,
                    config: &c,
                    player: ObjectId(player),
                    session: 1,
                },
                &input,
            )
            .unwrap();
        assert_eq!(matches!(action, Action::Quit), allowed);
    }
    s.outbox.borrow_mut().clear();
    assert!(run(&s, &c, 2, "@EX #2").contains("Wizard(#2)"));
    assert!(run(&s, &c, 2, "@flag/unknown me=dark").contains("Unsupported command switch"));
    assert!(run(&s, &c, 2, "home/quiet").contains("Movement command switches"));
    assert!(matches!(
        commands::run(&s, &c, ObjectId(2), 1, "@FI/NeXt").unwrap(),
        Action::Find(_)
    ));
    assert!(run(&s, &c, 2, "l").contains("Staff Nexus"));
    assert!(run(&s, &c, 2, "\"hello/there").contains("You say, \"hello/there\""));
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    assert!(run(&s, &c, 2, "@EX/anything #2").contains("Permission denied."));
    assert!(matches!(
        commands::run(&s, &c, ObjectId(2), 1, "@find/next").unwrap(),
        Action::Reply(_)
    ));
}
#[test]
fn lua_metadata_is_required_and_patterns_are_validated() {
    let lua = mlua::Lua::new();
    for declaration in [
        "pattern='^x$',handler=function()end",
        "name='x',pattern='^x$',handler=function()end",
        "name=42,permission='everyone',pattern='x',handler=function()end",
        "name='bad/name',permission='everyone',pattern='x',handler=function()end",
        "name='x',permission='admin',pattern='x',handler=function()end",
        "name='x',permission='everyone',pattern='x'",
        "name='x',permission='everyone',pattern=1,handler=function()end",
        "name='x',permission='everyone',pattern='^never[',handler=function()end",
        "name='x',permission='everyone',pattern='^never%',handler=function()end",
        "name='x',permission='everyone',pattern='^never(',handler=function()end",
        "name='x',permission='everyone',pattern='%fabc',handler=function()end",
        "name='x',permission='everyone',pattern='%b(',handler=function()end",
        "name='x',permission='everyone',pattern='%1',handler=function()end",
    ] {
        let module = lua
            .load(format!("return {{commands={{{{{declaration}}}}}}}"))
            .eval::<mlua::Table>()
            .unwrap();
        let error = CommandRegistry::new()
            .register_lua(&lua, &module, "global_logic/bad.lua", CommandScope::Global)
            .err()
            .unwrap();
        let error = format!("{error:#}");
        assert!(error.contains("global_logic/bad.lua: command 1"), "{error}");
    }
    for pattern in [
        "^([%w_]+)%s+(.*)$",
        "%f[%a]word",
        "%b()",
        "(.)%1",
        "[]]",
        "[^]]",
        "()hello",
        "%%",
    ] {
        let module = lua.create_table().unwrap();
        let entries = lua.create_table().unwrap();
        let command = lua.create_table().unwrap();
        command.set("name", "TEST").unwrap();
        command.set("permission", "everyone").unwrap();
        command.set("pattern", pattern).unwrap();
        command
            .set(
                "handler",
                lua.load("return function() return true end")
                    .eval::<mlua::Function>()
                    .unwrap(),
            )
            .unwrap();
        entries.set(1, command).unwrap();
        module.set("commands", entries).unwrap();
        let mut registry = CommandRegistry::new();
        registry
            .register_lua(&lua, &module, "valid.lua", CommandScope::Global)
            .unwrap();
        assert_eq!(registry.definitions().last().unwrap().name, "test");
    }
}
#[tokio::test(flavor = "current_thread")]
async fn lua_scopes_permissions_captures_and_frozen_registration() {
    let (d, c, mut w) = fixture().await;
    // Both nearby players use the same module: instance order must be retained.
    for id in [1, 2] {
        let o = w.objects.get_mut(&ObjectId(id)).unwrap();
        o.lua_parent = "registry.lua".into();
        o.flags.remove(Flag::NoCommand);
        o.flags.remove(Flag::Halted);
    }
    std::fs::write(d.path().join("lua/object_logic/registry.lua"),r#"local module={commands={
      {name='probe',permission='god',pattern='^probe%s+(.*)$',handler=function(ctx,value) mux.world.pemit(ctx.enactor,'god:'..ctx.object..':'..value); return false end},
      {name='probe',permission='everyone',pattern='^probe%s+(.*)$',handler=function(ctx,value) mux.world.pemit(ctx.enactor,'local:'..ctx.object..':'..value); return false end}
    }}; _registry_module=module; return module"#).unwrap();
    for (file, label) in [("aa_registry.lua", "first"), ("zz_registry.lua", "last")] {
        std::fs::write(d.path().join("lua/global_logic").join(file),format!(r#"return {{commands={{{{name='probe',permission='everyone',pattern='^probe%s+(.*)$',handler=function(ctx,value) assert(ctx.scope=='global'); mux.world.pemit(ctx.enactor,'{label}:'..value); return {} end}}}}}}"#,label=="last")).unwrap();
    }
    let s = Scripts::new(&c, Rc::new(RefCell::new(w))).unwrap();
    let defs: Vec<_> = s
        .commands
        .definitions()
        .filter(|d| d.name == "probe")
        .collect();
    assert_eq!(defs.len(), 4);
    assert_eq!(defs[0].source, "object_logic/registry.lua");
    assert_eq!(defs[1].declaration, Some(2));
    assert_eq!(
        run(&s, &c, 2, "probe value"),
        "local:1:value\nlocal:2:value\nfirst:value\nlast:value"
    );
    assert_eq!(
        run(&s, &c, 1, "probe value"),
        "god:1:value\nlocal:1:value\ngod:2:value\nlocal:2:value\nfirst:value\nlast:value"
    );
    s.lua.load("_registry_module.commands[2].permission='god'; _registry_module.commands[2].handler=function() error('replaced') end; _registry_module.commands[2].pattern='never'").exec().unwrap();
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::NoCommand);
    assert_eq!(
        run(&s, &c, 2, "probe frozen"),
        "local:2:frozen\nfirst:frozen\nlast:frozen"
    );
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(Flag::Halted);
    assert_eq!(run(&s, &c, 2, "probe halted"), "first:halted\nlast:halted");
}
#[tokio::test(flavor = "current_thread")]
async fn lua_aliases_and_restricted_matches_fall_through_to_exits() {
    let (d, _c, w) = fixture().await;
    let path = d.path().join("stompymux.toml");
    let original = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        format!("{original}\n[aliases.commands]\np='probe'\n'p/special'='probe'\nf='@find'\n"),
    )
    .unwrap();
    let c = Config::load(d.path()).unwrap();
    std::fs::write(d.path().join("lua/global_logic/registry.lua"),r#"return {commands={
      {name='probe',permission='wizard',pattern='^probe%s+(.*)$',handler=function(ctx,value) mux.world.pemit(ctx.enactor,'wizard:'..value); return true end},
      {name='probe',permission='everyone',pattern='^probe%s+(.*)$',handler=function(ctx,value) mux.world.pemit(ctx.enactor,'everyone:'..value); return true end},
      {name='out',permission='god',pattern='^out$',handler=function() error('restricted handler ran') end},
      {name='trap',permission='everyone',pattern='^@find',handler=function() error('native fallback ran') end}
    }}"#).unwrap();
    let s = Scripts::new(&c, Rc::new(RefCell::new(w))).unwrap();
    assert_eq!(
        run(&s, &c, 2, "P/special Hello World"),
        "wizard:Hello World"
    );
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    assert_eq!(run(&s, &c, 2, "p Hello World"), "everyone:Hello World");
    assert!(run(&s, &c, 2, "PROBE Hello World").contains("Huh?"));
    assert!(matches!(
        commands::run(&s, &c, ObjectId(2), 1, "f/unknown").unwrap(),
        Action::Reply(_)
    ));
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .location = Some(ObjectId(4));
    assert!(run(&s, &c, 2, "out").contains("You cannot go that way."));
    // Verify the Lua callback was skipped and the ordinary exit lock was reached.
    assert_eq!(
        s.world.borrow().objects[&ObjectId(2)].location,
        Some(ObjectId(4))
    );
}
