//! Automatic mixed-unit acquisition, notification controls and transactional server replay.
use crate::support;
use stompymux_rs::*;

/// Two Mechs and two vehicles at opposite ends of a north/south lane.
async fn fixture(
    tiles: &str,
    vehicle: &str,
) -> (tempfile::TempDir, Config, World, ObjectId, [ObjectId; 4]) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Sight lane".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "sight",
        MapAsset::from_cells(&format!("1 5\n{tiles}")).unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for index in 0..4 {
        let id = world.create(&config, format!("Unit {index}"), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        if index < 2 {
            let mut definition =
                MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml"))
                    .unwrap();
            definition
                .attributes
                .insert("specials".into(), "FlipArms Searchlight".into());
            create_battle_unit(&mut world, id, definition).unwrap();
        } else {
            create_battle_vehicle(
                &mut world,
                id,
                VehicleTemplate::parse("test", vehicle).unwrap(),
            )
            .unwrap();
        }
        place_battle_unit(&mut world, id, map, 0, if index % 2 == 0 { 4 } else { 0 }).unwrap();
        ids.push(id);
    }
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    (dir, config, world, map, ids.try_into().unwrap())
}

/// Assign scenario power without introducing crew actions into sensor tests.
fn power(world: &mut World, ids: &[ObjectId], value: Power) {
    for id in ids {
        world.btech.set_unit_power(*id, value).unwrap();
    }
}

/// A close mixed formation guarantees contact acquisition without a random fixture dependency.
async fn formation() -> (tempfile::TempDir, Config, World, [ObjectId; 4]) {
    let (dir, config, mut world, map, ids) = fixture(
        ".0\n.0\n.0\n.0\n.0\n",
        include_str!("../game/units/Demolisher.toml"),
    )
    .await;
    for id in ids {
        place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    }
    power(&mut world, &ids, Power::Running);
    (dir, config, world, ids)
}

/// Spread the fixture one hex apart so weather can hide every pair; sight always reaches a
/// unit sharing the observer's hex.
async fn spread() -> (tempfile::TempDir, Config, World, ObjectId, [ObjectId; 4]) {
    let (dir, config, mut world, map, ids) = fixture(
        ".0\n.0\n.0\n.0\n.0\n",
        include_str!("../game/units/Demolisher.toml"),
    )
    .await;
    for (y, id) in ids.into_iter().enumerate() {
        place_battle_unit(&mut world, id, map, 0, y as i64).unwrap();
    }
    power(&mut world, &ids, Power::Running);
    (dir, config, world, map, ids)
}

/// Silence the sensor band and drop weather visibility so no unit perceives another.
fn blind_map(world: &mut World, map: ObjectId) {
    set_battle_map_perception(world, map, MapPerceptionFlag::Sensors, false).unwrap();
    set_battle_map_visibility(world, map, Light::Day, 0).unwrap();
}

/// Mixed scans acquire every unit at once, retain contacts, lose them together when no channel
/// reaches, and share brief notification settings.
#[tokio::test]
async fn mixed_automatic_scans_retain_contacts_and_share_brief_notifications() {
    let (_dir, config, mut world, map, ids) = spread().await;
    let [a, _b, c, d] = ids;
    set_battle_unit_signature(
        &mut world,
        d,
        UnitSignature {
            team: 4,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(battle_contact_observers(&world), ids);
    let mut duplicated = world.clone();
    let events = refresh_battle_contacts(&mut world, &ids).unwrap();
    let mut duplicate_ids = ids.to_vec();
    duplicate_ids.extend(ids);
    assert_eq!(
        refresh_battle_contacts(&mut duplicated, &duplicate_ids).unwrap(),
        events
    );
    assert_eq!(duplicated.btech, world.btech);
    assert_eq!(events.len(), 12);
    assert!(
        events
            .iter()
            .all(|e| e.acquired && e.identified && !e.lock_lost)
    );
    assert_eq!(world.btech.vehicles()[&c].contacts().len(), 3);
    assert_eq!(world.btech.constructed_units()[&a].contacts().len(), 3);
    let before = world.btech.clone();
    assert!(
        refresh_battle_contacts(&mut world, &ids)
            .unwrap()
            .is_empty()
    );
    assert_eq!(world.btech, before);
    let mut loss = world.clone();
    blind_map(&mut loss, map);
    let lost = refresh_battle_contacts(&mut loss, &ids).unwrap();
    assert_eq!(lost.len(), 12);
    assert!(lost.iter().all(|event| !event.acquired && event.identified));
    assert!(
        loss.btech
            .vehicles()
            .values()
            .all(|v| v.contacts().is_empty())
    );
    assert!(
        lost.iter()
            .find(|e| e.observer == c && e.target == d)
            .unwrap()
            .notice(&loss)
            .unwrap()
            .text
            .contains("[fg=yellow]")
    );
    assert_eq!(
        roll_unit_dice(&mut loss, c, 3).unwrap(),
        roll_unit_dice(&mut world.clone(), c, 3).unwrap()
    );
    let friendly = events
        .iter()
        .find(|e| e.observer == c && e.target == a)
        .unwrap();
    let hostile = events
        .iter()
        .find(|e| e.observer == c && e.target == d)
        .unwrap();
    let label = world.btech.constructed_units()[&a]
        .battlefield_id()
        .unwrap()
        .to_lowercase();
    assert!(
        friendly
            .notice(&world)
            .unwrap()
            .text
            .contains(&format!("[{label}]"))
    );
    assert!(hostile.notice(&world).unwrap().text.contains("[fg=red]"));
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(c);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "brief A 5")
            .contains("Autocontact brevity set")
    );
    assert!(friendly.notice(&scripts.world()).is_none());
    let notice = hostile.notice(&scripts.world()).unwrap();
    assert!(notice.text.starts_with("Seen:"));
    assert!(!notice.text.contains("[fg="));
    scripts
        .eval_callback::<()>(&format!("btech.unit.brief({},1,'A 6')", c.0))
        .unwrap();
    assert!(hostile.notice(&scripts.world()).is_none());
    let before = scripts.world().clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.brief({},1,'A 0'); error('abort')",
                c.0
            ))
            .is_err()
    );
    assert_eq!(before.btech, scripts.world().btech);
    persistence::save(&config.database(), &before)
        .await
        .unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(before.btech, restored.btech);
    assert_eq!(restored.btech.vehicles()[&c].brief_settings().automatic, 6);
    assert_eq!(
        restored.btech.vehicles()[&c].battlefield_id(),
        Some("AC".into())
    );
}

/// Units still starting up are skipped, then acquire every mixed contact on their first scan.
#[tokio::test]
async fn vehicle_scan_admission_skips_startup_and_acquires_mixed_contacts() {
    let (_dir, config, mut world, [a, b, c, d]) = formation().await;
    power(&mut world, &[c], Power::Starting { remaining: 1 });
    world
        .objects
        .get_mut(&d)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let observers = battle_contact_observers(&world);
    assert_eq!(observers, [a, b, d]);
    advance_battle_units(&mut world, 0);
    refresh_battle_contacts(&mut world, &observers).unwrap();
    assert!(world.btech.vehicles()[&c].contacts().is_empty());
    assert!(battle_contact_observers(&world).contains(&c));
    let observers = battle_contact_observers(&world);
    assert_eq!(observers, [a, b, c, d]);
    refresh_battle_contacts(&mut world, &observers).unwrap();
    let contacts = world.btech.vehicles()[&c].contacts();
    assert_eq!(contacts.len(), 3);
    assert!(contacts.values().all(|contact| contact.identified));
    assert_eq!(world.btech.vehicles()[&d].contacts().len(), 3);
    assert_eq!(world.btech.constructed_units()[&a].contacts().len(), 3);
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][c.0.to_string()]["brief"]["automatic"] = serde_json::json!(7);
    assert!(serde_json::from_value::<BtechState>(saved).is_err());
    persistence::save(&config.database(), &world).await.unwrap();
}

#[tokio::test]
async fn failed_server_saves_retry_the_entire_mixed_contact_update() {
    tokio::task::LocalSet::new().run_until(async {
        use sqlx::Connection;
        let (_dir, config, mut world, ids) = formation().await;
        for id in ids {
            world.objects.get_mut(&id).unwrap().flags.insert(Flag::InCharacter);
        }
        persistence::save(&config.database(), &world).await.unwrap();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::raw_sql("CREATE TRIGGER deny_scanner BEFORE UPDATE ON btech_vehicles BEGIN SELECT RAISE(ABORT,'scanner failure'); END;").execute(&mut sql).await.unwrap();
        let (_address, shutdown, server, _lua, mut heartbeats) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        heartbeats.attempt().await;
        let saved = persistence::load(&config.database()).await.unwrap();
        assert!(saved.btech.vehicles().values().all(|v| v.contacts().is_empty()));
        assert!(saved.btech.constructed_units().values().all(|v| v.contacts().is_empty()));
        sqlx::raw_sql("DROP TRIGGER deny_scanner").execute(&mut sql).await.unwrap();
        let saved = heartbeats.until_saved(&config, 5, |saved| saved.btech.vehicles()[&ids[2]].contacts().len() == 3).await;
        assert!(saved.btech.vehicles().values().all(|v| v.contacts().len() == 3));
        assert!(saved.btech.constructed_units().values().all(|v| v.contacts().len() == 3));
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        server.await.unwrap().unwrap();
    }).await;
}

/// In-character scanners commit the same acquisitions and losses as tactical ones and replay
/// identically after a save.
#[tokio::test]
async fn character_scanners_share_cached_perception_and_durable_contact_transitions() {
    let (_dir, config, mut tactical, map, ids) = spread().await;
    let mut state = serde_json::to_value(&tactical.btech).unwrap();
    for (index, id) in ids.iter().enumerate() {
        let class = if index < 2 { "constructed" } else { "vehicles" };
        state[class][id.0.to_string()]["scanner_perception"] = serde_json::json!(8);
        state[class][id.0.to_string()]["dice"] =
            serde_json::to_value(Dice::seeded([index as u8; 32])).unwrap();
    }
    tactical.btech = serde_json::from_value(state).unwrap();
    let mut character = tactical.clone();
    for id in ids {
        character
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
    }
    assert_eq!(battle_contact_observers(&character), ids);
    let expected = refresh_battle_contacts(&mut tactical, &ids).unwrap();
    let acquired = refresh_battle_contacts(&mut character, &ids).unwrap();
    assert!(!acquired.is_empty());
    assert_eq!(acquired, expected);
    assert_eq!(character.btech, tactical.btech);
    persistence::save(&config.database(), &character)
        .await
        .unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, character.btech);
    assert_eq!(battle_contact_observers(&restored), ids);
    for world in [&mut character, &mut restored] {
        blind_map(world, map);
    }
    let lost = refresh_battle_contacts(&mut character, &ids).unwrap();
    assert_eq!(lost.len(), acquired.len());
    assert!(lost.iter().all(|event| !event.acquired));
    assert_eq!(refresh_battle_contacts(&mut restored, &ids).unwrap(), lost);
    assert_eq!(restored.btech, character.btech);
}

#[tokio::test]
async fn hostile_character_acquisition_shares_perception_awards_and_exact_dice() {
    for vehicle in [false, true] {
        for case in ["disconnected", "award", "gate", "skill", "throttle"] {
            let eligible = case != "disconnected";
            let awarded = case == "award";
            let (_dir, config, mut world, ids) = formation().await;
            let observer = if vehicle { ids[2] } else { ids[0] };
            let target = ids[3];
            for id in [observer, target] {
                world
                    .objects
                    .get_mut(&id)
                    .unwrap()
                    .flags
                    .insert(Flag::InCharacter);
            }
            set_battle_unit_signature(
                &mut world,
                target,
                UnitSignature {
                    team: 1,
                    ..Default::default()
                },
            )
            .unwrap();
            world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(observer);
            if eligible {
                world
                    .objects
                    .get_mut(&ObjectId(1))
                    .unwrap()
                    .flags
                    .insert(Flag::Connected);
            } else {
                world
                    .objects
                    .get_mut(&ObjectId(1))
                    .unwrap()
                    .flags
                    .remove(Flag::Connected);
            }
            assign_battle_pilot(&mut world, observer, ObjectId(1)).unwrap();
            support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
            set_battle_character(
                &mut world,
                ObjectId(1),
                Character {
                    bruise: 0,
                    lethal: 0,
                    build: 5,
                    reflexes: 5,
                    intuition: 5,
                    learn: 5,
                    charisma: 5,
                },
            )
            .unwrap();
            support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
            if case == "throttle" {
                set_battle_character_value(
                    &mut world,
                    ObjectId(1),
                    "Perception",
                    CharacterValue {
                        last_used: i64::MAX,
                        ..Default::default()
                    },
                )
                .unwrap();
            }
            let seed = (0..=255)
                .find(|seed| {
                    let mut dice = Dice::seeded([*seed; 32]);
                    let gate = dice.die(6).unwrap();
                    if case == "gate" {
                        return gate != 1;
                    }
                    gate == 1 && (dice.two_d6() >= 4) == (case != "skill")
                })
                .unwrap();
            let mut expected = Dice::seeded([seed; 32]);
            assert_eq!(expected.die(6).unwrap() == 1, case != "gate");
            if eligible && case != "gate" {
                assert_eq!(expected.two_d6() >= 4, case != "skill");
            }
            let mut state = serde_json::to_value(&world.btech).unwrap();
            let class = if vehicle { "vehicles" } else { "constructed" };
            state[class][observer.0.to_string()]["dice"] =
                serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
            state[class][observer.0.to_string()]["scanner_perception"] = serde_json::json!(6);
            world.btech = serde_json::from_value(state).unwrap();
            let events = refresh_battle_contacts(&mut world, &[observer]).unwrap();
            assert_eq!(events.len(), 3);
            let messages: Vec<_> = events
                .iter()
                .filter_map(|e| e.experience_message.as_ref())
                .collect();
            assert_eq!(messages.len(), usize::from(awarded), "{case}");
            if awarded {
                assert_eq!(messages[0].channel, DiagnosticChannel::Experience);
                assert!(messages[0].text.contains("gained 1 perception XP"));
                assert_eq!(
                    world.btech.character_values()[&ObjectId(1)]["Perception"].experience_balance(),
                    1
                );
            }
            let balance = world
                .btech
                .character_values()
                .get(&ObjectId(1))
                .and_then(|values| values.get("Perception"))
                .map_or(0, |value| value.experience_balance());
            assert_eq!(balance, u32::from(awarded), "{case}");
            let state = serde_json::to_value(&world.btech).unwrap();
            assert_eq!(
                state[class][observer.0.to_string()]["dice"],
                serde_json::to_value(expected).unwrap()
            );
            let before = world.btech.clone();
            assert!(
                refresh_battle_contacts(&mut world, &[observer])
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(world.btech, before);
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
        }
    }
}
