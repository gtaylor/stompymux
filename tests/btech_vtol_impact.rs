//! Aircraft impacts share armor and critical transactions.
use crate::support;
use stompymux_rs::*;

#[tokio::test]
async fn aircraft_impacts_apply_location_rotor_and_armor_effects_with_saved_replay() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Aircraft impact".into(), Kind::Thing);

    for proof in [false, true] {
        let source = if proof {
            include_str!("../game/mechs/Kestrel.toml").replacen(
                "\"CargoTech\"",
                "\"CargoTech\", \"CritProof_Tech\"",
                1,
            )
        } else {
            include_str!("../game/mechs/Kestrel.toml").into()
        };
        let unit = Vehicle::new(VehicleTemplate::parse("Kestrel", &source).unwrap()).unwrap();
        for arc in [HitArc::Front, HitArc::Rear, HitArc::Left, HitArc::Right] {
            for roll in 2..=12 {
                let seed = (0..=255)
                    .find(|seed| {
                        let mut dice = Dice::seeded([*seed; 32]);
                        let first = dice.two_d6();
                        (if proof { dice.two_d6() } else { first }) == roll
                    })
                    .unwrap();
                let mut saved = serde_json::to_value(&unit).unwrap();
                saved["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
                let mut state = serde_json::to_value(&world.btech).unwrap();
                state["vehicles"][id.0.to_string()] = saved;
                world.btech = serde_json::from_value(state).unwrap();
                let mut replay = world.clone();
                let mut rules = VehicleImpactRules::STANDARD;
                rules.criticals.enabled = false;
                let selected = unit.definition().vtol_hit(arc, roll).unwrap();
                let report =
                    resolve_battle_vehicle_impact(&mut world, id, arc, 1, None, rules).unwrap();
                assert_eq!(report.hit, Some(selected.hit));
                assert_eq!(report.rolls.len(), if proof { 2 } else { 1 });
                assert_eq!(*report.rolls.last().unwrap(), roll);
                assert_eq!(
                    resolve_battle_vehicle_impact(&mut replay, id, arc, 1, None, rules).unwrap(),
                    report
                );
                assert_eq!(world.btech, replay.btech);
                let changed = &world.btech.vehicles()[&id];
                assert_eq!(
                    changed.rotor_destroyed(),
                    selected.rotor == Some(RotorHit::Destroy)
                );
                if selected.rotor == Some(RotorHit::Damage) {
                    assert_eq!(changed.maximum_speed(), 182.75);
                }
                assert!(changed.lost_criticals().is_empty());
                assert!(!changed.crew_killed());
                assert_eq!(
                    serde_json::from_value::<Vehicle>(serde_json::to_value(changed).unwrap())
                        .unwrap(),
                    *changed
                );
                if !changed.rotor_destroyed() {
                    assert_eq!(report.damage.as_ref().unwrap().absorbed, 1);
                }
            }
        }
    }
}

#[tokio::test]
async fn safe_aircraft_impacts_preserve_material_and_rejected_input_rolls_back() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Protected aircraft".into(), Kind::Thing);
    let unit = Vehicle::new(
        VehicleTemplate::parse("Kestrel", include_str!("../game/mechs/Kestrel.toml")).unwrap(),
    )
    .unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["vehicles"][id.0.to_string()] = serde_json::to_value(&unit).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    let before = world.btech.clone();
    let mut rules = VehicleImpactRules::STANDARD;
    rules.criticals.table = VehicleCriticalTable::Advanced;
    assert!(resolve_battle_vehicle_impact(&mut world, id, HitArc::Front, 0, None, rules).is_err());
    assert_eq!(world.btech, before);
    rules.criticals.table = VehicleCriticalTable::Standard;
    assert!(resolve_battle_vehicle_impact(&mut world, id, HitArc::Left, 0, None, rules).is_err());
    assert_eq!(world.btech, before);
    rules.criticals.combat_safe = true;
    let report =
        resolve_battle_vehicle_impact(&mut world, id, HitArc::Left, 1, None, rules).unwrap();
    assert!(report.hit.is_none());
    let changed = &world.btech.vehicles()[&id];
    assert_eq!(changed.sections(), unit.sections());
    assert_eq!(changed.lost_criticals(), unit.lost_criticals());
    assert_eq!(changed.maximum_speed(), unit.maximum_speed());
    assert_eq!(changed.vtol_flight(), unit.vtol_flight());
}

#[tokio::test]
async fn advanced_aircraft_impacts_use_the_aircraft_table_and_shared_armor_pipeline() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Advanced aircraft".into(), Kind::Thing);
    let unit = Vehicle::new(
        VehicleTemplate::parse("Kestrel", include_str!("../game/mechs/Kestrel.toml")).unwrap(),
    )
    .unwrap();
    for arc in [HitArc::Front, HitArc::Rear, HitArc::Left, HitArc::Right] {
        for roll in 2..=12 {
            let seed = (0..=255)
                .find(|seed| {
                    let mut dice = Dice::seeded([*seed; 32]);
                    dice.two_d6();
                    dice.two_d6() == roll
                })
                .unwrap();
            let mut saved = serde_json::to_value(&unit).unwrap();
            saved["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
            let mut state = serde_json::to_value(&world.btech).unwrap();
            state["vehicles"][id.0.to_string()] = saved;
            world.btech = serde_json::from_value(state).unwrap();
            let mut replay = world.clone();
            let mut rules = VehicleImpactRules::STANDARD;
            rules.criticals.table = VehicleCriticalTable::Advanced;
            rules.criticals.enabled = false;
            let mut dice = Dice::seeded([seed; 32]);
            dice.two_d6();
            dice.two_d6();
            let selected = unit.advanced_vtol_hit(arc, roll, 2, 60, &mut dice).unwrap();
            let report =
                resolve_battle_vehicle_impact(&mut world, id, arc, 1, None, rules).unwrap();
            assert_eq!(report.hit, Some(selected.hit));
            assert_eq!(report.rolls.len(), 2);
            assert_eq!(report.damage.as_ref().unwrap().absorbed, 1);
            assert_eq!(
                world.btech.vehicles()[&id].maximum_speed(),
                unit.maximum_speed()
            );
            assert_eq!(
                resolve_battle_vehicle_impact(&mut replay, id, arc, 1, None, rules).unwrap(),
                report
            );
            assert_eq!(replay.btech, world.btech);
        }
    }
}

#[tokio::test]
async fn impact_and_critical_routing_honor_aircraft_policy_over_ground_settings() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Aircraft policy".into(), Kind::Thing);
    let unit = Vehicle::new(
        VehicleTemplate::parse("Kestrel", include_str!("../game/mechs/Kestrel.toml")).unwrap(),
    )
    .unwrap();
    for (ground, aircraft) in [
        (
            VehicleCriticalTable::Advanced,
            VehicleCriticalTable::Standard,
        ),
        (
            VehicleCriticalTable::Standard,
            VehicleCriticalTable::Advanced,
        ),
    ] {
        let mut state = serde_json::to_value(&world.btech).unwrap();
        let mut saved = serde_json::to_value(&unit).unwrap();
        saved["dice"] = serde_json::to_value(Dice::seeded([42; 32])).unwrap();
        state["vehicles"][id.0.to_string()] = saved;
        world.btech = serde_json::from_value(state).unwrap();
        let mut critical_world = world.clone();
        let mut rules = VehicleImpactRules::STANDARD;
        rules.criticals.table = ground;
        rules.criticals.vtol_table = Some(aircraft);
        rules.criticals.enabled = false;
        let report =
            resolve_battle_vehicle_impact(&mut world, id, HitArc::Front, 1, None, rules).unwrap();
        assert_eq!(
            report.rolls.len(),
            if aircraft == VehicleCriticalTable::Advanced {
                2
            } else {
                1
            }
        );
        rules.criticals.enabled = true;
        let selected = roll_battle_vehicle_critical(
            &mut critical_world,
            id,
            VehicleSection::Front,
            rules.criticals,
        )
        .unwrap();
        assert_eq!(selected.table, aircraft);
        let mut dice = Dice::seeded([42; 32]);
        assert_eq!(
            selected.rolls,
            [if aircraft == VehicleCriticalTable::Advanced {
                dice.two_d6()
            } else {
                dice.d6()
            }]
        );
    }
}
