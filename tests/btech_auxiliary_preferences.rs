//! Retained preference flags share configuration and persistence without changing combat behavior.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Native/Lua toggles and field bits share storage and survive restart across all chassis.
#[tokio::test]
async fn retained_preferences_share_controls_and_leave_attack_results_unchanged() {
    for source in firing::templates() {
        let (_dir, config, world, id, target, index) =
            firing::fixture_with_target(&source, Some(Weapon::MediumLaser), &source).await;
        let normal = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        assert!(
            support::run_text(&native, &config, ObjectId(1), 1, "mechprefs BTHDebug ON")
                .contains("BTH Debugging is now ON")
        );
        lua.eval_callback::<()>(&format!("btech.unit.bth_debug({},1,true)", id.0))
            .unwrap();
        assert_eq!(native.world().btech, lua.world().btech);
        assert!(
            lua.eval_callback::<()>(&format!("btech.unit.bth_debug({},2,false)", id.0))
                .is_err()
        );
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.bth_debug({},1,false); error('abort')",
                id.0
            ))
            .is_err()
        );
        assert_eq!(native.world().btech, lua.world().btech);
        set_battle_unit_field_action(&native, &config, ObjectId(1), id, "MechPrefs", "fj").unwrap();
        let report =
            view_battle_unit_fields_action(&native, &config, ObjectId(1), id, "MechPrefs").unwrap();
        assert_eq!(report.fields[0].value.as_deref(), Some("fj"));
        native.drain_outbox();
        let saved = native.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, saved.btech);
        let restored = Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
        let selected: bool = restored
            .eval_callback(&format!("return btech.unit.state({}).bth_debug", id.0))
            .unwrap();
        assert!(selected);
        restored
            .eval_callback::<()>(&format!("btech.unit.bth_debug({},1,false)", id.0))
            .unwrap();
        let report =
            view_battle_unit_fields_action(&restored, &config, ObjectId(1), id, "MechPrefs")
                .unwrap();
        assert_eq!(report.fields[0].value.as_deref(), Some("f"));
        restored.drain_outbox();
        let command = format!("fire {index} #{}", target.0);
        assert_eq!(
            support::run_text(&native, &config, ObjectId(1), 1, &command),
            support::run_text(&normal, &config, ObjectId(1), 1, &command)
        );
        set_battle_unit_field_action(&native, &config, ObjectId(1), id, "MechPrefs", "0").unwrap();
        assert_eq!(native.world().btech, normal.world().btech);
    }
}
