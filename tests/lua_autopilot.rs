//! In-game Lua management of a unit-attached controller.

use crate::support;
use stompymux_rs::{BattleMapAsset, Kind, ObjectId, Scripts, create_battle_map, place_battle_unit};

#[tokio::test(flavor = "current_thread")]
async fn lua_controls_a_typed_autopilot_queue() {
    let (_directory, config, mut world) = support::isolated_world().await;
    let unit = world.create(&config, "Autopilot test unit".into(), Kind::Thing);
    world.objects.get_mut(&unit).unwrap().home = Some(ObjectId(config.home()));
    let map = world.create(&config, "Autopilot test map".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "autopilot.lua",
        BattleMapAsset::parse("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["registrations"][unit.0.to_string()] = serde_json::json!("MECH");
    world.btech = serde_json::from_value(state).unwrap();
    let root = config.path(&config.database.mech_database);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("PARITY"),
        include_str!("fixtures/btech/mechs/PARITY.toml"),
    )
    .unwrap();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("unit_id", unit.0)
        .unwrap();
    scripts
        .eval_callback::<()>("btech.unit.load_template(mux.world.object(unit_id), 'PARITY')")
        .unwrap();
    place_battle_unit(&mut scripts.world_mut(), unit, map, 0, 0).unwrap();
    scripts
        .eval_callback::<()>(
            r#"
        local u = mux.world.object(unit_id)
        local a = btech.autopilot
        a.attach(u)
        assert(a.status(u).state == 'paused')
        local result = a.submit(u, {{kind=a.orders.HOLD}}, a.submission_modes.APPEND)
        assert(result.ids[1] == 1 and result.revision == 1)
        a.resume(u)
        assert(a.status(u).state == 'executing')
        local history = a.feedback(u)
        assert(#history.records >= 2 and history.history_gap == false)
        assert(a.cancel(u, 1) == true)
        a.detach(u)
    "#,
        )
        .unwrap();
}
