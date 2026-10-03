//! Water destruction preserves material and crew facts while using ordinary wreck cleanup.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Intact vehicle with a working electronic suite and command computer.
fn template() -> BattleVehicleTemplate {
    let mut template =
        BattleVehicleTemplate::parse("Demolisher", include_str!("../game/mechs/Demolisher.toml"))
            .unwrap();
    for (slot, equipment) in [(0, "Ecm"), (1, "C3Master")] {
        template
            .sections
            .get_mut(&BattleVehicleSection::Front)
            .unwrap()
            .criticals
            .insert(
                slot,
                CriticalDefinition {
                    equipment: equipment.into(),
                    data: "-".into(),
                    modes: vec![],
                },
            );
    }
    template
}

#[test]
fn flooding_preserves_armor_ammunition_and_crew_without_reviving_a_wreck() {
    let mut unit = BattleVehicle::new(template()).unwrap();
    let before = unit.clone();
    assert!(unit.destroy_by_flooding());
    assert!(unit.flooded() && unit.is_destroyed());
    assert_eq!(unit.sections(), before.sections());
    assert_eq!(unit.ammunition(), before.ammunition());
    assert_eq!(unit.definition(), before.definition());
    assert_eq!(unit.pilot_injuries(), 0);
    assert!(!unit.c3_operational().unwrap());
    let mut saved = serde_json::to_value(&unit).unwrap();
    saved["burning_sections"]["front"] = 15.into();
    unit = serde_json::from_value(saved).unwrap();
    let burning = unit.clone();
    assert!(!unit.destroy_by_flooding());
    assert_eq!(unit, burning);
    unit.damage_phase(
        BattleVehicleSection::Front,
        1,
        BattleDamagePhase::Armor { rear: false },
    )
    .unwrap();
    assert_eq!(unit.burning_sections(), burning.burning_sections());
    let restored: BattleVehicle =
        serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
    assert_eq!(restored, unit);
    let mut hull_wreck = before;
    hull_wreck
        .damage_phase(
            BattleVehicleSection::Front,
            u16::MAX,
            BattleDamagePhase::Internal,
        )
        .unwrap();
    let snapshot = hull_wreck.clone();
    assert!(!hull_wreck.destroy_by_flooding());
    assert!(!hull_wreck.flooded());
    assert_eq!(hull_wreck, snapshot);
}

#[tokio::test]
async fn water_destruction_clears_controls_and_survives_sqlite_and_lua() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Flood field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(&mut world, id, template()).unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    toggle_battle_electronics(
        &mut world,
        id,
        ObjectId(1),
        BattleElectronicSuite::Guardian,
        BattleElectronicMode::Ecm,
    )
    .unwrap();
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    let value = &mut saved["vehicles"][id.0.to_string()];
    value["motion"]["speed"] = 20.into();
    value["motion"]["desired_speed"] = 20.into();
    value["inferno_remaining"] = 30.into();
    value["burning_sections"]["front"] = 15.into();
    value["extinguishing"] = 20.into();
    world.btech = serde_json::from_value(saved).unwrap();
    let mut unit = world.btech.vehicles()[&id].clone();
    let before = unit.clone();
    assert!(unit.destroy_by_flooding());
    assert_eq!(unit.power(), BattlePower::Off);
    assert_eq!(unit.pilot(), None);
    assert_eq!(unit.motion().unwrap().speed, 0.0);
    assert_eq!(unit.motion().unwrap().desired_speed, 0.0);
    assert_eq!(unit.electronics().guardian, BattleElectronicMode::Off);
    assert_eq!(unit.inferno_remaining(), 0);
    assert!(unit.burning_sections().is_empty());
    assert_eq!(unit.extinguishing(), None);
    assert_eq!(unit.sections(), before.sections());
    assert_eq!(unit.ammunition(), before.ammunition());
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][id.0.to_string()] = serde_json::to_value(&unit).unwrap();
    world.btech = serde_json::from_value(saved).unwrap();
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
    let flags: (bool, bool) = scripts
        .eval_callback(&format!(
            "local s=btech.unit.state({}); return s.flooded,s.destroyed",
            id.0
        ))
        .unwrap();
    assert_eq!(flags, (true, true));
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!("btech.unit.start({},1)", id.0))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    let mut invalid = serde_json::to_value(&unit).unwrap();
    invalid["power"] = serde_json::to_value(BattlePower::Running).unwrap();
    assert!(serde_json::from_value::<BattleVehicle>(invalid).is_err());
}
