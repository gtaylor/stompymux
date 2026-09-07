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
    if let Action::Reply(text)
    | Action::Report(text)
    | Action::StyledReport(text)
    | Action::LiteralReport(text)
    | Action::CommitReply(text) = action
    {
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
        .filter(|d| matches!(d.handler, commands::CommandHandler::Native(_)))
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
            "@search",
            "@stats",
            "@state",
            "@examine",
            "@entrances",
            "@find",
            "@force",
            "@wait",
            "@halt",
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
                    object: None,
                    scripts: &s,
                    config: &c,
                    player: ObjectId(player),
                    session: Some(1),
                    cause: ObjectId(player),
                    origin: commands::InputOrigin::Interactive,
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
        Action::Reply(ref text) if text.contains("Unsupported @find switch")
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
        "local:2:value\nlocal:1:value\nfirst:value\nlast:value"
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
      {name='trap',permission='everyone',pattern='^@find',handler=function(ctx) mux.world.pemit(ctx.enactor,'Lua shadows native'); return true end}
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
    assert_eq!(run(&s, &c, 2, "f/unknown"), "Lua shadows native");
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

/// Queue commands use catalog permissions, aliases and literal argument handling.
#[tokio::test(flavor = "current_thread")]
async fn queue_registration_syntax_aliases_and_macro_restrictions() {
    use commands::queue::Request;
    let (d, _, mut w) = fixture().await;
    let aliases = d.path().join("aliases.toml");
    let contents = std::fs::read_to_string(&aliases).unwrap().replace(
        "[aliases.commands]",
        "[aliases.commands]\nfqueue='@force'\ncancelall='@halt/all'",
    );
    std::fs::write(aliases, contents).unwrap();
    let c = Config::load(d.path()).unwrap();
    w.objects.get_mut(&ObjectId(2)).unwrap().location = Some(ObjectId(c.start()));
    let ordinary = w.create(&c, "Ordinary".into(), stompymux_rs::world::Kind::Thing);
    w.objects.get_mut(&ordinary).unwrap().location = Some(ObjectId(c.start()));
    let s = Scripts::new(&c, Rc::new(RefCell::new(w))).unwrap();
    assert!(!s.commands.definitions().any(|d| d.name == "#"));
    for line in ["@force me=say no", "@wait 0=say no", "@halt", "@halt/all"] {
        assert!(run(&s, &c, ordinary.0, line).contains("Permission denied"));
    }
    assert!(run(&s, &c, 2, "@force #1=say no").contains("Permission denied"));
    assert!(
        matches!(commands::run(&s, &c, ObjectId(2), 1, &format!("FQUEUE #{}=say a;b", ordinary.0)).unwrap(), Action::Queue(Request::Add { executor, cause: ObjectId(2), seconds: 0, text }) if executor == ordinary && text == "say a;b")
    );
    assert!(
        matches!(commands::run(&s, &c, ObjectId(2), 1, "@wait -1={say a;say b}").unwrap(), Action::Queue(Request::Add { seconds: -1, text, .. }) if text == "say a;say b")
    );
    assert!(matches!(
        commands::run(&s, &c, ObjectId(2), 1, "cancelall").unwrap(),
        Action::Queue(Request::Halt { target: None })
    ));
    for line in [
        "@force/x me=say no",
        "@wait/x 0=say no",
        "@halt/no",
        "@halt/all me",
        "@wait nan=say no",
        "@wait 2147483648=say no",
        "@force",
        "@wait",
    ] {
        assert!(
            matches!(
                commands::run(&s, &c, ObjectId(2), 1, line).unwrap(),
                Action::Reply(_)
            ),
            "{line}"
        );
    }
    assert!(run(&s, &c, 2, "#2 say NO-SHORTHAND").contains("Huh?"));
    assert!(run(&s, &c, 2, ".create QueueMacros").contains("set"));
    run(&s, &c, 2, ".def frc=fqueue me=say no");
    run(&s, &c, 2, ".def wait=@wait 0=say no");
    let output = run(&s, &c, 2, ".frc");
    assert!(output.contains("unavailable as macro"), "{output}");
    assert!(run(&s, &c, 2, ".wait").contains("unavailable as macro"));
}

/// Background dispatch keeps native authority separate from Lua cause and never invents a descriptor.
#[tokio::test(flavor = "current_thread")]
async fn queued_context_locks_callbacks_and_session_rejection() {
    use commands::{ExecutionContext, InputOrigin};
    let (d, c, mut w) = fixture().await;
    let start = ObjectId(c.start());
    let thing = w.create(&c, "QueueThing".into(), stompymux_rs::world::Kind::Thing);
    w.objects.get_mut(&thing).unwrap().location = Some(start);
    w.objects.get_mut(&ObjectId(2)).unwrap().location = Some(start);
    w.objects
        .get_mut(&start)
        .unwrap()
        .flags
        .insert(Flag::Auditorium);
    w.objects.get_mut(&start).unwrap().lua_parent = "queue_room.lua".into();
    std::fs::write(
        d.path().join("lua/object_logic/queue_room.lua"),
        format!(
            r#"return {{locks={{speak=function(ctx)
      assert(ctx.enactor=={id} and ctx.subject=={id} and ctx.cause==1 and ctx.descriptor==nil)
      return true
    end}}}}"#,
            id = thing.0
        ),
    )
    .unwrap();
    std::fs::write(d.path().join("lua/global_logic/queue_context.lua"), format!(r#"return {{commands={{{{name="queue-context",permission="everyone",pattern="^queue%-context$",handler=function(ctx)
      assert(ctx.enactor=={id} and ctx.cause==1 and ctx.descriptor==nil)
      mux.world.object(ctx.enactor):state('queue'):set('context', true)
      mux.world.pemit(ctx.enactor, 'QUEUED-LUA')
      return true
    end}}}}}}"#, id=thing.0)).unwrap();
    let s = Scripts::new(&c, Rc::new(RefCell::new(w))).unwrap();
    let execution = ExecutionContext {
        executor: thing,
        cause: ObjectId(1),
        session: None,
        origin: InputOrigin::Queued,
    };
    assert!(matches!(
        commands::execute(&s, &c, execution, "queue-context").unwrap(),
        Action::Continue
    ));
    assert_eq!(
        s.world.borrow().objects[&thing].state["queue"]["context"],
        stompymux_rs::world::Scalar::Boolean(true)
    );
    assert!(matches!(
        commands::execute(&s, &c, execution, "say test").unwrap(),
        Action::Continue
    ));
    assert!(
        s.outbox
            .borrow()
            .iter()
            .any(|(_, d)| d.contains("says \"test\""))
    );
    // A GOD cause does not confer GOD or Wizard authority on the executor.
    assert!(
        matches!(commands::execute(&s, &c, execution, "@wait 0=say no").unwrap(), Action::Reply(ref text) if text.contains("Permission denied"))
    );
    let wizard = ExecutionContext {
        executor: ObjectId(2),
        ..execution
    };
    for line in [
        "quit",
        "color off",
        "help",
        "@session",
        "@telnet #2",
        "@lua/schedule",
        "@boot #2",
    ] {
        s.outbox.borrow_mut().clear();
        let result = commands::execute(&s, &c, wizard, line).unwrap();
        let mut text = s
            .outbox
            .borrow()
            .iter()
            .map(|(_, d)| d.source().to_owned())
            .collect::<Vec<_>>()
            .join("\n");
        if let Action::Reply(reply) = result {
            text.push_str(&reply);
        }
        assert!(
            text.contains("requires an interactive session"),
            "{line}: {text}"
        );
    }
    let context = s.context(Some(ObjectId(2)), None, Some(99)).unwrap();
    assert_eq!(context.get::<i64>("cause").unwrap(), 2);
    assert_eq!(context.get::<u64>("descriptor").unwrap(), 99);
}

/// Discovery uses captured registrations and the same source enumeration as dispatch.
#[tokio::test(flavor = "current_thread")]
async fn discovery_is_read_only_permission_filtered_and_scope_accurate() {
    let (d, c, mut w) = fixture().await;
    let global = d.path().join("lua/global_logic/discovery.lua");
    std::fs::write(&global,r#"return {commands={
      {name='catalog-public',permission='everyone',pattern='^public$',handler=function()error('must not execute')end},
      {name='catalog-secret',permission='god',pattern='^secret$',handler=function()error('must not execute')end}}}"#).unwrap();
    std::fs::write(d.path().join("lua/object_logic/discovery.lua"),r#"return {commands={
      {name='catalog-local',permission='wizard',pattern='^local1$',handler=function()error('must not execute')end},
      {name='catalog-local',permission='wizard',pattern='^local2$',handler=function()error('must not execute')end}}}"#).unwrap();
    w.objects.get_mut(&ObjectId(2)).unwrap().location = Some(ObjectId(4));
    let nearby = w.create(&c, "CatalogNearby".into(), stompymux_rs::world::Kind::Thing);
    let inventory = w.create(
        &c,
        "CatalogInventory".into(),
        stompymux_rs::world::Kind::Thing,
    );
    for (id, location) in [(nearby, ObjectId(4)), (inventory, ObjectId(2))] {
        let o = w.objects.get_mut(&id).unwrap();
        o.location = Some(location);
        o.lua_parent = "discovery.lua".into();
    }
    let s = Scripts::new(&c, Rc::new(RefCell::new(w))).unwrap();
    let before = serde_json::to_vec(&*s.world.borrow()).unwrap();
    let text = run(&s, &c, 2, "@list com");
    assert!(
        text.contains("Built-in commands (global native):")
            && text.contains("Global commands (global Lua):")
            && text.contains("Object commands:")
    );
    assert!(text.contains("catalog-public: everyone"));
    assert!(!text.contains("catalog-secret"));
    assert!(text.contains("CatalogNearby"));
    assert!(text.contains("CatalogInventory"));
    assert_eq!(text.matches("catalog-local: wizard").count(), 4);
    assert!(text.contains("declaration 1") && text.contains("declaration 2"));
    assert!(text.contains("aliases:") && text.contains("prefix: \""));
    assert!(run(&s, &c, 1, "@list commands").contains("catalog-secret: god"));
    let permissions = run(&s, &c, 2, "@list pe");
    assert!(permissions.contains("requires_session") && permissions.contains("no_macro"));
    let switches = run(&s, &c, 2, "@list sw");
    assert!(switches.contains("@clone: /inventory [wizard; min 3]"));
    assert!(!switches.contains("@find:") && !switches.contains("catalog-local"));
    for command in [
        "@list commands",
        "@list permissions",
        "@list switches",
        "@search",
        "@stats",
    ] {
        assert!(matches!(
            commands::run(&s, &c, ObjectId(2), 1, command).unwrap(),
            Action::LiteralReport(_)
        ));
    }
    assert_eq!(serde_json::to_vec(&*s.world.borrow()).unwrap(), before);
    assert!(s.outbox.borrow().is_empty());
    s.world
        .borrow_mut()
        .objects
        .get_mut(&nearby)
        .unwrap()
        .flags
        .insert(Flag::Halted);
    assert!(!run(&s, &c, 2, "@list commands").contains("CatalogNearby"));
    std::fs::write(&global, "return {}").unwrap();
    assert!(run(&s, &c, 2, "@list commands").contains("catalog-public"));
    let reload = Scripts::new(&c, s.world.clone()).unwrap();
    assert!(!run(&reload, &c, 2, "@list commands").contains("catalog-public"));
    assert!(run(&s, &c, 2, "@list site_information").contains("not implemented"));
    assert!(run(&s, &c, 2, "@stats extra").contains("Usage: @stats"));
}

/// Every implemented switch is discoverable with its actual spelling and role restriction.
#[test]
fn native_switch_catalog_fixture() {
    let registry = CommandRegistry::new();
    let mut rows = registry
        .definitions()
        .filter(|d| !d.switch_definitions.is_empty())
        .map(|d| {
            format!(
                "{} {}",
                d.name,
                d.switch_definitions
                    .iter()
                    .map(|s| {
                        assert!(s.accepts(s.name));
                        assert!(s.accepts(&s.name[..s.minimum]));
                        assert!(!s.accepts(&s.name[..s.minimum - 1]));
                        format!("{}:{}:{}", s.name, s.minimum, s.permission.name())
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        })
        .collect::<Vec<_>>();
    rows.sort();
    assert_eq!(
        rows.join("\n") + "\n",
        include_str!("fixtures/native-switches.txt")
    );
    for name in ["@find", "@search", "@stats"] {
        let d = registry.definitions().find(|d| d.name == name).unwrap();
        assert!(!d.requires_session && d.switch_definitions.is_empty());
    }
}

/// Portable sources and zone fallbacks share deterministic, deduplicated discovery.
#[tokio::test(flavor = "current_thread")]
async fn portable_dispatch_stages_and_inventory() {
    use stompymux_rs::world::Kind;
    let (d, c, mut w) = fixture().await;
    w.objects.get_mut(&ObjectId(2)).unwrap().location = Some(ObjectId(4));
    let item = w.create(&c, "[bold]Portable[/]".into(), Kind::Thing);
    let nested = w.create(&c, "Nested".into(), Kind::Thing);
    let zone = w.create(&c, "CommandZone".into(), Kind::Room);
    let zoned = w.create(&c, "ZoneSource".into(), Kind::Thing);
    for (id, location, module) in [
        (item, ObjectId(2), "portable.lua"),
        (nested, item, "portable.lua"),
        (zoned, zone, "zone_commands.lua"),
    ] {
        let o = w.objects.get_mut(&id).unwrap();
        o.location = Some(location);
        o.lua_parent = module.into();
        o.flags.remove(Flag::NoCommand);
    }
    w.objects.get_mut(&ObjectId(4)).unwrap().zone = Some(zone);
    std::fs::write(d.path().join("lua/object_logic/portable.lua"), r#"return {commands={
      {name='look',permission='everyone',pattern='^look$',handler=function(ctx) mux.world.pemit(ctx.enactor,'portable look'); return true end},
      {name='pass',permission='everyone',pattern='^pass$',handler=function(ctx) mux.world.pemit(ctx.enactor,'local false'); return false end}
    }}"#).unwrap();
    std::fs::write(d.path().join("lua/object_logic/zone_commands.lua"), r#"return {commands={{name='zoneprobe',permission='everyone',pattern='^zoneprobe$',handler=function(ctx) mux.world.pemit(ctx.enactor,'zone command');return true end}}}"#).unwrap();
    std::fs::write(d.path().join("lua/global_logic/portable_global.lua"), r#"return {commands={
      {name='pass',permission='everyone',pattern='^pass$',handler=function(ctx) mux.world.pemit(ctx.enactor,'global pass'); return true end},
      {name='inventory',permission='everyone',pattern='^shadow$',handler=function(ctx) mux.world.pemit(ctx.enactor,'global shadow'); return true end}
    }}"#).unwrap();
    let mut s = Scripts::new(&c, Rc::new(RefCell::new(w))).unwrap();
    assert_eq!(run(&s, &c, 2, "look"), "portable look");
    assert_eq!(run(&s, &c, 2, "zoneprobe"), "zone command");
    assert_eq!(run(&s, &c, 2, "pass"), "local false\nglobal pass");
    fn local(ctx: &CommandContext<'_>, _: &CommandInput) -> anyhow::Result<Action> {
        assert!(ctx.object.is_some());
        assert_eq!(ctx.cause, ObjectId(2));
        Ok(Action::Reply("local native".into()))
    }
    s.commands
        .register_native(CommandDefinition::object_native(
            "pass",
            P::EVERYONE,
            "portable.lua",
            local,
        ))
        .unwrap();
    assert_eq!(run(&s, &c, 2, "pass"), "local native");
    s.outbox.borrow_mut().clear();
    let listing = run(&s, &c, 2, "@list commands");
    assert!(listing.contains("local native / inventory"));
    assert!(listing.contains("location-zone fallback"));
    assert!(!listing.contains("Nested"));
    for flag in [Flag::Halted, Flag::NoCommand, Flag::Going] {
        s.world
            .borrow_mut()
            .objects
            .get_mut(&item)
            .unwrap()
            .flags
            .insert(flag);
        assert!(!run(&s, &c, 2, "look").contains("portable look"));
        s.world
            .borrow_mut()
            .objects
            .get_mut(&item)
            .unwrap()
            .flags
            .remove(flag);
    }
    let inv = run(&s, &c, 2, "inventory");
    assert!(inv.contains("[bold]Portable[/]") && inv.contains(&format!("(#{}", item.0)));
    assert!(!inv.contains("Nested"));
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    assert!(!run(&s, &c, 2, "inventory").contains(&format!("(#{}", item.0)));
    s.world
        .borrow_mut()
        .objects
        .get_mut(&item)
        .unwrap()
        .location = Some(ObjectId(0));
    assert!(!run(&s, &c, 2, "look").contains("portable look"));
    assert!(run(&s, &c, 2, "inventory").contains("You aren't carrying anything."));
}

/// Zone references do not recurse or duplicate a room-zone source, and exits win before Lua.
#[tokio::test(flavor = "current_thread")]
async fn zone_source_identity_and_exit_precedence() {
    use stompymux_rs::world::Kind;
    let (d, c, mut w) = fixture().await;
    w.objects.get_mut(&ObjectId(2)).unwrap().location = Some(ObjectId(4));
    let zone = w.create(&c, "Zone".into(), Kind::Room);
    let member = w.create(&c, "Member".into(), Kind::Thing);
    let player_zone = w.create(&c, "PlayerZone".into(), Kind::Thing);
    w.objects.get_mut(&member).unwrap().location = Some(zone);
    w.objects.get_mut(&ObjectId(4)).unwrap().zone = Some(zone);
    w.objects.get_mut(&ObjectId(2)).unwrap().zone = Some(zone);
    let sources = commands::sources::sources(&w, ObjectId(2));
    assert_eq!(sources[0].object, ObjectId(2));
    assert!(sources.iter().any(|s| s.object == member));
    assert!(!sources.iter().any(|s| s.object == zone));
    w.objects.get_mut(&ObjectId(2)).unwrap().zone = Some(player_zone);
    w.objects.get_mut(&player_zone).unwrap().zone = Some(player_zone);
    let sources = commands::sources::sources(&w, ObjectId(2));
    assert_eq!(sources.last().unwrap().object, player_zone);
    w.objects.get_mut(&ObjectId(4)).unwrap().zone = Some(player_zone);
    assert_eq!(
        commands::sources::sources(&w, ObjectId(2))
            .iter()
            .filter(|s| s.object == player_zone)
            .count(),
        1
    );
    let exit = w.create(&c, "look;testexit".into(), Kind::Exit);
    w.objects.get_mut(&exit).unwrap().location = Some(ObjectId(4));
    std::fs::write(d.path().join("lua/global_logic/exit_collision.lua"), "return {commands={{name='look',permission='everyone',pattern='^look$',handler=function() error('exit must win') end}}}").unwrap();
    let s = Scripts::new(&c, Rc::new(RefCell::new(w))).unwrap();
    assert!(run(&s, &c, 2, "look").contains("You can't go that way."));
    s.world
        .borrow_mut()
        .objects
        .get_mut(&exit)
        .unwrap()
        .location = Some(ObjectId(2));
    let inventory = run(&s, &c, 2, "inventory");
    assert!(inventory.contains("Exits:\nlook"));
    assert!(!inventory.contains("testexit"));
    assert!(run(&s, &c, 2, "testexit").contains("Huh?"));
}
