//! Conventional cluster probabilities, packet boundaries and atomic multi-location salvos.
use crate::support;
use stompymux_rs::BattleWeaponSalvo;
use stompymux_rs::{
    BattleDice, BattleHitArc, BattleHitRules, BattleTemplate, BattleWeapon as Weapon, Kind,
    ObjectId, create_battle_unit, persistence, resolve_battle_salvo,
};

#[test]
fn conventional_cluster_distributions_and_group_sizes_match_reference_facts() {
    for (weapon, total) in [
        (Weapon::Srm2, 51),
        (Weapon::Lrm5, 114),
        (Weapon::Lrm10, 227),
        (Weapon::Lrm15, 342),
        (Weapon::Srm4, 95),
        (Weapon::Srm6, 144),
        (Weapon::Lrm20, 457),
        (Weapon::Mrm10, 227),
        (Weapon::Mrm20, 457),
        (Weapon::Mrm30, 684),
        (Weapon::Mrm40, 914),
        (Weapon::LrDfm5, 114),
        (Weapon::LrDfm10, 227),
        (Weapon::LrDfm15, 342),
        (Weapon::LrDfm20, 457),
        (Weapon::SrDfm2, 51),
        (Weapon::SrDfm4, 95),
        (Weapon::SrDfm6, 144),
        (Weapon::Elrm5, 114),
        (Weapon::Elrm10, 227),
        (Weapon::Elrm15, 342),
        (Weapon::Elrm20, 457),
    ] {
        let mut count = 0;
        for a in 1..=6 {
            for b in 1..=6 {
                let missiles = weapon.missile_hits(a + b).unwrap();
                count += u32::from(missiles);
                let groups = weapon.damage_groups(Some(a + b)).unwrap();
                assert_eq!(
                    groups.iter().sum::<u16>(),
                    u16::from(missiles) * u16::from(weapon.profile().damage)
                );
                assert!(groups.iter().all(|damage| *damage > 0 && *damage <= 5));
            }
        }
        assert_eq!(count, total);
        assert!(weapon.missile_hits(1).is_err());
        assert!(weapon.missile_hits(13).is_err());
        assert!(weapon.damage_groups(None).is_err());
    }
    assert_eq!(Weapon::Srm2.damage_groups(Some(7)).unwrap(), vec![2]);
    assert_eq!(Weapon::Srm2.damage_groups(Some(8)).unwrap(), vec![2, 2]);
    assert_eq!(Weapon::Lrm5.damage_groups(Some(12)).unwrap(), vec![5]);
    assert_eq!(Weapon::Lrm10.damage_groups(Some(12)).unwrap(), vec![5, 5]);
    assert_eq!(
        Weapon::Lrm15.damage_groups(Some(12)).unwrap(),
        vec![5, 5, 5]
    );
    assert_eq!(Weapon::Srm4.damage_groups(Some(7)).unwrap(), vec![2, 2, 2]);
    assert_eq!(
        Weapon::Lrm20.damage_groups(Some(9)).unwrap(),
        vec![5, 5, 5, 1]
    );
    assert_eq!(
        Weapon::Lrm20.damage_groups(Some(12)).unwrap(),
        vec![5, 5, 5, 5]
    );
    assert_eq!(Weapon::MediumLaser.damage_groups(None).unwrap(), vec![5]);
    assert_eq!(Weapon::Ac20.damage_groups(None).unwrap(), vec![20]);
    assert!(Weapon::MediumLaser.missile_hits(7).is_err());
    assert!(Weapon::Ac20.damage_groups(Some(7)).is_err());
}

#[tokio::test]
async fn salvo_locations_replay_and_restart_preserves_every_group_and_roll() {
    use sqlx::Connection;
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Salvo Atlas".into(), Kind::Thing);
    let object = world.objects.get_mut(&id).unwrap();
    object.location = Some(ObjectId(config.start()));
    object.home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        id,
        BattleTemplate::parse("AS7-D", include_str!("fixtures/btech/mechs/AS7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
    world
        .btech
        .set_unit_dice(id, BattleDice::seeded([0; 32]))
        .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let before = world.clone();
    let rules = BattleHitRules {
        inferno_penalty: false,
        exile_stun_mode: 0,
    };
    assert!(
        resolve_battle_salvo(
            &mut world,
            ObjectId(-1),
            Weapon::Srm6,
            BattleHitArc::Front,
            rules
        )
        .is_err()
    );
    assert_eq!(world.btech, before.btech);
    let report =
        resolve_battle_salvo(&mut world, id, Weapon::Srm6, BattleHitArc::Front, rules).unwrap();
    assert_eq!(
        report.groups.len(),
        usize::from(
            Weapon::Srm6
                .missile_hits(report.cluster_roll.unwrap())
                .unwrap()
        )
    );
    assert!(report.groups.iter().all(|group| group.damage == 2));
    assert!(
        report
            .groups
            .iter()
            .map(|group| group.hit.section)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            > 1
    );
    let mut repeated = before.clone();
    assert_eq!(
        resolve_battle_salvo(&mut repeated, id, Weapon::Srm6, BattleHitArc::Front, rules).unwrap(),
        report
    );
    assert_eq!(repeated.btech, world.btech);
    let mut sql = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TRIGGER reject_salvo BEFORE UPDATE ON btech_units BEGIN SELECT RAISE(ABORT,'salvo failure'); END;").execute(&mut sql).await.unwrap();
    assert!(persistence::save(&config.database(), &world).await.is_err());
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        before.btech
    );
    sqlx::query("DROP TRIGGER reject_salvo")
        .execute(&mut sql)
        .await
        .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    assert_eq!(
        resolve_battle_salvo(
            &mut loaded,
            id,
            Weapon::MediumLaser,
            BattleHitArc::Rear,
            rules
        )
        .unwrap(),
        resolve_battle_salvo(
            &mut world,
            id,
            Weapon::MediumLaser,
            BattleHitArc::Rear,
            rules
        )
        .unwrap()
    );
}

/// An occupied Atlas for tactical salvo composition, without requiring RPG character records.
async fn tactical_fixture() -> (
    tempfile::TempDir,
    stompymux_rs::Config,
    stompymux_rs::World,
    ObjectId,
) {
    let (dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Tactical Atlas".into(), Kind::Thing);
    let object = world.objects.get_mut(&id).unwrap();
    object.location = Some(ObjectId(config.start()));
    object.home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        id,
        BattleTemplate::parse("AS7-D", include_str!("fixtures/btech/mechs/AS7-D.toml")).unwrap(),
    )
    .unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    stompymux_rs::assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    (dir, config, world, id)
}

/// Seed only unit hit-selection dice, retaining the rest of a validated fixture snapshot.
fn seeded_target(world: &stompymux_rs::World, id: ObjectId, seed: u8) -> stompymux_rs::World {
    let mut trial = world.clone();
    trial
        .btech
        .set_unit_dice(id, BattleDice::seeded([seed; 32]))
        .unwrap();
    trial
}

#[tokio::test]
async fn tactical_salvo_applies_later_head_injury_and_rolls_back_an_entire_failed_cascade() {
    use stompymux_rs::{BattleSection, resolve_battle_tactical_salvo};
    let (_dir, config, world, id) = tactical_fixture().await;
    let rules = BattleHitRules {
        inferno_penalty: false,
        exile_stun_mode: 0,
    };
    let mut selected = None;
    for seed in 0..=255 {
        let original = seeded_target(&world, id, seed);
        let mut trial = original.clone();
        let material =
            resolve_battle_salvo(&mut trial, id, Weapon::Srm6, BattleHitArc::Front, rules).unwrap();
        if material.groups.len() >= 2
            && material.groups[0].hit.section != BattleSection::Head
            && material.groups[1].hit.section == BattleSection::Head
        {
            selected = Some((original, material));
            break;
        }
    }
    let (mut world, material) = selected.expect("seed with a later head hit");
    let mut invalid = world.clone();
    invalid.objects.get_mut(&ObjectId(1)).unwrap().location = Some(ObjectId(config.start()));
    let before = invalid.btech.clone();
    assert!(
        resolve_battle_tactical_salvo(
            &mut invalid,
            id,
            Weapon::Srm6,
            BattleHitArc::Front,
            stompymux_rs::BattleFallRules {
                vehicle_impact: stompymux_rs::BattleVehicleImpactRules::STANDARD,
                stacking: stompymux_rs::BattleStackingRules::STANDARD,
                stagger: stompymux_rs::BattleStaggerMode::Retain,
                hit: rules,
                extended_piloting: true,
                toughness: false
            }
        )
        .is_err()
    );
    assert_eq!(invalid.btech, before);
    let report = resolve_battle_tactical_salvo(
        &mut world,
        id,
        Weapon::Srm6,
        BattleHitArc::Front,
        stompymux_rs::BattleFallRules {
            vehicle_impact: stompymux_rs::BattleVehicleImpactRules::STANDARD,
            stacking: stompymux_rs::BattleStackingRules::STANDARD,
            stagger: stompymux_rs::BattleStaggerMode::Retain,
            hit: rules,
            extended_piloting: true,
            toughness: false,
        },
    )
    .unwrap();
    assert_eq!(report.cluster_roll, material.cluster_roll);
    assert_eq!(report.groups.len(), material.groups.len());
    for (actual, expected) in report.groups.iter().zip(&material.groups) {
        assert_eq!(actual.hit, expected.hit);
        assert_eq!(actual.damage, expected.damage);
        assert_eq!(actual.impact.phases, expected.impact.phases);
        assert!(
            !actual
                .impact
                .pending_effects
                .contains(&stompymux_rs::BattleImpactEffect::HeadInjury)
        );
    }
    assert_eq!(report.groups[1].pilot_injuries.len(), 1);
    assert!(world.btech.constructed_units()[&id].pilot_injuries() > 0);
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

#[tokio::test]
async fn pilot_loss_stops_remaining_missile_groups_without_rolling_them() {
    use stompymux_rs::{BattleSection, resolve_battle_tactical_salvo};
    let (_dir, config, mut world, id) = tactical_fixture().await;
    let rules = BattleHitRules {
        inferno_penalty: false,
        exile_stun_mode: 0,
    };
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["pilot_injuries"] = 5.into();
        })
        .unwrap();
    for seed in 0..=255 {
        let mut trial = seeded_target(&world, id, seed);
        let mut dice = BattleDice::seeded([seed; 32]);
        let cluster = dice.two_d6();
        if Weapon::Srm6.missile_hits(cluster).unwrap() < 2 || dice.two_d6() != 12 {
            continue;
        }
        let report = resolve_battle_tactical_salvo(
            &mut trial,
            id,
            Weapon::Srm6,
            BattleHitArc::Front,
            stompymux_rs::BattleFallRules {
                vehicle_impact: stompymux_rs::BattleVehicleImpactRules::STANDARD,
                stacking: stompymux_rs::BattleStackingRules::STANDARD,
                stagger: stompymux_rs::BattleStaggerMode::Retain,
                hit: rules,
                extended_piloting: true,
                toughness: false,
            },
        )
        .unwrap();
        assert_eq!(report.groups.len(), 1);
        assert_eq!(report.groups[0].hit.section, BattleSection::Head);
        assert!(report.groups[0].pilot_injuries[0].killed);
        assert!(report.groups[0].impact.destroyed);
        dice.two_d6(); // Material entry precedes the fatal head injury.
        let state = serde_json::to_value(&trial.btech.constructed_units()[&id]).unwrap();
        assert_eq!(state["dice"], serde_json::to_value(dice).unwrap());
        assert_eq!(trial.btech.constructed_units()[&id].pilot(), None);
        persistence::save(&config.database(), &trial).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            trial.btech
        );
        return;
    }
    panic!("No seed exercised pilot loss on the first missile");
}

#[tokio::test]
async fn rear_weapon_hits_ignite_one_dumped_salvo_and_replay_after_restart() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = tactical_fixture().await;
    let map = world.create(&config, "Ignition field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "ignition.map",
        MapAsset::from_cells("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, id, map, 1, 1).unwrap();
    let loadout = world.btech.constructed_units()[&id].loadout().unwrap();
    let (bin_index, bin) = loadout
        .ammunition
        .iter()
        .enumerate()
        .find(|(_, bin)| bin.weapon == Weapon::Srm6)
        .unwrap();
    let initial = world.btech.constructed_units()[&id].ammunition()[bin_index];
    let rules = BattleMovementRules::STANDARD.fall;
    let mut selected = None;
    for seed in 0..=255 {
        let mut trial = seeded_target(&world, id, seed);
        trial
            .btech
            .rewrite_unit_record(id, |record| {
                record["dumping"] = serde_json::to_value(BattleDump {
                    selection: BattleDumpSelection::Slot(bin.location),
                    phase: 4,
                })
                .unwrap();
            })
            .unwrap();
        let before = trial.clone();
        let report = resolve_battle_tactical_salvo(
            &mut trial,
            id,
            Weapon::MediumLaser,
            BattleHitArc::Rear,
            rules,
        )
        .unwrap();
        let group = &report.groups[0];
        if group.hit.section == BattleSection::CenterTorso
            && !group.hit.through_armor_critical
            && group.impact.phases.len() == 3
            && group.impact.criticals.is_empty()
        {
            selected = Some((before, trial, report));
            break;
        }
    }
    let (before, after, report) = selected.expect("plain rear center torso hit");
    let ignition = &report.groups[0].impact.dump_ignitions;
    assert_eq!(ignition.len(), 1);
    assert_eq!(ignition[0].damage, 12);
    assert_eq!(ignition[0].bin_index, bin_index);
    assert_eq!(ignition[0].section, BattleSection::CenterTorso);
    assert_eq!(
        after.btech.constructed_units()[&id].ammunition()[bin_index],
        initial - 1
    );
    assert!(after.btech.constructed_units()[&id].dumping().is_none());
    assert_eq!(
        before.btech.constructed_units()[&id].sections()[&BattleSection::CenterTorso].rear
            - after.btech.constructed_units()[&id].sections()[&BattleSection::CenterTorso].rear,
        14
    );
    assert_eq!(
        before.btech.constructed_units()[&id].sections()[&BattleSection::CenterTorso].internal
            - after.btech.constructed_units()[&id].sections()[&BattleSection::CenterTorso].internal,
        3
    );
    assert!(
        report.groups[0]
            .notices
            .iter()
            .any(|notice| notice.text.contains("ignites!"))
    );
    persistence::save(&config.database(), &before)
        .await
        .unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        resolve_battle_tactical_salvo(
            &mut restored,
            id,
            Weapon::MediumLaser,
            BattleHitArc::Rear,
            rules
        )
        .unwrap(),
        report
    );
    assert_eq!(restored.btech, after.btech);
    let mut ordinary = before.clone();
    let hit = report.groups[0].hit;
    let raw = resolve_battle_impact(&mut ordinary, id, hit, 5).unwrap();
    assert!(raw.dump_ignitions.is_empty());
    assert_eq!(
        ordinary.btech.constructed_units()[&id].ammunition()[bin_index],
        initial
    );
    assert!(ordinary.btech.constructed_units()[&id].dumping().is_some());
    let mut front = before;
    let report = resolve_battle_tactical_salvo(
        &mut front,
        id,
        Weapon::MediumLaser,
        BattleHitArc::Front,
        rules,
    )
    .unwrap();
    assert!(
        report
            .groups
            .iter()
            .all(|group| group.impact.dump_ignitions.is_empty())
    );
    assert!(front.btech.constructed_units()[&id].dumping().is_some());
}
