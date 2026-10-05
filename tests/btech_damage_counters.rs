//! Damage totals follow initial packet admission and preserve attacker attribution across chassis.
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Inspect the durable owner without invoking another gameplay action.
fn counters(world: &World, id: ObjectId) -> serde_json::Value {
    let key = if world.btech.vehicles().contains_key(&id) {
        "vehicles"
    } else {
        "constructed"
    };
    serde_json::to_value(&world.btech).unwrap()[key][id.0.to_string()]["damage_counters"].clone()
}

/// Physical attackers receive the same accounting when the next chassis family is the target.
#[tokio::test]
async fn direct_damage_attribution_replays_and_rolls_back_for_mixed_chassis() {
    let sources = firing::templates();
    let seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 12)
        .unwrap();
    for (i, source) in sources.iter().enumerate() {
        let (_dir, config, mut world, id, target, index) = firing::fixture_with_target(
            source,
            Some(Weapon::MediumLaser),
            &sources[(i + 1) % sources.len()],
        )
        .await;
        firing::edit(&mut world, id, |unit| {
            unit["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap()
        });
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let command = format!("btech.unit.fire({},1,{index},{})", id.0, target.0);
        let before = serde_json::to_value(&scripts.world().btech).unwrap();
        assert!(
            scripts
                .eval_callback::<()>(&format!("{command}; error('abort')"))
                .is_err()
        );
        assert!(serde_json::to_value(&scripts.world().btech).unwrap() == before);
        assert!(scripts.drain_outbox().is_empty());
        let glancing: bool = scripts
            .eval_callback(&format!("local r={command}; return r.glancing"))
            .unwrap();
        let damage = if glancing { 3 } else { 5 };
        assert_eq!(
            counters(&scripts.world(), id),
            serde_json::json!({"taken":0,"inflicted":damage})
        );
        assert_eq!(
            counters(&scripts.world(), target),
            serde_json::json!({"taken":damage,"inflicted":0})
        );
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(counters(&restored, id), counters(&saved, id));
        assert_eq!(counters(&restored, target), counters(&saved, target));
    }
}

/// Standalone damage is self-attributed and counts incoming damage only once through penetration.
#[tokio::test]
async fn located_packets_count_overflow_once_and_combat_safe_counts_nothing() {
    for source in firing::templates() {
        let (_dir, config, world, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        for safe in [false, true] {
            let mut candidate = world.clone();
            firing::edit(&mut candidate, id, |unit| {
                unit["combat_safe"] = safe.into();
                for rounds in unit["ammunition"].as_array_mut().unwrap() {
                    *rounds = 0.into();
                }
            });
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(candidate))).unwrap();
            let section = if scripts.world().btech.vehicles().contains_key(&id) {
                "Front"
            } else {
                scripts.world().btech.constructed_units()[&id]
                    .chassis()
                    .section_name(MechSection::LeftArm)
            };
            battle_damage_section_action(
                &scripts,
                &config,
                ObjectId(1),
                id,
                ScenarioHit {
                    section,
                    damage: 200,
                    rear: false,
                    critical: false,
                },
            )
            .unwrap();
            assert_eq!(
                counters(&scripts.world(), id),
                serde_json::json!({"taken":if safe {0} else {200},"inflicted":0})
            );
        }
    }
}

/// Rotor scaling precedes accounting; hardened armor and internal reinforcement follow it.
/// VTOLs cannot mount hardened armor, so the rotor case checks reinforcement alone.
#[tokio::test]
async fn accounting_uses_the_admitted_packet_before_material_reductions() {
    for rotor in [false, true] {
        let source = if rotor {
            crate::support::templates::with_flags(
                include_str!("../game/units/Kestrel.toml"),
                &["ReinforcedInternal_Tech"],
            )
        } else {
            crate::support::templates::with_flags(
                include_str!("../game/units/Demolisher.toml"),
                &["HardenedArmor_Tech", "ReinforcedInternal_Tech"],
            )
        };
        let (_dir, _, mut world, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let mut rules = VehicleImpactRules::STANDARD.criticals;
        rules.enabled = false;
        rules.rotor_damage_divisor = 3;
        let report = resolve_battle_vehicle_armor_damage(
            &mut world,
            id,
            VehicleArmorHit {
                damage_class: DamageClass::Ordinary,
                section: if rotor {
                    VehicleSection::Rotor
                } else {
                    VehicleSection::Front
                },
                amount: 9,
                through_armor_critical: false,
                armor_piercing: None,
            },
            rules,
        )
        .unwrap();
        let incoming = if rotor { 3 } else { 9 };
        assert_eq!(
            counters(&world, id),
            serde_json::json!({"taken":incoming,"inflicted":0})
        );
        assert_eq!(report.armor_damage, if rotor { 3 } else { 5 });
        let internal =
            resolve_battle_vehicle_internal_damage(&mut world, id, VehicleSection::Rear, 5, rules)
                .unwrap();
        assert_eq!(internal.structural_damage, 3);
        assert_eq!(
            counters(&world, id),
            serde_json::json!({"taken":incoming+5,"inflicted":0})
        );
    }
}

/// A rejected counter increment restores both participants, material and dice.
#[tokio::test]
async fn counter_overflow_rolls_back_the_damage_transaction() {
    for source in firing::templates() {
        let (_dir, config, world, id, target, index) =
            firing::fixture_with_target(&source, Some(Weapon::MediumLaser), &source).await;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let seed = (0..=255)
            .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 12)
            .unwrap();
        firing::edit(&mut scripts.world_mut(), id, |unit| {
            unit["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap()
        });
        set_battle_unit_field_action(
            &scripts,
            &config,
            ObjectId(1),
            target,
            "damage_taken",
            "2147483647",
        )
        .unwrap();
        scripts.drain_outbox();
        let before = serde_json::to_value(&scripts.world().btech).unwrap();
        let error = scripts
            .eval_callback::<()>(&format!("btech.unit.fire({},1,{index},{})", id.0, target.0))
            .unwrap_err();
        assert!(
            format!("{error:#}").contains("Damage counter overflow"),
            "{error:#}"
        );
        assert!(serde_json::to_value(&scripts.world().btech).unwrap() == before);
        assert!(scripts.drain_outbox().is_empty());
    }
}

/// Redirecting a new hit out of an already destroyed Mech limb uses continuation accounting.
#[tokio::test]
async fn destroyed_limb_redirection_does_not_count_a_second_initial_packet() {
    for source in firing::templates().into_iter().take(2) {
        let (_dir, config, world, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let section = world.btech.constructed_units()[&id]
            .chassis()
            .section_name(MechSection::LeftArm);
        let limb = &world.btech.constructed_units()[&id].sections()[&MechSection::LeftArm];
        let amount = i32::from(limb.armor) + i32::from(limb.internal);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        battle_damage_section_action(
            &scripts,
            &config,
            ObjectId(1),
            id,
            ScenarioHit {
                section,
                damage: amount,
                rear: false,
                critical: false,
            },
        )
        .unwrap();
        assert_eq!(
            scripts.world().btech.constructed_units()[&id].sections()[&MechSection::LeftArm]
                .internal,
            0
        );
        let before = counters(&scripts.world(), id);
        let torso = scripts.world().btech.constructed_units()[&id].sections()
            [&MechSection::LeftTorso]
            .armor;
        battle_damage_section_action(
            &scripts,
            &config,
            ObjectId(1),
            id,
            ScenarioHit {
                section,
                damage: 1,
                rear: false,
                critical: false,
            },
        )
        .unwrap();
        assert_eq!(
            scripts.world().btech.constructed_units()[&id].sections()[&MechSection::LeftTorso]
                .armor,
            torso - 1
        );
        assert_eq!(counters(&scripts.world(), id), before);
    }
}
