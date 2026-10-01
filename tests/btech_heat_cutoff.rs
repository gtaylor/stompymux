//! Heat cutoff preserves delayed cockpit intent and regulates existing thermal samples.
use crate::support;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};
use stompymux_rs::*;

/// Isolate thermal sampling from startup timers without changing any other saved state.
fn reactor(world: &mut World, id: ObjectId, power: BattlePower) {
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["constructed"][id.0.to_string()]["power"] = serde_json::to_value(power).unwrap();
    world.btech = serde_json::from_value(saved).unwrap();
}

/// A powered-off cockpit, optionally placed on an authored environment tile.
async fn fixture(
    source: &str,
    tile: Option<(&str, i32)>,
) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Heat regulator".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(&mut world, id, BattleTemplate::parse("test",source).unwrap()).unwrap();
    if let Some((tile, temperature)) = tile {
        let map = world.create(&config, "Environment".into(), Kind::Room);
        create_battle_map(
            &mut world,
            map,
            "environment",
            BattleMapAsset::parse(&format!("1 1\n{tile}\n2: 100 {temperature}\n")).unwrap(),
        )
        .unwrap();
        place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    }
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    (dir, config, world, id)
}

/// Native and Lua controls share authority, four-second delay, callback rollback and saved state.
#[tokio::test]
async fn cutoff_cockpit_transition_and_restart() {
    for source in [
        include_str!("../game/mechs/JR7-D.toml"),
        include_str!("../game/mechs/GOL-1H.toml"),
        include_str!("../game/mechs/Daishi-H.toml"),
    ] {
        let (_dir, config, world, id) = fixture(source, None).await;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let initial = scripts.world().btech.clone();
        assert!(
            toggle_battle_heat_cutoff(&mut scripts.world_mut(), id, ObjectId(2), true).is_err()
        );
        assert!(
            toggle_battle_heat_cutoff(&mut scripts.world_mut(), id, ObjectId(1), false).is_err()
        );
        assert_eq!(scripts.world().btech, initial);
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.heatcutoff({},1); error('abort')",
                    id.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, initial);
        let text = support::run_text(&scripts, &config, ObjectId(1), 1, "heatcutoff ignored");
        assert!(text.contains("Engaging"), "{text}");
        let remaining: u8 = scripts
            .eval_callback(&format!(
                "return btech.unit.state({}).heat_cutoff.remaining",
                id.0
            ))
            .unwrap();
        assert_eq!(remaining, 4);
        let pending = scripts.world().btech.clone();
        assert!(
            scripts
                .eval_callback::<()>(&format!("btech.unit.heatcutoff({},1)", id.0))
                .is_err()
        );
        assert_eq!(scripts.world().btech, pending);
        for seconds in [3, 2] {
            assert!(advance_battle_heat(&mut scripts.world_mut()).is_empty());
            assert_eq!(
                scripts.world().btech.constructed_units()[&id]
                    .heat_cutoff()
                    .remaining,
                Some(seconds)
            );
        }
        let checkpoint = scripts.world().clone();
        persistence::save(&config.database(), &checkpoint)
            .await
            .unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, checkpoint.btech);
        assert!(advance_battle_heat(&mut restored).is_empty());
        let notices = advance_battle_heat(&mut restored);
        assert_eq!(notices.len(), 1);
        assert!(notices[0].text.contains("engaged"));
        let unit = &restored.btech.constructed_units()[&id];
        assert!(unit.heat_cutoff().enabled);
        assert_eq!(unit.heat_cutoff().remaining, None);
        assert_eq!(unit.heat_cutoff().disabled, 0);
        assert_eq!(
            unit.overheat_clock(),
            checkpoint.btech.constructed_units()[&id].overheat_clock()
        );
        let idle = restored.btech.clone();
        for _ in 0..100 {
            advance_battle_heat(&mut restored);
        }
        assert_eq!(restored.btech, idle);
        let map = restored.create(&config, "Startup field".into(), Kind::Room);
        create_battle_map(
            &mut restored,
            map,
            "startup",
            BattleMapAsset::parse("1 1\n.0\n").unwrap(),
        )
        .unwrap();
        place_battle_unit(&mut restored, id, map, 0, 0).unwrap();
        assign_battle_pilot(&mut restored, id, ObjectId(1)).unwrap();
        let mut powered = serde_json::to_value(&restored.btech).unwrap();
        powered["constructed"][id.0.to_string()]["power"] =
            serde_json::to_value(BattlePower::Running).unwrap();
        restored.btech = serde_json::from_value(powered).unwrap();
        advance_battle_heat(&mut restored);
        let unit = &restored.btech.constructed_units()[&id];
        assert_eq!(
            unit.heat_cutoff().disabled,
            if unit.definition().has_double_heat_sinks() {
                4
            } else {
                2
            }
        );
        for _ in 0..100 {
            advance_battle_heat(&mut restored);
        }
        let unit = &restored.btech.constructed_units()[&id];
        assert_eq!(unit.heat_cutoff().disabled, unit.definition().heat_sinks);
        assert!(!unit.heat_active(&restored));
        let capacity = unit.definition().heat_sinks;
        let clock = unit.overheat_clock();
        reactor(&mut restored, id, BattlePower::Off);
        toggle_battle_heat_cutoff(&mut restored, id, ObjectId(1), true).unwrap();
        for _ in 0..4 {
            advance_battle_heat(&mut restored);
        }
        assert!(
            !restored.btech.constructed_units()[&id]
                .heat_cutoff()
                .enabled
        );
        assert_eq!(
            restored.btech.constructed_units()[&id]
                .heat_cutoff()
                .disabled,
            capacity
        );
        assert_eq!(
            restored.btech.constructed_units()[&id].overheat_clock(),
            clock
        );
        assert!(!restored.btech.constructed_units()[&id].heat_active(&restored));
        reactor(&mut restored, id, BattlePower::Running);
        for _ in 0..100 {
            advance_battle_heat(&mut restored);
        }
        assert_eq!(
            restored.btech.constructed_units()[&id].heat_cutoff(),
            BattleHeatCutoff::default()
        );
        restored.validate(&config).unwrap();
        for bad in [
            serde_json::json!({"enabled":false,"disabled":101,"remaining":null}),
            serde_json::json!({"enabled":false,"disabled":0,"remaining":0}),
        ] {
            let mut saved = serde_json::to_value(&restored.btech).unwrap();
            saved["constructed"][id.0.to_string()]["heat_cutoff"] = bad;
            if let Ok(state) = serde_json::from_value(saved) {
                let mut invalid = restored.clone();
                invalid.btech = state;
                assert!(invalid.validate(&config).is_err());
            }
        }
    }
}

/// Environment modifiers precede regulation; submerged bonuses are recomputed next sample.
#[tokio::test]
async fn cutoff_environment_samples_and_damaged_capacity() {
    for (tile, temperature, inferno, stored, disabled, sampled_cooling, inspected_cooling) in [
        (".0", 20, 0, 20.0, 2, 8.0, 8.0),
        ("~2", 20, 0, 20.0, 6, 10.0, 8.0),
        (".0", 20, 5, 20.0, 2, 2.0, 2.0),
        (".0", 51, 0, 13.0, 6, 3.0, 3.0),
        (".0", -41, 0, 16.0, 6, 6.0, 6.0),
    ] {
        let (_dir, config, mut world, id) = fixture(
            include_str!("../game/mechs/JR7-D.toml"),
            Some((tile, temperature)),
        )
        .await;
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        let unit = &mut saved["constructed"][id.0.to_string()];
        unit["heat_cutoff"] = serde_json::json!({"enabled":true,"disabled":4,"remaining":null});
        unit["heat"] = serde_json::json!({"stored":stored,"excess":0.0});
        unit["inferno_remaining"] = inferno.into();
        world.btech = serde_json::from_value(saved).unwrap();
        world.validate(&config).unwrap();
        advance_battle_heat(&mut world);
        let unit = &world.btech.constructed_units()[&id];
        assert_eq!(
            unit.heat_cutoff().disabled,
            disabled,
            "{tile}, {temperature}"
        );
        assert!((unit.heat().stored - (stored - sampled_cooling / 30.0)).abs() < 1e-10);
        assert_eq!(unit.heat().excess, (stored - sampled_cooling).max(0.0));
        assert_eq!(unit.heat_rates(&world).dissipation, inspected_cooling);
    }
    let (_dir, config, mut world, id) = fixture(include_str!("../game/mechs/JR7-D.toml"), None).await;
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["constructed"][id.0.to_string()]["heat_cutoff"] =
        serde_json::json!({"enabled":true,"disabled":10,"remaining":null});
    saved["constructed"][id.0.to_string()]["heat"] = serde_json::json!({"stored":9.5,"excess":0.0});
    world.btech = serde_json::from_value(saved).unwrap();
    let sink = world.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .systems
        .into_iter()
        .find(|part| part.system == BattleSystem::HeatSink)
        .unwrap();
    destroy_battle_critical(&mut world, id, sink.location).unwrap();
    world.validate(&config).unwrap();
    advance_battle_heat(&mut world);
    assert_eq!(
        world.btech.constructed_units()[&id].heat_cutoff().disabled,
        9
    );
    assert_eq!(
        world.btech.constructed_units()[&id]
            .heat_rates(&world)
            .dissipation,
        0.0
    );
}

/// The live server admits a cutoff timer even with no map, running reactor or stored heat.
#[tokio::test(flavor = "current_thread")]
async fn idle_cutoff_transition_runs_on_server_heartbeat() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (dir, _config, mut world, id) =
                fixture(include_str!("../game/mechs/JR7-D.toml"), None).await;
            toggle_battle_heat_cutoff(&mut world, id, ObjectId(1), true).unwrap();
            let path = dir.path().join("stompymux.toml");
            let mut settings: toml::Value =
                toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            settings["battletech"]
                .as_table_mut()
                .unwrap()
                .insert("heatcutoff".into(), 0.into());
            std::fs::write(path, toml::to_string(&settings).unwrap()).unwrap();
            let config = Config::load(dir.path()).unwrap();
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let text = support::run_text(&scripts, &config, ObjectId(1), 1, "heatcutoff");
            assert!(text.contains("disabled"), "{text}");
            assert!(
                scripts
                    .eval_callback::<()>(&format!("btech.unit.heatcutoff({},1)", id.0))
                    .is_err()
            );
            assert_eq!(scripts.world().btech, world.btech);
            persistence::save(&config.database(), &world).await.unwrap();
            let (_address, shutdown, task, _lua) =
                support::start(&config, Rc::new(Cell::new(1))).await;
            tokio::time::timeout(Duration::from_secs(9), async {
                loop {
                    let saved = persistence::load(&config.database()).await.unwrap();
                    let unit = &saved.btech.constructed_units()[&id];
                    assert_eq!(unit.power(), BattlePower::Off);
                    if unit.heat_cutoff().enabled {
                        assert_eq!(unit.heat_cutoff().disabled, 0);
                        assert_eq!(
                            unit.overheat_clock(),
                            world.btech.constructed_units()[&id].overheat_clock()
                        );
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            })
            .await
            .unwrap();
            shutdown.send(ShutdownRequest::Sigterm).unwrap();
            task.await.unwrap().unwrap();
        })
        .await;
}
