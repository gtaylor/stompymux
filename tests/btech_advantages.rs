//! Advantage kinds and shared live interpretation across cockpit injury and movement consequences.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[allow(dead_code)]
#[path = "support/btech_firing.rs"]
mod firing;
mod support;

/// Ordinary character health; three cockpit hits reach the seven-point consciousness threshold.
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

/// Set raw storage deliberately: noncanonical boolean values must remain inspectable but inactive.
fn value(world: &mut World, name: &str, raw: u8) {
    set_battle_character_value(
        world,
        ObjectId(1),
        name,
        BattleCharacterValue {
            value: raw,
            experience: 123,
            last_used: 456,
        },
    )
    .unwrap();
}

/// The typed catalog distinguishes all reference ranks and attribute masks from boolean switches.
#[tokio::test]
async fn advantage_catalog_is_complete_case_insensitive_and_detached_in_lua() {
    assert_eq!(BATTLE_ADVANTAGES.len(), 22);
    for entry in BATTLE_ADVANTAGES {
        assert_eq!(
            battle_advantage_definition(&entry.name.to_ascii_lowercase()),
            Some(entry)
        );
        assert_eq!(
            battle_advantage_definition(&entry.name.to_ascii_uppercase()),
            Some(entry)
        );
    }
    for name in [
        "Contact",
        "Dropship",
        "Extra_Edge",
        "Land_Grant",
        "Title",
        "Wealth",
        "Well-Connected",
        "Well_Equipped",
    ] {
        assert_eq!(
            battle_advantage_definition(name).unwrap().kind,
            BattleAdvantageKind::Ranked
        );
    }
    assert_eq!(
        battle_advantage_definition("Exceptional_Attribute")
            .unwrap()
            .kind,
        BattleAdvantageKind::AttributeMask
    );
    assert_eq!(
        BATTLE_ADVANTAGES
            .iter()
            .filter(|entry| entry.kind == BattleAdvantageKind::Boolean)
            .count(),
        13
    );
    for name in ["Tough", "Lives", "ShotsHit", "Piloting-Biped", " Toughness"] {
        assert!(battle_advantage_definition(name).is_none());
    }
    let (_dir, config, world) = support::isolated_world().await;
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let encoded: String = scripts.eval_callback("local a=btech.character.advantages(); assert(#a==22); a[1].name='changed'; a[1].kind='ranked'; return btech.character.advantages()[1].name..':'..btech.character.advantages()[1].kind").unwrap();
    assert_eq!(encoded, "Ambidextrous:boolean");
    assert_eq!(scripts.world().btech, before);
}

/// Injury uses the same exact-one boolean rule for every supported cockpit family and name casing.
#[tokio::test]
async fn pain_resistance_applies_only_for_one_and_survives_restart() {
    for source in firing::templates() {
        let (_dir, config, mut base, id, _, _) =
            firing::fixture_with_target(&source, None, &firing::templates()[0]).await;
        set_battle_character(&mut base, ObjectId(1), profile()).unwrap();
        prepare_battle_recovery(&mut base, ObjectId(1)).unwrap();
        base.objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        // Keep every raw key present across saves; row deletion is a separate character operation.
        for name in ["Pain_Resistance", "pain_resistance", "PAIN_RESISTANCE"] {
            value(&mut base, name, 0);
        }
        for name in ["Pain_Resistance", "pain_resistance", "PAIN_RESISTANCE"] {
            for raw in [0, 1, 2, 255] {
                let mut world = base.clone();
                value(&mut world, name, raw);
                persistence::save(&config.database(), &world).await.unwrap();
                let mut restored = persistence::load(&config.database()).await.unwrap();
                let report = injure_battle_character_pilot(&mut world, id, 3, false).unwrap();
                let replay = injure_battle_character_pilot(&mut restored, id, 3, false).unwrap();
                assert_eq!(report, replay);
                assert_eq!(world.btech, restored.btech);
                assert_eq!(
                    report.consciousness.unwrap().target,
                    if raw == 1 { 6 } else { 7 }
                );
                assert_eq!(
                    world.btech.character_values()[&ObjectId(1)][name].value,
                    raw
                );
                world.validate(&config).unwrap();
            }
        }
    }
}

/// Moving shutdown must not grant extra saving dice to non-one values or ignore lowercase names.
#[tokio::test]
async fn toughness_shutdown_rules_agree_across_mechs_and_vehicles() {
    let sources = firing::templates();
    for source in [&sources[0], &sources[2]] {
        let (_dir, config, mut base, id, _, _) =
            firing::fixture_with_target(source, None, &sources[0]).await;
        set_battle_character(&mut base, ObjectId(1), profile()).unwrap();
        let seed = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 2)
            .unwrap();
        firing::edit(&mut base, id, |unit| {
            unit["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
            unit["motion"]["speed"] = 43.0.into();
            unit["motion"]["desired_speed"] = 43.0.into();
        });
        let mut outcomes = Vec::new();
        for (name, raw) in [
            ("Toughness", 0),
            ("Toughness", 1),
            ("toughness", 1),
            ("Toughness", 2),
            ("toughness", 255),
        ] {
            let mut world = base.clone();
            value(&mut world, name, raw);
            let notices = stop_battle_unit(
                &mut world,
                id,
                ObjectId(1),
                BattleFallRules::configured(&config),
            )
            .unwrap();
            world.validate(&config).unwrap();
            let mut encoded = serde_json::to_value(&world.btech).unwrap();
            // Only the deliberately varied advantage input differs between equivalent cases.
            encoded["character_values"] = serde_json::json!({});
            outcomes.push((encoded, notices));
        }
        assert_eq!(outcomes[1], outcomes[2]);
        assert_eq!(outcomes[0], outcomes[3]);
        assert_eq!(outcomes[0], outcomes[4]);
        assert!(
            outcomes[0] != outcomes[1],
            "The fixture must exercise an advantage-dependent saving check"
        );
    }
}
