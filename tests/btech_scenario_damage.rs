//! Wizard located damage shares native/Lua impacts, criticals and transaction boundaries.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod firing;
mod support;

/// Both interfaces use identical damage and dice across seven chassis, including lethal critical hits.
#[tokio::test]
async fn scenario_section_damage_matches_native_lua_and_restart() {
    for (index, template) in firing::templates().iter().enumerate() {
        let (_dir, config, world, unit, other, _) =
            firing::fixture_with_target(template, Some(BattleWeapon::MediumLaser), template).await;
        let locations = if index < 2 {
            ["h", "lt", "ct"]
        } else {
            ["f", "f", "f"]
        };
        for (section, damage, rear, critical) in [
            (locations[0], 1, false, false),
            (locations[1], 5, true, false),
            (locations[2], 1000, false, true),
        ] {
            let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let report: mlua::Table = lua.eval_callback(&format!("return btech.unit.damage_section(1, {}, '{section}', {damage}, {rear}, {critical})", unit.0)).unwrap();
            let report = serde_json::to_value(report).unwrap();
            assert_eq!(report["kind"], if index < 2 { "mech" } else { "vehicle" });
            if index < 2 {
                assert!(
                    report["impact"]["phases"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|phase| phase["absorbed"].as_u64().unwrap() > 0)
                );
                if damage == 1000 {
                    assert_eq!(report["impact"]["destroyed"], true);
                }
            } else {
                assert!(report["impact"]["armor_damage"].as_u64().unwrap() > 0);
                if damage == 1000 {
                    assert_eq!(report["impact"]["unit_destroyed"], true);
                }
            }
            if index >= 2 && rear {
                assert_eq!(report["impact"]["section"], "rear");
                assert!(report["impact"]["rolls"].as_array().unwrap().len() >= 2);
            }
            let text = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!(
                    "@damagesection {section} {damage} {} {}",
                    if rear { -1 } else { 0 },
                    if critical { 2 } else { 0 }
                ),
            );
            assert!(!text.contains("Invalid"), "{text}");
            assert_eq!(native.world().btech, lua.world().btech);
            assert_ne!(native.world().btech, world.btech);
            let before = serde_json::to_value(&world.btech).unwrap();
            let after = serde_json::to_value(&native.world().btech).unwrap();
            let group = if index < 2 { "constructed" } else { "vehicles" };
            if damage < 1000 {
                assert_eq!(
                    after[group][other.0.to_string()],
                    before[group][other.0.to_string()]
                );
            }
            let saved = native.world().clone();
            persistence::save(&config.database(), &saved).await.unwrap();
            let restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(restored.btech, saved.btech);
        }
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let before = scripts.world().btech.clone();
        for args in [
            "",
            "invalid 1 0 0",
            "h 0 0 0",
            "h 1001 0 0",
            "h 2147483648 0 0",
            "h 1 false 0",
            "h 1 0 0 extra",
        ] {
            let text = support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("@damagesection {args}"),
            );
            assert!(!text.is_empty());
            assert_eq!(scripts.world().btech, before);
        }
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.damage_section(1, {}, '{}', 1000, false, true); error('abort')",
                    unit.0, locations[2]
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
        let visitor = scripts
            .world_mut()
            .create(&config, "Visitor".into(), Kind::Player);
        assert!(
            battle_damage_section_action(
                &scripts,
                &config,
                visitor,
                unit,
                BattleScenarioHit {
                    section: locations[0],
                    damage: 1,
                    rear: false,
                    critical: false
                }
            )
            .is_err()
        );
    }
}
