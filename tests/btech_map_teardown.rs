//! Map-role teardown shares unit shutdown while preserving containers and external route markers.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod firing;
use crate::support;

/// Every supported chassis survives map retirement as a stopped, detached unit in the same container.
#[tokio::test]
async fn unregister_map_shuts_down_all_chassis_and_survives_restart() {
    for source in firing::templates() {
        let (_dir, config, world, unit, target, _) =
            firing::fixture_with_target(&source, None, include_str!("../game/mechs/AS7-D")).await;
        let map = world.btech.units()[&unit].map.unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        scripts
            .eval_callback::<()>(&format!("btech.inventory.set(1,{},422,0,7)", map.0))
            .unwrap();
        scripts.drain_outbox();
        let before = scripts.world().clone();
        persistence::save(&config.database(), &before)
            .await
            .unwrap();
        let output = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("@btech/u #{}", map.0),
        );
        assert!(
            output.contains(&format!("Unregistered #{} from BTech.", map.0)),
            "{output}"
        );
        assert!(output.contains("Map Cleared"), "{output}");
        let saved = scripts.world().clone();
        assert!(!saved.btech.maps().contains_key(&map));
        assert!(!saved.btech.registrations().contains_key(&map));
        assert_eq!(saved.objects[&map].kind, Kind::Room);
        for id in [unit, target] {
            assert_eq!(saved.objects[&id].location, Some(map));
            assert_eq!(saved.btech.units()[&id].map, None);
            assert_eq!(saved.btech.registrations()[&id], "MECH");
            let state = serde_json::to_value(&saved.btech).unwrap();
            let store = if saved.btech.vehicles().contains_key(&id) {
                "vehicles"
            } else {
                "constructed"
            };
            assert_eq!(state[store][id.0.to_string()]["power"]["state"], "off");
        }
        assert_eq!(battle_inventory(&saved, map).unwrap()[0].quantity, 7);
        persistence::save(&config.database(), &saved).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, saved.btech);
        assert_eq!(restored.objects[&unit].location, Some(map));
    }
}

/// Authored configuration is removed but another map's explicit entrance and exit objects persist.
#[tokio::test]
async fn external_markers_survive_unregistration_and_reactivation() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let removed = world.create(&config, "Interior".into(), Kind::Thing);
    let exterior = world.create(&config, "Exterior".into(), Kind::Thing);
    for id in [removed, exterior] {
        create_battle_map(
            &mut world,
            id,
            "map",
            BattleMapAsset::parse("2 2\n.0.0\n.0.0\n").unwrap(),
        )
        .unwrap();
    }
    let entrance = BattleBuildingEntrance {
        coordinate: BattleHexCoordinate { x: 0, y: 0 },
        interior: removed,
        data_char: 0,
        data_short: 0,
        data_int: 0,
    };
    set_building_entrance(&mut world, exterior, 0, Some(entrance)).unwrap();
    set_battle_building_exit(&mut world, exterior, 1, Some(removed)).unwrap();
    set_battle_map_link(
        &mut world,
        exterior,
        Some(BattleMapLink {
            parent: removed,
            coordinate: BattleHexCoordinate { x: 1, y: 1 },
            entrances: Default::default(),
        }),
    )
    .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .world_mut()
        .objects
        .get_mut(&removed)
        .unwrap()
        .flags
        .insert(Flag::Going);
    let output = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        &format!("@btech/unregister #{}", removed.0),
    );
    assert!(output.contains("Unregistered"), "{output}");
    let saved = scripts.world().clone();
    assert!(saved.btech.maps()[&exterior].authored_link().is_none());
    assert_eq!(
        saved.btech.maps()[&exterior].building_entrances()[&0],
        entrance
    );
    assert_eq!(
        saved.btech.maps()[&exterior].building_exits()[&1].destination,
        removed
    );
    persistence::save(&config.database(), &saved).await.unwrap();
    *scripts.world_mut() = persistence::load(&config.database()).await.unwrap();
    assert_eq!(scripts.world().btech, saved.btech);
    scripts
        .world_mut()
        .objects
        .get_mut(&removed)
        .unwrap()
        .flags
        .remove(Flag::Going);
    let output = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        &format!("@btech/register #{}=MAP", removed.0),
    );
    assert!(output.contains("Registered"), "{output}");
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        restored.btech.maps()[&exterior].building_entrances()[&0],
        entrance
    );
    assert_eq!(restored.btech.maps()[&removed].name, "Default Map");
}

/// Failed output admission rolls back partial shutdown and leaves the map registered.
#[tokio::test]
async fn teardown_failure_restores_world_and_notifications() {
    let (dir, _config, world, unit, _, _) = firing::fixture_with_target(
        include_str!("../game/mechs/JR7-D"),
        None,
        include_str!("../game/mechs/AS7-D"),
    )
    .await;
    let map = world.btech.units()[&unit].map.unwrap();
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
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    let output = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        &format!("@btech/u #{}", map.0),
    );
    assert!(!output.contains("Unregistered"), "{output}");
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
}

/// A failed durable deletion restores terrain, membership and unit records together.
#[tokio::test]
async fn failed_map_save_keeps_the_previous_database_intact() {
    use sqlx::{Connection, SqliteConnection};
    let (_dir, config, world, unit, _, _) = firing::fixture_with_target(
        include_str!("../game/mechs/JR7-D"),
        None,
        include_str!("../game/mechs/AS7-D"),
    )
    .await;
    let map = world.btech.units()[&unit].map.unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let before = persistence::load(&config.database()).await.unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let output = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        &format!("@btech/u #{}", map.0),
    );
    assert!(output.contains("Unregistered"), "{output}");
    let mut sql = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TRIGGER refuse_map_removal BEFORE DELETE ON btech_maps BEGIN SELECT RAISE(ABORT, 'test save failure'); END").execute(&mut sql).await.unwrap();
    sql.close().await.unwrap();
    let saved = scripts.world().clone();
    assert!(persistence::save(&config.database(), &saved).await.is_err());
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, before.btech);
    assert_eq!(restored.btech.units()[&unit].map, Some(map));
}

/// Firing at a retained entrance whose interior is no longer a map is a harmless building miss.
#[tokio::test]
async fn retired_building_target_does_not_panic_or_reappear() {
    let (_dir, config, mut world, shooter, _, index) = firing::fixture_with_target(
        include_str!("../game/mechs/JR7-D"),
        Some(BattleWeapon::SmallLaser),
        include_str!("../game/mechs/AS7-D"),
    )
    .await;
    let exterior = world.btech.units()[&shooter].map.unwrap();
    let interior = world.create(&config, "Retired building".into(), Kind::Thing);
    create_battle_map(
        &mut world,
        interior,
        "interior",
        BattleMapAsset::parse("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let coordinate = BattleHexCoordinate { x: 0, y: 9 };
    set_building_entrance(
        &mut world,
        exterior,
        0,
        Some(BattleBuildingEntrance {
            coordinate,
            interior,
            data_char: 0,
            data_short: 0,
            data_int: 0,
        }),
    )
    .unwrap();
    select_battle_hex_target(
        &mut world,
        shooter,
        ObjectId(1),
        coordinate,
        BattleHexTargetMode::Building,
    )
    .unwrap();
    let seed = (0..=255)
        .find(|&seed| BattleDice::seeded([seed; 32]).two_d6() == 12)
        .unwrap();
    firing::edit(&mut world, shooter, |unit| {
        unit["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
    });
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let output = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        &format!("@btech/u #{}", interior.0),
    );
    assert!(output.contains("Unregistered"), "{output}");
    let report = resolve_battle_hex_shot(
        &mut scripts.world_mut(),
        shooter,
        ObjectId(1),
        coordinate,
        index,
        BattleShotRules {
            range_damage: false,
            tsm_tow_bonus: true,
            tsm_sprint_bonus: true,
            vehicle_impact: BattleVehicleImpactRules::STANDARD,
            stacking: BattleStackingRules::STANDARD,
            glancing: BattleGlancingMode::Disabled,
            stagger: BattleStaggerMode::Retain,
            aim: BattleAimRules {
                woods_damage: false,
                dig_bonus: 3,
                dig_only_front: false,
                hit_arc_mode: 0,
                fasa_turning: false,
                extended_movement: false,
                extended_ranges: false,
                hotload_half_minimum: false,
                override_weapon_arcs: false,
            },
            hit: BattleHitRules {
                fasa_criticals: false,
                inferno_penalty: false,
                exile_stun_mode: 0,
            },
            hit_arc_mode: 0,
            extended_gunnery: true,
            extended_piloting: true,
            target_toughness: false,
        },
    )
    .unwrap();
    assert!(report.launched && report.hit);
    assert!(report.buildings.is_empty());
    assert!(!scripts.world().btech.maps().contains_key(&interior));
}
