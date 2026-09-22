//! Scheduler regressions under deliberately reduced shared budgets.
use super::*;
use crate::{BattleMapAsset, BattleUnitTemplate, Kind};

fn fixture() -> (Config, World, Vec<ObjectId>) {
    let config =
        Config::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"))
            .unwrap();
    let mut world = World::default();
    let map = world.create(&config, "Scheduler test".into(), Kind::Room);
    crate::create_battle_map(
        &mut world,
        map,
        "scheduler",
        BattleMapAsset::parse(&format!("20 20\n{}", (".0".repeat(20) + "\n").repeat(20))).unwrap(),
    )
    .unwrap();
    let mut units = Vec::new();
    for row in [2, 8, 14] {
        let id = world.create(&config, format!("Scheduler unit {row}"), Kind::Thing);
        BattleUnitTemplate::parse(include_str!("../../../game/mechs/JR7-D"))
            .unwrap()
            .create(&mut world, id)
            .unwrap();
        crate::place_battle_unit(&mut world, id, map, 1, row).unwrap();
        Arc::make_mut(&mut world.btech.constructed)
            .get_mut(&id)
            .unwrap()
            .power = BattlePower::Running;
        let mut controller = super::super::AutopilotController::new();
        controller
            .submit(
                vec![AutopilotOrder::Move {
                    destination: BattlePosition {
                        map,
                        x: 18,
                        y: row as u16,
                    },
                    arrival_radius: 0,
                }],
                super::super::AutopilotSubmissionMode::Replace,
                None,
            )
            .unwrap();
        controller.resume(None).unwrap();
        Arc::make_mut(&mut world.btech.controllers).insert(id, controller);
        units.push(id);
    }
    (config, world, units)
}

#[test]
fn one_expansion_budget_rotates_service_until_every_route_is_found() {
    let (config, mut world, ids) = fixture();
    let mut found = std::collections::BTreeSet::new();
    for tick in 1..=400 {
        let mut metrics = AutopilotRuntimeMetrics::default();
        advance_budgeted(&mut world, &config, tick, Some(&mut metrics), 1, 1200).unwrap();
        assert!(metrics.expansions <= 1);
        assert!(total_search_records(&world) <= 1200);
        for id in &ids {
            if world
                .btech
                .autopilot_plans
                .get(id)
                .is_some_and(|p| !p.route.is_empty())
            {
                found.insert(*id);
                Arc::make_mut(&mut world.btech.controllers)
                    .get_mut(id)
                    .unwrap()
                    .pause(None)
                    .unwrap();
            }
        }
        if found.len() == ids.len() {
            return;
        }
    }
    panic!("rotating scheduler starved a route under a one-expansion global budget");
}

#[test]
fn single_frontier_reservation_defers_other_jobs_without_deadlock_or_failure() {
    let (config, mut world, ids) = fixture();
    let mut found = std::collections::BTreeSet::new();
    for tick in 1..=400 {
        advance_budgeted(&mut world, &config, tick, None, 1, 400).unwrap();
        let frontiers = world
            .btech
            .autopilot_plans
            .values()
            .filter(|p| p.search.is_some())
            .count();
        assert!(frontiers <= 1);
        assert!(total_search_records(&world) <= 400);
        assert!(
            world
                .btech
                .controllers()
                .values()
                .all(|c| c.state() != AutopilotState::Blocked)
        );
        for id in &ids {
            if world
                .btech
                .autopilot_plans
                .get(id)
                .is_some_and(|p| !p.route.is_empty())
            {
                found.insert(*id);
                Arc::make_mut(&mut world.btech.controllers)
                    .get_mut(id)
                    .unwrap()
                    .pause(None)
                    .unwrap();
            }
        }
        if found.len() == ids.len() {
            return;
        }
    }
    panic!("deferred route never acquired the released reservation");
}

#[test]
fn valid_combat_target_is_retained_between_reassessment_ticks_but_loss_is_immediate() {
    use super::super::observations::{AutopilotContact, AutopilotOwnReadiness};
    let observer = ObjectId(10);
    let old = ObjectId(20);
    let challenger = ObjectId(21);
    let contact = |unit, range| AutopilotContact {
        unit,
        position: BattlePosition {
            map: ObjectId(1),
            x: 1,
            y: 1,
        },
        friendly: false,
        identified: true,
        known_destroyed: false,
        range,
        seen_at: 2,
    };
    let mut observation = AutopilotObservation {
        unit: observer,
        time: 2,
        position: None,
        heading: None,
        speed: 0.0,
        own: AutopilotOwnReadiness {
            power: BattlePower::Running,
            maximum_speed: 0.0,
            heat: None,
            weapons: Vec::new(),
        },
        contacts: vec![contact(old, 10.0), contact(challenger, 1.0)],
        remembered: Vec::new(),
    };
    let config = AutopilotConfig {
        fire_mode: super::super::AutopilotFireMode::Opportunistic,
        ..Default::default()
    };
    assert_eq!(combat_target(&config, &observation, Some(old)), Some(old));
    observation.time = 4;
    assert_eq!(
        combat_target(&config, &observation, Some(old)),
        Some(challenger)
    );
    observation.time = 5;
    observation.contacts[0].known_destroyed = true;
    assert_eq!(
        combat_target(&config, &observation, Some(old)),
        Some(challenger)
    );
    observation.contacts[0].known_destroyed = false;
    observation.contacts[0].friendly = true;
    assert_eq!(
        combat_target(&config, &observation, Some(old)),
        Some(challenger)
    );
}

#[test]
fn projected_heat_accounts_for_burst_modes_damage_and_gatling_bound_without_dice() {
    use crate::{BattleFireMode as Mode, BattleWeapon as Weapon};
    assert_eq!(
        projected_launch_heat(Weapon::MediumLaser, Mode::Normal, 2),
        u16::from(Weapon::MediumLaser.profile().heat) + 2
    );
    assert_eq!(
        projected_launch_heat(Weapon::UltraAc5, Mode::Ultra, 0),
        2 * u16::from(Weapon::UltraAc5.profile().heat)
    );
    assert_eq!(
        projected_launch_heat(Weapon::MachineGun, Mode::Gatling, 1),
        7
    );
}
