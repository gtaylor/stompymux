//! Native and detached Lua weapon inspections share vehicle modes, stable indices and readiness.
use crate::support;
use stompymux_rs::*;

/// A running vehicle with a present pilot, initially disconnected from a session.
async fn fixture(template: &str) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Test field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        BattleMapAsset::parse("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        BattleVehicleTemplate::parse("test",template).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    (dir, config, world, id)
}

#[tokio::test]
async fn vehicle_weapon_inspection_preserves_indices_failure_details_and_replay() {
    let (_dir, config, initial, id) = fixture(include_str!("../game/mechs/Demolisher.toml")).await;
    for (condition, word) in [
        ("ready", "ready"),
        ("recycle", "not ready"),
        ("disabled", "disabled"),
        ("jammed", "jammed"),
        ("physical", "disabled"),
        ("ap", "not ready"),
    ] {
        let mut world = initial.clone();
        if condition == "physical" {
            destroy_battle_vehicle_critical(
                &mut world,
                id,
                VehicleCriticalLocation {
                    section: BattleVehicleSection::Turret,
                    slot: 0,
                },
            )
            .unwrap();
        } else {
            let mut saved = serde_json::to_value(&world.btech).unwrap();
            let vehicle = &mut saved["vehicles"][id.0.to_string()];
            match condition {
                "recycle" => vehicle["weapon_recycle"]["0"] = serde_json::json!(3),
                "disabled" => vehicle["weapon_failures"]["0"] = serde_json::json!("disabled"),
                "jammed" => {
                    vehicle["weapon_failures"]["0"] = serde_json::json!("jammed");
                    vehicle["weapon_recycle"]["0"] = serde_json::json!(60);
                }
                "ap" => {
                    vehicle["ammunition_modes"]["0"] =
                        serde_json::to_value(BattleAmmunitionMode::ArmorPiercing).unwrap()
                }
                _ => {}
            }
            world.btech = serde_json::from_value(saved).unwrap();
        }
        let expected = battle_weapon_status(&world, id).unwrap();
        let first = expected.lines().nth(1).unwrap();
        assert!(first.starts_with("0: IS.AC/20"));
        assert!(first.contains(&format!("in Turret; {word};")), "{first}");
        if condition == "ap" {
            assert!(first.contains("[AP]") && first.ends_with("; 0"));
        }
        assert!(expected.lines().nth(2).unwrap().starts_with("1: IS.AC/20"));
        let before = world.btech.clone();
        persistence::save(&config.database(), &world).await.unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(battle_weapon_status(&loaded, id).unwrap(), expected);
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        assert_eq!(
            support::run_text(&scripts, &config, ObjectId(1), 1, "weapons"),
            expected
        );
        let report: mlua::Table = scripts
            .eval_callback(&format!("return btech.unit.weapon_states({})", id.0))
            .unwrap();
        let json = serde_json::to_value(report).unwrap();
        assert_eq!(json.as_array().unwrap().len(), 2);
        assert_eq!(json[0]["index"], 0);
        assert_eq!(json[1]["index"], 1);
        assert_eq!(json[0]["section"], "turret");
        if condition == "disabled" {
            assert_eq!(json[0]["failure"], "disabled");
            assert_eq!(json[0]["readiness"]["intact"], true);
        }
        scripts.eval_callback::<()>(&format!("local w=btech.unit.weapon_states({}); w[1].readiness.ammunition=999; w[1].failure='disabled'", id.0)).unwrap();
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
    }
}

#[tokio::test]
async fn electrical_vehicle_failures_display_shorted_without_claiming_physical_loss() {
    let template = include_str!("../game/mechs/Demolisher.toml")
        .lines()
        .filter(|line| !line.contains("Ammo_"))
        .collect::<Vec<_>>()
        .join("\n")
        .replace("IS.AC/20", "IS.MediumLaser");
    let (_dir, config, mut world, id) = fixture(&template).await;
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][id.0.to_string()]["weapon_failures"]["0"] = serde_json::json!("shorted");
    saved["vehicles"][id.0.to_string()]["weapon_recycle"]["0"] = serde_json::json!(60);
    world.btech = serde_json::from_value(saved).unwrap();
    let expected = battle_weapon_status(&world, id).unwrap();
    assert!(
        expected.contains("0: IS.MediumLaser in Turret; shorted; 60s; -"),
        "{expected}"
    );
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let intact: bool = scripts
        .eval_callback(&format!(
            "return btech.unit.weapon_states({})[1].readiness.intact",
            id.0
        ))
        .unwrap();
    assert!(intact);
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "weapons"),
        expected
    );
    assert_eq!(scripts.world().btech, before);
}
