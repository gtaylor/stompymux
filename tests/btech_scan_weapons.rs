//! Reference scan weapon columns, limited readiness disclosure and independent display numbering.
use crate::support;
use crate::support::btech_firing as firing;
use stompymux_rs::*;

/// An empty artillery mount is inspected by a separately piloted scanner with an acquired contact.
async fn fixture(template: &str) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId, usize) {
    let (dir, config, mut world, subject, scanner, index) = firing::fixture_with_target(
        template,
        Some(Weapon::ClanArrowIv),
        include_str!("../game/units/JR7-D.toml"),
    )
    .await;
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(scanner);
    assign_battle_pilot(&mut world, scanner, ObjectId(2)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
    firing::edit(&mut world, scanner, |state| {
        state["motion"]["heading"] = 180.into();
        state["motion"]["desired_heading"] = 180.into();
        state["contacts"][subject.0.to_string()] = serde_json::json!({"identified":true});
    });
    assert!(
        visible_battle_contact(&world, scanner, subject)
            .unwrap()
            .is_some()
    );
    (dir, config, world, subject, scanner, index)
}

/// Reference columns retain only weapon name, display number, location and five-character condition.
#[tokio::test]
async fn scan_weapon_columns_and_countdowns_share_all_chassis() {
    use std::{cell::RefCell, rc::Rc};
    for template in firing::templates() {
        let (_dir, config, world, subject, scanner, index) = fixture(&template).await;
        for remaining in [0, 1, 2, 60] {
            let mut world = world.clone();
            firing::edit(&mut world, subject, |state| {
                if remaining > 0 {
                    state["weapon_recycle"][index.to_string()] = remaining.into();
                }
            });
            let before = serde_json::to_value(&world.btech).unwrap();
            let report = scan_battle_unit(&world, scanner, ObjectId(2), subject, "W").unwrap();
            let plain = text::plain(&report);
            let table = plain
                .split_once("================WEAPON SYSTEMS================\r\n")
                .unwrap()
                .1;
            assert!(table.starts_with("----- Weapon ------ [##]  Location ---- Status\r\n"));
            let row = table
                .lines()
                .find(|row| row.contains("ArrowIVSystem"))
                .unwrap();
            assert_eq!(row.len(), 45);
            assert_eq!(&row[..19], " ArrowIVSystem     ");
            assert_eq!(&row[19..21], " [");
            assert_eq!(&row[40..], if remaining == 0 { "Ready" } else { "-----" });
            assert!(!table.contains("CL.") && !table.contains("AMMUNITION"));
            assert_eq!(serde_json::to_value(&world.btech).unwrap(), before);
            persistence::save(&config.database(), &world).await.unwrap();
            let scripts = Scripts::new(
                &config,
                Rc::new(RefCell::new(
                    persistence::load(&config.database()).await.unwrap(),
                )),
            )
            .unwrap();
            let lua: String = scripts
                .eval_callback(&format!(
                    "return btech.unit.scan({},2,{},'W')",
                    scanner.0, subject.0
                ))
                .unwrap();
            assert_eq!(lua, report);
            scripts.drain_outbox();
            let native = support::run_text_for_player(
                &scripts,
                &config,
                ObjectId(2),
                2,
                &format!("scan #{} W", subject.0),
            );
            assert_eq!(native, report);
            assert_eq!(
                serde_json::to_value(&scripts.world().btech).unwrap(),
                before
            );
        }
    }
}

/// Scan damage checks the first critical; damage elsewhere does not turn this public marker into firing admission.
#[tokio::test]
async fn scan_weapon_damage_uses_first_critical() {
    for template in firing::templates() {
        let (_dir, _, world, subject, scanner, index) = fixture(&template).await;
        let slot_count = world.btech.vehicles().get(&subject).map_or_else(
            || {
                world.btech.constructed_units()[&subject]
                    .loadout()
                    .unwrap()
                    .weapons[index]
                    .criticals
                    .len()
            },
            |unit| unit.loadout().unwrap().weapons[index].criticals.len(),
        );
        for slot in [0, slot_count - 1] {
            let mut world = world.clone();
            if let Some(unit) = world.btech.vehicles().get(&subject) {
                let location = unit.loadout().unwrap().weapons[index].criticals[slot];
                destroy_battle_vehicle_critical(&mut world, subject, location).unwrap();
            } else {
                let location = world.btech.constructed_units()[&subject]
                    .loadout()
                    .unwrap()
                    .weapons[index]
                    .criticals[slot];
                destroy_battle_critical(&mut world, subject, location).unwrap();
            }
            let before = serde_json::to_value(&world.btech).unwrap();
            let report = scan_battle_unit(&world, scanner, ObjectId(2), subject, "W").unwrap();
            let plain = text::plain(&report);
            let row = plain
                .lines()
                .find(|row| row.contains("ArrowIVSystem"))
                .unwrap();
            assert!(row.ends_with(if slot == 0 { "*****" } else { "Ready" }));
            assert_eq!(serde_json::to_value(&world.btech).unwrap(), before);
        }
    }
}

/// Omitting a destroyed section compacts scan numbers without moving the surviving mount's timer.
#[tokio::test]
async fn scan_numbers_skip_destroyed_sections_without_reindexing_state() {
    let (_dir, config, mut world, subject, scanner, index) =
        fixture(include_str!("../game/units/JR7-D.toml")).await;
    assert!(index > 0);
    let lost = world.btech.constructed_units()[&subject]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .filter(|mount| mount.criticals[0].section == MechSection::LeftArm)
        .count();
    assert!(lost > 0 && index >= lost);
    apply_damage_phase(
        &mut world,
        subject,
        MechSection::LeftArm,
        1000,
        DamagePhase::Internal,
    )
    .unwrap();
    firing::edit(&mut world, subject, |state| {
        state["weapon_recycle"][index.to_string()] = 7.into()
    });
    let before = serde_json::to_value(&world.btech).unwrap();
    let report = scan_battle_unit(&world, scanner, ObjectId(2), subject, "W").unwrap();
    let plain = text::plain(&report);
    let rows: Vec<_> = plain
        .lines()
        .skip_while(|line| !line.starts_with("----- Weapon"))
        .skip(1)
        .collect();
    assert!(!rows.iter().any(|row| row.contains("Left Arm")));
    for (number, row) in rows.iter().enumerate() {
        assert_eq!(row[21..23].trim().parse::<usize>().unwrap(), number);
    }
    let artillery = rows
        .iter()
        .find(|row| row.contains("ArrowIVSystem"))
        .unwrap();
    assert_eq!(
        artillery[21..23].trim().parse::<usize>().unwrap(),
        index - lost
    );
    assert!(artillery.ends_with("-----"));
    assert_eq!(
        world.btech.constructed_units()[&subject].weapon_recycle()[&index],
        7
    );
    assert_eq!(serde_json::to_value(&world.btech).unwrap(), before);
    world.validate(&config).unwrap();
}
