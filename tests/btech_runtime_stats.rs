//! Runtime diagnostics measure saved Rust data and share the server's live-work admission.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Empty and active worlds expose measured counts without changing simulation state or dice.
#[tokio::test]
async fn runtime_statistics_match_live_state_native_lua_and_restart() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let empty = battle_runtime_stats(&world, &config, ObjectId(1)).unwrap();
    assert!(empty.simulation_pending);
    assert_eq!(empty.reactor_startup_remaining, 31);
    for _ in 0..31 {
        advance_battle_reactor_windows(&mut world);
    }
    assert!(
        !battle_runtime_stats(&world, &config, ObjectId(1))
            .unwrap()
            .simulation_pending
    );
    assert_eq!((empty.mechs, empty.vehicles, empty.maps), (0, 0, 0));
    for template in firing::templates() {
        let (_dir, config, mut world, parent, target, _) =
            firing::fixture_with_target(&template, Some(BattleWeapon::MediumLaser), &template)
                .await;
        let player = world.create(&config, "Player".into(), Kind::Player);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        select_battle_target(&mut scripts.world_mut(), parent, ObjectId(1), Some(target)).unwrap();
        scripts.drain_outbox();
        let before = scripts.world().btech.clone();
        let stats = battle_runtime_stats(&scripts.world(), &config, ObjectId(1)).unwrap();
        assert!(stats.simulation_pending);
        assert_eq!(stats.mechs + stats.vehicles, 2);
        assert_eq!(stats.artillery_shots, 0);
        assert_eq!(
            stats.encoded_state_bytes,
            serde_json::to_vec(&before).unwrap().len() as u64
        );
        assert!(stats.inline_record_bytes > std::mem::size_of_val(&before));
        let lua: mlua::Table = scripts
            .eval_callback("return btech.runtime.stats(1)")
            .unwrap();
        assert_eq!(
            serde_json::to_value(lua).unwrap(),
            serde_json::to_value(&stats).unwrap()
        );
        let events = support::run_text(&scripts, &config, ObjectId(1), 1, "eventstats");
        assert!(events.contains("Artillery shots in flight: 0"), "{events}");
        let memory = support::run_text(&scripts, &config, ObjectId(1), 1, "memstats LONG");
        assert!(
            memory.contains(&format!(
                "Encoded state bytes (JSON): {}",
                stats.encoded_state_bytes
            )),
            "{memory}"
        );
        for (kind, count) in &stats.registration_kinds {
            assert!(memory.contains(&format!("{kind}: {count} registrations")));
        }
        assert!(memory.contains("Allocator totals: unavailable"));
        assert!(battle_runtime_stats(&scripts.world(), &config, player).is_err());
        assert!(
            scripts
                .eval_callback::<mlua::Table>(&format!("return btech.runtime.stats({})", player.0))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            battle_runtime_stats(&restored, &config, ObjectId(1)).unwrap(),
            stats
        );
    }
}

/// A solitary digging vehicle must keep ticking without scanner peers or unrelated timers.
#[tokio::test]
async fn digging_alone_keeps_simulation_pending_until_completion() {
    for movement in ["track", "wheel"] {
        let (_dir, config, mut world) = support::isolated_world().await;
        for _ in 0..31 {
            advance_battle_reactor_windows(&mut world);
        }
        let map = world.create(&config, "Digging field".into(), Kind::Room);
        create_battle_map(
            &mut world,
            map,
            "dig",
            MapAsset::from_cells("1 1\n.0\n").unwrap(),
        )
        .unwrap();
        support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
        let id = world.create(&config, "Digger".into(), Kind::Thing);
        let source = include_str!("../game/mechs/Demolisher.toml").replace(
            "movement = \"track\"",
            &format!("movement = \"{movement}\""),
        );
        BattleUnitTemplate::parse("test", &source)
            .unwrap()
            .create(&mut world, id)
            .unwrap();
        support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
        place_battle_unit(&mut world, id, map, 0, 0).unwrap();
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
        assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
        assert!(battle_contact_observers(&world).is_empty());
        assert!(
            !battle_runtime_stats(&world, &config, ObjectId(1))
                .unwrap()
                .simulation_pending
        );
        dig_battle_unit(&mut world, id, ObjectId(1)).unwrap();
        for elapsed in 0..20 {
            assert!(
                battle_runtime_stats(&world, &config, ObjectId(1))
                    .unwrap()
                    .simulation_pending,
                "digging must retain the next tick at {elapsed}s"
            );
            if elapsed == 7 {
                persistence::save(&config.database(), &world).await.unwrap();
                let loaded = persistence::load(&config.database()).await.unwrap();
                assert_eq!(loaded.btech, world.btech);
                world = loaded;
            }
            advance_battle_units(&mut world, 0);
        }
        assert_eq!(
            world.btech.vehicles()[&id].dig_state(),
            BattleDigState::covered()
        );
        assert!(
            !battle_runtime_stats(&world, &config, ObjectId(1))
                .unwrap()
                .simulation_pending
        );
    }
}
