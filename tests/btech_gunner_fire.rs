//! Registered gunners use shared launch and damage without replacing parent pilots or selections.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod firing;
use crate::support;

/// Build a claimed, settled station alongside the parent's equally settled lock.
async fn fixture(
    template: &str,
    weapon: BattleWeapon,
    arcs: i32,
) -> (
    tempfile::TempDir,
    Config,
    World,
    ObjectId,
    ObjectId,
    ObjectId,
    ObjectId,
    usize,
) {
    let target_source = include_str!("../game/mechs/AS7-D");
    let (dir, config, mut world, parent, target, index) = if weapon.profile().ammunition_per_ton
        == 0
    {
        firing::fixture_with_target(template, Some(weapon), target_source).await
    } else {
        firing::fixture_with_supply(template, Some(weapon), target_source, false, Some("")).await
    };
    let station = world.create(&config, "Station".into(), Kind::Thing);
    let gunner = world.create(&config, "Gunner".into(), Kind::Player);
    world.objects.get_mut(&gunner).unwrap().location = Some(station);
    register_gunner_station(&mut world, ObjectId(1), station, parent, arcs).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    gunner_station_action(&scripts, station, gunner, true).unwrap();
    select_battle_target(&mut scripts.world_mut(), station, gunner, Some(target)).unwrap();
    for _ in 0..8 {
        advance_battle_target_locks(&mut scripts.world_mut());
    }
    let world = scripts.world().clone();
    (dir, config, world, parent, target, station, gunner, index)
}

/// Build a claimed, settled station on a supplied base world through one reused
/// setup sandbox; identities stay stable because every scenario re-clones.
#[allow(clippy::too_many_arguments)]
fn station_fixture_on(
    base: &World,
    config: &Config,
    setup: &Scripts,
    template: &str,
    weapon: BattleWeapon,
    arcs: i32,
) -> (World, ObjectId, ObjectId, ObjectId, ObjectId, usize) {
    let target_source = include_str!("../game/mechs/AS7-D");
    let (mut world, parent, target, index) = if weapon.profile().ammunition_per_ton == 0 {
        firing::supply_fixture_on(
            base.clone(),
            config,
            template,
            Some(weapon),
            target_source,
            false,
            None,
        )
    } else {
        firing::supply_fixture_on(
            base.clone(),
            config,
            template,
            Some(weapon),
            target_source,
            false,
            Some(""),
        )
    };
    let station = world.create(config, "Station".into(), Kind::Thing);
    let gunner = world.create(config, "Gunner".into(), Kind::Player);
    world.objects.get_mut(&gunner).unwrap().location = Some(station);
    register_gunner_station(&mut world, ObjectId(1), station, parent, arcs).unwrap();
    support::install(setup, world);
    gunner_station_action(setup, station, gunner, true).unwrap();
    select_battle_target(&mut setup.world_mut(), station, gunner, Some(target)).unwrap();
    for _ in 0..8 {
        advance_battle_target_locks(&mut setup.world_mut());
    }
    (
        setup.world().clone(),
        parent,
        target,
        station,
        gunner,
        index,
    )
}

/// Native and Lua station shots must produce the same physical results as the existing pilot path.
#[tokio::test]
async fn station_direct_fire_shares_all_chassis_and_weapon_families() {
    let (_dir, config, base) = support::isolated_world().await;
    let boot = |world: World| Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let setup = boot(base.clone());
    let pilot = boot(base.clone());
    let gun = boot(base.clone());
    let native = boot(base.clone());
    let restored = boot(base.clone());
    for template in firing::templates() {
        for weapon in [
            BattleWeapon::MediumLaser,
            BattleWeapon::Lrm5,
            BattleWeapon::Ac5,
            BattleWeapon::Mml5,
        ] {
            let (world, parent, target, station, gunner, index) =
                station_fixture_on(&base, &config, &setup, &template, weapon, 0);
            support::install(&pilot, world.clone());
            support::install(&gun, world.clone());
            support::install(&native, world.clone());
            assert_eq!(
                support::run_text(&native, &config, gunner, 1, "weapons"),
                support::run_text(&pilot, &config, ObjectId(1), 1, "weapons"),
            );

            let expected: mlua::Table = pilot
                .eval_callback(&format!("return btech.unit.fire({},1,{index})", parent.0))
                .unwrap();
            let actual: mlua::Table = gun
                .eval_callback(&format!(
                    "return btech.gunner.fire({},{},{index})",
                    station.0, gunner.0
                ))
                .unwrap();
            assert_eq!(actual.get::<i64>("target").unwrap(), target.0);
            assert_eq!(
                serde_json::to_value(actual).unwrap(),
                serde_json::to_value(expected).unwrap()
            );
            assert_eq!(gun.world().btech, pilot.world().btech);
            if weapon == BattleWeapon::MediumLaser {
                assert_ne!(
                    gun.world().btech.constructed_units()[&target].sections(),
                    world.btech.constructed_units()[&target].sections()
                );
            }

            let text = support::run_text(&native, &config, gunner, 1, &format!("fire {index}"));
            assert!(text.contains("You fire"), "{weapon:?}: {text}");
            assert_eq!(native.world().btech, gun.world().btech);
            assert_eq!(
                gun.world().btech.gunner_stations(),
                world.btech.gunner_stations()
            );
            assert_ne!(gun.world().btech, world.btech);
            support::install(&restored, world.clone());
            assert!(
                restored
                    .eval_callback::<()>(&format!(
                        "btech.gunner.fire({},{},{index}); error('abort')",
                        station.0, gunner.0
                    ))
                    .is_err()
            );
            assert_eq!(restored.world().btech, world.btech);
            assert!(restored.drain_outbox().is_empty());
        }
    }
}

/// Station ownership cannot grant movement, foreign equipment, held weapons or an unassigned arc.
#[tokio::test]
async fn station_direct_fire_guards_preserve_equipment() {
    for template in firing::templates() {
        let (_dir, config, world, parent, target, station, gunner, index) =
            fixture(&template, BattleWeapon::MediumLaser, 8).await;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let query = format!("btech.gunner.fire({},{},{index})", station.0, gunner.0);
        let before = scripts.world().btech.clone();
        let error = scripts.eval_callback::<()>(&query).unwrap_err();
        assert!(
            error.to_string().contains("do not control that firing arc"),
            "{error}"
        );
        assert_eq!(scripts.world().btech, before);
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.fire({},{},{index},{})",
                    target.0, gunner.0, parent.0
                ))
                .is_err()
        );
        assert!(
            stop_battle_unit(
                &mut scripts.world_mut(),
                parent,
                gunner,
                BattleMovementRules::STANDARD.fall
            )
            .is_err()
        );
        set_battle_weapons_hold(&mut scripts.world_mut(), parent, true).unwrap();
        let before = scripts.world().btech.clone();
        let error = scripts.eval_callback::<()>(&query).unwrap_err();
        assert!(error.to_string().contains("weapons hold"), "{error}");
        assert_eq!(scripts.world().btech, before);
        gunner_station_action(&scripts, station, gunner, false).unwrap();
        assert!(scripts.eval_callback::<()>(&query).is_err());
    }
}

/// Live fire reads station skills and selection even when the parent has no target.
#[tokio::test]
async fn direct_fire_uses_station_skill_and_restarts_committed_expenditure() {
    for template in firing::templates() {
        let (_dir, config, mut world, parent, target, station, gunner, index) =
            fixture(&template, BattleWeapon::Ac5, 1).await;
        world
            .objects
            .get_mut(&gunner)
            .unwrap()
            .flags
            .insert(Flag::Connected);
        select_battle_target(&mut world, parent, ObjectId(1), None).unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let station_before = scripts.world().btech.gunner_stations()[&station].clone();
        let report: mlua::Table = scripts
            .eval_callback(&format!(
                "return btech.gunner.fire({},{},{index})",
                station.0, gunner.0
            ))
            .unwrap();
        assert_eq!(report.get::<i64>("target").unwrap(), target.0);
        let aim: mlua::Table = report.get("aim").unwrap();
        assert_eq!(aim.get::<i16>("gunnery").unwrap(), 18);
        assert_eq!(aim.get::<u8>("target_lock").unwrap(), 0);
        assert_eq!(
            scripts.world().btech.gunner_stations()[&station],
            station_before
        );
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, saved.btech);
        // Mech-only public entry points must reject vehicle storage rather than indexing it.
        if restored.btech.vehicles().contains_key(&parent) {
            let mut candidate = restored.clone();
            assert!(spend_battle_weapon(&mut candidate, parent, ObjectId(1), index).is_err());
            assert_eq!(candidate.btech, restored.btech);
        }
    }
}

/// A station operator receives both XP eligibility and the resulting shared skill award.
#[tokio::test]
async fn station_gunnery_experience_belongs_to_the_gunner() {
    for template in firing::templates() {
        let (_dir, _config, mut world, parent, target, station, gunner, _) =
            fixture(&template, BattleWeapon::MediumLaser, 1).await;
        world
            .objects
            .get_mut(&gunner)
            .unwrap()
            .flags
            .insert(Flag::Connected);
        set_battle_character(
            &mut world,
            gunner,
            BattleCharacter {
                bruise: 0,
                lethal: 0,
                build: 3,
                reflexes: 3,
                intuition: 3,
                learn: 2,
                charisma: 1,
            },
        )
        .unwrap();
        for (id, team) in [(parent, 1), (target, 2)] {
            world
                .objects
                .get_mut(&id)
                .unwrap()
                .flags
                .insert(Flag::InCharacter);
            set_battle_sensor_signature(
                &mut world,
                id,
                BattleSensorSignature {
                    team,
                    hidden: false,
                    illuminated: false,
                },
            )
            .unwrap();
        }
        for mode in [
            BattleGunneryExperienceMode::Classic,
            BattleGunneryExperienceMode::BattleValue {
                difficulty_modifier: true,
            },
        ] {
            assert!(battle_gunnery_experience_eligible(
                &world, parent, gunner, target, 7, mode
            ));
        }
        let pilot_before = world.btech.character_values().get(&ObjectId(1)).cloned();
        let award = award_battle_classic_gunnery_experience(
            &mut world,
            BattleGunneryAwardRequest {
                tsm_tow_bonus: false,

                tsm_sprint_bonus: true,
                attacker: parent,
                pilot: gunner,
                target,
                weapon: BattleWeapon::MediumLaser,
                damage: 1000,
                base_to_hit: 7,
                extended_gunnery: false,
                extended_piloting: false,
                use_unit_modifier: false,
                now: 1000,
            },
        )
        .unwrap()
        .unwrap();
        assert!(award.amount.is_some_and(|amount| amount > 0));
        assert!(world.btech.character_values()[&gunner][award.skill].experience > 0);
        assert_eq!(
            world.btech.character_values().get(&ObjectId(1)),
            pilot_before.as_ref()
        );
        world.objects.get_mut(&gunner).unwrap().location = Some(parent);
        assert!(!battle_gunnery_experience_eligible(
            &world,
            parent,
            gunner,
            target,
            7,
            BattleGunneryExperienceMode::Classic
        ));
        world.objects.get_mut(&gunner).unwrap().location = Some(station);
        world
            .objects
            .get_mut(&gunner)
            .unwrap()
            .flags
            .remove(Flag::Connected);
        assert!(!battle_gunnery_experience_eligible(
            &world,
            parent,
            gunner,
            target,
            7,
            BattleGunneryExperienceMode::Classic
        ));
    }
}

/// Failure while staging the extra gunner audience restores shot, damage, dice and all notices.
#[tokio::test]
async fn station_firing_publication_failure_is_atomic() {
    let (dir, _config, world, _, _, station, gunner, index) =
        fixture(&firing::templates()[0], BattleWeapon::MediumLaser, 1).await;
    let path = dir.path().join("stompymux.toml");
    let mut table: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
    table
        .entry("lua")
        .or_insert(toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .unwrap()
        .insert("output_entry_limit".into(), 1.into());
    std::fs::write(path, toml::to_string(&table).unwrap()).unwrap();
    let config = Config::load(dir.path()).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.gunner.fire({},{},{index})",
                station.0, gunner.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, world.btech);
    assert!(scripts.drain_outbox().is_empty());
    let sight = format!("btech.gunner.sight({},{},{index})", station.0, gunner.0);
    assert!(
        scripts
            .eval_callback::<()>(&format!("{sight}; {sight}"))
            .is_err()
    );
    assert_eq!(scripts.world().btech, world.btech);
    assert!(scripts.drain_outbox().is_empty());
}

/// Every assigned hull/torso mask overrides mounting direction without becoming an unrestricted arc.
#[tokio::test]
async fn station_arc_masks_follow_parent_heading() {
    for template in firing::templates() {
        for (heading, mask) in [(0.0, 1), (90.0, 2), (180.0, 8), (270.0, 4)] {
            let (_dir, config, mut world, parent, _, station, gunner, index) =
                fixture(&template, BattleWeapon::MediumLaser, mask).await;
            if heading != 0.0
                && world.btech.vehicles().get(&parent).is_some_and(|unit| {
                    unit.definition().movement == BattleVehicleMovement::Stationary
                })
            {
                continue; // Fixed platforms retain the heading established by placement.
            }
            firing::edit(&mut world, parent, |state| {
                state["motion"]["heading"] = heading.into()
            });
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let result = scripts.eval_callback::<mlua::Table>(&format!(
                "return btech.gunner.fire({},{},{index})",
                station.0, gunner.0
            ));
            assert!(result.is_ok(), "heading {heading}, mask {mask}: {result:?}");
        }
    }
}

/// Sensing uses the physical parent and recovery uses the gunner before any shot expenditure.
#[tokio::test]
async fn gunner_firing_health_guards_precede_expenditure() {
    for template in firing::templates() {
        let (_dir, config, world, parent, _, station, gunner, index) =
            fixture(&template, BattleWeapon::MediumLaser, 1).await;
        for blind in [false, true] {
            let mut candidate = world.clone();
            let mut state = serde_json::to_value(&candidate.btech).unwrap();
            if blind {
                let class = if candidate.btech.vehicles().contains_key(&parent) {
                    "vehicles"
                } else {
                    "constructed"
                };
                state[class][parent.0.to_string()]["blinded_remaining"] = 1.into();
            } else {
                state["recoveries"][gunner.0.to_string()] = serde_json::json!({"remaining":1,"pain_resistance":false,"toughness":false,"dice":BattleDice::seeded([34;32])});
            }
            candidate.btech = serde_json::from_value(state).unwrap();
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(candidate.clone()))).unwrap();
            let error = scripts
                .eval_callback::<()>(&format!(
                    "btech.gunner.fire({},{},{index})",
                    station.0, gunner.0
                ))
                .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains(if blind { "blinded" } else { "unconscious" }),
                "{error}"
            );
            assert_eq!(scripts.world().btech, candidate.btech);
            assert!(scripts.drain_outbox().is_empty());
        }
    }
}

/// In-character damage reaches the shared XP pipeline with the gunner as recipient.
#[tokio::test]
async fn live_station_damage_awards_gunner_experience() {
    for template in [
        include_str!("../game/mechs/JR7-D"),
        include_str!("../game/mechs/Demolisher"),
        include_str!("../game/mechs/Kestrel"),
    ] {
        let (_dir, config, mut world, parent, target, station, gunner, index) =
            fixture(template, BattleWeapon::MediumLaser, 1).await;
        world
            .objects
            .get_mut(&gunner)
            .unwrap()
            .flags
            .insert(Flag::Connected);
        set_battle_character(
            &mut world,
            gunner,
            BattleCharacter {
                bruise: 0,
                lethal: 0,
                build: 3,
                reflexes: 6,
                intuition: 6,
                learn: 2,
                charisma: 1,
            },
        )
        .unwrap();
        for (id, team) in [(parent, 1), (target, 2)] {
            world
                .objects
                .get_mut(&id)
                .unwrap()
                .flags
                .insert(Flag::InCharacter);
            set_battle_sensor_signature(
                &mut world,
                id,
                BattleSensorSignature {
                    team,
                    hidden: false,
                    illuminated: false,
                },
            )
            .unwrap();
        }
        let pilot_before = world.btech.character_values().get(&ObjectId(1)).cloned();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let report: mlua::Table = scripts
            .eval_callback(&format!(
                "return btech.gunner.fire({},{},{index})",
                station.0, gunner.0
            ))
            .unwrap();
        let aim: mlua::Table = report.get("aim").unwrap();
        assert_eq!(aim.get::<i16>("gunnery").unwrap(), 6);
        assert!(
            scripts.world().btech.character_values()[&gunner]
                .values()
                .any(|skill| skill.experience > 0)
        );
        assert_eq!(
            scripts.world().btech.character_values().get(&ObjectId(1)),
            pilot_before.as_ref()
        );
    }
}

/// All terrain purposes use station intent with the same launch, terrain effects and publication as pilots.
#[tokio::test]
async fn station_terrain_fire_shares_modes_and_physical_effects() {
    let (_dir, config, base) = support::isolated_world().await;
    let boot = |world: World| Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let setup = boot(base.clone());
    let pilot = boot(base.clone());
    let gun = boot(base.clone());
    let native = boot(base.clone());
    let independent = boot(base.clone());
    let failed = boot(base.clone());
    let pristine_db = support::snapshot_database(&config);
    for template in firing::templates() {
        for weapon in [BattleWeapon::MediumLaser, BattleWeapon::Lrm5] {
            let (settled, parent, _, station, gunner, index) =
                station_fixture_on(&base, &config, &setup, &template, weapon, 0);
            let map = settled.btech.units()[&parent].map.unwrap();
            let coordinate = BattleHexCoordinate { x: 0, y: 9 };
            set_battle_map_hex_action(
                &setup,
                &config,
                ObjectId(1),
                map,
                coordinate,
                Terrain::LightForest,
                0,
            )
            .unwrap();
            let scenario = setup.world().clone();
            for mode in [
                BattleHexTargetMode::UnitAtHex,
                BattleHexTargetMode::Hex,
                BattleHexTargetMode::Building,
                BattleHexTargetMode::Ignite,
                BattleHexTargetMode::Clear,
            ] {
                let mut world = scenario.clone();
                select_battle_hex_target(&mut world, parent, ObjectId(1), coordinate, mode)
                    .unwrap();
                select_battle_hex_target(&mut world, station, gunner, coordinate, mode).unwrap();
                support::install(&pilot, world.clone());
                support::install(&gun, world.clone());
                support::install(&native, world.clone());
                let query = format!("btech.gunner.fire({},{},{index})", station.0, gunner.0);
                let expected: mlua::Table = pilot
                    .eval_callback(&format!("return btech.unit.fire({},1,{index})", parent.0))
                    .unwrap();
                let actual: mlua::Table = gun.eval_callback(&format!("return {query}")).unwrap();
                assert_eq!(
                    serde_json::to_value(actual).unwrap(),
                    serde_json::to_value(expected).unwrap()
                );
                assert_eq!(gun.world().btech, pilot.world().btech);
                let text = support::run_text(&native, &config, gunner, 1, &format!("fire {index}"));
                assert!(
                    text.contains("You fire") && text.contains("(0,9)"),
                    "{text}"
                );
                assert_eq!(native.world().btech, gun.world().btech);
                support::install(&independent, world.clone());
                select_battle_target(&mut independent.world_mut(), parent, ObjectId(1), None)
                    .unwrap();
                let report: mlua::Table = independent
                    .eval_callback(&format!("return {query}"))
                    .unwrap();
                let aim: mlua::Table = report.get("aim").unwrap();
                assert_eq!(
                    aim.get::<String>("mode").unwrap(),
                    serde_json::to_value(mode).unwrap().as_str().unwrap()
                );
                support::install(&failed, world.clone());
                assert!(
                    failed
                        .eval_callback::<()>(&format!("{query}; error('abort')"))
                        .is_err()
                );
                assert_eq!(failed.world().btech, world.btech);
                assert!(failed.drain_outbox().is_empty());
                if mode == BattleHexTargetMode::Clear {
                    let saved = gun.world().clone();
                    support::restore_database(&config, &pristine_db);
                    persistence::save(&config.database(), &saved).await.unwrap();
                    let loaded = persistence::load(&config.database()).await.unwrap();
                    assert_eq!(loaded.btech, saved.btech);
                }
            }
        }
    }
}

/// Explicit empty coordinates still honor the station mask and publish nothing on rejection.
#[tokio::test]
async fn station_terrain_rejects_unassigned_arcs() {
    for template in firing::templates() {
        let (_dir, config, world, _, _, station, gunner, index) =
            fixture(&template, BattleWeapon::MediumLaser, 8).await;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let error = scripts
            .eval_callback::<()>(&format!(
                "btech.gunner.fire({},{},{index},{{x=0,y=9}})",
                station.0, gunner.0
            ))
            .unwrap_err();
        assert!(
            error.to_string().contains("do not control that firing arc"),
            "{error}"
        );
        assert_eq!(scripts.world().btech, world.btech);
        assert!(scripts.drain_outbox().is_empty());
    }
}

/// SIGHT shares cockpit arithmetic and preparation dice while leaving equipment and cover untouched.
#[tokio::test]
async fn station_sight_shares_chassis_targets_and_preserves_combat_state() {
    let (_dir, config, base) = support::isolated_world().await;
    let boot = |world: World| Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let setup = boot(base.clone());
    let pilot = boot(base.clone());
    let gun = boot(base.clone());
    let native = boot(base.clone());
    let pristine_db = support::snapshot_database(&config);
    for template in firing::templates() {
        for weapon in [
            BattleWeapon::MediumLaser,
            BattleWeapon::Lrm5,
            BattleWeapon::Ac5,
            BattleWeapon::Mml5,
        ] {
            let (mut world, parent, target, station, gunner, index) =
                station_fixture_on(&base, &config, &setup, &template, weapon, 0);
            firing::edit(&mut world, parent, |state| {
                state["weapons_hold"] = true.into();
                state["weapon_recycle"][index.to_string()] = 30.into();
                state["sensor_signature"]["hidden"] = true.into();
                for count in state["ammunition"].as_array_mut().unwrap() {
                    *count = 0.into();
                }
            });
            for (arg, native_arg) in [
                ("nil".into(), "".into()),
                (target.0.to_string(), format!("#{}", target.0)),
                ("{x=0,y=9}".into(), "0 9".into()),
            ] {
                support::install(&pilot, world.clone());
                support::install(&gun, world.clone());
                support::install(&native, world.clone());
                let query = format!(
                    "btech.gunner.sight({},{},{index},{arg})",
                    station.0, gunner.0
                );
                assert!(
                    gun.eval_callback::<()>(&format!("{query}; error('abort')"))
                        .is_err()
                );
                assert_eq!(gun.world().btech, world.btech);
                assert!(gun.drain_outbox().is_empty());
                let actual: mlua::Table = gun.eval_callback(&format!("return {query}")).unwrap();
                let expected: mlua::Table = pilot
                    .eval_callback(&format!(
                        "return btech.unit.sight({},1,{index},{arg})",
                        parent.0
                    ))
                    .unwrap();
                assert_eq!(
                    serde_json::to_value(actual).unwrap(),
                    serde_json::to_value(expected).unwrap()
                );
                let text = support::run_text(
                    &native,
                    &config,
                    gunner,
                    1,
                    &format!("sight {index} {native_arg}"),
                );
                assert!(text.contains("You aim"), "{text}");
                assert_eq!(gun.world().btech, pilot.world().btech);
                assert_eq!(gun.world().btech, native.world().btech);
                let mut expected = world.clone();
                let mut dice = BattleDice::seeded([42; 32]);
                dice.two_d6();
                firing::edit(&mut expected, parent, |state| {
                    state["dice"] = serde_json::to_value(dice).unwrap()
                });
                assert_eq!(gun.world().btech, expected.btech);
                let saved = gun.world().clone();
                support::restore_database(&config, &pristine_db);
                persistence::save(&config.database(), &saved).await.unwrap();
                assert_eq!(
                    persistence::load(&config.database()).await.unwrap().btech,
                    saved.btech
                );
            }
        }
    }
}

/// Station sighting owns its target, skill, mask and authority without affecting the parent lock.
#[tokio::test]
async fn station_sight_uses_independent_selection_skill_and_arc() {
    for template in firing::templates() {
        let (_dir, config, mut world, parent, target, station, gunner, index) =
            fixture(&template, BattleWeapon::MediumLaser, 1).await;
        select_battle_target(&mut world, parent, ObjectId(1), None).unwrap();
        world
            .objects
            .get_mut(&gunner)
            .unwrap()
            .flags
            .insert(Flag::Connected);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let query = format!("btech.gunner.sight({},{},{index})", station.0, gunner.0);
        let (selected, skill): (i64, i16) = scripts
            .eval_callback(&format!("local r={query}; return r.target,r.aim.gunnery"))
            .unwrap();
        assert_eq!(selected, target.0);
        assert_eq!(skill, 18);
        assert!(scripts.world().btech.vehicles().get(&parent).map_or_else(
            || {
                scripts.world().btech.constructed_units()[&parent]
                    .target_selection()
                    .is_none()
            },
            |unit| unit.target_selection().is_none(),
        ));
        select_battle_hex_target(
            &mut scripts.world_mut(),
            station,
            gunner,
            BattleHexCoordinate { x: 0, y: 9 },
            BattleHexTargetMode::Clear,
        )
        .unwrap();
        let coordinate: i32 = scripts
            .eval_callback(&format!("return {query}.coordinate.y"))
            .unwrap();
        assert_eq!(coordinate, 9);
        let before = scripts.world().btech.clone();
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.gunner.sight({},{},{index},{{x=0,y=11}})",
                    station.0, gunner.0
                ))
                .unwrap_err()
                .to_string()
                .contains("firing arc")
        );
        assert_eq!(scripts.world().btech, before);
        gunner_station_action(&scripts, station, gunner, false).unwrap();
        let before = scripts.world().btech.clone();
        assert!(scripts.eval_callback::<()>(&query).is_err());
        assert_eq!(scripts.world().btech, before);
    }
}

#[tokio::test]
async fn parent_firing_reaches_registered_stations_once_and_keeps_private_reports_private() {
    for template in firing::templates() {
        let (_dir, config, mut world, parent, _target, station, gunner, index) =
            fixture(&template, BattleWeapon::MediumLaser, 0).await;
        let second = world.create(&config, "Second station".into(), Kind::Thing);
        let observer = world.create(&config, "Second gunner".into(), Kind::Player);
        world.objects.get_mut(&observer).unwrap().location = Some(second);
        register_gunner_station(&mut world, ObjectId(1), second, parent, 0).unwrap();
        for routing in 0..3 {
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            if routing == 1 {
                set_gunner_field(
                    &scripts,
                    &config,
                    ObjectId(1),
                    second,
                    "parent",
                    &station.0.to_string(),
                )
                .unwrap();
            } else if routing == 2 {
                scripts
                    .world_mut()
                    .objects
                    .get_mut(&second)
                    .unwrap()
                    .flags
                    .insert(Flag::Going);
            }
            scripts
                .eval_callback::<mlua::Value>(&format!(
                    "return btech.unit.fire({},1,{index})",
                    parent.0
                ))
                .unwrap();
            let messages = scripts.drain_outbox();
            let for_player = |player| {
                messages
                    .iter()
                    .filter(|(recipient, _)| *recipient == player)
                    .map(|(_, text)| text.source().to_owned())
                    .collect::<Vec<_>>()
            };
            let pilot = for_player(ObjectId(1));
            assert!(!pilot.is_empty());
            assert_eq!(for_player(gunner), pilot);
            assert_eq!(
                for_player(observer),
                if routing == 0 { pilot } else { vec![] }
            );
            view_battle_unit_fields_action(&scripts, &config, ObjectId(1), parent, "xpmod")
                .unwrap();
            assert!(
                scripts
                    .drain_outbox()
                    .iter()
                    .all(|(recipient, _)| *recipient == ObjectId(1))
            );
        }
    }
}

#[tokio::test]
async fn explicit_cockpit_destinations_relay_once_alongside_registered_stations() {
    for template in firing::templates() {
        let (_dir, config, mut world, parent, _target, station, gunner, index) =
            fixture(&template, BattleWeapon::MediumLaser, 0).await;
        let room = world.create(&config, "Remote cockpit".into(), Kind::Room);
        let observer = world.create(&config, "Remote observer".into(), Kind::Player);
        world.objects.get_mut(&observer).unwrap().location = Some(room);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        for (field, destination) in [
            ("turret0", station),
            ("turret1", station),
            ("turret2", room),
        ] {
            set_battle_unit_field_action(
                &scripts,
                &config,
                ObjectId(1),
                parent,
                field,
                &destination.0.to_string(),
            )
            .unwrap();
        }
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, saved.btech);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
        scripts
            .eval_callback::<mlua::Value>(&format!(
                "return btech.unit.fire({},1,{index})",
                parent.0
            ))
            .unwrap();
        let messages = scripts.drain_outbox();
        let for_player = |player| {
            messages
                .iter()
                .filter(|(id, _)| *id == player)
                .map(|(_, doc)| doc.source().to_owned())
                .collect::<Vec<_>>()
        };
        let pilot = for_player(ObjectId(1));
        assert!(!pilot.is_empty());
        assert_eq!(for_player(gunner), pilot);
        assert_eq!(for_player(observer), pilot);
    }
}
