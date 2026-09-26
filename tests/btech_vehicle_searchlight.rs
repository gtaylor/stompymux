//! All chassis share searchlight switching, beam geometry and persistence; exposure follows anatomy.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Add installed lighting without changing the fixture's other technologies.
fn with_lamp(source: &str) -> String {
    let mut found = false;
    let mut lines: Vec<_> = source
        .lines()
        .map(|line| {
            if line.split_whitespace().next() == Some("Specials") {
                found = true;
                line.replacen('{', "{ Searchlight ", 1)
            } else {
                line.into()
            }
        })
        .collect();
    if !found {
        lines.insert(0, "Specials { Searchlight }".into());
    }
    lines.join("\n")
}

/// Switching and illumination use shared native/Lua controls across ground and rotorcraft anatomy.
#[tokio::test]
async fn shared_lamps_switch_light_units_and_terrain_and_resume_after_restart() {
    for source in firing::templates() {
        let source = with_lamp(&source);
        let (_dir, config, world, id, target, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        assert!(support::run_text(&native, &config, ObjectId(1), 1, "slite").contains("warm up"));
        lua.eval_callback::<()>(&format!("btech.unit.slite({},1)", id.0))
            .unwrap();
        assert_eq!(native.world().btech, lua.world().btech);
        let before = native.world().btech.clone();
        assert!(support::run_text(&native, &config, ObjectId(1), 1, "slite").contains("already"));
        assert_eq!(native.world().btech, before);
        assert!(
            lua.eval_callback::<()>(&format!("btech.unit.slite({},2)", id.0))
                .is_err()
        );
        assert_eq!(lua.world().btech, before);
        let mut world = native.world().clone();
        assert!(!battle_unit_illuminated(&world, target));
        for _ in 0..2 {
            assert!(advance_battle_searchlights(&mut world).is_empty());
        }
        persistence::save(&config.database(), &world).await.unwrap();
        world = persistence::load(&config.database()).await.unwrap();
        for _ in 0..2 {
            assert!(advance_battle_searchlights(&mut world).is_empty());
        }
        let notices = advance_battle_searchlights(&mut world);
        assert!(
            notices
                .iter()
                .any(|n| n.unit == id && n.text.contains("full power"))
        );
        assert!(advance_battle_searchlights(&mut world).is_empty());
        assert!(battle_unit_illuminated(&world, id));
        assert!(battle_unit_illuminated(&world, target));
        let map = world.objects[&id].location.unwrap();
        assert!(battle_hex_illuminated(&world, map, BattleHexCoordinate { x: 0, y: 9 }).unwrap());
        assert!(
            battle_unit_status(&world, id, "")
                .unwrap()
                .contains("SEARCHLIGHT ON")
        );
        if world
            .btech
            .vehicles()
            .get(&id)
            .is_none_or(|unit| unit.definition().movement != BattleVehicleMovement::Stationary)
        {
            firing::edit(&mut world, id, |unit| {
                unit["motion"]["heading"] = serde_json::json!(180.0)
            });
            assert!(!battle_unit_illuminated(&world, target));
            assert!(
                !battle_hex_illuminated(&world, map, BattleHexCoordinate { x: 0, y: 9 }).unwrap()
            );
        }
        let notices = stop_battle_unit(
            &mut world,
            id,
            ObjectId(1),
            BattleMovementRules::STANDARD.fall,
        )
        .unwrap();
        assert!(
            notices
                .iter()
                .any(|n| n.text == "Your searchlight shuts off.")
        );
        assert!(!battle_unit_illuminated(&world, id));
        assert!(
            battle_unit_status(&world, id, "")
                .unwrap()
                .contains("SLITE([fg=green]Off[reset])")
        );
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

/// Front ground-vehicle hits expose lamps; other faces, VTOLs and combat-safe hits do not.
#[tokio::test]
async fn vehicle_lamp_damage_shares_rolls_and_cancels_pending_switches() {
    for source in firing::templates().into_iter().skip(2) {
        let source = with_lamp(&source);
        let (_dir, config, mut world, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        firing::edit(&mut world, id, |unit| {
            unit["searchlight"] = serde_json::json!({"on":true,"destroyed":false,"remaining":3})
        });
        let rules = BattleVehicleCriticalRules {
            rotor_damage_divisor: 0,
            extended_piloting: false,
            vtol_table: None,
            table: BattleVehicleCriticalTable::Advanced,
            enabled: false,
            combat_safe: false,
            toughness: false,
        };
        let hit = BattleVehicleArmorHit {
            section: BattleVehicleSection::Front,
            amount: 1,
            through_armor_critical: false,
            armor_piercing: None,
        };
        let is_vtol = world.btech.vehicles()[&id].definition().is_vtol();
        let mut destroyed = None;
        for seed in 0..32 {
            let mut candidate = world.clone();
            firing::edit(&mut candidate, id, |unit| {
                unit["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
            });
            let mut replay = candidate.clone();
            let report =
                resolve_battle_vehicle_armor_damage(&mut candidate, id, hit, rules).unwrap();
            assert_eq!(
                resolve_battle_vehicle_armor_damage(&mut replay, id, hit, rules).unwrap(),
                report
            );
            assert_eq!(candidate.btech, replay.btech);
            if candidate.btech.vehicles()[&id].searchlight().destroyed {
                destroyed = Some(candidate);
                break;
            }
        }
        assert_eq!(destroyed.is_some(), !is_vtol);
        if let Some(mut destroyed) = destroyed {
            assert_eq!(
                destroyed.btech.vehicles()[&id].searchlight(),
                BattleSearchlight {
                    destroyed: true,
                    on: false,
                    remaining: 0
                }
            );
            assert!(toggle_battle_searchlight(&mut destroyed, id, ObjectId(1)).is_err());
            assert!(advance_battle_searchlights(&mut destroyed).is_empty());
            persistence::save(&config.database(), &destroyed)
                .await
                .unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                destroyed.btech
            );
        }
        for (section, safe) in [
            (BattleVehicleSection::Left, false),
            (BattleVehicleSection::Front, true),
        ] {
            let mut candidate = world.clone();
            let report = resolve_battle_vehicle_armor_damage(
                &mut candidate,
                id,
                BattleVehicleArmorHit { section, ..hit },
                BattleVehicleCriticalRules {
                    combat_safe: safe,
                    ..rules
                },
            )
            .unwrap();
            assert!(
                report
                    .notices
                    .iter()
                    .all(|notice| !notice.text.contains("searchlight"))
            );
            assert_eq!(
                candidate.btech.vehicles()[&id].searchlight(),
                world.btech.vehicles()[&id].searchlight()
            );
        }
    }
}
