//! Character clearing restores default stats without interrupting recovery or replacing dice.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Saved XP, use timestamps and health clear; recovery resumes from its existing stream.
#[tokio::test]
async fn clear_restores_defaults_preserves_recovery_and_survives_restart() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let player = ObjectId(2);
    world
        .objects
        .get_mut(&player)
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    set_battle_character(
        &mut world,
        player,
        BattleCharacter {
            build: 4,
            reflexes: 5,
            intuition: 6,
            learn: 7,
            charisma: 8,
            bruise: 35,
            lethal: 2,
        },
    )
    .unwrap();
    for name in ["Acrobatics", "Toughness", "Pain_Resistance", "ShotsHit"] {
        set_battle_character_value(
            &mut world,
            player,
            name,
            BattleCharacterValue {
                value: 1,
                experience: 123,
                last_used: 456,
            },
        )
        .unwrap();
    }
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    let recovery = &mut encoded["recoveries"]["2"];
    recovery["mode"] = serde_json::json!({"kind":"character"});
    recovery["remaining"] = 12.into();
    recovery["pain_resistance"] = true.into();
    recovery["toughness"] = true.into();
    world.btech = serde_json::from_value(encoded).unwrap();
    let before = world.clone();
    assert!(clear_battle_character(&mut world, player, player).is_err());
    assert!(clear_battle_character(&mut world, ObjectId(1), ObjectId(0)).is_err());
    assert_eq!(world.btech, before.btech);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let name = before.objects[&player].name.clone();
    let report = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        &format!("+charclear {name}"),
    );
    assert!(report.contains("Player #2 stats cleared"), "{report}");
    let mut cleared = scripts.world().clone();
    assert_eq!(
        cleared.btech.characters()[&player],
        BattleCharacter {
            build: 1,
            reflexes: 1,
            intuition: 1,
            learn: 1,
            charisma: 1,
            bruise: 0,
            lethal: 0,
        }
    );
    assert!(!cleared.btech.character_values().contains_key(&player));
    let mut expected = before.btech.recoveries()[&player].clone();
    expected.pain_resistance = false;
    expected.toughness = false;
    assert_eq!(cleared.btech.recoveries()[&player], expected);
    assert_eq!(
        cleared.btech.constructed_units(),
        before.btech.constructed_units()
    );
    assert_eq!(cleared.btech.vehicles(), before.btech.vehicles());
    let unchanged = cleared.btech.clone();
    clear_battle_character(&mut cleared, ObjectId(1), player).unwrap();
    assert_eq!(cleared.btech, unchanged);
    persistence::save(&config.database(), &cleared)
        .await
        .unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, cleared.btech);
    let mut dice: BattleDice =
        serde_json::from_value(serde_json::to_value(&expected).unwrap()["dice"].clone()).unwrap();
    let expected_roll = dice.consciousness_roll(false);
    for _ in 0..12 {
        assert_eq!(
            advance_battle_recovery(&mut cleared),
            advance_battle_recovery(&mut restored)
        );
    }
    assert_eq!(restored.btech, cleared.btech);
    assert_eq!(
        cleared.btech.recoveries()[&player].remaining,
        if expected_roll >= 3 { 0 } else { 30 }
    );
    assert_eq!(
        serde_json::to_value(&cleared.btech.recoveries()[&player]).unwrap()["dice"],
        serde_json::to_value(dice).unwrap()
    );
    let state = scripts.world().btech.clone();
    for (command, message) in [
        ("+charclear", "Who do you want to clear the stats from?"),
        ("+charclear no_such_player", "I don't know who that is"),
        ("+charclear #0", "I don't know who that is"),
    ] {
        assert!(support::run_text(&scripts, &config, ObjectId(1), 1, command).contains(message));
    }
    assert!(
        support::run_text(&scripts, &config, player, 2, "+charclear #1")
            .contains("Permission denied")
    );
    assert_eq!(scripts.world().btech, state);
    // A player without a profile gets effective defaults without a new random stream.
    let had_recovery = scripts
        .world()
        .btech
        .recoveries()
        .contains_key(&ObjectId(1));
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "+charclear #1")
            .contains("Player #1 stats cleared")
    );
    assert_eq!(
        scripts
            .world()
            .btech
            .recoveries()
            .contains_key(&ObjectId(1)),
        had_recovery
    );
}

/// Personal resets leave both pilot assignment and existing cockpit injury counters intact.
#[tokio::test]
async fn clear_preserves_mech_and_vehicle_pilot_state() {
    for source in [
        include_str!("../game/mechs/JR7-D.toml"),
        include_str!("../game/mechs/Demolisher.toml"),
    ] {
        let (_dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(source, None, source).await;
        world
            .objects
            .get_mut(&unit)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        set_battle_character(
            &mut world,
            ObjectId(1),
            BattleCharacter {
                build: 3,
                reflexes: 3,
                intuition: 3,
                learn: 3,
                charisma: 3,
                bruise: 0,
                lethal: 0,
            },
        )
        .unwrap();
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        injure_battle_character_pilot(&mut world, unit, 1, false).unwrap();
        let before = world.btech.clone();
        clear_battle_character(&mut world, ObjectId(1), ObjectId(1)).unwrap();
        assert_eq!(world.btech.constructed_units(), before.constructed_units());
        assert_eq!(world.btech.vehicles(), before.vehicles());
        assert_eq!(
            world.btech.recoveries()[&ObjectId(1)].remaining,
            before.recoveries()[&ObjectId(1)].remaining
        );
        world.validate(&config).unwrap();
        // The cleared profile remains usable by the same assigned pilot.
        let injury = injure_battle_character_pilot(&mut world, unit, 1, false).unwrap();
        assert_eq!(injury.injury.bruise_added, 2);
        assert!(!injury.injury.fatal);
    }
}
