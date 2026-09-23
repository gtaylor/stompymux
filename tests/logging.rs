//! Logging controls, conservative audit redaction, staged Lua effects and safe file appends.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::{
    Config, ObjectId, Scripts,
    commands::{self, Action, ExecutionContext, InputOrigin},
    logging::{self, Category, FileRequest, Record},
    persistence,
};
use support::{Client, isolated_world, stable_world, start};
async fn fixture() -> (tempfile::TempDir, Config, Scripts) {
    let (d, c, w) = isolated_world().await;
    std::fs::create_dir_all(d.path().join("logs")).unwrap();
    std::fs::write(d.path().join("logs/test.log"), "").unwrap();
    let s = Scripts::new(&c, Rc::new(RefCell::new(w))).unwrap();
    s.inspect_lua()
        .globals()
        .set(
            "_parents",
            s.inspect_lua()
                .named_registry_value::<mlua::Table>("mux.parents")
                .unwrap(),
        )
        .unwrap();
    s.inspect_lua()
        .globals()
        .set(
            "_object_parents",
            s.inspect_lua()
                .named_registry_value::<mlua::Table>("mux.object_parents")
                .unwrap(),
        )
        .unwrap();
    (d, c, s)
}
fn edit(c: &mut Config, s: &mut Scripts, name: &str, value: &str) -> Vec<String> {
    let request = stompymux_rs::config::administration::Request {
        directive: name.into(),
        value: value.into(),
    };
    let candidate = c
        .administer(&request, &s.world(), ObjectId(1), s.commands())
        .unwrap();
    s.configure(&candidate.config).unwrap();
    *c = candidate.config;
    candidate.diagnostics
}
#[tokio::test(flavor = "current_thread")]
async fn categories_live_controls_formatting_and_redaction() {
    let (_d, mut c, mut s) = fixture().await;
    assert!(!Category::AllCommands.enabled(&c));
    assert!(Category::Bugs.enabled(&c));
    edit(&mut c, &mut s, "all_commands", "yes");
    assert!(Category::AllCommands.enabled(&c));
    assert_eq!(
        edit(&mut c, &mut s, "log_options", "!timestamp flags unknown").len(),
        1
    );
    let r = Record::new(&c, "CMD", "ALL", "[bold]test[/]\nforged\x1b[31mRED\x1b[0m");
    assert!(r.text.contains("CMD/ALL"));
    assert!(!r.text.contains("[bold]") && !r.text.contains('\x1b'));
    assert_eq!(r.text.matches('\n').count(), 1);
    assert!(r.text.contains("\\nforged"));
    edit(&mut c, &mut s, "alias", "pw @newpassword");
    edit(&mut c, &mut s, "alias", "fi @find");
    {
        use stompymux_rs::macros::{MacroEntry, MacroSet, MacroSlots};
        let mut w = s.world_mut();
        w.macros.sets.push(MacroSet {
            id: Default::default(),
            origin: Default::default(),
            owner: ObjectId(1),
            modes: Default::default(),
            description: "audit".into(),
            entries: vec![
                MacroEntry {
                    origin: Default::default(),
                    alias: "pw".into(),
                    expansion: "@newpassword #2=secret".into(),
                },
                MacroEntry {
                    origin: Default::default(),
                    alias: "def".into(),
                    expansion: "say secret".into(),
                },
                MacroEntry {
                    origin: Default::default(),
                    alias: "safe".into(),
                    expansion: "say expanded text".into(),
                },
            ],
        });
        let index = w.macros.sets.len() - 1;
        w.macros.players.insert(
            ObjectId(1),
            MacroSlots {
                current: Some(index),
                slots: [Some(index), None, None, None, None],
            },
        );
    }
    for command in [
        ".pw",
        "@pcreate User=secret",
        "pw #2=secret",
        "@newpassword/anything #2=secret",
        "@wait 1=@pcreate User=secret",
        "@force me=@wait 1=pw #2=secret",
        ".def x=pw #2=secret",
    ] {
        let safe = logging::audit::safe_command(&c, &s.world(), ObjectId(1), command);
        assert!(!safe.contains("secret"), "{safe}");
        assert!(safe.contains("redacted"), "{safe}");
    }
    assert!(
        logging::audit::safe_command(&c, &s.world(), ObjectId(1), "say hello").contains("hello")
    );
    let message = logging::audit::message(
        &c,
        &s.world(),
        ExecutionContext {
            executor: ObjectId(1),
            cause: ObjectId(2),
            session: Some(9),
            origin: InputOrigin::Interactive,
        },
        "say hi",
    );
    assert!(message.contains("cause #2; session 9"));
    for original in ["@fi Foo", ".safe"] {
        let message = logging::audit::message(
            &c,
            &s.world(),
            ExecutionContext {
                executor: ObjectId(1),
                cause: ObjectId(1),
                session: Some(9),
                origin: InputOrigin::Interactive,
            },
            original,
        );
        assert!(
            message.contains(&format!("entered: '{original}'")),
            "{message}"
        );
        assert!(!message.contains("expanded text"), "{message}");
    }
    let queued = logging::audit::message(
        &c,
        &s.world(),
        ExecutionContext {
            executor: ObjectId(1),
            cause: ObjectId(1),
            session: None,
            origin: InputOrigin::Queued,
        },
        ".pw",
    );
    assert!(queued.contains(".pw [arguments redacted]"));
    assert!(!queued.contains("@newpassword"));
    assert!(logging::report(&c).contains("all_commands: enabled"));
    for name in ["logging", "logfiles"] {
        let action = commands::run(&s, &c, ObjectId(1), 1, &format!("@list {name}")).unwrap();
        assert!(matches!(
            action,
            Action::Report(commands::Report::Literal(_))
        ));
    }
    c.logger.shutdown(&c).await.unwrap();
}
#[tokio::test(flavor = "current_thread")]
async fn file_validation_appends_cache_and_failures() {
    let (d, c, _s) = fixture().await;
    let before = std::fs::read(c.database()).unwrap();
    for filename in ["", "../outside", "a..b", "a/b", "nul\0file"] {
        assert!(FileRequest::new(filename, "message").is_err());
    }
    assert!(FileRequest::new(&"x".repeat(201), "message").is_err());
    assert!(FileRequest::new("test.log", "nul\0message").is_err());
    c.logger
        .write(&c, FileRequest::new("test.log", "first").unwrap())
        .await
        .unwrap();
    c.logger
        .write(&c, FileRequest::new("test.log", "second").unwrap())
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(d.path().join("logs/test.log")).unwrap(),
        "first\nsecond\n"
    );
    assert!(c.logger.report().contains("test.log"));
    assert!(
        c.logger
            .write(&c, FileRequest::new("missing.log", "no").unwrap())
            .await
            .is_err()
    );
    assert!(!d.path().join("logs/missing.log").exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::{PermissionsExt, symlink};
        std::fs::write(d.path().join("outside"), "").unwrap();
        symlink(d.path().join("outside"), d.path().join("logs/link")).unwrap();
        assert!(
            c.logger
                .write(&c, FileRequest::new("link", "no").unwrap())
                .await
                .is_err()
        );
        assert_eq!(std::fs::read(d.path().join("outside")).unwrap(), b"");
        std::fs::write(d.path().join("logs/readonly"), "").unwrap();
        std::fs::set_permissions(
            d.path().join("logs/readonly"),
            std::fs::Permissions::from_mode(0o400),
        )
        .unwrap();
        assert!(
            c.logger
                .write(&c, FileRequest::new("readonly", "no").unwrap())
                .await
                .is_err()
        );
    }
    c.logger
        .write(&c, FileRequest::new("test.log", &"é".repeat(5000)).unwrap())
        .await
        .unwrap();
    let text = std::fs::read_to_string(d.path().join("logs/test.log")).unwrap();
    assert_eq!(text.lines().last().unwrap().len(), 4094);
    c.logger.shutdown(&c).await.unwrap();
    assert!(c.logger.report().contains("no open logfile"));
    assert_eq!(before, std::fs::read(c.database()).unwrap());
}
#[tokio::test(flavor = "current_thread")]
async fn lua_staging_nested_rollback_and_checking() {
    let (d, c, s) = fixture().await;
    assert!(
        s.inspect_lua()
            .load("return mux.log('test.log','outside')")
            .eval::<bool>()
            .is_err()
    );
    assert!(s.eval_callback::<bool>("return mux.log('','')").unwrap());
    assert!(
        !s.eval_callback::<bool>("return mux.log('../escape','no')")
            .unwrap()
    );
    assert!(
        s.eval_callback::<bool>("return mux.log('test.log','before commit')")
            .unwrap()
    );
    assert_eq!(std::fs::read(d.path().join("logs/test.log")).unwrap(), b"");
    s.rollback_effects_for_inspection();
    assert!(s.drain_logs_for_inspection().is_empty());
    assert!(
        s.eval_callback::<()>("assert(mux.log('test.log','discarded'));error('rollback')")
            .is_err()
    );
    assert!(s.drain_logs_for_inspection().is_empty());
    s.eval_callback::<()>("_parents['default_thing.lua'].locks={take=function(ctx) mux.log('test.log','nested discarded');error('nested') end}; local o=mux.world.create_object{type=mux.world.types.THING,name='logger',location=0,home=0}; mux.log('test.log','outer');mux.world.lock_passes{object=o,enactor=1,lock=mux.world.locks.TAKE};mux.log('test.log','last')").unwrap();
    for request in s.drain_logs_for_inspection() {
        c.logger.submit(&c, request);
    }
    c.logger
        .write(&c, FileRequest::new("test.log", "barrier").unwrap())
        .await
        .unwrap();
    let text = std::fs::read_to_string(d.path().join("logs/test.log")).unwrap();
    assert!(text.contains("outer\n") && text.contains("last\n"));
    assert!(!text.contains("discarded"));
    let checking = s
        .from_sources_for_inspection(
            &c,
            s.help().clone(),
            s.sources().clone(),
            stompymux_rs::lua::RuntimeMode::Checking,
        )
        .unwrap();
    assert!(
        checking
            .eval_callback::<bool>("return mux.log('test.log','check')")
            .unwrap_err()
            .to_string()
            .contains("unavailable.checking")
    );
    c.logger.shutdown(&c).await.unwrap();
}

use std::{cell::Cell, time::Duration};
use stompymux_rs::{Flag, ShutdownRequest, accounts};
/// Seed known credentials and a non-Wizard exclusively in the temporary database.
async fn credentials(c: &Config) {
    let mut world = persistence::load(&c.database()).await.unwrap();
    for id in [ObjectId(1), ObjectId(2)] {
        world.accounts.get_mut(&id).unwrap().hash = Some(accounts::hash("secret", c).unwrap());
    }
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    persistence::save(&c.database(), &world).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn tcp_log_commands_commit_and_rollback() {
    tokio::task::LocalSet::new().run_until(async {
 let (_d,c,_s)=fixture().await;
 let path=c.root.join("stompymux.toml");
 let mut doc:toml::Value=toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
 doc["security"]["password_hash_opslimit"]=toml::Value::Integer(1);
 doc["security"]["password_hash_memlimit"]=toml::Value::Integer(1048576);
 std::fs::write(&path,toml::to_string(&doc).unwrap()+"\n[aliases.commands]\n\"@lg\"=\"@log\"\n").unwrap();
 let c=Config::load(&c.root).unwrap();
 credentials(&c).await;
            let scripts=Scripts::new(&c,Rc::new(RefCell::new(persistence::load(&c.database()).await.unwrap()))).unwrap();
            scripts.communication(&c).create("SuspectsLog").unwrap();
            scripts.communication(&c).join(ObjectId(1),"SuspectsLog",true).unwrap();
            let world=scripts.world().clone();persistence::save(&c.database(),&world).await.unwrap();
 std::fs::write(c.lua_dir().join("global_logic/logtest.lua"),r#"return {commands={
 {name='logok',permission='everyone',pattern='^logok$',handler=function(ctx) assert(mux.log('test.log','lua committed'));return true end},
 {name='logmut',permission='everyone',pattern='^logmut$',handler=function(ctx) assert(mux.log('test.log','durable log'));mux.world.object(1):state('logtest'):set('written',1);return true end},
 {name='logfail',permission='everyone',pattern='^logfail$',handler=function(ctx) assert(mux.log('test.log','discarded'));error('rollback probe') end}
 }}"#).unwrap();
 let (address,shutdown,task,_)=start(&c,Rc::new(Cell::new(120))).await;
 let mut wizard=Client::connect(address,1).await;
 let mut player=Client::connect(address,2).await;
 let before=stable_world(&c.database()).await;
 player.send("@log test.log=forbidden").await;player.until("Permission denied.").await;
 wizard.send("@lg test.log=native").await;wizard.until("Message logged.").await;
 wizard.send("@log missing=no").await;wizard.until("Request failed.").await;
 wizard.send("@list logfiles").await;wizard.until("test.log").await;
 wizard.send("@list logging").await;wizard.until("all_commands").await;
 wizard.send("logok").await;
 wizard.send("logfail").await;
 wizard.send("@log test.log=barrier").await;wizard.until("Message logged.").await;
 assert_eq!(std::fs::read_to_string(c.root.join("logs/test.log")).unwrap(),"native\nlua committed\nbarrier\n");
 assert_eq!(before,stable_world(&c.database()).await);
 use sqlx::Connection;
 let mut db=sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(c.database()).foreign_keys(false)).await.unwrap();
 sqlx::raw_sql("CREATE TRIGGER reject_logs BEFORE INSERT ON object_state WHEN NEW.namespace='logtest' BEGIN SELECT RAISE(ABORT,'blocked'); END").execute(&mut db).await.unwrap();
 wizard.send("logmut").await;wizard.until("Unable to save your changes.").await;
 wizard.send("@log test.log=after failure").await;wizard.until("Message logged.").await;
 assert!(!std::fs::read_to_string(c.root.join("logs/test.log")).unwrap().contains("durable log"));
 sqlx::raw_sql("DROP TRIGGER reject_logs").execute(&mut db).await.unwrap();
 wizard.send("@flag #2=suspect").await;wizard.until("SUSPECT set.").await;
 player.send("@newpassword #2=hidden-secret").await;player.until("Permission denied.").await;
 let audit=wizard.until("[arguments redacted]").await;assert!(!audit.contains("hidden-secret"));
 let count=persistence::load(&c.database()).await.unwrap().channels["SuspectsLog"].messages;
 sqlx::raw_sql("CREATE TRIGGER reject_audits BEFORE UPDATE ON comsys_channels BEGIN SELECT RAISE(ABORT,'audit blocked'); END").execute(&mut db).await.unwrap();
 player.send("say audit failure does not block commands").await;player.until("audit failure does not block commands").await;
 assert_eq!(count,persistence::load(&c.database()).await.unwrap().channels["SuspectsLog"].messages);
 sqlx::raw_sql("DROP TRIGGER reject_audits").execute(&mut db).await.unwrap();
 wizard.send("@flag #2=going").await;wizard.until("GOING set.").await;
 let count=persistence::load(&c.database()).await.unwrap().channels["SuspectsLog"].messages;
 player.send("version").await;player.until("Attempt to execute command by halted object #2").await;
 assert_eq!(count,persistence::load(&c.database()).await.unwrap().channels["SuspectsLog"].messages);
 wizard.send("@flag #2=!going").await;wizard.until("GOING cleared.").await;
 db.close().await.unwrap();
 shutdown.send(ShutdownRequest::Sigterm).unwrap();
 tokio::time::timeout(Duration::from_secs(10),task).await.unwrap().unwrap().unwrap();
 assert!(c.logger.report().contains("no open logfile"));
 }).await;
}

/// Defaults are grounded in configuration_registry.c and server/log.c, not operator TOML.
#[test]
fn category_catalog_matches_compiled_c_fixture() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("stompymux.toml"), "").unwrap();
    let c = Config::load(d.path()).unwrap();
    let catalog: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/config/legacy-catalog.json")).unwrap();
    assert_eq!(logging::CATEGORIES.len(), 16);
    for (category, name, min) in logging::CATEGORIES {
        let row = catalog
            .iter()
            .find(|v| v["path"] == format!("logging.topics.{name}"))
            .unwrap();
        assert_eq!(
            category.enabled(&c),
            row["default"].as_bool().unwrap(),
            "{name}"
        );
        assert!(*min > 0 && *min <= name.len());
    }
}
