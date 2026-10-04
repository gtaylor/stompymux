//! Biped mount boundaries, torso offsets, flipped arms and rear-mount precedence.
use stompymux_rs::BattleMountArcs;
use stompymux_rs::{
    BattleFacing, BattleSection as Section, BattleTorso, BattleWeapon, CriticalLocation,
    WeaponMount,
};

fn mount(section: Section, rear_mount: bool) -> WeaponMount {
    WeaponMount {
        weapon: BattleWeapon::MediumLaser,
        criticals: vec![CriticalLocation { section, slot: 0 }],
        rear_mount,
        on_targeting_computer: false,
        one_shot: false,
        initially_spent: false,
        initial_ammunition_mode: stompymux_rs::BattleAmmunitionMode::Normal,
        initial_fire_mode: stompymux_rs::BattleFireMode::Normal,
    }
}

#[test]
fn torso_arm_and_rear_boundaries_are_distinct() {
    let normal = BattleFacing::default();
    for bearing in [0.0, 60.0, 300.0, 360.0] {
        assert!(
            mount(Section::CenterTorso, false)
                .bears_on(stompymux_rs::BattleMechChassis::Biped, 0.0, bearing, normal)
                .unwrap()
        );
    }
    for bearing in [60.001, 120.0, 180.0, 240.0, 299.999] {
        assert!(
            !mount(Section::CenterTorso, false)
                .bears_on(stompymux_rs::BattleMechChassis::Biped, 0.0, bearing, normal)
                .unwrap()
        );
    }
    assert!(
        mount(Section::LeftArm, false)
            .bears_on(stompymux_rs::BattleMechChassis::Biped, 0.0, 240.0, normal)
            .unwrap()
    );
    assert!(
        !mount(Section::LeftArm, false)
            .bears_on(stompymux_rs::BattleMechChassis::Biped, 0.0, 120.0, normal)
            .unwrap()
    );
    assert!(
        mount(Section::RightArm, false)
            .bears_on(stompymux_rs::BattleMechChassis::Biped, 0.0, 120.0, normal)
            .unwrap()
    );
    assert!(
        !mount(Section::RightArm, false)
            .bears_on(stompymux_rs::BattleMechChassis::Biped, 0.0, 240.0, normal)
            .unwrap()
    );
    for bearing in [120.0, 240.0] {
        assert!(
            !mount(Section::CenterTorso, true)
                .bears_on(stompymux_rs::BattleMechChassis::Biped, 0.0, bearing, normal)
                .unwrap()
        );
    }
    assert!(
        mount(Section::CenterTorso, true)
            .bears_on(stompymux_rs::BattleMechChassis::Biped, 0.0, 180.0, normal)
            .unwrap()
    );
}

#[test]
fn torso_twist_excludes_legs_and_rear_mount_overrides_arm_flipping() {
    let right = BattleFacing {
        torso: BattleTorso::Right,
        arms_flipped: false,
    };
    assert!(
        mount(Section::CenterTorso, false)
            .bears_on(stompymux_rs::BattleMechChassis::Biped, 0.0, 119.0, right)
            .unwrap()
    );
    assert!(
        !mount(Section::CenterTorso, false)
            .bears_on(stompymux_rs::BattleMechChassis::Biped, 0.0, 119.001, right)
            .unwrap()
    );
    assert!(
        !mount(Section::LeftLeg, false)
            .bears_on(stompymux_rs::BattleMechChassis::Biped, 0.0, 119.0, right)
            .unwrap()
    );
    let flipped = BattleFacing {
        torso: BattleTorso::Center,
        arms_flipped: true,
    };
    assert!(
        mount(Section::LeftArm, false)
            .bears_on(stompymux_rs::BattleMechChassis::Biped, 0.0, 180.0, flipped)
            .unwrap()
    );
    assert!(
        mount(Section::LeftArm, false)
            .bears_on(stompymux_rs::BattleMechChassis::Biped, 0.0, 270.0, flipped)
            .unwrap()
    );
    assert!(
        !mount(Section::LeftArm, true)
            .bears_on(stompymux_rs::BattleMechChassis::Biped, 0.0, 270.0, flipped)
            .unwrap()
    );
    assert!(
        !mount(Section::LeftArm, false)
            .bears_on(stompymux_rs::BattleMechChassis::Biped, 0.0, 0.0, flipped)
            .unwrap()
    );
    assert!(
        mount(Section::RightArm, false)
            .bears_on(stompymux_rs::BattleMechChassis::Biped, 0.0, 90.0, flipped)
            .unwrap()
    );
    assert!(
        mount(Section::CenterTorso, false)
            .bears_on(
                stompymux_rs::BattleMechChassis::Biped,
                350.0,
                10.0,
                BattleFacing::default()
            )
            .unwrap()
    );
    assert!(
        mount(Section::CenterTorso, false)
            .bears_on(stompymux_rs::BattleMechChassis::Biped, f64::NAN, 0.0, right)
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
                    let facing = |torso| BattleFacing {
                        torso,
                        arms_flipped: flipped,
                    };
                    assert_eq!(
                        mount(section, rear)
                            .bears_on(
                                stompymux_rs::BattleMechChassis::Biped,
                                17.0,
                                f64::from(bearing),
                                facing(BattleTorso::Both)
                            )
                            .unwrap(),
                        mount(section, rear)
                            .bears_on(
                                stompymux_rs::BattleMechChassis::Biped,
                                17.0,
                                f64::from(bearing),
                                facing(BattleTorso::Right)
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
    use stompymux_rs::BattleContactArc as Arc;
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
            let facing = BattleFacing {
                torso: BattleTorso::Center,
                arms_flipped,
            };
            assert_eq!(facing.contact_arc(0.0, bearing).unwrap(), expected);
            assert_eq!(
                facing.contact_arc(360.0, bearing + 360.0).unwrap(),
                expected
            );
        }
    }
    assert!(BattleFacing::default().contact_arc(f64::NAN, 0.0).is_err());
    assert!(
        BattleFacing::default()
            .contact_arc(0.0, f64::INFINITY)
            .is_err()
    );
}

/// Quad front legs keep their side arcs but use chassis heading even with a stored twist.
#[test]
fn quad_front_leg_geometry_ignores_torso_pose_and_preserves_mount_arcs() {
    use stompymux_rs::BattleMechChassis::{Biped, Quad};
    for section in Section::ALL {
        for rear in [false, true] {
            let weapon = mount(section, rear);
            for flipped in [false, true] {
                let centered = BattleFacing {
                    torso: BattleTorso::Center,
                    arms_flipped: flipped,
                };
                for torso in [BattleTorso::Left, BattleTorso::Right, BattleTorso::Both] {
                    let facing = BattleFacing {
                        torso,
                        arms_flipped: flipped,
                    };
                    for bearing in 0..360 {
                        let expected = if Quad.is_leg(section) {
                            centered
                        } else {
                            facing
                        };
                        assert_eq!(
                            weapon
                                .bears_on(Quad, 17.0, f64::from(bearing), facing)
                                .unwrap(),
                            weapon
                                .bears_on(Biped, 17.0, f64::from(bearing), expected)
                                .unwrap(),
                            "{section:?} {rear} {facing:?} {bearing}"
                        );
                    }
                }
            }
        }
    }
    assert!(
        mount(Section::LeftArm, false)
            .bears_on(Quad, 0.0, 270.0, BattleFacing::default())
            .unwrap()
    );
    assert!(
        mount(Section::RightArm, false)
            .bears_on(Quad, 0.0, 90.0, BattleFacing::default())
            .unwrap()
    );
    assert!(
        !mount(Section::LeftArm, false)
            .bears_on(Quad, 0.0, 90.0, BattleFacing::default())
            .unwrap()
    );
}
