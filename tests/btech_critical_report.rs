//! Slot reports preserve anatomy, durable equipment state and native/Lua access semantics.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Supported chassis share the same report fixtures, including an authored stationary vehicle.
fn templates() -> Vec<String> {
    let vehicle = include_str!("../game/mechs/Demolisher.toml");
    vec![
        include_str!("../game/mechs/JR7-D.toml").into(),
        include_str!("../game/mechs/GOL-1H.toml").into(),
        vehicle.into(),
        vehicle.replace("movement = \"track\"", "movement = \"wheel\""),
        vehicle.replace("movement = \"track\"", "movement = \"hover\""),
        vehicle
            .replace("movement = \"track\"", "movement = \"none\"")
            .replace("walk_mp = 5", "walk_mp = 0"),
        include_str!("../game/mechs/Kestrel.toml").into(),
    ]
}

/// A shutdown, mapless cockpit makes unintended running or placement requirements observable.
async fn fixture(template: UnitTemplate) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Critical inspection".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    template.create(&mut world, id).unwrap();
    support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    (dir, config, world, id)
}

/// Lua omits absent optional fields rather than representing them as JSON null.
fn without_nulls(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(fields) => {
            fields.retain(|_, value| !value.is_null());
            for value in fields.values_mut() {
                without_nulls(value);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                without_nulls(value);
            }
        }
        _ => (),
    }
}

/// Every physical section agrees across native display, detached Lua rows and database restart.
#[tokio::test]
async fn section_reports_preserve_all_supported_chassis() {
    for source in templates() {
        let template = UnitTemplate::parse("test", &source).unwrap();
        let sections: Vec<_> = match &template {
            UnitTemplate::Mech(unit) => unit
                .sections
                .iter()
                .filter(|(_, layout)| layout.internal > 0)
                .map(|(section, _)| {
                    (
                        unit.chassis().unwrap().section_name(*section).to_owned(),
                        unit.chassis().unwrap().critical_slots(*section),
                    )
                })
                .collect(),
            UnitTemplate::Vehicle(unit) => unit
                .sections
                .iter()
                .filter(|(_, layout)| layout.internal > 0)
                .map(|(section, _)| (section.name().to_owned(), 12))
                .collect(),
        };
        let (_dir, config, mut world, id) = fixture(template).await;
        assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let before = scripts.world().btech.clone();
        for (section, count) in &sections {
            let report = battle_critical_report(&scripts.world(), id, section).unwrap();
            assert_eq!(report.slots.len(), usize::from(*count));
            assert_eq!(
                report
                    .slots
                    .iter()
                    .map(|slot| slot.slot)
                    .collect::<Vec<_>>(),
                (0..*count).collect::<Vec<_>>()
            );
            assert!(report.slots.iter().all(|slot| matches!(
                slot.condition,
                EquipmentCondition::Empty | EquipmentCondition::Operational
            )));
            let lua: mlua::Table = scripts
                .eval_callback(&format!(
                    "return btech.unit.criticals({}, '{}')",
                    id.0, section
                ))
                .unwrap();
            let mut expected = serde_json::to_value(&report).unwrap();
            without_nulls(&mut expected);
            assert_eq!(serde_json::to_value(lua).unwrap(), expected);
            let rendered = battle_critical_status(&scripts.world(), id, section).unwrap();
            let native = support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("critstatus {section} ignored"),
            );
            for line in rendered.lines() {
                assert!(native.contains(line), "{native} missing {line}");
            }
            assert_eq!(rendered.lines().count(), 1 + usize::from(*count / 2));
            let mutation = format!(
                "local r=btech.unit.criticals({}, '{}'); r.slots[1].equipment='changed'",
                id.0, section
            );
            scripts.eval_callback::<()>(&mutation).unwrap();
            assert!(
                scripts
                    .eval_callback::<()>(&format!("{mutation}; error('abort')"))
                    .is_err()
            );
            assert_eq!(
                battle_critical_report(&scripts.world(), id, section).unwrap(),
                report
            );
        }
        assert_eq!(scripts.world().btech, before);
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        for (section, _) in sections {
            assert_eq!(
                battle_critical_report(&saved, id, &section).unwrap(),
                battle_critical_report(&loaded, id, &section).unwrap()
            );
        }
    }
}

/// Pilot authority precedes syntax checks; neither startup nor map placement is required.
#[tokio::test]
async fn critical_report_native_access_and_anatomy() {
    for source in templates() {
        let (_dir, config, mut world, id) =
            fixture(UnitTemplate::parse("test", &source).unwrap()).await;
        let section = if world.btech.vehicles().contains_key(&id) {
            "fs"
        } else {
            "h"
        };
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        assert!(
            support::run_text(&scripts, &config, ObjectId(1), 1, "critstatus")
                .contains("Take the cockpit")
        );
        assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        assert!(
            support::run_text(&scripts, &config, ObjectId(1), 1, "critstatus")
                .contains("specify a section")
        );
        assert!(
            support::run_text(&scripts, &config, ObjectId(1), 1, "critstatus nonsense")
                .contains("Invalid section")
        );
        assert!(
            support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("critstatus {section}")
            )
            .contains("Criticals")
        );
        assert!(
            support::run_text(&scripts, &config, ObjectId(1), 1, "critstatus/bad h")
                .contains("takes no switches")
        );
        let mut impaired = world.clone();
        let mut state = serde_json::to_value(&impaired.btech).unwrap();
        state["recoveries"]["1"] = serde_json::json!({"remaining":1,"pain_resistance":false,"toughness":false,"dice":Dice::seeded([12;32])});
        impaired.btech = serde_json::from_value(state).unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(impaired))).unwrap();
        let before = scripts.world().btech.clone();
        assert!(
            support::run_text(&scripts, &config, ObjectId(1), 1, "critstatus invalid")
                .contains("unconscious")
        );
        assert_eq!(scripts.world().btech, before);
    }
    let (_dir, _config, world, id) =
        fixture(UnitTemplate::parse("GOL-1H", include_str!("../game/mechs/GOL-1H.toml")).unwrap())
            .await;
    let front = battle_critical_report(&world, id, "fll").unwrap();
    assert_eq!(front.slots.len(), 6);
    assert_eq!(front.slots[0].equipment, "Hip");
    assert_eq!(front.slots[3].equipment, "Foot Actuator");
    assert!(battle_critical_report(&world, id, "la").is_err());
    assert!(battle_critical_report(&world, id, "turret").is_err());
}

/// Slot damage, flood disabling and section loss remain distinct without hiding empty positions.
#[tokio::test]
async fn critical_report_material_conditions() {
    let (_dir, config, mut world, id) =
        fixture(UnitTemplate::parse("GOL-1H", include_str!("../game/mechs/GOL-1H.toml")).unwrap())
            .await;
    let mount = world.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .find(|mount| mount.weapon == Weapon::Ppc)
        .unwrap()
        .clone();
    let location = mount.criticals[0];
    let section = location.section.name();
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["weapon_damage"] =
                serde_json::json!([WeaponDamage::new(location, WeaponDamageKind::Focus)]);
        })
        .unwrap();
    world.validate(&config).unwrap();
    assert_eq!(
        battle_critical_report(&world, id, section).unwrap().slots[usize::from(location.slot)]
            .condition,
        EquipmentCondition::Damaged
    );
    let mut flooded = world.clone();
    flooded
        .btech
        .rewrite_unit_record(id, |record| {
            record["flooded_sections"] = serde_json::json!([location.section]);
        })
        .unwrap();
    assert_eq!(
        battle_critical_report(&flooded, id, section).unwrap().slots[usize::from(location.slot)]
            .condition,
        EquipmentCondition::Disabled
    );
    destroy_battle_critical(&mut world, id, location).unwrap();
    let report = battle_critical_report(&world, id, section).unwrap();
    assert_eq!(
        report.slots[usize::from(location.slot)].condition,
        EquipmentCondition::Destroyed
    );
    assert_eq!(
        report.slots[usize::from(mount.criticals[1].slot)].condition,
        EquipmentCondition::Broken
    );
    apply_damage_phase(
        &mut world,
        id,
        location.section,
        u16::MAX,
        DamagePhase::Internal,
    )
    .unwrap();
    let report = battle_critical_report(&world, id, section).unwrap();
    assert_eq!(report.slots.len(), 12);
    assert!(report.slots.iter().all(|row| matches!(
        row.condition,
        EquipmentCondition::Empty | EquipmentCondition::Destroyed
    )));
    world.validate(&config).unwrap();

    let (_dir, _config, mut world, id) = fixture(
        UnitTemplate::parse("Demolisher", include_str!("../game/mechs/Demolisher.toml")).unwrap(),
    )
    .await;
    let bin = world.btech.vehicles()[&id].loadout().unwrap().ammunition[0].clone();
    let text = battle_critical_status(&world, id, bin.location.section.name()).unwrap();
    assert!(text.contains("[005/005]"));
    destroy_battle_vehicle_critical(&mut world, id, bin.location).unwrap();
    let row = &battle_critical_report(&world, id, bin.location.section.name())
        .unwrap()
        .slots[usize::from(bin.location.slot)];
    assert_eq!(row.condition, EquipmentCondition::Destroyed);
    assert_eq!(row.ammunition_remaining, Some(0));
    let text = battle_critical_status(&world, id, bin.location.section.name()).unwrap();
    let line = text
        .lines()
        .find(|line| line.contains("Destroyed"))
        .unwrap();
    assert!(!line.contains("[000/"));
}

/// Live one-shot state, bin capacities, Artemis links and construction names survive formatting.
#[tokio::test]
async fn critical_equipment_labels() {
    let mut template =
        MechTemplate::parse("JR7-D", include_str!("../game/mechs/JR7-D.toml")).unwrap();
    support::templates::small_cockpit(&mut template, "SMCPIT");
    let missile = template
        .sections
        .get_mut(&MechSection::CenterTorso)
        .unwrap()
        .criticals
        .get_mut(&10)
        .unwrap();
    missile.modes = vec!["OneShot".into(), "OneShot_Used".into(), "RearMount".into()];
    template
        .sections
        .get_mut(&MechSection::Head)
        .unwrap()
        .criticals
        .insert(
            5,
            CriticalDefinition {
                equipment: "ArtemisIV".into(),
                data: "11".into(),
                modes: vec![],
            },
        );
    template
        .sections
        .get_mut(&MechSection::RightTorso)
        .unwrap()
        .criticals
        .get_mut(&0)
        .unwrap()
        .modes = vec!["Halfton".into(), "Inferno".into()];
    let (_dir, config, mut world, id) = fixture(UnitTemplate::Mech(template)).await;
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    assert!(
        battle_critical_status(&world, id, "rt")
            .unwrap()
            .contains("SRM-4 Inferno Ammo [012/012]")
    );
    assert!(
        battle_critical_status(&world, id, "h")
            .unwrap()
            .contains("Small Cockpit")
    );
    assert!(
        battle_critical_status(&world, id, "h")
            .unwrap()
            .contains("[Controls Slot 11]")
    );
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let text = support::run_text(&scripts, &config, ObjectId(1), 1, "critstatus ct");
    assert!(text.contains("OS SRM-4 (Empty) (R)"), "{text}");
    let lua: String = scripts
        .eval_callback(&format!(
            "return btech.unit.criticals({}, 'ct').slots[11].equipment",
            id.0
        ))
        .unwrap();
    assert_eq!(lua, "SRM-4");
    destroy_battle_critical(
        &mut world,
        id,
        CriticalLocation {
            section: MechSection::Head,
            slot: 5,
        },
    )
    .unwrap();
    let text = battle_critical_status(&world, id, "h").unwrap();
    assert!(text.contains("ArtemisIV (Destroyed)"));
    assert!(!text.contains("Controls Slot"));

    let (_dir, _config, world, id) = fixture(
        UnitTemplate::parse("Daishi-H", include_str!("../game/mechs/Daishi-H.toml")).unwrap(),
    )
    .await;
    assert!(
        battle_critical_status(&world, id, "ll")
            .unwrap()
            .contains("Double Heatsink")
    );
    assert!(
        battle_critical_status(&world, id, "ct")
            .unwrap()
            .contains("Engine (XL)")
    );
    assert!(
        battle_critical_status(&world, id, "la")
            .unwrap()
            .contains("CL GaussRifle")
    );
}
