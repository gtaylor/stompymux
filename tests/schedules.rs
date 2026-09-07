//! Deterministic cron, captured registration, scheduled transactions and live TCP regressions.
use sqlx::Connection;
use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::Rc,
    time::Duration,
};
use stompymux_rs::{
    ScheduleQueue as Queue, accounts,
    commands::{self, Action},
    config::Config,
    flags::Flag,
    lua::Scripts,
    persistence, schedule_jitter as jitter,
    server::{self, ShutdownRequest},
    world::{Kind, ObjectId, Scalar},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

/// Isolate modules, configuration and SQL from operator data.
fn copy(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let to = target.join(entry.file_name());
        if entry.path().is_dir() {
            copy(&entry.path(), &to)
        } else {
            std::fs::copy(entry.path(), to).unwrap();
        }
    }
}

/// Use fast test maintenance and password hashing, without changing production defaults.
async fn fixture() -> (tempfile::TempDir, Config) {
    let d = tempfile::tempdir().unwrap();
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"),
        d.path(),
    );
    let path = d.path().join("stompymux.toml");
    let mut doc: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    doc.as_table_mut()
        .unwrap()
        .entry("runtime")
        .or_insert(toml::Value::Table(Default::default()))
        .as_table_mut()
        .unwrap()
        .insert("maintenance_interval_ms".into(), toml::Value::Integer(20));
    doc["security"]["password_hash_opslimit"] = toml::Value::Integer(1);
    doc["security"]["password_hash_memlimit"] = toml::Value::Integer(1048576);
    doc.as_table_mut()
        .unwrap()
        .entry("aliases")
        .or_insert(toml::Value::Table(Default::default()))
        .as_table_mut()
        .unwrap()
        .entry("commands")
        .or_insert(toml::Value::Table(Default::default()))
        .as_table_mut()
        .unwrap()
        .insert(
            "@lsched".into(),
            toml::Value::String("@lua/schedule".into()),
        );
    std::fs::write(path, toml::to_string(&doc).unwrap()).unwrap();
    {
        let c = Config::load(d.path()).unwrap();
        (d, c)
    }
}

/// Write an isolated named game module.
fn module(c: &Config, path: &str, source: &str) {
    let path = c.lua_dir().join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, source).unwrap();
}

/// Load callbacks against the relational fixture.
async fn scripts(c: &Config) -> Scripts {
    Scripts::new(
        c,
        Rc::new(RefCell::new(
            persistence::load(&c.database()).await.unwrap(),
        )),
    )
    .unwrap()
}

#[tokio::test(flavor = "current_thread")]
async fn registration_rejects_malformed_declarations_and_captures_handlers() {
    let (_d, c) = fixture().await;
    for (source, fragment) in [
        ("return {schedules=true}", "array"),
        (
            "return {schedules={[2]={name='a',cron='* * * * *',handler=function()end}}}",
            "array",
        ),
        (
            "return {schedules={{cron='* * * * *',handler=function()end}}}",
            "declaration 1",
        ),
        (
            "return {schedules={{name=42,cron='* * * * *',handler=function()end}}}",
            "name",
        ),
        (
            "return {schedules={{name='',cron='* * * * *',handler=function()end}}}",
            "empty",
        ),
        (
            "return {schedules={{name='a',cron='*,bad * * * *',handler=function()end}}}",
            "cron",
        ),
        (
            "return {schedules={{name='a',cron='* * * * *',handler='not a function'}}}",
            "handler",
        ),
        (
            "return {schedules={{name='a',cron='* * * * *',handler=function()end},{name='a',cron='* * * * *',handler=function()end}}}",
            "duplicate",
        ),
    ] {
        module(&c, "global_logic/probe.lua", source);
        let result = Scripts::new(
            &c,
            Rc::new(RefCell::new(
                persistence::load(&c.database()).await.unwrap(),
            )),
        );
        let error = result.err().expect("invalid declaration").to_string();
        assert!(error.contains("global_logic/probe.lua"));
        // Full chains retain declaration and exact type/parser failure diagnostics.
        let result = Scripts::new(
            &c,
            Rc::new(RefCell::new(
                persistence::load(&c.database()).await.unwrap(),
            )),
        );
        assert!(
            format!("{:#}", result.err().unwrap()).contains(fragment),
            "{source}"
        );
    }
    module(
        &c,
        "global_logic/probe.lua",
        r#"local m={schedules={{name='tick',cron='* * * * *',handler=function(ctx) assert(ctx.scope=='global' and ctx.enactor==nil and ctx.cause==nil and ctx.object==nil and ctx.descriptor==nil and ctx.subject==nil and ctx.command==nil);assert(ctx.event=='schedule' and ctx.schedule=='tick' and ctx.cron=='* * * * *' and #ctx.args==0);mux.world.object(1):state('schedule'):set('captured',true) end}}}; schedule_module=m;return m"#,
    );
    let s = scripts(&c).await;
    assert!(!s.warnings.iter().any(|w| w.contains("schedules deferred")));
    s.lua.load("schedule_module.schedules[1].handler=function() error('replacement') end; schedule_module.schedules[1].cron='bad';schedule_module.schedules[1].name='changed'").exec().unwrap();
    let mut q = Queue::default();
    q.observe(&s.schedules, &s.world.borrow(), 120);
    assert!(q.take_due(174).is_none());
    q.observe(&s.schedules, &s.world.borrow(), 234);
    let job = q.take_due(234).unwrap();
    assert!(s.run_schedule(&job).unwrap());
    assert!(q.take_due(234).is_none());
    assert_eq!(
        s.world.borrow().objects[&ObjectId(1)].state["schedule"]["captured"],
        Scalar::Boolean(true)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn clock_jumps_expiry_objects_and_stable_order() {
    let (_d, c) = fixture().await;
    module(
        &c,
        "global_logic/a.lua",
        "return {schedules={{name='global',cron='* * * * *',handler=function() table.insert(order,'global') end}}}",
    );
    module(
        &c,
        "object_logic/z/test.lua",
        r#"return {schedules={{name='tick',cron='* * * * *',handler=function(ctx) assert(ctx.event=='schedule' and ctx.scope=='object' and ctx.enactor==1 and ctx.cause==1 and ctx.descriptor==nil and ctx.subject==nil and #ctx.args==0);table.insert(order,ctx.object) end}}}"#,
    );
    let s = scripts(&c).await;
    s.lua.load("order={}").exec().unwrap();
    {
        let mut w = s.world.borrow_mut();
        let o = w.objects.get_mut(&ObjectId(2)).unwrap();
        o.lua_parent = "z/test.lua".into();
        o.flags.insert(Flag::Halted);
        o.flags.insert(Flag::NoCommand);
    }
    let mut q = Queue::default();
    q.observe(&s.schedules, &s.world.borrow(), 120);
    q.observe(&s.schedules, &s.world.borrow(), 180);
    assert!(!q.ready(187));
    let global = q.take_due(188).unwrap();
    assert_eq!(global.due, 188);
    s.run_schedule(&global).unwrap();
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .lua_parent = "default_player.lua".into();
    let object = q.take_due(216).unwrap();
    s.run_schedule(&object).unwrap(); // retained original module after parent change
    assert_eq!(
        s.lua
            .load("return table.concat(order,',')")
            .eval::<String>()
            .unwrap(),
        "global,2"
    );
    q.observe(&s.schedules, &s.world.borrow(), 200);
    assert!(q.take_due(234).is_none());
    q.observe(&s.schedules, &s.world.borrow(), 60);
    q.observe(&s.schedules, &s.world.borrow(), 234);
    assert!(q.take_due(234).is_none());
    q.observe(&s.schedules, &s.world.borrow(), 420);
    assert!(q.take_due(480).is_none()); // queued current minute expired
    q.observe(&s.schedules, &s.world.borrow(), 594);
    assert_eq!(q.take_due(594).unwrap().expires, 600);
    assert!(q.take_due(594).is_none()); // no backfill
    let id = s
        .world
        .borrow_mut()
        .create(&c, "Scheduled".into(), Kind::Thing);
    s.world
        .borrow_mut()
        .objects
        .get_mut(&id)
        .unwrap()
        .lua_parent = "z/test.lua".into();
    q.observe(&s.schedules, &s.world.borrow(), 654);
    while let Some(job) = q.take_due(654) {
        if job.description().contains("object_logic/") {
            s.world
                .borrow_mut()
                .objects
                .get_mut(&id)
                .unwrap()
                .flags
                .insert(Flag::Going);
            assert!(!s.run_schedule(&job).unwrap());
        }
    }
    s.world
        .borrow_mut()
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .remove(Flag::Going);
    q.observe(&s.schedules, &s.world.borrow(), 714);
    while let Some(job) = q.take_due(714) {
        if job.description().contains("object_logic/") {
            s.world
                .borrow_mut()
                .objects
                .get_mut(&id)
                .unwrap()
                .generation = Default::default();
            assert!(!s.run_schedule(&job).unwrap());
        }
    }
    // Collection also excludes GOING and Garbage, without relying on execution checks.
    s.world
        .borrow_mut()
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::Going);
    q.observe(&s.schedules, &s.world.borrow(), 774);
    while let Some(job) = q.take_due(774) {
        assert!(!job.description().contains("object_logic/"));
    }
    s.world
        .borrow_mut()
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .remove(Flag::Going);
    q.observe(&s.schedules, &s.world.borrow(), 834);
    while let Some(job) = q.take_due(834) {
        if job.description().contains("object_logic/") {
            s.world.borrow_mut().objects.get_mut(&id).unwrap().kind = Kind::Garbage;
            assert!(!s.run_schedule(&job).unwrap());
        }
    }
    q.observe(&s.schedules, &s.world.borrow(), 894);
    while let Some(job) = q.take_due(894) {
        assert!(!job.description().contains("object_logic/"));
    }
    q.observe(&s.schedules, &s.world.borrow(), 900);
    q.clear();
    assert!(!q.ready(954));
}

#[tokio::test(flavor = "current_thread")]
async fn errors_rollback_state_and_output_and_do_not_retry() {
    let (_d, c) = fixture().await;
    module(
        &c,
        "global_logic/errors.lua",
        r#"return {schedules={
        {name='failure',cron='* * * * *',handler=function() mux.world.object(1):state('schedule'):set('leak',true);mux.world.pemit(1,'LEAK');error('injected') end},
        {name='budget',cron='* * * * *',handler=function() mux.world.object(1):state('schedule'):set('loop',true);while true do end end},
        {name='output',cron='* * * * *',handler=function() for i=1,1100 do mux.world.pemit(1,'LEAK') end end},
        {name='memory',cron='* * * * *',handler=function() mux.world.object(1):state('schedule'):set('heap',true);local big=string.rep('x',67108864);mux.world.pemit(1,big) end},
        {name='works',cron='* * * * *',handler=function() mux.world.object(1):state('schedule'):set('works',true) end}
    }}"#,
    );
    let s = scripts(&c).await;
    let mut q = Queue::default();
    q.observe(&s.schedules, &s.world.borrow(), 120);
    q.observe(&s.schedules, &s.world.borrow(), 234);
    let mut errors = 0;
    while let Some(job) = q.take_due(234) {
        if s.run_schedule(&job).is_err() {
            errors += 1;
        }
    }
    assert_eq!(errors, 4);
    assert!(s.outbox.borrow().is_empty());
    let w = s.world.borrow();
    assert_eq!(w.objects[&ObjectId(1)].state["schedule"].len(), 1);
    assert_eq!(
        w.objects[&ObjectId(1)].state["schedule"]["works"],
        Scalar::Boolean(true)
    );
    drop(w);
    q.observe(&s.schedules, &s.world.borrow(), 234);
    assert!(q.take_due(234).is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn inspection_is_captured_bounded_and_permission_checked() {
    let (_d, c) = fixture().await;
    module(
        &c,
        "object_logic/inspect.lua",
        r#"return {schedules={{name='escaped\027name',cron='* * * * *',handler=function()end}}}"#,
    );
    let s = scripts(&c).await;
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .lua_parent = "inspect.lua".into();
    let report = |target: &str, limit| {
        String::from_utf8(
            s.schedules
                .inspect(&s.world.borrow(), ObjectId(1), target, limit),
        )
        .unwrap()
    };
    assert!(report("", 65536).contains("object_logic/inspect.lua: 1 schedules (1 objects)"));
    assert!(report("inspect.lua", 65536).contains("Wizard (#2)"));
    assert!(report("#2", 65536).contains("escaped\\x1Bname: * * * * *"));
    assert!(!report("#2", 65536).contains("Objects:"));
    assert!(report("default_room.lua", 65536).contains("(none)"));
    for target in [
        "../inspect.lua",
        "/inspect.lua",
        "global_logic/../../inspect.lua",
        "missing.lua",
    ] {
        assert!(report(target, 65536).contains("unavailable"));
    }
    let small = report("inspect.lua", 50);
    assert!(small.len() <= 50);
    assert!(small.contains("truncated"));
    assert!(
        matches!(commands::run(&s,&c,ObjectId(1),1,"@lsched #2").unwrap(),Action::LuaSchedules(t) if t=="#2")
    );
    assert!(
        matches!(commands::run(&s,&c,ObjectId(1),1,"@lua").unwrap(),Action::Reply(t) if t.contains("/schedule"))
    );
    assert!(matches!(
        commands::run(&s, &c, ObjectId(1), 1, "@lua/reload").unwrap(),
        Action::LuaAdmin(stompymux_rs::lua::AdminRequest::Reload)
    ));
    s.world
        .borrow_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    assert!(
        matches!(commands::run(&s,&c,ObjectId(2),1,"@lua/schedule").unwrap(),Action::Reply(t) if t=="Permission denied.")
    );
}

/// A real socket client that retains unread response bytes across assertions.
struct Client {
    socket: TcpStream,
    pending: Vec<u8>,
}

impl Client {
    async fn connect(address: std::net::SocketAddr, player: i64) -> Self {
        let mut client = Self {
            socket: TcpStream::connect(address).await.unwrap(),
            pending: Vec::new(),
        };
        client.until("Who are you? ").await;
        client.send(&format!("#{player}")).await;
        client.until("Password: ").await;
        client.send("secret").await;
        client.until("Staff Nexus").await;
        client
    }

    async fn send(&mut self, text: &str) {
        self.socket
            .write_all(format!("{text}\r\n").as_bytes())
            .await
            .unwrap();
    }

    async fn until(&mut self, needle: &str) -> String {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Some(index) = self
                    .pending
                    .windows(needle.len())
                    .position(|b| b == needle.as_bytes())
                {
                    let bytes = self
                        .pending
                        .drain(..index + needle.len())
                        .collect::<Vec<_>>();
                    return String::from_utf8_lossy(&bytes).into_owned();
                }
                let mut bytes = [0; 8192];
                let count = self.socket.read(&mut bytes).await.unwrap();
                assert!(
                    count > 0,
                    "closed waiting for {needle}: {:?}",
                    String::from_utf8_lossy(&self.pending)
                );
                self.pending.extend_from_slice(&bytes[..count]);
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "timeout waiting for {needle}: {:?}",
                String::from_utf8_lossy(&self.pending)
            )
        })
    }
}

/// Keep the injected clock out of protocol/configuration; only the embedded test owner controls it.
async fn start(
    c: &Config,
    clock: Rc<Cell<i64>>,
) -> (
    std::net::SocketAddr,
    oneshot::Sender<ShutdownRequest>,
    tokio::task::JoinHandle<anyhow::Result<()>>,
    mlua::Lua,
) {
    let scripts = server::prepare(c).await.unwrap();
    let vm = scripts.lua.clone();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel();
    let config = c.clone();
    let task = tokio::task::spawn_local(async move {
        server::run_with_schedule_clock(
            config,
            scripts,
            listener,
            async { rx.await.unwrap_or(ShutdownRequest::Sigterm) },
            move || clock.get(),
        )
        .await
    });
    (address, tx, task, vm)
}

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
async fn tcp_schedules_commit_before_output_private_inspection_restart_and_shutdown() {
    tokio::task::LocalSet::new().run_until(async {
        let (_d,c)=fixture().await;credentials(&c).await;
        module(&c,"global_logic/tcp.lua",r#"return {schedules={{name='delivery',cron='* * * * *',handler=function(ctx)
            local object=mux.world.object(1);assert(object:flags():has(mux.world.flags.CONNECTED))
            local state=object:state('scheduled');local count=state:get('count',0)+1;state:set('count',count)
            mux.world.pemit(1,'Scheduled delivery '..count)
        end}}}"#);
        let clock=Rc::new(Cell::new(120));let (address,shutdown,task,_vm)=start(&c,clock.clone()).await;
        let mut first=Client::connect(address,1).await;let mut second=Client::connect(address,1).await;let mut ordinary=Client::connect(address,2).await;
        first.send("@lsched global_logic/tcp.lua").await;first.until("delivery: * * * * *").await;
        ordinary.send("@lua/schedule").await;ordinary.until("Permission denied.").await;
        // A read-only inspection must not even update snapshot metadata.
        let before=std::fs::read(c.database()).unwrap();first.send("@lua/schedule #1").await;first.until("(none)").await;assert_eq!(before,std::fs::read(c.database()).unwrap());
        clock.set(234);first.until("Scheduled delivery 1").await;let other=second.until("Scheduled delivery 1").await;assert!(!other.contains("Schedules for"));
        assert_eq!(persistence::load(&c.database()).await.unwrap().objects[&ObjectId(1)].state["scheduled"]["count"],Scalar::Integer(1));
        first.send("say still responsive").await;first.until("still responsive").await;
        shutdown.send(ShutdownRequest::Sigterm).unwrap();tokio::time::timeout(Duration::from_secs(5),task).await.unwrap().unwrap().unwrap();
        assert!(TcpStream::connect(address).await.is_err());
        let saved=persistence::load(&c.database()).await.unwrap();assert!(!saved.objects[&ObjectId(1)].flags.contains(Flag::Connected));
        // Restart in the same minute neither repeats the job nor catches up earlier minutes.
        let (address,shutdown,task,_vm)=start(&c,clock.clone()).await;let mut client=Client::connect(address,1).await;
        clock.set(294);client.until("Scheduled delivery 2").await;
        assert_eq!(persistence::load(&c.database()).await.unwrap().objects[&ObjectId(1)].state["scheduled"]["count"],Scalar::Integer(2));
        shutdown.send(ShutdownRequest::Sigint).unwrap();task.await.unwrap().unwrap();
    }).await;
}

#[tokio::test(flavor = "current_thread")]
async fn tcp_scheduled_persistence_failure_consumes_job_and_discards_messages() {
    tokio::task::LocalSet::new().run_until(async {
        let (_d,c)=fixture().await;credentials(&c).await;
        module(&c,"global_logic/write.lua",r#"return {schedules={{name='write',cron='* * * * *',handler=function()
            schedule_attempts=(schedule_attempts or 0)+1;local state=mux.world.object(1):state('scheduled');state:set('written',state:get('written',0)+1);mux.world.pemit(1,'SCHEDULE SAVED')
        end}}}"#);
        let clock=Rc::new(Cell::new(120));let (address,shutdown,task,vm)=start(&c,clock.clone()).await;let mut client=Client::connect(address,1).await;
        let mut db=sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(c.database()).foreign_keys(false)).await.unwrap();
        sqlx::raw_sql("CREATE TRIGGER reject_schedule BEFORE INSERT ON object_state WHEN NEW.namespace='scheduled' BEGIN SELECT RAISE(ABORT,'schedule blocked'); END").execute(&mut db).await.unwrap();
        clock.set(234);
        // Lua globals intentionally outlive world rollback, so this observes an actual attempt.
        tokio::time::timeout(Duration::from_secs(5),async {
            while vm.globals().get::<Option<i64>>("schedule_attempts").unwrap()!=Some(1) {tokio::task::yield_now().await;}
        }).await.unwrap();
        client.send("@state/examine #1/scheduled").await;let text=client.until("No state namespace named scheduled.").await;assert!(!text.contains("SCHEDULE SAVED"));
        assert!(!persistence::load(&c.database()).await.unwrap().objects[&ObjectId(1)].state.contains_key("scheduled"));
        client.send("@examine me").await;client.until("CONNECTED").await;
        sqlx::raw_sql("DROP TRIGGER reject_schedule").execute(&mut db).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        client.send("@state/examine #1/scheduled").await;client.until("No state namespace named scheduled.").await; // no retry within the same minute
        clock.set(294);client.until("SCHEDULE SAVED").await;
        // Queue the next minute at its beginning, then shut down before this job's jitter deadline.
        let next=(6..100).find(|minute|jitter("write.lua","write",None,*minute)>5).unwrap();
        clock.set(next*60);tokio::time::sleep(Duration::from_millis(50)).await;
        client.send("@shutdown").await;client.until("Game: Shutdown by").await;task.await.unwrap().unwrap();drop(shutdown);
        clock.set(next*60+54);
        assert_eq!(persistence::load(&c.database()).await.unwrap().objects[&ObjectId(1)].state["scheduled"]["written"],Scalar::Integer(1));
        db.close().await.unwrap();
    }).await;
}

/// Successful replacement cancels captured jobs; failed replacement preserves them and minute history.
#[tokio::test(flavor = "current_thread")]
async fn tcp_reload_schedule_queue_is_atomic() {
    tokio::task::LocalSet::new().run_until(async {
        let (_d,c)=fixture().await;credentials(&c).await;
        let name=(0..100).map(|i|format!("job{i}")).find(|n| jitter("reload_queue.lua",n,None,180)>5 && jitter("reload_queue.lua",n,None,240)>5).unwrap();
        let source=|label: &str|format!(r#"return {{schedules={{{{name='{name}',cron='* * * * *',handler=function()
          local state=mux.world.object(1):state('reload_queue');state:set('{label}',state:get('{label}',0)+1);mux.world.pemit(1,'{label}_JOB')
        end}}}}}}"#);
        module(&c,"global_logic/reload_queue.lua",&source("OLD"));
        let clock=Rc::new(Cell::new(120));
        let (address,shutdown,task,_vm)=start(&c,clock.clone()).await;
        let mut client=Client::connect(address,1).await;
        clock.set(180);tokio::time::sleep(Duration::from_millis(80)).await;
        module(&c,"global_logic/reload_queue.lua","return {broken =");
        client.send("@lua/reload").await;client.until("Lua reload failed:").await;
        clock.set(234);client.until("OLD_JOB").await;
        clock.set(240);tokio::time::sleep(Duration::from_millis(80)).await;
        module(&c,"global_logic/reload_queue.lua",&source("NEW"));
        client.send("@lua/reload").await;client.until("Lua reloaded.").await;
        clock.set(294);tokio::time::sleep(Duration::from_millis(80)).await;
        let world=persistence::load(&c.database()).await.unwrap();
        assert_eq!(world.objects[&ObjectId(1)].state["reload_queue"]["OLD"],Scalar::Integer(1));
        assert!(!world.objects[&ObjectId(1)].state["reload_queue"].contains_key("NEW"));
        client.send("@lua/reload").await;client.until("Lua reloaded.").await;
        let before=std::fs::read(c.database()).unwrap();
        client.send("@lua/check").await;client.until("All Lua module checks passed.").await;
        assert_eq!(before,std::fs::read(c.database()).unwrap());
        clock.set(354);client.until("NEW_JOB").await;
        let world=persistence::load(&c.database()).await.unwrap();
        assert_eq!(world.objects[&ObjectId(1)].state["reload_queue"]["NEW"],Scalar::Integer(1));
        shutdown.send(ShutdownRequest::Sigterm).unwrap();task.await.unwrap().unwrap();
    }).await;
}

/// Automatic cleaning shares manual repair semantics without real-minute sleeps or unsolicited summaries.
#[tokio::test(flavor = "current_thread")]
async fn tcp_cleaning_controls_purge_failure_and_connected_players() {
    tokio::task::LocalSet::new().run_until(async {
        let (_d, c) = fixture().await;
        credentials(&c).await;
        let mut world = persistence::load(&c.database()).await.unwrap();
        let item = world.create(&c, "Doomed".into(), Kind::Thing);
        world.objects.get_mut(&item).unwrap().location = Some(ObjectId(0));
        persistence::save(&c.database(), &world).await.unwrap();
        let scripts = server::prepare(&c).await.unwrap();
        let shared = scripts.world.clone();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let now = tokio::time::Instant::now();
        let clock = Rc::new(Cell::new(now));
        let ticking = clock.clone();
        let config = c.clone();
        let (tx, rx) = oneshot::channel();
        let task = tokio::task::spawn_local(async move { server::run_with_clocks(config, scripts, listener, async { rx.await.unwrap() }, accounts::now, move || ticking.get()).await });
        let mut god = Client::connect(address, 1).await;
        let mut player = Client::connect(address, 2).await;
        let mut second = Client::connect(address, 2).await;
        god.send("@disable cl").await;god.until("Disabled.").await;
        let mut failure = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(c.database()).foreign_keys(false)).await.unwrap();
        sqlx::raw_sql("CREATE TRIGGER reject_destroy BEFORE UPDATE OF has_going_flag ON objects BEGIN SELECT RAISE(ABORT,'schedule failure'); END;").execute(&mut failure).await.unwrap();
        god.send(&format!("@destroy #{}",item.0)).await;
        let failed = god.until("Please try again.").await;
        assert!(!failed.contains("begins to crumble"));
        assert!(!shared.borrow().objects[&item].flags.contains(Flag::Going));
        sqlx::raw_sql("DROP TRIGGER reject_destroy").execute(&mut failure).await.unwrap();failure.close().await.unwrap();
        god.send(&format!("@destroy #{}",item.0)).await;god.until("begins to crumble.").await;
        god.send(&format!("@flag #{}=!going", item.0)).await;god.until("GOING cleared.").await;
        assert!(!persistence::load(&c.database()).await.unwrap().objects[&item].flags.contains(Flag::Going));
        god.send(&format!("@destroy #{}",item.0)).await;god.until("begins to crumble.").await;
        assert!(persistence::load(&c.database()).await.unwrap().objects[&item].flags.contains(Flag::Going));
        clock.set(now + Duration::from_secs(10000));
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert_eq!(shared.borrow().objects[&item].kind, Kind::Thing);
        god.send("@list globals").await;god.until("cleaning...disabled").await;
        let mut db = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(c.database()).foreign_keys(false)).await.unwrap();
        sqlx::raw_sql("CREATE TRIGGER reject_cleaning BEFORE UPDATE ON objects WHEN NEW.type=5 BEGIN SELECT RAISE(ABORT,'cleaning failure'); END;").execute(&mut db).await.unwrap();
        god.send("@enable cleaning").await;god.until("Enabled.").await;
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert_eq!(shared.borrow().objects[&item].kind, Kind::Thing);
        god.send("@list globals").await;
        let status = god.until("cleaning...enabled").await;
        assert!(!status.contains("Database check"));
        sqlx::raw_sql("DROP TRIGGER reject_cleaning").execute(&mut db).await.unwrap();
        db.close().await.unwrap();
        god.send("@destroy #2").await;god.until("player shakes and begins to crumble.").await;
        assert!(shared.borrow().objects[&ObjectId(2)].flags.contains(Flag::Connected));
        clock.set(now + Duration::from_secs(20000));
        player.until("You have been destroyed!").await;
        second.until("You have been destroyed!").await;
        let saved = persistence::load(&c.database()).await.unwrap();
        assert_eq!(saved.objects[&item].kind, Kind::Garbage);
        assert_eq!(saved.objects[&ObjectId(2)].kind, Kind::Garbage);
        assert!(!saved.accounts.contains_key(&ObjectId(2)));
        assert!(!shared.borrow().objects[&ObjectId(2)].flags.contains(Flag::Connected));
        god.send("@list globals").await;
        assert!(!god.until("cleaning...enabled").await.contains("Database check:"));
        tx.send(ShutdownRequest::Sigterm).unwrap();task.await.unwrap().unwrap();
    }).await;
}
