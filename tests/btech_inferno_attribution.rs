//! Initial inferno consequences retain the shooter; later fire pulses are self-attributed.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Read unit-owned totals independently of the attack result.
fn field(world: &World, id: ObjectId, name: &str) -> serde_json::Value {
    let owner = if world.btech.vehicles().contains_key(&id) {
        "vehicles"
    } else {
        "constructed"
    };
    serde_json::to_value(&world.btech).unwrap()[owner][id.0.to_string()][name].clone()
}

/// Standard heat explosions and advanced first pulses retain attribution through both launchers.
#[tokio::test]
async fn inferno_initial_effects_credit_the_shooter_and_replay() {
    let targets = firing::templates();
    for shooter in [&targets[0], &targets[2]] {
        for target_source in targets.iter().skip(2) {
            for advanced in [false, true] {
                let (dir, _, mut base, id, target, index) = firing::fixture_with_supply(
                    shooter,
                    Some(BattleWeapon::Srm2),
                    target_source,
                    false,
                    Some("Inferno"),
                )
                .await;
                let path = dir.path().join("stompymux.toml");
                let mut settings: toml::Value =
                    toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
                settings["battletech"]
                    .as_table_mut()
                    .unwrap()
                    .insert("fasaadvvhlfire".into(), i64::from(advanced).into());
                std::fs::write(path, toml::to_string(&settings).unwrap()).unwrap();
                let config = Config::load(dir.path()).unwrap();
                toggle_battle_inferno(&mut base, id, ObjectId(1), index).unwrap();
                let seed = (0..=255)
                    .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
                    .unwrap();
                firing::edit(&mut base, id, |unit| {
                    unit["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
                });
                let seed = (0..=255)
                    .find(|seed| {
                        let mut dice = BattleDice::seeded([*seed; 32]);
                        dice.two_d6();
                        dice.two_d6() == 9
                    })
                    .unwrap();
                firing::edit(&mut base, target, |unit| {
                    unit["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
                });
                let vtol = base.btech.vehicles()[&target].definition().is_vtol();
                let stationary = base.btech.vehicles()[&target].definition().movement
                    == BattleVehicleMovement::Stationary;
                let command = format!("btech.unit.fire({},1,{index},{})", id.0, target.0);
                let lua = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
                assert!(
                    lua.eval_callback::<()>(&format!("{command}; error('abort')"))
                        .is_err()
                );
                assert!(lua.world().btech == base.btech);
                assert!(lua.drain_outbox().is_empty());
                let report: mlua::Table = lua.eval_callback(&format!("return {command}")).unwrap();
                let report = serde_json::to_value(report).unwrap();
                let inferno = &report["salvo"]["report"]["inferno"];
                assert!(inferno["missiles"].as_u64().unwrap() > 0);
                let inflicted: u64 = inferno["damage"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|damage| damage["armor_damage"].as_u64().unwrap())
                    .sum();
                if !advanced && !stationary {
                    assert_eq!(inferno["explosion_roll"], 9, "{inferno}");
                }
                assert_eq!(
                    field(&lua.world(), id, "damage_counters")["inflicted"],
                    inflicted
                );
                assert_eq!(
                    field(&lua.world(), id, "units_killed"),
                    i32::from(!stationary && (!advanced || vtol))
                );
                if advanced && vtol {
                    // This seed penetrates the rotor and rolls a power-plant
                    // catastrophe, which must not add another kill per hull face.
                    let rotor = inferno["damage"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|d| d["section"] == "rotor")
                        .unwrap();
                    assert_eq!(
                        rotor["internal"]["criticals"][0]["selection"]["effect"],
                        "power_plant"
                    );
                }
                if !stationary {
                    let overflow =
                        Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
                    let field_name = if advanced {
                        "damage_inflicted"
                    } else {
                        "units_killed"
                    };
                    set_battle_unit_field_action(
                        &overflow,
                        &config,
                        ObjectId(1),
                        id,
                        field_name,
                        "2147483647",
                    )
                    .unwrap();
                    overflow.drain_outbox();
                    let before = overflow.world().btech.clone();
                    let error = overflow.eval_callback::<()>(&command).unwrap_err();
                    assert!(format!("{error:#}").contains("counter overflow"));
                    assert!(overflow.world().btech == before);
                    assert!(overflow.drain_outbox().is_empty());
                }
                let native = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
                support::run_text(
                    &native,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("fire {index} #{}", target.0),
                );
                assert!(native.world().btech == lua.world().btech);
                let saved = lua.world().clone();
                persistence::save(&config.database(), &saved).await.unwrap();
                let restored = persistence::load(&config.database()).await.unwrap();
                assert_eq!(
                    field(&restored, id, "units_killed"),
                    field(&saved, id, "units_killed")
                );
                assert_eq!(
                    field(&restored, id, "damage_counters"),
                    field(&saved, id, "damage_counters")
                );
                if advanced && !stationary && !vtol {
                    let mut burning = restored;
                    for _ in 0..60 {
                        let _ = advance_battle_vehicle_fires(&mut burning, &config).unwrap();
                    }
                    assert!(
                        field(&burning, target, "damage_counters")["taken"]
                            .as_i64()
                            .unwrap()
                            > field(&saved, target, "damage_counters")["taken"]
                                .as_i64()
                                .unwrap()
                    );
                    assert_eq!(
                        field(&burning, id, "damage_counters"),
                        field(&saved, id, "damage_counters")
                    );
                    assert_eq!(
                        field(&burning, id, "units_killed"),
                        field(&saved, id, "units_killed")
                    );
                }
            }
        }
    }
}
