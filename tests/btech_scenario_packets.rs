//! Wizard packet damage exercises shared combat and complete native/Lua transaction boundaries.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Packet counts, truncation, flags, lethal traversal and restart agree for every supported chassis.
#[tokio::test]
async fn scenario_packets_match_native_lua_and_restart() {
    for (index, source) in firing::templates().iter().enumerate() {
        let (_dir, config, world, unit, _, _) =
            firing::fixture_with_target(source, Some(BattleWeapon::MediumLaser), source).await;
        for (damage, clusters, rear) in [(11, 3, false), (10, 1, true), (1000, 4, false)] {
            let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let report: mlua::Table = lua
                .eval_callback(&format!(
                    "return btech.unit.damage(1, {}, {damage}, {clusters}, {rear}, true)",
                    unit.0
                ))
                .unwrap();
            let report = serde_json::to_value(report).unwrap();
            assert_eq!(report["packet_damage"], damage / clusters);
            assert_eq!(report["discarded_damage"], damage % clusters);
            let impacts = report["impacts"].as_array().unwrap();
            assert_eq!(impacts.len(), clusters as usize);
            assert_eq!(
                impacts[0]["kind"],
                if index < 2 { "mech" } else { "vehicle" }
            );
            if index < 2 {
                assert!(
                    impacts[0]["report"]["impact"]["phases"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|phase| phase["absorbed"].as_u64().unwrap() > 0)
                );
            } else {
                assert!(
                    impacts[0]["report"]["damage"]["armor_damage"]
                        .as_u64()
                        .unwrap()
                        > 0
                );
            }
            let text = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!(
                    "@damage {damage} {clusters} {} 0",
                    if rear { -1 } else { 0 }
                ),
            );
            assert!(!text.contains("Invalid"), "{text}");
            // True and false critical input produce the same random location eligibility.
            assert_eq!(native.world().btech, lua.world().btech);
            assert_ne!(native.world().btech, world.btech);
            if damage == 1000 {
                let state = serde_json::to_value(&native.world().btech).unwrap();
                let group = if index < 2 { "constructed" } else { "vehicles" };
                assert_ne!(
                    state[group][unit.0.to_string()],
                    serde_json::to_value(&world.btech).unwrap()[group][unit.0.to_string()]
                );
                let destroyed = if index < 2 {
                    native.world().btech.constructed_units()[&unit].is_destroyed()
                } else {
                    native.world().btech.vehicles()[&unit].is_destroyed()
                };
                assert!(destroyed, "chassis {index}");
            }
            let saved = native.world().clone();
            persistence::save(&config.database(), &saved).await.unwrap();
            let restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(restored.btech, saved.btech);
        }
    }
}

/// Guard failures and a late callback failure leave all random state, damage and messages untouched.
#[tokio::test]
async fn scenario_packets_guard_and_rollback_across_chassis() {
    for source in firing::templates() {
        let (dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(&source, Some(BattleWeapon::MediumLaser), &source).await;
        let visitor = world.create(&config, "Visitor".into(), Kind::Player);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        for args in [
            "",
            "1",
            "1 1 0 0 extra",
            "0 1 0 0",
            "1001 1 0 0",
            "10 0 0 0",
            "10 -1 0 0",
            "10 11 0 0",
            "2147483648 1 0 0",
            "10 1 false 0",
            "10 1 0 false",
        ] {
            let text = support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("@damage {args}"),
            );
            assert!(!text.is_empty());
            assert_eq!(scripts.world().btech, world.btech);
        }
        assert!(
            battle_damage_action(
                &scripts,
                &config,
                visitor,
                unit,
                BattleScenarioSalvo {
                    damage: 11,
                    clusters: 3,
                    rear: false,
                    critical: false,
                }
            )
            .is_err()
        );
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.damage(1, {}, 1000, 4, false, true); error('abort')",
                    unit.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, world.btech);
        assert!(scripts.drain_outbox().is_empty());
        // Permit the first notification, then fail later publication after damage was resolved.
        let path = dir.path().join("stompymux.toml");
        let mut table: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
        table
            .entry("lua")
            .or_insert(toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .unwrap()
            .insert("output_entry_limit".into(), 2.into());
        std::fs::write(path, toml::to_string(&table).unwrap()).unwrap();
        let config = Config::load(dir.path()).unwrap();
        let limited = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let failure = battle_damage_action(
            &limited,
            &config,
            ObjectId(1),
            unit,
            BattleScenarioSalvo {
                damage: 1000,
                clusters: 4,
                rear: false,
                critical: false,
            },
        )
        .unwrap_err();
        assert!(failure.to_string().contains("output limit"), "{failure:#}");
        assert_eq!(limited.world().btech, world.btech);
        assert!(limited.drain_outbox().is_empty());
    }
}

/// Scenario work also accepts unplaced, unpowered units without pilots and respects combat safety.
#[tokio::test]
async fn scenario_packets_work_without_placement_and_preserve_safe_material() {
    for source in firing::templates() {
        let (_dir, config, mut world) = support::isolated_world().await;
        let unit = world.create(&config, "Scenario unit".into(), Kind::Thing);
        world.objects.get_mut(&unit).unwrap().home = Some(ObjectId(config.home()));
        BattleUnitTemplate::parse("test",&source)
            .unwrap()
            .create(&mut world, unit)
            .unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let report = battle_damage_action(
            &scripts,
            &config,
            ObjectId(1),
            unit,
            BattleScenarioSalvo {
                damage: 11,
                clusters: 3,
                rear: false,
                critical: false,
            },
        )
        .unwrap();
        assert_eq!(report.impacts.len(), 3);
        scripts.drain_outbox();
        set_battle_combat_safe(&mut scripts.world_mut(), unit, true).unwrap();
        let before = serde_json::to_value(&scripts.world().btech).unwrap();
        let report = battle_damage_action(
            &scripts,
            &config,
            ObjectId(1),
            unit,
            BattleScenarioSalvo {
                damage: 1000,
                clusters: 1000,
                rear: false,
                critical: true,
            },
        )
        .unwrap();
        assert_eq!(report.impacts.len(), 1000);
        let after = serde_json::to_value(&scripts.world().btech).unwrap();
        let group = if scripts.world().btech.vehicles().contains_key(&unit) {
            "vehicles"
        } else {
            "constructed"
        };
        assert_eq!(
            before[group][unit.0.to_string()]["sections"],
            after[group][unit.0.to_string()]["sections"]
        );
        assert_ne!(
            before[group][unit.0.to_string()]["dice"],
            after[group][unit.0.to_string()]["dice"]
        );
    }
}

/// Damage-induced balance checks stay private and transactional across packet boundaries.
#[tokio::test]
async fn scenario_packets_preserve_private_balance_feedback() {
    let source = include_str!("../game/mechs/JR7-D.toml");
    let (_dir, config, mut baseline, unit, _, _) =
        firing::fixture_with_target(source, Some(BattleWeapon::MediumLaser), source).await;
    baseline
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let passenger = baseline.create(&config, "Passenger".into(), Kind::Player);
    baseline.objects.get_mut(&passenger).unwrap().location = Some(unit);
    baseline
        .objects
        .get_mut(&passenger)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    for seed in 1..=64_u8 {
        let mut state = serde_json::to_value(&baseline.btech).unwrap();
        state["constructed"][unit.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        let mut world = baseline.clone();
        world.btech = serde_json::from_value(state).unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let report = battle_damage_action(
            &scripts,
            &config,
            ObjectId(1),
            unit,
            BattleScenarioSalvo {
                damage: 100,
                clusters: 10,
                rear: false,
                critical: false,
            },
        )
        .unwrap();
        let expected: Vec<_> = report
            .impacts
            .iter()
            .flat_map(|impact| match impact {
                BattleBlastImpact::Mech(impact) => impact.pilot_notices.clone(),
                BattleBlastImpact::Vehicle(_) => Vec::new(),
            })
            .collect();
        if expected.is_empty() {
            continue;
        }
        let output = scripts.drain_outbox();
        let actual: Vec<_> = output
            .iter()
            .filter(|(_, message)| {
                message.source() == "You make a piloting skill roll!"
                    || message.source().starts_with("Modified Pilot Skill:")
            })
            .map(|(who, message)| (*who, message.source().to_owned()))
            .collect();
        assert_eq!(
            actual,
            expected
                .iter()
                .map(|notice| (notice.pilot, notice.text.clone()))
                .collect::<Vec<_>>()
        );
        assert!(actual.iter().all(|(who, _)| *who == ObjectId(1)));
        assert!(output.iter().any(|(who, _)| *who == passenger));
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.damage(1, {}, 100, 10, false, false); error('abort')",
                unit.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, world.btech);
        assert!(lua.drain_outbox().is_empty());
        lua.eval_callback::<()>(&format!(
            "btech.unit.damage(1, {}, 100, 10, false, false)",
            unit.0
        ))
        .unwrap();
        assert_eq!(lua.world().btech, scripts.world().btech);
        assert_eq!(lua.drain_outbox(), output);
        return;
    }
    panic!("fixture must exercise a real damage-induced pilot check");
}
