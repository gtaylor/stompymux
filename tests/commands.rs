//! Registry metadata, access checks, aliases and scoped Lua dispatch regressions.
use std::{cell::RefCell, path::Path, rc::Rc};
use stompymux_rs::{
    commands::{
        self, Action, CommandContext, CommandDefinition, CommandInput, CommandPermissions as P,
        CommandRegistry, CommandScope,
    },
    config::Config,
    flags::Flag,
    lua::Scripts,
    persistence,
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
    let w = persistence::load(&c.database()).await.unwrap();
    (d, c, w)
}
/// Execute through native matching and collect ordinary player-directed output.
fn run(s: &Scripts, c: &Config, player: i64, line: &str) -> String {
    let action = commands::run(s, c, ObjectId(player), 1, line).unwrap();
    if let Action::Reply(text) | Action::Report(text) = action {
        return text;
    }
    s.outbox
        .borrow_mut()
        .drain(..)
        .map(|(_, s)| s.source().to_string())
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
            "addcom",
            "delcom",
            "clearcom",
            "comlist",
            "allcom",
            "page",
            "@chan",
            "look",
            "say",
            "pose",
            ";",
            "\\",
            "@emit",
            "@pemit",
            "@npemit",
            "@oemit",
            "@fsay",
            "@fpose",
            "@femit",
            "@wall",
            "@session",
            "@telnet",
            "@shutdown",
            "@dbck",
            "color",
            "help",
            "@pcreate",
            "@newpassword",
            "@boot",
            "@last",
            "@lua",
            "@help",
            "quit",
            "home",
            "@teleport",
            "@flag",
            "@power",
            "@list",
            "@state",
            "@examine",
            "@entrances",
            "@find",
            ".add",
            ".clear",
            ".chmod",
            ".chown",
            ".create",
            ".def",
            ".del",
            ".name",
            ".chslot",
            ".ex",
            ".gex",
            ".glist",
            ".list",
            ".undef",
            "get",
            "drop",
            "give",
            "use",
            "enter",
            "leave",
            "inventory",
            "@create",
            "@dig",
            "@name",
            "@alias",
            "@description",
            "@internal-description",
            "@chzone",
            "@open",
            "@link",
            "@unlink",
            "@clone"
        ]
    );
    for d in &definitions {
        assert_eq!(
            d.permission,
            if [
                "look",
                "say",
                "pose",
                ";",
                "\\",
                "quit",
                "color",
                "help",
                "addcom",
                "delcom",
                "clearcom",
                "comlist",
                "allcom",
                "page",
                "get",
                "drop",
                "give",
                "use",
                "enter",
                "leave",
                "inventory"
            ]
            .contains(&d.name.as_str())
                || (d.direct_input_only && d.name != ".chown")
            {
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
    assert!(run(&s, &c, 2, "@EX #2").contains("Wizard(#2:"));
    assert!(run(&s, &c, 2, "@flag/unknown me=dark").contains("Unsupported command switch"));
    assert!(run(&s, &c, 2, "home/quiet").contains("Movement command switches"));
    assert!(matches!(
        commands::run(&s, &c, ObjectId(2), 1, "@FI/NeXt").unwrap(),
        Action::Find(_)
    ));
    assert!(run(&s, &c, 2, "l").contains("Staff Nexus"));
    assert!(run(&s, &c, 2, "\"hello/there").contains("You say \"hello/there\""));
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

/// Omitted and whitespace-only targets use the same matching as explicit `here`.
#[tokio::test(flavor = "current_thread")]
async fn examine_defaults_to_here() {
    let (_d, c, w) = fixture().await;
    let s = Scripts::new(&c, Rc::new(RefCell::new(w))).unwrap();
    let expected = run(&s, &c, 2, "@examine here");
    assert!(expected.contains("Staff Nexus"));
    for command in ["@examine", "@examine   ", "@EX"] {
        assert_eq!(run(&s, &c, 2, command), expected, "{command}");
    }
    assert!(run(&s, &c, 2, "@examine me").contains("Wizard(#2:"));
    assert!(run(&s, &c, 4, "@examine").contains("Permission denied."));
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

#[tokio::test(flavor = "current_thread")]
async fn account_command_validation_and_history() {
    use stompymux_rs::account_admin::{BootTarget, Request};
    use stompymux_rs::world::Login;
    let (_d, c, mut w) = fixture().await;
    let a = w.accounts.get_mut(&ObjectId(2)).unwrap();
    a.alias = Some("TestAlias".into());
    a.successes = 99;
    a.failures = 50;
    a.history = vec![
        Login {
            success: true,
            at: 1,
            host: "older".into(),
        },
        Login {
            success: false,
            at: 3,
            host: "failed".into(),
        },
        Login {
            success: true,
            at: 2,
            host: "newer".into(),
        },
    ];
    let s = Scripts::new(&c, Rc::new(RefCell::new(w))).unwrap();
    let before = std::fs::read(c.database()).unwrap();
    let history = run(&s, &c, 1, "@LAST tEsTaLiAs");
    assert!(history.contains("Total successful connects: 99"));
    assert!(history.contains("Total failed connects: 50"));
    assert!(history.find("newer").unwrap() < history.find("older").unwrap());
    assert!(history.contains("1970-01-01T00:00:02Z"));
    assert_eq!(std::fs::read(c.database()).unwrap(), before);
    assert_eq!(run(&s, &c, 1, "@last"), run(&s, &c, 1, "@last me"));
    for command in [
        "@pcreate/switch Person=secret",
        "@newpassword/switch #2=secret",
        "@last/quiet",
        "@boot/nonsense #2",
        "@boot//quiet #2",
        "@newpassword #2=",
        "@pcreate Person=",
        "@pcreate 123=secret",
        "@boot/port wrong",
    ] {
        assert!(
            matches!(
                commands::run(&s, &c, ObjectId(1), 1, command).unwrap(),
                Action::Reply(_)
            ),
            "{command}"
        );
    }
    assert!(matches!(
        commands::run(&s, &c, ObjectId(1), 1, "@boot/p/q 42").unwrap(),
        Action::AccountAdmin(Request::Boot {
            target: BootTarget::Session(42),
            quiet: true
        })
    ));
    assert!(matches!(
        commands::run(&s, &c, ObjectId(1), 1, "@newpassword TestAlias=secret").unwrap(),
        Action::AccountAdmin(Request::Reset {
            target: ObjectId(2),
            ..
        })
    ));
    assert!(run(&s, &c, 1, "@newpassword #1=secret").contains("You cannot change"));
}
