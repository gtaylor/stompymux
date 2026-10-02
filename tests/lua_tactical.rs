//! Atomic tactical admission and sensor-bounded snapshots on isolated fixture worlds.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

async fn fixture() -> (tempfile::TempDir, Config, Scripts, Vec<ObjectId>, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Tactical map".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "tactical",
        BattleMapAsset::from_cells(&format!("1 12\n{}", ".0\n".repeat(12))).unwrap(),
    )
    .unwrap();
    let mut units = Vec::new();
    for index in 0..3 {
        let id = world.create(&config, format!("Tactical {index}"), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        BattleUnitTemplate::parse("JR7-D", include_str!("../game/mechs/JR7-D.toml"))
            .unwrap()
            .create(&mut world, id)
            .unwrap();
        place_battle_unit(
            &mut world,
            id,
            map,
            0,
            if index == 2 { 0 } else { index * 3 + 1 },
        )
        .unwrap();
        units.push(id);
    }
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["simulation_seconds"] = 42.into();
    for unit in &units {
        state["constructed"][unit.0.to_string()]["power"] =
            serde_json::to_value(BattlePower::Running).unwrap();
    }
    world.btech = serde_json::from_value(state).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("a_id", units[0].0)
        .unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("b_id", units[1].0)
        .unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("enemy_id", units[2].0)
        .unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("map_id", map.0)
        .unwrap();
    scripts
        .eval_callback::<()>("btech.autopilot.attach(a_id); btech.autopilot.attach(b_id)")
        .unwrap();
    (dir, config, scripts, units, map)
}

#[tokio::test]
async fn tactical_batch_is_atomic_even_when_lua_catches_rejection() {
    let (_dir, _config, scripts, units, _) = fixture().await;
    scripts.eval_callback::<()>(r#"
        local a, t = btech.autopilot, btech.tactical
        local function intent(unit, revision, orders)
            return {unit=unit,expected_revision=revision,mode=a.submission_modes.REPLACE,orders=orders or {{kind=a.orders.HOLD}}}
        end
        assert(not pcall(t.submit, {intent(a_id,0),intent(b_id,99)}))
        assert(a.status(a_id).revision==0 and #a.status(a_id).queue==0)
        assert(#a.feedback(a_id).records==0)
        assert(not pcall(t.submit, {intent(a_id,0),intent(b_id,0,{{kind=a.orders.ATTACK,target=enemy_id}})}))
        assert(a.status(a_id).revision==0)
        local result=t.submit({intent(b_id,0),intent(a_id,0)})
        assert(result[1].unit==b_id and result[1].ids[1]==1)
        assert(result[2].unit==a_id and result[2].revision==1)
        assert(a.status(a_id).state=='paused')
        assert(a.feedback(a_id).records[1].simulation_time==42)
        assert(not pcall(t.submit,{intent(a_id,1),intent(a_id,1)}))
        assert(not pcall(t.submit,{{unit=a_id,mode=a.submission_modes.REPLACE,orders={}}}))
        assert(not pcall(t.submit,{[1]=intent(a_id,1),[3]=intent(b_id,1)}))
        assert(a.status(a_id).revision==1)
    "#).unwrap();
    let before = scripts.world().btech.controllers()[&units[0]].clone();
    assert!(scripts.eval_callback::<()>("btech.tactical.submit({{unit=a_id,expected_revision=1,mode=btech.autopilot.submission_modes.REPLACE,orders={}}}); error('rollback')").is_err());
    assert_eq!(&before, &scripts.world().btech.controllers()[&units[0]]);
}

#[tokio::test]
async fn tactical_snapshots_are_detached_sorted_and_use_committed_time() {
    let (_dir, config, scripts, units, _map) = fixture().await;
    scripts
        .eval_callback::<()>(
            r#"
        local t=btech.tactical
        local snapshot=t.observe({b_id,a_id})
        assert(snapshot.version==1 and snapshot.time==42)
        assert(snapshot.units[1].unit==a_id and snapshot.units[2].unit==b_id)
        assert(snapshot.units[1].observation.time==42)
        snapshot.units[1].status.revision=999
        snapshot.units[1].observation.position.y=999
        assert(t.observe({a_id}).units[1].revision==0)
        assert(t.observe({a_id}).units[1].observation.position.y==1)
        assert(not pcall(t.observe,{}))
        assert(not pcall(t.observe,{a_id,a_id}))
        assert(not pcall(t.observe,{a_id},{[b_id]=1}))
        btech.autopilot.configure(a_id,{speed_percent=50})
        assert(btech.autopilot.feedback(a_id).records[1].simulation_time==42)
        assert(btech.autopilot.observe(a_id).time==42)
    "#,
        )
        .unwrap();
    let world_snapshot = scripts.world().clone();
    persistence::save(&config.database(), &world_snapshot)
        .await
        .unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech.simulation_time(), 42);
    assert_eq!(
        observe_tactical(&loaded, &units[..2], &Default::default())
            .unwrap()
            .time,
        42
    );
}

#[tokio::test]
async fn shared_sightings_do_not_grant_attack_acquisition_or_leak_hidden_changes() {
    let (_dir, _config, scripts, units, map) = fixture().await;
    {
        let mut world = scripts.world_mut();
        set_battle_unit_signature(
            &mut world,
            units[2],
            BattleUnitSignature {
                team: 1,
                hidden: false,
                illuminated: false,
            },
        )
        .unwrap();
        refresh_battle_contacts(&mut world, &[units[0]]).unwrap();
    }
    scripts.eval_callback::<()>(r#"
        local t,a=btech.tactical,btech.autopilot
        local s=t.observe({a_id,b_id})
        local found=false
        for _,contact in ipairs(s.contacts) do
            if contact.unit==enemy_id then
                found=true
                assert(#contact.observations==1)
                assert(contact.observations[1].observer==a_id and contact.observations[1].current)
            end
        end
        assert(found)
        assert(not pcall(t.submit,{{unit=b_id,expected_revision=0,mode=a.submission_modes.APPEND,orders={{kind=a.orders.ATTACK,target=enemy_id}}}}))
        t.submit({{unit=b_id,expected_revision=0,mode=a.submission_modes.APPEND,orders={{kind=a.orders.MOVE,destination={map=map_id,x=0,y=7}}}}})
    "#).unwrap();
    let before = serde_json::to_value(
        observe_tactical(&scripts.world(), &[units[1]], &Default::default()).unwrap(),
    )
    .unwrap();
    {
        let mut world = scripts.world_mut();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][units[2].0.to_string()]["power"] =
            serde_json::to_value(BattlePower::Off).unwrap();
        world.btech = serde_json::from_value(state).unwrap();
        place_battle_unit(&mut world, units[2], map, 0, 11).unwrap();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][units[2].0.to_string()]["power"] =
            serde_json::to_value(BattlePower::Running).unwrap();
        world.btech = serde_json::from_value(state).unwrap();
    }
    let after = serde_json::to_value(
        observe_tactical(&scripts.world(), &[units[1]], &Default::default()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        before, after,
        "an unobserved enemy move must not affect tactical input"
    );
    assert!(
        observe_tactical(&scripts.world(), &[units[0], units[2]], &Default::default()).is_err()
    );
}

#[tokio::test]
async fn tactical_live_operations_are_unavailable_in_checking_mode() {
    let (_dir, config, live, _, _) = fixture().await;
    let sources = std::sync::Arc::new(stompymux_rs::LuaSources::read(&config).unwrap());
    let checking = Scripts::from_sources(
        &config,
        Rc::new(RefCell::new(live.world().clone())),
        live.help().clone(),
        sources,
        stompymux_rs::lua::RuntimeMode::Checking,
    )
    .unwrap();
    let lua = checking.inspect_lua();
    lua.load(
        r#"
        assert(type(btech.tactical.observe)=='function')
        assert(type(btech.tactical.submit)=='function')
        local ok,err=pcall(btech.tactical.observe,{})
        assert(not ok and err.code=='mux.unavailable.checking')
        ok,err=pcall(btech.tactical.submit,{})
        assert(not ok and err.code=='mux.unavailable.checking')
    "#,
    )
    .exec()
    .unwrap();
}

#[tokio::test]
async fn tactical_feedback_gaps_and_memories_remain_bounded() {
    let (_dir, _config, scripts, units, map) = fixture().await;
    scripts
        .eval_callback::<()>(
            r#"
        local a=btech.autopilot
        for i=1,140 do a.configure(a_id,{speed_percent=50+i%2}) end
        local s=btech.tactical.observe({a_id},{[a_id]=1})
        assert(s.units[1].feedback.history_gap)
        assert(#s.units[1].feedback.records==128)
        assert(s.units[1].feedback.records[1].simulation_time==42)
    "#,
        )
        .unwrap();
    let mut world = scripts.world_mut();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["controllers"][units[0].0.to_string()]["sightings"] = serde_json::json!({
        units[2].0.to_string(): {"position":{"map":map.0,"x":0,"y":7},"seen_at":12}
    });
    world.btech = serde_json::from_value(state.clone()).unwrap();
    let observed = observe_tactical(&world, &[units[0]], &Default::default()).unwrap();
    assert_eq!(observed.contacts.len(), 1);
    assert!(!observed.contacts[0].observations[0].current);
    assert_eq!(observed.contacts[0].observations[0].known_destroyed, None);
    assert!(observed.units[0].status.sightings().is_empty());
    state["simulation_seconds"] = 43.into();
    world.btech = serde_json::from_value(state).unwrap();
    assert!(
        observe_tactical(&world, &[units[0]], &Default::default())
            .unwrap()
            .contacts
            .is_empty()
    );
}

#[tokio::test]
async fn completed_tactical_order_stays_completed_after_restart() {
    let (_dir, config, scripts, units, _) = fixture().await;
    scripts
        .eval_callback::<()>(
            r#"
        local a=btech.autopilot
        btech.tactical.submit({{unit=a_id,expected_revision=0,mode=a.submission_modes.REPLACE,
            orders={{kind=a.orders.MOVE,destination={map=map_id,x=0,y=2}}}}})
        a.resume(a_id)
    "#,
        )
        .unwrap();
    let world_snapshot = scripts.world().clone();
    persistence::save(&config.database(), &world_snapshot)
        .await
        .unwrap();
    let mut harness = HeartbeatHarness::new(config.clone(), scripts.world().clone()).unwrap();
    let succeeded = |world: &World| {
        world.btech.controllers()[&units[0]]
            .feedback_records()
            .iter()
            .filter(|record| record.event == btech::AutopilotFeedbackEvent::OrderSucceeded)
            .count()
    };
    for tick in 1..=120 {
        assert!(harness.step(tick).await.committed);
        if succeeded(&harness.world()) == 1 {
            break;
        }
    }
    let before = harness.world();
    assert_eq!(
        succeeded(&before),
        1,
        "tactical move must finish before restart"
    );
    let revision = before.btech.controllers()[&units[0]].revision();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        loaded.btech.simulation_time(),
        before.btech.simulation_time()
    );
    let mut restored = HeartbeatHarness::new(config, loaded).unwrap();
    for tick in 200..205 {
        assert!(restored.step(tick).await.committed);
    }
    let after = restored.world();
    assert_eq!(
        succeeded(&after),
        1,
        "restart must not duplicate the completion event"
    );
    assert_eq!(after.btech.controllers()[&units[0]].revision(), revision);
    assert_eq!(
        after.btech.simulation_time(),
        before.btech.simulation_time() + 5
    );
}
