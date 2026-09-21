//! Mech vacuum damage shares exposure effects, random streams and casualty transactions.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod firing;

/// A located hit with no additional hit-table effects.
fn hit(section: BattleSection) -> BattleHit {
    BattleHit {
        section,
        rear_armor: false,
        through_armor_critical: false,
        crew_stun: false,
    }
}

/// Seed a stream whose first material critical roll causes no independent damage.
fn seed(world: &mut World, id: ObjectId, wanted: u8) -> BattleDice {
    let dice = (0..=255)
        .map(|value| BattleDice::seeded([value; 32]))
        .find(|dice| {
            let mut sample = dice.clone();
            sample.two_d6();
            sample.two_d6() == wanted
        })
        .unwrap();
    firing::edit(world, id, |state| {
        state["dice"] = serde_json::to_value(&dice).unwrap()
    });
    dice
}

/// Change only the map environment through the operator implementation.
fn environment(world: &mut World, id: ObjectId, gravity: u8, vacuum: bool) {
    let map = world.btech.units()[&id].map.unwrap();
    set_battle_map_environment(
        world,
        ObjectId(1),
        map,
        BattleMapEnvironment {
            gravity,
            temperature: 20,
            vacuum,
            underground: false,
        },
    )
    .unwrap();
}

/// Validate decoded scenario state through the same world boundary used by persistence.
fn invalid_state_error(world: &World, config: &Config, state: serde_json::Value) -> String {
    let mut invalid = world.clone();
    invalid.btech = serde_json::from_value(state).unwrap();
    invalid.validate(config).unwrap_err().to_string()
}

/// Penetration disables installed weapons and empties exposed Mech bins without destroying slots.
#[tokio::test]
async fn penetration_shares_mech_equipment_effects_and_restart() {
    for template in firing::templates().into_iter().take(2) {
        let (_dir, config, mut world, id, _, index) = firing::fixture_with_supply(
            &template,
            Some(BattleWeapon::Lrm5),
            include_str!("../game/mechs/AS7-D"),
            false,
            Some(""),
        )
        .await;
        let section = BattleSection::LeftTorso;
        let before = world.btech.constructed_units()[&id].sections()[&section].clone();
        let mount = world.btech.constructed_units()[&id]
            .loadout()
            .unwrap()
            .weapons[index]
            .clone();
        environment(&mut world, id, 100, true);
        seed(&mut world, id, 4);
        let report = resolve_battle_impact(&mut world, id, hit(section), before.armor + 1).unwrap();
        assert_eq!(report.exposures.len(), 1);
        assert_eq!(report.exposures[0].cause, BattleSectionExposure::Vacuum);
        let unit = &world.btech.constructed_units()[&id];
        assert_eq!(unit.sections()[&section].internal, before.internal - 1);
        assert!(unit.breached_sections().contains(&section));
        assert!(!unit.flooded_sections().contains(&section));
        assert!(!unit.critical_destroyed(mount.criticals[0]));
        assert!(unit.critical_unavailable(mount.criticals[0]));
        assert!(!unit.weapon_readiness(index).unwrap().ready);
        for (bin, rounds) in unit
            .loadout()
            .unwrap()
            .ammunition
            .iter()
            .zip(unit.ammunition())
        {
            if bin.location.section == section {
                assert_eq!(*rounds, 0);
            }
        }
        assert_eq!(
            battle_weapon_diagnostics(&world, id).unwrap()[index].condition,
            BattleEquipmentCondition::Disabled
        );
        environment(&mut world, id, 100, false);
        let mut invalid = serde_json::to_value(&world.btech).unwrap();
        invalid["constructed"][id.0.to_string()]["flooded_sections"] = serde_json::json!([section]);
        assert!(
            invalid_state_error(&world, &config, invalid).contains("Invalid breached Mech section")
        );
        let bin = world.btech.constructed_units()[&id]
            .loadout()
            .unwrap()
            .ammunition
            .iter()
            .position(|bin| bin.location.section == section)
            .unwrap();
        let mut invalid = serde_json::to_value(&world.btech).unwrap();
        invalid["constructed"][id.0.to_string()]["ammunition"][bin] = 1.into();
        assert!(
            invalid_state_error(&world, &config, invalid)
                .contains("Breached Mech section retains ammunition")
        );
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

/// Armor checks consume exactly one extra roll under special conditions, including dry maps.
#[tokio::test]
async fn armor_checks_follow_shared_threshold_dice_and_repeat_policy() {
    for template in firing::templates().into_iter().take(2) {
        let (_dir, config, initial, id, _, _) = firing::fixture_with_target(
            &template,
            Some(BattleWeapon::MediumLaser),
            include_str!("../game/mechs/AS7-D"),
        )
        .await;
        for wanted in [9, 10, 12] {
            for (gravity, vacuum) in [(100, false), (50, false), (100, true)] {
                let mut world = initial.clone();
                environment(&mut world, id, gravity, vacuum);
                let mut dice = seed(&mut world, id, wanted);
                dice.two_d6();
                if vacuum || gravity != 100 {
                    dice.two_d6();
                }
                let report =
                    resolve_battle_impact(&mut world, id, hit(BattleSection::LeftArm), 1).unwrap();
                assert_eq!(report.exposures.len(), usize::from(vacuum && wanted >= 10));
                assert_eq!(
                    serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"],
                    serde_json::to_value(dice).unwrap()
                );
                if vacuum && wanted >= 10 {
                    seed(&mut world, id, wanted);
                    let report =
                        resolve_battle_impact(&mut world, id, hit(BattleSection::LeftArm), 1)
                            .unwrap();
                    assert!(report.exposures.is_empty());
                }
                world.validate(&config).unwrap();
            }
        }
    }
}

/// Both Mech anatomies lose support when a surviving leg is exposed, with immediate saved fall effects.
#[tokio::test]
async fn exposed_legs_fall_without_losing_structure() {
    for template in firing::templates().into_iter().take(2) {
        let (_dir, config, mut world, id, _, _) = firing::fixture_with_target(
            &template,
            Some(BattleWeapon::MediumLaser),
            include_str!("../game/mechs/AS7-D"),
        )
        .await;
        let section = if world.btech.constructed_units()[&id].chassis() == BattleMechChassis::Quad {
            BattleSection::LeftArm
        } else {
            BattleSection::LeftLeg
        };
        let armor = world.btech.constructed_units()[&id].sections()[&section].armor;
        environment(&mut world, id, 100, true);
        seed(&mut world, id, 4);
        let report = resolve_battle_tactical_impact(
            &mut world,
            id,
            hit(section),
            armor + 1,
            BattleMovementRules::STANDARD.fall,
        )
        .unwrap();
        assert!(
            report
                .impact
                .exposures
                .iter()
                .any(|report| report.section == section && report.fall.is_some())
        );
        assert_eq!(
            world.btech.constructed_units()[&id].posture(),
            BattlePosture::Prone
        );
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

/// Head exposure evacuates IC occupants atomically, including failure after damage and injury.
#[tokio::test]
async fn cockpit_exposure_rolls_back_failed_evacuation_and_survives_restart() {
    let (_dir, config, mut world, id, _, _) = firing::fixture_with_target(
        include_str!("../game/mechs/JR7-D"),
        Some(BattleWeapon::MediumLaser),
        include_str!("../game/mechs/AS7-D"),
    )
    .await;
    release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let pilot = ObjectId(2);
    world
        .objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    world.objects.get_mut(&pilot).unwrap().location = Some(id);
    set_battle_character(
        &mut world,
        pilot,
        BattleCharacter {
            build: 5,
            reflexes: 5,
            intuition: 5,
            learn: 5,
            charisma: 5,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    assign_battle_pilot(&mut world, id, pilot).unwrap();
    environment(&mut world, id, 100, true);
    let mut mechanical = world.clone();
    seed(&mut mechanical, id, 10);
    let before_mechanical = mechanical.btech.clone();
    assert!(resolve_battle_impact(&mut mechanical, id, hit(BattleSection::LeftArm), 1).is_err());
    assert_eq!(mechanical.btech, before_mechanical);
    // An armored head checks vacuum after its ordinary nonlethal pilot injury.
    let chosen = (0..=255)
        .find(|value| {
            let mut candidate = world.clone();
            firing::edit(&mut candidate, id, |state| {
                state["dice"] = serde_json::to_value(BattleDice::seeded([*value; 32])).unwrap()
            });
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(candidate))).unwrap();
            resolve_battle_impact_action(&scripts, &config, id, hit(BattleSection::Head), 1)
                .is_ok_and(|report| {
                    report.crew_casualty() == Some(BattleCrewCasualty::VacuumExposure)
                })
        })
        .unwrap();
    firing::edit(&mut world, id, |state| {
        state["dice"] = serde_json::to_value(BattleDice::seeded([chosen; 32])).unwrap()
    });
    let before = world.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let afterlife = ObjectId(config.battletech.afterlife_dbref);
    scripts.world_mut().objects.remove(&afterlife);
    let invalid = scripts.world().clone();
    assert!(
        resolve_battle_impact_action(&scripts, &config, id, hit(BattleSection::Head), 1).is_err()
    );
    assert_eq!(scripts.world().btech, invalid.btech);
    assert_eq!(scripts.world().objects[&pilot].location, Some(id));
    *scripts.world_mut() = before;
    let report =
        resolve_battle_impact_action(&scripts, &config, id, hit(BattleSection::Head), 1).unwrap();
    assert_eq!(
        report.crew_casualty(),
        Some(BattleCrewCasualty::VacuumExposure)
    );
    assert_eq!(scripts.world().objects[&pilot].location, Some(afterlife));
    assert!(
        scripts.world().btech.constructed_units()[&id].sections()[&BattleSection::Head].internal
            > 0
    );
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
}

/// Losing the remaining jets during a jump completes a fall in the enclosing impact.
#[tokio::test]
async fn vacuum_loss_of_last_jets_interrupts_flight() {
    let (_dir, config, mut world, id, _, _) = firing::fixture_with_target(
        include_str!("../game/mechs/JR7-D"),
        None,
        include_str!("../game/mechs/AS7-D"),
    )
    .await;
    let section = BattleSection::RightTorso;
    let other_jets: Vec<_> = world.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .systems
        .iter()
        .filter(|part| {
            part.system == BattleSystem::JumpJet
                && (part.location.section != section || part.location.slot == 1)
        })
        .map(|part| part.location)
        .collect();
    for location in other_jets {
        destroy_battle_critical(&mut world, id, location).unwrap();
    }
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 1.0).unwrap();
    advance_battle_jumps(&mut world, BattleMovementRules::STANDARD).unwrap();
    assert!(world.btech.constructed_units()[&id].flight().is_some());
    environment(&mut world, id, 100, true);
    seed(&mut world, id, 4);
    let armor = world.btech.constructed_units()[&id].sections()[&section].armor;
    let report = resolve_battle_tactical_impact(
        &mut world,
        id,
        hit(section),
        armor + 1,
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    assert!(
        report
            .impact
            .exposures
            .iter()
            .any(|exposure| exposure.section == section)
    );
    assert!(report.balance.iter().any(|balance| balance.fall.is_some()));
    assert!(report.balance.iter().any(|balance| matches!(balance.cause, BattleBalanceCause::Critical { location, system: BattleSystem::JumpJet } if location.section==section && location.slot==2)));
    assert!(world.btech.constructed_units()[&id].flight().is_none());
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

/// A small internal explosion checks vacuum even while armor remains intact.
#[tokio::test]
async fn internal_explosion_can_breach_surviving_armored_section() {
    let (_dir, config, mut initial, id, _, _) = firing::fixture_with_supply(
        include_str!("../game/mechs/JR7-D"),
        Some(BattleWeapon::Ac2),
        include_str!("../game/mechs/AS7-D"),
        false,
        Some(""),
    )
    .await;
    let bin = initial.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .ammunition
        .iter()
        .position(|bin| bin.location.section == BattleSection::LeftTorso)
        .unwrap();
    firing::edit(&mut initial, id, |state| {
        state["ammunition"][bin] = 1.into()
    });
    environment(&mut initial, id, 100, true);
    let mut found = false;
    for value in 0..=255 {
        let mut world = initial.clone();
        firing::edit(&mut world, id, |state| {
            state["dice"] = serde_json::to_value(BattleDice::seeded([value; 32])).unwrap()
        });
        let report =
            explode_battle_ammunition(&mut world, id, bin, BattleMovementRules::STANDARD.fall)
                .unwrap();
        if report
            .impact
            .exposures
            .iter()
            .any(|exposure| exposure.section == BattleSection::LeftTorso)
        {
            assert!(
                world.btech.constructed_units()[&id].sections()[&BattleSection::LeftTorso].armor
                    > 0
            );
            world.validate(&config).unwrap();
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
            found = true;
            break;
        }
    }
    assert!(
        found,
        "No surviving internal explosion breached its section"
    );
}

/// Engine exposure shares configured reactor explosions while retaining structure when blasts are disabled.
#[tokio::test]
async fn core_exposure_shares_engine_and_reactor_consequences() {
    let (_dir, config, initial, id, _, _) = firing::fixture_with_target(
        include_str!("../game/mechs/JR7-D"),
        None,
        include_str!("../game/mechs/AS7-D"),
    )
    .await;
    for enabled in [false, true] {
        let mut world = initial.clone();
        configure_battle_reactor_policy(&mut world, enabled, false);
        environment(&mut world, id, 100, true);
        seed(&mut world, id, 4);
        let before =
            world.btech.constructed_units()[&id].sections()[&BattleSection::CenterTorso].clone();
        let report = resolve_battle_tactical_impact(
            &mut world,
            id,
            hit(BattleSection::CenterTorso),
            before.armor + 1,
            BattleMovementRules::STANDARD.fall,
        )
        .unwrap();
        assert!(report.impact.destroyed);
        let exposure = report
            .impact
            .exposures
            .iter()
            .find(|exposure| exposure.section == BattleSection::CenterTorso)
            .unwrap();
        assert_eq!(exposure.reactor_explosion.is_some(), enabled);
        let unit = &world.btech.constructed_units()[&id];
        assert_eq!(
            unit.sections()[&BattleSection::CenterTorso].internal,
            if enabled { 0 } else { before.internal - 1 }
        );
        assert_eq!(unit.power(), BattlePower::Off);
        assert!(unit.system_hits(BattleSystem::Engine) >= 3);
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        // Reactor policy is host configuration, while the consequences are durable.
        assert_eq!(
            loaded.btech.constructed_units(),
            world.btech.constructed_units()
        );
    }
}

/// Support exposure cancels a pending stand recovery even when the Mech is already prone.
#[tokio::test]
async fn exposed_support_cancels_prone_stand_recovery() {
    for template in firing::templates().into_iter().take(2) {
        let (_dir, config, mut world, id, _, _) =
            firing::fixture_with_target(&template, None, include_str!("../game/mechs/AS7-D")).await;
        firing::edit(&mut world, id, |state| {
            state["posture"] = serde_json::to_value(BattlePosture::Prone).unwrap();
            state["stand_timer"] =
                serde_json::to_value(BattleStandTimer::Recovering { remaining: 10 }).unwrap();
        });
        environment(&mut world, id, 100, true);
        seed(&mut world, id, 4);
        let armor = world.btech.constructed_units()[&id].sections()[&BattleSection::LeftLeg].armor;
        let report = resolve_battle_tactical_impact(
            &mut world,
            id,
            hit(BattleSection::LeftLeg),
            armor + 1,
            BattleMovementRules::STANDARD.fall,
        )
        .unwrap();
        assert!(
            report
                .impact
                .exposures
                .iter()
                .any(|report| report.section == BattleSection::LeftLeg && report.fall.is_none())
        );
        assert!(world.btech.constructed_units()[&id].stand_timer().is_none());
        assert_eq!(
            world.btech.constructed_units()[&id].posture(),
            BattlePosture::Prone
        );
        world.validate(&config).unwrap();
    }
}
