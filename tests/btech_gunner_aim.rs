//! Station previews share targeting and aim arithmetic while preserving independently owned locks.
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Stable locks, coordinate intent and operator skills remain separate on every supported chassis.
#[tokio::test]
async fn station_previews_keep_selection_skill_and_dice_independent() {
    for template in firing::templates() {
        let (_dir, config, mut world, parent, target, index) =
            firing::fixture_with_target(&template, Some(BattleWeapon::Lrm5), &template).await;
        let station = world.create(&config, "Station".into(), Kind::Thing);
        let gunner = world.create(&config, "Gunner".into(), Kind::Player);
        world.objects.get_mut(&gunner).unwrap().location = Some(station);
        world
            .objects
            .get_mut(&gunner)
            .unwrap()
            .flags
            .insert(Flag::Connected);
        set_battle_character(
            &mut world,
            gunner,
            BattleCharacter {
                bruise: 0,
                lethal: 0,
                build: 3,
                reflexes: 4,
                intuition: 3,
                learn: 2,
                charisma: 1,
            },
        )
        .unwrap();
        register_gunner_station(&mut world, ObjectId(1), station, parent, 0).unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        gunner_station_action(&scripts, station, gunner, true).unwrap();
        scripts.drain_outbox();
        let query = format!("btech.gunner.aim({},{},{index})", station.0, gunner.0);
        // A parent's existing lock must never fill an empty station selection.
        let before = scripts.world().btech.clone();
        assert!(
            scripts
                .eval_callback::<()>(&query)
                .unwrap_err()
                .to_string()
                .contains("Select a target")
        );
        assert_eq!(scripts.world().btech, before);
        select_battle_target(&mut scripts.world_mut(), station, gunner, Some(target)).unwrap();
        let result: (i64, i64, i16, u8) = scripts
            .eval_callback(&format!(
                "local r={query}; return r.parent,r.target,r.aim.gunnery,r.aim.target_lock"
            ))
            .unwrap();
        assert_eq!(result, (parent.0, target.0, 11, 1));
        for _ in 0..8 {
            advance_battle_target_locks(&mut scripts.world_mut());
        }
        let penalty: u8 = scripts
            .eval_callback(&format!("return {query}.aim.target_lock"))
            .unwrap();
        assert_eq!(penalty, 0);
        select_battle_target(&mut scripts.world_mut(), parent, ObjectId(1), None).unwrap();
        let before = scripts.world().btech.clone();
        let penalty: u8 = scripts
            .eval_callback(&format!("return {query}.aim.target_lock"))
            .unwrap();
        assert_eq!(penalty, 0);
        assert_eq!(scripts.world().btech, before);
        // Lua results are detached; failed callbacks cannot change parent dice or selections.
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "local r={query}; r.aim.gunnery=99; error('abort')"
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
        for (mode, expected) in [("H", "hex"), ("I", "ignite"), ("C", "clear")] {
            scripts
                .eval_callback::<()>(&format!(
                    "btech.gunner.lock_hex({},{},0,9,'{mode}')",
                    station.0, gunner.0
                ))
                .unwrap();
            scripts.drain_outbox();
            let before = scripts.world().btech.clone();
            let result: (Option<i64>, i32, i32, String) = scripts
                .eval_callback(&format!(
                    "local r={query}; return r.target,r.coordinate.x,r.coordinate.y,r.aim.mode"
                ))
                .unwrap();
            assert_eq!(result, (None, 0, 9, expected.into()));
            assert_eq!(scripts.world().btech, before);
        }
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        let restored = Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
        let disconnected: i16 = restored
            .eval_callback(&format!("return {query}.aim.gunnery"))
            .unwrap();
        assert_eq!(disconnected, 6);
        restored
            .world_mut()
            .objects
            .get_mut(&gunner)
            .unwrap()
            .flags
            .insert(Flag::Connected);

        let expected: mlua::Table = scripts.eval_callback(&format!("return {query}")).unwrap();
        let actual: mlua::Table = restored.eval_callback(&format!("return {query}")).unwrap();
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
        gunner_station_action(&scripts, station, gunner, false).unwrap();
        assert!(scripts.eval_callback::<()>(&query).is_err());
    }
}

/// An explicit arc override changes the lock cost without changing the parent's targeting state.
#[tokio::test]
async fn station_arc_override_preview_does_not_inherit_cockpit_lock() {
    let template = &firing::templates()[0];
    let (_dir, config, mut world, parent, target, index) =
        firing::fixture_with_target(template, Some(BattleWeapon::MediumLaser), template).await;
    let station = world.create(&config, "Station".into(), Kind::Thing);
    let gunner = world.create(&config, "Gunner".into(), Kind::Player);
    world.objects.get_mut(&gunner).unwrap().location = Some(station);
    register_gunner_station(&mut world, ObjectId(1), station, parent, 5).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    gunner_station_action(&scripts, station, gunner, true).unwrap();
    let before = scripts.world().btech.clone();
    let penalty: u8 = scripts
        .eval_callback(&format!(
            "return btech.gunner.aim({},{},{index},{}).aim.target_lock",
            station.0, gunner.0, target.0
        ))
        .unwrap();
    assert_eq!(penalty, 0);
    assert_eq!(scripts.world().btech, before);
    assert!(
        scripts.world().btech.gunner_stations()[&station]
            .target_selection()
            .is_none()
    );
}
