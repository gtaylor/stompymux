//! Booster construction, live critical eligibility and durable hardware inspection.
use crate::support;
use stompymux_rs::*;

/// Install `count` contiguous MASC slots in the first vacant run of an Atlas torso or arm,
/// with armor making room for their mass.
fn with_masc(count: usize, clan: bool) -> (MechTemplate, Vec<CriticalLocation>) {
    let mut template =
        MechTemplate::parse("AS7-D", include_str!("fixtures/btech/units/AS7-D.toml")).unwrap();
    template.attributes.insert(
        "specials".into(),
        if clan { "Clan Masc" } else { "Masc" }.into(),
    );
    if clan {
        for section in template.sections.values_mut() {
            section
                .criticals
                .retain(|_, part| part.equipment != "HeatSink");
        }
        template.heat_sinks = 10;
        template.attributes.insert("heat_sinks".into(), "10".into());
    }
    let mut locations = Vec::new();
    for section in [
        MechSection::LeftTorso,
        MechSection::RightTorso,
        MechSection::LeftArm,
        MechSection::RightArm,
    ] {
        let criticals = &template.sections[&section].criticals;
        let Some(first) = (0..12u8).find(|&first| {
            (first..first + count as u8).all(|slot| slot < 12 && !criticals.contains_key(&slot))
        }) else {
            continue;
        };
        for slot in first..first + count as u8 {
            template
                .sections
                .get_mut(&section)
                .unwrap()
                .criticals
                .insert(
                    slot,
                    CriticalDefinition {
                        equipment: "Masc".into(),
                        data: "-".into(),
                        modes: vec![],
                    },
                );
            locations.push(CriticalLocation { section, slot });
        }
        break;
    }
    assert_eq!(locations.len(), count);
    (crate::support::templates::legal(template), locations)
}

/// MASC fills 5% of an Inner Sphere Mech's tonnage or 4% of a Clan Mech's, rounded to the
/// nearest slot (TechManual p. 52): construction refuses any other count, it weighs a ton per
/// slot, and losing a slot stops it working while the hardware stays installed.
#[test]
fn masc_thresholds_mass_and_critical_loss_follow_live_installed_slots() {
    for (clan, required) in [(false, 5), (true, 4)] {
        let (base, _) = with_masc(0, clan);
        let base = Mech::from_template(base).unwrap();
        assert!(!base.masc_installed().unwrap());
        assert!(!base.masc_operational().unwrap());
        for count in [required - 1, required + 1] {
            let error = Mech::from_template(with_masc(count, clan).0).unwrap_err();
            assert!(error.to_string().contains("MASC needs"), "{error}");
        }
        let (template, locations) = with_masc(required, clan);
        let mut unit = Mech::from_template(template).unwrap();
        assert_eq!(unit.masc_required_slots(), required);
        assert!(unit.masc_installed().unwrap());
        assert!(unit.masc_operational().unwrap());
        assert_eq!(
            unit.mass().unwrap().equipment - base.mass().unwrap().equipment,
            required as u32 * 1024
        );
        let mass = unit.mass().unwrap();
        assert!(matches!(
            unit.destroy_critical(locations[0]).unwrap(),
            Some(CriticalLoss::System {
                system: System::Masc
            })
        ));
        assert!(!unit.masc_operational().unwrap());
        assert!(unit.masc_installed().unwrap());
        assert_eq!(unit.mass().unwrap(), mass);
        assert!(unit.destroy_critical(locations[0]).unwrap().is_none());
    }
    // Normal rounding, with at least one slot on the lightest chassis.
    for (tons, clan, required) in [
        (20, false, 1),
        (25, false, 1),
        (30, false, 2),
        (35, false, 2),
        (40, false, 2),
        (45, true, 2),
        (50, false, 3),
        (50, true, 2),
        (95, false, 5),
        (95, true, 4),
        (100, true, 4),
    ] {
        let (mut template, _) = with_masc(0, clan);
        template.tons = tons;
        assert_eq!(template.masc_slots(), required, "{tons} {clan}");
    }
}

#[tokio::test]
async fn masc_hardware_and_damage_survive_save_and_lua_inspection() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Booster test".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    let (template, locations) = with_masc(5, false);
    create_battle_unit(&mut world, id, template).unwrap();
    destroy_battle_critical(&mut world, id, locations[0]).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, world.btech);
    let scripts =
        Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
    let (installed, operational): (bool, bool) = scripts
        .eval_callback(&format!(
            "local s = btech.unit.state({}); return s.masc_installed, s.masc_operational",
            id.0
        ))
        .unwrap();
    assert_eq!((installed, operational), (true, false));
    assert!(scripts.drain_outbox().is_empty());
}

/// Edit only the committed global phase, as database load would.
fn phase(world: &mut World, value: u8) {
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["turn_clock"] = value.into();
    world.btech = serde_json::from_value(saved).unwrap();
}

/// A running MASC-equipped Atlas on clear terrain.
async fn powered_masc() -> (tempfile::TempDir, Config, World, ObjectId) {
    powered_template(with_masc(5, false).0).await
}

/// Place and start a supplied booster design.
async fn powered_template(template: MechTemplate) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Booster field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "boost.map",
        MapAsset::from_cells(&format!("20 20\n{}", (".0".repeat(20) + "\n").repeat(20))).unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "MASC Atlas".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(&mut world, id, template).unwrap();
    place_battle_unit(&mut world, id, map, 10, 10).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0, true);
    }
    (dir, config, world, id)
}

/// MASC doubles the walking figure, a turn of use is checked once at the turn-clock boundary
/// against the first interval, and an unused turn decays the interval again (Total Warfare
/// p. 137); the whole history survives a save and replays.
#[tokio::test]
async fn masc_toggle_speed_boundary_check_recovery_and_restart_are_shared() {
    let (_dir, config, mut world, id) = powered_masc().await;
    // The Atlas walks 3 and runs 5; MASC doubles the walking figure to a run of 6.
    let base = world.btech.constructed_units()[&id].movement_maximum_speed();
    let walking = world.btech.constructed_units()[&id].walking_speed();
    let boosted = walking * 2.0;
    set_battle_speed(&mut world, id, ObjectId(1), -walking).unwrap();
    phase(&mut world, 5);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!("btech.unit.masc({},1); error('abort')", id.0))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    assert!(support::run_text(&scripts, &config, ObjectId(1), 1, "masc").contains("turned on"));
    {
        let snapshot = scripts.world();
        let unit = &snapshot.btech.constructed_units()[&id];
        assert!(unit.masc_active());
        assert!(unit.masc().used);
        assert_eq!(unit.masc().counter, 0);
        assert!((unit.movement_maximum_speed() - boosted).abs() < 1e-9);
        assert!(boosted > base);
        assert!((unit.motion().unwrap().desired_speed + walking).abs() < 1e-9);
    }
    set_battle_speed(&mut scripts.world_mut(), id, ObjectId(1), boosted).unwrap();
    for _ in 0..20 {
        advance_battle_motion(&mut scripts.world_mut(), MovementRules::STANDARD).unwrap();
    }
    assert!(
        (scripts.world().btech.constructed_units()[&id]
            .motion()
            .unwrap()
            .speed
            - boosted)
            .abs()
            < 1e-9
    );
    // Inside the turn nothing is rolled; the boundary makes the first-interval check.
    assert!(
        advance_battle_boosters_action(&scripts, &config)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        scripts.world().btech.constructed_units()[&id]
            .masc()
            .counter,
        0
    );
    phase(&mut scripts.world_mut(), 0);
    let notices = advance_battle_boosters_action(&scripts, &config).unwrap();
    assert!(
        notices
            .iter()
            .any(|notice| notice.text.starts_with("MASC: turn 1, fails on 2 or less")),
        "{notices:?}"
    );
    {
        let snapshot = scripts.world();
        let unit = &snapshot.btech.constructed_units()[&id];
        assert_eq!(unit.masc().counter, 1);
        assert!(!unit.masc().used);
        assert!(!unit.masc().rested);
    }
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "weapons").contains("MASC: 1 (On)")
    );
    // Switched off, each unused turn steps the interval back down.
    phase(&mut scripts.world_mut(), 3);
    let _: mlua::Value = scripts
        .eval_callback(&format!("return btech.unit.masc({},1)", id.0))
        .unwrap();
    assert!(!scripts.world().btech.constructed_units()[&id].masc_active());
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    let replay =
        Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
    for tick in 4..=31 {
        phase(&mut scripts.world_mut(), tick % 30);
        phase(&mut replay.world_mut(), tick % 30);
        assert_eq!(
            advance_battle_boosters_action(&scripts, &config).unwrap(),
            advance_battle_boosters_action(&replay, &config).unwrap()
        );
    }
    assert_eq!(scripts.world().btech, replay.world().btech);
    let snapshot = scripts.world();
    let unit = &snapshot.btech.constructed_units()[&id];
    assert_eq!(unit.masc().counter, 0);
    assert!(unit.masc().rested);
    assert!(!unit.masc().used);
}

/// A failed MASC check puts one critical hit on each leg and nothing else (Total Warfare
/// p. 137): no fall, no stop, and the MASC itself keeps working.
#[tokio::test]
async fn masc_failure_damages_each_leg_once_and_keeps_masc_working() {
    let (_dir, config, world, id) = powered_masc().await;
    for speed in [0.0_f64, 10.75, -21.5, 64.5] {
        let mut trial = world.clone();
        for pilot in [ObjectId(1), ObjectId(2)] {
            trial.objects.get_mut(&pilot).unwrap().location = Some(id);
            trial
                .objects
                .get_mut(&pilot)
                .unwrap()
                .flags
                .insert(Flag::Connected);
        }
        toggle_battle_masc(&mut trial, id, ObjectId(1)).unwrap();
        // The fifth consecutive turn of use fails automatically.
        trial
            .btech
            .rewrite_unit_record(id, |record| {
                record["masc"]["counter"] = serde_json::json!(4);
                record["motion"]["speed"] = serde_json::json!(speed);
                record["dice"] = serde_json::to_value(Dice::seeded([2; 32])).unwrap();
            })
            .unwrap();
        phase(&mut trial, 0);
        let lost = trial.btech.constructed_units()[&id].lost_criticals().len();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(trial))).unwrap();
        let notices = advance_battle_boosters_action(&scripts, &config).unwrap();
        assert!(
            notices
                .iter()
                .any(|notice| notice.text.starts_with("MASC: turn 5, fails on 12 or less")),
            "{notices:?}"
        );
        assert!(
            notices
                .iter()
                .any(|notice| notice.text == "Your MASC overloads and damages your legs!")
        );
        // The MASC failure itself forces no fall; any piloting roll that follows is the
        // ordinary consequence of the leg actuator or hip it happened to hit (TW p. 125).
        scripts.drain_outbox();
        let snapshot = scripts.world();
        let unit = &snapshot.btech.constructed_units()[&id];
        assert!(!unit.masc().failed);
        assert!(unit.masc().enabled);
        assert!(unit.masc_operational().unwrap());
        assert_eq!(unit.masc().counter, 5);
        assert_eq!(unit.lost_criticals().len(), lost + 2);
        for section in [MechSection::LeftLeg, MechSection::RightLeg] {
            assert_eq!(
                unit.lost_criticals()
                    .iter()
                    .filter(|location| location.section == section)
                    .count(),
                1
            );
        }
        assert!(unit.mobility().maximum_speed > 0.0);
        // Whatever the legs lost, the throttle stays within the new ceiling.
        let envelope = unit.speed_envelope();
        assert!(unit.motion().unwrap().speed.abs() <= envelope.running + 1e-9);
        drop(snapshot);
        stop_battle_unit(&mut scripts.world_mut(), id, ObjectId(1)).unwrap();
        assert_eq!(
            scripts.world().btech.constructed_units()[&id]
                .masc()
                .counter,
            0
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn masc_failure_retries_failed_server_saves_atomically() {
    use sqlx::Connection;
    tokio::task::LocalSet::new().run_until(async {
        let (_dir, config, mut world, id) = powered_masc().await;
        toggle_battle_masc(&mut world, id, ObjectId(1)).unwrap();
        world.btech
            .rewrite_unit_record(id, |record| {
        record["masc"]["counter"] = serde_json::json!(4);
        })
            .unwrap();
        // The next heartbeat crosses the turn boundary and makes the automatic failure.
        phase(&mut world, 29);
        let lost = world.btech.constructed_units()[&id].lost_criticals().len();
        persistence::save(&config.database(), &world).await.unwrap();
        let before = world.btech.clone();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::raw_sql("CREATE TRIGGER deny_masc BEFORE UPDATE ON btech_units BEGIN SELECT RAISE(ABORT,'masc failure'); END;").execute(&mut sql).await.unwrap();
        let (_address, shutdown, task, _lua, mut heartbeats) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        heartbeats.attempt().await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech, before);
        sqlx::query("DROP TRIGGER deny_masc").execute(&mut sql).await.unwrap();
        let restored = heartbeats.until_saved(&config, 5, |restored| restored.btech.constructed_units()[&id].lost_criticals().len() > lost).await;
        assert_eq!(restored.btech.constructed_units()[&id].lost_criticals().len(), lost + 2);
        assert!(restored.btech.constructed_units()[&id].masc().enabled);
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

/// A compressor critical and the explicit supercharger technology flag.
fn with_supercharger() -> (MechTemplate, CriticalLocation) {
    let (mut template, _) = with_masc(5, false);
    template
        .attributes
        .insert("specials".into(), "Masc SuperCharger_Tech".into());
    let section = MechSection::CenterTorso;
    let slots = &mut template.sections.get_mut(&section).unwrap().criticals;
    let slot = 10;
    assert_eq!(slots[&slot].equipment, "IS.MediumLaser");
    slots.insert(
        slot,
        CriticalDefinition {
            equipment: "SuperCharger".into(),
            data: "-".into(),
            modes: vec![],
        },
    );
    (
        crate::support::templates::legal(template),
        CriticalLocation { section, slot },
    )
}

#[tokio::test]
async fn combined_boosters_preserve_speed_envelopes_check_order_and_lua_rollback() {
    let (template, _) = with_supercharger();
    let (_dir, config, mut world, id) = powered_template(template).await;
    let base = world.btech.constructed_units()[&id].movement_maximum_speed();
    set_battle_speed(&mut world, id, ObjectId(1), base).unwrap();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.supercharger({},1); error('abort')",
                id.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "scharge")
            .contains("Supercharger has been turned on")
    );
    let _: mlua::Value = scripts
        .eval_callback(&format!("return btech.unit.masc({},1)", id.0))
        .unwrap();
    {
        let snapshot = scripts.world();
        let unit = &snapshot.btech.constructed_units()[&id];
        // Walk 3 under both boosters runs at 3 x 2.5 rounded up: 8 MP, and a forward throttle
        // keeps pace with the ceiling through each toggle.
        assert!((unit.movement_maximum_speed() - 86.0).abs() < 1e-9);
        assert!((unit.motion().unwrap().desired_speed - 86.0).abs() < 1e-9);
    }
    phase(&mut scripts.world_mut(), 0);
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    let replay =
        Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
    let notices = advance_battle_boosters_action(&scripts, &config).unwrap();
    assert_eq!(
        notices,
        advance_battle_boosters_action(&replay, &config).unwrap()
    );
    // Each device makes its own separate activation roll (TacOps AUE p. 345).
    let checks: Vec<_> = notices
        .iter()
        .filter(|notice| notice.text.contains("fails on"))
        .collect();
    assert_eq!(checks.len(), 2, "{notices:?}");
    assert!(checks[0].text.starts_with("Supercharger:"));
    assert!(checks[1].text.starts_with("MASC:"));
    assert_eq!(scripts.world().btech, replay.world().btech);
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "weapons").contains("SCHARGE: 1 (On)")
    );
    let _: mlua::Value = scripts
        .eval_callback(&format!("return btech.unit.supercharger({},1)", id.0))
        .unwrap();
    assert!(!scripts.world().btech.constructed_units()[&id].supercharger_active());
}

/// A failed supercharger check rolls on the Determining Critical Hits Table and applies the
/// hits to the upper-most undamaged center-torso slots, the Atlas's engine, then destroys
/// the supercharger (Tactical Operations: Advanced Units and Equipment p. 345).
#[tokio::test]
async fn supercharger_failure_rolls_critical_hits_onto_the_upper_torso_slots_and_replays() {
    let (template, compressor) = with_supercharger();
    let (_dir, config, world, id) = powered_template(template).await;
    for (wanted, critical_roll) in [(0, 7), (1, 8), (2, 10), (3, 12)] {
        let seed = (0..=255)
            .find(|seed| {
                let mut dice = Dice::seeded([*seed; 32]);
                dice.two_d6();
                dice.two_d6() == critical_roll
            })
            .unwrap();
        let mut trial = world.clone();
        toggle_battle_supercharger(&mut trial, id, ObjectId(1)).unwrap();
        trial
            .btech
            .rewrite_unit_record(id, |record| {
                record["supercharger"]["counter"] = serde_json::json!(4);
                record["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
            })
            .unwrap();
        phase(&mut trial, 0);
        persistence::save(&config.database(), &trial).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(trial))).unwrap();
        let replay =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
        let notices = advance_battle_boosters_action(&scripts, &config).unwrap();
        assert_eq!(
            notices,
            advance_battle_boosters_action(&replay, &config).unwrap()
        );
        assert_eq!(scripts.world().btech, replay.world().btech);
        let snapshot = scripts.world();
        let unit = &snapshot.btech.constructed_units()[&id];
        assert!(unit.supercharger().failed);
        assert!(!unit.supercharger_operational());
        assert!(unit.critical_destroyed(compressor));
        assert_eq!(unit.system_hits(System::Engine), wanted as u8);
        assert_eq!(unit.is_destroyed(), wanted >= 3);
        assert!(notices.iter().any(|notice| notice.text
            == format!(
                "Your supercharger overloads and explodes!  Critical roll: {critical_roll}"
            )));
        assert!(
            notices
                .iter()
                .any(|notice| notice.text.contains("supercharger overloads"))
        );
    }
}

/// Running both boosters adds no penalty to either roll: an 11 on the fourth interval
/// (fails on 10 or less) passes with or without the MASC engaged.
#[tokio::test]
async fn simultaneous_boosters_roll_separately_without_a_penalty() {
    let (_dir, config, world, id) = powered_template(with_supercharger().0).await;
    let seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 11)
        .unwrap();
    for both in [false, true] {
        let mut trial = world.clone();
        toggle_battle_supercharger(&mut trial, id, ObjectId(1)).unwrap();
        if both {
            toggle_battle_masc(&mut trial, id, ObjectId(1)).unwrap();
        }
        trial
            .btech
            .rewrite_unit_record(id, |record| {
                record["supercharger"]["counter"] = serde_json::json!(3);
                record["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
            })
            .unwrap();
        phase(&mut trial, 0);
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(trial))).unwrap();
        let notices = advance_battle_boosters_action(&scripts, &config).unwrap();
        assert!(
            notices
                .iter()
                .any(|notice| notice.text == "Supercharger: turn 4, fails on 10 or less, Roll: 11"),
            "{notices:?}"
        );
        assert!(
            !scripts.world().btech.constructed_units()[&id]
                .supercharger()
                .failed
        );
    }
}

/// A supercharger is one critical slot beside the engine and the SuperCharger_Tech flag
/// together (Tactical Operations: Advanced Units & Equipment p. 223); construction refuses
/// either alone.
#[test]
fn supercharger_technology_flag_and_critical_mass_match_construction_behavior() {
    let (template, compressor) = with_supercharger();
    let mut base_template = template.clone();
    base_template
        .sections
        .get_mut(&compressor.section)
        .unwrap()
        .criticals
        .remove(&compressor.slot);
    base_template
        .attributes
        .insert("specials".into(), "Masc".into());
    let base = Mech::from_template(crate::support::templates::legal(base_template)).unwrap();
    let mut unit = Mech::from_template(template.clone()).unwrap();
    assert!(unit.supercharger_installed());
    assert!(unit.supercharger_operational());
    // A supercharger weighs a tenth of the 19-ton engine, rounded up to the half ton.
    let (with, without) = (unit.mass().unwrap(), base.mass().unwrap());
    assert_eq!(with.equipment, without.equipment + 2048);
    assert!(matches!(
        unit.destroy_critical(compressor).unwrap(),
        Some(CriticalLoss::System {
            system: System::Supercharger
        })
    ));
    assert!(unit.supercharger_operational());
    let mut unflagged = template.clone();
    unflagged
        .attributes
        .insert("specials".into(), "Masc".into());
    assert!(Mech::from_template(unflagged).is_err());
    let mut slotless = template;
    slotless
        .sections
        .get_mut(&compressor.section)
        .unwrap()
        .criticals
        .remove(&compressor.slot);
    assert!(Mech::from_template(slotless).is_err());
}

/// A fast biped with both boosters and passive myomer; vacant slots carry the test equipment.
fn myomer_booster_design() -> MechTemplate {
    let mut template =
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap();
    // MASC and triple strength myomer may not be combined (TechManual p. 52), so the
    // supercharger is the booster beside the passive myomer.
    template.attributes.insert(
        "specials".into(),
        "SuperCharger_Tech TripleMyomerTech".into(),
    );
    // The supercharger shares the center torso with the engine; its jump jet moves out.
    let center = &mut template
        .sections
        .get_mut(&MechSection::CenterTorso)
        .unwrap()
        .criticals;
    let jet = center
        .insert(
            11,
            CriticalDefinition {
                equipment: "SuperCharger".into(),
                data: "-".into(),
                modes: vec![],
            },
        )
        .unwrap();
    let torso = &mut template
        .sections
        .get_mut(&MechSection::LeftTorso)
        .unwrap()
        .criticals;
    let vacant = (0..12).find(|slot| !torso.contains_key(slot)).unwrap();
    torso.insert(vacant, jet);
    let mut equipment = vec!["TripleStrengthMyomer"; 6];
    for section in [
        MechSection::CenterTorso,
        MechSection::LeftTorso,
        MechSection::RightTorso,
        MechSection::LeftArm,
        MechSection::RightArm,
    ] {
        for slot in 0..12 {
            let slots = &mut template.sections.get_mut(&section).unwrap().criticals;
            if slots.contains_key(&slot) {
                continue;
            }
            let Some(name) = equipment.pop() else {
                return crate::support::templates::legal(template);
            };
            slots.insert(
                slot,
                CriticalDefinition {
                    equipment: name.into(),
                    data: "-".into(),
                    modes: vec![],
                },
            );
        }
    }
    assert!(equipment.is_empty());
    crate::support::templates::legal(template)
}

/// Set the last committed thermal sample without advancing movement or booster timers.
fn sample_booster_heat(world: &mut World, id: ObjectId, heat: f64) {
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["heat"]["excess"] = serde_json::json!(heat);
        })
        .unwrap();
}

#[tokio::test]
async fn hot_myomer_replaces_the_extra_movement_boost_and_preserves_turning() {
    let (_dir, config, world, id) = powered_template(myomer_booster_design()).await;
    // A 7/11 at heat 9 walks 7 + 2 - 1 = 8 and runs 12; the supercharger doubles the walk to
    // a run of 16 (Total Warfare p. 137). The unit accelerates by a twentieth of the ceiling it
    // would have while cool, 9/14 and up.
    for (compressor, control, acceleration) in
        [(false, 129.0, 150.5 / 20.0), (true, 172.0, 193.5 / 20.0)]
    {
        let mut trial = world.clone();
        sample_booster_heat(&mut trial, id, 9.0);
        if compressor {
            toggle_battle_supercharger(&mut trial, id, ObjectId(1)).unwrap();
        }
        assert_eq!(
            trial.btech.constructed_units()[&id].movement_maximum_speed(),
            control
        );
        set_battle_speed(&mut trial, id, ObjectId(1), control).unwrap();
        let mut turning = trial.clone();
        set_battle_heading(&mut turning, id, ObjectId(1), 90.0).unwrap();
        advance_battle_motion(&mut turning, MovementRules::STANDARD).unwrap();
        assert!(
            (turning.btech.constructed_units()[&id]
                .motion()
                .unwrap()
                .heading
                - 1.5 * control / 10.75)
                .abs()
                < 1e-9
        );
        advance_battle_motion(&mut trial, MovementRules::STANDARD).unwrap();
        assert!(
            (trial.btech.constructed_units()[&id].motion().unwrap().speed - acceleration).abs()
                < 1e-9
        );
        trial.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn hot_throttle_toggle_rounding_survives_save_and_cooling() {
    let (_dir, config, world, id) = powered_template(myomer_booster_design()).await;
    for direction in [1.0, -2.0 / 3.0] {
        let mut trial = world.clone();
        sample_booster_heat(&mut trial, id, 9.0);
        // Backward travel is limited to the walking ceiling, 86 km/h at heat 9.
        let request = |ceiling: f64| if direction > 0.0 { ceiling } else { -86.0 };
        set_battle_speed(&mut trial, id, ObjectId(1), request(129.0)).unwrap();
        toggle_battle_supercharger(&mut trial, id, ObjectId(1)).unwrap();
        let desired = trial.btech.constructed_units()[&id]
            .motion()
            .unwrap()
            .desired_speed;
        // A forward throttle follows the ceiling to 172; a reverse one is left alone.
        assert!((desired - request(172.0)).abs() < 1e-9);
        trial.validate(&config).unwrap();
        persistence::save(&config.database(), &trial).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(trial.btech, restored.btech);
        sample_booster_heat(&mut trial, id, 8.0);
        sample_booster_heat(&mut restored, id, 8.0);
        advance_battle_motion(&mut trial, MovementRules::STANDARD).unwrap();
        advance_battle_motion(&mut restored, MovementRules::STANDARD).unwrap();
        assert_eq!(trial.btech, restored.btech);
        trial.validate(&config).unwrap();
    }
}
