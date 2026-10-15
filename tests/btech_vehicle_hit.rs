//! The Ground Combat Vehicle Hit Location Table: every arc and roll, turret fallback, motive
//! rolls on the dagger rows and the critical-proof and stationary exemptions.
use stompymux_rs::{
    Dice, HitArc as Arc, Vehicle, VehicleMotiveHit, VehicleSection as S, VehicleTemplate,
};

/// A Demolisher with the supplied armor on every face.
fn vehicle(armor: u16) -> Vehicle {
    let vehicle = Vehicle::new(crate::support::templates::fitted(
        VehicleTemplate::parse("Demolisher", include_str!("../game/units/Demolisher.toml"))
            .unwrap(),
    ))
    .unwrap();
    let mut state = serde_json::to_value(vehicle).unwrap();
    for section in ["left", "right", "front", "rear", "turret"] {
        state["definition"]["sections"][section]["armor"] = 100.into();
        state["sections"][section]["armor"] = armor.into();
    }
    serde_json::from_value(state).unwrap()
}

/// Total Warfare p. 193 rows 2 through 12 for each attack direction, `T` standing for the turret.
fn expected_rows() -> [(Arc, [S; 11]); 4] {
    use S::*;
    [
        (
            Arc::Front,
            [
                Front, Front, Front, Right, Front, Front, Front, Left, Turret, Turret, Turret,
            ],
        ),
        (
            Arc::Rear,
            [
                Rear, Rear, Rear, Left, Rear, Rear, Rear, Right, Turret, Turret, Turret,
            ],
        ),
        (
            Arc::Left,
            [
                Left, Left, Left, Front, Left, Left, Left, Rear, Turret, Turret, Turret,
            ],
        ),
        (
            Arc::Right,
            [
                Right, Right, Right, Front, Right, Right, Right, Rear, Turret, Turret, Turret,
            ],
        ),
    ]
}

#[test]
fn locations_follow_total_warfare_with_turret_hits_falling_back_to_the_face() {
    let intact = vehicle(70);
    let mut lost = intact.clone();
    lost.damage_phase(S::Turret, 100, stompymux_rs::DamagePhase::Internal)
        .unwrap();
    let turretless = Vehicle::new(crate::support::templates::fitted(
        VehicleTemplate::parse(
            "Flatbed_Truck",
            include_str!("../game/units/Flatbed_Truck.toml"),
        )
        .unwrap(),
    ))
    .unwrap();
    for (arc, row) in expected_rows() {
        let face = arc.vehicle_section();
        for roll in 2..=12u8 {
            let expected = row[usize::from(roll - 2)];
            let mut dice = Dice::seeded([3; 32]);
            let hit = intact.hit(arc, roll, &mut dice).unwrap();
            assert_eq!(hit.section, expected, "{arc:?}/{roll}");
            let side = matches!(arc, Arc::Left | Arc::Right);
            assert_eq!(
                hit.through_armor_critical,
                matches!(roll, 2 | 12) || (side && roll == 8),
                "{arc:?}/{roll}"
            );
            // Dagger rows roll once on the Motive System Damage Table; nothing else draws dice.
            let motive = matches!(roll, 3 | 4 | 5 | 9);
            assert_eq!(hit.motive.is_some(), motive, "{arc:?}/{roll}");
            let mut expected_dice = Dice::seeded([3; 32]);
            if motive {
                let rolled = expected_dice.two_d6();
                assert_eq!(hit.motive.unwrap().roll, rolled);
            }
            assert_eq!(dice, expected_dice);
            for vehicle in [&lost, &turretless] {
                let section = vehicle
                    .hit(arc, roll, &mut Dice::seeded([3; 32]))
                    .unwrap()
                    .section;
                assert_eq!(
                    section,
                    if expected == S::Turret {
                        face
                    } else {
                        expected
                    },
                    "{arc:?}/{roll} without a turret"
                );
            }
        }
    }
    let mut dice = Dice::seeded([4; 32]);
    let before = dice.clone();
    for roll in [0, 1, 13, 255] {
        assert!(intact.hit(Arc::Front, roll, &mut dice).is_err());
    }
    assert_eq!(dice, before);
}

#[test]
fn motive_rolls_apply_direction_and_vehicle_type_modifiers() {
    // Tracked +0, wheeled +2, hover +3; rear +1, sides +2 (TW p. 193).
    for (movement, type_modifier) in [("track", 0u8), ("wheel", 2), ("hover", 3)] {
        let text = crate::support::templates::with_movement(
            include_str!("../game/units/Demolisher.toml"),
            movement,
        )
        .replace("walk_mp = 5", "walk_mp = 8");
        let vehicle = Vehicle::new(crate::support::templates::fitted(
            VehicleTemplate::parse("test", &text).unwrap(),
        ))
        .unwrap();
        for (arc, direction) in [
            (Arc::Front, 0u8),
            (Arc::Rear, 1),
            (Arc::Left, 2),
            (Arc::Right, 2),
        ] {
            for seed in 0..=255u8 {
                let mut dice = Dice::seeded([seed; 32]);
                let roll = dice.two_d6();
                let hit = vehicle.hit(arc, 3, &mut Dice::seeded([seed; 32])).unwrap();
                let motive = hit.motive.unwrap();
                assert_eq!(motive.roll, roll);
                assert_eq!(motive.modifier, direction + type_modifier);
                let total = roll + direction + type_modifier;
                let (expected_hit, tier) = match total {
                    2..=5 => (None, 0),
                    6 | 7 => (None, 1),
                    8 | 9 => (Some(VehicleMotiveHit::SpeedLoss { movement_points: 1 }), 2),
                    10 | 11 => (Some(VehicleMotiveHit::HalveSpeed), 3),
                    _ => (Some(VehicleMotiveHit::Immobilize), 0),
                };
                assert_eq!(motive.hit, expected_hit, "{movement}/{arc:?}/{total}");
                assert_eq!(motive.driving_penalty, tier, "{movement}/{arc:?}/{total}");
            }
        }
    }
}

#[test]
fn critproof_and_stationary_vehicles_take_no_criticals_or_motive_rolls() {
    let mut state = serde_json::to_value(vehicle(0)).unwrap();
    state["definition"]["attributes"]["specials"] = "CritProof_Tech".into();
    let proof: Vehicle = serde_json::from_value(state.clone()).unwrap();
    state["definition"]["attributes"]["specials"] = "".into();
    state["definition"]["movement"] = "stationary".into();
    let fixed: Vehicle = serde_json::from_value(state).unwrap();
    for unit in [&proof, &fixed] {
        let mut dice = Dice::seeded([5; 32]);
        let before = dice.clone();
        for roll in [2, 3, 8, 12] {
            let hit = unit.hit(Arc::Left, roll, &mut dice).unwrap();
            assert!(!hit.through_armor_critical);
            assert!(hit.motive.is_none());
        }
        assert_eq!(dice, before);
    }
    // Unarmored faces are not special: a 2 is a critical like any other.
    let mut state = serde_json::to_value(vehicle(0)).unwrap();
    state["definition"]["sections"]["front"]["armor"] = 0.into();
    let unarmored: Vehicle = serde_json::from_value(state).unwrap();
    assert!(
        unarmored
            .hit(Arc::Front, 2, &mut Dice::seeded([5; 32]))
            .unwrap()
            .through_armor_critical
    );
}
