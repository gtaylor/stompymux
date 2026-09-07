//! C Lua contracts: symbols, typed errors, generational identities and transactional repair.
use std::{cell::RefCell, path::Path, rc::Rc};
use stompymux_rs::{config::Config, lua::Scripts, persistence, world::ObjectId};
fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        if e.path().is_dir() {
            copy(&e.path(), &to.join(e.file_name()))
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
    let s = Scripts::new(
        &c,
        Rc::new(RefCell::new(
            persistence::load(&c.database()).await.unwrap(),
        )),
    )
    .unwrap();
    (d, c, s)
}

#[tokio::test(flavor = "current_thread")]
async fn public_catalog_and_structured_errors() {
    let (_d, _c, s) = fixture().await;
    let inventory: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/lua-api.json")).unwrap();
    s.lua
        .globals()
        .set(
            "inventory",
            mlua::LuaSerdeExt::to_value(&s.lua, &inventory).unwrap(),
        )
        .unwrap();
    s.eval_callback::<()>(r#"
 local o=mux.world.object(1)
 local channel=mux.comsys.create_channel('Parity')
 local representatives={Object=o,Flags=o:flags(),Powers=o:powers(),State=o:state('parity'),Channel=channel,ChannelFlags=channel:flags(),Error=mux.error.new{code='testing.runtime',message='test'}}
 for _,entry in ipairs(inventory) do
   local class,method=entry.symbol:match('^([^:]+):(.+)$')
   if class then assert(type(representatives[class][method])=='function',entry.symbol)
   else local value=_G;for part in entry.symbol:gmatch('[^.]+')do value=value[part] end;assert(type(value)=='function',entry.symbol) end
 end
 assert(require('mux')==mux and _native==nil and _connected_players==nil and _parents==nil and _object_parents==nil and mux.world._lock_result==nil)
 assert(mux.error.codes==mux.error.code_tree('mux'))
 assert(tostring(mux.error.codes.object.invalid)=='mux.object.invalid')
 assert(not pcall(function() mux.error.codes.object.invalid='forged' end))
 assert(not pcall(function() return mux.error.codes.object.invented end))
 local custom=mux.error.namespace('mygame',{'access.denied','input.invalid'})
 assert(tostring(custom.access.denied)=='mygame.access.denied')
 local ok,e=pcall(mux.error.namespace,'mux.private',{'thing'})
 assert(not ok and mux.error.is(e,mux.error.codes.arg.invalid))
 local detail={value=1};ok,e=pcall(mux.error.raise,custom.access.denied,'denied',detail)
 assert(not ok and e.detail==detail and e:is('mygame.access'))
 local wrapped=mux.error.wrap(e,'mygame.outer','outer');assert(wrapped:root()==e)
 local arbitrary={code='mygame.test',x=1};ok,e=mux.error.pcall(function() error(arbitrary) end)
 assert(not ok and e==arbitrary and type(e.traceback)=='string')
 local yes,a,b,c=mux.error.pcall(function() return 'a',nil,'c' end)
 assert(yes and a=='a' and b==nil and c=='c')
 ok,e=mux.error.pcall(function() error('ordinary') end)
 assert(not ok and e:is(mux.error.codes.runtime))
 for _,spec in ipairs({{mux.world.flags,'flag'},{mux.world.powers,'power'}}) do
   local ok,e=pcall(function()return spec[1].INVALID end)
   assert(not ok and e:is('mux.'..spec[2]..'.invalid'))
   ok,e=pcall(function()spec[1].INVALID=1 end)
   assert(not ok and e:is('mux.'..spec[2]..'.invalid'))
 end
 ok,e=pcall(mux.error.new,12);assert(not ok and type(e)=='string')
 local value={};assert(mux.error.check(value)==value)
 ok,e=pcall(mux.error.check,false,value);assert(not ok and e==value)
 ok,e=pcall(mux.config.get,'does_not_exist');assert(not ok and e:is(mux.error.codes.config.not_found))
 "#).unwrap();
}
#[tokio::test(flavor = "current_thread")]
async fn object_handles_and_synchronous_repairs_roll_back() {
    let (_d, c, s) = fixture().await;
    s.eval_callback::<()>(
        r#"
 local a=mux.world.object(1);local b=mux.world.object(1)
 assert(type(a)=='userdata' and a==b and tostring(a)=='object(#1)')
 assert(not pcall(function()a._id=2 end))
 assert(not pcall(mux.world.object,{_id=1}))
 held=mux.world.create_object{type=mux.world.types.THING,name='Temporary',location=0,home=0}
 held_flags=held:flags();assert(type(held_flags)=='userdata')
 mux.world.destroy_object(held)
 assert(held_flags:has(mux.world.flags.GOING))
 mux.check_db()
 local ok,e=pcall(function()return held:name() end)
 assert(not ok and e:is(mux.error.codes.object.invalid))
 assert(not pcall(function()held_flags:add(mux.world.flags.DARK)end))
 mux.check_db()
 "#,
    )
    .unwrap();
    let before = s.world.borrow().clone();
    assert!(s.eval_callback::<()>(r#"leaked=mux.world.create_object{type=mux.world.types.THING,name='Rollback',location=0,home=0};mux.world.destroy_object(leaked);mux.check_db();error('undo')"#).is_err());
    assert_eq!(before.objects.len(), s.world.borrow().objects.len());
    s.eval_callback::<()>(r#"local replacement=mux.world.create_object{type=mux.world.types.THING,name='Replacement',location=0,home=0};assert(not pcall(function()return leaked:dbref()end))"#).unwrap();
    assert!(s.world.borrow().validate(&c).is_ok());
    let before = s.world.borrow().clone();
    assert!(
        s.eval_callback::<()>("mux.world.destroy_object(mux.world.object(1))")
            .is_err()
    );
    assert_eq!(
        before.objects[&ObjectId(1)].flags,
        s.world.borrow().objects[&ObjectId(1)].flags
    );
}

use std::{cell::Cell, time::Duration};
use stompymux_rs::{
    accounts,
    flags::Flag,
    server::{self, ShutdownRequest},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};
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
async fn tcp_environment_and_maintenance_commit_boundary() {
    tokio::task::LocalSet::new().run_until(async {
  let (_d,c,_)=fixture().await;
  let path=c.root.join("stompymux.toml");let mut doc:toml::Value=toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
  doc["security"]["password_hash_opslimit"]=toml::Value::Integer(1);doc["security"]["password_hash_memlimit"]=toml::Value::Integer(1048576);
  std::fs::write(path,toml::to_string(&doc).unwrap()).unwrap();let c=Config::load(&c.root).unwrap();credentials(&c).await;
  std::fs::write(c.lua_dir().join("global_logic/parity.lua"),r#"return {commands={
    {name='envprobe',permission='everyone',pattern='^envprobe$',handler=function(ctx)
      assert(mux.telnet.environment_has(ctx.descriptor,'uservar','binary'))
      assert(mux.telnet.environment_get(ctx.descriptor,'uservar','binary')==string.char(0,255,128))
      assert(not mux.telnet.environment_has(ctx.descriptor,'var','binary'))
      assert(mux.telnet.environment_get(ctx.descriptor,'var','missing')==nil)
      mux.world.pemit(ctx.enactor,'ENVIRONMENT OK');return true
    end},
    {name='purgeprobe',permission='god',pattern='^purgeprobe$',handler=function(ctx)
      local player=mux.world.object(2);mux.world.destroy_object(player);mux.check_db()
      assert(not pcall(function()return player:dbref()end))
      mux.world.pemit(ctx.enactor,'PURGE COMMITTED');return true
    end}
  }}"#).unwrap();
  let (address,shutdown,task,_)=start(&c,Rc::new(Cell::new(120))).await;
  let mut god=Client::connect(address,1).await;let mut player=Client::connect(address,2).await;
  let mut bytes=vec![255,251,39,255,250,39,0,3];bytes.extend_from_slice(b"binary");bytes.extend_from_slice(&[1,2,0,255,255,128,255,240]);
  god.socket.write_all(&bytes).await.unwrap();god.send("envprobe").await;god.until("ENVIRONMENT OK").await;
  use sqlx::Connection;
  let mut db=sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(c.database()).foreign_keys(false)).await.unwrap();
  sqlx::raw_sql("CREATE TRIGGER reject_lua_purge BEFORE UPDATE OF type ON objects WHEN NEW.type=5 BEGIN SELECT RAISE(ABORT,'purge blocked'); END").execute(&mut db).await.unwrap();
  god.send("purgeprobe").await;let output=god.until("Unable to save your changes.").await;assert!(!output.contains("PURGE COMMITTED"));
  player.send("say still connected").await;player.until("still connected").await;
  assert!(persistence::load(&c.database()).await.unwrap().accounts.contains_key(&ObjectId(2)));
  sqlx::raw_sql("DROP TRIGGER reject_lua_purge").execute(&mut db).await.unwrap();
  god.send("purgeprobe").await;god.until("PURGE COMMITTED").await;player.until("Your character has been destroyed.").await;
  assert!(!persistence::load(&c.database()).await.unwrap().accounts.contains_key(&ObjectId(2)));
  db.close().await.unwrap();shutdown.send(ShutdownRequest::Sigterm).unwrap();task.await.unwrap().unwrap();
 }).await;
}

/// Checking VMs expose immutable catalogs and pure helpers but no live services.
#[tokio::test(flavor = "current_thread")]
async fn checking_rejects_live_services_with_typed_errors() {
    let (_d, c, s) = fixture().await;
    let checking = Scripts::from_sources(
        &c,
        s.world.clone(),
        stompymux_rs::help::HelpIndex::load(&c).unwrap(),
        std::sync::Arc::new(stompymux_rs::lua::sources::Sources::read(&c).unwrap()),
        stompymux_rs::lua::RuntimeMode::Checking,
    )
    .unwrap();
    checking
        .lua
        .load(
            r#"
        assert(require('mux') == mux and debug == nil and _native == nil)
        assert(type(mux.text.width('test')) == 'number')
        assert(mux.config.get('server.port'))
        for _, call in ipairs({
            function() return mux.world.object(1) end,
            function() return mux.world.list_objects() end,
            function() return mux.world.destroy_object(1) end,
            function() return mux.world.teleport_object{object=1,destination=0} end,
            function() return mux.check_db() end,
            function() return mux.telnet.environment_has(1,'var','TERM') end,
            function() return mux.telnet.environment_get(1,'var','TERM') end,
            function() return mux.session.connected_players() end,
            function() return mux.comsys.channel('Public') end,
            function() return mux.log('test.log','discarded') end,
        }) do
            local ok, e = pcall(call)
            assert(not ok and mux.error.is(e, mux.error.codes.unavailable.checking), tostring(e))
        end
    "#,
        )
        .exec()
        .unwrap();
}

/// Teleporting from a Lua callback invokes shared hooks and preserves nested rollback.
#[tokio::test(flavor = "current_thread")]
async fn movement_callbacks_and_maintenance_effects_are_atomic() {
    let (_d, _c, s) = fixture().await;
    // Inject hooks from Rust without exposing the module registry to game scripts.
    let parents: mlua::Table = s.lua.named_registry_value("mux.parents").unwrap();
    s.lua
        .globals()
        .set(
            "test_parent",
            parents.get::<mlua::Table>("default_room.lua").unwrap(),
        )
        .unwrap();
    s.eval_callback::<()>(
        r#"
      local room=mux.world.create_object{type=mux.world.types.ROOM,name='Destination'}
      local thing=mux.world.create_object{type=mux.world.types.THING,name='Cargo',location=0,home=0}
      test_parent.events={on_enter=function(ctx)
        assert(ctx.cause==1 and ctx.enactor==thing:dbref() and ctx.descriptor==nil)
        thing:state('parity'):set('entered',true)
      end}
      mux.world.teleport_object{object=thing,destination=room}
      assert(thing:location()==room and thing:state('parity'):get('entered'))
      test_parent.events.on_exit=function()
        mux.world.destroy_object(thing)
        mux.check_db()
        error('reject relocation')
      end
      local ok=pcall(mux.world.teleport_object,{object=thing,destination=0})
      assert(not ok and thing:location()==room and not thing:flags():has(mux.world.flags.GOING))
      test_parent.events={}
    "#,
    )
    .unwrap();
    // A nested caught failure must discard its pending SQL cleanup as well.
    assert!(
        s.world
            .borrow()
            .objects
            .values()
            .all(|o| o.kind != stompymux_rs::world::Kind::Garbage)
    );
}

/// Native movement must not replenish the budget on each nested callback.
#[tokio::test(flavor = "current_thread")]
async fn nested_movement_shares_instruction_budget() {
    let (_d, c, s) = fixture().await;
    let path = c.root.join("stompymux.toml");
    let mut doc: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    doc["lua"]
        .as_table_mut()
        .unwrap()
        .insert("instruction_limit".into(), toml::Value::Integer(20000));
    std::fs::write(path, toml::to_string(&doc).unwrap()).unwrap();
    let c = Config::load(&c.root).unwrap();
    let s = Scripts::new(&c, s.world.clone()).unwrap();
    let before = s.world.borrow().objects.len();
    let result = s.eval_callback::<()>(r#"
        local room=mux.world.create_object{type=mux.world.types.ROOM,name='Budget room'}
        local thing=mux.world.create_object{type=mux.world.types.THING,name='Budget cargo',location=0,home=0}
        for i=1,10000 do
            mux.world.teleport_object{object=thing,destination=room}
            mux.world.teleport_object{object=thing,destination=0}
        end
    "#);
    assert!(result.is_err());
    assert_eq!(s.world.borrow().objects.len(), before);
}
