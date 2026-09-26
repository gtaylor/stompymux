//! Station scans retain physical sensors and warning identity with independent target and output ownership.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod firing;
use crate::support;

/// A claimed station targeting a visible unit while its parent has no selected target.
async fn fixture(
    template: &str,
    recipient: &str,
) -> (
    tempfile::TempDir,
    Config,
    World,
    ObjectId,
    ObjectId,
    ObjectId,
    ObjectId,
) {
    let (dir, config, mut world, parent, target, _) =
        firing::fixture_with_target(template, Some(BattleWeapon::MediumLaser), recipient).await;
    let station = world.create(&config, "Station".into(), Kind::Thing);
    let gunner = world.create(&config, "Gunner".into(), Kind::Player);
    world.objects.get_mut(&gunner).unwrap().location = Some(station);
    register_gunner_station(&mut world, ObjectId(1), station, parent, 0).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    gunner_station_action(&scripts, station, gunner, true).unwrap();
    select_battle_target(&mut scripts.world_mut(), station, gunner, Some(target)).unwrap();
    select_battle_target(&mut scripts.world_mut(), parent, ObjectId(1), None).unwrap();
    let world = scripts.world().clone();
    (dir, config, world, parent, target, station, gunner)
}

/// Unit and occupant scans share disclosure, warnings and native/Lua results across supported chassis.
#[tokio::test]
async fn station_unit_scans_and_reports_share_targets_and_disclosure() {
    let templates = firing::templates();
    for (index, template) in templates.iter().enumerate() {
        let (_dir, config, world, parent, target, station, gunner) =
            fixture(template, &templates[(index + 1) % templates.len()]).await;
        for option in ["", "A", "I", "W", "AIW"] {
            let make = || Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let lua = make();
            let native = make();
            let pilot = make();
            let query = format!(
                "btech.gunner.scan_selected({},{},'{option}')",
                station.0, gunner.0
            );
            assert!(
                lua.eval_callback::<()>(&format!("{query}; error('abort')"))
                    .is_err()
            );
            assert_eq!(lua.world().btech, world.btech);
            assert!(lua.drain_outbox().is_empty());
            let actual: String = lua
                .eval_callback(&format!("return {query}.report"))
                .unwrap();
            assert_eq!(
                actual,
                scan_battle_unit(&world, parent, ObjectId(1), target, option).unwrap()
            );
            let explicit: String = lua
                .eval_callback(&format!(
                    "return btech.gunner.scan({},{},{},'{option}')",
                    station.0, gunner.0, target.0
                ))
                .unwrap();
            assert_eq!(explicit, actual);
            let occupant: String = lua
                .eval_callback(&format!(
                    "return btech.gunner.scan_hex({},{},0,10,'{option}')",
                    station.0, gunner.0
                ))
                .unwrap();
            assert_eq!(occupant, actual);
            assert_eq!(
                support::run_text(&native, &config, gunner, 1, &format!("scan {option}")),
                support::run_text(
                    &pilot,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("scan #{} {option}", target.0)
                )
            );
            assert_eq!(native.world().btech, world.btech);
            assert_eq!(lua.world().btech, world.btech);
            lua.drain_outbox();
            let report: String = lua
                .eval_callback(&format!(
                    "return btech.gunner.report({},{},{})",
                    station.0, gunner.0, target.0
                ))
                .unwrap();
            assert_eq!(
                report,
                report_battle_unit(&world, parent, ObjectId(1), target).unwrap()
            );
            assert!(lua.drain_outbox().is_empty());
            assert_eq!(
                support::run_text(&native, &config, gunner, 1, "report"),
                support::run_text(
                    &pilot,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("report #{}", target.0)
                )
            );
            assert_eq!(
                support::run_text(&native, &config, gunner, 1, "report 0 10"),
                support::run_text(&pilot, &config, ObjectId(1), 1, "report 0 10")
            );
        }
        for mode in [
            BattleHexTargetMode::UnitAtHex,
            BattleHexTargetMode::Ignite,
            BattleHexTargetMode::Clear,
        ] {
            let mut selected = world.clone();
            select_battle_hex_target(
                &mut selected,
                station,
                gunner,
                BattleHexCoordinate { x: 0, y: 10 },
                mode,
            )
            .unwrap();
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(selected.clone()))).unwrap();
            let actual: String = scripts
                .eval_callback(&format!(
                    "return btech.gunner.scan_selected({},{}).report",
                    station.0, gunner.0
                ))
                .unwrap();
            assert_eq!(
                actual,
                scan_battle_unit(&world, parent, ObjectId(1), target, "").unwrap()
            );
            assert_eq!(scripts.world().btech, selected.btech);
        }
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            report_battle_unit(&restored, station, gunner, target).unwrap(),
            report_battle_unit(&world, station, gunner, target).unwrap()
        );
    }
}

/// Concealed structures and mines share cached scanner perception, but earned XP belongs to the gunner.
#[tokio::test]
async fn station_structure_scans_preserve_perception_experience_and_restart() {
    for template in firing::templates() {
        let (_dir, config, mut world, parent, _target, station, gunner) =
            fixture(&template, &template).await;
        let map = world.btech.units()[&parent].map.unwrap();
        let coordinate = BattleHexCoordinate { x: 0, y: 9 };
        let interior = world.create(&config, "Hangar".into(), Kind::Room);
        create_battle_map(
            &mut world,
            interior,
            "hangar.map",
            BattleMapAsset::parse("1 1\n.0\n").unwrap(),
        )
        .unwrap();
        set_building_state(
            &mut world,
            interior,
            BattleBuildingState {
                integrity: 31,
                maximum_integrity: 50,
                flags: 4,
                regeneration: 1,
            },
        )
        .unwrap();
        set_building_entrance(
            &mut world,
            map,
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
        set_minefield(
            &mut world,
            map,
            0,
            Some(BattleMinefield {
                coordinate,
                kind: BattleMineKind::Command,
                strength: 12,
                extra: 123,
                owner: ObjectId(1),
            }),
        )
        .unwrap();
        world
            .objects
            .get_mut(&parent)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
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
                build: 5,
                reflexes: 5,
                intuition: 5,
                learn: 5,
                charisma: 5,
            },
        )
        .unwrap();
        firing::edit(&mut world, parent, |state| {
            state["scanner_perception"] = (-10).into()
        });
        select_battle_hex_target(
            &mut world,
            station,
            gunner,
            coordinate,
            BattleHexTargetMode::Hex,
        )
        .unwrap();
        let before = world.clone();
        let building = scan_battle_building(&mut world, station, gunner, coordinate, 1000).unwrap();
        assert_eq!(building.text, "The Hangar's CF is 31.");
        assert_eq!(
            world.btech.character_values()[&gunner]["Perception"].experience_balance(),
            1
        );
        assert_eq!(
            world.btech.character_values().get(&ObjectId(1)),
            before.btech.character_values().get(&ObjectId(1))
        );
        let mine = scan_battle_mines(&mut world, station, gunner, coordinate, 1031).unwrap();
        assert!(mine.found);
        persistence::save(&config.database(), &world).await.unwrap();
        // Loading restores the referenced damaged interior's missing repair interval.
        let mut expected = serde_json::to_value(&world.btech).unwrap();
        expected["maps"][interior.0.to_string()]["building_repair"] = 120.into();
        assert_eq!(
            serde_json::to_value(persistence::load(&config.database()).await.unwrap().btech)
                .unwrap(),
            expected
        );
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(before.clone()))).unwrap();
        let query = format!("btech.gunner.scan_selected({},{})", station.0, gunner.0);
        assert!(
            scripts
                .eval_callback::<()>(&format!("{query}; error('abort')"))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before.btech);
        assert!(scripts.drain_outbox().is_empty());
        let found: bool = scripts
            .eval_callback(&format!("return {query}.report.mines.found"))
            .unwrap();
        assert!(found);
        let native = Scripts::new(&config, Rc::new(RefCell::new(before))).unwrap();
        let text = support::run_text(&native, &config, gunner, 1, "scan");
        assert!(
            text.contains("Hangar") && text.contains("bomblets"),
            "{text}"
        );
    }
}

/// A second publication failure rolls back mine dice and both phases of a station hex scan.
#[tokio::test]
async fn station_scan_publication_and_authority_failures_are_atomic() {
    for template in firing::templates() {
        let (dir, _config, mut world, parent, target, station, gunner) =
            fixture(&template, &template).await;
        let map = world.btech.units()[&parent].map.unwrap();
        set_minefield(
            &mut world,
            map,
            0,
            Some(BattleMinefield {
                coordinate: BattleHexCoordinate { x: 0, y: 9 },
                kind: BattleMineKind::Command,
                strength: 12,
                extra: 123,
                owner: ObjectId(1),
            }),
        )
        .unwrap();
        let path = dir.path().join("stompymux.toml");
        let mut settings: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
        settings
            .entry("lua")
            .or_insert(toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .unwrap()
            .insert("output_entry_limit".into(), 2.into());
        std::fs::write(path, toml::to_string(&settings).unwrap()).unwrap();
        let config = Config::load(dir.path()).unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let prefill = format!("btech.gunner.scan_building({},{},0,9)", station.0, gunner.0);
        scripts.eval_callback::<()>(&prefill).unwrap();
        assert_eq!(scripts.drain_outbox().len(), 1);
        let error = scripts
            .eval_callback::<()>(&format!(
                "{prefill}; btech.gunner.scan_terrain({},{},0,9)",
                station.0, gunner.0
            ))
            .unwrap_err();
        assert!(error.to_string().contains("output limit"), "{error}");
        assert_eq!(scripts.world().btech, world.btech);
        assert!(scripts.drain_outbox().is_empty());
        assert!(
            scripts
                .eval_callback::<()>(&format!("btech.gunner.scan({},1,{})", parent.0, target.0))
                .is_err()
        );
        for condition in ["stopped"] {
            let mut rejected = world.clone();
            firing::edit(&mut rejected, parent, |state| {
                state["power"] = serde_json::to_value(BattlePower::Off).unwrap();
            });
            let guarded = Scripts::new(&config, Rc::new(RefCell::new(rejected.clone()))).unwrap();
            for (method, args) in [
                ("scan", target.0.to_string()),
                ("report", target.0.to_string()),
                ("scan_selected", "nil".into()),
                ("scan_hex", "0,10".into()),
                ("scan_terrain", "0,9".into()),
                ("scan_building", "0,9".into()),
            ] {
                assert!(
                    guarded
                        .eval_callback::<()>(&format!(
                            "btech.gunner.{method}({},{},{args})",
                            station.0, gunner.0
                        ))
                        .is_err(),
                    "{condition} {method}"
                );
                assert_eq!(guarded.world().btech, rejected.btech);
                assert!(guarded.drain_outbox().is_empty());
            }
        }
        gunner_station_action(&scripts, station, gunner, false).unwrap();
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.gunner.report({},{},{})",
                    station.0, gunner.0, target.0
                ))
                .is_err()
        );
        let text = support::run_text(&scripts, &config, gunner, 1, "report");
        assert!(text.contains("initialized"), "{text}");
    }
}
