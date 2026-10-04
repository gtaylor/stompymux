//! Shared tow ownership, validation, selective persistence, and destruction cleanup.
use crate::support;
use stompymux_rs::*;

const CHASSIS: [&str; 3] = [
    include_str!("fixtures/btech/mechs/JR7-D.toml"),
    include_str!("../game/mechs/Demolisher.toml"),
    include_str!("../game/mechs/Kestrel.toml"),
];

/// Construct stationary units together without imposing pickup equipment policy.
async fn fixture(sources: &[&str]) -> (tempfile::TempDir, Config, World, ObjectId, Vec<ObjectId>) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Tow yard".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "yard",
        MapAsset::from_cells("2 2\n.0.0\n.0.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let mut units = Vec::new();
    for source in sources {
        let id = world.create(&config, "Unit".into(), Kind::Thing);
        BattleUnitTemplate::parse("test", source)
            .unwrap()
            .create(&mut world, id)
            .unwrap();
        support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
        place_battle_unit(&mut world, id, map, 0, 0).unwrap();
        units.push(id);
    }
    (dir, config, world, map, units)
}

#[tokio::test]
async fn tow_pairs_share_all_chassis_and_reject_overlap_without_mutation() {
    for carrier in CHASSIS {
        for target in CHASSIS {
            let (_dir, config, mut world, map, ids) = fixture(&[carrier, target, CHASSIS[0]]).await;
            let [a, b, c] = ids[..] else { unreachable!() };
            set_battle_tow(&mut world, a, Some(b)).unwrap();
            assert_eq!(world.btech.tows().get(&a), Some(&b));
            assert_eq!(world.btech.towed_by(b), Some(a));
            let before = world.btech.clone();
            for id in [a, b] {
                assert!(place_battle_unit(&mut world, id, map, 1, 0).is_err());
                assert!(remove_battle_unit(&mut world, id, ObjectId(0)).is_err());
                assert_eq!(world.btech, before);
            }
            for (carrier, target) in [(a, a), (a, c), (b, c), (c, a), (c, b), (a, b)] {
                assert!(set_battle_tow(&mut world, carrier, Some(target)).is_err());
                assert_eq!(world.btech, before);
            }
            world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(b);
            assign_battle_pilot(&mut world, b, ObjectId(1)).unwrap();
            support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
            let before = world.btech.clone();
            assert!(
                start_battle_unit(&mut world, b, ObjectId(1), true)
                    .unwrap_err()
                    .to_string()
                    .contains("Detach tow")
            );
            assert_eq!(world.btech, before);
            world.validate(&config).unwrap();
            set_battle_tow(&mut world, a, None).unwrap();
            assert!(world.btech.tows().is_empty());
            assert_eq!(world.btech.towed_by(b), None);
            start_battle_unit(&mut world, b, ObjectId(1), true).unwrap();
            let before = world.btech.clone();
            assert!(set_battle_tow(&mut world, a, Some(b)).is_err());
            assert_eq!(world.btech, before);
        }
    }
}

#[tokio::test]
async fn tow_validation_rejects_bad_placement_and_corrupt_relationships() {
    let (_dir, config, mut world, map, ids) = fixture(&CHASSIS).await;
    let [a, b, c] = ids[..] else { unreachable!() };
    place_battle_unit(&mut world, b, map, 1, 0).unwrap();
    assert!(set_battle_tow(&mut world, a, Some(b)).is_err());
    place_battle_unit(&mut world, b, map, 0, 0).unwrap();
    for pairs in [
        vec![(a, a)],
        vec![(a, b), (c, b)],
        vec![(a, b), (b, c)],
        vec![(a, ObjectId(999999))],
    ] {
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        encoded["tows"] = serde_json::to_value(
            pairs
                .into_iter()
                .collect::<std::collections::BTreeMap<_, _>>(),
        )
        .unwrap();
        let mut invalid = world.clone();
        invalid.btech = serde_json::from_value(encoded).unwrap();
        assert!(invalid.validate(&config).is_err());
    }
    world.objects.get_mut(&b).unwrap().flags.insert(Flag::Going);
    assert!(set_battle_tow(&mut world, a, Some(b)).is_err());
    assert!(world.btech.tows().is_empty());
}

#[tokio::test]
async fn tow_partner_swaps_preserve_extra_columns_and_replay() {
    use sqlx::Connection;
    let (_dir, config, mut world, _, ids) =
        fixture(&[CHASSIS[0], CHASSIS[1], CHASSIS[2], CHASSIS[0]]).await;
    let [a, b, c, d] = ids[..] else {
        unreachable!()
    };
    set_battle_tow(&mut world, a, Some(b)).unwrap();
    set_battle_tow(&mut world, c, Some(d)).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut sql = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query("ALTER TABLE btech_tows ADD COLUMN annotation TEXT NOT NULL DEFAULT 'retained'")
        .execute(&mut sql)
        .await
        .unwrap();
    sqlx::query("UPDATE btech_tows SET annotation='custom value'")
        .execute(&mut sql)
        .await
        .unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    set_battle_tow(&mut world, a, None).unwrap();
    set_battle_tow(&mut world, c, None).unwrap();
    set_battle_tow(&mut world, a, Some(d)).unwrap();
    set_battle_tow(&mut world, c, Some(b)).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    let annotations: Vec<String> =
        sqlx::query_scalar("SELECT annotation FROM btech_tows ORDER BY carrier_dbref")
            .fetch_all(&mut sql)
            .await
            .unwrap();
    assert_eq!(annotations, ["custom value", "custom value"]);
    set_battle_tow(&mut world, a, None).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    // The world invariant, not update order, enforces unique targets across both columns.
    sqlx::query("INSERT INTO btech_tows(carrier_dbref,target_dbref) VALUES (?,?)")
        .bind(a.0)
        .bind(b.0)
        .execute(&mut sql)
        .await
        .unwrap();
    assert!(persistence::load(&config.database()).await.is_err());
}

#[tokio::test]
async fn purging_either_endpoint_or_battlefield_removes_tow_relationships() {
    for endpoint in 0..3 {
        let (_dir, config, mut world, map, ids) = fixture(&CHASSIS[..2]).await;
        let [a, b] = ids[..] else { unreachable!() };
        set_battle_tow(&mut world, a, Some(b)).unwrap();
        let removed = [a, b, map][endpoint];
        world
            .objects
            .get_mut(&removed)
            .unwrap()
            .flags
            .insert(Flag::Going);
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        persistence::repair(&config.database(), config.database.busy_timeout_ms, |raw| {
            dbck::plan(&world, raw, &config)
        })
        .await
        .unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert!(loaded.btech.tows().is_empty());
        loaded.validate(&config).unwrap();
    }
}

/// Read chassis storage only for cross-type assertions; gameplay uses the shared projection.
fn state(world: &World, id: ObjectId) -> serde_json::Value {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        serde_json::to_value(unit).unwrap()
    } else {
        serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()
    }
}

#[tokio::test]
async fn towing_follows_position_facing_and_height_for_every_chassis_pair() {
    for (carrier_type, carrier) in CHASSIS.into_iter().enumerate() {
        for target in CHASSIS {
            let (_dir, config, mut world, _, ids) = fixture(&[carrier, target]).await;
            let [a, b] = ids[..] else { unreachable!() };
            world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(a);
            assign_battle_pilot(&mut world, a, ObjectId(1)).unwrap();
            support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
            start_battle_unit(&mut world, a, ObjectId(1), true).unwrap();
            for _ in 0..5 {
                advance_battle_units(&mut world, 0);
            }
            let mut encoded = serde_json::to_value(&world.btech).unwrap();
            let key = if carrier_type == 0 {
                "constructed"
            } else {
                "vehicles"
            };
            let source = &mut encoded[key][a.0.to_string()];
            source["motion"]["heading"] = 90.0.into();
            source["motion"]["desired_heading"] = 90.0.into();
            source["motion"]["speed"] = 20.0.into();
            source["motion"]["desired_speed"] = 20.0.into();
            if carrier_type == 2 {
                source["vtol_flight"]["phase"] = serde_json::json!({"kind":"airborne"});
                source["vtol_flight"]["altitude"] = 20.0.into();
            }
            world.btech = serde_json::from_value(encoded).unwrap();
            set_battle_tow(&mut world, a, Some(b)).unwrap();
            let original = state(&world, b);
            let unloaded = if let Some(unit) = world.btech.vehicles().get(&a) {
                unit.maximum_speed()
            } else {
                world.btech.constructed_units()[&a].mobility().maximum_speed
            };
            let maximum = battle_unit_load(&world, a, true)
                .unwrap()
                .maximum_speed(unloaded)
                .unwrap();
            for _ in 0..5 {
                advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
                let source = state(&world, a);
                let target = state(&world, b);
                assert!(source["motion"]["speed"].as_f64().unwrap().abs() <= maximum);
                assert!(source["motion"]["desired_speed"].as_f64().unwrap().abs() <= maximum);
                if maximum == 0.0 {
                    assert_eq!(source["motion"]["point"], original["motion"]["point"]);
                }
                assert_eq!(source["position"], target["position"]);
                assert_eq!(source["motion"]["point"], target["motion"]["point"]);
                assert_eq!(source["motion"]["heading"], target["motion"]["heading"]);
                assert_eq!(source["motion"]["speed"], target["motion"]["speed"]);
                assert_eq!(original["map_slot"], target["map_slot"]);
                assert_eq!(original["dice"], target["dice"]);
                assert_eq!(target["motion"]["desired_speed"].as_f64(), Some(0.0));
                assert_eq!(
                    if target["vtol_flight"].is_object() {
                        target["vtol_flight"]["altitude"].as_f64()
                    } else {
                        target["ground_elevation"].as_f64()
                    },
                    Some(if carrier_type == 2 { 20.0 } else { 0.0 })
                );
                world.validate(&config).unwrap();
            }
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
            if carrier_type == 0 {
                let before = world.btech.clone();
                assert!(launch_battle_jump(&mut world, a, ObjectId(1), 90, 1.0).is_err());
                assert_eq!(world.btech, before);
            }
        }
    }
}

#[tokio::test]
async fn external_speed_exceeds_disabled_target_limits_but_requires_a_tow() {
    for speed in [160.0_f64, -100.0] {
        let (_dir, config, mut world, _, ids) = fixture(&[
            CHASSIS[2],
            include_str!("../game/mechs/Savannah_Master.toml"),
        ])
        .await;
        let [a, b] = ids[..] else { unreachable!() };
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(a);
        assign_battle_pilot(&mut world, a, ObjectId(1)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        start_battle_unit(&mut world, a, ObjectId(1), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        let source = &mut encoded["vehicles"][a.0.to_string()];
        let heading = if speed < 0.0 { 270.0 } else { 90.0 };
        source["motion"]["heading"] = heading.into();
        source["motion"]["desired_heading"] = heading.into();
        source["motion"]["speed"] = speed.into();
        source["motion"]["desired_speed"] = speed.into();
        source["vtol_flight"]["phase"] = serde_json::json!({"kind":"airborne"});
        source["vtol_flight"]["altitude"] = 5.0.into();
        encoded["vehicles"][b.0.to_string()]["immobilized"] = true.into();
        world.btech = serde_json::from_value(encoded).unwrap();
        set_battle_tow(&mut world, a, Some(b)).unwrap();
        let maximum = battle_unit_load(&world, a, true)
            .unwrap()
            .maximum_speed(world.btech.vehicles()[&a].maximum_speed())
            .unwrap();
        let expected = speed.clamp(-maximum * 2.0 / 3.0, maximum);
        advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
        assert_eq!(world.btech.vehicles()[&b].maximum_speed(), 0.0);
        assert_eq!(world.btech.vehicles()[&b].motion().unwrap().speed, expected);
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
        advance_battle_motion(&mut replay, BattleMovementRules::STANDARD).unwrap();
        assert_eq!(world.btech, replay.btech);
        let mut invalid = world.clone();
        let mut encoded = serde_json::to_value(&invalid.btech).unwrap();
        encoded["tows"] = serde_json::json!({});
        invalid.btech = serde_json::from_value(encoded).unwrap();
        assert!(invalid.validate(&config).is_err());
        set_battle_tow(&mut world, a, None).unwrap();
        let stopped = world.btech.vehicles()[&b].motion().unwrap();
        assert_eq!(stopped.speed, 0.0);
        assert_eq!(stopped.desired_speed, 0.0);
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn towing_load_shares_equipment_discounts_and_live_mass_across_chassis() {
    for source in CHASSIS {
        for target in CHASSIS {
            for equipment in [
                "",
                "SalvageTech",
                "Carrier_Tech",
                "SalvageTech Carrier_Tech",
            ] {
                let flags: Vec<_> = equipment.split_whitespace().collect();
                let carrier = crate::support::templates::with_flags(source, &flags);
                assert!(flags.iter().all(|flag| carrier.contains(flag)));
                let (_dir, config, mut world, _, ids) = fixture(&[&carrier, target]).await;
                let [a, b] = ids[..] else { unreachable!() };
                set_battle_tow(&mut world, a, Some(b)).unwrap();
                let before = world.btech.clone();
                let load = battle_unit_load(&world, a, false).unwrap();
                let mass = if let Some(unit) = world.btech.vehicles().get(&b) {
                    unit.mass().unwrap().total
                } else {
                    world.btech.constructed_units()[&b].mass().unwrap().total
                };
                let divisor = 1 << equipment.split_whitespace().count();
                assert_eq!(load.carried_mass, u64::from(mass) * 2 / divisor);
                assert_eq!(world.btech, before);
                assert!(load.maximum_speed(100.0).unwrap() <= 100.0);
                world.btech.set_unit_ammunition_bin(b, 0, 0).unwrap();
                let lighter = battle_unit_load(&world, a, false).unwrap();
                assert!(lighter.carried_mass < load.carried_mass);
                assert!(
                    lighter.maximum_speed(100.0).unwrap() >= load.maximum_speed(100.0).unwrap()
                );
                persistence::save(&config.database(), &world).await.unwrap();
                let replay = persistence::load(&config.database()).await.unwrap();
                assert_eq!(battle_unit_load(&replay, a, false).unwrap(), lighter);
            }
        }
    }
}

#[tokio::test]
async fn hot_myomer_tow_discount_requires_hardware_heat_and_configuration() {
    let (_dir, config, mut world, map, ids) = fixture(&CHASSIS[..1]).await;
    let mut template = BattleTemplate::parse("test", CHASSIS[0]).unwrap();
    let mut remaining = 6;
    for location in [BattleSection::LeftTorso, BattleSection::RightTorso] {
        let section = template.sections.get_mut(&location).unwrap();
        for slot in 0..12 {
            if remaining == 0 || section.criticals.contains_key(&slot) {
                continue;
            }
            section.criticals.insert(
                slot,
                CriticalDefinition {
                    equipment: "TripleStrengthMyomer".into(),
                    data: "-".into(),
                    modes: vec![],
                },
            );
            remaining -= 1;
        }
    }
    assert_eq!(remaining, 0);
    let carrier = world.create(&config, "Myomer carrier".into(), Kind::Thing);
    create_battle_unit(&mut world, carrier, template).unwrap();
    support::seed_object_dice(&mut world, carrier, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, carrier, map, 0, 0).unwrap();
    set_battle_tow(&mut world, carrier, Some(ids[0])).unwrap();
    let undiscounted = battle_unit_load(&world, carrier, false)
        .unwrap()
        .carried_mass;
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(carrier);
    assign_battle_pilot(&mut world, carrier, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, carrier, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }

    let observer = world.create(&config, "Observer".into(), Kind::Thing);
    create_battle_unit(
        &mut world,
        observer,
        BattleTemplate::parse("test", CHASSIS[0]).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, observer, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, observer, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(observer);
    assign_battle_pilot(&mut world, observer, ObjectId(2)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, observer, ObjectId(2), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    set_battle_observer(&mut world, observer, true).unwrap();
    world
        .btech
        .rewrite_unit_record(observer, |record| {
            record["contacts"][carrier.0.to_string()] = serde_json::json!({"identified": true});
        })
        .unwrap();
    for heat in [8.0, 9.0] {
        world
            .btech
            .rewrite_unit_record(carrier, |record| {
                record["heat"]["excess"] = heat.into();
            })
            .unwrap();
        for enabled in [false, true] {
            let load = battle_unit_load(&world, carrier, enabled).unwrap();
            let path = config.root.join("stompymux.toml");
            let mut settings: toml::Value =
                toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            settings
                .as_table_mut()
                .unwrap()
                .entry("battletech")
                .or_insert_with(|| toml::Value::Table(Default::default()))
                .as_table_mut()
                .unwrap()
                .insert(
                    "tsm_tow_bonus".into(),
                    toml::Value::Integer(i64::from(enabled)),
                );
            std::fs::write(&path, toml::to_string(&settings).unwrap()).unwrap();
            let configured = Config::load(&config.root).unwrap();
            let native = Scripts::new(
                &configured,
                std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
            )
            .unwrap();
            let lua = Scripts::new(
                &configured,
                std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
            )
            .unwrap();
            let maximum = battle_throttle_maximum(&world, carrier, enabled).unwrap();
            let output = support::run_text(&native, &configured, ObjectId(1), 1, "speed run");
            assert!(output.contains("Desired speed changed to "), "{output}");
            lua.eval_callback::<()>(&format!("btech.unit.speed({},1,'run')", carrier.0))
                .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            assert_eq!(
                state(&lua.world(), carrier)["motion"]["desired_speed"].as_f64(),
                Some(maximum)
            );
            assert_eq!(
                lua.eval_callback::<f64>(&format!(
                    "return btech.unit.state({}).movement_maximum_speed",
                    carrier.0
                ))
                .unwrap(),
                maximum
            );

            let unchanged = lua.world().btech.clone();
            lua.drain_outbox();
            let native_status =
                support::run_text(&native, &configured, ObjectId(1), 1, "status info");
            let lua_status: String = lua
                .eval_callback(&format!("return btech.unit.status({},'info')", carrier.0))
                .unwrap();
            let scan = scan_battle_unit_action(&lua, observer, ObjectId(2), carrier, "i").unwrap();
            let expected = format!("MaxSpeed: {:3}", maximum as i32);
            for report in [&native_status, &lua_status, &scan] {
                assert!(report.contains(&expected), "{report}");
            }
            assert_eq!(lua.world().btech, unchanged);
            assert_eq!(native.world().btech, unchanged);
            assert!(lua.drain_outbox().is_empty());
            assert_eq!(
                load.carried_mass,
                if enabled && heat >= 9.0 {
                    undiscounted / 2
                } else {
                    undiscounted
                }
            );
        }
    }
}

#[tokio::test]
async fn loaded_acceleration_and_reverse_motion_use_the_same_ceiling_for_all_chassis() {
    for (index, source) in CHASSIS.into_iter().enumerate() {
        for reverse in [false, true] {
            let (_dir, config, mut world, _, ids) =
                fixture(&[source, include_str!("../game/mechs/Savannah_Master.toml")]).await;
            let [a, b] = ids[..] else { unreachable!() };
            world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(a);
            assign_battle_pilot(&mut world, a, ObjectId(1)).unwrap();
            support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
            start_battle_unit(&mut world, a, ObjectId(1), true).unwrap();
            for _ in 0..5 {
                advance_battle_units(&mut world, 0);
            }
            let unloaded = if let Some(unit) = world.btech.vehicles().get(&a) {
                unit.maximum_speed()
            } else {
                world.btech.constructed_units()[&a].mobility().maximum_speed
            };
            let mut encoded = serde_json::to_value(&world.btech).unwrap();
            let key = if index == 0 {
                "constructed"
            } else {
                "vehicles"
            };
            let unit = &mut encoded[key][a.0.to_string()];
            let heading = if reverse { 270.0 } else { 90.0 };
            unit["motion"]["heading"] = heading.into();
            unit["motion"]["desired_heading"] = heading.into();
            unit["motion"]["desired_speed"] = (if reverse {
                -unloaded * 2.0 / 3.0
            } else {
                unloaded
            })
            .into();
            if index == 2 {
                unit["vtol_flight"]["phase"] = serde_json::json!({"kind":"airborne"});
                unit["vtol_flight"]["altitude"] = 5.0.into();
            }
            world.btech = serde_json::from_value(encoded).unwrap();
            set_battle_tow(&mut world, a, Some(b)).unwrap();
            let maximum = battle_unit_load(&world, a, true)
                .unwrap()
                .maximum_speed(unloaded)
                .unwrap();
            assert!(maximum > 0.0 && maximum < unloaded);
            advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
            let actual = state(&world, a)["motion"]["speed"].as_f64().unwrap();
            let expected = maximum / 20.0 * if reverse { -1.0 } else { 1.0 };
            assert!(
                (actual - expected).abs() < 1e-9,
                "chassis {index}: {actual} != {expected}"
            );
            assert_eq!(
                state(&world, a)["motion"]["speed"],
                state(&world, b)["motion"]["speed"]
            );
            world.validate(&config).unwrap();
            persistence::save(&config.database(), &world).await.unwrap();
            let mut replay = persistence::load(&config.database()).await.unwrap();
            advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
            advance_battle_motion(&mut replay, BattleMovementRules::STANDARD).unwrap();
            assert_eq!(world.btech, replay.btech);
        }
    }
}

#[tokio::test]
async fn native_and_lua_tow_speed_limits_and_reports_agree() {
    for (index, source) in CHASSIS.into_iter().enumerate() {
        let (_dir, config, mut world, _, ids) =
            fixture(&[source, include_str!("../game/mechs/Savannah_Master.toml")]).await;
        let [a, b] = ids[..] else { unreachable!() };
        // This throttle-envelope test includes reverse, which requires towing equipment.
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        let key = if index == 0 {
            "constructed"
        } else {
            "vehicles"
        };
        let old = encoded[key][a.0.to_string()]["definition"]["attributes"]["specials"]
            .as_str()
            .unwrap_or("");
        encoded[key][a.0.to_string()]["definition"]["attributes"]["specials"] =
            format!("{} SalvageTech", old.trim_matches('-')).into();
        world.btech = serde_json::from_value(encoded).unwrap();
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(a);
        assign_battle_pilot(&mut world, a, ObjectId(1)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        start_battle_unit(&mut world, a, ObjectId(1), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
        if index == 2 {
            world
                .btech
                .rewrite_unit_record(a, |record| {
                    record["vtol_flight"]["phase"] = serde_json::json!({"kind":"airborne"});
                    record["vtol_flight"]["altitude"] = 5.0.into();
                })
                .unwrap();
        }
        set_battle_tow(&mut world, a, Some(b)).unwrap();
        let maximum = battle_throttle_maximum(&world, a, true).unwrap();
        let horizontal = if index == 2 {
            world
                .btech
                .rewrite_unit_record(a, |record| {
                    record["vtol_flight"]["vertical_speed"] = (maximum * 0.6).into();
                })
                .unwrap();
            maximum * 0.8
        } else {
            maximum
        };
        for (argument, expected) in [
            ("run", horizontal),
            ("flank", horizontal),
            ("walk", maximum * 2.0 / 3.0),
            ("cruise", maximum * 2.0 / 3.0),
            ("back", -horizontal * 2.0 / 3.0),
            ("99999", horizontal),
            ("-99999", -horizontal * 2.0 / 3.0),
        ] {
            let native = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
            )
            .unwrap();
            let lua = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
            )
            .unwrap();
            let output = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("speed {argument}"),
            );
            assert!(output.contains("Desired speed changed to "), "{output}");
            lua.eval_callback::<()>(&format!("btech.unit.speed({},1,'{}')", a.0, argument))
                .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            assert!(
                (state(&native.world(), a)["motion"]["desired_speed"]
                    .as_f64()
                    .unwrap()
                    - expected)
                    .abs()
                    < 1e-9
            );
            let field = if index == 0 {
                "movement_maximum_speed"
            } else {
                "maximum_speed"
            };
            let reported: f64 = lua
                .eval_callback(&format!("return btech.unit.state({}).{}", a.0, field))
                .unwrap();
            assert_eq!(reported, maximum);
            lua.drain_outbox();
            let before = lua.world().btech.clone();
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.speed({},1,'stop'); error('abort speed')",
                    a.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
        }
    }
}

/// Reverse admission shares towing ownership and the salvage exception across mobile chassis.
#[tokio::test]
async fn reverse_towing_guard_is_shared_by_native_lua_and_direct_controls() {
    let ground = CHASSIS[1];
    for source in [
        CHASSIS[0].to_owned(),
        include_str!("../game/mechs/GOL-1H.toml").to_owned(),
        ground.to_owned(),
        ground.replace("movement = \"track\"", "movement = \"wheel\""),
        ground.replace("movement = \"track\"", "movement = \"hover\""),
        CHASSIS[2].to_owned(),
    ] {
        let (_dir, config, mut world, _, ids) =
            fixture(&[&source, include_str!("../game/mechs/Savannah_Master.toml")]).await;
        let [carrier, target] = ids[..] else {
            unreachable!()
        };
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(carrier);
        assign_battle_pilot(&mut world, carrier, ObjectId(1)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        start_battle_unit(&mut world, carrier, ObjectId(1), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
        let key = if world.btech.vehicles().contains_key(&carrier) {
            "vehicles"
        } else {
            "constructed"
        };
        if world
            .btech
            .vehicles()
            .get(&carrier)
            .is_some_and(|u| u.definition().is_vtol())
        {
            world
                .btech
                .rewrite_unit_record(carrier, |record| {
                    record["vtol_flight"]["phase"] = serde_json::json!({"kind":"airborne"});
                    record["vtol_flight"]["altitude"] = 5.0.into();
                })
                .unwrap();
        }
        set_battle_tow(&mut world, carrier, Some(target)).unwrap();
        for salvage in [false, true] {
            for speed in [0.0, 1.0] {
                let mut forward = world.clone();
                set_battle_speed(&mut forward, carrier, ObjectId(1), speed).unwrap();
            }
            let mut detached = world.clone();
            set_battle_tow(&mut detached, carrier, None).unwrap();
            set_battle_speed(&mut detached, carrier, ObjectId(1), -1.0).unwrap();
            if salvage {
                let mut encoded = serde_json::to_value(&world.btech).unwrap();
                let old =
                    encoded[key][carrier.0.to_string()]["definition"]["attributes"]["specials"]
                        .as_str()
                        .unwrap_or("");
                encoded[key][carrier.0.to_string()]["definition"]["attributes"]["specials"] =
                    format!("{} SalvageTech", old.trim_matches('-')).into();
                world.btech = serde_json::from_value(encoded).unwrap();
            }
            persistence::save(&config.database(), &world).await.unwrap();
            let restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(world.btech, restored.btech);
            let mut direct = restored.clone();
            let result = set_battle_speed(&mut direct, carrier, ObjectId(1), -1.0);
            assert_eq!(result.is_ok(), salvage, "{result:?}");
            if !salvage {
                assert_eq!(
                    result.unwrap_err().to_string(),
                    "You can not backup while towing!"
                );
                assert_eq!(direct.btech, world.btech);
            }
            for argument in ["back", "-99999"] {
                let native = Scripts::new(
                    &config,
                    std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
                )
                .unwrap();
                let lua = Scripts::new(
                    &config,
                    std::rc::Rc::new(std::cell::RefCell::new(restored.clone())),
                )
                .unwrap();
                let output = support::run_text(
                    &native,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("speed {argument}"),
                );
                let result = lua.eval_callback::<()>(&format!(
                    "btech.unit.speed({},1,'{argument}')",
                    carrier.0
                ));
                assert_eq!(result.is_ok(), salvage, "{result:?}");
                assert_eq!(native.world().btech, lua.world().btech);
                if !salvage {
                    assert!(
                        output.contains("You can not backup while towing!"),
                        "{output}"
                    );
                    assert_eq!(native.world().btech, world.btech);
                    assert!(lua.drain_outbox().is_empty());
                }
            }
        }
    }
}

#[tokio::test]
async fn vertical_commands_share_loaded_budget_and_atomic_native_lua_behavior() {
    for target in [
        include_str!("../game/mechs/Savannah_Master.toml"),
        CHASSIS[1],
    ] {
        let (_dir, config, mut world, _, ids) = fixture(&[CHASSIS[2], target]).await;
        let [a, b] = ids[..] else { unreachable!() };
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(a);
        assign_battle_pilot(&mut world, a, ObjectId(1)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        start_battle_unit(&mut world, a, ObjectId(1), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
        world
            .btech
            .rewrite_unit_record(a, |record| {
                let unit = record;
                unit["vtol_flight"]["phase"] = serde_json::json!({"kind":"airborne"});
                unit["vtol_flight"]["altitude"] = 20.0.into();
                unit["motion"]["heading"] = 90.0.into();
                unit["motion"]["desired_heading"] = 90.0.into();
            })
            .unwrap();
        set_battle_tow(&mut world, a, Some(b)).unwrap();
        let maximum = battle_throttle_maximum(&world, a, true).unwrap();
        if maximum > 0.0 {
            set_battle_speed(&mut world, a, ObjectId(1), maximum * 0.6).unwrap();
        }
        let vertical_limit = maximum * 0.8;
        for sign in [-1.0, 1.0] {
            let requested = vertical_limit * 0.99 * sign;
            let native = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
            )
            .unwrap();
            let lua = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
            )
            .unwrap();
            let output = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("vertical {requested}"),
            );
            assert!(output.contains("Vertical speed set to"), "{output}");
            lua.eval_callback::<()>(&format!("btech.unit.vertical({},1,{requested})", a.0))
                .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            assert_eq!(
                native.world().btech.vehicles()[&a]
                    .vtol_flight()
                    .unwrap()
                    .vertical_speed,
                requested
            );
            let before = native.world().btech.clone();
            let rejected = (vertical_limit + 1.0) * sign;
            let output = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("vertical {rejected}"),
            );
            assert!(output.contains("remaining velocity budget"), "{output}");
            assert_eq!(native.world().btech, before);
            lua.drain_outbox();
            assert!(
                lua.eval_callback::<()>(&format!("btech.unit.vertical({},1,{rejected})", a.0))
                    .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.vertical({},1,0); error('abort climb')",
                    a.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            let mut moved = native.world().clone();
            persistence::save(&config.database(), &moved).await.unwrap();
            let mut replay = persistence::load(&config.database()).await.unwrap();
            advance_battle_motion(&mut moved, BattleMovementRules::STANDARD).unwrap();
            advance_battle_motion(&mut replay, BattleMovementRules::STANDARD).unwrap();
            assert_eq!(moved.btech, replay.btech);
            moved.validate(&config).unwrap();
        }
    }
}

#[tokio::test]
async fn conventional_carrier_status_reports_loaded_and_mechanical_limits_separately() {
    let (_dir, config, mut world, _, ids) = fixture(&[CHASSIS[0], CHASSIS[1]]).await;
    let [carrier, target] = ids[..] else {
        unreachable!()
    };
    set_battle_tow(&mut world, carrier, Some(target)).unwrap();
    let before = world.btech.clone();
    let maximum = battle_throttle_maximum(&world, carrier, true).unwrap();
    let report = battle_unit_status(&world, carrier, "info").unwrap();
    assert!(
        report.contains(&format!("MaxSpeed: {:3}", maximum as i32)),
        "{report}"
    );
    assert_eq!(
        world.btech.constructed_units()[&carrier]
            .mobility()
            .maximum_speed,
        118.25
    );
    assert!(report.contains("Towing Demolisher"), "{report}");
    assert_eq!(world.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let replay = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_unit_status(&replay, carrier, "info").unwrap(),
        report
    );
}

/// Prepare one visible carrier for admission tests without attaching its target.
fn prepare_pickup(world: &mut World, carrier: ObjectId, target: ObjectId) {
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(carrier);
    assign_battle_pilot(world, carrier, ObjectId(1)).unwrap();
    support::seed_object_dice(world, ObjectId(1), support::FIXTURE_DICE_SEED);
    start_battle_unit(world, carrier, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(world, 0);
    }
    set_battle_towable(world, target, true).unwrap();
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    let class = if world.btech.vehicles().contains_key(&carrier) {
        "vehicles"
    } else {
        "constructed"
    };
    saved[class][carrier.0.to_string()]["contacts"][target.0.to_string()] =
        serde_json::json!({"identified": true});
    if class == "vehicles" {
        let old = saved[class][carrier.0.to_string()]["definition"]["attributes"]["specials"]
            .as_str()
            .unwrap_or("");
        let flags = format!("{old} SalvageTech");
        saved[class][carrier.0.to_string()]["definition"]["attributes"]["specials"] = flags.into();
    }
    world.btech = serde_json::from_value(saved).unwrap();
}

#[tokio::test]
async fn pickup_admission_and_towable_permission_are_shared_and_persisted() {
    for source in [
        include_str!("fixtures/btech/mechs/AS7-D.toml"),
        CHASSIS[1],
        CHASSIS[2],
    ] {
        for victim in CHASSIS {
            let (_dir, config, mut world, _, ids) = fixture(&[source, victim]).await;
            let [carrier, target] = ids[..] else {
                unreachable!()
            };
            assert!(!battle_unit_towable(&world, target).unwrap());
            prepare_pickup(&mut world, carrier, target);
            let before = world.btech.clone();
            battle_pickup_admission(&world, carrier, ObjectId(1), target).unwrap();
            assert_eq!(world.btech, before);
            assert!(world.btech.tows().is_empty());
            assert!(battle_pickup_admission(&world, carrier, ObjectId(2), target).is_err());
            set_battle_towable(&mut world, target, false).unwrap();
            assert!(battle_pickup_admission(&world, carrier, ObjectId(1), target).is_err());
            world
                .objects
                .get_mut(&target)
                .unwrap()
                .flags
                .insert(Flag::InCharacter);
            battle_pickup_admission(&world, carrier, ObjectId(1), target).unwrap();
            world
                .objects
                .get_mut(&target)
                .unwrap()
                .flags
                .remove(Flag::InCharacter);
            set_battle_towable(&mut world, target, true).unwrap();
            persistence::save(&config.database(), &world).await.unwrap();
            let restored = persistence::load(&config.database()).await.unwrap();
            assert!(battle_unit_towable(&restored, target).unwrap());
            battle_pickup_admission(&restored, carrier, ObjectId(1), target).unwrap();
            assert_eq!(restored.btech, world.btech);
        }
    }
}

#[tokio::test]
async fn pickup_admission_rejects_motion_hidden_targets_enemies_and_overlap_without_mutation() {
    for source in [
        include_str!("fixtures/btech/mechs/AS7-D.toml"),
        CHASSIS[1],
        CHASSIS[2],
    ] {
        let (_dir, _, mut world, _, ids) = fixture(&[source, CHASSIS[0]]).await;
        let [carrier, target] = ids[..] else {
            unreachable!()
        };
        prepare_pickup(&mut world, carrier, target);
        let class = if world.btech.vehicles().contains_key(&carrier) {
            "vehicles"
        } else {
            "constructed"
        };
        for (speed, allowed) in [(-1.01, false), (-1.0, true), (1.0, true), (1.01, false)] {
            let mut saved = serde_json::to_value(&world.btech).unwrap();
            saved[class][carrier.0.to_string()]["motion"]["speed"] = speed.into();
            let mut case = world.clone();
            case.btech = serde_json::from_value(saved).unwrap();
            let before = case.btech.clone();
            assert_eq!(
                battle_pickup_admission(&case, carrier, ObjectId(1), target).is_ok(),
                allowed
            );
            assert_eq!(case.btech, before);
        }
        let mut hidden = world.clone();
        set_battle_unit_signature(
            &mut hidden,
            target,
            BattleUnitSignature {
                hidden: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(battle_pickup_admission(&hidden, carrier, ObjectId(1), target).is_err());
        set_battle_unit_signature(
            &mut hidden,
            target,
            BattleUnitSignature {
                team: 1,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(battle_pickup_admission(&hidden, carrier, ObjectId(1), target).is_err());
        set_battle_tow(&mut world, carrier, Some(target)).unwrap();
        let before = world.btech.clone();
        assert!(battle_pickup_admission(&world, carrier, ObjectId(1), target).is_err());
        assert_eq!(world.btech, before);
    }
}

#[tokio::test]
async fn pickup_equipment_requires_both_arms_and_one_working_shoulder_hand_pair() {
    let (_dir, _, mut world, _, ids) =
        fixture(&[include_str!("fixtures/btech/mechs/AS7-D.toml"), CHASSIS[1]]).await;
    let [carrier, target] = ids[..] else {
        unreachable!()
    };
    prepare_pickup(&mut world, carrier, target);
    destroy_battle_critical(
        &mut world,
        carrier,
        CriticalLocation {
            section: BattleSection::LeftArm,
            slot: 3,
        },
    )
    .unwrap();
    battle_pickup_admission(&world, carrier, ObjectId(1), target).unwrap();
    let mut broken_arm = serde_json::to_value(&world.btech).unwrap();
    broken_arm["constructed"][carrier.0.to_string()]["sections"]["LeftArm"]["internal"] = 0.into();
    let mut broken = world.clone();
    broken.btech = serde_json::from_value(broken_arm).unwrap();
    assert!(battle_pickup_admission(&broken, carrier, ObjectId(1), target).is_err());

    destroy_battle_critical(
        &mut world,
        carrier,
        CriticalLocation {
            section: BattleSection::RightArm,
            slot: 0,
        },
    )
    .unwrap();
    assert!(battle_pickup_admission(&world, carrier, ObjectId(1), target).is_err());
    let (_dir, _, mut vehicle_world, _, ids) = fixture(&[CHASSIS[1], CHASSIS[0]]).await;
    let [carrier, target] = ids[..] else {
        unreachable!()
    };
    prepare_pickup(&mut vehicle_world, carrier, target);
    vehicle_world
        .btech
        .rewrite_unit_record(carrier, |record| {
            record["definition"]["attributes"]["specials"] = "ICEEngine_Tech".into();
        })
        .unwrap();
    assert!(battle_pickup_admission(&vehicle_world, carrier, ObjectId(1), target).is_err());
    let (_dir, _, mut handless, _, ids) = fixture(&[CHASSIS[0], CHASSIS[1]]).await;
    let [carrier, target] = ids[..] else {
        unreachable!()
    };
    prepare_pickup(&mut handless, carrier, target);
    assert!(
        battle_pickup_admission(&handless, carrier, ObjectId(1), target)
            .unwrap_err()
            .to_string()
            .contains("functioning arm")
    );
}

#[tokio::test]
async fn pickup_vtol_height_and_vertical_speed_boundaries_are_read_only() {
    let (_dir, _, mut world, _, ids) = fixture(&[CHASSIS[2], CHASSIS[0]]).await;
    let [carrier, target] = ids[..] else {
        unreachable!()
    };
    prepare_pickup(&mut world, carrier, target);
    for (height, target_height, vertical, allowed) in [
        (3.0, 0, 1.0, true),
        (4.0, 0, 0.0, false),
        (0.0, 2, -1.0, true),
        (0.0, 3, 0.0, false),
        (1.0, 0, 1.01, false),
        (1.0, 0, -1.01, false),
    ] {
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        encoded["vehicles"][carrier.0.to_string()]["vtol_flight"]["phase"] =
            serde_json::json!({"kind":"airborne"});
        encoded["vehicles"][carrier.0.to_string()]["vtol_flight"]["altitude"] = height.into();
        encoded["vehicles"][carrier.0.to_string()]["vtol_flight"]["vertical_speed"] =
            vertical.into();
        encoded["constructed"][target.0.to_string()]["ground_elevation"] = target_height.into();
        let mut case = world.clone();
        case.btech = serde_json::from_value(encoded).unwrap();
        let before = case.btech.clone();
        let result = battle_pickup_admission(&case, carrier, ObjectId(1), target);
        assert_eq!(
            result.is_ok(),
            allowed,
            "height {height}, target {target_height}, vertical {vertical}: {result:?}"
        );
        assert_eq!(case.btech, before);
    }
}

#[tokio::test]
async fn pickup_preparation_shares_shutdown_for_all_chassis_without_target_pilot_authority() {
    for carrier_source in [
        include_str!("fixtures/btech/mechs/AS7-D.toml"),
        CHASSIS[1],
        CHASSIS[2],
    ] {
        for target_source in CHASSIS {
            for power in ["off", "starting", "running"] {
                let (_dir, config, mut world, _, ids) =
                    fixture(&[carrier_source, target_source]).await;
                let [carrier, target] = ids[..] else {
                    unreachable!()
                };
                prepare_pickup(&mut world, carrier, target);
                // An uncrewed target has no operator who could authorize ordinary shutdown.
                let mut saved = serde_json::to_value(&world.btech).unwrap();
                let collection = if world.btech.vehicles().contains_key(&target) {
                    "vehicles"
                } else {
                    "constructed"
                };
                let record = &mut saved[collection][target.0.to_string()];
                record["power"] = if power == "starting" {
                    serde_json::json!({"state": "starting", "remaining": 3})
                } else {
                    serde_json::json!({"state": power})
                };
                if power == "running" {
                    record["motion"]["speed"] = 21.5.into();
                    record["motion"]["desired_speed"] = 21.5.into();
                }
                world.btech = serde_json::from_value(saved).unwrap();
                let before = state(&world, target);
                let notices = prepare_battle_pickup(
                    &mut world,
                    carrier,
                    ObjectId(1),
                    target,
                    BattleMovementRules::STANDARD.fall,
                )
                .unwrap();
                let after = state(&world, target);
                assert_eq!(after["power"]["state"], "off");
                assert_eq!(after["motion"]["speed"], 0.0);
                assert_eq!(after["motion"]["desired_speed"], 0.0);
                assert_eq!(after["sections"], before["sections"]);
                assert_eq!(after["dice"], before["dice"]);
                if power != "off" {
                    assert!(after["pilot"].is_null());
                }
                assert_eq!(notices.is_empty(), power == "off");
                if collection == "constructed" {
                    assert_eq!(
                        world.btech.constructed_units()[&target].posture(),
                        BattlePosture::Prone
                    );
                    assert!(
                        world.btech.constructed_units()[&target]
                            .free_fall()
                            .is_none()
                    );
                }
                assert!(world.btech.tows().is_empty());
                // Preparation produces state accepted by the shared relationship primitive.
                set_battle_tow(&mut world, carrier, Some(target)).unwrap();
                world.validate(&config).unwrap();
                persistence::save(&config.database(), &world).await.unwrap();
                let loaded = persistence::load(&config.database()).await.unwrap();
                assert_eq!(loaded.btech, world.btech);
            }
        }
    }
}

#[tokio::test]
async fn rejected_pickup_preparation_preserves_motion_power_and_relationships() {
    for source in [
        include_str!("fixtures/btech/mechs/AS7-D.toml"),
        CHASSIS[1],
        CHASSIS[2],
    ] {
        let (_dir, _, mut world, _, ids) = fixture(&[source, CHASSIS[0]]).await;
        let [carrier, target] = ids[..] else {
            unreachable!()
        };
        prepare_pickup(&mut world, carrier, target);
        set_battle_towable(&mut world, target, false).unwrap();
        let before = world.btech.clone();
        assert!(
            prepare_battle_pickup(
                &mut world,
                carrier,
                ObjectId(1),
                target,
                BattleMovementRules::STANDARD.fall
            )
            .is_err()
        );
        assert_eq!(world.btech, before);
    }
}

#[tokio::test]
async fn vehicle_descent_shares_timing_and_persistence_across_ground_and_rotorcraft() {
    for source in [CHASSIS[1], CHASSIS[2]] {
        let (_dir, config, mut world, _, ids) = fixture(&[source]).await;
        let id = ids[0];
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        let record = &mut saved["vehicles"][id.0.to_string()];
        if world.btech.vehicles()[&id].vtol_flight().is_some() {
            record["vtol_flight"]["altitude"] = 5.0.into();
        } else {
            record["ground_elevation"] = 5.into();
        }
        world.btech = serde_json::from_value(saved).unwrap();
        begin_battle_vehicle_descent(&mut world, id).unwrap();
        let before = world.btech.clone();
        assert!(begin_battle_vehicle_descent(&mut world, id).is_err());
        assert_eq!(world.btech, before);
        for expected in [
            BattleVehicleDescentEvent::Waiting,
            BattleVehicleDescentEvent::Waiting,
            BattleVehicleDescentEvent::Descending,
        ] {
            assert_eq!(
                advance_battle_vehicle_descent(&mut world, id, BattleMovementRules::STANDARD.fall)
                    .unwrap(),
                expected
            );
        }
        assert_eq!(
            world.btech.vehicles()[&id].free_fall().unwrap().elevation(),
            3
        );
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, world.btech);
        for _ in 0..3 {
            let event =
                advance_battle_vehicle_descent(&mut world, id, BattleMovementRules::STANDARD.fall)
                    .unwrap();
            assert_eq!(
                event,
                advance_battle_vehicle_descent(&mut loaded, id, BattleMovementRules::STANDARD.fall)
                    .unwrap()
            );
            if let BattleVehicleDescentEvent::Impact { levels, .. } = event {
                assert_eq!(levels, 6);
            }
        }
        assert!(world.btech.vehicles()[&id].free_fall().is_none());
        assert_eq!(world.btech, loaded.btech);
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn ground_descent_rejects_conflicting_state_and_advances_on_the_shared_heartbeat() {
    let (_dir, config, mut world, _, ids) = fixture(&[CHASSIS[1]]).await;
    let id = ids[0];
    let before = world.btech.clone();
    assert!(begin_battle_vehicle_descent(&mut world, id).is_err());
    assert_eq!(world.btech, before);
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["ground_elevation"] = 5.into();
        })
        .unwrap();
    begin_battle_vehicle_descent(&mut world, id).unwrap();
    let saved = serde_json::to_value(&world.btech).unwrap();
    for key in ["ground_elevation", "motion", "position"] {
        let mut invalid = saved.clone();
        invalid["vehicles"][id.0.to_string()][key] = if key == "ground_elevation" {
            3.into()
        } else {
            serde_json::Value::Null
        };
        assert!(serde_json::from_value::<BtechState>(invalid).is_err());
    }
    for _ in 0..3 {
        advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
    }
    assert_eq!(
        world.btech.vehicles()[&id].free_fall().unwrap().elevation(),
        3
    );
    world.validate(&config).unwrap();
}

/// Raise carried material without advancing the carrier, to exercise release heights precisely.
fn tow_height(world: &mut World, target: ObjectId, height: i16) {
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    let collection = if world.btech.vehicles().contains_key(&target) {
        "vehicles"
    } else {
        "constructed"
    };
    let record = &mut saved[collection][target.0.to_string()];
    if world
        .btech
        .vehicles()
        .get(&target)
        .is_some_and(|unit| unit.vtol_flight().is_some())
    {
        record["vtol_flight"]["altitude"] = f64::from(height).into();
    } else {
        record["ground_elevation"] = height.into();
    }
    world.btech = serde_json::from_value(saved).unwrap();
}

/// The two established tick adapters share descent clocks and return unit-addressed feedback.
fn tick_descent(world: &mut World, target: ObjectId) -> Vec<BattleNotice> {
    if world.btech.vehicles().contains_key(&target) {
        advance_battle_motion(world, BattleMovementRules::STANDARD).unwrap()
    } else {
        advance_battle_jumps(world, BattleMovementRules::STANDARD).unwrap()
    }
}

#[tokio::test]
async fn tow_release_settles_or_falls_for_every_chassis_pair_and_replays() {
    for carrier in CHASSIS {
        for target_source in CHASSIS {
            for height in [2, 3, 12] {
                let (_dir, config, mut world, _, ids) = fixture(&[carrier, target_source]).await;
                let [carrier, target] = ids[..] else {
                    unreachable!()
                };
                set_battle_tow(&mut world, carrier, Some(target)).unwrap();
                tow_height(&mut world, target, height);
                world
                    .objects
                    .get_mut(&target)
                    .unwrap()
                    .flags
                    .insert(Flag::Going);
                let before = world.btech.clone();
                assert!(release_battle_tow(&mut world, carrier).is_err());
                assert_eq!(world.btech, before);
                world
                    .objects
                    .get_mut(&target)
                    .unwrap()
                    .flags
                    .remove(Flag::Going);
                let notices = release_battle_tow(&mut world, carrier).unwrap();
                assert!(
                    notices
                        .iter()
                        .any(|notice| notice.unit == target && notice.text.contains("released"))
                );
                assert!(world.btech.tows().is_empty());
                world.validate(&config).unwrap();
                persistence::save(&config.database(), &world).await.unwrap();
                let mut loaded = persistence::load(&config.database()).await.unwrap();
                let mut impact = false;
                for _ in 0..12 {
                    let notices = tick_descent(&mut world, target);
                    assert_eq!(notices, tick_descent(&mut loaded, target));
                    impact |= notices.iter().any(|notice| {
                        notice.unit == target && notice.text == "You hit the ground!"
                    });
                    assert_eq!(world.btech, loaded.btech);
                }
                assert_eq!(impact, height > 2);
                world.validate(&config).unwrap();
                let before = world.btech.clone();
                assert!(release_battle_tow(&mut world, carrier).is_err());
                assert_eq!(world.btech, before);
            }
        }
    }
}

#[tokio::test]
async fn released_mech_wrecks_finish_descent_before_and_after_destruction() {
    for destroy_in_flight in [false, true] {
        let (_dir, config, mut world, _, ids) = fixture(&[CHASSIS[2], CHASSIS[0]]).await;
        let [carrier, target] = ids[..] else {
            unreachable!()
        };
        if !destroy_in_flight {
            apply_damage_phase(
                &mut world,
                target,
                BattleSection::CenterTorso,
                100,
                BattleDamagePhase::Internal,
            )
            .unwrap();
        }
        set_battle_tow(&mut world, carrier, Some(target)).unwrap();
        tow_height(&mut world, target, 5);
        release_battle_tow(&mut world, carrier).unwrap();
        if destroy_in_flight {
            apply_damage_phase(
                &mut world,
                target,
                BattleSection::CenterTorso,
                100,
                BattleDamagePhase::Internal,
            )
            .unwrap();
        }
        assert!(
            world.btech.constructed_units()[&target]
                .free_fall()
                .is_some()
        );
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        let mut impact = false;
        for _ in 0..6 {
            let notices = tick_descent(&mut world, target);
            assert_eq!(notices, tick_descent(&mut loaded, target));
            impact |= notices
                .iter()
                .any(|notice| notice.text == "You hit the ground!");
        }
        assert!(impact);
        assert!(
            world.btech.constructed_units()[&target]
                .free_fall()
                .is_none()
        );
        assert_eq!(world.btech, loaded.btech);
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn pickup_composes_prior_tow_release_attachment_and_replay_for_all_chassis() {
    for source in [
        include_str!("fixtures/btech/mechs/AS7-D.toml"),
        CHASSIS[1],
        CHASSIS[2],
    ] {
        for target_source in CHASSIS {
            let (_dir, config, mut world, _, ids) =
                fixture(&[source, target_source, CHASSIS[0]]).await;
            let [carrier, target, previous] = ids[..] else {
                unreachable!()
            };
            prepare_pickup(&mut world, carrier, target);
            set_battle_tow(&mut world, target, Some(previous)).unwrap();
            tow_height(&mut world, previous, 5);
            let report = pickup_battle_unit(
                &mut world,
                carrier,
                ObjectId(1),
                target,
                BattleMovementRules::STANDARD.fall,
                true,
            )
            .unwrap();
            assert_eq!(world.btech.tows().len(), 1);
            assert_eq!(world.btech.tows().get(&carrier), Some(&target));
            assert!(
                world.btech.constructed_units()[&previous]
                    .free_fall()
                    .is_some()
            );
            assert!(
                report
                    .notices
                    .iter()
                    .any(|notice| notice.unit == previous && notice.text.contains("released"))
            );
            assert!(report.ice.is_none());
            world.validate(&config).unwrap();
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
        }
    }
}

#[tokio::test]
async fn native_and_lua_pickup_dropoff_share_state_and_rollback() {
    for source in [
        include_str!("fixtures/btech/mechs/AS7-D.toml"),
        CHASSIS[1],
        CHASSIS[2],
    ] {
        for target_source in CHASSIS {
            let (_dir, config, mut world, _, ids) = fixture(&[source, target_source]).await;
            let [carrier, target] = ids[..] else {
                unreachable!()
            };
            prepare_pickup(&mut world, carrier, target);
            for id in [carrier, target] {
                world
                    .objects
                    .get_mut(&id)
                    .unwrap()
                    .flags
                    .insert(Flag::InCharacter);
            }

            let native = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
            )
            .unwrap();
            let lua =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
            let output = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("pickup #{}", target.0),
            );
            assert!(output.contains("tow lines"), "{output}");
            lua.eval_callback::<()>(&format!("btech.unit.pickup({},1,{})", carrier.0, target.0))
                .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            lua.drain_outbox();
            let before = lua.world().btech.clone();
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.dropoff({},1); error('abort release')",
                    carrier.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            let output = support::run_text(&native, &config, ObjectId(1), 1, "dropoff");
            assert!(output.contains("drop the unit"), "{output}");
            lua.eval_callback::<()>(&format!("btech.unit.dropoff({},1)", carrier.0))
                .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            lua.drain_outbox();
            let before = lua.world().btech.clone();
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.pickup({},1,{}); error('abort pickup')",
                    carrier.0, target.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            native.world().validate(&config).unwrap();
        }
    }
}

#[tokio::test]
async fn pickup_through_ice_uses_shared_breakage_for_ground_and_airborne_carriers() {
    for (source, height) in [
        (include_str!("fixtures/btech/mechs/AS7-D.toml"), 0),
        (CHASSIS[1], 0),
        (CHASSIS[2], 1),
        (include_str!("../game/mechs/J_Edgar.toml"), 0),
    ] {
        let (_dir, config, mut world, _, ids) = fixture(&[source, CHASSIS[0]]).await;
        let [carrier, target] = ids[..] else {
            unreachable!()
        };
        let map = world.create(&config, "Ice".into(), Kind::Room);
        create_battle_map(
            &mut world,
            map,
            "ice",
            MapAsset::from_cells(&format!("1 1\n{}1\n", Terrain::Ice.symbol())).unwrap(),
        )
        .unwrap();
        support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
        for id in [carrier, target] {
            place_battle_unit(&mut world, id, map, 0, 0).unwrap();
        }
        prepare_pickup(&mut world, carrier, target);
        tow_height(&mut world, carrier, height);
        tow_height(&mut world, target, -1);
        for player in [ObjectId(1), ObjectId(2)] {
            world.objects.get_mut(&player).unwrap().location = Some(carrier);
            world
                .objects
                .get_mut(&player)
                .unwrap()
                .flags
                .insert(Flag::Connected);
        }
        let before = world.clone();
        let report = pickup_battle_unit(
            &mut world,
            carrier,
            ObjectId(1),
            target,
            BattleMovementRules::STANDARD.fall,
            true,
        )
        .unwrap();
        assert!(report.ice.is_some());
        assert_eq!(
            world.btech.maps()[&map].base_hex(0, 0).unwrap().terrain(),
            Terrain::Water
        );
        assert_eq!(
            battle_unit_elevation(&world, target).unwrap(),
            battle_unit_elevation(&world, carrier).unwrap()
        );
        world.validate(&config).unwrap();
        if before.btech.constructed_units().contains_key(&carrier) {
            let scripts = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(before.clone())),
            )
            .unwrap();
            let call = format!("btech.unit.pickup({},1,{})", carrier.0, target.0);
            assert!(
                scripts
                    .eval_callback::<()>(&format!("{call}; error('abort ice pickup')"))
                    .is_err()
            );
            assert_eq!(scripts.world().btech, before.btech);
            assert!(scripts.drain_outbox().is_empty());
            scripts.eval_callback::<()>(&call).unwrap();
            let output = scripts.drain_outbox();
            let pilot: Vec<_> = output
                .iter()
                .filter(|(who, _)| *who == ObjectId(1))
                .map(|(_, message)| message.source())
                .collect();
            let index = pilot
                .iter()
                .position(|message| *message == "You make a piloting skill roll!")
                .unwrap();
            assert!(index > 0);
            assert!(pilot[index + 1].starts_with("Modified Pilot Skill: BTH "));
            assert!(!output.iter().any(|(who, message)| *who == ObjectId(2)
                && (message.source().starts_with("Modified Pilot Skill:")
                    || message.source() == "You make a piloting skill roll!")));
            let native =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(before))).unwrap();
            battle_tow_action(&native, &config, carrier, ObjectId(1), Some(target)).unwrap();
            assert_eq!(native.world().btech, scripts.world().btech);
            let replay = native.drain_outbox();
            assert_eq!(
                replay
                    .iter()
                    .map(|(who, message)| (*who, message.source()))
                    .collect::<Vec<_>>(),
                output
                    .iter()
                    .map(|(who, message)| (*who, message.source()))
                    .collect::<Vec<_>>()
            );
        }
    }
}

#[tokio::test]
async fn pickup_ice_failure_restores_the_previous_tow_and_all_material_state() {
    let (_dir, config, mut world, _, ids) = fixture(&[
        include_str!("fixtures/btech/mechs/AS7-D.toml"),
        CHASSIS[0],
        CHASSIS[0],
    ])
    .await;
    let [carrier, target, previous] = ids[..] else {
        unreachable!()
    };
    let map = world.create(&config, "Ice".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "ice",
        MapAsset::from_cells(&format!("1 1\n{}1\n", Terrain::Ice.symbol())).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    for id in ids {
        place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    }
    prepare_pickup(&mut world, carrier, target);
    tow_height(&mut world, target, -1);
    set_battle_tow(&mut world, target, Some(previous)).unwrap();
    world
        .objects
        .get_mut(&previous)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let before = world.btech.clone();
    let error = pickup_battle_unit(
        &mut world,
        carrier,
        ObjectId(1),
        target,
        BattleMovementRules::STANDARD.fall,
        true,
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Fall requires a live tactical unit"),
        "{error:#}"
    );
    assert_eq!(world.btech, before);
    assert_eq!(world.btech.tows().get(&target), Some(&previous));
}

#[tokio::test]
async fn native_and_lua_scenario_permission_share_inspection_persistence_and_rollback() {
    for target_source in CHASSIS {
        let (_dir, config, mut world, _, ids) = fixture(&[
            include_str!("fixtures/btech/mechs/AS7-D.toml"),
            target_source,
        ])
        .await;
        let [carrier, target] = ids[..] else {
            unreachable!()
        };
        prepare_pickup(&mut world, carrier, target);
        set_battle_towable(&mut world, target, false).unwrap();
        let mortal = world.create(&config, "Scenario visitor".into(), Kind::Player);
        let player = world.objects.get_mut(&mortal).unwrap();
        player.location = Some(ObjectId(config.start()));
        player.home = Some(ObjectId(config.home()));
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let lua = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let command = format!("@btech unit-towable #{}=on", target.0);
        let before = native.world().btech.clone();
        let output = support::run_text(&native, &config, mortal, 2, &command);
        assert!(!output.contains("towing enabled"), "{output}");
        assert_eq!(native.world().btech, before);
        let output = support::run_text(&native, &config, ObjectId(1), 1, &command);
        assert!(output.contains("towing enabled"), "{output}");
        assert!(
            lua.eval_callback::<bool>(&format!("return btech.unit.towable({}, true)", target.0))
                .unwrap()
        );
        assert_eq!(native.world().btech, lua.world().btech);
        assert!(
            lua.eval_callback::<bool>(&format!("return btech.unit.state({}).towable", target.0))
                .unwrap()
        );
        lua.eval_callback::<()>(&format!("btech.unit.pickup({},1,{})", carrier.0, target.0))
            .unwrap();
        assert_eq!(
            lua.eval_callback::<Option<i64>>(&format!(
                "return btech.unit.state({}).towing",
                carrier.0
            ))
            .unwrap(),
            Some(target.0)
        );
        assert_eq!(
            lua.eval_callback::<Option<i64>>(&format!(
                "return btech.unit.state({}).towed_by",
                target.0
            ))
            .unwrap(),
            Some(carrier.0)
        );
        lua.drain_outbox();
        let before = lua.world().btech.clone();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.towable({},false); error('abort permission')",
                target.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, before);
        assert!(lua.drain_outbox().is_empty());
        assert!(
            !lua.eval_callback::<bool>(&format!("return btech.unit.towable({},false)", target.0))
                .unwrap()
        );
        assert_eq!(lua.world().btech.tows().get(&carrier), Some(&target));
        let output = support::run_text(
            &lua,
            &config,
            ObjectId(1),
            1,
            &format!("@btech inspect #{}", target.0),
        );
        assert!(
            output.contains("Out-of-character towable: false"),
            "{output}"
        );
        assert!(
            output.contains(&format!("towed by: #{}", carrier.0)),
            "{output}"
        );
        let before = lua.world().btech.clone();
        let output = support::run_text(
            &lua,
            &config,
            ObjectId(1),
            1,
            &format!("@btech unit-towable #{}=maybe", target.0),
        );
        assert!(output.contains("Usage:"), "{output}");
        assert_eq!(lua.world().btech, before);
        let snapshot = lua.world().clone();
        persistence::save(&config.database(), &snapshot)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            before
        );
    }
}

#[tokio::test]
async fn lua_vehicle_inspection_exposes_shared_descent_without_advancing_it() {
    for source in [CHASSIS[1], CHASSIS[2]] {
        let (_dir, config, mut world, _, ids) = fixture(&[source]).await;
        let id = ids[0];
        tow_height(&mut world, id, 5);
        begin_battle_vehicle_descent(&mut world, id).unwrap();
        let before = world.btech.clone();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        assert_eq!(
            scripts
                .eval_callback::<i32>(&format!(
                    "return btech.unit.state({}).free_fall.elevation",
                    id.0
                ))
                .unwrap(),
            5
        );
        assert_eq!(scripts.world().btech, before);
    }
}

/// Continuous transport height is mirrored once for every chassis pairing and feeds shared geometry.
#[tokio::test]
async fn fractional_tow_height_and_range_survive_persistence_for_all_pairs() {
    for carrier_source in CHASSIS {
        for target_source in CHASSIS {
            let (_dir, config, mut world, _, ids) =
                fixture(&[carrier_source, target_source, CHASSIS[0]]).await;
            let [carrier, target, observer] = ids[..] else {
                unreachable!()
            };
            set_battle_tow(&mut world, carrier, Some(target)).unwrap();
            let mut saved = serde_json::to_value(&world.btech).unwrap();
            let class = if world.btech.vehicles().contains_key(&carrier) {
                "vehicles"
            } else {
                "constructed"
            };
            let record = &mut saved[class][carrier.0.to_string()];
            if record["vtol_flight"].is_object() {
                record["vtol_flight"]["altitude"] = 3.75.into();
            } else {
                record["ground_elevation"] = 3.75.into();
            }
            world.btech = serde_json::from_value(saved).unwrap();
            advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
            assert_eq!(battle_unit_altitude(&world, carrier).unwrap(), Some(3.75));
            assert_eq!(battle_unit_altitude(&world, target).unwrap(), Some(3.75));
            assert_eq!(battle_unit_elevation(&world, target).unwrap(), Some(3));
            assert_eq!(
                battle_unit_range(&world, carrier, target).unwrap().spatial,
                0.0
            );
            assert_eq!(
                battle_unit_range(&world, observer, target).unwrap().spatial,
                0.75
            );
            world.validate(&config).unwrap();
            persistence::save(&config.database(), &world).await.unwrap();
            let mut replay = persistence::load(&config.database()).await.unwrap();
            assert_eq!(world.btech, replay.btech);
            for _ in 0..3 {
                assert_eq!(
                    advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap(),
                    advance_battle_motion(&mut replay, BattleMovementRules::STANDARD).unwrap()
                );
                assert_eq!(world.btech, replay.btech);
                assert_eq!(battle_unit_altitude(&world, target).unwrap(), Some(3.75));
            }
            let scripts =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
            assert_eq!(
                scripts
                    .eval_callback::<f64>(&format!(
                        "return btech.unit.state({}).altitude",
                        target.0
                    ))
                    .unwrap(),
                3.75
            );
        }
    }
}

/// Release uses integer terrain thresholds but retains fractional height until its first fall event.
#[tokio::test]
async fn fractional_release_threshold_and_descent_clock_are_shared_and_restartable() {
    for target_source in CHASSIS {
        for height in [2.99, 3.75] {
            let (_dir, config, mut world, _, ids) = fixture(&[CHASSIS[2], target_source]).await;
            let [carrier, target] = ids[..] else {
                unreachable!()
            };
            set_battle_tow(&mut world, carrier, Some(target)).unwrap();
            world
                .btech
                .rewrite_unit_record(carrier, |record| {
                    record["vtol_flight"]["altitude"] = height.into();
                })
                .unwrap();
            advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
            release_battle_tow(&mut world, carrier).unwrap();
            assert_eq!(
                battle_unit_altitude(&world, target).unwrap(),
                Some(if height < 3.0 { 0.0 } else { height })
            );
            world.validate(&config).unwrap();
            persistence::save(&config.database(), &world).await.unwrap();
            let mut replay = persistence::load(&config.database()).await.unwrap();
            for tick in 1..=6 {
                assert_eq!(
                    tick_descent(&mut world, target),
                    tick_descent(&mut replay, target)
                );
                assert_eq!(world.btech, replay.btech);
                if height >= 3.0 && tick < 3 {
                    assert_eq!(battle_unit_altitude(&world, target).unwrap(), Some(height));
                }
                if height >= 3.0 && tick == 3 {
                    assert_eq!(battle_unit_altitude(&world, target).unwrap(), Some(1.0));
                }
            }
            assert_eq!(battle_unit_altitude(&world, target).unwrap(), Some(0.0));
            world.validate(&config).unwrap();
        }
    }
}

/// Floating-point storage rejects values that cannot yield a valid terrain-rule integer height.
#[tokio::test]
async fn retained_and_falling_altitudes_reject_nonfinite_or_out_of_range_values() {
    for altitude in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::from(i32::MAX) + 1.0,
        f64::from(i32::MIN) - 1.0,
    ] {
        assert!(BattleFreeFall::at_altitude(altitude).is_err());
    }
    for source in [CHASSIS[0], CHASSIS[1]] {
        let (_dir, config, world, _, ids) = fixture(&[source]).await;
        let id = ids[0];
        for altitude in [1e30, -1e30] {
            let mut saved = serde_json::to_value(&world.btech).unwrap();
            let class = if world.btech.vehicles().contains_key(&id) {
                "vehicles"
            } else {
                "constructed"
            };
            saved[class][id.0.to_string()]["ground_elevation"] = altitude.into();
            if let Ok(btech) = serde_json::from_value(saved) {
                let mut candidate = world.clone();
                candidate.btech = btech;
                assert!(candidate.validate(&config).is_err());
            }
        }
    }
}

/// Real VTOL climb steps retain the same fractional height on every carried chassis.
#[tokio::test]
async fn vtol_vertical_towing_preserves_continuous_height_each_tick() {
    for target_source in CHASSIS {
        let (_dir, config, mut world, _, ids) = fixture(&[CHASSIS[2], target_source]).await;
        let [carrier, target] = ids[..] else {
            unreachable!()
        };
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(carrier);
        assign_battle_pilot(&mut world, carrier, ObjectId(1)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        start_battle_unit(&mut world, carrier, ObjectId(1), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
        world
            .btech
            .rewrite_unit_record(carrier, |record| {
                let flight = &mut record["vtol_flight"];
                flight["phase"] = serde_json::json!({"kind":"airborne"});
                flight["altitude"] = 5.25.into();
                flight["vertical_speed"] = 1.0.into();
            })
            .unwrap();
        set_battle_tow(&mut world, carrier, Some(target)).unwrap();
        let mut previous = 5.25;
        for _ in 0..5 {
            advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
            let height = battle_unit_altitude(&world, carrier).unwrap().unwrap();
            assert!(height > previous && height.fract() > 0.0);
            assert_eq!(battle_unit_altitude(&world, target).unwrap(), Some(height));
            assert_eq!(
                battle_unit_range(&world, carrier, target).unwrap().spatial,
                0.0
            );
            previous = height;
        }
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        assert_eq!(world.btech, replay.btech);
        assert_eq!(
            advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap(),
            advance_battle_motion(&mut replay, BattleMovementRules::STANDARD).unwrap()
        );
        assert_eq!(world.btech, replay.btech);
    }
}

/// A carrier loses lift on shutdown without silently releasing its external load.
#[tokio::test]
async fn airborne_carrier_shutdown_retains_tow_and_saved_descent() {
    for target in CHASSIS {
        let (_dir, config, mut world, _, ids) = fixture(&[CHASSIS[2], target]).await;
        let [carrier, load] = ids[..] else {
            unreachable!()
        };
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(carrier);
        assign_battle_pilot(&mut world, carrier, ObjectId(1)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        world
            .btech
            .rewrite_unit_record(carrier, |record| {
                let unit = record;
                unit["power"] = serde_json::to_value(BattlePower::Running).unwrap();
                unit["vtol_flight"]["phase"] = serde_json::json!({"kind":"airborne"});
                unit["vtol_flight"]["altitude"] = 5.5.into();
            })
            .unwrap();
        set_battle_tow(&mut world, carrier, Some(load)).unwrap();
        world.validate(&config).unwrap();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        stop_battle_unit_action(&scripts, &config, carrier, ObjectId(1)).unwrap();
        let mut saved = scripts.world().clone();
        assert_eq!(saved.btech.tows().get(&carrier), Some(&load));
        let flight = saved.btech.vehicles()[&carrier].vtol_flight().unwrap();
        assert_eq!(flight.phase, BattleVtolFlightPhase::Falling);
        assert_eq!(flight.fall, Some(BattleFreeFall::at_altitude(5.5).unwrap()));
        persistence::save(&config.database(), &saved).await.unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        assert_eq!(replay.btech, saved.btech);
        for _ in 0..8 {
            advance_battle_motion(&mut saved, BattleMovementRules::STANDARD).unwrap();
            advance_battle_motion(&mut replay, BattleMovementRules::STANDARD).unwrap();
            assert_eq!(saved.btech, replay.btech);
            saved.validate(&config).unwrap();
        }
    }
}
