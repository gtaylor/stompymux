//! Shared cocoon combat intercepts packets, changes aim and survives transactional firing/restart.
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Install deterministic protection through the validated durable state format.
fn protect(world: &mut World, id: ObjectId, mass: i64, seed: u8) {
    firing::edit(world, id, |state| {
        state["orbital_drop"] = serde_json::to_value(OrbitalDrop::new(mass, 2).unwrap()).unwrap();
        state["ground_elevation"] = serde_json::Value::Null;
        state["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
    });
}

/// Pick a protected hit after the diagnostic damage-entry roll.
fn intercept_seed() -> u8 {
    (0..=255)
        .find(|seed| {
            let mut dice = Dice::seeded([*seed; 32]);
            dice.two_d6();
            dice.two_d6() > 8
        })
        .unwrap()
}

/// Expose anatomy-neutral saved material for exact damage and altitude comparisons.
fn state(world: &World, id: ObjectId) -> serde_json::Value {
    let key = if world.btech.vehicles().contains_key(&id) {
        "vehicles"
    } else {
        "constructed"
    };
    serde_json::to_value(&world.btech).unwrap()[key][id.0.to_string()].clone()
}

#[tokio::test]
async fn intercepted_packets_preserve_material_and_breach_without_overflow() {
    for (index, source) in firing::templates().into_iter().take(6).enumerate() {
        let (_dir, config, base, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        for (mass, damage) in [(100 * 1024, 3), (0, 1000)] {
            let mut world = base.clone();
            protect(&mut world, id, mass, intercept_seed());
            let before = state(&world, id);
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let section = if index < 2 { "ct" } else { "f" };
            let call = format!(
                "btech.unit.damage_section(1, {}, '{section}', {damage}, false, false)",
                id.0
            );
            let snapshot = scripts.world().btech.clone();
            assert!(
                scripts
                    .eval_callback::<()>(&format!("{call}; error('abort')"))
                    .is_err()
            );
            assert_eq!(scripts.world().btech, snapshot);
            assert!(scripts.drain_outbox().is_empty());
            scripts
                .eval_callback::<mlua::Table>(&format!("return {call}"))
                .unwrap();
            let after = state(&scripts.world(), id);
            assert_eq!(after["sections"], before["sections"]);
            assert_eq!(after["lost_criticals"], before["lost_criticals"]);
            assert_eq!(after["stagger"], before["stagger"]);
            let notices = scripts
                .drain_outbox()
                .into_iter()
                .map(|(_, doc)| doc.source().to_owned())
                .collect::<Vec<_>>()
                .join("\n");
            assert!(notices.contains("cocoon has been hit"), "{notices}");
            if damage == 3 {
                assert_eq!(after["orbital_drop"]["protection"]["integrity"], 18);
            } else if index == 0 {
                assert_eq!(after["orbital_drop"]["protection"]["state"], "jump_jets");
                assert!(after["free_fall"].is_null());
            } else {
                assert!(after["orbital_drop"].is_null());
                assert!(!after["free_fall"].is_null());
            }
            scripts.world().validate(&config).unwrap();
            let saved = scripts.world().clone();
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                saved.btech
            );
        }
    }
}

#[tokio::test]
async fn firing_opens_cocoons_across_chassis_and_rolls_back_with_the_shot() {
    for (index, source) in firing::templates().into_iter().take(6).enumerate() {
        let (_dir, config, mut world, shooter, target, weapon) =
            firing::fixture_with_target(&source, Some(Weapon::MediumLaser), &source).await;
        protect(&mut world, shooter, 100 * 1024, 42);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let before = scripts.world().btech.clone();
        let call = format!("btech.unit.fire({},1,{weapon},{})", shooter.0, target.0);
        assert!(
            scripts
                .eval_callback::<()>(&format!("{call}; error('abort')"))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
        scripts
            .eval_callback::<mlua::Table>(&format!("return {call}"))
            .unwrap();
        let after = state(&scripts.world(), shooter);
        let notices = scripts
            .drain_outbox()
            .into_iter()
            .map(|(_, doc)| doc.source().to_owned())
            .collect::<Vec<_>>()
            .join("\n");
        if index == 0 {
            assert_eq!(after["orbital_drop"]["protection"]["state"], "jump_jets");
            assert!(notices.contains("opened cocoon"), "{notices}");
        } else {
            assert!(after["orbital_drop"].is_null());
            assert!(!after["free_fall"].is_null());
            assert!(notices.contains("splits open the cocoon"), "{notices}");
        }
        scripts.world().validate(&config).unwrap();
    }
}

#[tokio::test]
async fn intact_target_bonus_is_shared_and_disappears_after_breach() {
    for source in firing::templates().into_iter().take(6) {
        let (_dir, _config, mut world, shooter, target, weapon) =
            firing::fixture_with_target(&source, Some(Weapon::MediumLaser), &source).await;
        protect(&mut world, target, 100 * 1024, 42);
        let rules = AimRules {
            woods_damage: false,
            dig_bonus: 2,
            dig_only_front: false,
            hit_arc_mode: 0,
            fasa_turning: false,
            extended_movement: false,
            extended_ranges: false,
            hotload_half_minimum: false,
            override_weapon_arcs: false,
        };
        let protected = battle_aim_modifiers(&world, shooter, target, weapon, 4, rules).unwrap();
        assert_eq!(protected.orbital_drop, -2);
        firing::edit(&mut world, target, |state| {
            state["orbital_drop"]["protection"] = serde_json::json!({"state":"jump_jets"});
        });
        let opened = battle_aim_modifiers(&world, shooter, target, weapon, 4, rules).unwrap();
        assert_eq!(opened.orbital_drop, 0);
        assert_eq!(
            opened.subtotal().unwrap(),
            protected.subtotal().unwrap() + 2
        );
    }
}

#[tokio::test]
async fn vehicle_internal_packets_intercept_once_and_safe_damage_skips_the_roll() {
    for source in firing::templates().into_iter().skip(2).take(4) {
        let (_dir, config, mut world, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        protect(&mut world, id, 100 * 1024, intercept_seed());
        let before = state(&world, id);
        let mut rules = VehicleCriticalRules {
            rotor_damage_divisor: 0,
            table: VehicleCriticalTable::Standard,
            vtol_table: None,
            enabled: true,
            combat_safe: true,
            toughness: false,
            extended_piloting: false,
        };
        let mut safe = world.clone();
        let safe_report = resolve_battle_vehicle_internal_damage(
            &mut safe,
            id,
            VehicleSection::Front,
            1000,
            rules,
        )
        .unwrap();
        assert_eq!(safe_report.absorbed, 0);
        let mut expected = Dice::seeded([intercept_seed(); 32]);
        expected.two_d6();
        assert_eq!(
            state(&safe, id)["dice"],
            serde_json::to_value(&expected).unwrap()
        );
        assert_eq!(state(&safe, id)["orbital_drop"], before["orbital_drop"]);
        rules.combat_safe = false;
        let report =
            resolve_battle_vehicle_internal_damage(&mut world, id, VehicleSection::Front, 3, rules)
                .unwrap();
        expected.two_d6();
        assert_eq!(
            state(&world, id)["dice"],
            serde_json::to_value(expected).unwrap()
        );
        assert_eq!(state(&world, id)["sections"], before["sections"]);
        assert_eq!(report.absorbed, 0);
        assert!(report.criticals.is_empty());
        assert!(
            report
                .notices
                .iter()
                .any(|n| n.text.contains("cocoon has been hit"))
        );
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn failed_streak_lock_still_opens_the_cocoon() {
    let seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 2)
        .unwrap();
    for source in firing::templates().into_iter().take(6) {
        let (_dir, config, mut world, id, target, weapon) = firing::fixture_with_supply(
            &source,
            Some(Weapon::StreakSrm2),
            &source,
            false,
            Some(""),
        )
        .await;
        protect(&mut world, id, 100 * 1024, seed);
        let before = state(&world, id);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let report: mlua::Table = scripts
            .eval_callback(&format!(
                "return btech.unit.fire({},1,{weapon},{})",
                id.0, target.0
            ))
            .unwrap();
        let report = serde_json::to_value(report).unwrap();
        assert_eq!(report["launched"], false);
        let after = state(&scripts.world(), id);
        assert_ne!(after["orbital_drop"], before["orbital_drop"]);
        assert_eq!(after["ammunition"], before["ammunition"]);
        let text = scripts
            .drain_outbox()
            .into_iter()
            .map(|(_, doc)| doc.source().to_owned())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("cocoon"), "{text}");
        assert!(text.contains("fails to lock"), "{text}");
        scripts.world().validate(&config).unwrap();
    }
}

#[tokio::test]
async fn coordinate_launches_publish_the_same_cocoon_breach() {
    for source in firing::templates().into_iter().take(6) {
        let (_dir, config, mut world, id, _, weapon) =
            firing::fixture_with_target(&source, Some(Weapon::MediumLaser), &source).await;
        protect(&mut world, id, 100 * 1024, 42);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let report: mlua::Table = scripts
            .eval_callback(&format!(
                "return btech.unit.fire({},1,{weapon},{{x=0,y=9}})",
                id.0
            ))
            .unwrap();
        let report = serde_json::to_value(report).unwrap();
        assert_eq!(report["launched"], true);
        assert!(!report["launch_notices"].as_array().unwrap().is_empty());
        let text = scripts
            .drain_outbox()
            .into_iter()
            .map(|(_, doc)| doc.source().to_owned())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("cocoon"), "{text}");
        scripts.world().validate(&config).unwrap();
    }
}
