//! Biped mount boundaries, torso offsets, flipped arms and rear-mount precedence.
use stompymux_rs::MountArcs;
use stompymux_rs::{CriticalLocation, Facing, MechSection as Section, Torso, Weapon, WeaponMount};

fn mount(section: Section, rear_mount: bool) -> WeaponMount {
    WeaponMount {
        weapon: Weapon::MediumLaser,
        criticals: vec![CriticalLocation { section, slot: 0 }],
        rear_mount,
        one_shot: false,
        initially_spent: false,
        initial_ammunition_mode: stompymux_rs::AmmunitionMode::Normal,
        initial_fire_mode: stompymux_rs::FireMode::Normal,
    }
}

#[test]
fn torso_arm_and_rear_boundaries_are_distinct() {
    let normal = Facing::default();
    for bearing in [0.0, 60.0, 300.0, 360.0] {
        assert!(
            mount(Section::CenterTorso, false)
                .bears_on(stompymux_rs::MechChassis::Biped, 0.0, bearing, normal)
                .unwrap()
        );
    }
    for bearing in [60.001, 120.0, 180.0, 240.0, 299.999] {
        assert!(
            !mount(Section::CenterTorso, false)
                .bears_on(stompymux_rs::MechChassis::Biped, 0.0, bearing, normal)
                .unwrap()
        );
    }
    assert!(
        mount(Section::LeftArm, false)
            .bears_on(stompymux_rs::MechChassis::Biped, 0.0, 240.0, normal)
            .unwrap()
    );
    assert!(
        !mount(Section::LeftArm, false)
            .bears_on(stompymux_rs::MechChassis::Biped, 0.0, 120.0, normal)
            .unwrap()
    );
    assert!(
        mount(Section::RightArm, false)
            .bears_on(stompymux_rs::MechChassis::Biped, 0.0, 120.0, normal)
            .unwrap()
    );
    assert!(
        !mount(Section::RightArm, false)
            .bears_on(stompymux_rs::MechChassis::Biped, 0.0, 240.0, normal)
            .unwrap()
    );
    for bearing in [120.0, 240.0] {
        assert!(
            !mount(Section::CenterTorso, true)
                .bears_on(stompymux_rs::MechChassis::Biped, 0.0, bearing, normal)
                .unwrap()
        );
    }
    assert!(
        mount(Section::CenterTorso, true)
            .bears_on(stompymux_rs::MechChassis::Biped, 0.0, 180.0, normal)
            .unwrap()
    );
}

#[test]
fn torso_twist_excludes_legs_and_rear_mount_overrides_arm_flipping() {
    let right = Facing {
        torso: Torso::Right,
        arms_flipped: false,
    };
    // A twist is one hexside, so the right-twisted forward arc reaches bearing 120.
    assert!(
        mount(Section::CenterTorso, false)
            .bears_on(stompymux_rs::MechChassis::Biped, 0.0, 120.0, right)
            .unwrap()
    );
    assert!(
        !mount(Section::CenterTorso, false)
            .bears_on(stompymux_rs::MechChassis::Biped, 0.0, 120.001, right)
            .unwrap()
    );
    assert!(
        !mount(Section::LeftLeg, false)
            .bears_on(stompymux_rs::MechChassis::Biped, 0.0, 120.0, right)
            .unwrap()
    );
    // Reversed arms fire into the rear arc only; their side and forward arcs are gone.
    let flipped = Facing {
        torso: Torso::Center,
        arms_flipped: true,
    };
    assert!(
        mount(Section::LeftArm, false)
            .bears_on(stompymux_rs::MechChassis::Biped, 0.0, 180.0, flipped)
            .unwrap()
    );
    assert!(
        !mount(Section::LeftArm, false)
            .bears_on(stompymux_rs::MechChassis::Biped, 0.0, 270.0, flipped)
            .unwrap()
    );
    assert!(
        !mount(Section::LeftArm, true)
            .bears_on(stompymux_rs::MechChassis::Biped, 0.0, 270.0, flipped)
            .unwrap()
    );
    assert!(
        !mount(Section::LeftArm, false)
            .bears_on(stompymux_rs::MechChassis::Biped, 0.0, 0.0, flipped)
            .unwrap()
    );
    assert!(
        !mount(Section::RightArm, false)
            .bears_on(stompymux_rs::MechChassis::Biped, 0.0, 90.0, flipped)
            .unwrap()
    );
    assert!(
        mount(Section::RightArm, false)
            .bears_on(stompymux_rs::MechChassis::Biped, 0.0, 239.0, flipped)
            .unwrap()
    );
    assert!(
        mount(Section::CenterTorso, false)
            .bears_on(
                stompymux_rs::MechChassis::Biped,
                350.0,
                10.0,
                Facing::default()
            )
            .unwrap()
    );
    assert!(
        mount(Section::CenterTorso, false)
            .bears_on(stompymux_rs::MechChassis::Biped, f64::NAN, 0.0, right)
            .is_err()
    );
}

/// Merged torso flags retain rightward geometry across ordinary mount orientations.
#[test]
fn merged_torso_geometry_uses_right_priority() {
    for section in [
        Section::LeftArm,
        Section::RightArm,
        Section::LeftLeg,
        Section::CenterTorso,
    ] {
        for rear in [false, true] {
            for flipped in [false, true] {
                for bearing in 0..360 {
                    let facing = |torso| Facing {
                        torso,
                        arms_flipped: flipped,
                    };
                    assert_eq!(
                        mount(section, rear)
                            .bears_on(
                                stompymux_rs::MechChassis::Biped,
                                17.0,
                                f64::from(bearing),
                                facing(Torso::Both)
                            )
                            .unwrap(),
                        mount(section, rear)
                            .bears_on(
                                stompymux_rs::MechChassis::Biped,
                                17.0,
                                f64::from(bearing),
                                facing(Torso::Right)
                            )
                            .unwrap()
                    );
                }
            }
        }
    }
}

#[test]
fn contact_arc_boundaries_use_display_bearing_and_ignore_arm_flips() {
    use stompymux_rs::ContactArc as Arc;
    for (bearing, expected) in [
        (0.0, Arc::Front),
        (60.49, Arc::Front),
        (60.5, Arc::Right),
        (120.49, Arc::Right),
        (120.5, Arc::Rear),
        (239.49, Arc::Rear),
        (239.5, Arc::Left),
        (299.49, Arc::Left),
        (299.5, Arc::Front),
        (360.0, Arc::Front),
    ] {
        for arms_flipped in [false, true] {
            let facing = Facing {
                torso: Torso::Center,
                arms_flipped,
            };
            assert_eq!(facing.contact_arc(0.0, bearing).unwrap(), expected);
            assert_eq!(
                facing.contact_arc(360.0, bearing + 360.0).unwrap(),
                expected
            );
        }
    }
    assert!(Facing::default().contact_arc(f64::NAN, 0.0).is_err());
    assert!(Facing::default().contact_arc(0.0, f64::INFINITY).is_err());
}

/// Quad front legs are legs: a biped leg's forward-only geometry on the chassis heading,
/// whatever torso or arm pose is stored.
#[test]
fn quad_front_leg_geometry_is_leg_geometry() {
    use stompymux_rs::MechChassis::{Biped, Quad};
    for section in Section::ALL {
        for rear in [false, true] {
            let weapon = mount(section, rear);
            let (reference, reference_facing) = if Quad.is_leg(section) {
                (mount(Section::LeftLeg, rear), Facing::default())
            } else {
                (weapon.clone(), Facing::default())
            };
            for flipped in [false, true] {
                for torso in [Torso::Center, Torso::Left, Torso::Right, Torso::Both] {
                    let facing = Facing {
                        torso,
                        arms_flipped: flipped,
                    };
                    let expected = if Quad.is_leg(section) {
                        reference_facing
                    } else {
                        facing
                    };
                    for bearing in 0..360 {
                        assert_eq!(
                            weapon
                                .bears_on(Quad, 17.0, f64::from(bearing), facing)
                                .unwrap(),
                            reference
                                .bears_on(Biped, 17.0, f64::from(bearing), expected)
                                .unwrap(),
                            "{section:?} {rear} {facing:?} {bearing}"
                        );
                    }
                }
            }
        }
    }
    for section in [Section::LeftArm, Section::RightArm] {
        assert!(
            mount(section, false)
                .bears_on(Quad, 0.0, 60.0, Facing::default())
                .unwrap()
        );
        for bearing in [90.0, 270.0] {
            assert!(
                !mount(section, false)
                    .bears_on(Quad, 0.0, bearing, Facing::default())
                    .unwrap(),
                "{section:?} {bearing}"
            );
        }
    }
}
