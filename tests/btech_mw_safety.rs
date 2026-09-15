//! MechWarrior safety configuration is shared across chassis and resets only on completed startup.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod firing;
mod support;

/// Safety command, preference and Lua edit the same inverse flag with normal rollback and persistence.
#[tokio::test]
async fn safety_controls_share_state_and_startup_resets_only_the_safety_bit() {
    for source in firing::templates() {
        let (_dir, config, world, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let prefs = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        assert!(support::run_text(&native, &config, ObjectId(1), 1, "safety").contains("ON"));
        assert!(support::run_text(&native, &config, ObjectId(1), 1, "safety off").contains("OFF"));
        assert!(
            support::run_text(&prefs, &config, ObjectId(1), 1, "mechprefs MWSafety OFF")
                .contains("OFF")
        );
        lua.eval_callback::<()>(&format!("btech.unit.mw_safety({},1,false)", id.0))
            .unwrap();
        assert_eq!(native.world().btech, prefs.world().btech);
        assert_eq!(native.world().btech, lua.world().btech);
        let enabled: bool = lua
            .eval_callback(&format!("return btech.unit.state({}).mw_safety", id.0))
            .unwrap();
        assert!(!enabled);
        assert!(
            battle_unit_status(&native.world(), id, "")
                .unwrap()
                .contains("Weapon Safeties are [fg=red bold]OFF[reset].")
        );
        let before = native.world().btech.clone();
        assert!(
            lua.eval_callback::<()>(&format!("btech.unit.mw_safety({},2,true)", id.0))
                .is_err()
        );
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.mw_safety({},1,true); error('abort')",
                id.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, before);
        assert!(
            support::run_text(&native, &config, ObjectId(1), 1, "safety unknown").contains("OFF")
        );
        assert_eq!(native.world().btech, before);
        set_battle_unit_field_action(&native, &config, ObjectId(1), id, "MechPrefs", "aj").unwrap();
        let mut world = native.world().clone();
        let safety = |world: &World| {
            world.btech.constructed_units().get(&id).map_or_else(
                || world.btech.vehicles()[&id].mw_safety(),
                |unit| unit.mw_safety(),
            )
        };
        stop_battle_unit(
            &mut world,
            id,
            ObjectId(1),
            BattleMovementRules::STANDARD.fall,
        )
        .unwrap();
        assert!(!safety(&world));
        assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
        advance_battle_units(&mut world, 100);
        stop_battle_unit(
            &mut world,
            id,
            ObjectId(1),
            BattleMovementRules::STANDARD.fall,
        )
        .unwrap();
        assert!(!safety(&world));
        assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
        for now in 101..105 {
            advance_battle_units(&mut world, now);
            assert!(!safety(&world));
        }
        persistence::save(&config.database(), &world).await.unwrap();
        world = persistence::load(&config.database()).await.unwrap();
        assert!(!safety(&world));
        advance_battle_units(&mut world, 105);
        assert!(safety(&world));
        persistence::save(&config.database(), &world).await.unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(world.btech, loaded.btech);
        let restored = Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
        let report =
            view_battle_unit_fields_action(&restored, &config, ObjectId(1), id, "MechPrefs")
                .unwrap();
        assert_eq!(report.fields[0].value.as_deref(), Some("j"));
    }
}
