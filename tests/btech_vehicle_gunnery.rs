//! Vehicle gunnery uses class-specific basic skills and shared extended weapon families.
use crate::support;
use stompymux_rs::*;

/// A vehicle with a single weapon family and a present assigned operator.
async fn fixture(weapon: &str) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Gunnery vehicle".into(), Kind::Thing);
    let object = world.objects.get_mut(&id).unwrap();
    object.home = Some(ObjectId(config.home()));
    object.location = Some(ObjectId(config.start()));
    let text = include_str!("../game/mechs/Demolisher.toml")
        .replace("\"IS.AC/20\"", &format!("\"{weapon}\""));
    create_battle_vehicle(
        &mut world,
        id,
        VehicleTemplate::parse("test", &text).unwrap(),
    )
    .unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    (dir, config, world, id)
}

#[tokio::test]
async fn vehicle_gunnery_selects_class_or_weapon_family_with_saved_earned_levels() {
    for (weapon, skill) in [
        ("IS.AC/20", "Gunnery-Ballistic"),
        ("IS.MediumLaser", "Gunnery-Laser"),
        ("IS.SRM-4", "Gunnery-Missile"),
        ("IS.Flamer", "Gunnery-Laser"),
        ("IS.Thumper", "Gunnery-Artillery"),
    ] {
        let (_dir, config, mut world, id) = fixture(weapon).await;
        assert_eq!(battle_unit_gunnery_target(&world, id, 0, true).unwrap(), 6);
        world
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        assert_eq!(battle_unit_gunnery_target(&world, id, 0, true).unwrap(), 18);
        assert_eq!(
            battle_unit_gunnery_target(&world, id, 0, false).unwrap(),
            18
        );
        set_battle_character(
            &mut world,
            ObjectId(1),
            Character {
                build: 5,
                reflexes: 4,
                intuition: 3,
                learn: 2,
                charisma: 1,
                bruise: 0,
                lethal: 0,
            },
        )
        .unwrap();
        for (name, value) in [
            (skill, 4),
            ("Gunnery-Conventional", 6),
            ("Gunnery-Battlemech", 1),
        ] {
            set_battle_character_value(
                &mut world,
                ObjectId(1),
                name,
                CharacterValue {
                    value,
                    experience: 16_777_216 + 20,
                    last_used: 123,
                },
            )
            .unwrap();
        }
        let before = world.btech.clone();
        assert_eq!(battle_unit_gunnery_target(&world, id, 0, true).unwrap(), 6);
        assert_eq!(battle_unit_gunnery_target(&world, id, 0, false).unwrap(), 4);
        assert_eq!(world.btech, before);
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        loaded
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        assert_eq!(battle_unit_gunnery_target(&loaded, id, 0, true).unwrap(), 6);
        assert_eq!(
            battle_unit_gunnery_target(&loaded, id, 0, false).unwrap(),
            4
        );
        set_battle_character_value(
            &mut loaded,
            ObjectId(1),
            skill,
            CharacterValue {
                value: 20,
                ..CharacterValue::default()
            },
        )
        .unwrap();
        assert_eq!(
            battle_unit_gunnery_target(&loaded, id, 0, true).unwrap(),
            -9
        );
    }
}

#[tokio::test]
async fn vehicle_gunnery_fallback_does_not_bypass_weapon_validation_or_claim_a_gunner() {
    let (_dir, _config, mut world, id) = fixture("IS.AC/20").await;
    let before = world.btech.clone();
    assert!(battle_unit_gunnery_target(&world, id, 99, true).is_err());
    assert!(battle_unit_gunnery_target(&world, ObjectId(-1), 0, false).is_err());
    assert_eq!(world.btech, before);
    release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    assert_eq!(battle_unit_gunnery_target(&world, id, 0, true).unwrap(), 6);
    assert_eq!(battle_unit_gunnery_target(&world, id, 0, false).unwrap(), 6);
    assert_eq!(world.btech.vehicles()[&id].pilot(), None);
    assert!(battle_unit_gunnery_target(&world, id, 99, false).is_err());
}
