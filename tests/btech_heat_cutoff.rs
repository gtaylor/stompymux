//! Shutting off heat sinks: the pilot's count takes effect at the turn boundary, survives
//! restart and damage, and the remaining sinks cool as before.
use crate::support;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use stompymux_rs::*;

/// A powered-off cockpit, optionally placed on an authored environment tile.
async fn fixture(
    source: &str,
    tile: Option<(&str, i32)>,
) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Heat sinks".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(&mut world, id, MechTemplate::parse("test", source).unwrap()).unwrap();
    if let Some((tile, temperature)) = tile {
        let map = world.create(&config, "Environment".into(), Kind::Room);
        create_battle_map(
            &mut world,
            map,
            "environment",
            MapAsset::from_cells(&format!("1 1\n{tile}\n2: 100 {temperature}\n")).unwrap(),
        )
        .unwrap();
        place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    }
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    (dir, config, world, id)
}

/// Native and Lua controls share authority, the turn-boundary delay, callback rollback and
/// saved state; double heat sinks count as one sink each (Total Warfare p. 159).
#[tokio::test]
async fn cutoff_request_applies_at_the_turn_boundary_and_survives_restart() {
    for source in [
        include_str!("../game/units/JR7-D.toml"),
        include_str!("../game/units/GOL-1H.toml"),
        include_str!("../game/units/Daishi-H.toml"),
    ] {
        let (_dir, config, world, id) = fixture(source, None).await;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let initial = scripts.world().btech.clone();
        let (working, points_each) = {
            let world = scripts.world();
            let unit = &world.btech.constructed_units()[&id];
            (
                unit.working_heat_sinks(),
                unit.cooling_capacity() / unit.working_heat_sinks(),
            )
        };
        assert!(set_battle_heat_cutoff(&mut scripts.world_mut(), id, ObjectId(2), 1).is_err());
        assert!(
            set_battle_heat_cutoff(&mut scripts.world_mut(), id, ObjectId(1), working + 1).is_err()
        );
        assert_eq!(scripts.world().btech, initial);
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.heatcutoff({},1,2); error('abort')",
                    id.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, initial);
        let text = support::run_text(&scripts, &config, ObjectId(1), 1, "heatcutoff 2");
        assert!(
            text.contains(&format!(
                "0 of {working} heat sinks shut off; 2 will be shut off from the next turn."
            )),
            "{text}"
        );
        let state: (u16, u16) = scripts
            .eval_callback(&format!(
                "local s = btech.unit.state({}).heat_cutoff; return s.requested, s.shut_off",
                id.0
            ))
            .unwrap();
        assert_eq!(state, (2, 0));
        let text = support::run_text(&scripts, &config, ObjectId(1), 1, "heatcutoff");
        assert!(text.contains("2 will be shut off"), "{text}");
        // A later request replaces the pending one.
        scripts
            .eval_callback::<()>(&format!("btech.unit.heatcutoff({},1,3)", id.0))
            .unwrap();
        assert_eq!(
            scripts.world().btech.constructed_units()[&id].heat_cutoff(),
            HeatCutoff {
                requested: 3,
                shut_off: 0
            }
        );
        // The idle, powered-off unit's turn clock runs while a request waits.
        for second in 1..=12 {
            assert!(advance_battle_heat(&mut scripts.world_mut()).is_empty());
            let world = scripts.world();
            let unit = &world.btech.constructed_units()[&id];
            assert_eq!(unit.overheat_clock().phase, second);
            assert!(unit.heat_cutoff().pending());
        }
        let checkpoint = scripts.world().clone();
        persistence::save(&config.database(), &checkpoint)
            .await
            .unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, checkpoint.btech);
        for _ in 13..=29 {
            assert!(advance_battle_heat(&mut restored).is_empty());
        }
        assert!(
            restored.btech.constructed_units()[&id]
                .heat_cutoff()
                .pending()
        );
        let notices = advance_battle_heat(&mut restored);
        assert_eq!(notices.len(), 1);
        assert!(notices[0].text.contains("3 heat sinks are now shut off"));
        let unit = &restored.btech.constructed_units()[&id];
        assert_eq!(
            unit.heat_cutoff(),
            HeatCutoff {
                requested: 3,
                shut_off: 3
            }
        );
        assert_eq!(unit.overheat_clock().phase, 0);
        assert_eq!(
            unit.heat_rates(&restored).dissipation,
            f64::from(unit.cooling_capacity() - 3 * points_each)
        );
        assert!(!unit.heat_active(&restored));
        let idle = restored.btech.clone();
        for _ in 0..100 {
            advance_battle_heat(&mut restored);
        }
        assert_eq!(restored.btech, idle);
        // Switching every sink back on waits for the boundary as well.
        set_battle_heat_cutoff(&mut restored, id, ObjectId(1), 0).unwrap();
        for _ in 0..29 {
            assert!(advance_battle_heat(&mut restored).is_empty());
        }
        let notices = advance_battle_heat(&mut restored);
        assert_eq!(notices.len(), 1);
        assert!(notices[0].text.contains("All heat sinks are back on"));
        assert_eq!(
            restored.btech.constructed_units()[&id].heat_cutoff(),
            HeatCutoff::default()
        );
        assert!(!restored.btech.constructed_units()[&id].heat_active(&restored));
        // `all` shuts off every working sink, and losing a sink lowers both counts.
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(restored))).unwrap();
        let text = support::run_text(&scripts, &config, ObjectId(1), 1, "heatcutoff all");
        assert!(
            text.contains(&format!("{working} will be shut off")),
            "{text}"
        );
        let text = support::run_text(&scripts, &config, ObjectId(1), 1, "heatcutoff lots");
        assert!(text.contains("number of heat sinks"), "{text}");
        let mut world = scripts.world().clone();
        for _ in 0..30 {
            advance_battle_heat(&mut world);
        }
        assert_eq!(
            world.btech.constructed_units()[&id].heat_cutoff().shut_off,
            working
        );
        let sink = world.btech.constructed_units()[&id]
            .loadout()
            .unwrap()
            .systems
            .into_iter()
            .find(|part| part.system == System::HeatSink)
            .unwrap();
        destroy_battle_critical(&mut world, id, sink.location).unwrap();
        // The next thermal sample lowers both counts to the sinks that still work.
        assert!(world.btech.constructed_units()[&id].heat_active(&world));
        advance_battle_heat(&mut world);
        let unit = &world.btech.constructed_units()[&id];
        assert_eq!(unit.working_heat_sinks(), working - 1);
        assert_eq!(
            unit.heat_cutoff(),
            HeatCutoff {
                requested: working - 1,
                shut_off: working - 1
            }
        );
        assert_eq!(unit.heat_rates(&world).dissipation, 0.0);
        world.validate(&config).unwrap();
        for bad in [
            serde_json::json!({"requested": working + 1, "shut_off": 0}),
            serde_json::json!({"requested": 0, "shut_off": working + 1}),
        ] {
            let mut saved = serde_json::to_value(&world.btech).unwrap();
            saved["constructed"][id.0.to_string()]["heat_cutoff"] = bad;
            if let Ok(state) = serde_json::from_value(saved) {
                let mut invalid = world.clone();
                invalid.btech = state;
                assert!(invalid.validate(&config).is_err());
            }
        }
    }
}

/// Water doubles only the sinks left on, extreme temperatures still apply, and the per-turn
/// netting of the remaining cooling stays exact.
#[tokio::test]
async fn cutoff_environment_samples_use_the_remaining_sinks() {
    for (tile, temperature, stored, cooling) in [
        (".0", 20, 20.0, 6.0),
        ("~2", 20, 20.0, 12.0),
        (".0", 51, 13.0, 5.0),
        (".0", -41, 16.0, 8.0),
    ] {
        let (_dir, config, mut world, id) = fixture(
            include_str!("../game/units/JR7-D.toml"),
            Some((tile, temperature)),
        )
        .await;
        world
            .btech
            .rewrite_unit_record(id, |record| {
                record["heat_cutoff"] = serde_json::json!({"requested": 4, "shut_off": 4});
                record["heat"] = serde_json::json!({"stored": stored, "excess": 0.0});
            })
            .unwrap();
        world.validate(&config).unwrap();
        advance_battle_heat(&mut world);
        let unit = &world.btech.constructed_units()[&id];
        assert_eq!(
            unit.heat_rates(&world).dissipation,
            cooling,
            "{tile}, {temperature}"
        );
        assert!((unit.heat().stored - (stored - cooling / 30.0)).abs() < 1e-10);
        assert_eq!(unit.heat().excess, (stored - cooling).max(0.0));
        for _ in 1..30 {
            advance_battle_heat(&mut world);
        }
        assert!(
            (world.btech.constructed_units()[&id].heat().stored - (stored - cooling)).abs() < 1e-9,
            "{tile}, {temperature}"
        );
    }
}

/// The live server applies a pending request even with no map, running reactor or stored heat.
#[tokio::test(flavor = "current_thread")]
async fn idle_cutoff_request_applies_on_server_heartbeat() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (dir, config, mut world, id) =
                fixture(include_str!("../game/units/JR7-D.toml"), None).await;
            set_battle_heat_cutoff(&mut world, id, ObjectId(1), 5).unwrap();
            let _ = dir.path();
            persistence::save(&config.database(), &world).await.unwrap();
            let (_address, shutdown, task, _lua, mut heartbeats) =
                support::start(&config, Rc::new(Cell::new(1))).await;
            let saved = heartbeats
                .until_saved(&config, 40, |saved| {
                    let unit = &saved.btech.constructed_units()[&id];
                    assert_eq!(unit.power(), Power::Off);
                    !unit.heat_cutoff().pending()
                })
                .await;
            let unit = &saved.btech.constructed_units()[&id];
            assert_eq!(
                unit.heat_cutoff(),
                HeatCutoff {
                    requested: 5,
                    shut_off: 5
                }
            );
            assert_eq!(unit.overheat_clock().phase, 0);
            shutdown.send(ShutdownRequest::Sigterm).unwrap();
            task.await.unwrap().unwrap();
        })
        .await;
}
