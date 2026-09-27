//! Conventional VTOL cockpit gunnery uses the aerospace skill family.
use crate::support::btech_firing as firing;
use stompymux_rs::*;

/// Distinct attribute and skill values make the selected skill family observable.
fn profile(world: &mut World, player: ObjectId) {
    set_battle_character(
        world,
        player,
        BattleCharacter {
            bruise: 0,
            lethal: 0,
            build: 3,
            reflexes: 4,
            intuition: 3,
            learn: 2,
            charisma: 1,
        },
    )
    .unwrap();
    for (index, skill) in [
        "Gunnery-Battlemech",
        "Gunnery-Conventional",
        "Gunnery-Aerospace",
        "Gunnery-Laser",
        "Gunnery-Missile",
        "Gunnery-Ballistic",
        "Gunnery-Artillery",
    ]
    .into_iter()
    .enumerate()
    {
        set_battle_character_value(
            world,
            player,
            skill,
            BattleCharacterValue {
                value: index as u8,
                experience: if index == 3 { 16_777_216 } else { 0 },
                last_used: 123,
            },
        )
        .unwrap();
    }
    world
        .objects
        .get_mut(&player)
        .unwrap()
        .flags
        .insert(Flag::Connected);
}

#[tokio::test]
async fn conventional_vtol_cockpit_uses_aerospace_skill_and_preserves_piloting() {
    let template = &firing::templates()[6];
    let (_dir, config, mut world, parent, _, index) =
        firing::fixture_with_target(template, Some(BattleWeapon::MediumLaser), template).await;
    profile(&mut world, ObjectId(1));
    let before = world.btech.clone();
    assert_eq!(
        battle_unit_gunnery_target(&world, parent, index, false).unwrap(),
        9
    );
    assert_eq!(
        battle_unit_gunnery_target(&world, parent, index, true).unwrap(),
        7
    );
    assert_eq!(unit_artillery_gunnery_target(&world, parent).unwrap(), 5);
    let piloting = battle_unit_piloting_target(&world, parent, false).unwrap();
    assert_eq!(world.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    loaded
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    assert_eq!(
        battle_unit_gunnery_target(&loaded, parent, index, false).unwrap(),
        9
    );
    assert_eq!(
        battle_unit_piloting_target(&loaded, parent, false).unwrap(),
        piloting
    );
}

#[tokio::test]
async fn conventional_vtol_experience_uses_the_same_aerospace_skill() {
    let template = &firing::templates()[6];
    let (_dir, _config, mut world, parent, target, index) =
        firing::fixture_with_target(template, Some(BattleWeapon::MediumLaser), template).await;
    profile(&mut world, ObjectId(1));
    for (unit, team) in [(parent, 1), (target, 2)] {
        world
            .objects
            .get_mut(&unit)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        set_battle_unit_signature(
            &mut world,
            unit,
            BattleUnitSignature {
                team,
                hidden: false,
                illuminated: false,
            },
        )
        .unwrap();
    }
    assert_eq!(
        battle_unit_gunnery_target(&world, parent, index, false).unwrap(),
        9
    );
    let award = award_battle_classic_gunnery_experience(
        &mut world,
        BattleGunneryAwardRequest {
            tsm_tow_bonus: false,

            attacker: parent,
            pilot: ObjectId(1),
            target,
            weapon: BattleWeapon::MediumLaser,
            damage: 1000,
            base_to_hit: 7,
            extended_gunnery: false,
            extended_piloting: false,
            use_unit_modifier: false,
            now: 1000,
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(award.skill, "Gunnery-Aerospace");
    assert!(award.amount.is_some_and(|amount| amount > 0));
    let skills = &world.btech.character_values()[&ObjectId(1)];
    assert!(skills["Gunnery-Aerospace"].experience > 0);
    assert_eq!(skills["Gunnery-Conventional"].experience, 0);
}
