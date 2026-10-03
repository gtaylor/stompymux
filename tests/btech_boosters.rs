//! Booster construction, live critical eligibility and durable hardware inspection.
use crate::support;
use stompymux_rs::*;

/// Install booster slots only in genuinely vacant torso/arm positions.
fn with_masc(count: usize, clan: bool) -> (BattleTemplate, Vec<CriticalLocation>) {
    let mut template =
        BattleTemplate::parse("AS7-D", include_str!("fixtures/btech/mechs/AS7-D.toml")).unwrap();
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
        template.heat_sinks = 20;
        template.attributes.insert("heat_sinks".into(), "20".into());
    }
    let mut locations = Vec::new();
    for section in [
        BattleSection::LeftTorso,
        BattleSection::RightTorso,
        BattleSection::LeftArm,
        BattleSection::RightArm,
    ] {
        for slot in 0..12 {
            if locations.len() == count {
                return (template, locations);
            }
            let criticals = &mut template.sections.get_mut(&section).unwrap().criticals;
            if criticals.contains_key(&slot) {
                continue;
            }
            criticals.insert(
                slot,
                CriticalDefinition {
                    equipment: "Masc".into(),
                    data: "-".into(),
                    modes: vec![],
                    brand: None,
                },
            );
            locations.push(CriticalLocation { section, slot });
        }
    }
    assert_eq!(locations.len(), count);
    (template, locations)
}

#[test]
fn masc_thresholds_mass_and_critical_loss_follow_live_installed_slots() {
    for (clan, required) in [(false, 5), (true, 4)] {
        let (base, _) = with_masc(0, clan);
        let base = BattleUnit::from_template(base).unwrap();
        assert!(!base.masc_installed().unwrap());
        assert!(!base.masc_operational().unwrap());
        for count in [required - 1, required, required + 1] {
            let (template, locations) = with_masc(count, clan);
            let mut unit = BattleUnit::from_template(template).unwrap();
            assert_eq!(unit.masc_required_slots(), required);
            assert_eq!(unit.masc_installed().unwrap(), count >= required);
            assert_eq!(unit.masc_operational().unwrap(), count >= required);
            assert_eq!(
                unit.mass().unwrap().equipment - base.mass().unwrap().equipment,
                count as u32 * 1024
            );
            let mass = unit.mass().unwrap();
            assert!(matches!(
                unit.destroy_critical(locations[0]).unwrap(),
                Some(BattleCriticalLoss::System {
                    system: BattleSystem::Masc
                })
            ));
            assert_eq!(unit.masc_operational().unwrap(), count > required);
            assert_eq!(unit.masc_installed().unwrap(), count >= required);
            assert_eq!(unit.mass().unwrap(), mass);
            assert!(unit.destroy_critical(locations[0]).unwrap().is_none());
        }
    }
    // Integer thresholds, including the minimum slot on light chassis.
    for (tons, clan, required) in [
        (20, false, 1),
        (35, false, 1),
        (40, false, 2),
        (45, true, 1),
        (50, true, 2),
        (95, false, 4),
        (95, true, 3),
    ] {
        let (mut template, _) = with_masc(5, clan);
        template.tons = tons;
        template.attributes.insert("tons".into(), tons.to_string());
        let unit = BattleUnit::from_template(template).unwrap();
        assert_eq!(unit.masc_required_slots(), required);
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

/// A running MASC-equipped Atlas on clear terrain.
async fn powered_masc() -> (tempfile::TempDir, Config, World, ObjectId) {
    powered_template(with_masc(5, false).0).await
}

/// Place and start a supplied booster design.
async fn powered_template(
    template: BattleTemplate,
) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Booster field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "boost.map",
        BattleMapAsset::from_cells(&format!("20 20\n{}", (".0".repeat(20) + "\n").repeat(20)))
            .unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "MASC Atlas".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(&mut world, id, template).unwrap();
    place_battle_unit(&mut world, id, map, 10, 10).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    (dir, config, world, id)
}

#[tokio::test]
async fn masc_toggle_speed_first_check_recovery_and_restart_are_shared() {
    let (_dir, config, mut world, id) = powered_masc().await;
    let base = world.btech.constructed_units()[&id].movement_maximum_speed();
    set_battle_speed(&mut world, id, ObjectId(1), -base * 2.0 / 3.0).unwrap();
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
        assert_eq!(unit.masc().remaining, 1);
        assert!((unit.movement_maximum_speed() - base * 4.0 / 3.0).abs() < 1e-9);
        assert!((unit.motion().unwrap().desired_speed + base * 2.0 / 3.0 * 4.0 / 3.0).abs() < 1e-9);
    }
    set_battle_speed(
        &mut scripts.world_mut(),
        id,
        ObjectId(1),
        base * (4.0 / 3.0),
    )
    .unwrap();
    for _ in 0..20 {
        advance_battle_motion(&mut scripts.world_mut(), BattleMovementRules::STANDARD).unwrap();
    }
    assert!(
        (scripts.world().btech.constructed_units()[&id]
            .motion()
            .unwrap()
            .speed
            - base * (4.0 / 3.0))
            .abs()
            < 1e-9
    );
    // God receives the reference's early-check wizard protection.
    let notices = advance_battle_boosters_action(&scripts, &config).unwrap();
    assert!(
        notices
            .iter()
            .any(|notice| notice.text.contains("MASC: BTH 3+"))
    );
    assert_eq!(
        scripts.world().btech.constructed_units()[&id]
            .masc()
            .counter,
        1
    );
    assert_eq!(
        scripts.world().btech.constructed_units()[&id]
            .masc()
            .remaining,
        60
    );
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "weapons").contains("MASC: 1 (On)")
    );
    let _: mlua::Value = scripts
        .eval_callback(&format!("return btech.unit.masc({},1)", id.0))
        .unwrap();
    assert!(!scripts.world().btech.constructed_units()[&id].masc_active());
    for _ in 0..30 {
        advance_battle_boosters_action(&scripts, &config).unwrap();
    }
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    let replay =
        Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
    for _ in 0..30 {
        assert_eq!(
            advance_battle_boosters_action(&scripts, &config).unwrap(),
            advance_battle_boosters_action(&replay, &config).unwrap()
        );
    }
    assert_eq!(scripts.world().btech, replay.world().btech);
    assert_eq!(
        scripts.world().btech.constructed_units()[&id]
            .masc()
            .counter,
        0
    );
    assert_eq!(
        scripts.world().btech.constructed_units()[&id]
            .masc()
            .remaining,
        0
    );
}

#[tokio::test]
async fn masc_failure_breaks_both_hips_and_falls_only_above_one_mp() {
    let (_dir, config, world, id) = powered_masc().await;
    for speed in [0.0_f64, 10.75, -10.75, 10.76, -10.76] {
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
        let mut state = serde_json::to_value(&trial.btech).unwrap();
        state["constructed"][id.0.to_string()]["masc"]["counter"] = serde_json::json!(5);
        state["constructed"][id.0.to_string()]["motion"]["speed"] = serde_json::json!(speed);
        state["constructed"][id.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([2; 32])).unwrap();
        // The failed fall injures the pilot; pin the consciousness stream too.
        state["recoveries"][ObjectId(1).0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([2; 32])).unwrap();
        trial.btech = serde_json::from_value(state).unwrap();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(trial))).unwrap();
        let notices = advance_battle_boosters_action(&scripts, &config).unwrap();
        assert!(notices.iter().any(|notice| notice.text.contains("BTH 13+")));
        let output = scripts.drain_outbox();
        assert!(!output.iter().any(|(who, message)| *who == ObjectId(2)
            && (message.source().starts_with("Modified Pilot Skill:")
                || message.source() == "You make a piloting skill roll!")));
        let pilot: Vec<_> = output
            .iter()
            .filter(|(who, _)| *who == ObjectId(1))
            .map(|(_, message)| message.source())
            .collect();
        if speed.abs() > 10.75 {
            let index = pilot
                .iter()
                .position(|message| *message == "You make a piloting skill roll!")
                .unwrap();
            assert!(index > 0);
            assert!(
                pilot[..index]
                    .iter()
                    .any(|message| message.contains("BTH 13+"))
            );
            let mut dice = BattleDice::seeded([2; 32]);
            dice.two_d6();
            assert!(pilot[index + 1].ends_with(&format!("\tRoll: {}", dice.two_d6())));
        } else {
            assert!(
                !pilot
                    .iter()
                    .any(|message| message.starts_with("Modified Pilot Skill:"))
            );
        }
        let snapshot = scripts.world();
        let unit = &snapshot.btech.constructed_units()[&id];
        assert!(unit.masc().failed);
        assert!(!unit.masc_operational().unwrap());
        assert_eq!(unit.mobility().maximum_speed, 0.0);
        assert_eq!(unit.motion().unwrap().speed, 0.0);
        assert_eq!(unit.posture() == BattlePosture::Prone, speed.abs() > 10.75);
        for section in [BattleSection::LeftLeg, BattleSection::RightLeg] {
            assert!(unit.critical_destroyed(CriticalLocation { section, slot: 0 }));
        }
        drop(snapshot);
        stop_battle_unit(
            &mut scripts.world_mut(),
            id,
            ObjectId(1),
            BattleMovementRules::STANDARD.fall,
        )
        .unwrap();
        assert!(scripts.world().btech.constructed_units()[&id].masc().failed);
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
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][id.0.to_string()]["masc"]["counter"] = serde_json::json!(5);
        world.btech = serde_json::from_value(state).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let before = world.btech.clone();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::raw_sql("CREATE TRIGGER deny_masc BEFORE UPDATE ON btech_units BEGIN SELECT RAISE(ABORT,'masc failure'); END;").execute(&mut sql).await.unwrap();
        let (_address, shutdown, task, _lua) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        support::attempt_heartbeat().await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech, before);
        sqlx::query("DROP TRIGGER deny_masc").execute(&mut sql).await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let restored = persistence::load(&config.database()).await.unwrap();
                if restored.btech.constructed_units()[&id].masc().failed {
                    assert_eq!(restored.btech.constructed_units()[&id].mobility().maximum_speed, 0.0);
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }).await.unwrap();
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

/// A compressor critical and the explicit supercharger technology flag.
fn with_supercharger() -> (BattleTemplate, CriticalLocation) {
    let (mut template, _) = with_masc(5, false);
    template
        .attributes
        .insert("specials".into(), "Masc SuperCharger_Tech".into());
    let section = BattleSection::CenterTorso;
    let slots = &mut template.sections.get_mut(&section).unwrap().criticals;
    let slot = 10;
    assert_eq!(slots[&slot].equipment, "IS.MediumLaser");
    slots.insert(
        slot,
        CriticalDefinition {
            equipment: "SuperCharger".into(),
            data: "-".into(),
            modes: vec![],
            brand: None,
        },
    );
    (template, CriticalLocation { section, slot })
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
        assert!((unit.movement_maximum_speed() - base * (5.0 / 3.0)).abs() < 1e-9);
        assert!((unit.motion().unwrap().desired_speed - base * (16.0 / 9.0)).abs() < 1e-9);
    }
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
    let checks: Vec<_> = notices
        .iter()
        .filter(|notice| notice.text.contains("BTH"))
        .collect();
    assert_eq!(checks.len(), 2);
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

#[tokio::test]
async fn supercharger_failure_damages_ordered_engine_slots_and_replays() {
    let (template, compressor) = with_supercharger();
    let (_dir, config, world, id) = powered_template(template).await;
    for wanted in 1..=4 {
        let seed = (0..=255)
            .find(|seed| {
                let mut dice = BattleDice::seeded([*seed; 32]);
                dice.two_d6();
                dice.die(4).unwrap() == wanted
            })
            .unwrap();
        let mut trial = world.clone();
        toggle_battle_supercharger(&mut trial, id, ObjectId(1)).unwrap();
        let mut state = serde_json::to_value(&trial.btech).unwrap();
        state["constructed"][id.0.to_string()]["supercharger"]["counter"] = serde_json::json!(5);
        state["constructed"][id.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        trial.btech = serde_json::from_value(state).unwrap();
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
        assert_eq!(unit.system_hits(BattleSystem::Engine), wanted as u8);
        assert_eq!(unit.is_destroyed(), wanted >= 3);
        assert!(
            notices
                .iter()
                .any(|notice| notice.text.contains("supercharger overloads"))
        );
    }
}

#[tokio::test]
async fn simultaneous_boosters_apply_the_other_devices_roll_penalty() {
    let (_dir, config, world, id) = powered_template(with_supercharger().0).await;
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 11)
        .unwrap();
    for both in [false, true] {
        let mut trial = world.clone();
        if both {
            toggle_battle_masc(&mut trial, id, ObjectId(1)).unwrap();
        }
        toggle_battle_supercharger(&mut trial, id, ObjectId(1)).unwrap();
        let mut state = serde_json::to_value(&trial.btech).unwrap();
        state["constructed"][id.0.to_string()]["supercharger"]["counter"] = serde_json::json!(4);
        if both {
            state["constructed"][id.0.to_string()]["masc"]["remaining"] = serde_json::json!(60);
        }
        state["constructed"][id.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        trial.btech = serde_json::from_value(state).unwrap();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(trial))).unwrap();
        let notices = advance_battle_boosters_action(&scripts, &config).unwrap();
        assert!(notices.iter().any(|notice| notice.text
            == format!("Supercharger: BTH 11, Roll: {}", if both { 10 } else { 11 })));
        assert_eq!(
            scripts.world().btech.constructed_units()[&id]
                .supercharger()
                .failed,
            both
        );
    }
}

#[test]
fn supercharger_technology_flag_and_critical_mass_match_construction_behavior() {
    let (mut template, compressor) = with_supercharger();
    let mut base_template = template.clone();
    base_template
        .sections
        .get_mut(&compressor.section)
        .unwrap()
        .criticals
        .remove(&compressor.slot);
    let base = BattleUnit::from_template(base_template).unwrap();
    let mut unit = BattleUnit::from_template(template.clone()).unwrap();
    assert!(unit.supercharger_installed());
    assert!(unit.supercharger_operational());
    // The reference's critical-weight switch assigns no separate mass to this part.
    assert_eq!(unit.mass().unwrap(), base.mass().unwrap());
    assert!(matches!(
        unit.destroy_critical(compressor).unwrap(),
        Some(BattleCriticalLoss::System {
            system: BattleSystem::Supercharger
        })
    ));
    assert!(unit.supercharger_operational());
    template.attributes.insert("specials".into(), "Masc".into());
    assert!(
        !BattleUnit::from_template(template.clone())
            .unwrap()
            .supercharger_installed()
    );
    template
        .attributes
        .insert("specials".into(), "Masc SuperCharger_Tech".into());
    template
        .sections
        .get_mut(&compressor.section)
        .unwrap()
        .criticals
        .remove(&compressor.slot);
    assert!(
        BattleUnit::from_template(template)
            .unwrap()
            .supercharger_operational()
    );
}

/// A fast biped with both boosters and passive myomer; vacant slots carry the test equipment.
fn myomer_booster_design() -> BattleTemplate {
    let mut template =
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
    template.attributes.insert(
        "specials".into(),
        "Masc SuperCharger_Tech TripleMyomerTech".into(),
    );
    let mut equipment = vec!["Masc", "SuperCharger"];
    equipment.extend(["TripleStrengthMyomer"; 6]);
    for section in [
        BattleSection::CenterTorso,
        BattleSection::LeftTorso,
        BattleSection::RightTorso,
        BattleSection::LeftArm,
        BattleSection::RightArm,
    ] {
        for slot in 0..12 {
            let slots = &mut template.sections.get_mut(&section).unwrap().criticals;
            if slots.contains_key(&slot) {
                continue;
            }
            let Some(name) = equipment.pop() else {
                return template;
            };
            slots.insert(
                slot,
                CriticalDefinition {
                    equipment: name.into(),
                    data: "-".into(),
                    modes: vec![],
                    brand: None,
                },
            );
        }
    }
    assert!(equipment.is_empty());
    template
}

/// Set the last committed thermal sample without advancing movement or booster timers.
fn sample_booster_heat(world: &mut World, id: ObjectId, heat: f64) {
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][id.0.to_string()]["heat"]["excess"] = serde_json::json!(heat);
    world.btech = serde_json::from_value(state).unwrap();
}

#[tokio::test]
async fn hot_myomer_replaces_the_extra_movement_boost_and_preserves_turning() {
    let (_dir, config, world, id) = powered_template(myomer_booster_design()).await;
    // These ceilings follow the independent control and update conversions for the 7/11 chassis.
    for (masc, compressor, control, update) in [
        (false, false, 129.0, 150.5),
        (true, false, 182.75, 193.5),
        (false, true, 182.75, 193.5),
        (true, true, 215.0, 225.75),
    ] {
        let mut trial = world.clone();
        sample_booster_heat(&mut trial, id, 9.0);
        if masc {
            toggle_battle_masc(&mut trial, id, ObjectId(1)).unwrap();
        }
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
        advance_battle_motion(&mut turning, BattleMovementRules::STANDARD).unwrap();
        assert!(
            (turning.btech.constructed_units()[&id]
                .motion()
                .unwrap()
                .heading
                - 1.5 * (control + 16.125) / 10.75)
                .abs()
                < 1e-9
        );
        advance_battle_motion(&mut trial, BattleMovementRules::STANDARD).unwrap();
        assert!(
            (trial.btech.constructed_units()[&id].motion().unwrap().speed - update / 20.0).abs()
                < 1e-9
        );
        trial.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn hot_throttle_toggle_rounding_survives_save_and_cooling() {
    let (_dir, config, world, id) = powered_template(myomer_booster_design()).await;
    for (intermediate, direction) in [
        (false, 1.0),
        (true, 1.0),
        (false, -2.0 / 3.0),
        (true, -2.0 / 3.0),
    ] {
        let mut trial = world.clone();
        sample_booster_heat(&mut trial, id, 9.0);
        if intermediate {
            toggle_battle_masc(&mut trial, id, ObjectId(1)).unwrap();
            set_battle_speed(&mut trial, id, ObjectId(1), 182.75 * direction).unwrap();
        } else {
            set_battle_speed(&mut trial, id, ObjectId(1), 129.0 * direction).unwrap();
            toggle_battle_masc(&mut trial, id, ObjectId(1)).unwrap();
        }
        toggle_battle_supercharger(&mut trial, id, ObjectId(1)).unwrap();
        let desired = trial.btech.constructed_units()[&id]
            .motion()
            .unwrap()
            .desired_speed;
        let expected = if intermediate {
            182.75 * (4.0 / 3.0)
        } else {
            129.0 * (16.0 / 9.0)
        };
        assert!((desired - expected * direction).abs() < 1e-9);
        trial.validate(&config).unwrap();
        persistence::save(&config.database(), &trial).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(trial.btech, restored.btech);
        sample_booster_heat(&mut trial, id, 8.0);
        sample_booster_heat(&mut restored, id, 8.0);
        advance_battle_motion(&mut trial, BattleMovementRules::STANDARD).unwrap();
        advance_battle_motion(&mut restored, BattleMovementRules::STANDARD).unwrap();
        assert_eq!(trial.btech, restored.btech);
        trial.validate(&config).unwrap();
    }
}
