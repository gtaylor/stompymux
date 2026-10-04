//! Equipment reports reuse durable conditions and catalogue facts across supported chassis.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Representative assets as (reference, document) pairs, including stationary construction
/// authored from a tracked chassis.
fn templates() -> Vec<(&'static str, String)> {
    let vehicle = include_str!("../game/mechs/Demolisher.toml");
    vec![
        ("JR7-D", include_str!("../game/mechs/JR7-D.toml").into()),
        ("GOL-1H", include_str!("../game/mechs/GOL-1H.toml").into()),
        ("Demolisher", vehicle.into()),
        (
            "Demolisher",
            vehicle.replace("movement = \"track\"", "movement = \"wheel\""),
        ),
        (
            "Demolisher",
            vehicle.replace("movement = \"track\"", "movement = \"hover\""),
        ),
        (
            "Demolisher",
            vehicle
                .replace("movement = \"track\"", "movement = \"none\"")
                .replace("walk_mp = 5", "walk_mp = 0"),
        ),
        ("Kestrel", include_str!("../game/mechs/Kestrel.toml").into()),
    ]
}

/// Shutdown units with an ordinary passenger exercise report access without a pilot assignment.
async fn fixture(
    reference: &str,
    source: &str,
    placed: bool,
) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Equipment report".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    BattleUnitTemplate::parse(reference, source)
        .unwrap()
        .create(&mut world, id)
        .unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    if placed {
        let map = world.create(&config, "Report yard".into(), Kind::Room);
        create_battle_map(
            &mut world,
            map,
            "yard",
            BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
        )
        .unwrap();
        place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    }
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    (dir, config, world, id)
}

/// Native reports, detached Lua data and restart agree without consuming dice or changing equipment.
#[tokio::test]
async fn reports_share_chassis_state_and_preserve_empty_ammunition() {
    for (reference, source) in templates() {
        let (_dir, config, mut world, id) = fixture(reference, &source, true).await;
        world
            .btech
            .rewrite_unit_record(id, |record| {
                for rounds in record["ammunition"].as_array_mut().unwrap() {
                    *rounds = serde_json::json!(0);
                }
            })
            .unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let before = scripts.world().btech.clone();
        let diagnostics = battle_weapon_diagnostics(&scripts.world(), id).unwrap();
        assert!(!diagnostics.is_empty());
        assert!(
            diagnostics
                .iter()
                .all(|row| row.condition == BattleEquipmentCondition::Operational)
        );
        let specs =
            battle_weapon_specifications(&scripts.world(), id, config.battletech.erange != 0)
                .unwrap();
        let distinct = diagnostics
            .iter()
            .map(|row| row.weapon.name())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(specs.len(), distinct.len());
        for (method, mut expected) in [
            (
                "weapon_diagnostics",
                serde_json::to_value(&diagnostics).unwrap(),
            ),
            (
                "weapon_specifications",
                serde_json::to_value(&specs).unwrap(),
            ),
        ] {
            // Lua represents absent optional fields by omitted keys rather than JSON null.
            for row in expected.as_array_mut().unwrap() {
                row.as_object_mut()
                    .unwrap()
                    .retain(|_, value| !value.is_null());
            }
            let actual: mlua::Table = scripts
                .eval_callback(&format!("return btech.unit.{method}({})", id.0))
                .unwrap();
            assert_eq!(serde_json::to_value(actual).unwrap(), expected);
            assert!(scripts.eval_callback::<()>(&format!("local rows = btech.unit.{method}({}); rows[1].weapon = 'changed'; error('abort')", id.0)).is_err());
        }
        for (command, report) in [
            (
                "weaponstatus",
                battle_weapon_diagnostic_text(&scripts.world(), id).unwrap(),
            ),
            (
                "weaponspecs",
                battle_weapon_specification_text(
                    &scripts.world(),
                    id,
                    config.battletech.erange != 0,
                )
                .unwrap(),
            ),
        ] {
            let native = support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("{command} ignored arguments"),
            );
            for line in report.lines() {
                assert!(native.contains(line), "{command}: {native} missing {line}");
            }
            assert!(
                support::run_text(
                    &scripts,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("{command}/invalid")
                )
                .contains("takes no switches")
            );
        }
        assert_eq!(scripts.world().btech, before);
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(battle_weapon_diagnostics(&loaded, id).unwrap(), diagnostics);
        assert_eq!(
            battle_weapon_specifications(&loaded, id, config.battletech.erange != 0).unwrap(),
            specs
        );
    }
}

/// Mapless specifications remain available; damage diagnostics retain battlefield admission.
#[tokio::test]
async fn report_access_and_artillery_ranges() {
    for (reference, source) in [
        ("Daishi-H", include_str!("../game/mechs/Daishi-H.toml")),
        ("Naga-A", include_str!("../game/mechs/Naga-A.toml")),
    ] {
        let (_dir, config, world, id) = fixture(reference, source, false).await;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        assert!(
            support::run_text(&scripts, &config, ObjectId(1), 1, "weaponstatus")
                .contains("not on a map")
        );
        let output = support::run_text(&scripts, &config, ObjectId(1), 1, "weaponspecs");
        assert!(output.contains("Weapons statistics for"));
        for row in battle_weapon_specifications(&scripts.world(), id, true).unwrap() {
            let profile = row.weapon.profile();
            let normal =
                u16::from(profile.long_range) * if row.weapon.is_artillery() { 20 } else { 1 };
            assert_eq!(row.long_range, normal);
            assert_eq!(
                row.extended_range,
                Some(normal.max(u16::from(profile.medium_range) * 2))
            );
        }
    }
}

/// Whole-weapon diagnostics retain mount numbers, report live damage and keep destroyed types in specs.
#[tokio::test]
async fn damage_and_vehicle_failures_are_distinct_from_readiness() {
    let (_dir, config, mut world, id) =
        fixture("GOL-1H", include_str!("../game/mechs/GOL-1H.toml"), true).await;
    let loadout = world.btech.constructed_units()[&id].loadout().unwrap();
    let index = loadout
        .weapons
        .iter()
        .position(|mount| mount.weapon == BattleWeapon::Ppc)
        .unwrap();
    let location = loadout.weapons[index].criticals[0];
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["weapon_damage"] = serde_json::json!([BattleWeaponDamage::new(
                location,
                BattleWeaponDamageKind::Focus
            )]);
        })
        .unwrap();
    world.validate(&config).unwrap();
    let row = &battle_weapon_diagnostics(&world, id).unwrap()[index];
    assert_eq!(row.condition, BattleEquipmentCondition::Damaged);
    assert_eq!(row.damaged_slots, 1);
    assert_eq!(
        row.effects,
        world.btech.constructed_units()[&id]
            .weapon_damage_effects(index)
            .unwrap()
    );
    assert!(
        battle_weapon_diagnostic_text(&world, id)
            .unwrap()
            .contains("Focus misalignment")
    );
    let specs = battle_weapon_specifications(&world, id, false).unwrap();
    destroy_battle_critical(&mut world, id, location).unwrap();
    assert_eq!(
        battle_weapon_diagnostics(&world, id).unwrap()[index].condition,
        BattleEquipmentCondition::Destroyed
    );
    let destroyed = &battle_weapon_diagnostics(&world, id).unwrap()[index];
    assert_eq!(destroyed.damaged_slots, 0);
    assert_eq!(destroyed.destroyed_slots, 1);
    assert_eq!(destroyed.effects, BattleWeaponDamageEffects::default());
    assert!(
        !battle_weapon_diagnostic_text(&world, id)
            .unwrap()
            .contains("Focus misalignment")
    );
    assert_eq!(
        battle_weapon_specifications(&world, id, false).unwrap(),
        specs
    );

    let (_dir, _config, mut world, id) = fixture(
        "Demolisher",
        include_str!("../game/mechs/Demolisher.toml"),
        true,
    )
    .await;
    let jam = jam_battle_vehicle_weapon(&mut world, id, BattleVehicleSection::Turret)
        .unwrap()
        .unwrap();
    let row = &battle_weapon_diagnostics(&world, id).unwrap()[jam.index];
    assert_eq!(row.condition, BattleEquipmentCondition::Jammed);
    assert_eq!(row.destroyed_slots, 0);
    assert_eq!(row.effects, BattleWeaponDamageEffects::default());
}

/// Diagnostic observation requires consciousness and vision; catalogue inspection has no such gate.
#[tokio::test]
async fn report_observation_guards_preserve_state() {
    for (reference, source) in templates() {
        let (_dir, config, mut world, _) = fixture(reference, &source, true).await;
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["recoveries"]["1"] = serde_json::json!({
            "remaining": 1, "pain_resistance": false, "toughness": false,
            "dice": BattleDice::seeded([34;32])
        });
        world.btech = serde_json::from_value(state).unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let before = scripts.world().btech.clone();
        let failure = support::run_text(&scripts, &config, ObjectId(1), 1, "weaponstatus");
        assert!(failure.contains("unconscious"), "{failure}");
        assert!(
            support::run_text(&scripts, &config, ObjectId(1), 1, "weaponspecs")
                .contains("Weapons statistics for")
        );
        assert_eq!(scripts.world().btech, before);
    }
}

/// The reference menu has fixed columns, literal model titles and configured range headings.
#[tokio::test]
async fn specification_menu_matches_reference_columns_and_preserves_state() {
    for (reference, source) in templates() {
        let (_dir, config, world, id) = fixture(reference, &source, false).await;
        let before = world.btech.clone();
        for extended in [false, true] {
            let report = battle_weapon_specification_text(&world, id, extended).unwrap();
            assert!(report.lines().all(|line| text::width(line) == 78));
            let plain = text::plain(&report);
            let lines: Vec<_> = plain.lines().collect();
            assert_eq!(lines[0], "-".repeat(78));
            assert_eq!(lines[3], lines[0]);
            assert_eq!(lines.last().copied().unwrap(), lines[0]);
            let header = if extended {
                "Weapon Name             Heat  Damage  Range: Min Short Med Long Ext VRT"
            } else {
                "Weapon Name             Heat  Damage  Range: Min  Short  Med  Long  VRT"
            };
            assert_eq!(lines[2].trim_end(), header);
            let palette = text::Palette::default();
            let spans = text::Document::Styled(report.clone())
                .spans(&palette, &text::RenderOptions::default());
            let header_span = spans
                .iter()
                .find(|span| span.text.contains("Weapon Name"))
                .unwrap();
            assert_eq!(
                header_span.style.foreground,
                palette.color("green").unwrap()
            );
            let title_span = spans
                .iter()
                .find(|span| span.text.contains("Weapons statistics for"))
                .unwrap();
            assert_eq!(title_span.style.foreground, palette.color("blue").unwrap());
            assert!(title_span.style.bold);
            assert!(!plain.contains('|'));
            if source.contains("Jenner") {
                assert_eq!(lines[1].trim(), "Weapons statistics for Jenner: JR7-D");
                let laser = lines
                    .iter()
                    .find(|line| line.starts_with("IS.MediumLaser "))
                    .unwrap();
                assert_eq!(
                    laser.trim_end(),
                    if extended {
                        "IS.MediumLaser            3      5            0   3     6    9   12 20"
                    } else {
                        "IS.MediumLaser            3      5            0     3      6    9   20"
                    }
                );
            }
        }
        assert_eq!(world.btech, before);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let action = commands::run(&scripts, &config, ObjectId(1), 1, "weaponspecs").unwrap();
        assert!(matches!(
            action,
            CommandAction::Report(CommandReport::Styled(_))
        ));
        assert_eq!(scripts.world().btech, before);
    }
}
