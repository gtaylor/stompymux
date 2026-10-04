//! Kill totals commit with the first attributed destruction across both chassis stores.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Read the durable owner directly, independently of field rendering.
fn kills(world: &World, id: ObjectId) -> i64 {
    let owner = if world.btech.vehicles().contains_key(&id) {
        "vehicles"
    } else {
        "constructed"
    };
    serde_json::to_value(&world.btech).unwrap()[owner][id.0.to_string()]["units_killed"]
        .as_i64()
        .unwrap()
}

/// Both construction stores expose the same lifecycle predicate.
fn destroyed(world: &World, id: ObjectId) -> bool {
    world.btech.constructed_units().get(&id).map_or_else(
        || world.btech.vehicles()[&id].is_destroyed(),
        |unit| unit.is_destroyed(),
    )
}

/// A penetrating lethal shot awards once, with command agreement, restart and atomic overflow.
#[tokio::test]
async fn lethal_shots_credit_once_across_chassis_and_rollback() {
    let sources = firing::templates();
    let hit_seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
        .unwrap();
    for (i, source) in sources.iter().enumerate() {
        let (_dir, config, mut base, id, target, index) = firing::fixture_with_target(
            source,
            Some(BattleWeapon::MediumLaser),
            &sources[(i + 1) % sources.len()],
        )
        .await;
        firing::edit(&mut base, id, |unit| {
            unit["dice"] = serde_json::to_value(BattleDice::seeded([hit_seed; 32])).unwrap();
            unit["units_killed"] = 4.into();
        });
        let vehicle = base.btech.vehicles().contains_key(&target);
        firing::edit(&mut base, target, |unit| {
            for (name, section) in unit["sections"].as_object_mut().unwrap() {
                // Keep legs intact: losing a fragile leg can instead cause a
                // lethal self-attributed fall before the shot transfers inward.
                let lethal_section = if vehicle {
                    !matches!(name.as_str(), "Turret" | "Rotor")
                } else {
                    name == "CenterTorso"
                };
                if !lethal_section {
                    continue;
                }
                section["armor"] = 0.into();
                section["rear"] = 0.into();
                section["internal"] = 1.into();
            }
            for rounds in unit["ammunition"].as_array_mut().unwrap() {
                *rounds = 0.into();
            }
        });
        assert!(!destroyed(&base, target));
        let command = format!("btech.unit.fire({},1,{index},{})", id.0, target.0);
        // Find a lethal location rather than assuming rotor and turret hits kill hulls.
        let mut chosen = None;
        for seed in 0..32 {
            firing::edit(&mut base, target, |unit| {
                unit["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
            });
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
            scripts.eval_callback::<()>(&command).unwrap();
            if destroyed(&scripts.world(), target) {
                chosen = Some(scripts);
                break;
            }
            assert_eq!(kills(&scripts.world(), id), 4);
        }
        let lua = chosen.expect("seed matrix must include a lethal hull or Mech hit");
        assert_eq!(kills(&lua.world(), id), 5, "shooter family {i}");
        assert_eq!(kills(&lua.world(), target), 0);
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
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(kills(&loaded, id), 5);
        assert_eq!(kills(&loaded, target), 0);
        let abort = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
        assert!(
            abort
                .eval_callback::<()>(&format!("{command}; error('abort')"))
                .is_err()
        );
        assert!(abort.world().btech == base.btech);
        assert!(abort.drain_outbox().is_empty());
        set_battle_unit_field_action(
            &abort,
            &config,
            ObjectId(1),
            id,
            "units_killed",
            "2147483647",
        )
        .unwrap();
        abort.drain_outbox();
        let before = abort.world().btech.clone();
        let error = abort.eval_callback::<()>(&command).unwrap_err();
        assert!(
            format!("{error:#}").contains("Kill counter overflow"),
            "{error:#}"
        );
        assert!(abort.world().btech == before);
        assert!(abort.drain_outbox().is_empty());
    }
}

/// Signed field edits retain bounds and environmental destruction never awards a kill.
#[tokio::test]
async fn signed_fields_and_self_destruction_share_the_unit_owner() {
    for source in firing::templates() {
        let (_dir, config, world, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        for value in [i32::MIN, -3, 0, i32::MAX] {
            set_battle_unit_field_action(
                &scripts,
                &config,
                ObjectId(1),
                id,
                "units_killed",
                &value.to_string(),
            )
            .unwrap();
            assert_eq!(kills(&scripts.world(), id), i64::from(value));
            let field =
                view_battle_unit_fields_action(&scripts, &config, ObjectId(1), id, "units_killed")
                    .unwrap();
            assert!(format!("{field:?}").contains(&value.to_string()));
        }
        let before = scripts.world().btech.clone();
        for value in ["2147483648", "-2147483649", "no"] {
            assert!(
                set_battle_unit_field_action(
                    &scripts,
                    &config,
                    ObjectId(1),
                    id,
                    "units_killed",
                    value
                )
                .is_err()
            );
            assert!(scripts.world().btech == before);
        }
        let section = if scripts.world().btech.vehicles().contains_key(&id) {
            "Front"
        } else {
            "Center_Torso"
        };
        battle_damage_section_action(
            &scripts,
            &config,
            ObjectId(1),
            id,
            BattleScenarioHit {
                section,
                damage: 1000,
                rear: false,
                critical: false,
            },
        )
        .unwrap();
        assert!(destroyed(&scripts.world(), id));
        assert_eq!(kills(&scripts.world(), id), i64::from(i32::MAX));
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(kills(&restored, id), i64::from(i32::MAX));
    }
}

/// Vacuum retains the shot author; water flooding remains self-attributed even after a shot.
#[tokio::test]
async fn shot_induced_water_and_vacuum_exposure_use_distinct_attribution() {
    for water in [false, true] {
        for target_source in [
            include_str!("../game/mechs/JR7-D.toml"),
            include_str!("../game/mechs/GOL-1H.toml"),
        ] {
            let (_dir, config, mut base, id, target, index) = firing::fixture_with_target(
                include_str!("../game/mechs/JR7-D.toml"),
                Some(BattleWeapon::MediumLaser),
                target_source,
            )
            .await;
            let map = base.create(&config, "Exposure field".into(), Kind::Room);
            create_battle_map(
                &mut base,
                map,
                "exposure",
                MapAsset::from_cells(&format!(
                    "1 12\n{}",
                    if water { "~2\n" } else { ".0\n" }.repeat(12)
                ))
                .unwrap(),
            )
            .unwrap();
            support::seed_object_dice(&mut base, map, support::FIXTURE_DICE_SEED);
            for (unit, y) in [(id, 11), (target, 10)] {
                firing::edit(&mut base, unit, |state| {
                    state["power"] = serde_json::to_value(BattlePower::Off).unwrap()
                });
                place_battle_unit(&mut base, unit, map, 0, y).unwrap();
                firing::edit(&mut base, unit, |state| {
                    state["power"] = serde_json::to_value(BattlePower::Running).unwrap()
                });
                if water {
                    firing::edit(&mut base, unit, |state| {
                        state["ground_elevation"] = (-2).into()
                    });
                }
            }
            if !water {
                set_battle_map_environment(
                    &mut base,
                    ObjectId(1),
                    map,
                    BattleMapEnvironment {
                        gravity: 100,
                        temperature: 20,
                        vacuum: true,
                        underground: false,
                    },
                )
                .unwrap();
            }
            firing::edit(&mut base, target, |state| {
                state["sections"]["Head"]["armor"] = 3.into();
                for rounds in state["ammunition"].as_array_mut().unwrap() {
                    *rounds = 0.into();
                }
            });
            let hit_seed = (0..=255)
                .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
                .unwrap();
            firing::edit(&mut base, id, |state| {
                state["dice"] = serde_json::to_value(BattleDice::seeded([hit_seed; 32])).unwrap()
            });
            refresh_battle_contacts(&mut base, &[id]).unwrap();
            select_battle_target(&mut base, id, ObjectId(1), Some(target)).unwrap();
            let command = format!("btech.unit.fire({},1,{index},{})", id.0, target.0);
            let mut chosen = None;
            for seed in 0..=255 {
                firing::edit(&mut base, target, |state| {
                    state["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
                });
                let scripts = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
                scripts.eval_callback::<()>(&command).unwrap();
                let exposed = {
                    let world = scripts.world();
                    let unit = &world.btech.constructed_units()[&target];
                    let sections = if water {
                        unit.flooded_sections()
                    } else {
                        unit.breached_sections()
                    };
                    sections.contains(&BattleSection::Head)
                        && unit.sections()[&BattleSection::Head].internal > 0
                };
                if exposed {
                    chosen = Some(scripts);
                    break;
                }
            }
            let scripts = chosen.expect("seed matrix must include a surviving, breached head");
            assert!(destroyed(&scripts.world(), target));
            assert_eq!(kills(&scripts.world(), id), i64::from(!water));
            assert_eq!(kills(&scripts.world(), target), 0);
            let saved = scripts.world().clone();
            persistence::save(&config.database(), &saved).await.unwrap();
            let loaded = persistence::load(&config.database()).await.unwrap();
            assert_eq!(kills(&loaded, id), i64::from(!water));
            let abort = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
            assert!(
                abort
                    .eval_callback::<()>(&format!("{command}; error('abort')"))
                    .is_err()
            );
            assert!(abort.world().btech == base.btech);
            assert!(abort.drain_outbox().is_empty());
        }
    }
}

/// The reference returns after disabling vehicle equipment, before its unreachable death branch.
#[tokio::test]
async fn vehicle_vacuum_breaches_preserve_life_and_award_no_kill() {
    for source in firing::templates().into_iter().skip(2) {
        let (_dir, config, mut base, id, target, index) = firing::fixture_with_target(
            include_str!("../game/mechs/JR7-D.toml"),
            Some(BattleWeapon::MediumLaser),
            &source,
        )
        .await;
        let map = base.btech.units()[&target].map.unwrap();
        set_battle_map_environment(
            &mut base,
            ObjectId(1),
            map,
            BattleMapEnvironment {
                gravity: 100,
                temperature: 20,
                vacuum: true,
                underground: false,
            },
        )
        .unwrap();
        firing::edit(&mut base, target, |state| {
            for section in state["sections"].as_object_mut().unwrap().values_mut() {
                section["armor"] = section["armor"].as_u64().unwrap().min(4).into();
            }
            for rounds in state["ammunition"].as_array_mut().unwrap() {
                *rounds = 0.into();
            }
        });
        let hit_seed = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
            .unwrap();
        firing::edit(&mut base, id, |state| {
            state["dice"] = serde_json::to_value(BattleDice::seeded([hit_seed; 32])).unwrap()
        });
        let command = format!("btech.unit.fire({},1,{index},{})", id.0, target.0);
        let mut chosen = None;
        for seed in 0..64 {
            firing::edit(&mut base, target, |state| {
                state["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
            });
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
            scripts.eval_callback::<()>(&command).unwrap();
            let eligible = {
                let world = scripts.world();
                let unit = &world.btech.vehicles()[&target];
                !unit.breached_sections().is_empty()
                    && unit.sections().values().all(|section| section.internal > 0)
                    && !unit.crew_killed()
                    && unit.pilot_injuries() == 0
            };
            if eligible {
                chosen = Some(scripts);
                break;
            }
        }
        let scripts =
            chosen.expect("seed matrix must breach surviving structure without a crew critical");
        assert!(!destroyed(&scripts.world(), target));
        assert_eq!(kills(&scripts.world(), id), 0);
        assert_eq!(kills(&scripts.world(), target), 0);
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert!(!destroyed(&loaded, target));
        assert_eq!(kills(&loaded, id), 0);
        let abort = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
        assert!(
            abort
                .eval_callback::<()>(&format!("{command}; error('abort')"))
                .is_err()
        );
        assert!(abort.world().btech == base.btech);
        assert!(abort.drain_outbox().is_empty());
        // A full kill counter cannot reject a nonlethal breach.
        set_battle_unit_field_action(
            &abort,
            &config,
            ObjectId(1),
            id,
            "units_killed",
            "2147483647",
        )
        .unwrap();
        abort.drain_outbox();
        abort.eval_callback::<()>(&command).unwrap();
        assert_eq!(kills(&abort.world(), id), i64::from(i32::MAX));
    }
}
