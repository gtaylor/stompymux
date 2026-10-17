//! To-hit conditions of Total Warfare's weapon attacks that depend on the units' bodies and the
//! ground around them: a limbless 'Mech as an immobile target, leg weapons behind the firer's
//! own partial cover, and no attacks on a unit in the firer's own hex.
use crate::btech_motion_common::{fixture_assets, lock_fixture, optical_aim_rules, shot_rules};
use crate::support;
use stompymux_rs::*;

/// A biped that has lost both legs is not yet immobile, but one that has lost both arms too is
/// (Total Warfare p. 123), and so takes the -4 immobile modifier (p. 110).
#[tokio::test]
async fn a_mech_without_legs_or_arms_is_an_immobile_target() {
    let (_dir, _config, mut world, _id, target) = lock_fixture().await;
    world.btech.set_unit_power(target, Power::Running).unwrap();
    let immobile = |world: &World| {
        battle_unit_target_movement(world, target, 1.0)
            .unwrap()
            .immobile
    };
    assert_eq!(immobile(&world), 0);
    for section in [MechSection::LeftLeg, MechSection::RightLeg] {
        apply_damage_phase(&mut world, target, section, 100, DamagePhase::Internal).unwrap();
    }
    assert_eq!(
        immobile(&world),
        0,
        "a legless 'Mech can still prop itself up"
    );
    apply_damage_phase(
        &mut world,
        target,
        MechSection::LeftArm,
        100,
        DamagePhase::Internal,
    )
    .unwrap();
    assert_eq!(immobile(&world), 0);
    apply_damage_phase(
        &mut world,
        target,
        MechSection::RightArm,
        100,
        DamagePhase::Internal,
    )
    .unwrap();
    assert_eq!(immobile(&world), -4);
}

/// A four-legged 'Mech is immobile only once all four of its legs are gone (Total Warfare
/// p. 123).
#[tokio::test]
async fn a_quad_without_legs_is_an_immobile_target() {
    let source = format!("12 12\n{}", format!("{}\n", ".0".repeat(12)).repeat(12));
    let template =
        MechTemplate::parse("SCP-1N", include_str!("../game/units/SCP-1N.toml")).unwrap();
    let (_dir, config, mut world, id) = fixture_assets(&source, template.clone()).await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let target = world.create(&config, "Quad target".into(), Kind::Thing);
    world.objects.get_mut(&target).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(&mut world, target, template).unwrap();
    place_battle_unit(&mut world, target, map, 5, 6).unwrap();
    world.btech.set_unit_power(target, Power::Running).unwrap();
    let legs = [
        MechSection::LeftArm,
        MechSection::RightArm,
        MechSection::LeftLeg,
        MechSection::RightLeg,
    ];
    for (lost, leg) in legs.into_iter().enumerate() {
        assert_eq!(
            battle_unit_target_movement(&world, target, 1.0)
                .unwrap()
                .immobile,
            0,
            "{lost} legs lost"
        );
        apply_damage_phase(&mut world, target, leg, 100, DamagePhase::Internal).unwrap();
    }
    assert_eq!(
        battle_unit_target_movement(&world, target, 1.0)
            .unwrap()
            .immobile,
        -4
    );
}

/// A Jenner whose left-arm medium laser has moved to its right leg.
fn leg_laser_jenner() -> MechTemplate {
    let mut template =
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap();
    let weapon = template
        .sections
        .get_mut(&MechSection::LeftArm)
        .unwrap()
        .criticals
        .remove(&2)
        .unwrap();
    template
        .sections
        .get_mut(&MechSection::RightLeg)
        .unwrap()
        .criticals
        .insert(4, weapon);
    template
}

/// A 'Mech standing behind a one-level rise has partial cover from a target beyond it, so its
/// leg-mounted weapons may not fire through that hex; its other weapons still can (Total
/// Warfare p. 105). Without the rise the leg weapon fires.
#[tokio::test]
async fn leg_weapons_cannot_fire_through_the_firers_own_partial_cover() {
    for rise in [false, true] {
        let source = format!(
            "12 12\n{}",
            (0..12)
                .map(|y| {
                    (0..12)
                        .map(|x| if rise && (x, y) == (5, 4) { ".1" } else { ".0" })
                        .collect::<String>()
                        + "\n"
                })
                .collect::<String>()
        );
        let (_dir, config, mut world, id) = fixture_assets(&source, leg_laser_jenner()).await;
        let map = world.btech.constructed_units()[&id].position().unwrap().map;
        let target = world.create(&config, "Target".into(), Kind::Thing);
        world.objects.get_mut(&target).unwrap().home = Some(ObjectId(config.home()));
        create_battle_unit(
            &mut world,
            target,
            MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap(),
        )
        .unwrap();
        support::seed_object_dice(&mut world, target, support::FIXTURE_DICE_SEED);
        place_battle_unit(&mut world, target, map, 5, 2).unwrap();
        refresh_battle_contacts(&mut world, &[id]).unwrap();
        assert!(!battle_unit_terrain_los(&world, id, target).unwrap().blocked);
        let loadout = world.btech.constructed_units()[&id].loadout().unwrap();
        let leg = loadout
            .weapons
            .iter()
            .position(|mount| mount.criticals[0].section == MechSection::RightLeg)
            .unwrap();
        let arm = loadout
            .weapons
            .iter()
            .position(|mount| mount.criticals[0].section == MechSection::RightArm)
            .unwrap();
        let mut leg_shot = world.clone();
        let result = resolve_battle_shot(&mut leg_shot, id, ObjectId(1), target, leg, shot_rules())
            .map(|_| ())
            .map_err(|error| error.to_string());
        if rise {
            assert_eq!(
                result.unwrap_err(),
                "Your leg-mounted weapons cannot fire through the cover in front of you."
            );
            assert_eq!(leg_shot.btech, world.btech);
        } else {
            result.unwrap();
        }
        let _shot =
            resolve_battle_shot(&mut world, id, ObjectId(1), target, arm, shot_rules()).unwrap();
    }
}

/// No unit may make a weapon attack on another unit in its own hex (Total Warfare p. 106); the
/// shot is refused before anything is spent. A unit next door can be fired on.
#[tokio::test]
async fn units_cannot_fire_on_a_unit_in_their_own_hex() {
    let (_dir, _config, mut world, id, target) = lock_fixture().await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    assert!(
        battle_aim_modifiers(&world, id, target, 0, 4, optical_aim_rules())
            .unwrap()
            .perception
            .is_some()
    );
    place_battle_unit(&mut world, target, map, 5, 5).unwrap();
    refresh_battle_contacts(&mut world, &[id]).unwrap();
    let before = world.btech.clone();
    assert_eq!(
        resolve_battle_shot(&mut world, id, ObjectId(1), target, 0, shot_rules())
            .unwrap_err()
            .to_string(),
        "You cannot fire on a unit in your own hex!"
    );
    assert_eq!(world.btech, before);
}
