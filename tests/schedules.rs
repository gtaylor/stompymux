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

/// Admission capacity, runtime controls and cache reload share the real TCP owner.
#[tokio::test(flavor = "current_thread")]
async fn tcp_admission_controls_cache_and_existing_queue() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (d, old) = fixture().await;
            credentials(&old).await;
            let path = d.path().join("stompymux.toml");
            let mut config: toml::Value =
                toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            config
                .as_table_mut()
                .unwrap()
                .entry("mux")
                .or_insert(toml::Value::Table(Default::default()))
                .as_table_mut()
                .unwrap()
                .insert("max_players".into(), toml::Value::Integer(2));
            for key in ["login_attempt_burst", "login_hash_limit"] {
                config["security"]
                    .as_table_mut()
                    .unwrap()
                    .insert(key.into(), toml::Value::Integer(100));
            }
            std::fs::write(path, toml::to_string(&config).unwrap()).unwrap();
            std::fs::write(d.path().join("text/full.txt"), "FULL OLD").unwrap();
            std::fs::write(d.path().join("text/down.txt"), "DOWN MESSAGE").unwrap();
            let c = Config::load(d.path()).unwrap();
            let clock = Rc::new(Cell::new(0));
            let (address, shutdown, task, _) = start(&c, clock).await;
            let mut god = Client::connect(address, 1).await;
            let mut player = Client::connect(address, 2).await;
            async fn attempt(address: std::net::SocketAddr, name: &str, end: &str) -> String {
                let mut client = Client {
                    socket: TcpStream::connect(address).await.unwrap(),
                    pending: Vec::new(),
                };
                client.until("Who are you? ").await;
                client.send(name).await;
                client.until("Password: ").await;
                client.send("secret").await;
                client.until(end).await
            }
            let before =
                persistence::load(&c.database()).await.unwrap().accounts[&ObjectId(2)].successes;
            attempt(address, "#2", "FULL OLD").await;
            let mut registration = Client {
                socket: TcpStream::connect(address).await.unwrap(),
                pending: Vec::new(),
            };
            registration.until("Who are you? ").await;
            registration.send("CapacityCandidate").await;
            registration.until("[Y/n] ").await;
            registration.send("y").await;
            registration.until("Choose a password: ").await;
            registration.send("secret").await;
            registration.until("Retype password: ").await;
            registration.send("secret").await;
            registration.until("FULL OLD").await;
            assert!(
                persistence::load(&c.database())
                    .await
                    .unwrap()
                    .find_player("CapacityCandidate")
                    .is_none()
            );

            assert_eq!(
                persistence::load(&c.database()).await.unwrap().accounts[&ObjectId(2)].successes,
                before
            );
            std::fs::write(d.path().join("text/full.txt"), "FULL NEW").unwrap();
            attempt(address, "#2", "FULL OLD").await;
            god.send("@readcache").await;
            god.until("Banners...0").await;
            attempt(address, "#2", "FULL NEW").await;
            god.send("@wait 1=say QUEUE_FINISHED").await;
            god.send("@disable qu").await;
            god.until("Disabled.").await;
            god.send("@wait 0=say SHOULD_NOT_RUN").await;
            god.until("queueing and triggering are not allowed now.")
                .await;
            god.until("QUEUE_FINISHED").await;
            god.send("@disable log").await;
            god.until("Disabled.").await;
            attempt(address, "#2", "DOWN MESSAGE").await;
            let mut privileged = Client::connect(address, 1).await;
            privileged.send("@list globals").await;
            privileged.until("logins...disabled").await;
            player.send("look").await;
            player.until("Staff Nexus").await;
            let database = std::fs::read(c.database()).unwrap();
            god.send("@list globals").await;
            god.until("logins...disabled").await;
            assert_eq!(database, std::fs::read(c.database()).unwrap());
            shutdown.send(ShutdownRequest::Sigterm).unwrap();
            task.await.unwrap().unwrap();
        })
        .await;
}

/// Forbidden connections get cached safe text before protocol negotiation or authentication.
#[tokio::test(flavor = "current_thread")]
async fn tcp_site_rejection_and_private_inspection() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (d, old) = fixture().await;
            credentials(&old).await;
            let path = d.path().join("stompymux.toml");
            let mut doc: toml::Value =
                toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            doc["sites"] = toml::from_str::<toml::Value>(
                "forbid=[{address='127.0.0.2',mask='255.255.255.255'}]",
            )
            .unwrap();
            std::fs::write(&path, toml::to_string(&doc).unwrap()).unwrap();
            let c = Config::load(d.path()).unwrap();
            std::fs::write(
                c.path(&c.mux.badsite_file),
                "[bold]DENIED[/bold]\r\n\u{1b}]0;unsafe\u{7}",
            )
            .unwrap();
            let (address, shutdown, task, _) = start(&c, Rc::new(Cell::new(0))).await;
            let mut god = Client::connect(address, 1).await;
            let mut ordinary = Client::connect(address, 2).await;
            let before = std::fs::read(c.database()).unwrap();
            let socket = tokio::net::TcpSocket::new_v4().unwrap();
            socket.bind("127.0.0.2:0".parse().unwrap()).unwrap();
            let mut denied = socket.connect(address).await.unwrap();
            let mut bytes = Vec::new();
            tokio::time::timeout(Duration::from_secs(3), denied.read_to_end(&mut bytes))
                .await
                .unwrap()
                .unwrap();
            let text = String::from_utf8(bytes.clone()).unwrap();
            assert!(text.contains("DENIED"), "{text:?}");
            assert!(!bytes.contains(&255) && !bytes.contains(&27));
            assert!(!text.contains("Who are you") && !text.contains("unsafe"));
            god.send("@list si").await;
            let report = god.until("----- Suspected Sites -----").await;
            assert!(report.contains("127.0.0.2") && report.contains("Forbidden"));
            god.send("@telnet #1").await;
            god.until("site status: Trusted").await;
            ordinary.send("@list si").await;
            ordinary.until("Permission denied.").await;
            assert_eq!(before, std::fs::read(c.database()).unwrap());
            god.send("say responsive").await;
            god.until("responsive").await;
            shutdown.send(ShutdownRequest::Sigterm).unwrap();
            task.await.unwrap().unwrap();
        })
        .await;
}

/// Monitor notices describe every actual session transition, while suspect-channel writes commit.
#[tokio::test(flavor = "current_thread")]
async fn tcp_site_monitor_and_suspect_lifecycle() {
    tokio::task::LocalSet::new().run_until(async {
        let (d, old) = fixture().await;
        credentials(&old).await;
        let path = d.path().join("stompymux.toml");
        let mut doc: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        for key in ["login_attempt_burst", "login_hash_limit"] {
            doc["security"].as_table_mut().unwrap().insert(key.into(), toml::Value::Integer(100));
        }
        doc["sites"] = toml::from_str::<toml::Value>("suspect=[{address='127.0.0.0',mask='255.0.0.0'}]").unwrap();
        std::fs::write(path, toml::to_string(&doc).unwrap()).unwrap();
        let c = Config::load(d.path()).unwrap();
        module(&c, "global_logic/monitor_failure.lua", r#"return {events={on_player_disconnect=function(ctx)
            if fail_disconnect and ctx.enactor==2 then
                mux.world.object(2):state('monitor_failure'):set('leak', true)
                mux.world.pemit(1, 'LEAKED_MONITOR_CALLBACK')
                error('injected disconnect callback failure')
            end
        end}}"#);
        let s = scripts(&c).await;
        let service = s.communication(&c);
        service.create("Suspect").unwrap();
        service.add(ObjectId(1), "Suspect", "sus", true, true).unwrap();
        s.world.borrow_mut().objects.get_mut(&ObjectId(1)).unwrap().flags.insert(Flag::Monitor);
        s.world.borrow_mut().objects.get_mut(&ObjectId(2)).unwrap().flags.insert(Flag::Suspect);
        s.world.borrow_mut().objects.get_mut(&ObjectId(2)).unwrap().flags.insert(Flag::Dark);
        let god_name = s.world.borrow().objects[&ObjectId(1)].name.clone();
        let player_name = s.world.borrow().objects[&ObjectId(2)].name.clone();
        let saved = s.world.borrow().clone();
        persistence::save(&c.database(), &saved).await.unwrap();
        let (address, shutdown, task, vm) = start(&c, Rc::new(Cell::new(0))).await;
        let mut god = Client::connect(address, 1).await;
        let mut other_monitor = Client::connect(address, 1).await;
        let player = Client::connect(address, 2).await;
        god.until(&format!("GAME: {player_name} has DARK-connected.")).await;
        other_monitor.until(&format!("GAME: {player_name} has DARK-connected.")).await;
        god.until(&format!("[Suspect] {player_name} has connected.")).await;
        god.until(&format!("[Suspect site: 127.0.0.1] {player_name} has connected.")).await;
        let mut second = Client::connect(address, 2).await;
        god.until(&format!("GAME: {player_name} has reconnected.")).await;
        second.send("quit").await;
        god.until(&format!("GAME: {player_name} has partially disconnected.")).await;
        assert!(persistence::load(&c.database()).await.unwrap().accounts.contains_key(&ObjectId(2)));
        // Failure in channel persistence suppresses channel output, not the actual monitor notice.
        let mut db = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(c.database()).foreign_keys(false)).await.unwrap();
        sqlx::raw_sql("CREATE TRIGGER reject_site_channels BEFORE UPDATE ON comsys_channels BEGIN SELECT RAISE(ABORT,'notification failure'); END;").execute(&mut db).await.unwrap();
        let count = persistence::load(&c.database()).await.unwrap().channels["Suspect"].messages;
        vm.globals().set("fail_disconnect", true).unwrap();
        drop(player);
        god.until(&format!("GAME: {player_name} has disconnected.")).await;
        god.send("@examine #2").await;
        let output = god.until("Powers:").await;
        assert!(!output.contains("CONNECTED") && !output.contains("LEAKED_MONITOR_CALLBACK"), "{output}");
        assert!(!persistence::load(&c.database()).await.unwrap().objects[&ObjectId(2)].state.contains_key("monitor_failure"));
        assert_eq!(count, persistence::load(&c.database()).await.unwrap().channels["Suspect"].messages);
        sqlx::raw_sql("DROP TRIGGER reject_site_channels").execute(&mut db).await.unwrap();
        db.close().await.unwrap();
        for (flag, response) in [("!dark", "DARK cleared."), ("!suspect", "SUSPECT cleared."), ("monitor", "MONITOR set.")] {
            god.send(&format!("@flag #2={flag}")).await;
            god.until(response).await;
        }
        god.send("@chan/destroy Suspect").await;
        god.until("Channel Suspect destroyed.").await;
        // A non-Wizard monitor receives notices; a missing channel is not recreated.
        let mut ordinary_monitor = Client::connect(address, 2).await;
        god.until(&format!("GAME: {player_name} has connected.")).await;
        let third = Client::connect(address, 1).await;
        ordinary_monitor.until(&format!("GAME: {god_name} has reconnected.")).await;
        god.send("@flag #2=!monitor").await;
        god.until("MONITOR cleared.").await;
        drop(third);
        god.until(&format!("GAME: {god_name} has partially disconnected.")).await;
        ordinary_monitor.send("look").await;
        assert!(!ordinary_monitor.until("Staff Nexus").await.contains("GAME:"));
        assert!(!persistence::load(&c.database()).await.unwrap().channels.contains_key("Suspect"));
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

/// Real dispatch uses executor authority, configured aliases and preserved reload policies.
#[tokio::test(flavor = "current_thread")]
async fn tcp_access_policies_queue_reload_and_private_discovery() {
    tokio::task::LocalSet::new().run_until(async {
        let (d,old)=fixture().await;credentials(&old).await;
        let path=d.path().join("stompymux.toml");
        std::fs::rename(&path,d.path().join("base.toml")).unwrap();
        std::fs::write(&path,r#"include=['base.toml']
[security]
login_attempt_burst=100
login_hash_limit=100
[aliases.commands]
acl="acl-probe"
[access.commands]
"@list"="!wizard"
say="wizard"
"look/outside"="god"
"acl-probe"="wizard queue_enabled"
[access.lists]
permissions="!wizard"
"#).unwrap();
        let c=Config::load(d.path()).unwrap();
        let source=|label:&str|format!(r#"return {{commands={{{{name='acl-probe',permission='everyone',pattern='^acl%-probe$',handler=function(ctx) mux.world.pemit(ctx.enactor,'ACL_{label}');return true end}}}}}}"#);
        module(&c,"global_logic/access_tcp.lua",&source("OLD"));
        let (address,shutdown,task,_)=start(&c,Rc::new(Cell::new(0))).await;
        let mut god=Client::connect(address,1).await;
        let mut player=Client::connect(address,2).await;
        let mut other=Client::connect(address,2).await;
        let before=std::fs::read(c.database()).unwrap();
        player.send("@list permissions").await;player.until("Object commands:").await;
        other.send("look").await;
        assert!(!other.until("Staff Nexus").await.contains("Built-in commands"));
        player.send("say BLOCKED").await;player.until("Permission denied.").await;
        player.send("l/outside").await;player.until("Permission denied.").await;
        player.send("acl").await;player.until("Huh?").await;
        assert_eq!(before,std::fs::read(c.database()).unwrap());
        player.send(".create access").await;player.until("created in slot").await;
        player.send(".def x=say BLOCKED_MACRO").await;player.until("defined.").await;
        player.send(".x").await;player.until("Permission denied.").await;
        god.send("@force #2=say BLOCKED_FORCE").await;player.until("Permission denied.").await;
        god.send("@flag #2=wizard").await;god.until("WIZARD set.").await;
        player.send("acl").await;player.until("ACL_OLD").await;
        god.send("@wait 1=say DRAINED_QUEUE").await;
        god.send("@disable qu").await;god.until("Disabled.").await;
        god.until("DRAINED_QUEUE").await;
        player.send("acl").await;player.until("Huh?").await;
        module(&c,"global_logic/access_tcp.lua","return {}");
        god.send("@lua/reload").await;god.until("Lua reload failed:").await;
        god.send("@enable qu").await;god.until("Enabled.").await;
        player.send("acl").await;player.until("ACL_OLD").await;
        module(&c,"global_logic/access_tcp.lua",&source("NEW"));
        god.send("@disable qu").await;god.until("Disabled.").await;
        god.send("@lua/reload").await;god.until("Lua reloaded.").await;
        player.send("acl").await;player.until("Huh?").await;
        god.send("@enable qu").await;god.until("Enabled.").await;
        player.send("acl").await;player.until("ACL_NEW").await;
        god.send("@flag #2=!wizard").await;god.until("WIZARD cleared.").await;
        player.send("acl").await;player.until("Huh?").await;
        shutdown.send(ShutdownRequest::Sigterm).unwrap();task.await.unwrap().unwrap();
    }).await;
}

/// Administration edits stay private and runtime-only across sessions and Lua replacement.
#[tokio::test(flavor = "current_thread")]
async fn tcp_runtime_administration_and_reload() {
    tokio::task::LocalSet::new().run_until(async {
        let (d,c)=fixture().await; credentials(&c).await;
        let source=|label:&str| format!("return {{commands={{{{name='livecfg',permission='everyone',pattern='^livecfg$',handler=function(ctx) mux.world.pemit(ctx.enactor,'{label}:'..mux.config.get('max_players')); return true end}}}}}} ");
        module(&c,"global_logic/livecfg.lua",&source("OLD"));
        let (address,shutdown,task,_)=start(&c,Rc::new(Cell::new(0))).await;
        let mut god=Client::connect(address,1).await;
        let mut other=Client::connect(address,1).await;
        let mut player=Client::connect(address,2).await;
        let before=std::fs::read(c.database()).unwrap();
        let toml=std::fs::read(d.path().join("stompymux.toml")).unwrap();
        player.send("@admin max_players=10").await; player.until("Permission denied.").await;
        god.send("@admin/no max_players=10").await;god.until("Unsupported command switch.").await;
        god.send("@admin max_players=10").await;god.until("Set.").await;
        player.send("livecfg").await;player.until("OLD:10").await;
        other.send("livecfg").await;let text=other.until("OLD:10").await;assert!(!text.contains("Set."));
        god.send("@admin alias=lc livecfg").await;god.until("Set.").await;
        player.send("lc").await;player.until("OLD:10").await;
        god.send("@admin access=livecfg wizard broken_token").await;let text=god.until("Set.").await;assert!(text.contains("broken_token"));
        player.send("lc").await;player.until("Huh?").await;
        module(&c,"global_logic/livecfg.lua","return {}");
        god.send("@lua/reload").await;god.until("Lua reload failed:").await;
        god.send("lc").await;god.until("OLD:10").await;
        module(&c,"global_logic/livecfg.lua",&source("NEW"));
        god.send("@lua/reload").await;god.until("Lua reloaded.").await;
        god.send("lc").await;god.until("NEW:10").await;
        god.send("@admin access=livecfg !wizard").await;god.until("Set.").await;
        player.send("lc").await;player.until("NEW:10").await;
        god.send("@admin forbid_site=127.0.0.0 255.0.0.0").await;god.until("Set.").await;
        let mut banned=TcpStream::connect(address).await.unwrap();let mut bytes=Vec::new();
        tokio::time::timeout(Duration::from_secs(3),banned.read_to_end(&mut bytes)).await.unwrap().unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("Who are you?"));
        player.send("lc").await;player.until("NEW:10").await;
        god.send("@admin permit_site=127.0.0.1 255.255.255.255").await;god.until("Set.").await;
        let mut allowed=TcpStream::connect(address).await.unwrap();let mut bytes=[0;1024];
        let count=tokio::time::timeout(Duration::from_secs(3),allowed.read(&mut bytes)).await.unwrap().unwrap(); assert!(count>0);
        god.send("@list config_permissions").await;god.until("player_zone:").await;
        god.send("@list options").await;god.until("Maximum authenticated sessions: 10").await;
        assert_eq!(before,std::fs::read(c.database()).unwrap());
        assert_eq!(toml,std::fs::read(d.path().join("stompymux.toml")).unwrap());
        god.send("@force me=@admin max_players=11").await;god.until("Set.").await;
        god.send("lc").await;god.until("NEW:11").await;
        shutdown.send(ShutdownRequest::Sigterm).unwrap();task.await.unwrap().unwrap();
        assert_ne!(Config::load(d.path()).unwrap().mux.max_players,11);
    }).await;
}
