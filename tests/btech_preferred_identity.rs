//! Saved preferred IDs are distinct from live labels and share selection across all supported chassis.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod firing;
mod support;

/// Configuration, override precedence, collisions and restart use the same rules for every chassis.
#[tokio::test]
async fn preferred_identity_configuration_selection_and_restart() {
    for source in firing::templates() {
        let (_dir, config, mut world, unit, other, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let key = if world.btech.vehicles().contains_key(&unit) {
            "vehicles"
        } else {
            "constructed"
        };
        let before = serde_json::to_value(&world.btech).unwrap();
        assert_eq!(
            set_battle_preferred_id(&mut world, unit, Some("qX"))
                .unwrap()
                .as_deref(),
            Some("QX")
        );
        assert_eq!(battle_preferred_id(&world, unit).unwrap(), Some("QX"));
        let mut after = serde_json::to_value(&world.btech).unwrap();
        after[key][unit.0.to_string()]["preferred_id"] = serde_json::Value::Null;
        assert_eq!(after, before);
        for argument in [None, Some(""), Some("x")] {
            assert_eq!(
                assign_battlefield_id(&mut world, unit, argument).unwrap(),
                "QX"
            );
        }
        assert_eq!(
            assign_battlefield_id(&mut world, unit, Some("rtignored")).unwrap(),
            "RT"
        );
        assert_eq!(battle_preferred_id(&world, unit).unwrap(), Some("QX"));
        assign_battlefield_id(&mut world, unit, None).unwrap();
        set_battle_preferred_id(&mut world, other, Some("QX")).unwrap();
        let mut replay = world.clone();
        let random = assign_battlefield_id(&mut world, other, None).unwrap();
        assert_ne!(random, "QX");
        assert_eq!(
            assign_battlefield_id(&mut replay, other, None).unwrap(),
            random
        );
        assert_eq!(world.btech, replay.btech);
        assert_eq!(battle_preferred_id(&world, other).unwrap(), Some("QX"));
        remove_battle_map_membership(&mut world, unit).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, world.btech);
        let map = world.objects[&unit].location.unwrap();
        assert_eq!(
            reassign_battle_map(&mut restored, unit, map, None)
                .unwrap()
                .label,
            "QX"
        );
        for value in [None, Some("")] {
            let before = serde_json::to_value(&restored.btech).unwrap();
            assert!(
                set_battle_preferred_id(&mut restored, unit, value)
                    .unwrap()
                    .is_none()
            );
            let after = serde_json::to_value(&restored.btech).unwrap();
            for field in ["battlefield_label", "dice", "position", "map_slot"] {
                assert_eq!(
                    before[key][unit.0.to_string()][field],
                    after[key][unit.0.to_string()][field]
                );
            }
        }
        restored.validate(&config).unwrap();
    }
}

/// Guarded Lua configuration feeds both SETMAPINDX interfaces and rolls back with its callback.
#[tokio::test]
async fn preferred_identity_lua_authority_and_native_selection() {
    for source in firing::templates() {
        let (_dir, config, world, unit, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let value: String = scripts
            .eval_callback(&format!(
                "return btech.unit.set_preferred_id(1, {}, 'zq')",
                unit.0
            ))
            .unwrap();
        assert_eq!(value, "ZQ");
        assert_eq!(
            scripts
                .eval_callback::<String>(&format!(
                    "return btech.unit.state({}).preferred_id",
                    unit.0
                ))
                .unwrap(),
            "ZQ"
        );
        let configured = scripts.world().clone();
        let map = configured.objects[&unit].location.unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(configured.clone()))).unwrap();
        let text = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("setmapindx {} x", map.0),
        );
        assert!(text.contains("Your ID: ZQ"), "{text}");
        lua.eval_callback::<mlua::Table>(&format!(
            "return btech.unit.setmapindex(1, {}, {})",
            unit.0, map.0
        ))
        .unwrap();
        assert_eq!(scripts.world().btech, lua.world().btech);
        let before = scripts.world().btech.clone();
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.set_preferred_id(1, {}, 'AB'); error('rollback')",
                    unit.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        for invalid in ["A", "ABC", "A1", "é"] {
            assert!(
                set_battle_preferred_id_action(&scripts, ObjectId(1), unit, Some(invalid)).is_err()
            );
            assert_eq!(scripts.world().btech, before);
        }
        let visitor = scripts
            .world_mut()
            .create(&config, "Visitor".into(), Kind::Player);
        assert!(set_battle_preferred_id_action(&scripts, visitor, unit, Some("AB")).is_err());
        let cleared: Option<String> = scripts
            .eval_callback(&format!(
                "return btech.unit.set_preferred_id(1, {}, nil)",
                unit.0
            ))
            .unwrap();
        assert!(cleared.is_none());
    }
}

/// Preferences are configurable before placement and malformed saved values cannot bypass validation.
#[tokio::test]
async fn unplaced_preference_and_invalid_saved_values() {
    let (_dir, config, mut world) = support::isolated_world().await;
    for source in firing::templates() {
        let unit = world.create(&config, "Unit".into(), Kind::Thing);
        world.objects.get_mut(&unit).unwrap().home = Some(ObjectId(config.home()));
        BattleUnitTemplate::parse(&source)
            .unwrap()
            .create(&mut world, unit)
            .unwrap();
        set_battle_preferred_id(&mut world, unit, Some("xy")).unwrap();
        assert_eq!(battle_preferred_id(&world, unit).unwrap(), Some("XY"));
        let key = if world.btech.vehicles().contains_key(&unit) {
            "vehicles"
        } else {
            "constructed"
        };
        let mut invalid = serde_json::to_value(&world.btech).unwrap();
        invalid[key][unit.0.to_string()]["preferred_id"] = "A1".into();
        assert!(serde_json::from_value::<BtechState>(invalid).is_err());
        world
            .objects
            .get_mut(&unit)
            .unwrap()
            .flags
            .insert(Flag::Going);
        assert!(set_battle_preferred_id(&mut world, unit, Some("AB")).is_err());
        assert_eq!(battle_preferred_id(&world, unit).unwrap(), Some("XY"));
    }
    assert!(set_battle_preferred_id(&mut world, ObjectId(1), Some("AB")).is_err());
    assert!(battle_preferred_id(&world, ObjectId(-1)).is_err());
}
