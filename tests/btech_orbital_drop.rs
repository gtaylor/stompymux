//! Shared orbital-drop rules preserve timing, interception and the reference's landing arithmetic.
use stompymux_rs::*;

/// Ordinary powered Mech landing; individual cases alter only the relevant crew or terrain fact.
fn landing(target: i16, roll: u8) -> DropLandingInput {
    DropLandingInput {
        base_target: target,
        roll: Some(roll),
        hex: Hex::new(Terrain::Clear, 0),
        running: true,
        prone: false,
        incapacitated: false,
        absent_character_pilot: false,
        combat_safe: false,
        mech: true,
    }
}

/// Resolve an independent copy so each arithmetic case starts with intact protection.
fn touchdown(mut drop: OrbitalDrop, input: DropLandingInput) -> DropLanding {
    drop.land(input).unwrap()
}

#[test]
fn orbital_descent_restarts_and_retains_surface_geometry() {
    let surface = DropSurface {
        upper: 0,
        lower: 0,
        landing: 0,
    };
    let mut drop = OrbitalDrop::new(35 * 1024, ORBITAL_DROP_ALTITUDE).unwrap();
    assert_eq!(drop.protection(), DropProtection::Cocoon { integrity: 8 });
    for _ in 0..87 {
        assert_eq!(drop.advance(surface).unwrap(), OrbitalDropStep::Descending);
    }
    let mut restored: OrbitalDrop =
        serde_json::from_value(serde_json::to_value(drop).unwrap()).unwrap();
    for _ in 87..149 {
        assert_eq!(drop.advance(surface).unwrap(), OrbitalDropStep::Descending);
        assert_eq!(
            restored.advance(surface).unwrap(),
            OrbitalDropStep::Descending
        );
        assert_eq!(drop, restored);
    }
    assert_eq!(drop.elevation(), 2);
    assert_eq!(
        drop.advance(surface).unwrap(),
        OrbitalDropStep::Touchdown { surface: 0 }
    );
    assert_eq!(
        restored.advance(surface).unwrap(),
        OrbitalDropStep::Touchdown { surface: 0 }
    );
    assert_eq!(
        drop.land(landing(5, 8)).unwrap(),
        restored.land(landing(5, 8)).unwrap()
    );
    assert_eq!(drop.advance(surface).unwrap(), OrbitalDropStep::Inactive);
    // The support and landing offsets are both used by the reference's drop-height test.
    let raised = DropSurface {
        upper: 5,
        lower: 5,
        landing: 5,
    };
    let mut drop = OrbitalDrop::new(1024, 14).unwrap();
    assert_eq!(drop.advance(raised).unwrap(), OrbitalDropStep::Descending);
    assert_eq!(drop.elevation(), 12);
    assert_eq!(
        drop.advance(raised).unwrap(),
        OrbitalDropStep::Touchdown { surface: 5 }
    );
    let bridge = DropSurface {
        upper: 5,
        lower: -3,
        landing: -3,
    };
    assert_eq!(bridge.height_above_surface(4), 10);
    assert_eq!(bridge.height_above_surface(6), 4);
}

#[test]
fn cocoon_interception_and_firing_share_breach_transitions() {
    for roll in 2..=8 {
        let mut drop = OrbitalDrop::new(35 * 1024, 300).unwrap();
        let before = drop;
        assert!(
            !drop
                .intercept(u32::MAX, roll, 0, false)
                .unwrap()
                .intercepted
        );
        assert_eq!(drop, before);
    }
    for roll in 9..=12 {
        let mut drop = OrbitalDrop::new(35 * 1024, 300).unwrap();
        assert_eq!(drop.target_modifier(), -2);
        let hit = drop.intercept(7, roll, 0, false).unwrap();
        assert!(hit.intercepted);
        assert!(hit.breach.is_none());
        assert_eq!(drop.protection(), DropProtection::Cocoon { integrity: 1 });
        let hit = drop.intercept(u32::MAX, roll, 0, true).unwrap();
        assert!(hit.intercepted);
        assert_eq!(hit.breach, Some(DropBreach::JumpJets));
        assert!(!drop.protected());
        assert_eq!(drop.target_modifier(), 0);
        assert!(!drop.intercept(100, 0, 0, false).unwrap().intercepted);
        assert_eq!(drop.open_for_fire(0, false), Some(DropBreach::FreeFall));
    }
    let mut drop = OrbitalDrop::new(35 * 1024, 300).unwrap();
    let before = drop;
    assert!(drop.intercept(1, 1, 0, true).is_err());
    assert_eq!(drop, before);
    assert_eq!(drop.open_for_fire(0, true), Some(DropBreach::JumpJets));
    assert_eq!(
        drop.advance(DropSurface {
            upper: 0,
            lower: 0,
            landing: 0
        })
        .unwrap(),
        OrbitalDropStep::Descending
    );
    let mut grounded = OrbitalDrop::new(35 * 1024, 0).unwrap();
    assert_eq!(grounded.open_for_fire(0, false), None);
    assert_eq!(
        grounded.intercept(100, 9, 0, true).unwrap().breach,
        Some(DropBreach::AtSurface)
    );
}

#[test]
fn altitude_overflow_rejects_the_step_without_changing_the_drop() {
    let mut drop = OrbitalDrop::new(1024, i32::MIN).unwrap();
    let before = drop;
    let surface = DropSurface {
        upper: i32::MIN,
        lower: i32::MIN,
        landing: i32::MIN,
    };
    assert!(drop.advance(surface).is_err());
    assert_eq!(drop, before);
}

#[test]
fn landing_margins_chassis_multipliers_and_experience_match_reference_rules() {
    let armored = OrbitalDrop::new(35 * 1024, 2).unwrap();
    let good = touchdown(armored, landing(5, 7));
    assert_eq!(
        (
            good.target,
            good.margin,
            good.fall_levels,
            good.experience_reason
        ),
        (Some(5), 4, 0, Some(6))
    );
    let bad = touchdown(armored, landing(7, 5));
    assert_eq!((bad.margin, bad.fall_levels), (-4, 8));
    let mut vehicle = landing(7, 5);
    vehicle.mech = false;
    assert_eq!(touchdown(armored, vehicle).fall_levels, 12);
    let mut parachute = OrbitalDrop::new(5119, 2).unwrap();
    let result = parachute.land(vehicle).unwrap();
    assert!(result.parachute);
    assert_eq!(result.fall_levels, 4);
    let mut jets = armored;
    assert_eq!(jets.open_for_fire(0, true), Some(DropBreach::JumpJets));
    let result = jets.land(landing(7, 5)).unwrap();
    assert_eq!(
        (result.target, result.margin, result.fall_levels),
        (Some(11), -12, 24)
    );
    let mut breached = armored;
    assert_eq!(breached.open_for_fire(0, false), Some(DropBreach::FreeFall));
    let result = breached.land(vehicle).unwrap();
    assert_eq!(
        (result.target, result.margin, result.fall_levels),
        (Some(17), -24, 72)
    );
    let mut prone = landing(7, 9);
    prone.prone = true;
    let result = touchdown(armored, prone);
    assert_eq!(
        (result.margin, result.fall_levels, result.experience_reason),
        (-6, 12, Some(18))
    );
    prone.incapacitated = true;
    let result = touchdown(armored, prone);
    assert_eq!(
        (result.margin, result.fall_levels, result.experience_reason),
        (-16, 32, Some(20))
    );
    let mut off = landing(7, 12);
    off.running = false;
    let result = touchdown(armored, off);
    assert_eq!(
        (result.target, result.margin, result.fall_levels),
        (Some(17), -30, 60)
    );
    for (terrain, levels) in [(Terrain::Water, 8), (Terrain::Ice, 12), (Terrain::Road, 0)] {
        let mut input = landing(5, 5);
        input.hex = Hex::new(terrain, 1);
        assert_eq!(touchdown(armored, input).fall_levels, levels);
    }
    let mut absent = landing(5, 12);
    absent.absent_character_pilot = true;
    let result = touchdown(armored, absent);
    assert_eq!(
        (result.target, result.margin, result.fall_levels),
        (Some(104), -184, 368)
    );
    assert!(
        touchdown(armored, landing(2, 12))
            .experience_reason
            .is_none()
    );
}

#[test]
fn safe_landings_skip_dice_and_rejected_rolls_leave_state_unchanged() {
    let mut drop = OrbitalDrop::new(35 * 1024, 2).unwrap();
    let before = drop;
    for roll in [None, Some(0), Some(1), Some(13)] {
        let mut input = landing(5, 7);
        input.roll = roll;
        assert!(drop.land(input).is_err());
        assert_eq!(drop, before);
    }
    let mut input = landing(7, 2);
    input.roll = None;
    input.combat_safe = true;
    input.running = false;
    input.incapacitated = true;
    let report = drop.land(input).unwrap();
    assert_eq!(
        (
            report.target,
            report.roll,
            report.margin,
            report.fall_levels,
            report.experience_reason
        ),
        (None, None, 0, 0, None)
    );
    assert_eq!(drop.protection(), DropProtection::Breached);
}
