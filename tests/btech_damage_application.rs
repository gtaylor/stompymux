//! Vehicle damage fields replace material atomically through both administrative entry points.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

#[tokio::test]
async fn vehicle_material_replacement_matches_native_lua_and_restart() {
    for source in firing::templates().into_iter().skip(2) {
        let (_dir, config, world, id, _, index) =
            firing::fixture_with_supply(&source, Some(Weapon::Mml3), &source, false, Some(""))
                .await;
        let original = world.btech.vehicles()[&id].clone();
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let value = "A:2/2,A:2/1,I:2/1,R:2/1(2),R:2/1(1),G:2/0(6)";
        let output = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("@setmech mechdamage {value}"),
        );
        assert!(output.is_empty(), "{output}");
        lua.eval_callback::<()>(&format!(
            "btech.unit.set_field(1,{},'mechdamage','{value}')",
            id.0
        ))
        .unwrap();
        assert_eq!(native.world().btech, lua.world().btech);
        {
            let world = lua.world();
            let unit = &world.btech.vehicles()[&id];
            assert_eq!(
                unit.sections()[&VehicleSection::Front].armor,
                original.sections()[&VehicleSection::Front].armor - 1
            );
            assert_eq!(
                unit.weapon_failures()[&index],
                EquipmentFailure::AmmunitionJam
            );
            assert_eq!(
                unit.ammunition().iter().sum::<u16>(),
                original.ammunition().iter().sum::<u16>() - 1
            );
            assert_eq!(unit.definition(), original.definition());
            assert_eq!(unit.pilot(), original.pilot());
            assert_eq!(unit.motion(), original.motion());
            world.validate(&config).unwrap();
        }
        let before = lua.world().btech.clone();
        for bad in [
            "G:2/0(8)", "C:7/11", "R:2/0(1)", "A:2/999", "I:2/999", "garbage",
        ] {
            assert!(
                set_battle_unit_field_action(&lua, &config, ObjectId(1), id, "mechdamage", bad)
                    .is_err()
            );
            assert_eq!(lua.world().btech, before);
        }
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'mechdamage',''); error('rollback')",
                id.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, before);
        let saved = lua.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
        set_battle_unit_field_action(&lua, &config, ObjectId(1), id, "mechdamage", "").unwrap();
        assert_eq!(lua.world().btech.vehicles()[&id], original);
    }
}

#[tokio::test]
async fn vehicle_section_loss_and_restoration_use_material_lifecycle_without_combat_rolls() {
    for source in firing::templates().into_iter().skip(2) {
        let (_dir, config, world, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let original = world.btech.vehicles()[&id].sections()[&VehicleSection::Front].clone();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let description = format!("A:2/{},I:2/{}", original.armor, original.internal);
        set_battle_unit_field_action(
            &scripts,
            &config,
            ObjectId(1),
            id,
            "mechdamage",
            &description,
        )
        .unwrap();
        {
            let world = scripts.world();
            let unit = &world.btech.vehicles()[&id];
            assert!(unit.is_destroyed());
            assert_eq!(unit.power(), Power::Off);
            assert!(unit.pilot().is_none());
            world.validate(&config).unwrap();
        }
        set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "mechdamage", "").unwrap();
        let world = scripts.world();
        let unit = &world.btech.vehicles()[&id];
        assert!(!unit.is_destroyed());
        assert_eq!(unit.power(), Power::Off);
        assert!(unit.pilot().is_none());
        assert_eq!(unit.sections()[&VehicleSection::Front], original);
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn restoring_a_launcher_clears_its_spent_state_without_resetting_vehicle_corrections() {
    for source in firing::templates().into_iter().skip(2) {
        let (_dir, config, mut world, id, _, index) =
            firing::fixture_with_supply(&source, Some(Weapon::Mml3), &source, false, Some(""))
                .await;
        firing::edit(&mut world, id, |state| {
            state["definition"]["sections"]["front"]["criticals"]["0"]["modes"] =
                serde_json::json!(["OneShot"]);
            state["spent_launchers"] = serde_json::json!([index]);
            state["weapon_recycle"] = serde_json::json!({(index.to_string()):2});
            state["propulsion"]["maximum"] = 0.0.into();
            state["crew_stun_remaining"] = 10.into();
        });
        world.validate(&config).unwrap();
        let before = serde_json::to_value(&world.btech.vehicles()[&id]).unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        set_battle_unit_field_action(
            &scripts,
            &config,
            ObjectId(1),
            id,
            "mechdamage",
            "C:2/0,G:2/0(6)",
        )
        .unwrap();
        {
            let world = scripts.world();
            let unit = &world.btech.vehicles()[&id];
            assert!(unit.weapon_recycle().is_empty());
            assert!(unit.weapon_failures().is_empty());
            assert!(unit.spent_launchers().contains(&index));
        }
        set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "mechdamage", "").unwrap();
        let world = scripts.world();
        let unit = &world.btech.vehicles()[&id];
        assert!(unit.lost_criticals().is_empty());
        assert!(unit.spent_launchers().is_empty());
        let after = serde_json::to_value(unit).unwrap();
        for field in [
            "propulsion",
            "crew_stun_remaining",
            "dice",
            "pilot_injuries",
            "definition",
        ] {
            assert_eq!(after[field], before[field], "{field}");
        }
        world.validate(&config).unwrap();
    }
}

/// Protection-only edits preserve live corrections; changed criticals rebuild Mech systems.
#[tokio::test]
async fn mech_replacement_recalculates_only_after_critical_changes() {
    for (chassis, source) in firing::templates().into_iter().take(2).enumerate() {
        let (_dir, config, world, id, _, index) =
            firing::fixture_with_supply(&source, Some(Weapon::Mml3), &source, false, Some(""))
                .await;
        let original = world.btech.constructed_units()[&id].clone();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        for (field, value) in [
            ("maxspeed", "12"),
            ("maxjumpspeed", "21.5"),
            ("scanrange", "17"),
        ] {
            set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, field, value).unwrap();
        }
        let corrected =
            serde_json::to_value(&scripts.world().btech.constructed_units()[&id]).unwrap();
        set_battle_unit_field_action(
            &scripts,
            &config,
            ObjectId(1),
            id,
            "mechdamage",
            "A:2/1,R:2/2(1),G:2/0(6)",
        )
        .unwrap();
        {
            let world = scripts.world();
            let unit = &world.btech.constructed_units()[&id];
            assert_eq!(
                unit.weapon_failures()[&index],
                EquipmentFailure::AmmunitionJam
            );
            assert_eq!(
                unit.sections()[&MechSection::LeftTorso].armor,
                original.sections()[&MechSection::LeftTorso].armor - 1
            );
            let saved = serde_json::to_value(unit).unwrap();
            for field in [
                "propulsion",
                "hardware",
                "dice",
                "pilot_injuries",
                "definition",
            ] {
                assert_eq!(saved[field], corrected[field], "{field}");
            }
            world.validate(&config).unwrap();
        }
        set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "mechdamage", "C:2/0")
            .unwrap();
        {
            let world = scripts.world();
            let unit = &world.btech.constructed_units()[&id];
            assert!(!unit.weapon_intact(index).unwrap());
            assert!(unit.weapon_failures().is_empty());
            assert_eq!(unit.mobility().maximum_speed, unit.template_speed());
            // The injected torso launcher replaces the Jenner's two torso jets.
            assert_eq!(
                unit.jump_capacity(100).unwrap().speed,
                if chassis == 0 { 32.25 } else { 0.0 }
            );
            let saved = serde_json::to_value(unit).unwrap();
            assert!(saved["hardware"]["scan"].is_null());
            assert_eq!(saved["dice"], corrected["dice"]);
            world.validate(&config).unwrap();
        }
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
        set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "mechdamage", "").unwrap();
        let world = scripts.world();
        let unit = &world.btech.constructed_units()[&id];
        assert!(unit.weapon_intact(index).unwrap());
        assert_eq!(unit.sections(), original.sections());
        assert_eq!(unit.ammunition(), original.ammunition());
        world.validate(&config).unwrap();
    }
}

/// Both host entry points share Mech replacement, failure rollback and destroyed-head cleanup.
#[tokio::test]
async fn mech_native_lua_replacement_and_hull_lifecycle_agree() {
    for source in firing::templates().into_iter().take(2) {
        let (_dir, config, world, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let head = world.btech.constructed_units()[&id].sections()[&MechSection::Head].clone();
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        for value in [
            "A:2/1,C:7/0".to_owned(),
            format!("A:7/{},I:7/{}", head.armor, head.internal),
        ] {
            let output = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("@setmech mechdamage {value}"),
            );
            assert!(output.is_empty(), "{output}");
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'mechdamage','{value}')",
                id.0
            ))
            .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
        }
        {
            let world = lua.world();
            let unit = &world.btech.constructed_units()[&id];
            assert!(unit.is_destroyed());
            assert_eq!(unit.power(), Power::Off);
            assert!(unit.pilot().is_none());
            world.validate(&config).unwrap();
        }
        let before = lua.world().btech.clone();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'mechdamage',''); error('rollback')",
                id.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, before);
        for value in ["C:7/11", "G:0/0(8)", "I:7/999"] {
            assert!(
                set_battle_unit_field_action(&lua, &config, ObjectId(1), id, "mechdamage", value)
                    .is_err()
            );
            assert_eq!(lua.world().btech, before);
        }
        set_battle_unit_field_action(&lua, &config, ObjectId(1), id, "mechdamage", "").unwrap();
        let saved = lua.world().clone();
        let unit = &saved.btech.constructed_units()[&id];
        assert!(!unit.is_destroyed());
        assert_eq!(unit.power(), Power::Off);
        assert!(unit.pilot().is_none());
        assert_eq!(unit.sections()[&MechSection::Head], head);
        saved.validate(&config).unwrap();
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
    }
}

/// Restoring gyro slots retires corrections based on losses that no longer exist.
#[tokio::test]
async fn hardened_gyro_replacement_rebuilds_damage_without_stale_loss_baselines() {
    let source =
        support::templates::with_flags(include_str!("../game/mechs/JR7-D.toml"), &["HDGyro_Tech"]);
    let (_dir, config, world, id, _, _) = firing::fixture_with_target(&source, None, &source).await;
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    set_battle_unit_field_action(
        &scripts,
        &config,
        ObjectId(1),
        id,
        "mechdamage",
        "C:4/3,C:4/4",
    )
    .unwrap();
    assert_eq!(
        scripts.world().btech.constructed_units()[&id].gyro_damage(),
        1
    );
    set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "critstatus2", "0").unwrap();
    let before = scripts.world().btech.clone();
    set_battle_unit_field_action(
        &scripts,
        &config,
        ObjectId(1),
        id,
        "mechdamage",
        "A:2/1,C:4/3,C:4/4",
    )
    .unwrap();
    let previous = serde_json::to_value(&before.constructed_units()[&id]).unwrap();
    let current = serde_json::to_value(&scripts.world().btech.constructed_units()[&id]).unwrap();
    assert_eq!(
        previous["critical_conditions"],
        current["critical_conditions"]
    );
    set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "mechdamage", "").unwrap();
    assert_eq!(
        scripts.world().btech.constructed_units()[&id].gyro_damage(),
        0
    );
    scripts.world().validate(&config).unwrap();
    set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "mechdamage", "C:4/3")
        .unwrap();
    assert_eq!(
        scripts.world().btech.constructed_units()[&id].gyro_damage(),
        0
    );
    scripts.world().validate(&config).unwrap();
}

/// Rebuilding primary gyro damage must not grant a second use of consumed protection.
#[tokio::test]
async fn gyro_reconstruction_preserves_secondary_protection_and_future_hit_behavior() {
    for source in firing::templates().into_iter().take(2) {
        let source = support::templates::with_flags(&source, &["HDGyro_Tech"]);
        for protection_used in [false, true] {
            let (_dir, config, world, id, _, _) =
                firing::fixture_with_target(&source, None, &source).await;
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            set_battle_unit_field_action(
                &scripts,
                &config,
                ObjectId(1),
                id,
                "critstatus2",
                if protection_used { "a" } else { "-" },
            )
            .unwrap();
            for material in ["C:4/3,C:4/4", ""] {
                set_battle_unit_field_action(
                    &scripts,
                    &config,
                    ObjectId(1),
                    id,
                    "mechdamage",
                    material,
                )
                .unwrap();
                let report = view_battle_unit_fields_action(
                    &scripts,
                    &config,
                    ObjectId(1),
                    id,
                    "critstatus2",
                )
                .unwrap();
                assert_eq!(
                    report.fields[0].value.as_deref(),
                    Some(if protection_used { "a" } else { "-" })
                );
            }
            assert_eq!(
                scripts.world().btech.constructed_units()[&id].gyro_damage(),
                0
            );
            let mut replay = scripts.world().clone();
            replay.btech =
                serde_json::from_value(serde_json::to_value(&replay.btech).unwrap()).unwrap();
            let location = CriticalLocation {
                section: MechSection::CenterTorso,
                slot: 3,
            };
            let outcome = destroy_battle_critical(&mut scripts.world_mut(), id, location).unwrap();
            assert_eq!(
                outcome,
                destroy_battle_critical(&mut replay, id, location).unwrap()
            );
            assert_eq!(scripts.world().btech, replay.btech);
            assert_eq!(
                scripts.world().btech.constructed_units()[&id].gyro_damage(),
                u8::from(protection_used)
            );
            let report =
                view_battle_unit_fields_action(&scripts, &config, ObjectId(1), id, "critstatus2")
                    .unwrap();
            assert_eq!(report.fields[0].value.as_deref(), Some("a"));
            scripts.world().validate(&config).unwrap();
        }
    }
}

/// Raw slot replacement retains probe conditions; subsequent combat criticals still assert failure.
#[tokio::test]
async fn material_replacement_preserves_light_probe_conditions_across_chassis() {
    for (chassis, source) in firing::templates().into_iter().enumerate() {
        for failed in [false, true] {
            let (_dir, config, mut world, id, _, _) =
                firing::fixture_with_target(&source, None, &source).await;
            let mech = world.btech.constructed_units().contains_key(&id);
            firing::edit(&mut world, id, |unit| {
                let section = if mech { "LeftTorso" } else { "front" };
                for slot in [10, 11] {
                    unit["definition"]["sections"][section]["criticals"][slot.to_string()] = serde_json::json!({
                        "equipment":"Light_BAP","data":"-","modes":[]
                    });
                }
            });
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            if failed {
                set_battle_unit_field_action(
                    &scripts,
                    &config,
                    ObjectId(1),
                    id,
                    "critstatus2",
                    "b",
                )
                .unwrap();
            }
            for damage in ["C:2/10,C:2/11", ""] {
                set_battle_unit_field_action(
                    &scripts,
                    &config,
                    ObjectId(1),
                    id,
                    "mechdamage",
                    damage,
                )
                .unwrap();
                let report = view_battle_unit_fields_action(
                    &scripts,
                    &config,
                    ObjectId(1),
                    id,
                    "critstatus2",
                )
                .unwrap();
                assert_eq!(
                    report.fields[0].value.as_deref(),
                    Some(if failed { "b" } else { "-" })
                );
                let world = scripts.world();
                let available = if mech {
                    world.btech.constructed_units()[&id]
                        .active_probe_available(ActiveProbe::Light)
                        .unwrap()
                } else {
                    world.btech.vehicles()[&id]
                        .active_probe_available(ActiveProbe::Light)
                        .unwrap()
                };
                assert_eq!(available, !failed);
            }
            let saved = scripts.world().clone();
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                saved.btech
            );
            if mech {
                destroy_battle_critical(
                    &mut scripts.world_mut(),
                    id,
                    CriticalLocation {
                        section: MechSection::LeftTorso,
                        slot: 10,
                    },
                )
                .unwrap();
            } else {
                destroy_battle_vehicle_critical(
                    &mut scripts.world_mut(),
                    id,
                    VehicleCriticalLocation {
                        section: VehicleSection::Front,
                        slot: 10,
                    },
                )
                .unwrap();
            }
            let report =
                view_battle_unit_fields_action(&scripts, &config, ObjectId(1), id, "critstatus2")
                    .unwrap();
            assert_eq!(
                report.fields[0].value.as_deref(),
                Some("b"),
                "chassis={chassis} initial_failure={failed}"
            );
            set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "mechdamage", "")
                .unwrap();
            let report =
                view_battle_unit_fields_action(&scripts, &config, ObjectId(1), id, "critstatus2")
                    .unwrap();
            assert_eq!(report.fields[0].value.as_deref(), Some("b"));
            scripts.world().validate(&config).unwrap();
        }
    }
}

/// Material replacement does not perform excluded repair-bay breach clearing.
#[tokio::test]
async fn replacement_retains_exposure_and_keeps_breached_equipment_unavailable() {
    for source in firing::templates() {
        let (_dir, config, mut world, id, _, index) =
            firing::fixture_with_supply(&source, Some(Weapon::Mml3), &source, false, Some(""))
                .await;
        let mech = world.btech.constructed_units().contains_key(&id);
        let bins: Vec<_> = world
            .btech
            .constructed_units()
            .get(&id)
            .map(|unit| {
                unit.loadout()
                    .unwrap()
                    .ammunition
                    .iter()
                    .enumerate()
                    .filter(|(_, bin)| bin.location.section == MechSection::LeftTorso)
                    .map(|(index, _)| index)
                    .collect()
            })
            .unwrap_or_default();
        firing::edit(&mut world, id, |unit| {
            let section = if mech { "LeftTorso" } else { "front" };
            unit["definition"]["sections"][section]["criticals"]["11"] = serde_json::json!({
                "equipment":"CASE","data":"-","modes":[]
            });
            unit["breached_sections"] = serde_json::json!([section]);
            for &bin in &bins {
                unit["ammunition"][bin] = 0.into();
            }
        });
        world.validate(&config).unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        for material in ["C:2/11,G:2/0(6)", ""] {
            set_battle_unit_field_action(
                &scripts,
                &config,
                ObjectId(1),
                id,
                "mechdamage",
                material,
            )
            .unwrap();
            let world = scripts.world();
            if mech {
                let unit = &world.btech.constructed_units()[&id];
                assert!(unit.breached_sections().contains(&MechSection::LeftTorso));
                assert!(!unit.weapon_readiness(index).unwrap().intact);
                assert!(bins.iter().all(|&bin| unit.ammunition()[bin] == 0));
                assert!(unit.weapon_failures().is_empty());
            } else {
                let unit = &world.btech.vehicles()[&id];
                assert!(unit.breached_sections().contains(&VehicleSection::Front));
                assert!(!unit.weapon_readiness(index).unwrap().intact);
                assert!(unit.weapon_failures().is_empty());
            }
            world.validate(&config).unwrap();
        }
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
    }
}

/// Replacement preserves a matching feed-clearing attempt and cancels one whose mount is lost.
#[tokio::test]
async fn replacement_reconciles_active_feed_recovery_without_consuming_early_rolls() {
    for source in firing::templates() {
        let (_dir, config, mut world, id, _, index) =
            firing::fixture_with_supply(&source, Some(Weapon::Mml3), &source, false, Some(""))
                .await;
        let seed = (0..=255)
            .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 12)
            .unwrap();
        firing::edit(&mut world, id, |unit| {
            unit["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap()
        });
        set_battle_weapon_failure(&mut world, id, index, Some(EquipmentFailure::AmmunitionJam))
            .unwrap();
        begin_battle_unjam(&mut world, id, ObjectId(1), index).unwrap();
        for _ in 0..10 {
            assert!(
                advance_battle_unjamming(&mut world, false, false)
                    .unwrap()
                    .is_empty()
            );
        }
        let before = world.btech.clone();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "mechdamage", "G:2/0(6)")
            .unwrap();
        assert_eq!(scripts.world().btech, before);
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        let mut continuing = saved.clone();
        for _ in 0..49 {
            assert!(
                advance_battle_unjamming(&mut continuing, false, false)
                    .unwrap()
                    .is_empty()
            );
        }
        for _ in 0..49 {
            advance_battle_unjamming(&mut replay, false, false).unwrap();
        }
        let notices = advance_battle_unjamming(&mut continuing, false, false).unwrap();
        assert!(
            notices
                .iter()
                .any(|(_, message)| message.contains("manage to clear the jam"))
        );
        assert_eq!(
            notices,
            advance_battle_unjamming(&mut replay, false, false).unwrap()
        );
        assert_eq!(continuing.btech, replay.btech);
        continuing.validate(&config).unwrap();

        set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "mechdamage", "C:2/0")
            .unwrap();
        let mut cancelled = scripts.world().clone();
        let pending = cancelled
            .btech
            .constructed_units()
            .get(&id)
            .map(|unit| unit.unjam())
            .unwrap_or_else(|| cancelled.btech.vehicles()[&id].unjam());
        assert!(pending.is_none());
        let checkpoint = cancelled.btech.clone();
        for _ in 0..60 {
            assert!(
                advance_battle_unjamming(&mut cancelled, false, false)
                    .unwrap()
                    .is_empty()
            );
        }
        assert_eq!(cancelled.btech, checkpoint);
        cancelled.validate(&config).unwrap();
    }
}

/// Rebuilding equipment retains the ammunition-ejection event and its committed cadence.
#[tokio::test]
async fn critical_replacement_preserves_dump_cadence_and_defers_empty_bin_completion() {
    for source in firing::templates().into_iter().take(2) {
        let (_dir, config, mut world, id, _, _) = firing::fixture_with_supply(
            &source,
            Some(Weapon::GaussRifle),
            &source,
            false,
            Some(""),
        )
        .await;
        let (bin, location, capacity) = world.btech.constructed_units()[&id]
            .loadout()
            .unwrap()
            .ammunition
            .iter()
            .enumerate()
            .find(|(_, bin)| bin.weapon == Weapon::GaussRifle)
            .map(|(index, bin)| (index, bin.location, bin.capacity))
            .unwrap();
        begin_battle_dump(
            &mut world,
            id,
            ObjectId(1),
            &format!("lt {}", location.slot + 1),
        )
        .unwrap();
        for _ in 0..5 {
            advance_battle_dumping(&mut world).unwrap();
        }
        let pending = world.btech.constructed_units()[&id].dumping().unwrap();
        assert_eq!(pending.phase, 5);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "mechdamage", "C:2/0")
            .unwrap();
        assert_eq!(
            scripts.world().btech.constructed_units()[&id].dumping(),
            Some(pending)
        );
        assert_eq!(
            scripts.world().btech.constructed_units()[&id].ammunition()[bin],
            capacity
        );
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        let mut continuing = saved.clone();
        let notices = advance_battle_dumping(&mut continuing).unwrap();
        assert_eq!(notices, advance_battle_dumping(&mut replay).unwrap());
        assert_eq!(continuing.btech, replay.btech);
        assert_eq!(
            continuing.btech.constructed_units()[&id]
                .dumping()
                .unwrap()
                .phase,
            6
        );
        assert_eq!(
            continuing.btech.constructed_units()[&id].ammunition()[bin],
            capacity - 1
        );

        set_battle_unit_field_action(
            &scripts,
            &config,
            ObjectId(1),
            id,
            "mechdamage",
            &format!("C:2/{}", location.slot),
        )
        .unwrap();
        let mut empty = scripts.world().clone();
        assert_eq!(
            empty.btech.constructed_units()[&id].dumping(),
            Some(pending)
        );
        assert_eq!(empty.btech.constructed_units()[&id].ammunition()[bin], 0);
        let notices = advance_battle_dumping(&mut empty).unwrap();
        assert!(notices.iter().any(|notice| notice.text.contains("dumped!")));
        assert!(empty.btech.constructed_units()[&id].dumping().is_none());
        empty.validate(&config).unwrap();
    }
}
