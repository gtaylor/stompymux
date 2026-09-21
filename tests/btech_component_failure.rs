//! Nonweapon failure codes round-trip through inspection without changing system operation.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod firing;

#[tokio::test]
async fn component_failures_round_trip_without_changing_gameplay_on_any_chassis() {
    for source in firing::templates() {
        let (_dir, config, mut world, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let mech = world.btech.constructed_units().contains_key(&id);
        let section = if mech { "LeftTorso" } else { "front" };
        firing::edit(&mut world, id, |state| {
            state["definition"]["sections"][section]["criticals"]["11"] = serde_json::json!({
                "equipment":"CASE","data":"-","modes":[],"brand":null
            });
        });
        world.validate(&config).unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "mechdamage", "").unwrap();
        let pristine = scripts.world().btech.clone();
        for code in 1..=7 {
            let value = format!("G:2/11({code})");
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'mechdamage','{value}')",
                    id.0
                ))
                .unwrap();
            {
                let world = scripts.world();
                let field = battle_unit_damage_field(&world, id).unwrap();
                assert!(field.contains(&value), "{field}");
                let report =
                    battle_critical_report(&world, id, if mech { "LT" } else { "Front" }, false)
                        .unwrap();
                let expected = match code {
                    1 => BattleEquipmentCondition::Jammed,
                    2 => BattleEquipmentCondition::Shorted,
                    3 => BattleEquipmentCondition::Broken,
                    4 => BattleEquipmentCondition::Empty,
                    5 => BattleEquipmentCondition::Destroyed,
                    _ => BattleEquipmentCondition::AmmoJam,
                };
                assert_eq!(report.slots[11].condition, expected);
                let collection = if mech { "constructed" } else { "vehicles" };
                let mut saved = serde_json::to_value(&world.btech).unwrap();
                saved[collection][id.0.to_string()]["component_failures"] = serde_json::json!([]);
                assert_eq!(saved, serde_json::to_value(&pristine).unwrap());
                world.validate(&config).unwrap();
                if code == 1 {
                    for invalid_location in [false, true] {
                        let mut bad = serde_json::to_value(&world.btech).unwrap();
                        let entries = bad[collection][id.0.to_string()]["component_failures"]
                            .as_array_mut()
                            .unwrap();
                        if invalid_location {
                            entries[0]["location"]["slot"] = 99.into();
                        } else {
                            entries.push(entries[0].clone());
                        }
                        if let Ok(state) = serde_json::from_value(bad) {
                            let mut invalid = world.clone();
                            invalid.btech = state;
                            assert!(invalid.validate(&config).is_err());
                        }
                    }
                }
            }
            let saved = scripts.world().clone();
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                saved.btech
            );
            set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "mechdamage", "")
                .unwrap();
            assert_eq!(scripts.world().btech, pristine);
        }
        set_battle_unit_field_action(
            &scripts,
            &config,
            ObjectId(1),
            id,
            "mechdamage",
            "G:2/11(6),G:2/11(2)",
        )
        .unwrap();
        assert!(
            battle_unit_damage_field(&scripts.world(), id)
                .unwrap()
                .contains("G:2/11(2)")
        );
        let before = scripts.world().btech.clone();
        assert!(
            set_battle_unit_field_action(
                &scripts,
                &config,
                ObjectId(1),
                id,
                "mechdamage",
                "G:2/11(8)"
            )
            .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        set_battle_unit_field_action(
            &scripts,
            &config,
            ObjectId(1),
            id,
            "mechdamage",
            "G:2/11(6),C:2/11",
        )
        .unwrap();
        let report = battle_critical_report(
            &scripts.world(),
            id,
            if mech { "LT" } else { "Front" },
            false,
        )
        .unwrap();
        assert_eq!(
            report.slots[11].condition,
            BattleEquipmentCondition::Destroyed
        );
        let field = battle_unit_damage_field(&scripts.world(), id).unwrap();
        assert!(field.contains("C:2/11"));
        assert!(!field.contains("G:2/11"));
    }
}
