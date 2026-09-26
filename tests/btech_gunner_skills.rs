//! Explicit gunner skills share chassis and weapon-family arithmetic without pilot substitution.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod firing;

/// Distinct attribute and skill values make accidental parent-operator lookup observable.
fn profile(world: &mut World, player: ObjectId, gunner: bool) {
    set_battle_character(
        world,
        player,
        BattleCharacter {
            bruise: 0,
            lethal: 0,
            build: 3,
            reflexes: if gunner { 4 } else { 1 },
            intuition: if gunner { 3 } else { 1 },
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
                value: if gunner { index as u8 } else { 0 },
                experience: if gunner && index == 3 { 16_777_216 } else { 0 },
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
async fn explicit_operator_skills_cover_all_chassis_and_weapon_families() {
    for (chassis, template) in firing::templates().iter().enumerate() {
        for (weapon, extended_target) in [
            (BattleWeapon::MediumLaser, 7),
            (BattleWeapon::Lrm5, 7),
            (BattleWeapon::Ac5, 6),
        ] {
            let (_dir, config, mut world, parent, _, index) =
                firing::fixture_with_target(template, Some(weapon), template).await;
            let station = world.create(&config, "Station".into(), Kind::Thing);
            let gunner = world.create(&config, "Gunner".into(), Kind::Player);
            register_gunner_station(&mut world, ObjectId(1), station, parent, 5).unwrap();
            world.objects.get_mut(&gunner).unwrap().location = Some(station);
            profile(&mut world, ObjectId(1), false);
            profile(&mut world, gunner, true);
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            gunner_station_action(&scripts, station, gunner, true).unwrap();
            scripts.drain_outbox();
            let context = gunner_context(&scripts.world(), station, gunner).unwrap();
            let before = scripts.world().clone();
            let conventional_target = if chassis == 6 {
                9
            } else if chassis >= 2 {
                10
            } else {
                11
            };
            assert_eq!(
                context.gunnery_target(&before, index, false).unwrap(),
                conventional_target
            );
            assert_eq!(
                context.gunnery_target(&before, index, true).unwrap(),
                extended_target
            );
            assert_eq!(context.artillery_gunnery_target(&before).unwrap(), 5);
            assert_eq!(
                battle_unit_gunnery_target(&before, parent, index, false).unwrap(),
                16
            );
            assert_eq!(
                battle_unit_gunnery_target(&before, parent, index, true).unwrap(),
                16
            );
            let result: i16 = scripts
                .eval_callback(&format!(
                    "return btech.gunner.gunnery({},{},{index})",
                    station.0, gunner.0
                ))
                .unwrap();
            assert_eq!(
                result,
                if config.battletech.extended_gunnery != 0 {
                    extended_target
                } else {
                    conventional_target
                }
            );
            assert_eq!(
                scripts
                    .eval_callback::<i16>(&format!(
                        "return btech.gunner.artillery_gunnery({},{})",
                        station.0, gunner.0
                    ))
                    .unwrap(),
                5
            );
            assert_eq!(scripts.world().btech, before.btech);
            assert!(scripts.drain_outbox().is_empty());
            let mut disconnected = before.clone();
            disconnected
                .objects
                .get_mut(&ObjectId(1))
                .unwrap()
                .flags
                .remove(Flag::Connected);
            assert_eq!(
                context.gunnery_target(&disconnected, index, true).unwrap(),
                extended_target
            );
            assert_eq!(
                battle_unit_gunnery_target(&disconnected, parent, index, true).unwrap(),
                6
            );
            disconnected
                .objects
                .get_mut(&gunner)
                .unwrap()
                .flags
                .remove(Flag::Connected);
            assert_eq!(
                context.gunnery_target(&disconnected, index, true).unwrap(),
                6
            );
            assert_eq!(context.artillery_gunnery_target(&disconnected).unwrap(), 8);
            assert!(context.gunnery_target(&before, usize::MAX, true).is_err());
            let forged = BattleGunnerContext { arcs: 0, ..context };
            assert!(forged.gunnery_target(&before, index, true).is_err());
            gunner_station_action(&scripts, station, gunner, false).unwrap();
            assert!(
                context
                    .gunnery_target(&scripts.world(), index, true)
                    .is_err()
            );
            assert!(context.artillery_gunnery_target(&scripts.world()).is_err());
        }
    }
}

#[tokio::test]
async fn conventional_vtol_cockpit_uses_aerospace_skill_and_preserves_piloting() {
    let template = &firing::templates()[6];
    let (_dir, config, mut world, parent, _, index) =
        firing::fixture_with_target(template, Some(BattleWeapon::MediumLaser), template).await;
    profile(&mut world, ObjectId(1), true);
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
    profile(&mut world, ObjectId(1), true);
    for (unit, team) in [(parent, 1), (target, 2)] {
        world
            .objects
            .get_mut(&unit)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        set_battle_sensor_signature(
            &mut world,
            unit,
            BattleSensorSignature {
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
