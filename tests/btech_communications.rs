//! Communications equipment's ECCM field: its strength by tonnage, its controls and the damage
//! and shutdown that stop it (TechManual p. 212; Tactical Operations: Advanced Rules p. 98).
use crate::support;
use stompymux_rs::*;

/// A Demolisher carrying `tons` of communications equipment, or a Guardian ECM suite when
/// `tons` is zero, fitted to its tonnage.
fn vehicle(tons: u8) -> VehicleTemplate {
    let mut template =
        VehicleTemplate::parse("Demolisher", include_str!("../game/units/Demolisher.toml"))
            .unwrap();
    let (equipment, data) = if tons == 0 {
        ("Ecm", "-".to_owned())
    } else {
        ("Communications_Equipment", tons.to_string())
    };
    template
        .sections
        .get_mut(&VehicleSection::Front)
        .unwrap()
        .criticals
        .insert(
            0,
            CriticalDefinition {
                equipment: equipment.into(),
                data,
                modes: vec![],
            },
        );
    support::templates::fitted(template)
}

/// A friendly vehicle with `tons` of communications equipment two hexes from a hostile one
/// carrying a Guardian ECM suite in ECM mode, both running with their pilots aboard.
async fn fixture(tons: u8) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Comms field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "comms",
        MapAsset::from_cells("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for (index, template) in [vehicle(tons), vehicle(0)].into_iter().enumerate() {
        let id = world.create(&config, "Unit".into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        create_battle_vehicle(&mut world, id, template).unwrap();
        place_battle_unit(&mut world, id, map, index as i64 * 2, 1).unwrap();
        let pilot = ObjectId(index as i64 + 1);
        world.objects.get_mut(&pilot).unwrap().location = Some(id);
        assign_battle_pilot(&mut world, id, pilot).unwrap();
        start_battle_unit(&mut world, id, pilot, true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0, true);
        }
        ids.push(id);
    }
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][ids[1].0.to_string()]["signature"]["team"] = 99.into();
    world.btech = serde_json::from_value(saved).unwrap();
    toggle_battle_electronics(
        &mut world,
        ids[1],
        ObjectId(2),
        ElectronicSuite::Guardian,
        ElectronicMode::Ecm,
    )
    .unwrap();
    refresh_battle_electronic_fields(&mut world).unwrap();
    (dir, config, world, ids[0], ids[1])
}

/// Seven tons of communications equipment generate a Guardian suite's ECCM and cancel the
/// hostile suite's interference; four tons generate half as much and do not. The field
/// survives a restart and stops when the vehicle shuts down.
#[tokio::test]
async fn communications_eccm_counters_ecm_by_its_tonnage() {
    for (tons, cancels) in [(7, true), (4, false)] {
        let (_dir, config, mut world, comms, _) = fixture(tons).await;
        assert!(
            battle_electronic_field(&world, comms)
                .unwrap()
                .blocks_outgoing_guidance()
        );
        let mode = toggle_battle_electronics(
            &mut world,
            comms,
            ObjectId(1),
            ElectronicSuite::Communications,
            ElectronicMode::Eccm,
        )
        .unwrap();
        assert_eq!(mode, ElectronicMode::Eccm);
        refresh_battle_electronic_fields(&mut world).unwrap();
        assert_eq!(
            battle_electronic_field(&world, comms)
                .unwrap()
                .blocks_outgoing_guidance(),
            !cancels,
            "{tons} tons"
        );
        assert!(
            battle_unit_status(&world, comms, "W")
                .unwrap()
                .contains("Comms([fg=green bold]ECCM[reset])"),
            "{tons} tons"
        );
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, world.btech);
        stop_battle_unit(&mut restored, comms, ObjectId(1)).unwrap();
        assert_eq!(
            restored.btech.vehicles()[&comms]
                .electronics()
                .communications,
            ElectronicMode::Off
        );
    }
}

/// Communications equipment generates no ECM, and less than three tons generate no ECCM
/// (Tactical Operations: Advanced Rules p. 98).
#[tokio::test]
async fn communications_equipment_generates_only_eccm_from_three_tons() {
    let (_dir, _config, mut world, comms, _) = fixture(7).await;
    let refused = toggle_battle_electronics(
        &mut world,
        comms,
        ObjectId(1),
        ElectronicSuite::Communications,
        ElectronicMode::Ecm,
    )
    .unwrap_err();
    assert!(
        refused.to_string().contains("generates ECCM only"),
        "{refused}"
    );
    let (_dir, _config, mut world, light, _) = fixture(2).await;
    let refused = toggle_battle_electronics(
        &mut world,
        light,
        ObjectId(1),
        ElectronicSuite::Communications,
        ElectronicMode::Eccm,
    )
    .unwrap_err();
    assert!(
        refused
            .to_string()
            .contains("3 or more tons of working communications equipment"),
        "{refused}"
    );
    assert!(
        !battle_unit_status(&world, light, "W")
            .unwrap()
            .contains("Comms(")
    );
}

/// A Mech's communications equipment fills a slot per ton, and a sensor hit stops its field
/// (TechManual p. 212).
#[tokio::test]
async fn mech_communications_stop_with_their_sensors() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Comms field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "comms",
        MapAsset::from_cells("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    let mut template =
        MechTemplate::parse("AS7-D", include_str!("../game/units/AS7-D.toml")).unwrap();
    let arm = template.sections.get_mut(&MechSection::RightArm).unwrap();
    arm.criticals.retain(|&slot, _| slot < 4);
    for slot in 4..7 {
        arm.criticals.insert(
            slot,
            CriticalDefinition {
                equipment: "Communications_Equipment".into(),
                data: "-".into(),
                modes: vec![],
            },
        );
    }
    let id = world.create(&config, "Atlas".into(), Kind::Thing);
    create_battle_unit(&mut world, id, support::templates::legal(template)).unwrap();
    place_battle_unit(&mut world, id, map, 1, 1).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0, true);
    }
    assert_eq!(
        world.btech.constructed_units()[&id]
            .communications_tons()
            .unwrap(),
        3
    );
    toggle_battle_electronics(
        &mut world,
        id,
        ObjectId(1),
        ElectronicSuite::Communications,
        ElectronicMode::Eccm,
    )
    .unwrap();
    let sensors = CriticalLocation {
        section: MechSection::Head,
        slot: 1,
    };
    destroy_battle_critical(&mut world, id, sensors).unwrap();
    let unit = &world.btech.constructed_units()[&id];
    assert!(
        !unit
            .electronic_suite_available(ElectronicSuite::Communications)
            .unwrap()
    );
    assert_eq!(unit.electronics().communications, ElectronicMode::Off);
}
