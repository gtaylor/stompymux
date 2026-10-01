//! Standard vehicle hit tables, turret loss, and armor-dependent critical dice boundaries.
use stompymux_rs::{
    BattleDice, BattleHitArc as Arc, BattleVehicle, BattleVehicleSection as S,
    BattleVehicleTemplate,
};

/// Normalize armor to percentages while preserving valid vehicle anatomy and equipment.
fn vehicle(armor: u16) -> BattleVehicle {
    let vehicle = BattleVehicle::new(
        BattleVehicleTemplate::parse("Demolisher",include_str!("../game/mechs/Demolisher.toml")).unwrap(),
    )
    .unwrap();
    let mut state = serde_json::to_value(vehicle).unwrap();
    for section in ["left", "right", "front", "rear", "turret"] {
        state["definition"]["sections"][section]["armor"] = 100.into();
        state["sections"][section]["armor"] = armor.into();
    }
    serde_json::from_value(state).unwrap()
}

#[test]
fn standard_vehicle_locations_cover_every_arc_roll_and_turret_state() {
    let intact = vehicle(70);
    let mut lost = intact.clone();
    lost.damage_phase(S::Turret, 100, stompymux_rs::BattleDamagePhase::Internal)
        .unwrap();
    let turretless = BattleVehicle::new(
        BattleVehicleTemplate::parse("Flatbed_Truck",include_str!("../game/mechs/Flatbed_Truck.toml")).unwrap(),
    )
    .unwrap();
    for (arc, hull, row) in [
        (
            Arc::Front,
            S::Front,
            [
                S::Front,
                S::Front,
                S::Front,
                S::Front,
                S::Front,
                S::Front,
                S::Front,
                S::Front,
                S::Turret,
                S::Turret,
                S::Turret,
            ],
        ),
        (
            Arc::Rear,
            S::Rear,
            [
                S::Rear,
                S::Rear,
                S::Rear,
                S::Rear,
                S::Rear,
                S::Rear,
                S::Rear,
                S::Rear,
                S::Turret,
                S::Turret,
                S::Turret,
            ],
        ),
        (
            Arc::Left,
            S::Left,
            [
                S::Left,
                S::Left,
                S::Left,
                S::Left,
                S::Left,
                S::Left,
                S::Left,
                S::Left,
                S::Turret,
                S::Turret,
                S::Left,
            ],
        ),
        (
            Arc::Right,
            S::Right,
            [
                S::Right,
                S::Right,
                S::Right,
                S::Right,
                S::Right,
                S::Right,
                S::Right,
                S::Right,
                S::Turret,
                S::Turret,
                S::Right,
            ],
        ),
    ] {
        for roll in 2..=12 {
            let mut dice = BattleDice::seeded([3; 32]);
            let before = dice.clone();
            let hit = intact.standard_hit(arc, roll, 2, &mut dice).unwrap();
            assert_eq!(hit.section, row[usize::from(roll - 2)]);
            assert!(!hit.through_armor_critical);
            assert_eq!(dice, before);
            for vehicle in [&lost, &turretless] {
                assert_eq!(
                    vehicle
                        .standard_hit(arc, roll, 1, &mut dice)
                        .unwrap()
                        .section,
                    hull
                );
            }
        }
    }
    let mut dice = BattleDice::seeded([4; 32]);
    let before = dice.clone();
    for roll in [0, 1, 13, 255] {
        assert!(intact.standard_hit(Arc::Front, roll, 2, &mut dice).is_err());
    }
    assert_eq!(dice, before);
}

#[test]
fn critical_thresholds_and_modes_preserve_secondary_dice_order() {
    for (armor, arc, roll, sides) in [
        (39, Arc::Front, 2, Some(12)),
        (40, Arc::Front, 2, None),
        (49, Arc::Front, 11, Some(12)),
        (50, Arc::Front, 11, None),
        (100, Arc::Left, 12, Some(71)),
        (99, Arc::Left, 12, None),
        (0, Arc::Front, 10, None),
        (0, Arc::Rear, 12, Some(12)),
    ] {
        let vehicle = vehicle(armor);
        for seed in 0..=255 {
            let mut dice = BattleDice::seeded([seed; 32]);
            let mut expected = dice.clone();
            let critical = sides.is_some_and(|sides| {
                expected.die(sides).unwrap() == if sides == 12 { 6 } else { 23 }
            });
            assert_eq!(
                vehicle
                    .standard_hit(arc, roll, 2, &mut dice)
                    .unwrap()
                    .through_armor_critical,
                critical
            );
            assert_eq!(dice, expected);
        }
        let mut dice = BattleDice::seeded([3; 32]);
        let before = dice.clone();
        for mode in [-1, 0, 1] {
            assert!(
                !vehicle
                    .standard_hit(arc, roll, mode, &mut dice)
                    .unwrap()
                    .through_armor_critical
            );
        }
        assert_eq!(dice, before);
    }
}

#[test]
fn unarmored_faces_crit_without_dice_unless_stationary_or_critproof() {
    let mut state = serde_json::to_value(vehicle(0)).unwrap();
    state["definition"]["sections"]["front"]["armor"] = 0.into();
    let unarmored: BattleVehicle = serde_json::from_value(state.clone()).unwrap();
    let mut dice = BattleDice::seeded([5; 32]);
    let before = dice.clone();
    assert!(
        unarmored
            .standard_hit(Arc::Front, 2, 0, &mut dice)
            .unwrap()
            .through_armor_critical
    );
    assert_eq!(dice, before);
    state["definition"]["attributes"]["specials"] = "CritProof_Tech".into();
    let proof: BattleVehicle = serde_json::from_value(state.clone()).unwrap();
    assert!(
        !proof
            .standard_hit(Arc::Front, 2, 2, &mut dice)
            .unwrap()
            .through_armor_critical
    );
    state["definition"]["attributes"]["specials"] = "".into();
    state["definition"]["movement"] = "stationary".into();
    let fixed: BattleVehicle = serde_json::from_value(state).unwrap();
    assert!(
        !fixed
            .standard_hit(Arc::Front, 2, 2, &mut dice)
            .unwrap()
            .through_armor_critical
    );
    assert_eq!(dice, before);
}
