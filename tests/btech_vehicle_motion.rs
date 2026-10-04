//! Vehicle motion proposals preserve paved-surface bonuses and shared acceleration/turning rules.
use stompymux_rs::*;

/// An intact tracked chassis with a 53.75 kph nominal flank speed.
fn template() -> VehicleTemplate {
    VehicleTemplate::parse("Demolisher", include_str!("../game/mechs/Demolisher.toml")).unwrap()
}

#[test]
fn vehicle_motion_reaches_terrain_adjusted_targets_and_brakes_without_overshoot() {
    let template = template();
    for (terrain, target) in [
        (Terrain::Grassland, 53.75),
        (Terrain::Road, 64.5),
        (Terrain::Bridge, 64.5),
        (Terrain::Rough, 26.875),
        (Terrain::Snow, 26.875),
        (Terrain::LightForest, 26.875),
        (Terrain::HeavyForest, 53.75 / 3.0),
        (Terrain::Mountains, 53.75 / 3.0),
    ] {
        let mut motion = Motion::stationary(HexCoordinate { x: 50, y: 50 }.center());
        motion.desired_speed = 53.75;
        for _ in 0..30 {
            motion = template
                .ground_motion_step(motion, Hex::new(terrain, 0), VehicleMotionRules::STANDARD)
                .unwrap();
        }
        assert_eq!(motion.speed, target, "{terrain:?}");
        motion.desired_speed = 0.0;
        for _ in 0..30 {
            motion = template
                .ground_motion_step(motion, Hex::new(terrain, 0), VehicleMotionRules::STANDARD)
                .unwrap();
        }
        assert_eq!(motion.speed, 0.0);
        motion.desired_speed = -53.75 * 2.0 / 3.0;
        for _ in 0..30 {
            motion = template
                .ground_motion_step(motion, Hex::new(terrain, 0), VehicleMotionRules::STANDARD)
                .unwrap();
        }
        assert!((motion.speed + target * 2.0 / 3.0).abs() < 1e-10);
    }
}

#[test]
fn vehicle_motion_handles_road_turning_hover_advantage_and_map_rate() {
    let mut template = template();
    let mut motion = Motion::stationary(HexCoordinate { x: 50, y: 50 }.center());
    motion.desired_speed = template.max_speed;
    let normal = template
        .ground_motion_step(
            motion,
            Hex::new(Terrain::Grassland, 0),
            VehicleMotionRules::STANDARD,
        )
        .unwrap();
    assert_eq!(normal.speed, template.max_speed / 20.0);
    let fast = template
        .ground_motion_step(
            motion,
            Hex::new(Terrain::Grassland, 0),
            VehicleMotionRules {
                speed_demon: true,
                ..VehicleMotionRules::STANDARD
            },
        )
        .unwrap();
    assert_eq!(fast.speed, normal.speed * 1.25);
    let double = template
        .ground_motion_step(
            motion,
            Hex::new(Terrain::Grassland, 0),
            VehicleMotionRules {
                movement_modifier: 200,
                ..VehicleMotionRules::STANDARD
            },
        )
        .unwrap();
    assert!(
        (motion.point.range(double.point).unwrap()
            - 2.0 * motion.point.range(normal.point).unwrap())
        .abs()
            < 1e-12
    );
    motion.speed = 64.5;
    motion.desired_heading = 90.0;
    let road = template
        .ground_motion_step(
            motion,
            Hex::new(Terrain::Road, 0),
            VehicleMotionRules::STANDARD,
        )
        .unwrap();
    assert!((road.heading - (5.0 * (1.0 - 53.75 / 64.5 / 2.0))).abs() < 1e-12);
    let fasa = template
        .ground_motion_step(
            motion,
            Hex::new(Terrain::Road, 0),
            VehicleMotionRules {
                fasa_turning: true,
                ..VehicleMotionRules::STANDARD
            },
        )
        .unwrap();
    assert_eq!(fasa.heading, 0.0);
    template.movement = VehicleMovement::Hover;
    motion.heading = 0.0;
    motion.desired_heading = 0.0;
    motion.speed = template.max_speed;
    assert_eq!(
        template
            .ground_motion_step(
                motion,
                Hex::new(Terrain::Road, 0),
                VehicleMotionRules::STANDARD
            )
            .unwrap()
            .speed,
        template.max_speed
    );
    // Retained throttle may exceed today's ceiling after gravity or mode changes.
    motion.desired_speed = template.max_speed + 1.0;
    assert!(
        template
            .ground_motion_step(
                motion,
                Hex::new(Terrain::Grassland, 0),
                VehicleMotionRules::STANDARD
            )
            .is_ok()
    );
    for speed in [f64::NAN, f64::INFINITY, 10_000.0] {
        motion.desired_speed = speed;
        assert!(
            template
                .ground_motion_step(
                    motion,
                    Hex::new(Terrain::Grassland, 0),
                    VehicleMotionRules::STANDARD
                )
                .is_err()
        );
    }
    template.movement = VehicleMovement::Stationary;
    motion.desired_speed = 1.0;
    assert!(
        template
            .ground_motion_step(
                motion,
                Hex::new(Terrain::Grassland, 0),
                VehicleMotionRules::STANDARD
            )
            .is_err()
    );
}
