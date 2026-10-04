//! Cockpit reports share admission, weapon formatting and compact export across chassis.
use crate::support;
use stompymux_rs::*;

/// Guardian and Angel status lamps use the committed countermeasure field on both chassis stores.
#[tokio::test]
async fn electronic_status_colors_follow_countering_without_refreshing_the_field() {
    for (store, source) in [
        (
            "constructed",
            include_str!("../game/mechs/JR7-D.toml").replace(
                "slots = [\n    { at = \"1-2\", item = \"JumpJet\" },\n]\n\n[sections.right_torso]",
                "slots = [\n    { at = \"1-2\", item = \"JumpJet\" },\n    { at = \"3-4\", item = \"Ecm\" },\n    { at = \"5-6\", item = \"AngelEcm\" },\n]\n\n[sections.right_torso]",
            ),
        ),
        (
            "vehicles",
            include_str!("../game/mechs/Demolisher.toml").replace(
                "[sections.front_side]\n",
                "[sections.front_side]\nslots = [{ at = 1, item = \"Ecm\" }, { at = 2, item = \"AngelEcm\" }]\n",
            ),
        ),
    ] {
        let (_dir, config, mut world, id, _) = fixture(&source).await;
        start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
        for _ in 0..30 {
            advance_battle_units(&mut world, 0);
        }
        for (mode, countered, expected) in [
            (BattleElectronicMode::Off, false, "[fg=green]Off[reset]"),
            (BattleElectronicMode::Off, true, "[fg=red]Off[reset]"),
            (
                BattleElectronicMode::Ecm,
                false,
                "[fg=green bold]ECM[reset]",
            ),
            (BattleElectronicMode::Ecm, true, "[fg=red bold]ECM[reset]"),
            (
                BattleElectronicMode::Eccm,
                false,
                "[fg=green bold]ECCM[reset]",
            ),
            (
                BattleElectronicMode::Eccm,
                true,
                "[fg=green bold]ECCM[reset]",
            ),
        ] {
            let mut saved = serde_json::to_value(&world.btech).unwrap();
            let electronics = &mut saved[store][id.0.to_string()]["electronics"];
            electronics["guardian"] = serde_json::to_value(mode).unwrap();
            electronics["angel"] = serde_json::to_value(mode).unwrap();
            electronics["field"]["countered"] = countered.into();
            world.btech = serde_json::from_value(saved).unwrap();
            let before = world.btech.clone();
            let report = battle_unit_status(&world, id, "W").unwrap();
            let expected = format!("AdvTech: ECM({expected})  AngelECM({expected})  ");
            assert_eq!(
                report
                    .lines()
                    .find(|line| line.starts_with("AdvTech:"))
                    .unwrap(),
                expected
            );
            let scripts = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
            )
            .unwrap();
            let lua: String = scripts
                .eval_callback(&format!("return btech.unit.status({}, 'W')", id.0))
                .unwrap();
            assert_eq!(lua, report);
            let native = support::run_text(&scripts, &config, ObjectId(1), 1, "status weapons");
            assert!(text::plain(&native).contains(&text::plain(&expected)));
            assert_eq!(scripts.world().btech, before);
            assert_eq!(world.btech, before);
            persistence::save(&config.database(), &world).await.unwrap();
            let restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(battle_unit_status(&restored, id, "W").unwrap(), report);
        }
    }
}

/// TAG precedes booster counters even when all three installations share a cockpit.
#[tokio::test]
async fn advanced_technology_keeps_reference_order_and_counter_colors() {
    let source = support::templates::with_flags(include_str!("../game/mechs/JR7-D.toml"), &["SuperCharger_Tech"])
        .replace(
            "slots = [\n    { at = \"1-2\", item = \"JumpJet\" },\n]\n\n[sections.right_torso]",
            "slots = [\n    { at = \"1-2\", item = \"JumpJet\" },\n    { at = 3, item = \"TAG\" },\n    { at = 4, item = \"Masc\" },\n]\n\n[sections.right_torso]",
        );
    let (_dir, config, mut world, id, _) = fixture(&source).await;
    for (counter, color) in [
        (0, "green"),
        (1, "yellow bold"),
        (3, "yellow bold"),
        (4, "red bold"),
    ] {
        world
            .btech
            .rewrite_unit_record(id, |record| {
                for field in ["masc", "supercharger"] {
                    record[field]["counter"] = counter.into();
                }
            })
            .unwrap();
        let before = world.btech.clone();
        let report = battle_unit_status(&world, id, "W").unwrap();
        let line = report
            .lines()
            .find(|line| line.starts_with("AdvTech:"))
            .unwrap();
        assert_eq!(
            line,
            format!(
                "AdvTech: TAG([fg=green]Rdy[reset])  SCHARGE: [fg={color}]{counter}[reset] (Off)MASC: [fg={color}]{counter}[reset] (Off)"
            )
        );
        let scripts = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let lua: String = scripts
            .eval_callback(&format!("return btech.unit.status({}, 'W')", id.0))
            .unwrap();
        assert_eq!(lua, report);
        assert!(
            text::plain(&support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                "status W"
            ))
            .contains(&text::plain(line))
        );
        assert_eq!(scripts.world().btech, before);
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(battle_unit_status(&restored, id, "W").unwrap(), report);
        assert_eq!(world.btech, before);
    }
}

/// The physical row reaches both command paths and survives a restart unchanged.
#[tokio::test]
async fn physical_weapon_row_is_shared_with_lua_and_native_status() {
    let (_dir, config, world, id, _) = fixture(include_str!("../game/mechs/AXM-2N.toml")).await;
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let report = battle_unit_status(&world, id, "W").unwrap();
    let line = text::plain(&report)
        .lines()
        .find(|l| l.starts_with("LARM:"))
        .unwrap()
        .to_owned();
    assert!(line.ends_with("Axe[RA]: Rdy "));
    let lua: String = scripts
        .eval_callback(&format!("return btech.unit.status({}, 'W')", id.0))
        .unwrap();
    assert_eq!(report, lua);
    assert!(
        text::plain(&support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            "status W"
        ))
        .contains(line.trim_end())
    );
    assert_eq!(scripts.world().btech, world.btech);
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(battle_unit_status(&restored, id, "W").unwrap(), report);
}

/// Every coordinate purpose uses the same target line for Mechs and vehicles.
#[tokio::test]
async fn coordinate_target_labels_keep_reference_spacing() {
    for template in [
        include_str!("../game/mechs/JR7-D.toml"),
        include_str!("../game/mechs/Demolisher.toml"),
        include_str!("../game/mechs/Kestrel.toml"),
    ] {
        let (_dir, _config, mut world, id, _) = fixture(template).await;
        start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
        for (mode, expected) in [
            (BattleHexTargetMode::UnitAtHex, "Target: 1 0"),
            (BattleHexTargetMode::Hex, "Target: Hex 1 0"),
            (BattleHexTargetMode::Building, "Target: Building at 1 0"),
            (BattleHexTargetMode::Ignite, "Target: Hex 1 0"),
            (BattleHexTargetMode::Clear, "Target: Hex 1 0"),
        ] {
            select_battle_hex_target(
                &mut world,
                id,
                ObjectId(1),
                HexCoordinate { x: 1, y: 0 },
                mode,
            )
            .unwrap();
            let before = world.btech.clone();
            for selector in ["I", "S"] {
                let report = text::plain(&battle_unit_status(&world, id, selector).unwrap());
                assert!(report.lines().any(|line| line == expected), "{report}");
            }
            assert_eq!(world.btech, before);
        }
    }
}

/// Limb countdowns use the same rounded two-second display ticks as weapon countdowns.
#[tokio::test]
async fn limb_recycling_uses_cockpit_ticks_without_advancing_time() {
    let (_dir, _config, mut world, id, _) = fixture(include_str!("../game/mechs/JR7-D.toml")).await;
    for (seconds, ticks) in [(1, 1), (2, 1), (3, 2), (60, 30)] {
        world
            .btech
            .rewrite_unit_record(id, |record| {
                record["limb_recycle"] =
                    serde_json::to_value(std::collections::BTreeMap::from([(
                        BattleSection::LeftArm,
                        seconds,
                    )]))
                    .unwrap();
            })
            .unwrap();
        let before = world.btech.clone();
        let report = text::plain(&battle_unit_status(&world, id, "W").unwrap());
        assert!(report.contains(&format!("LARM: {ticks:<5} RARM: Ready")));
        assert_eq!(world.btech, before);
    }
}

/// Motive damage uses the chassis label, independently of shutdown or reduced speed.
#[tokio::test]
async fn vehicle_damage_banners_follow_owned_motive_state() {
    for (movement, banner) in [
        ("track", "TRACK DESTROYED"),
        ("wheel", "AXLE DESTROYED"),
        ("hover", "LIFT FAN DESTROYED"),
    ] {
        let source = include_str!("../game/mechs/Demolisher.toml").replace(
            "movement = \"track\"",
            &format!("movement = \"{movement}\""),
        );
        let (_dir, _config, mut world, id, _) = fixture(&source).await;
        assert!(!text::plain(&battle_unit_status(&world, id, "").unwrap()).contains(banner));
        damage_battle_vehicle_motive(
            &mut world,
            id,
            BattleVehicleMotiveHit::SpeedLoss { movement_points: 1 },
        )
        .unwrap();
        assert!(!text::plain(&battle_unit_status(&world, id, "").unwrap()).contains(banner));
        damage_battle_vehicle_motive(&mut world, id, BattleVehicleMotiveHit::Immobilize).unwrap();
        let before = world.btech.clone();
        let report = text::plain(&battle_unit_status(&world, id, "").unwrap());
        assert_eq!(report.matches(banner).count(), 1);
        assert_eq!(world.btech, before);
    }
}

/// Independent vehicle and inferno fires share one condition banner.
#[tokio::test]
async fn concurrent_fire_sources_have_one_status_banner() {
    let (_dir, _config, mut world, id, _) =
        fixture(include_str!("../game/mechs/Demolisher.toml")).await;
    for (burning, inferno) in [(true, 0), (false, 30), (true, 30)] {
        world
            .btech
            .rewrite_unit_record(id, |record| {
                let vehicle = record;
                vehicle["burning_sections"] = if burning {
                    serde_json::json!({"front": 30})
                } else {
                    serde_json::json!({})
                };
                vehicle["inferno_remaining"] = serde_json::json!(inferno);
            })
            .unwrap();
        let before = world.btech.clone();
        let report = text::plain(&battle_unit_status(&world, id, "").unwrap());
        assert_eq!(report.matches("ON FIRE").count(), 1);
        assert_eq!(world.btech, before);
    }
}

/// A landed rotorcraft names its destroyed rotor without reporting hull destruction.
#[tokio::test]
async fn landed_rotor_loss_has_its_own_damage_banner() {
    let (_dir, _config, mut world, id, _) =
        fixture(include_str!("../game/mechs/Kestrel.toml")).await;
    damage_battle_vehicle_phase(
        &mut world,
        id,
        BattleVehicleSection::Rotor,
        u16::MAX,
        BattleDamagePhase::Internal,
    )
    .unwrap();
    let before = world.btech.clone();
    let report = text::plain(&battle_unit_status(&world, id, "").unwrap());
    assert!(report.lines().any(|line| line == "ROTOR DESTROYED"));
    assert!(!report.lines().any(|line| line == "DESTROYED"));
    assert_eq!(world.btech, before);
}

/// An occupied unit under a placeholder reference, with an observer Mech on unobstructed terrain.
async fn fixture(source: &str) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    named_fixture("test", source).await
}

/// An occupied unit with the given reference and a separate observer Mech on unobstructed terrain.
async fn named_fixture(
    reference: &str,
    source: &str,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Status yard".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "yard",
        MapAsset::from_cells("2 2\n.0.0\n.0.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Vehicle".into(), Kind::Thing);
    BattleUnitTemplate::parse(reference, source)
        .unwrap()
        .create(&mut world, id)
        .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    let observer = world.create(&config, "Observer".into(), Kind::Thing);
    BattleUnitTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml"))
        .unwrap()
        .create(&mut world, observer)
        .unwrap();
    place_battle_unit(&mut world, observer, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(observer);
    assign_battle_pilot(&mut world, observer, ObjectId(2)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, observer, ObjectId(2), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    set_battle_observer(&mut world, observer, true).unwrap();
    world
        .btech
        .rewrite_unit_record(observer, |record| {
            record["contacts"][id.0.to_string()] = serde_json::json!({"identified": true});
        })
        .unwrap();
    (dir, config, world, id, observer)
}

#[tokio::test]
async fn vehicle_cockpit_sections_share_native_lua_observer_and_restart_reports() {
    for (source, sinks) in [
        (include_str!("../game/mechs/Demolisher.toml").to_owned(), 0),
        (include_str!("../game/mechs/Kestrel.toml").to_owned(), 0),
        (
            support::templates::without_flags(
                include_str!("../game/mechs/Kestrel.toml"),
                &["ICEEngine_Tech"],
            ),
            10,
        ),
    ] {
        let (_dir, config, world, id, observer) = fixture(&source).await;
        assert!(
            battle_unit_status(&world, id, "N")
                .unwrap()
                .ends_with(&format!(" {sinks} "))
        );
        let before = world.btech.clone();
        let scripts = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        for selection in [
            "", "armor", "info", "weapons", "heat", "short", "N", "NW", "NAI", "SNW",
        ] {
            let report = battle_unit_status(&world, id, selection).unwrap();
            let lua: String = scripts
                .eval_callback(&format!(
                    "return btech.unit.status({},'{}')",
                    id.0, selection
                ))
                .unwrap();
            assert_eq!(report, lua);
            let native = support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("status {selection}"),
            );
            let native = text::plain(&native);
            for line in text::plain(&report).lines() {
                assert!(native.contains(line.trim_end()), "{native} missing {line}");
            }
        }
        let info = battle_unit_status(&world, id, "info").unwrap();
        assert!(info.contains("SHUTDOWN"));
        assert!(info.contains("Pilot Name:"));
        assert!(info.contains("Heat Sinks:"));
        assert!(info.contains("FlankSpeed:"));
        assert!(!info.contains("Armor and structure"));
        let armor = battle_unit_status(&world, id, "armor").unwrap();
        assert!(armor.contains("FRONT") && armor.contains("INTERNAL"));
        assert!(!armor.contains("Sensors:"));
        assert!(!armor.contains("Left_Arm"));
        let weapons = battle_unit_status(&world, id, "weapons").unwrap();
        assert!(weapons.contains("WEAPON SYSTEMS") && weapons.contains("AMMUNITION"));
        let mut ordinary = world.clone();
        set_battle_observer(&mut ordinary, observer, false).unwrap();
        let limited = scan_battle_unit(&ordinary, observer, ObjectId(2), id, "i").unwrap();
        assert!(!limited.contains("throttle maximum:"));
        assert!(!limited.contains("Fuel:"));
        let mut unavailable = world.clone();
        unavailable
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::Going);
        assert!(battle_unit_status(&unavailable, id, "short").is_err());

        assert_eq!(
            battle_unit_status(&world, id, "SNW").unwrap(),
            battle_unit_status(&world, id, "short").unwrap()
        );
        assert_eq!(
            battle_unit_status(&world, id, "NAI").unwrap(),
            battle_unit_status(&world, id, "N").unwrap()
        );
        assert_eq!(
            battle_unit_status(&world, id, "bogus").unwrap(),
            battle_unit_status(&world, id, "short").unwrap()
        );
        assert_eq!(
            battle_unit_status(&world, id, "?A!").unwrap(),
            battle_unit_status(&world, id, "armor").unwrap()
        );
        let header = battle_unit_status(&world, id, "?Z!").unwrap();
        assert!(header.contains("Vehicle Name:"));
        assert!(!header.contains("INTERNAL") && !header.contains("WEAPON SYSTEMS"));
        let observer_report =
            scan_battle_unit_action(&scripts, observer, ObjectId(2), id, "i").unwrap();
        assert!(observer_report.contains(&info), "{observer_report}");
        if world.btech.vehicles()[&id].vtol_flight().is_some() {
            assert!(info.contains("LANDED") && info.contains("Vertical Speed:"));
            assert!(info.contains("Fuel: 4000 (100.00 %)"));
        } else {
            assert!(info.contains("Turret Facing:"));
        }
        assert_eq!(scripts.world().btech, before);
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            battle_unit_status(&restored, id, "").unwrap(),
            battle_unit_status(&world, id, "").unwrap()
        );
    }
}

#[tokio::test]
async fn vehicle_status_tracks_live_load_damage_ammunition_and_flight() {
    let (_dir, config, mut world, id, _) =
        fixture(include_str!("../game/mechs/Kestrel.toml")).await;
    let target = world.create(&config, "Load".into(), Kind::Thing);
    BattleUnitTemplate::parse(
        "Savannah_Master",
        include_str!("../game/mechs/Savannah_Master.toml"),
    )
    .unwrap()
    .create(&mut world, target)
    .unwrap();
    support::seed_object_dice(&mut world, target, support::FIXTURE_DICE_SEED);
    let position = world.btech.vehicles()[&id].position().unwrap();
    place_battle_unit(
        &mut world,
        target,
        position.map,
        i64::from(position.x),
        i64::from(position.y),
    )
    .unwrap();
    set_battle_tow(&mut world, id, Some(target)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    world
        .btech
        .rewrite_unit_record(id, |record| {
            let unit = record;
            unit["vtol_flight"]["phase"] = serde_json::json!({"kind":"airborne"});
            unit["vtol_flight"]["altitude"] = 12.25.into();
            unit["vtol_flight"]["vertical_speed"] = 1.0.into();
            unit["vtol_fuel"]["remaining"] = 37.into();
        })
        .unwrap();
    world.validate(&config).unwrap();
    let info = battle_unit_status(&world, id, "info").unwrap();
    assert!(info.contains(&format!(
        "FlankSpeed: {:3}",
        battle_throttle_maximum(&world, id, true).unwrap() as i32
    )));
    assert!(text::plain(&info).contains("Vertical Speed:        1 KPH"));
    assert!(!info.contains("LANDED"));
    assert!(info.contains("Fuel: 37 (0.93 %)"));
    let (_dir2, config2, mut tank_world, tank, _) =
        fixture(include_str!("../game/mechs/Demolisher.toml")).await;
    let full = battle_unit_status(&tank_world, tank, "NW").unwrap();
    assert!(full.contains("|AC/20|"), "{full}");
    let bin = tank_world.btech.vehicles()[&tank]
        .loadout()
        .unwrap()
        .ammunition[0]
        .location;
    destroy_battle_vehicle_critical(&mut tank_world, tank, bin).unwrap();
    let partial = battle_unit_status(&tank_world, tank, "NW").unwrap();
    assert_ne!(full, partial);
    let bins = tank_world.btech.vehicles()[&tank]
        .loadout()
        .unwrap()
        .ammunition;
    for bin in bins {
        destroy_battle_vehicle_critical(&mut tank_world, tank, bin.location).unwrap();
    }
    let empty = battle_unit_status(&tank_world, tank, "NW").unwrap();
    assert!(!empty.contains("|AC/20|"), "{empty}");
    tank_world.validate(&config2).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(info, battle_unit_status(&restored, id, "info").unwrap());
}

/// Whole-display snapshots make whitespace, silhouette choice and block order reviewable.
#[tokio::test]
async fn status_layout_snapshots_cover_every_diagram() {
    for (name, reference, source) in [
        ("light", "JR7-D", include_str!("../game/mechs/JR7-D.toml")),
        (
            "medium",
            "SHD-2H",
            include_str!("../game/mechs/SHD-2H.toml"),
        ),
        ("heavy", "WHM-6R", include_str!("../game/mechs/WHM-6R.toml")),
        ("assault", "AS7-D", include_str!("../game/mechs/AS7-D.toml")),
        ("quad", "GOL-1H", include_str!("../game/mechs/GOL-1H.toml")),
        (
            "vehicle",
            "Demolisher",
            include_str!("../game/mechs/Demolisher.toml"),
        ),
        (
            "turretless",
            "Savannah_Master",
            include_str!("../game/mechs/Savannah_Master.toml"),
        ),
        (
            "vtol",
            "Kestrel",
            include_str!("../game/mechs/Kestrel.toml"),
        ),
    ] {
        let (_dir, config, world, id, _) = named_fixture(reference, source).await;
        let before = world.btech.clone();
        let rendered = battle_unit_status(&world, id, "").unwrap();
        let plain = text::plain(&rendered).replace("\r\n", "\n");
        let file =
            support::repository_root().join(format!("tests/fixtures/btech/status/{name}.txt"));
        if std::env::var_os("UPDATE_STATUS_SNAPSHOTS").is_some() {
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(&file, &plain).unwrap();
        }
        assert_eq!(plain, std::fs::read_to_string(file).unwrap(), "{name}");
        assert!(plain.contains("FRONT") && plain.contains("INTERNAL"));
        assert!(plain.find("INTERNAL").unwrap() < plain.find("WEAPON SYSTEMS").unwrap());
        let scripts = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let lua: String = scripts
            .eval_callback(&format!("return btech.unit.status({})", id.0))
            .unwrap();
        assert_eq!(lua, rendered, "{name}: Lua cockpit");
        let native = support::run_text(&scripts, &config, ObjectId(1), 1, "status");
        assert_eq!(
            text::plain(&native).trim_end(),
            text::plain(&rendered).trim_end(),
            "{name}: native cockpit"
        );
        assert_eq!(scripts.world().btech, before);
        assert_eq!(world.btech, before);
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(battle_unit_status(&restored, id, "").unwrap(), rendered);
    }
}

/// Numeric damage colors and erased limb strokes are display-only, preserving other columns.
#[tokio::test]
async fn status_diagram_damage_and_selectors_preserve_live_state() {
    use std::{cell::RefCell, rc::Rc};
    let (_dir, config, world, id, _) = fixture(include_str!("../game/mechs/JR7-D.toml")).await;
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let intact = battle_unit_status(&scripts.world(), id, "a").unwrap();
    set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "mechdamage", "A:0/3")
        .unwrap();
    let damaged = battle_unit_status(&scripts.world(), id, "a").unwrap();
    assert_ne!(damaged, intact);
    assert!(damaged.contains("[fg=red]"));
    let unit = scripts.world().btech.constructed_units()[&id].clone();
    let section = &unit.definition().sections[&BattleSection::LeftArm];
    set_battle_unit_field_action(
        &scripts,
        &config,
        ObjectId(1),
        id,
        "mechdamage",
        &format!("A:0/{},I:0/{}", section.armor, section.internal),
    )
    .unwrap();
    let destroyed = battle_unit_status(&scripts.world(), id, "a").unwrap();
    let before = scripts.world().btech.clone();
    assert_ne!(destroyed, damaged);
    assert_eq!(
        text::plain(&intact)
            .lines()
            .map(str::len)
            .collect::<Vec<_>>(),
        text::plain(&destroyed)
            .lines()
            .map(str::len)
            .collect::<Vec<_>>()
    );
    assert!(
        battle_unit_status(&scripts.world(), id, "h")
            .unwrap()
            .starts_with("Temp:")
    );
    assert_eq!(
        battle_unit_status(&scripts.world(), id, "R").unwrap(),
        battle_unit_status(&scripts.world(), id, "").unwrap()
    );
    assert!(
        battle_unit_status(&scripts.world(), id, "s")
            .unwrap()
            .starts_with("LOC:")
    );
    assert_eq!(scripts.world().btech, before);
}

/// R changes only header identity while preserving literal player-authored names.
#[tokio::test]
async fn model_header_selector_retains_custom_name_and_markup_safety() {
    use std::{cell::RefCell, rc::Rc};
    for source in [
        include_str!("../game/mechs/JR7-D.toml"),
        include_str!("../game/mechs/Demolisher.toml"),
    ] {
        let (_dir, config, world, id, _) = fixture(source).await;
        let model = battle_unit_status(&world, id, "R").unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        set_battle_unit_field_action(
            &scripts,
            &config,
            ObjectId(1),
            id,
            "displayname",
            "[bold]Fox",
        )
        .unwrap();
        let before = scripts.world().btech.clone();
        let normal = battle_unit_status(&scripts.world(), id, "").unwrap();
        assert!(text::plain(&normal).contains("[bold]Fox"));
        assert_eq!(
            battle_unit_status(&scripts.world(), id, "R").unwrap(),
            model
        );
        assert_eq!(
            normal.split_once("\r\n").unwrap().1,
            model.split_once("\r\n").unwrap().1
        );
        assert_eq!(scripts.world().btech, before);
    }
}

/// New-charge timers use reference two-second display ticks and the shared host policy.
#[tokio::test]
async fn charge_target_timer_uses_configured_native_and_lua_layout() {
    for template in [
        include_str!("../game/mechs/JR7-D.toml"),
        include_str!("../game/mechs/GOL-3S.toml"),
    ] {
        let (dir, config, mut world, id, target) = fixture(template).await;
        let config_path = dir.path().join("stompymux.toml");
        let original_config = std::fs::read_to_string(&config_path).unwrap();
        for elapsed in [0, 1, 3, 4, 60] {
            world
                .btech
                .rewrite_unit_record(id, |record| {
                    record["charge"] = serde_json::json!({
                        "target": target, "elapsed": elapsed, "distance": 0.0
                    });
                })
                .unwrap();
            for enabled in [false, true] {
                std::fs::write(
                    &config_path,
                    original_config.replace(
                        "newcharge = 1",
                        &format!("newcharge = {}", i64::from(enabled)),
                    ),
                )
                .unwrap();
                let config = Config::load(dir.path()).unwrap();
                let scripts = Scripts::new(
                    &config,
                    std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
                )
                .unwrap();
                for selector in ["", "I", "S", "A", "W", "H"] {
                    let lua: String = scripts
                        .eval_callback(&format!(
                            "return btech.unit.status({}, '{}')",
                            id.0, selector
                        ))
                        .unwrap();
                    let plain = text::plain(&lua);
                    let charge = plain.lines().find(|line| line.starts_with("ChargeTarget:"));
                    if enabled && matches!(selector, "" | "I" | "S") {
                        let line = charge.expect("charge target line");
                        assert!(line.starts_with("ChargeTarget: Jenner ["), "{line}");
                        assert!(
                            line.ends_with(&format!("\t  ChargeTimer: {}", elapsed / 2)),
                            "{line}"
                        );
                        let native = text::plain(&support::run_text(
                            &scripts,
                            &config,
                            ObjectId(1),
                            1,
                            &format!("status {selector}"),
                        ));
                        assert!(native.contains(line), "{native}");
                    } else {
                        assert!(charge.is_none(), "{plain}");
                    }
                }
                assert_eq!(scripts.world().btech, world.btech);
            }
        }
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            restored.btech.constructed_units()[&id].charge(),
            world.btech.constructed_units()[&id].charge()
        );
    }
}
