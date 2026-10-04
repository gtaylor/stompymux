//! Compact damage fields preserve common ordering, failure codes, read-only inspection and restart.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Armor, structure, ammunition and destroyed slots share one format across all supported chassis.
#[tokio::test]
async fn damage_fields_share_material_order_native_lua_and_restart() {
    for source in firing::templates() {
        let (_dir, config, mut world, id, _, weapon) =
            firing::fixture_with_supply(&source, Some(Weapon::Ac5), &source, false, Some("")).await;
        assert_eq!(battle_unit_damage_field(&world, id).unwrap(), "");
        let mech = world.btech.constructed_units().contains_key(&id);
        let section = if mech { "LeftTorso" } else { "front" };
        let (bin, slot, capacity) = if mech {
            let loadout = world.btech.constructed_units()[&id].loadout().unwrap();
            let (index, bin) = loadout
                .ammunition
                .iter()
                .enumerate()
                .find(|(_, bin)| bin.weapon == Weapon::Ac5)
                .unwrap();
            (index, bin.location.slot, bin.capacity)
        } else {
            let loadout = world.btech.vehicles()[&id].loadout().unwrap();
            let (index, bin) = loadout
                .ammunition
                .iter()
                .enumerate()
                .find(|(_, bin)| bin.weapon == Weapon::Ac5)
                .unwrap();
            (index, bin.location.slot, bin.capacity)
        };
        firing::edit(&mut world, id, |unit| {
            let state = &mut unit["sections"][section];
            state["armor"] = (state["armor"].as_u64().unwrap() - 3).into();
            state["internal"] = (state["internal"].as_u64().unwrap() - 2).into();
            if mech {
                state["rear"] = (state["rear"].as_u64().unwrap() - 1).into();
            }
            unit["ammunition"][bin] = (capacity - 2).into();
            unit["jammed_weapons"] = serde_json::json!([weapon]);
        });
        let prefix = if mech {
            "A:2/3,A(R):2/1,I:2/2"
        } else {
            "A:2/3,I:2/2"
        };
        let expected = format!("{prefix},G:2/0(6),R:2/{slot}(2)");
        let before = world.btech.clone();
        assert_eq!(battle_unit_damage_field(&world, id).unwrap(), expected);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let report =
            view_battle_unit_fields_action(&scripts, &config, ObjectId(1), id, "mechdamage")
                .unwrap();
        assert_eq!(report.fields[0].value.as_deref(), Some(expected.as_str()));
        let lua: mlua::Table = scripts
            .eval_callback(&format!(
                "return btech.unit.fields(1,{},'mechdamage')",
                id.0
            ))
            .unwrap();
        assert_eq!(
            serde_json::to_value(lua).unwrap(),
            serde_json::to_value(report).unwrap()
        );
        assert!(
            support::run_text(&scripts, &config, ObjectId(1), 1, "@viewmech mechdamage")
                .contains(&expected)
        );
        scripts.drain_outbox();
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.fields(1,{},'mechdamage'); error('abort')",
                    id.0
                ))
                .is_err()
        );
        assert!(scripts.drain_outbox().is_empty());
        assert_eq!(scripts.world().btech, before);
        let snapshot = scripts.world().clone();
        persistence::save(&config.database(), &snapshot)
            .await
            .unwrap();
        let mut world = persistence::load(&config.database()).await.unwrap();
        assert_eq!(world.btech, before);
        assert_eq!(battle_unit_damage_field(&world, id).unwrap(), expected);
        firing::edit(&mut world, id, |unit| {
            unit["jammed_weapons"] = serde_json::json!([]);
            unit["ammunition"][bin] = 0.into();
            unit["lost_criticals"] =
                serde_json::json!([{"section":section,"slot":0},{"section":section,"slot":slot}]);
        });
        assert_eq!(
            battle_unit_damage_field(&world, id).unwrap(),
            format!("{prefix},C:2/0,C:2/{slot}")
        );
    }
}

/// The powered-down state uses one primary-slot failure record even for multi-slot Gauss weapons.
#[tokio::test]
async fn damage_field_reports_powered_down_weapons_without_spurious_slot_losses() {
    for source in firing::templates() {
        let (_dir, _config, mut world, id, _, weapon) =
            firing::fixture_with_target(&source, Some(Weapon::GaussRifle), &source).await;
        firing::edit(&mut world, id, |unit| {
            unit["powered_down_weapons"] = serde_json::json!([weapon])
        });
        assert_eq!(battle_unit_damage_field(&world, id).unwrap(), "G:2/0(5)");
    }
}

/// Critical failure states retain their distinct codes instead of collapsing into feed jams.
#[tokio::test]
async fn damage_field_distinguishes_vehicle_and_enhanced_mech_failures() {
    for source in firing::templates() {
        let (_dir, _config, mut world, id, _, weapon) =
            firing::fixture_with_target(&source, Some(Weapon::Ac5), &source).await;
        let mech = world.btech.constructed_units().contains_key(&id);
        firing::edit(&mut world, id, |unit| {
            if mech {
                unit["weapon_damage"] = serde_json::json!([{"location":{"section":"LeftTorso","slot":1},"effects":["barrel"]}]);
                unit["weapon_damage_jams"] = serde_json::json!([weapon]);
            } else {
                unit["weapon_failures"] = serde_json::json!({weapon.to_string():"jammed"});
                unit["weapon_recycle"] = serde_json::json!({weapon.to_string():60});
            }
        });
        assert_eq!(
            battle_unit_damage_field(&world, id).unwrap(),
            if mech { "G:2/0(7)" } else { "G:2/0(1)" }
        );
    }
}
