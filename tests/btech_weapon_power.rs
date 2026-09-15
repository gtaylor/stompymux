//! Gauss power-down shares native/Lua controls, material safety and durable state across chassis.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod btech_firing;
mod support;
use btech_firing::{edit, fixture_with_supply, fixture_with_target, templates};

/// Observe readiness through the chassis boundary without duplicating control rules.
fn readiness(world: &World, id: ObjectId, index: usize) -> BattleWeaponReadiness {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return unit.weapon_readiness(index).unwrap();
    }
    world.btech.constructed_units()[&id]
        .weapon_readiness(index)
        .unwrap()
}

/// Every Gauss family retains material and ammunition while becoming unfireable through both adapters.
#[tokio::test]
async fn gauss_power_down_adapters_state_and_restart() {
    for source in templates() {
        for weapon in [
            BattleWeapon::GaussRifle,
            BattleWeapon::ClanGaussRifle,
            BattleWeapon::LightGaussRifle,
            BattleWeapon::MagshotGaussRifle,
            BattleWeapon::HeavyGaussRifle,
        ] {
            let (_dir, config, base, id, _target, index) = fixture_with_supply(
                &source,
                Some(weapon),
                include_str!("../game/mechs/JR7-D"),
                false,
                Some(""),
            )
            .await;
            let native = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
            let lua = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
            let before = serde_json::to_value(&native.world().btech).unwrap();
            let call = format!("btech.unit.disable({},1,{index})", id.0);
            assert!(
                lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                    .is_err()
            );
            assert_eq!(lua.world().btech, native.world().btech);
            assert!(lua.drain_outbox().is_empty());
            let output = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("disable {index}"),
            );
            assert!(
                output.contains(&format!("You power down weapon {index}.")),
                "{output}"
            );
            assert!(
                lua.eval_callback::<bool>(&format!("return {call}"))
                    .unwrap()
            );
            assert_eq!(lua.world().btech, native.world().btech);
            let mut expected = before;
            let group = if base.btech.vehicles().contains_key(&id) {
                "vehicles"
            } else {
                "constructed"
            };
            expected[group][id.0.to_string()]["powered_down_weapons"] = serde_json::json!([index]);
            assert_eq!(
                serde_json::to_value(&native.world().btech).unwrap(),
                expected
            );
            assert!(!readiness(&native.world(), id, index).ready);
            assert_eq!(
                battle_weapon_diagnostics(&native.world(), id).unwrap()[index].condition,
                BattleEquipmentCondition::Disabled
            );
            let section = if group == "vehicles" { "front" } else { "lt" };
            let report = battle_critical_report(&native.world(), id, section, false).unwrap();
            assert!(
                report
                    .slots
                    .iter()
                    .filter(|slot| slot.weapon_index == Some(index))
                    .all(|slot| slot.condition == BattleEquipmentCondition::Disabled)
            );
            let disabled = native.world().btech.clone();
            let output =
                support::run_text(&native, &config, ObjectId(1), 1, &format!("fire {index}"));
            assert!(output.contains("destroyed"), "{output}");
            assert_eq!(native.world().btech, disabled);
            let mut world = native.world().clone();
            stop_battle_unit(
                &mut world,
                id,
                ObjectId(1),
                BattleFallRules::configured(&config),
            )
            .unwrap();
            assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
            start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
            for _ in 0..70 {
                advance_battle_units(&mut world, 0);
            }
            assert!(!readiness(&world, id, index).ready);
            persistence::save(&config.database(), &world).await.unwrap();
            let loaded = persistence::load(&config.database()).await.unwrap();
            assert_eq!(loaded.btech, world.btech);
            assert!(!readiness(&loaded, id, index).ready);
            if let Some(unit) = loaded.btech.constructed_units().get(&id) {
                let mut damaged = unit.clone();
                let slot = damaged.loadout().unwrap().weapons[index].criticals[0];
                assert_eq!(
                    damaged.destroy_critical(slot).unwrap(),
                    Some(BattleCriticalLoss::Weapon {
                        index,
                        explosion_damage: 0
                    })
                );
                assert!(!damaged.weapon_intact(index).unwrap());
            }
        }
    }
}

/// Mechanical rejection and malformed saved identities cannot alter a valid unit.
#[tokio::test]
async fn gauss_power_down_guards_and_validation() {
    for source in templates() {
        // Keep a non-Gauss mount when the front-mounted VTOL armament is replaced.
        let source = source.replace(
            "Left_Side\n",
            "Left_Side\n  CRIT_12 { IS.MediumLaser - - }\n",
        );
        let (_dir, config, base, id, _, index) = fixture_with_target(
            &source,
            Some(BattleWeapon::MagshotGaussRifle),
            include_str!("../game/mechs/JR7-D"),
        )
        .await;
        for change in ["recycle", "off", "absent"] {
            let mut world = base.clone();
            match change {
                "recycle" => edit(&mut world, id, |state| {
                    state["weapon_recycle"] = serde_json::json!({index.to_string(): 3})
                }),
                "off" => {
                    stop_battle_unit(
                        &mut world,
                        id,
                        ObjectId(1),
                        BattleFallRules::configured(&config),
                    )
                    .unwrap();
                }
                _ => {
                    stop_battle_unit(
                        &mut world,
                        id,
                        ObjectId(1),
                        BattleFallRules::configured(&config),
                    )
                    .unwrap();
                    remove_battle_unit(&mut world, id, ObjectId(config.home())).unwrap();
                }
            }
            let before = world.btech.clone();
            assert!(disable_gauss_weapon(&mut world, id, ObjectId(1), index).is_err());
            assert_eq!(world.btech, before);
        }
        let mut world = base.clone();
        assert!(disable_gauss_weapon(&mut world, id, ObjectId(2), index).is_err());
        assert!(disable_gauss_weapon(&mut world, id, ObjectId(1), usize::MAX).is_err());
        let other = battle_weapon_diagnostics(&world, id)
            .unwrap()
            .into_iter()
            .find(|row| row.weapon.weapon_explosion_damage() == 0)
            .unwrap()
            .index;
        assert!(disable_gauss_weapon(&mut world, id, ObjectId(1), other).is_err());
        assert_eq!(world.btech, base.btech);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let output = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("disable {other},{index},{index}"),
        );
        assert!(output.contains("only disable Gauss"), "{output}");
        assert!(output.contains("power down"), "{output}");
        assert_eq!(output.matches("You power down weapon").count(), 2);
        scripts.world().validate(&config).unwrap();
        let mut invalid = serde_json::to_value(&base.btech).unwrap();
        let group = if base.btech.vehicles().contains_key(&id) {
            "vehicles"
        } else {
            "constructed"
        };
        invalid[group][id.0.to_string()]["powered_down_weapons"] = serde_json::json!([other]);
        let decoded = serde_json::from_value(invalid);
        if let Ok(state) = decoded {
            let mut candidate = base.clone();
            candidate.btech = state;
            assert!(candidate.validate(&config).is_err());
        }
    }
}

/// A critical still destroys the powered-down installation, without explosion damage or crew injury.
#[tokio::test]
async fn powered_down_vehicle_gauss_critical_is_inert() {
    for source in templates().into_iter().skip(2) {
        let (_dir, config, mut base, id, _, index) = fixture_with_supply(
            &source,
            Some(BattleWeapon::MagshotGaussRifle),
            include_str!("../game/mechs/JR7-D"),
            false,
            Some(""),
        )
        .await;
        disable_gauss_weapon(&mut base, id, ObjectId(1), index).unwrap();
        let rules = BattleVehicleCriticalRules {
            rotor_damage_divisor: 0,
            extended_piloting: false,
            vtol_table: None,
            table: BattleVehicleCriticalTable::Advanced,
            enabled: true,
            combat_safe: false,
            toughness: false,
        };
        if base.btech.vehicles()[&id].definition().movement == BattleVehicleMovement::Stationary {
            let result =
                resolve_battle_vehicle_critical(&mut base, id, BattleVehicleSection::Front, rules)
                    .unwrap();
            assert_eq!(result.selection.effect, None);
            continue;
        }
        let mut checked = false;
        for seed in 0..=255 {
            let mut world = base.clone();
            edit(&mut world, id, |state| {
                state["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
            });
            let before = world.btech.vehicles()[&id].sections().clone();
            let result =
                resolve_battle_vehicle_critical(&mut world, id, BattleVehicleSection::Front, rules)
                    .unwrap();
            if result.selection.effect != Some(BattleVehicleCriticalEffect::WeaponDestroyed) {
                continue;
            }
            assert!(result.internal_damage.is_empty());
            assert_eq!(world.btech.vehicles()[&id].sections(), &before);
            let location =
                world.btech.vehicles()[&id].loadout().unwrap().weapons[index].criticals[0];
            assert!(world.btech.vehicles()[&id].critical_destroyed(location));
            assert_eq!(world.btech.vehicles()[&id].pilot_injuries(), 0);
            world.validate(&config).unwrap();
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
            checked = true;
            break;
        }
        assert!(checked, "No weapon-destruction result exercised");
    }
}

/// Compact disabled conditions use the same explosion suppression as cockpit power-down.
#[tokio::test]
async fn mech_damage_failure_codes_preserve_gauss_explosion_rules() {
    for source in templates().into_iter().take(2) {
        for weapon in [
            BattleWeapon::GaussRifle,
            BattleWeapon::ClanGaussRifle,
            BattleWeapon::LightGaussRifle,
            BattleWeapon::MagshotGaussRifle,
            BattleWeapon::HeavyGaussRifle,
        ] {
            let (_dir, config, world, id, _, index) =
                fixture_with_target(&source, Some(weapon), &source).await;
            for code in 1..=7 {
                let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
                set_battle_unit_field_action(
                    &scripts,
                    &config,
                    ObjectId(1),
                    id,
                    "mechdamage",
                    &format!("G:2/0({code})"),
                )
                .unwrap();
                let before =
                    serde_json::to_value(&scripts.world().btech.constructed_units()[&id]).unwrap();
                let mut unit = scripts.world().btech.constructed_units()[&id].clone();
                let location = unit.loadout().unwrap().weapons[index].criticals[0];
                assert_eq!(
                    unit.destroy_critical(location).unwrap(),
                    Some(BattleCriticalLoss::Weapon {
                        index,
                        explosion_damage: if code == 5 {
                            0
                        } else {
                            weapon.weapon_explosion_damage()
                        },
                    })
                );
                assert!(!unit.weapon_intact(index).unwrap());
                assert!(unit.weapon_failures().is_empty());
                assert_eq!(serde_json::to_value(&unit).unwrap()["dice"], before["dice"]);
                let mut validation = scripts.world().clone();
                edit(&mut validation, id, |state| {
                    *state = serde_json::to_value(&unit).unwrap()
                });
                validation.validate(&config).unwrap();
            }
        }
    }
}
