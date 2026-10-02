//! Lua tactical-director planning and opt-in encounter coverage.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::{
    BattleMapAsset, BattlePower, BattleUnitSignature, BattleUnitTemplate, Config, HeartbeatHarness,
    Kind, ObjectId, Scripts, World, assign_battle_pilot, create_battle_map, persistence,
    place_battle_unit, refresh_battle_contacts, set_battle_unit_signature,
};

fn install_tactical_packages(config: &Config) {
    let packages = config.lua_dir().join("packages");
    std::fs::create_dir_all(&packages).unwrap();
    std::fs::write(
        packages.join("tactical_director.lua"),
        include_str!("../game/lua/packages/tactical_director.lua"),
    )
    .unwrap();
    std::fs::write(
        packages.join("tactical_encounter_example.lua"),
        include_str!("../game/lua/packages/tactical_encounter_example.lua"),
    )
    .unwrap();
}

#[tokio::test]
async fn director_plan_is_pure_and_reports_partial_units() {
    let (_directory, config, world) = support::isolated_world().await;
    install_tactical_packages(&config);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let snapshot = r#"
        local director = require('tactical_director')
        local btech = require('btech')
        local objective = { map = 7, x = 4, y = 4, arrival_radius = 0 }
        local matching = {
            kind = btech.autopilot.orders.ATTACK_MOVE,
            destination = { map = 7, x = 4, y = 4 },
            arrival_radius = 0,
        }
        local previous = { cycle = 3 }
        local input = {
            version = 1,
            time = 12,
            units = {
                { unit = 10, status = { state = 'idle', revision = 5, active = nil, queue = {} },
                  observation = { position = { map = 7, x = 1, y = 1 } } },
                { unit = 11, status = { state = 'executing', revision = 6, active = nil, queue = {} },
                  observation = { position = { map = 7, x = 4, y = 4 } } },
                { unit = 12, status = { state = 'paused', revision = 7, active = nil, queue = {} },
                  observation = { position = { map = 7, x = 1, y = 2 } } },
                { unit = 13, status = { state = 'blocked', revision = 8, active = nil, queue = {} },
                  observation = { position = { map = 7, x = 1, y = 3 } } },
                { unit = 14, status = { state = 'executing', revision = 9,
                  active = { order = matching }, queue = {} },
                  observation = { position = { map = 7, x = 1, y = 4 } } },
                { unit = 15, error = 'unavailable' },
            },
        }
        local intentions, state = director.plan(input, objective, previous)
        assert(#intentions == 1)
        assert(intentions[1].unit == 10)
        assert(intentions[1].expected_revision == 5)
        assert(intentions[1].mode == btech.autopilot.submission_modes.REPLACE)
        assert(intentions[1].orders[1].kind == btech.autopilot.orders.ATTACK_MOVE)
        assert(intentions[1].orders[1].destination.map == 7)
        assert(state.cycle == 4 and previous.cycle == 3)
        assert(#state.report.planned == 1 and state.report.planned[1].unit == 10)
        assert(#state.report.arrivals == 1 and state.report.arrivals[1].unit == 11)
        assert(#state.report.preserved == 1 and state.report.preserved[1].unit == 14)
        assert(#state.report.skipped == 2)
        assert(#state.report.unavailable == 1)
        assert(state.report.partial == true)
        assert(not pcall(director.plan, input, { map = math.huge, x = 0, y = 0 }))
        assert(not pcall(director.plan, input, { map = 1, x = 0 / 0, y = 0 }))
        return true
    "#;
    assert!(scripts.eval_callback::<bool>(snapshot).unwrap());
}

#[tokio::test]
async fn director_requires_success_feedback_before_reporting_completion() {
    let (_directory, config, world) = support::isolated_world().await;
    install_tactical_packages(&config);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .eval_callback::<()>(
            r#"
            local director = require('tactical_director')
            local btech = require('btech')
            local objective = { map = 3, x = 2, y = 2 }
            local order = {
              id = 41,
              state = 'running',
              order = { kind = btech.autopilot.orders.ATTACK_MOVE,
                        destination = { map = 3, x = 2, y = 2 }, arrival_radius = 0 },
            }
            local moving = {
              version = 1,
              units = {{ unit = 7,
                status = { state = 'executing', revision = 9, active = order, queue = {} },
                observation = { position = { map = 3, x = 2, y = 2 }, speed = 1 } }},
            }
            local _, pending = director.plan(moving, objective)
            assert(#pending.report.preserved == 1 and pending.report.success == false)
            local stopped = {
              version = 1,
              units = {{ unit = 7,
                status = { state = 'idle', revision = 10, active = nil, queue = {} },
                observation = { position = { map = 3, x = 2, y = 2 }, speed = 0 },
                feedback = { history_gap = false,
                  records = {{ order_id = 41, event = 'order_succeeded' }} } }},
            }
            local _, complete = director.plan(stopped, objective, pending)
            assert(complete.report.complete and complete.report.success)
            "#,
        )
        .unwrap();
}

#[tokio::test]
async fn encounter_example_is_require_only_until_started() {
    let (_directory, config, world) = support::isolated_world().await;
    install_tactical_packages(&config);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .eval_callback::<()>(
            r#"
            local example = require('tactical_encounter_example')
            local btech = require('btech')
            local encounter = example.create({ 21, 22 }, { map = 9, x = 2, y = 3 })
            assert(type(encounter) == 'table')
            assert(encounter.started == false and encounter.ticks == 0)
            "#,
        )
        .unwrap();
}

struct EncounterFixture {
    _directory: tempfile::TempDir,
    config: Config,
    world: World,
    map: ObjectId,
    units: [ObjectId; 3],
    enemy: ObjectId,
}

async fn encounter_fixture() -> EncounterFixture {
    let (directory, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Tactical director map".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "tactical.director",
        BattleMapAsset::from_cells(&format!("6 8\n{}", ".0.0.0.0.0.0\n".repeat(8))).unwrap(),
    )
    .unwrap();

    let mut units = [ObjectId(0), ObjectId(0), ObjectId(0)];
    for (index, slot) in units.iter_mut().enumerate() {
        let unit = world.create(&config, format!("Tactical friend {index}"), Kind::Thing);
        world.objects.get_mut(&unit).unwrap().home = Some(ObjectId(config.home()));
        BattleUnitTemplate::parse("JR7-D", include_str!("../game/mechs/JR7-D.toml"))
            .unwrap()
            .create(&mut world, unit)
            .unwrap();
        place_battle_unit(&mut world, unit, map, index as i64, 3).unwrap();
        *slot = unit;
    }
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(units[0]);
    assign_battle_pilot(&mut world, units[0], ObjectId(1)).unwrap();
    let enemy = world.create(&config, "Tactical local hostile".into(), Kind::Thing);
    world.objects.get_mut(&enemy).unwrap().home = Some(ObjectId(config.home()));
    BattleUnitTemplate::parse("JR7-D", include_str!("../game/mechs/JR7-D.toml"))
        .unwrap()
        .create(&mut world, enemy)
        .unwrap();
    // Keep the hostile inside the perception range of at least one friendly so
    // this fixture exercises the production filtered-contact path.
    place_battle_unit(&mut world, enemy, map, 3, 3).unwrap();
    set_battle_unit_signature(
        &mut world,
        enemy,
        BattleUnitSignature {
            team: 1,
            hidden: false,
            illuminated: false,
        },
    )
    .unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    for unit in units.into_iter().chain([enemy]) {
        state["constructed"][unit.0.to_string()]["power"] =
            serde_json::to_value(BattlePower::Running).unwrap();
    }
    world.btech = serde_json::from_value(state).unwrap();
    refresh_battle_contacts(&mut world, &units).unwrap();
    world.validate(&config).unwrap();

    EncounterFixture {
        _directory: directory,
        config,
        world,
        map,
        units,
        enemy,
    }
}

#[tokio::test]
async fn tactical_encounter_handles_friendlies_blocked_member_and_manual_intervention() {
    let fixture = encounter_fixture().await;
    install_tactical_packages(&fixture.config);
    let scripts = Scripts::new(&fixture.config, Rc::new(RefCell::new(fixture.world))).unwrap();
    let [first, second, blocked] = fixture.units;
    scripts
        .eval_callback::<()>(&format!(
            r#"
                example = require('tactical_encounter_example')
                encounter = example.create(
                  {{{first}, {second}, {blocked}}},
                  {{ map = {map}, x = 5, y = 3 }}
                )
                example.start(encounter)
                "#,
            first = first.0,
            second = second.0,
            blocked = blocked.0,
            map = fixture.map.0,
        ))
        .unwrap();

    // Force one participant into the durable blocked state. This simulates an
    // unreachable route without asking the test to depend on a particular map
    // search frontier or heartbeat ordering.
    let mut state = serde_json::to_value(&scripts.world().btech).unwrap();
    let controller = &mut state["controllers"][blocked.0.to_string()];
    controller["state"] = serde_json::json!("blocked");
    controller["blocking_reason"] = serde_json::json!("stuck");
    scripts.world_mut().btech = serde_json::from_value(state).unwrap();

    scripts
        .eval_callback::<()>(
            &format!(
                r#"
                local result = example.tick(encounter)
                assert(result.evaluated == true)
                assert(#result.intentions == 2)
                assert(result.state.report.partial == true)
                assert(#result.state.report.skipped == 1 and result.state.report.skipped[1].unit == {blocked})
                "#,
                blocked = blocked.0,
            ),
        )
        .unwrap();

    // A successful manual movement intervention pauses only that controller;
    // the next time-based evaluation leaves it out while the other friend keeps
    // its matching order.
    stompymux_rs::set_battle_speed(&mut scripts.world_mut(), first, ObjectId(1), 8.0).unwrap();
    let mut state = serde_json::to_value(&scripts.world().btech).unwrap();
    state["simulation_seconds"] = serde_json::json!(3);
    scripts.world_mut().btech = serde_json::from_value(state).unwrap();
    scripts
        .eval_callback::<()>(&format!(
            r#"
                local result = example.tick(encounter)
                assert(result.evaluated == true and result.time == 3)
                local intentions, plan_state = result.intentions, result.state.report
                assert(#intentions == 0)
                assert(plan_state.partial == true)
                assert(#plan_state.skipped == 2)
                assert(#plan_state.preserved == 1 and plan_state.preserved[1].unit == {second})
                "#,
            second = second.0,
        ))
        .unwrap();
}

#[tokio::test]
async fn tactical_encounter_reports_detached_member_without_partial_submit() {
    let fixture = encounter_fixture().await;
    install_tactical_packages(&fixture.config);
    let scripts = Scripts::new(&fixture.config, Rc::new(RefCell::new(fixture.world))).unwrap();
    let [first, second, third] = fixture.units;
    scripts
        .eval_callback::<()>(&format!(
            r#"
            local example = require('tactical_encounter_example')
            local btech = require('btech')
            encounter = example.create(
              {{{first}, {second}, {third}}},
              {{ map = {map}, x = 5, y = 3 }}
            )
            example.start(encounter)
            local initial = example.tick(encounter)
            assert(initial.evaluated, 'initial not evaluated')
            assert(#initial.intentions == 3, 'initial intentions')
            btech.autopilot.detach({second})
            local attached = pcall(btech.autopilot.status, {second})
            assert(not attached, 'detach did not remove controller')
            local grouped = pcall(btech.tactical.observe, {{{first}, {second}, {third}}})
            assert(not grouped, 'strict roster unexpectedly succeeded')
            local detached = example.tick(encounter)
            assert(detached.evaluated, 'detached not evaluated')
            assert(detached.snapshot.roster_incomplete == true, 'roster not incomplete')
            assert(#detached.intentions == 0, 'detached intentions')
            assert(detached.state.report.success == false, 'detached success')
            assert(detached.state.report.partial == true, 'detached partial')
            local unavailable = false
            for _, item in ipairs(detached.state.report.unavailable) do
              unavailable = unavailable or item.unit == {second}
            end
            assert(unavailable, 'detached unavailable')
            "#,
            first = first.0,
            second = second.0,
            third = third.0,
            map = fixture.map.0,
        ))
        .unwrap();
}

#[tokio::test]
async fn production_heartbeat_completes_opt_in_director_encounter() {
    let fixture = encounter_fixture().await;
    let map = fixture.map;
    let enemy = fixture.enemy;
    let units = fixture.units;
    persistence::save(&fixture.config.database(), &fixture.world)
        .await
        .unwrap();
    install_tactical_packages(&fixture.config);
    let mut harness = HeartbeatHarness::new(fixture.config.clone(), fixture.world).unwrap();
    let [first, second, blocked] = units;

    harness
        .scripts()
        .eval_callback::<()>(&format!(
            r#"
            local btech = require('btech')
            example = require('tactical_encounter_example')
            encounter = example.create(
              {{{first}, {second}, {blocked}}},
              {{ map = {map}, x = 5, y = 3, arrival_radius = 2 }},
              {{ config = {{ fire_mode = btech.autopilot.fire_modes.OPPORTUNISTIC }} }}
            )
            example.start(encounter)
            local snapshot = btech.tactical.observe({{{first}, {second}, {blocked}}})
            local hostile = false
            for _, contact in ipairs(snapshot.contacts) do
              if contact.unit == {enemy} then
                hostile = true
                for _, sighting in ipairs(contact.observations) do
                  assert(sighting.current and sighting.identified and not sighting.friendly)
                end
              end
            end
            assert(hostile)
            "#,
            first = first.0,
            second = second.0,
            blocked = blocked.0,
            map = map.0,
            enemy = enemy.0,
        ))
        .unwrap();

    // A blocked participant is left in place for the first policy pass. Its
    // explicit resume below is the only intervention needed for the director
    // to replace the missing order on the next simulation-time evaluation.
    let mut state = serde_json::to_value(&harness.world().btech).unwrap();
    let blocked_controller = &mut state["controllers"][blocked.0.to_string()];
    blocked_controller["state"] = serde_json::json!("blocked");
    blocked_controller["blocking_reason"] = serde_json::json!("stuck");
    harness.scripts().world_mut().btech = serde_json::from_value(state).unwrap();

    for tick in 0..180_i64 {
        if tick == 1 {
            harness
                .scripts()
                .eval_callback::<()>(&format!(
                    "btech.autopilot.resume({blocked})",
                    blocked = blocked.0
                ))
                .unwrap();
        }
        let evaluated_success = harness
            .scripts()
            .eval_callback::<bool>(
                "local result = example.tick(encounter); return result.evaluated and result.state.report.success == true",
            )
            .unwrap();
        let metrics = harness.step(tick + 1).await;
        assert!(
            metrics.committed,
            "production heartbeat must commit tick {tick}"
        );
        if evaluated_success {
            let world = harness.world();
            for unit in units {
                let position = world.btech.constructed_units()[&unit]
                    .position()
                    .expect("friendly remains placed after encounter");
                assert_eq!(position.map, map);
                assert!(
                    stompymux_rs::btech::autopilot::navigation::Hex::new(position.x, position.y)
                        .distance(stompymux_rs::btech::autopilot::navigation::Hex::new(5, 3))
                        <= 2
                );
            }
            return;
        }
    }
    panic!("director encounter did not report all assigned orders succeeded");
}
