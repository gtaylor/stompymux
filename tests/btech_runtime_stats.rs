//! Runtime diagnostics measure saved Rust data and share the server's live-work admission.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Empty and active worlds expose measured counts without changing simulation state or dice.
#[tokio::test]
async fn runtime_statistics_match_live_state_native_lua_and_restart() {
    let (_dir, _config, world) = support::isolated_world().await;
    let empty = battle_runtime_stats(&world, ObjectId(1)).unwrap();
    assert!(!empty.simulation_pending);
    assert_eq!((empty.mechs, empty.vehicles, empty.maps), (0, 0, 0));
    for template in firing::templates() {
        let (_dir, config, mut world, parent, target, _) =
            firing::fixture_with_target(&template, Some(Weapon::MediumLaser), &template).await;
        let player = world.create(&config, "Player".into(), Kind::Player);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        select_battle_target(&mut scripts.world_mut(), parent, ObjectId(1), Some(target)).unwrap();
        scripts.drain_outbox();
        let before = scripts.world().btech.clone();
        let stats = battle_runtime_stats(&scripts.world(), ObjectId(1)).unwrap();
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
        assert!(battle_runtime_stats(&scripts.world(), player).is_err());
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
        assert_eq!(battle_runtime_stats(&restored, ObjectId(1)).unwrap(), stats);
    }
}
