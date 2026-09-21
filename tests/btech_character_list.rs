//! Character list queries share catalogs and preserve the reference's skill-only filtering.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Queries preserve raw values, XP, recovery and detached catalog order across restart.
#[tokio::test]
async fn character_lists_filter_skills_only_and_remain_read_only() {
    let (_dir, config, mut world) = support::isolated_world().await;
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
    for (name, value, experience, last_used) in [
        ("Acrobatics", 1, 0, 0),
        ("Administration", 0, 1, 0),
        ("Alternate_Identity", 0, 1 << 24, 0),
        ("Appraisal", 0, 0, 123),
        ("Ambidextrous", 0, 0, 0),
    ] {
        set_battle_character_value(
            &mut world,
            ObjectId(1),
            name,
            BattleCharacterValue {
                value,
                experience,
                last_used,
            },
        )
        .unwrap();
    }
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let names: Vec<String> = scripts
        .eval_callback("return btech.character.list('skills')")
        .unwrap();
    assert_eq!(
        names,
        BATTLE_SKILLS
            .iter()
            .map(|skill| skill.name)
            .collect::<Vec<_>>()
    );
    let learned: Vec<String> = scripts
        .eval_callback("return btech.character.list('SKILLS', 1)")
        .unwrap();
    assert_eq!(
        learned,
        ["Acrobatics", "Administration", "Alternate_Identity"]
    );
    for target in ["#1", world.objects[&ObjectId(1)].name.as_str()] {
        let result: Vec<String> = scripts
            .eval_callback(&format!(
                "return btech.character.list('skills', {target:?})"
            ))
            .unwrap();
        assert_eq!(result, learned);
    }
    let missing: Vec<String> = scripts
        .eval_callback("return btech.character.list('skills', 2)")
        .unwrap();
    assert!(missing.is_empty());
    for player in ["", ", 1", ", 2"] {
        let advantages: Vec<String> = scripts
            .eval_callback(&format!(
                "return btech.character.list('advantages'{player})"
            ))
            .unwrap();
        assert_eq!(
            advantages,
            BATTLE_ADVANTAGES
                .iter()
                .map(|value| value.name)
                .collect::<Vec<_>>()
        );
        let attributes: Vec<String> = scripts
            .eval_callback(&format!(
                "return btech.character.list('ATTRIBUTES'{player})"
            ))
            .unwrap();
        assert_eq!(
            attributes,
            ["Build", "Reflexes", "Intuition", "Learn", "Charisma"]
        );
    }
    let unchanged: String = scripts.eval_callback("local names=btech.character.list('skills'); names[1]='changed'; return btech.character.list('skills')[1]").unwrap();
    assert_eq!(unchanged, "Acrobatics");
    for args in [
        "",
        "'a'",
        "'s'",
        "'adv'",
        "'char_skills'",
        "' skills '",
        "''",
        "'values'",
        "'skills', nil",
        "'skills', 999999",
        "'skills', 0",
        "'skills', 1, 2",
    ] {
        assert!(
            scripts
                .eval_callback::<()>(&format!("btech.character.list({args})"))
                .is_err(),
            "{args}"
        );
    }
    assert!(scripts.world().btech == world.btech);
    assert!(scripts.drain_outbox().is_empty());
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    let restarted = Scripts::new(&config, Rc::new(RefCell::new(restored))).unwrap();
    let result: Vec<String> = restarted
        .eval_callback("return btech.character.list('skills', 1)")
        .unwrap();
    assert_eq!(result, learned);
}
