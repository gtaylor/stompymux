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

/// Inferno heat explosions retain attribution through both launchers.
#[tokio::test]
async fn inferno_initial_effects_credit_the_shooter_and_replay() {
    let targets = firing::templates();
    for shooter in [&targets[0], &targets[2]] {
        for target_source in targets.iter().skip(2) {
            {
                let (_dir, config, mut base, id, target, index) = firing::fixture_with_supply(
                    shooter,
                    Some(Weapon::Srm2),
                    target_source,
                    false,
                    Some("Inferno"),
                )
                .await;
                toggle_battle_inferno(&mut base, id, ObjectId(1), index).unwrap();
                let seed = (0..=255)
                    .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 12)
                    .unwrap();
                firing::edit(&mut base, id, |unit| {
                    unit["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap()
                });
                let seed = (0..=255)
                    .find(|seed| {
                        let mut dice = Dice::seeded([*seed; 32]);
                        dice.two_d6();
                        dice.two_d6() == 9
                    })
                    .unwrap();
                firing::edit(&mut base, target, |unit| {
                    unit["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap()
                });
                let stationary = base.btech.vehicles()[&target].definition().movement
                    == VehicleMovement::Stationary;
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
                let missiles = inferno["missiles"].as_u64().unwrap();
                assert!(missiles > 0);
                // Infernos deal no armor damage; each missile rolls once on the vehicle's
                // critical hits table (Total Warfare p. 142).
                if !stationary {
                    assert_eq!(
                        inferno["criticals"].as_array().unwrap().len() as u64,
                        missiles,
                        "{inferno}"
                    );
                }
                assert_eq!(field(&lua.world(), id, "damage_counters")["inflicted"], 0);
                let killed = lua.world().btech.vehicles()[&target].is_destroyed();
                assert_eq!(field(&lua.world(), id, "units_killed"), i32::from(killed));
                if killed {
                    let overflow =
                        Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
                    set_battle_unit_field_action(
                        &overflow,
                        &config,
                        ObjectId(1),
                        id,
                        "units_killed",
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
            }
        }
    }
}
