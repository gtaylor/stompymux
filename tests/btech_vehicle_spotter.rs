//! Shared explicit observer links support both shooter anatomies without duplicate indirect aim.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Friendly mixed-class shooter/observer and an enemy, all within acquisition distance.
async fn fixture(
    vehicle_shooter: bool,
    vehicle_observer: bool,
) -> (tempfile::TempDir, Config, World, [ObjectId; 3], usize) {
    fixture_with_mml(vehicle_shooter, vehicle_observer, false).await
}

/// Use the same observer fixture with MML ammunition to exercise installation-specific capability.
async fn fixture_with_mml(
    vehicle_shooter: bool,
    vehicle_observer: bool,
    mml: bool,
) -> (tempfile::TempDir, Config, World, [ObjectId; 3], usize) {
    let source = |text: &str| {
        if mml {
            text.replace(
                "item = \"Ammo_IS.LRM-20\", rounds = 6",
                "item = \"Ammo_IS.MML-9\", rounds = 13, modes = [\"MML_LRM\"]",
            )
            .replace(
                "item = \"IS.LRM-20\"",
                "item = \"IS.MML-9\", modes = [\"MML_LRM\"]",
            )
        } else {
            text.into()
        }
    };
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Spotting field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        BattleMapAsset::parse("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for (index, vehicle) in [vehicle_shooter, vehicle_observer, false]
        .into_iter()
        .enumerate()
    {
        let id = world.create(&config, "Unit".into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        if vehicle {
            create_battle_vehicle(
                &mut world,
                id,
                BattleVehicleTemplate::parse(
                    "test",
                    &source(include_str!("../game/mechs/Hunter.toml")),
                )
                .unwrap(),
            )
            .unwrap();
        } else {
            create_battle_unit(
                &mut world,
                id,
                BattleTemplate::parse("test", &source(include_str!("../game/mechs/AS7-D.toml")))
                    .unwrap(),
            )
            .unwrap();
        }
        let (x, y) = [(1, 2), (2, 1), (1, 0)][index];
        place_battle_unit(&mut world, id, map, x, y).unwrap();
        if index < 2 {
            let pilot = ObjectId(index as i64 + 1);
            world.objects.get_mut(&pilot).unwrap().location = Some(id);
            assign_battle_pilot(&mut world, id, pilot).unwrap();
            start_battle_unit(&mut world, id, pilot, true).unwrap();
            for _ in 0..5 {
                advance_battle_units(&mut world, 0);
            }
        }
        ids.push(id);
    }
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["constructed"][ids[2].0.to_string()]["signature"]["team"] = 99.into();
    world.btech = serde_json::from_value(saved).unwrap();
    for _ in 0..10 {
        refresh_battle_contacts(&mut world, &[ids[0], ids[1]]).unwrap();
    }
    select_battle_target(&mut world, ids[1], ObjectId(2), Some(ids[2])).unwrap();
    let weapon = if mml {
        BattleWeapon::Mml9
    } else {
        BattleWeapon::Lrm20
    };
    let index = if vehicle_shooter {
        world.btech.vehicles()[&ids[0]]
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|mount| mount.weapon == weapon)
            .unwrap()
    } else {
        world.btech.constructed_units()[&ids[0]]
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|mount| mount.weapon == weapon)
            .unwrap()
    };
    (dir, config, world, ids.try_into().unwrap(), index)
}

#[tokio::test]
async fn mixed_spotters_share_native_lua_links_and_indirect_fire() {
    for (vehicle_shooter, vehicle_observer) in [(true, false), (false, true), (true, true)] {
        let (_dir, config, world, ids, index) = fixture(vehicle_shooter, vehicle_observer).await;
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let native = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        lua.eval_callback::<()>(&format!(
            "btech.unit.spot({},2,{}); btech.unit.spot({},1,{})",
            ids[1].0, ids[1].0, ids[0].0, ids[1].0
        ))
        .unwrap();
        support::run_text(
            &native,
            &config,
            ObjectId(2),
            2,
            &format!("spot #{}", ids[1].0),
        );
        support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("spot #{}", ids[1].0),
        );
        assert_eq!(lua.world().btech, native.world().btech);
        let linked = lua.world().clone();
        assert_eq!(
            battle_spotter_target(&linked, ids[0]).unwrap().target,
            ids[2]
        );
        // The firer retains its observer link but has no enemy contact and faces away from the target.
        let mut encoded = serde_json::to_value(&linked.btech).unwrap();
        let class = if vehicle_shooter {
            "vehicles"
        } else {
            "constructed"
        };
        encoded[class][ids[0].0.to_string()]["contacts"] = serde_json::json!({});
        encoded[class][ids[0].0.to_string()]["motion"]["heading"] = 180.0.into();
        if vehicle_observer {
            encoded["vehicles"][ids[1].0.to_string()]["motion"]["speed"] = 20.0.into();
            encoded["vehicles"][ids[1].0.to_string()]["motion"]["desired_speed"] = 20.0.into();
        }
        let mut prepared = linked.clone();
        prepared.btech = serde_json::from_value(encoded).unwrap();
        persistence::save(&config.database(), &prepared)
            .await
            .unwrap();
        let restored = Scripts::new(
            &config,
            Rc::new(RefCell::new(
                persistence::load(&config.database()).await.unwrap(),
            )),
        )
        .unwrap();
        let shot = Scripts::new(&config, Rc::new(RefCell::new(prepared.clone()))).unwrap();
        let command = format!(
            "local r=btech.unit.fire({},1,{index}); return r.target,r.aim.indirect.spotter,r.aim.target_lock,r.aim.indirect.target_lock,r.aim.indirect.movement",
            ids[0].0
        );
        let result: (i64, i64, i32, i32, u8) = shot.eval_callback(&command).unwrap();
        assert_eq!(
            result,
            (ids[2].0, ids[1].0, 0, 2, u8::from(vehicle_observer))
        );
        assert_eq!(
            restored
                .eval_callback::<(i64, i64, i32, i32, u8)>(&command)
                .unwrap(),
            result
        );
        assert_eq!(shot.world().btech, restored.world().btech);
        shot.world().validate(&config).unwrap();
        // The reference dispatches to the observer before resolving explicit arguments
        // when an IDF firer has no unit lock, even if the argument names another unit.
        for explicit in [ids[1], ids[2]] {
            let direct = Scripts::new(&config, Rc::new(RefCell::new(prepared.clone()))).unwrap();
            let (recipient, observer): (i64, i64) = direct.eval_callback(&format!(
                "local r=btech.unit.fire({},1,{index},{}); return r.target,r.aim.indirect.spotter",
                ids[0].0, explicit.0
            )).unwrap();
            assert_eq!((recipient, observer), (ids[2].0, ids[1].0));
            assert_eq!(direct.world().btech, shot.world().btech);
            let native = Scripts::new(&config, Rc::new(RefCell::new(prepared.clone()))).unwrap();
            support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("fire {index} #{}", explicit.0),
            );
            assert_eq!(native.world().btech, shot.world().btech);
        }

        for arguments in ["BOGUS", "1 nope", "999 999"] {
            let native = Scripts::new(&config, Rc::new(RefCell::new(prepared.clone()))).unwrap();
            let text = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("fire {index} {arguments}"),
            );
            assert!(text.contains("You fire"), "{text}");
            assert_eq!(native.world().btech, shot.world().btech);
        }
        let mut grouped_world = prepared.clone();
        let other = if vehicle_shooter {
            grouped_world.btech.vehicles()[&ids[0]]
                .loadout()
                .unwrap()
                .weapons
                .iter()
                .position(|mount| !mount.weapon.supports_indirect_fire())
                .unwrap()
        } else {
            grouped_world.btech.constructed_units()[&ids[0]]
                .loadout()
                .unwrap()
                .weapons
                .iter()
                .position(|mount| !mount.weapon.supports_indirect_fire())
                .unwrap()
        };
        edit_battle_tic(
            &mut grouped_world,
            ids[0],
            ObjectId(1),
            0,
            BattleTicEdit::Add(vec![index, other]),
        )
        .unwrap();
        let expected = Scripts::new(&config, Rc::new(RefCell::new(grouped_world.clone()))).unwrap();
        let reports =
            fire_battle_tics(&expected, &config, ids[0], ObjectId(1), vec![0], None).unwrap();
        assert_eq!(
            reports
                .iter()
                .filter(|report| report.report.is_some())
                .count(),
            1
        );
        assert_eq!(
            reports
                .iter()
                .filter(|report| report.rejection.is_some())
                .count(),
            1
        );
        for arguments in ["BOGUS", "1 nope", "999 999"] {
            let native =
                Scripts::new(&config, Rc::new(RefCell::new(grouped_world.clone()))).unwrap();
            support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("firetic 0 {arguments}"),
            );
            assert_eq!(native.world().btech, expected.world().btech);
        }

        // Ending the observer's declaration invalidates existing links without spending a shot.
        let mut invalid = linked;
        select_battle_spotter(&mut invalid, ids[1], ObjectId(2), None).unwrap();
        assert!(battle_spotter_target(&invalid, ids[0]).is_err());
    }
}

#[tokio::test]
async fn vehicle_spotter_guards_and_callback_rollback_preserve_state() {
    let (_dir, config, mut world, ids, index) = fixture(true, true).await;
    let before = world.btech.clone();
    assert!(select_battle_spotter(&mut world, ids[0], ObjectId(1), Some(ids[1])).is_err());
    assert_eq!(world.btech, before);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let command = format!(
        "btech.unit.spot({},2,{}); btech.unit.spot({},1,{}); error('undo links')",
        ids[1].0, ids[1].0, ids[0].0, ids[1].0
    );
    assert!(scripts.eval_callback::<()>(&command).is_err());
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    scripts
        .eval_callback::<()>(&format!("btech.unit.spot({},1,{})", ids[0].0, ids[0].0))
        .unwrap();
    let selected = scripts.world().btech.clone();
    scripts.drain_outbox();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.fire({},1,{index},{})",
                ids[0].0, ids[2].0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, selected);
    assert!(scripts.drain_outbox().is_empty());
    let mut recycling = scripts.world().clone();
    select_battle_spotter(&mut recycling, ids[0], ObjectId(1), None).unwrap();
    let mut encoded = serde_json::to_value(&recycling.btech).unwrap();
    encoded["vehicles"][ids[0].0.to_string()]["weapon_recycle"] =
        serde_json::json!({index.to_string(): 10});
    recycling.btech = serde_json::from_value(encoded).unwrap();
    let checkpoint = recycling.btech.clone();
    assert!(select_battle_spotter(&mut recycling, ids[0], ObjectId(1), Some(ids[0])).is_err());
    assert_eq!(recycling.btech, checkpoint);
    recycling.validate(&config).unwrap();
}

/// An occupied observer coordinate resolves the current occupant through shared indirect fire.
#[tokio::test]
async fn occupied_spotter_hexes_share_mixed_firing_without_sensor_aim_dice() {
    for (vehicle_shooter, vehicle_observer) in
        [(false, false), (false, true), (true, false), (true, true)]
    {
        let (_dir, config, mut world, ids, index) =
            fixture(vehicle_shooter, vehicle_observer).await;
        let [shooter, observer, target] = ids;
        select_battle_spotter(&mut world, observer, ObjectId(2), Some(observer)).unwrap();
        select_battle_spotter(&mut world, shooter, ObjectId(1), Some(observer)).unwrap();
        select_battle_hex_target(
            &mut world,
            observer,
            ObjectId(2),
            BattleHexCoordinate { x: 1, y: 0 },
            BattleHexTargetMode::Hex,
        )
        .unwrap();
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        let shooter_class = if vehicle_shooter {
            "vehicles"
        } else {
            "constructed"
        };
        saved[shooter_class][shooter.0.to_string()]["contacts"] = serde_json::json!({});
        saved[shooter_class][shooter.0.to_string()]["motion"]["heading"] = serde_json::json!(180.0);
        saved[shooter_class][shooter.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([42; 32])).unwrap();
        world.btech = serde_json::from_value(saved).unwrap();
        let before = world.btech.clone();
        assert_eq!(
            battle_spotter_target(&world, shooter).unwrap().target,
            target
        );
        assert_eq!(world.btech, before);
        let map = world.btech.constructed_units()[&target]
            .position()
            .unwrap()
            .map;
        let mut empty = world.clone();
        place_battle_unit(&mut empty, target, map, 2, 0).unwrap();
        let before_empty = empty.btech.clone();
        assert!(
            battle_spotter_target(&empty, shooter)
                .unwrap_err()
                .to_string()
                .contains("hex is empty")
        );
        assert_eq!(empty.btech, before_empty);
        persistence::save(&config.database(), &world).await.unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let replay = Scripts::new(
            &config,
            Rc::new(RefCell::new(
                persistence::load(&config.database()).await.unwrap(),
            )),
        )
        .unwrap();
        let call = format!("btech.unit.fire({},1,{index})", shooter.0);
        assert!(
            scripts
                .eval_callback::<()>(&format!("{call}; error('abort')"))
                .is_err()
        );
        assert_eq!(scripts.world().btech, world.btech);
        assert!(scripts.drain_outbox().is_empty());
        let code = format!(
            "local r={call}; return r.target,r.aim.perception.modifier,r.aim.indirect.spotter,r.roll or r.launch.roll"
        );
        let report: (i64, i16, i64, u8) = scripts.eval_callback(&code).unwrap();
        assert_eq!(
            report,
            (
                target.0,
                0,
                observer.0,
                BattleDice::seeded([42; 32]).two_d6()
            )
        );
        assert_eq!(
            report,
            replay.eval_callback::<(i64, i16, i64, u8)>(&code).unwrap()
        );
        let text = support::run_text(&native, &config, ObjectId(1), 1, &format!("fire {index}"));
        assert!(text.contains("You fire"), "{text}");
        assert_eq!(scripts.world().btech, native.world().btech);
        assert_eq!(scripts.world().btech, replay.world().btech);
        scripts.world().validate(&config).unwrap();
    }
}

/// Empty observer coordinates share terrain effects and launch admission across both anatomies.
#[tokio::test]
async fn empty_spotter_hexes_fire_through_blocked_firer_sightlines() {
    for (vehicle_shooter, vehicle_observer) in
        [(false, false), (false, true), (true, false), (true, true)]
    {
        let (_dir, config, mut world, ids, index) =
            fixture(vehicle_shooter, vehicle_observer).await;
        let [shooter, observer, target] = ids;
        let map = world.btech.constructed_units()[&target]
            .position()
            .unwrap()
            .map;
        place_battle_unit(&mut world, target, map, 0, 0).unwrap();
        let observer_class = if vehicle_observer {
            "vehicles"
        } else {
            "constructed"
        };
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        saved[observer_class][observer.0.to_string()]["power"] =
            serde_json::to_value(BattlePower::Off).unwrap();
        world.btech = serde_json::from_value(saved).unwrap();
        place_battle_unit(&mut world, observer, map, 2, 0).unwrap();
        assign_battle_pilot(&mut world, observer, ObjectId(2)).unwrap();
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        saved[observer_class][observer.0.to_string()]["power"] =
            serde_json::to_value(BattlePower::Running).unwrap();
        world.btech = serde_json::from_value(saved).unwrap();
        for _ in 0..10 {
            refresh_battle_contacts(&mut world, &[shooter, observer]).unwrap();
        }

        let hex = BattleHexCoordinate { x: 1, y: 0 };
        select_battle_spotter(&mut world, observer, ObjectId(2), Some(observer)).unwrap();
        select_battle_spotter(&mut world, shooter, ObjectId(1), Some(observer)).unwrap();
        select_battle_hex_target(
            &mut world,
            observer,
            ObjectId(2),
            hex,
            BattleHexTargetMode::Hex,
        )
        .unwrap();
        select_battle_hex_target(
            &mut world,
            shooter,
            ObjectId(1),
            hex,
            BattleHexTargetMode::Clear,
        )
        .unwrap();
        for _ in 0..8 {
            advance_battle_target_locks(&mut world);
        }
        let seed = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
            .unwrap();
        let class = if vehicle_shooter {
            "vehicles"
        } else {
            "constructed"
        };
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        saved["maps"][map.0.to_string()]["terrain"][4]["elevation"] = serde_json::json!(8);
        saved["maps"][map.0.to_string()]["terrain"][1]["terrain"] =
            serde_json::json!("heavy_forest");
        saved[class][shooter.0.to_string()]["contacts"] = serde_json::json!({});
        saved[class][shooter.0.to_string()]["motion"]["heading"] = serde_json::json!(180.0);
        saved[class][shooter.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        world.btech = serde_json::from_value(saved).unwrap();
        assert!(!battle_hex_visible(&world, shooter, hex).unwrap());
        assert!(battle_hex_visible(&world, observer, hex).unwrap());
        persistence::save(&config.database(), &world).await.unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let replay = Scripts::new(
            &config,
            Rc::new(RefCell::new(
                persistence::load(&config.database()).await.unwrap(),
            )),
        )
        .unwrap();
        let call = format!("btech.unit.fire({},1,{index})", shooter.0);
        assert!(
            scripts
                .eval_callback::<()>(&format!("{call}; error('abort')"))
                .is_err()
        );
        assert_eq!(scripts.world().btech, world.btech);
        assert!(scripts.drain_outbox().is_empty());
        let code = format!(
            "local r={call}; return r.launched,r.hit,r.roll,r.aim.indirect.spotter,r.coordinate.x,r.coordinate.y,#r.terrain"
        );
        let report: (bool, bool, u8, i64, i32, i32, usize) = scripts.eval_callback(&code).unwrap();
        assert_eq!(
            (report.0, report.1, report.2, report.3, report.4, report.5),
            (true, true, 12, observer.0, 1, 0)
        );
        assert!(report.6 > 0);
        assert_eq!(
            report,
            replay
                .eval_callback::<(bool, bool, u8, i64, i32, i32, usize)>(&code)
                .unwrap()
        );
        let text = support::run_text(&native, &config, ObjectId(1), 1, &format!("fire {index}"));
        assert!(
            text.contains("You fire") && text.contains("at (1,0)"),
            "{text}"
        );
        assert_eq!(scripts.world().btech, native.world().btech);
        assert_eq!(scripts.world().btech, replay.world().btech);
        scripts.world().validate(&config).unwrap();
        let invalid = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        select_battle_spotter(&mut invalid.world_mut(), observer, ObjectId(2), None).unwrap();
        let before = invalid.world().btech.clone();
        assert!(invalid.eval_callback::<()>(&call).is_err());
        assert_eq!(invalid.world().btech, before);
        let unlocked = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        select_battle_target(&mut unlocked.world_mut(), shooter, ObjectId(1), None).unwrap();
        let result: (bool, i32, i32) = unlocked
            .eval_callback(&format!(
                "local r={call}; return r.launched,r.coordinate.x,r.coordinate.y"
            ))
            .unwrap();
        assert_eq!(result, (true, 1, 0));
        let blind = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        // The observer's sensor band reaches the adjacent hex in any weather, so silence it too.
        set_battle_map_visibility(&mut blind.world_mut(), map, BattleLight::Day, 0).unwrap();
        set_battle_map_perception(
            &mut blind.world_mut(),
            map,
            BattleMapPerceptionFlag::Sensors,
            false,
        )
        .unwrap();
        let before = blind.world().btech.clone();
        assert!(blind.eval_callback::<()>(&call).is_err());
        assert_eq!(blind.world().btech, before);
        assert!(blind.drain_outbox().is_empty());
    }
}

/// Both shooter anatomies use the same pre-impact awards and current-shot skill advancement.
#[tokio::test]
async fn mixed_indirect_experience_shares_eligibility_levels_and_rollback() {
    for (vehicle_shooter, vehicle_observer) in
        [(false, false), (false, true), (true, false), (true, true)]
    {
        let (_dir, config, mut world, ids, index) =
            fixture(vehicle_shooter, vehicle_observer).await;
        let [shooter, observer, target] = ids;
        select_battle_spotter(&mut world, observer, ObjectId(2), Some(observer)).unwrap();
        select_battle_spotter(&mut world, shooter, ObjectId(1), Some(observer)).unwrap();
        for id in ids {
            world
                .objects
                .get_mut(&id)
                .unwrap()
                .flags
                .insert(Flag::InCharacter);
        }
        for pilot in [ObjectId(1), ObjectId(2)] {
            world
                .objects
                .get_mut(&pilot)
                .unwrap()
                .flags
                .insert(Flag::Connected);
            set_battle_character(
                &mut world,
                pilot,
                BattleCharacter {
                    build: 5,
                    reflexes: 5,
                    intuition: 5,
                    learn: 5,
                    charisma: 5,
                    bruise: 0,
                    lethal: 0,
                },
            )
            .unwrap();
        }
        world
            .objects
            .get_mut(&ObjectId(2))
            .unwrap()
            .flags
            .remove(Flag::Wizard);
        let class = if vehicle_shooter {
            "vehicles"
        } else {
            "constructed"
        };
        let seed = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 2)
            .unwrap();
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        saved[class][shooter.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        world.btech = serde_json::from_value(saved).unwrap();
        for case in [
            "ordinary",
            "level",
            "cooldown",
            "coordinate",
            "target",
            "observer",
            "firer",
            "disconnected",
        ] {
            let mut trial = world.clone();
            match case {
                "level" => set_battle_character_value(
                    &mut trial,
                    ObjectId(2),
                    "Gunnery-Spotting",
                    BattleCharacterValue {
                        experience: 1,
                        ..Default::default()
                    },
                )
                .unwrap(),
                "cooldown" => set_battle_character_value(
                    &mut trial,
                    ObjectId(2),
                    "Gunnery-Spotting",
                    BattleCharacterValue {
                        last_used: i64::MAX,
                        ..Default::default()
                    },
                )
                .unwrap(),
                "coordinate" => {
                    select_battle_hex_target(
                        &mut trial,
                        observer,
                        ObjectId(2),
                        BattleHexCoordinate { x: 1, y: 0 },
                        BattleHexTargetMode::Hex,
                    )
                    .unwrap();
                }
                "target" => {
                    trial
                        .objects
                        .get_mut(&target)
                        .unwrap()
                        .flags
                        .remove(Flag::InCharacter);
                }
                "observer" => {
                    trial
                        .objects
                        .get_mut(&observer)
                        .unwrap()
                        .flags
                        .remove(Flag::InCharacter);
                }
                "firer" => {
                    trial
                        .objects
                        .get_mut(&shooter)
                        .unwrap()
                        .flags
                        .remove(Flag::InCharacter);
                }
                "disconnected" => {
                    trial
                        .objects
                        .get_mut(&ObjectId(2))
                        .unwrap()
                        .flags
                        .remove(Flag::Connected);
                }
                _ => {}
            }
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(trial.clone()))).unwrap();
            let call = format!("btech.unit.fire({},1,{index})", shooter.0);
            assert!(
                scripts
                    .eval_callback::<()>(&format!("{call}; error('abort')"))
                    .is_err()
            );
            assert_eq!(scripts.world().btech, trial.btech);
            assert!(scripts.drain_outbox().is_empty());
            let result: (usize, bool, i16) = scripts.eval_callback(&format!("local r={call}; return #r.experience_messages,r.salvo==nil,r.aim.indirect.spotting")).unwrap();
            let spotting = !matches!(
                case,
                "cooldown" | "coordinate" | "target" | "observer" | "disconnected"
            );
            let artillery = !matches!(case, "coordinate" | "target" | "firer");
            assert_eq!(
                result.0,
                usize::from(spotting) + usize::from(artillery),
                "{case}"
            );
            assert!(result.1);
            if case == "level" {
                assert_eq!(result.2, 7);
            }
            for (pilot, skill, awarded) in [
                (ObjectId(1), "Gunnery-Artillery", artillery),
                (ObjectId(2), "Gunnery-Spotting", spotting),
            ] {
                let balance = scripts
                    .world()
                    .btech
                    .character_values()
                    .get(&pilot)
                    .and_then(|values| values.get(skill))
                    .map_or(0, |value| value.experience_balance());
                assert_eq!(
                    balance,
                    u32::from(awarded) + u32::from(case == "level" && pilot == ObjectId(2)),
                    "{case}: {skill}"
                );
            }
            let result = scripts.world().clone();
            result.validate(&config).unwrap();
            if matches!(case, "ordinary" | "level") {
                persistence::save(&config.database(), &result)
                    .await
                    .unwrap();
                let restored = persistence::load(&config.database()).await.unwrap();
                assert_eq!(restored.btech, result.btech);
            }
        }
    }
}

/// MML observer dispatch follows each installation's selected family for both anatomy adapters.
#[tokio::test]
async fn mml_indirect_fire_requires_lrm_selection() {
    for (vehicle_shooter, vehicle_observer) in
        [(false, false), (false, true), (true, false), (true, true)]
    {
        let (_dir, config, mut world, ids, index) =
            fixture_with_mml(vehicle_shooter, vehicle_observer, true).await;
        select_battle_spotter(&mut world, ids[1], ObjectId(2), Some(ids[1])).unwrap();
        select_battle_spotter(&mut world, ids[0], ObjectId(1), Some(ids[1])).unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let sight = format!("return btech.unit.sight({},1,{index})", ids[0].0);
        let report: mlua::Table = scripts.eval_callback(&sight).unwrap();
        let aim: mlua::Table = report.get("aim").unwrap();
        let indirect: mlua::Table = aim.get("indirect").unwrap();
        assert_eq!(indirect.get::<i64>("spotter").unwrap(), ids[1].0);
        let original = scripts.world().btech.clone();
        let mode: String = scripts
            .eval_callback(&format!("return btech.unit.mml({},1,{index})", ids[0].0))
            .unwrap();
        assert_eq!(mode, "normal");
        let srm = scripts.world().btech.clone();
        let error = scripts.eval_callback::<mlua::Table>(&sight).unwrap_err();
        assert!(error.to_string().contains("Remove your spotter"), "{error}");
        assert!(
            scripts
                .eval_callback::<mlua::Table>(&format!(
                    "return btech.unit.fire({},1,{index})",
                    ids[0].0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, srm);
        scripts
            .eval_callback::<()>(&format!("btech.unit.mml({},1,{index})", ids[0].0))
            .unwrap();
        // Toggling twice restores the post-sighting state, including its observer dice.
        assert_eq!(scripts.world().btech, original);
        let fired: mlua::Table = scripts
            .eval_callback(&format!("return btech.unit.fire({},1,{index})", ids[0].0))
            .unwrap();
        assert_eq!(fired.get::<i64>("target").unwrap(), ids[2].0);
        let aim: mlua::Table = fired.get("aim").unwrap();
        let indirect: mlua::Table = aim.get("indirect").unwrap();
        assert_eq!(indirect.get::<i64>("spotter").unwrap(), ids[1].0);
        scripts.world().validate(&config).unwrap();
    }
}
