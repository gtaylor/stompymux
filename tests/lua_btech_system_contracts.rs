//! Focused C-shape coverage for the btech.system Lua namespace.
use crate::support;
use stompymux_rs::{ObjectId, accounts, persistence};
use support::{Client, isolated_scripts, isolated_world};
use tokio::{net::TcpListener, sync::oneshot};

#[tokio::test(flavor = "current_thread")]
async fn system_contract_exposes_real_lag_and_sorted_zone_units() {
    let (_d, _c, s) = isolated_scripts().await;
    let excluded: Vec<i64> = s.eval_callback(r#"
      local a=mux.world.create_object{type=mux.world.types.THING,name='Non unit',location=0,home=0}
      local b=mux.world.create_object{type=mux.world.types.THING,name='Other zone',location=0,home=0}
      return {a:dbref(),b:dbref()}
    "#).unwrap();
    let now = stompymux_rs::clock::wall_time();
    s.configure_battle_event_telemetry(now - 20, 10);
    {
        let mut world = s.world_mut();
        world
            .objects
            .get_mut(&stompymux_rs::ObjectId(14))
            .unwrap()
            .zone = Some(stompymux_rs::ObjectId(0));
        world
            .objects
            .get_mut(&stompymux_rs::ObjectId(excluded[0]))
            .unwrap()
            .zone = Some(stompymux_rs::ObjectId(0));
        world
            .objects
            .get_mut(&stompymux_rs::ObjectId(excluded[1]))
            .unwrap()
            .zone = Some(stompymux_rs::ObjectId(4));
        world
            .objects
            .get_mut(&stompymux_rs::ObjectId(15))
            .unwrap()
            .zone = Some(stompymux_rs::ObjectId(0));
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["registrations"]["14"] = "UNIT".into();
        state["registrations"]["15"] = "UNIT".into();
        state["registrations"][excluded[1].to_string()] = "UNIT".into();
        world.btech = serde_json::from_value(state).unwrap();
    }
    s.eval_callback::<()>(r#"local lag=btech.system.event_lag();assert(lag>=100 and lag<=110);local z=mux.world.object(0);local units=btech.system.units_in_zone(z,'ignored');assert(#units==2 and tostring(units[1])=='object(#14)' and tostring(units[2])=='object(#15)')"#).unwrap();
    s.eval_callback::<()>("assert(btech.system.event_lag('ignored') >= 100)")
        .unwrap();
}

/// C game_lag reports `100 * elapsed / ticks - 100` clamped to int, against
/// process-lifetime timing: `process_start_time` outlives every Lua reload, so
/// an explicit `@lua/reload` (successful or failed) must keep the telemetry
/// baseline instead of restarting it.
#[tokio::test(flavor = "current_thread")]
async fn event_lag_process_state_survives_check_successful_and_failed_reload() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (directory, config, _world) = isolated_world().await;
            let module = config.lua_dir().join("global_logic/lag_contract.lua");
            std::fs::create_dir_all(module.parent().unwrap()).unwrap();
            let source = r#"return {commands={{
              name='lag-contract', permission='everyone', pattern='^lag%-contract$',
              handler=function(ctx)
                mux.world.pemit(ctx.enactor, 'LAG ' .. btech.system.event_lag())
                return true
              end,
            }}}"#;
            std::fs::write(&module, source).unwrap();
            let settings = directory.path().join("stompymux.toml");
            let text = std::fs::read_to_string(&settings)
                .unwrap()
                .replace("password_hash_opslimit = 3", "password_hash_opslimit = 1")
                .replace(
                    "password_hash_memlimit = 12582912",
                    "password_hash_memlimit = 1048576",
                );
            std::fs::write(&settings, text).unwrap();
            let config = stompymux_rs::Config::load(directory.path()).unwrap();
            {
                let mut world = persistence::load(&config.database()).await.unwrap();
                world.accounts.get_mut(&ObjectId(1)).unwrap().hash =
                    Some(accounts::hash("secret", &config).unwrap());
                persistence::save(&config.database(), &world).await.unwrap();
            }
            let scripts = stompymux_rs::prepare_server(&config).await.unwrap();
            // With a 1000-second-old process start and 100 recorded ticks the
            // lag percentage is 900; a reload that reset the telemetry would
            // instead report 100*0/100-100 = -100.
            scripts.configure_battle_event_telemetry(stompymux_rs::clock::wall_time() - 1000, 100);
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let (shutdown, request) = oneshot::channel();
            let server_config = config.clone();
            // The lag readings come from the configured telemetry, so no heartbeat runs.
            let (driver, _) = stompymux_rs::HeartbeatDriver::manual();
            let task = tokio::task::spawn_local(async move {
                stompymux_rs::run_with_schedule_clock(
                    server_config,
                    scripts,
                    listener,
                    async {
                        request
                            .await
                            .unwrap_or(stompymux_rs::ShutdownRequest::Sigterm)
                    },
                    || 0,
                    driver,
                )
                .await
            });
            let mut client = Client::connect(address, 1).await;
            async fn read_lag(client: &mut Client) -> i64 {
                client.send("lag-contract").await;
                client.until("LAG ").await;
                let tail = client.until("\r\n").await;
                let digits: String = tail
                    .chars()
                    .filter(|c| c.is_ascii_digit() || *c == '-')
                    .collect();
                digits
                    .parse()
                    .unwrap_or_else(|_| panic!("lag output was not numeric: {tail:?}"))
            }
            let before = read_lag(&mut client).await;
            assert!(
                (890..=1100).contains(&before),
                "baseline lag {before} should track the configured telemetry"
            );
            client.send("@lua/check").await;
            client.until("All Lua module checks passed.").await;
            let after_check = read_lag(&mut client).await;
            assert!(after_check >= before && after_check - before < 60);
            client.send("@lua/reload").await;
            client.until("Lua reloaded.").await;
            let after_reload = read_lag(&mut client).await;
            assert!(
                after_reload >= before && after_reload - before < 60,
                "lag {after_reload} must retain the pre-reload telemetry {before}"
            );
            std::fs::write(&module, "return {commands={ broken }").unwrap();
            client.send("@lua/reload").await;
            client.until("Lua reload failed:").await;
            let after_failure = read_lag(&mut client).await;
            assert!(
                after_failure >= before && after_failure - before < 60,
                "lag {after_failure} must survive a failed reload (baseline {before})"
            );
            shutdown
                .send(stompymux_rs::ShutdownRequest::Sigterm)
                .unwrap();
            task.await.unwrap().unwrap();
            drop(directory);
        })
        .await;
}
