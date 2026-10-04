//! Character injury arithmetic, selective persistence and failed-save recovery.
use crate::support;
use sqlx::Connection;
use stompymux_rs::{
    BattleCharacter, ObjectId, injure_battle_character, persistence, set_battle_character,
};

/// Explicit ordinary character profile; no implicit character creation is assumed.
fn profile() -> BattleCharacter {
    BattleCharacter {
        bruise: 0,
        lethal: 0,
        build: 5,
        reflexes: 4,
        intuition: 3,
        learn: 2,
        charisma: 1,
    }
}

#[test]
fn cockpit_injuries_spill_into_lethal_damage_and_report_fatal_overflow() {
    let mut character = profile();
    assert_eq!(character.injure(0).unwrap().bruise_added, 0);
    for (bruise, target) in [(10, 3), (20, 5), (30, 7), (40, 10), (50, 11)] {
        let injury = character.injure(1).unwrap();
        assert_eq!(injury.bruise_added, 10);
        assert!(!injury.fatal);
        assert_eq!(character.bruise, bruise);
        assert_eq!(character.consciousness_target(false).unwrap(), target);
        assert_eq!(character.consciousness_target(true).unwrap(), target - 1);
    }
    let injury = character.injure(2).unwrap();
    assert_eq!(
        (injury.bruise_added, injury.lethal_added, injury.fatal),
        (0, 20, false)
    );
    let injury = character.injure(3).unwrap();
    assert!(injury.fatal);
    assert_eq!((character.bruise, character.lethal), (50, 49));
    assert_eq!(
        (
            character.reflexes,
            character.intuition,
            character.learn,
            character.charisma
        ),
        (4, 3, 2, 1)
    );
}

#[test]
fn unsupported_health_is_rejected_before_mutation_and_large_hits_do_not_overflow() {
    for build in [0, 26, 255] {
        let mut character = BattleCharacter { build, ..profile() };
        let before = character;
        assert!(character.injure(1).is_err());
        assert_eq!(character, before);
    }
    let mut character = BattleCharacter {
        build: 25,
        ..profile()
    };
    assert!(character.injure(255).unwrap().fatal);
    assert_eq!((character.bruise, character.lethal), (250, 249));
    let mut character = BattleCharacter {
        bruise: 51,
        ..profile()
    };
    let before = character;
    assert!(character.injure(1).is_err());
    assert_eq!(character, before);
}

#[tokio::test]
async fn character_health_writes_preserve_skills_extensions_and_retry_failed_saves() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let player = ObjectId(1);
    set_battle_character(&mut world, player, profile()).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut sql = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::raw_sql("ALTER TABLE btech_character_state ADD COLUMN extension TEXT DEFAULT 'retained';
        INSERT INTO btech_character_values VALUES(1,'gunnery',4,27,12345);
        CREATE TRIGGER deny_injury BEFORE UPDATE ON btech_character_state BEGIN SELECT RAISE(ABORT,'injury failure'); END;")
        .execute(&mut sql).await.unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    assert_eq!(world.btech.characters()[&player], profile());
    let before = world.clone();
    let injury = injure_battle_character(&mut world, player, 2).unwrap();
    assert_eq!(injury.bruise_added, 20);
    assert!(persistence::save(&config.database(), &world).await.is_err());
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        before.btech
    );
    world = before;
    sqlx::query("DROP TRIGGER deny_injury")
        .execute(&mut sql)
        .await
        .unwrap();
    assert_eq!(
        injure_battle_character(&mut world, player, 2).unwrap(),
        injury
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    let extension: String =
        sqlx::query_scalar("SELECT extension FROM btech_character_state WHERE player_dbref=1")
            .fetch_one(&mut sql)
            .await
            .unwrap();
    assert_eq!(extension, "retained");
    let skill: (i64,i64,i64) = sqlx::query_as("SELECT value,xp,last_used FROM btech_character_values WHERE player_dbref=1 AND value_name='gunnery'").fetch_one(&mut sql).await.unwrap();
    assert_eq!(skill, (4, 27, 12345));
    let before = world.btech.clone();
    assert!(injure_battle_character(&mut world, ObjectId(99999), 1).is_err());
    assert!(set_battle_character(&mut world, ObjectId(0), profile()).is_err());
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn character_inspection_is_detached_and_purge_removes_owned_state() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let player = world.create(&config, "Test pilot".into(), stompymux_rs::Kind::Player);
    world.objects.get_mut(&player).unwrap().home = Some(ObjectId(config.home()));
    world.objects.get_mut(&player).unwrap().location = Some(ObjectId(config.start()));
    set_battle_character(&mut world, player, profile()).unwrap();
    support::seed_object_dice(&mut world, player, support::FIXTURE_DICE_SEED);
    stompymux_rs::set_battle_character_value(
        &mut world,
        player,
        "Perception",
        stompymux_rs::BattleCharacterValue {
            value: 4,
            ..Default::default()
        },
    )
    .unwrap();
    stompymux_rs::check_character_consciousness(&mut world, player, false, false).unwrap();
    let scripts =
        stompymux_rs::Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world)))
            .unwrap();
    let build: u8 = scripts
        .eval_callback(&format!(
            "local c=btech.character.state({}); c.build=0; return btech.character.state({}).build",
            player.0, player.0
        ))
        .unwrap();
    assert_eq!(build, 5);
    let perception: i16 = scripts.eval_callback(&format!("local c=btech.character.state({}); c.values.Perception.value=99; return btech.character.state({}).perception_target", player.0, player.0)).unwrap();
    assert_eq!(perception, 9);

    assert!(
        scripts
            .eval_callback::<()>("btech.character.state(99999)")
            .is_err()
    );
    let mut world = scripts.world().clone();
    world
        .objects
        .get_mut(&player)
        .unwrap()
        .flags
        .insert(stompymux_rs::Flag::Going);
    persistence::save(&config.database(), &world).await.unwrap();
    persistence::repair(&config.database(), config.database.busy_timeout_ms, |raw| {
        stompymux_rs::dbck::plan(&world, raw, &config)
    })
    .await
    .unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.objects[&player].kind, stompymux_rs::Kind::Garbage);
    assert!(!loaded.btech.characters().contains_key(&player));
    assert!(!loaded.btech.character_values().contains_key(&player));
    assert!(!loaded.btech.recoveries().contains_key(&player));
}

#[test]
fn consciousness_uses_exact_advantage_dice_and_invalid_health_preserves_stream() {
    use stompymux_rs::BattleDice;
    for seed in 0..64 {
        for toughness in [false, true] {
            let mut dice = BattleDice::seeded([seed; 32]);
            let mut expected = dice.clone();
            let first = expected.d6();
            let second = expected.d6();
            let expected_roll = if toughness {
                let third = expected.d6();
                first + second + third - first.min(second).min(third)
            } else {
                first + second
            };
            let character = BattleCharacter {
                bruise: 30,
                ..profile()
            };
            let check = character
                .check_consciousness(&mut dice, true, toughness)
                .unwrap();
            assert_eq!(check.target, 6);
            assert_eq!(check.roll, expected_roll);
            assert_eq!(check.conscious, expected_roll >= 6);
            assert_eq!(dice, expected);
        }
    }
    let invalid = BattleCharacter {
        build: 0,
        ..profile()
    };
    let mut dice = BattleDice::seeded([9; 32]);
    let before = dice.clone();
    assert!(invalid.check_consciousness(&mut dice, false, true).is_err());
    assert_eq!(dice, before);
}

#[test]
fn skill_targets_use_attribute_pairs_and_persisted_experience_levels_without_overflow() {
    use stompymux_rs::{BattleCharacterValue, BattleSkillCategory as Category};
    let value = BattleCharacterValue {
        value: 4,
        experience: 2 * 16_777_216 + 150,
        last_used: 123,
    };
    assert_eq!(value.effective_skill(), 6);
    assert_eq!(value.experience_balance(), 150);
    let character = profile();
    assert_eq!(character.skill_target(Category::Athletic, value), 3);
    assert_eq!(character.skill_target(Category::Physical, value), 5);
    assert_eq!(character.skill_target(Category::Mental, value), 7);
    assert_eq!(character.skill_target(Category::Social, value), 8);
    let mut maximum = character;
    maximum.intuition = 255;
    maximum.learn = 255;
    assert_eq!(
        maximum.skill_target(
            Category::Mental,
            BattleCharacterValue {
                value: 255,
                experience: u32::MAX,
                last_used: 0
            }
        ),
        -1002
    );
}

#[tokio::test]
async fn perception_reads_saved_values_and_value_writes_are_selective_and_transactional() {
    use stompymux_rs::{
        BattleCharacterValue, battle_perception_target, set_battle_character_value,
    };
    let (_dir, config, mut world) = support::isolated_world().await;
    let player = ObjectId(1);
    assert_eq!(battle_perception_target(&world, player).unwrap(), 18);
    set_battle_character(&mut world, player, profile()).unwrap();
    assert_eq!(battle_perception_target(&world, player).unwrap(), 13);
    let skill = BattleCharacterValue {
        value: 4,
        experience: 2 * 16_777_216 + 150,
        last_used: 123,
    };
    set_battle_character_value(&mut world, player, "Perception", skill).unwrap();
    set_battle_character_value(
        &mut world,
        player,
        "Future-Skill",
        BattleCharacterValue { value: 2, ..skill },
    )
    .unwrap();
    assert_eq!(battle_perception_target(&world, player).unwrap(), 7);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut sql = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::raw_sql("ALTER TABLE btech_character_values ADD COLUMN extension TEXT DEFAULT 'retained'; CREATE TRIGGER deny_skill BEFORE UPDATE ON btech_character_values BEGIN SELECT RAISE(ABORT,'skill failure'); END;").execute(&mut sql).await.unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    let before = world.clone();
    set_battle_character_value(
        &mut world,
        player,
        "Perception",
        BattleCharacterValue { value: 5, ..skill },
    )
    .unwrap();
    assert!(persistence::save(&config.database(), &world).await.is_err());
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        before.btech
    );
    sqlx::query("DROP TRIGGER deny_skill")
        .execute(&mut sql)
        .await
        .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    assert_eq!(battle_perception_target(&loaded, player).unwrap(), 6);
    let retained: String = sqlx::query_scalar("SELECT extension FROM btech_character_values WHERE player_dbref=1 AND value_name='Perception'").fetch_one(&mut sql).await.unwrap();
    assert_eq!(retained, "retained");
    assert_eq!(
        loaded.btech.character_values()[&player]["Future-Skill"].value,
        2
    );
    let before = world.btech.clone();
    assert!(set_battle_character_value(&mut world, player, "", skill).is_err());
    assert!(set_battle_character_value(&mut world, ObjectId(0), "Perception", skill).is_err());
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn initial_character_recovery_is_prepared_once_and_first_roll_replays() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let player = ObjectId(1);
    set_battle_character(&mut world, player, profile()).unwrap();
    assert_eq!(
        world.btech.recoveries()[&player].mode,
        stompymux_rs::BattleRecoveryMode::Ready
    );
    let mut corrupt = world.clone();
    let mut encoded = serde_json::to_value(&corrupt.btech).unwrap();
    encoded["recoveries"]["1"]["remaining"] = serde_json::json!(1);
    corrupt.btech = serde_json::from_value(encoded).unwrap();
    assert!(corrupt.validate(&config).is_err());
    let before = world.btech.clone();
    set_battle_character(&mut world, player, profile()).unwrap();
    stompymux_rs::prepare_battle_recovery(&mut world, player).unwrap();
    assert_eq!(world.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let first =
        stompymux_rs::check_character_consciousness(&mut world, player, false, false).unwrap();
    let expected = world.btech.clone();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        stompymux_rs::check_character_consciousness(&mut loaded, player, false, false).unwrap(),
        first
    );
    assert_eq!(loaded.btech, expected);
    let before = world.btech.clone();
    assert!(stompymux_rs::prepare_battle_recovery(&mut world, ObjectId(0)).is_err());
    assert_eq!(world.btech, before);
    // An imported profile without a stream must be prepared before an injury transaction.
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["recoveries"] = serde_json::json!({});
    world.btech = serde_json::from_value(encoded).unwrap();
    let before = world.btech.clone();
    assert!(stompymux_rs::check_character_consciousness(&mut world, player, false, false).is_err());
    assert!(injure_battle_character(&mut world, player, 1).is_err());
    assert_eq!(world.btech, before);
}

/// Earned levels and award intervals persist together and resume identically after restart.
#[tokio::test]
async fn experience_awards_persist_and_reject_without_mutation() {
    use stompymux_rs::*;
    let (_dir, config, mut world) = support::isolated_world().await;
    set_battle_character(&mut world, ObjectId(1), profile()).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Piloting-Biped",
        BattleCharacterValue {
            value: 7,
            ..Default::default()
        },
    )
    .unwrap();
    let rules = BattleExperienceRules {
        category: BattleSkillCategory::Physical,
        threshold: 3000,
        continuous: false,
    };
    let report = award_battle_character_experience(
        &mut world,
        ObjectId(1),
        "Piloting-Biped",
        3001,
        100,
        rules,
        false,
    )
    .unwrap();
    assert!(report.accepted);
    assert_eq!(report.after.effective_skill(), 8);
    assert_eq!(profile().skill_target(rules.category, report.after), 3);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(world.btech, loaded.btech);
    let before = loaded.btech.clone();
    assert!(
        !award_battle_character_experience(
            &mut loaded,
            ObjectId(1),
            "Piloting-Biped",
            9000,
            130,
            rules,
            false
        )
        .unwrap()
        .accepted
    );
    assert_eq!(loaded.btech, before);
    assert_eq!(
        award_battle_character_experience(
            &mut loaded,
            ObjectId(1),
            "Piloting-Biped",
            9000,
            131,
            rules,
            false
        )
        .unwrap(),
        award_battle_character_experience(
            &mut world,
            ObjectId(1),
            "Piloting-Biped",
            9000,
            131,
            rules,
            false
        )
        .unwrap()
    );
    assert_eq!(loaded.btech, world.btech);
    assert_eq!(
        loaded.btech.character_values()[&ObjectId(1)]["Piloting-Biped"].effective_skill(),
        9
    );
    let before = loaded.btech.clone();
    assert!(
        award_battle_character_experience(&mut loaded, ObjectId(1), "", 1, 200, rules, false)
            .is_err()
    );
    assert!(
        award_battle_character_experience(
            &mut loaded,
            ObjectId(999999),
            "Piloting-Biped",
            1,
            200,
            rules,
            false
        )
        .is_err()
    );
    assert_eq!(loaded.btech, before);
    loaded.validate(&config).unwrap();
}

/// Named awards canonicalize aliases, apply catalog timing and survive storage.
#[tokio::test]
async fn catalog_awards_and_detached_lua_skills() {
    use stompymux_rs::*;
    let (_dir, config, mut world) = support::isolated_world().await;
    set_battle_character(&mut world, ObjectId(1), profile()).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    for name in ["PilBip", "piloting-biped", "PILBIP"] {
        assert!(
            award_battle_skill_experience(&mut world, ObjectId(1), name, 1, 100, false)
                .unwrap()
                .accepted
        );
    }
    let values = &world.btech.character_values()[&ObjectId(1)];
    assert_eq!(values.len(), 1);
    assert_eq!(values["Piloting-Biped"].experience_balance(), 3);
    assert!(
        award_battle_skill_experience(&mut world, ObjectId(1), "Drive", 1, 100, false)
            .unwrap()
            .accepted
    );
    let before = world.btech.clone();
    assert!(
        !award_battle_skill_experience(&mut world, ObjectId(1), "drive", 1, 130, false)
            .unwrap()
            .accepted
    );
    for name in ["", "Build", "Melee_Specialist", "unknown"] {
        assert!(
            award_battle_skill_experience(&mut world, ObjectId(1), name, 1, 200, false).is_err()
        );
    }
    assert_eq!(before, world.btech);
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(world.btech, loaded.btech);
    let shared = std::rc::Rc::new(std::cell::RefCell::new(loaded));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    let count: usize = scripts
        .eval_callback(
            r#"
        local skills = btech.character.skills()
        local found = false
        for _, skill in ipairs(skills) do
            if skill.name == 'Piloting-Biped' then
                assert(skill.category == 'physical')
                assert(skill.threshold == 3000 and skill.continuous)
                found = true
            end
        end
        assert(found)
        skills[1].name = 'changed'
        skills[1].threshold = -1
        local fresh = btech.character.skills()
        assert(fresh[1].name == 'Acrobatics' and fresh[1].threshold == 50)
        return #fresh
    "#,
        )
        .unwrap();
    assert_eq!(count, BATTLE_SKILLS.len());
    assert_eq!(before, shared.borrow().btech);
    shared.borrow().validate(&config).unwrap();
}

/// Runtime policy is shared by native and Lua operations, checkpointed, and reset by database reload.
#[tokio::test]
async fn runtime_skill_thresholds_control_awards_and_roll_back() {
    use stompymux_rs::*;
    let (_dir, config, mut world) = support::isolated_world().await;
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    set_battle_character(&mut world, ObjectId(1), profile()).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Piloting-Biped",
        BattleCharacterValue {
            value: 7,
            ..Default::default()
        },
    )
    .unwrap();
    set_battle_skill_threshold(&mut world, ObjectId(1), "PilBip", 1).unwrap();
    assert_eq!(battle_skill_threshold(&world, "PILOTING-BIPED").unwrap(), 1);
    let award =
        award_battle_skill_experience(&mut world, ObjectId(1), "pilbip", 2, 100, false).unwrap();
    assert_eq!(award.after.effective_skill(), 8);
    set_battle_skill_threshold(&mut world, ObjectId(1), "Piloting-Biped", 0).unwrap();
    assert_eq!(
        world.btech.character_values()[&ObjectId(1)]["Piloting-Biped"],
        award.after
    );
    let award =
        award_battle_skill_experience(&mut world, ObjectId(1), "PilBip", 0, 100, false).unwrap();
    assert_eq!(award.after.effective_skill(), 7);
    assert_eq!(award.after.experience_balance(), 2);
    let before = world.btech.clone();
    for (actor, name, value) in [
        (2, "PilBip", 10),
        (1, "Build", 10),
        (1, "PilBip", -1),
        (1, "PilBip", i64::from(i32::MAX) + 1),
    ] {
        assert!(set_battle_skill_threshold(&mut world, ObjectId(actor), name, value).is_err());
        assert_eq!(before, world.btech);
    }
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(battle_skill_threshold(&loaded, "PilBip").unwrap(), 3000);
    assert_eq!(
        loaded.btech.character_values(),
        world.btech.character_values()
    );
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    let outside: bool = scripts
        .inspect_lua()
        .load("return pcall(btech.character.threshold, 'PilBip')")
        .eval()
        .unwrap();
    assert!(!outside);
    let denied = support::run_text(
        &scripts,
        &config,
        ObjectId(2),
        2,
        "@btech skill-threshold PilBip=12",
    );
    assert!(denied.contains("Permission denied"), "{denied}");
    assert_eq!(before, shared.borrow().btech);

    assert!(
        scripts
            .eval_callback::<()>("btech.character.set_threshold(1, 'PilBip', 17); error('abort')")
            .is_err()
    );
    assert_eq!(before, shared.borrow().btech);
    assert!(
        scripts
            .eval_callback::<()>("btech.character.set_threshold(2, 'PilBip', 17)")
            .is_err()
    );
    assert_eq!(before, shared.borrow().btech);
    let threshold: u32 = scripts.eval_callback("assert(btech.character.set_threshold(1, 'PilBip', 17)); return btech.character.threshold('piloting-biped')").unwrap();
    assert_eq!(threshold, 17);
    let report = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        "@btech skill-threshold PilBip=3000",
    );
    assert!(
        report.contains("Piloting-Biped XP threshold set to 3000"),
        "{report}"
    );
    assert_eq!(
        battle_skill_threshold(&shared.borrow(), "PilBip").unwrap(),
        3000
    );
    shared.borrow().validate(&config).unwrap();
}

/// Progress reports use cumulative strict boundaries and never silently recalculate stored levels.
#[tokio::test]
async fn skill_progress_tracks_thresholds_without_mutation() {
    use stompymux_rs::*;
    let (_dir, config, mut world) = support::isolated_world().await;
    set_battle_character(&mut world, ObjectId(1), profile()).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Piloting-Biped",
        BattleCharacterValue {
            value: 7,
            ..Default::default()
        },
    )
    .unwrap();
    for (amount, target, next, remaining) in [
        (3000, 4, 3001, 1),
        (1, 3, 12001, 9000),
        (9000, 2, 39001, 27000),
    ] {
        award_battle_skill_experience(&mut world, ObjectId(1), "PilBip", amount, 100, false)
            .unwrap();
        let before = world.btech.clone();
        let progress = battle_skill_progress(&world, ObjectId(1), "pilbip").unwrap();
        assert_eq!(progress.raw_target, 4);
        assert_eq!(progress.target, target);
        assert_eq!(progress.next_level_balance, Some(next));
        assert_eq!(progress.remaining, Some(remaining));
        assert_eq!(before, world.btech);
    }
    set_battle_skill_threshold(&mut world, ObjectId(1), "PilBip", 1).unwrap();
    let progress = battle_skill_progress(&world, ObjectId(1), "PilBip").unwrap();
    assert_eq!(progress.earned_levels, 2);
    assert_eq!(progress.next_level_balance, Some(14));
    assert_eq!(progress.remaining, Some(0));
    set_battle_skill_threshold(&mut world, ObjectId(1), "PilBip", 0).unwrap();
    assert_eq!(
        battle_skill_progress(&world, ObjectId(1), "PilBip")
            .unwrap()
            .next_level_balance,
        None
    );
    let before = world.btech.clone();
    assert!(battle_skill_progress(&world, ObjectId(999999), "PilBip").is_err());
    assert!(battle_skill_progress(&world, ObjectId(1), "Build").is_err());
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    let target: i16 = scripts.eval_callback("local p=btech.character.progress(1,'PilBip'); assert(p.next_level_balance==nil and p.remaining==nil); p.target=99; return btech.character.progress(1,'Piloting-Biped').target").unwrap();
    assert_eq!(target, 2);
    assert_eq!(before, shared.borrow().btech);
    let raw = BattleCharacterValue {
        value: 255,
        experience: u32::MAX,
        last_used: 0,
    };
    assert_eq!(
        raw.next_level_balance(
            profile(),
            BattleExperienceRules {
                category: BattleSkillCategory::Physical,
                threshold: u32::MAX,
                continuous: true
            }
        ),
        Some(u64::MAX)
    );
}

/// Loss recalculates earned levels across skills without awarding XP or changing use times.
#[tokio::test]
async fn retained_experience_is_atomic_and_persistent() {
    use stompymux_rs::*;
    let (_dir, config, mut world) = support::isolated_world().await;
    set_battle_character(&mut world, ObjectId(1), profile()).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    for name in ["Piloting-Biped", "Gunnery-Ballistic"] {
        set_battle_character_value(
            &mut world,
            ObjectId(1),
            name,
            BattleCharacterValue {
                value: 7,
                experience: 2 * 16_777_216 + 12001,
                last_used: 123,
            },
        )
        .unwrap();
    }
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Melee_Specialist",
        BattleCharacterValue {
            value: 1,
            ..Default::default()
        },
    )
    .unwrap();
    retain_battle_character_experience(&mut world, ObjectId(1), 500).unwrap();
    for name in ["Piloting-Biped", "Gunnery-Ballistic"] {
        let value = world.btech.character_values()[&ObjectId(1)][name];
        assert_eq!(value.value, 7);
        assert_eq!(value.last_used, 123);
        assert_eq!(value.experience_balance(), 6000);
        assert_eq!(value.effective_skill(), 8);
    }
    let mut disabled = world.clone();
    set_battle_skill_threshold(&mut disabled, ObjectId(1), "PilBip", 0).unwrap();
    retain_battle_character_experience(&mut disabled, ObjectId(1), 1000).unwrap();
    let value = disabled.btech.character_values()[&ObjectId(1)]["Piloting-Biped"];
    assert_eq!(value.experience, 6000);
    assert_eq!(value.last_used, 123);
    let before = world.btech.clone();
    retain_battle_character_experience(&mut world, ObjectId(1), 1000).unwrap();
    assert_eq!(before, world.btech);
    assert!(retain_battle_character_experience(&mut world, ObjectId(1), 1001).is_err());
    assert_eq!(before, world.btech);
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Unknown",
        BattleCharacterValue {
            experience: 1,
            ..Default::default()
        },
    )
    .unwrap();
    let before = world.btech.clone();
    retain_battle_character_experience(&mut world, ObjectId(1), 1000).unwrap();
    assert_eq!(before, world.btech);
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Unknown",
        BattleCharacterValue::default(),
    )
    .unwrap();
    retain_battle_character_experience(&mut world, ObjectId(1), 0).unwrap();
    let values = &world.btech.character_values()[&ObjectId(1)];
    assert_eq!(values["Piloting-Biped"].experience, 0);
    assert_eq!(values["Gunnery-Ballistic"].experience, 0);
    assert_eq!(values["Melee_Specialist"].value, 1);
    world.validate(&config).unwrap();
}

/// Standalone threshold grammar preserves silent success and the shared native/Lua policy.
#[tokio::test]
async fn standalone_threshold_edits_share_runtime_controls() {
    use std::{cell::RefCell, rc::Rc};
    use stompymux_rs::*;
    let (_dir, config, world) = support::isolated_world().await;
    let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for value in [0, 1, i32::MAX, 3000] {
        let output = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("setxplevel PilBip {value}"),
        );
        assert!(output.is_empty(), "{output}");
        lua.eval_callback::<bool>(&format!(
            "return btech.character.set_threshold(1,'piloting-biped',{value})"
        ))
        .unwrap();
        assert_eq!(
            battle_skill_threshold(&native.world(), "PilBip").unwrap(),
            value as u32
        );
        assert_eq!(native.world().btech, lua.world().btech);
    }
    let before = native.world().btech.clone();
    for input in [
        "setxplevel",
        "setxplevel PilBip",
        "setxplevel PilBip 1 extra",
        "setxplevel PilBip=1",
        "setxplevel PilBip -1",
        "setxplevel PilBip 2147483648",
        "setxplevel Build 1",
        "setxplevel Unknown 1",
        "setxplevel/nope PilBip 1",
    ] {
        assert!(!support::run_text(&native, &config, ObjectId(1), 1, input).is_empty());
        assert_eq!(native.world().btech, before, "{input}");
    }
    assert!(!support::run_text(&native, &config, ObjectId(4), 4, "setxplevel PilBip 1").is_empty());
    assert_eq!(native.world().btech, before);
}

/// Reduction selects skills, advantages and Lives, preserving unselected data and raw values.
#[tokio::test]
async fn retention_handles_every_advantage_and_signed_experience() {
    use stompymux_rs::*;
    let (_dir, config, mut base) = support::isolated_world().await;
    set_battle_character(&mut base, ObjectId(1), profile()).unwrap();
    support::seed_object_dice(&mut base, ObjectId(1), support::FIXTURE_DICE_SEED);
    for name in BATTLE_ADVANTAGES
        .iter()
        .map(|entry| entry.name)
        .chain(["Lives"])
    {
        set_battle_character_value(
            &mut base,
            ObjectId(1),
            name,
            BattleCharacterValue {
                value: 2,
                experience: 16_777_227,
                last_used: 987,
            },
        )
        .unwrap();
    }
    for name in ["Custom", "Build", "ShotsHit"] {
        set_battle_character_value(
            &mut base,
            ObjectId(1),
            name,
            BattleCharacterValue {
                value: 7,
                experience: 17,
                last_used: 456,
            },
        )
        .unwrap();
    }
    for (retained, balance) in [(0, 0), (333, 3), (500, 5), (999, 10), (1000, 11)] {
        let mut world = base.clone();
        retain_battle_character_experience(&mut world, ObjectId(1), retained).unwrap();
        for name in BATTLE_ADVANTAGES
            .iter()
            .map(|entry| entry.name)
            .chain(["Lives"])
        {
            assert_eq!(
                world.btech.character_values()[&ObjectId(1)][name],
                BattleCharacterValue {
                    value: 2,
                    experience: balance,
                    last_used: 987,
                },
                "{name}"
            );
        }
        for name in ["Custom", "Build", "ShotsHit"] {
            assert_eq!(
                world.btech.character_values()[&ObjectId(1)][name],
                base.btech.character_values()[&ObjectId(1)][name]
            );
        }
    }
    for experience in [0x8000_0000, u32::MAX] {
        let mut world = base.clone();
        for name in BATTLE_SKILLS
            .iter()
            .map(|entry| entry.name)
            .chain(BATTLE_ADVANTAGES.iter().map(|entry| entry.name))
            .chain(["Lives"])
        {
            set_battle_character_value(
                &mut world,
                ObjectId(1),
                name,
                BattleCharacterValue {
                    value: 1,
                    experience,
                    last_used: 987,
                },
            )
            .unwrap();
        }
        retain_battle_character_experience(&mut world, ObjectId(1), 1000).unwrap();
        for name in BATTLE_SKILLS
            .iter()
            .map(|entry| entry.name)
            .chain(BATTLE_ADVANTAGES.iter().map(|entry| entry.name))
            .chain(["Lives"])
        {
            assert_eq!(
                world.btech.character_values()[&ObjectId(1)][name].experience,
                0,
                "{name}"
            );
        }
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}
