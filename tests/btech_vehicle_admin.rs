//! Native/Lua vehicle administration shares asset dispatch, transaction rollback and detached state.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

#[tokio::test]
async fn vehicle_commands_and_lua_share_creation_placement_and_snapshot_state() {
    let (dir, config, mut world) = support::isolated_world().await;
    std::fs::create_dir_all(dir.path().join("mechs")).unwrap();
    std::fs::write(
        dir.path().join("mechs/Demolisher.toml"),
        include_str!("../game/mechs/Demolisher.toml"),
    )
    .unwrap();
    let map = world.create(&config, "Battlefield".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        BattleMapAsset::from_cells("2 1\n.0.0\n").unwrap(),
    )
    .unwrap();
    let first = world.create(&config, "First".into(), Kind::Thing);
    let second = world.create(&config, "Second".into(), Kind::Thing);
    for id in [first, second] {
        world.objects.get_mut(&id).unwrap().location = Some(ObjectId(config.start()));
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    }
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let text = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        &format!("@btech unit-create #{}=Demolisher", first.0),
    );
    assert!(text.contains("constructed"), "{text}");
    scripts
        .eval_callback::<()>(&format!("btech.unit.create({}, 'Demolisher')", second.0))
        .unwrap();
    let mut first_state = serde_json::to_value(&scripts.world().btech.vehicles()[&first]).unwrap();
    let mut second_state =
        serde_json::to_value(&scripts.world().btech.vehicles()[&second]).unwrap();
    // Newly created units have independent random streams; both adapters construct the same state.
    first_state.as_object_mut().unwrap().remove("dice");
    second_state.as_object_mut().unwrap().remove("dice");
    first_state["crew_recovery"]
        .as_object_mut()
        .unwrap()
        .remove("dice");
    second_state["crew_recovery"]
        .as_object_mut()
        .unwrap()
        .remove("dice");
    assert_eq!(first_state, second_state);
    let text = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        &format!("@btech unit-place #{}=#{},0,0", first.0, map.0),
    );
    assert!(text.contains("placed"), "{text}");
    let before = scripts.world().clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.place({}, {}, 1, 0); error('abort')",
                second.0, map.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before.btech);
    assert_eq!(
        scripts.world().objects[&second].location,
        before.objects[&second].location
    );
    scripts
        .eval_callback::<()>(&format!("btech.unit.place({}, {}, 1, 0)", second.0, map.0))
        .unwrap();
    let state: (String, bool, u16, u16) = scripts.eval_callback(&format!("local s=btech.unit.state({}); s.sections.front.armor=0; s.ammunition[1]=0; local t=btech.unit.state({}); return t.kind,t.simulation_supported,t.sections.front.armor,t.ammunition[1]", first.0, first.0)).unwrap();
    assert_eq!(state, ("vehicle".into(), false, 40, 5));
    let candidate = scripts.world().clone();
    persistence::save(&config.database(), &candidate)
        .await
        .unwrap();
    std::fs::remove_file(dir.path().join("mechs/Demolisher.toml")).unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, candidate.btech);
    scripts
        .eval_callback::<()>(&format!(
            "btech.unit.remove({}, {})",
            second.0,
            config.start()
        ))
        .unwrap();
    assert!(
        scripts.world().btech.vehicles()[&second]
            .position()
            .is_none()
    );
}

#[test]
fn construction_dispatch_uses_declared_class_and_confines_assets() {
    let mech = include_str!("fixtures/btech/mechs/JR7-D.toml");
    let vehicle = include_str!("../game/mechs/Demolisher.toml");
    assert!(matches!(
        BattleUnitTemplate::parse("JR7-D", mech).unwrap(),
        BattleUnitTemplate::Mech(_)
    ));
    assert!(matches!(
        BattleUnitTemplate::parse("Demolisher", vehicle).unwrap(),
        BattleUnitTemplate::Vehicle(_)
    ));
    assert!(
        BattleUnitTemplate::parse(
            "test",
            &vehicle.replace("class = \"vehicle\"", "class = \"vtol\"")
        )
        .is_err()
    );
    assert!(
        BattleUnitTemplate::parse(
            "test",
            &vehicle.replace("class = \"vehicle\"", "class = \"mech\"")
        )
        .is_err()
    );
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("vehicle.toml"), vehicle).unwrap();
    assert!(matches!(
        read_battle_unit_template(dir.path(), "vehicle").unwrap(),
        BattleUnitTemplate::Vehicle(_)
    ));
    assert!(read_battle_unit_template(dir.path(), "../vehicle").is_err());
    assert!(read_battle_unit_template(dir.path(), "/etc/passwd").is_err());
}

/// Operator inspection reports the same live fields for every supported chassis adapter.
#[tokio::test]
async fn operator_inspection_reports_mech_ground_and_vtol_state_without_mutation() {
    for (asset, vehicle) in [
        (include_str!("../game/mechs/JR7-D.toml"), false),
        (include_str!("../game/mechs/Demolisher.toml"), true),
        (include_str!("../game/mechs/Kestrel.toml"), true),
    ] {
        let (_dir, config, mut world) = support::isolated_world().await;
        let map = world.create(&config, "Inspection field".into(), Kind::Room);
        create_battle_map(
            &mut world,
            map,
            "inspection",
            BattleMapAsset::from_cells("2 1\n.0.0\n").unwrap(),
        )
        .unwrap();
        let id = world.create(&config, "Inspected unit".into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        if vehicle {
            create_battle_vehicle(
                &mut world,
                id,
                BattleVehicleTemplate::parse("test", asset).unwrap(),
            )
            .unwrap();
        } else {
            create_battle_unit(
                &mut world,
                id,
                BattleTemplate::parse("test", asset).unwrap(),
            )
            .unwrap();
        }
        place_battle_unit(&mut world, id, map, 1, 0).unwrap();
        world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(id);
        assign_battle_pilot(&mut world, id, ObjectId(2)).unwrap();
        start_battle_unit(&mut world, id, ObjectId(2), false).unwrap();
        for _ in 0..30 {
            advance_battle_units(&mut world, 0);
        }
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let before = scripts.world().btech.clone();
        let output = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("@btech inspect #{}", id.0),
        );
        for expected in [
            "Power: Running",
            "Current mass:",
            "AutoFall: OFF",
            "Heading",
            "Pilot #2",
            "Elevation 0",
            &format!("Hex 1,0 on map #{}", map.0),
        ] {
            assert!(output.contains(expected), "Missing {expected}: {output}");
        }
        assert!(!output.contains("(inactive)"), "{output}");
        let status = support::run_text(&scripts, &config, ObjectId(1), 1, "@btech status");
        assert!(
            status.contains("Mechs, ground vehicles and VTOLs"),
            "{status}"
        );
        assert_eq!(scripts.world().btech, before);
    }
}
