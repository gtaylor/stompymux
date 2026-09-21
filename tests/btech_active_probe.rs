//! Active-probe ranges, concealment, electronic rejection and hidden-contact acquisition.
use stompymux_rs::*;

/// The three probes share interference rules while Bloodhound penetrates signature concealment.
#[test]
fn active_probe_ranges_concealment_and_aim() {
    for probe in [
        BattleActiveProbe::Beagle,
        BattleActiveProbe::Light,
        BattleActiveProbe::Bloodhound,
    ] {
        let range = f64::from(probe.range());
        for jitter in 0..=2 {
            let report = probe
                .evaluate(range, false, false, false, false, jitter)
                .unwrap();
            assert!(report.eligible);
            assert_eq!(report.acquisition_factor, 101);
            assert_eq!(report.aim_modifier, i16::from(jitter));
        }
        assert!(
            !probe
                .evaluate(range + 0.001, false, false, false, false, 0)
                .unwrap()
                .eligible
        );
        for (disturbed, protected, disabled) in [
            (true, false, false),
            (false, true, false),
            (false, false, true),
        ] {
            assert!(
                !probe
                    .evaluate(0.0, disturbed, protected, false, disabled, 0)
                    .unwrap()
                    .eligible
            );
        }
        assert_eq!(
            probe
                .evaluate(range, false, false, true, false, 0)
                .unwrap()
                .eligible,
            probe == BattleActiveProbe::Bloodhound
        );
        assert!(
            probe
                .evaluate(f64::NAN, false, false, false, false, 0)
                .is_err()
        );
        assert!(probe.evaluate(0.0, false, false, false, false, 3).is_err());
    }
}

/// Probes remove hidden penalties, while rear arcs and secondary slots still reduce acquisition probability.
#[test]
fn active_probe_hidden_detection_retains_arc_and_secondary_weighting() {
    let report = BattleActiveProbe::Beagle
        .evaluate(6.0, false, false, false, false, 0)
        .unwrap();
    let rules = BattleDetectionRules {
        arc: BattleSensorArc::Rear,
        perception: 7,
        hostile: true,
        hidden: true,
        secondary: true,
    };
    let mut hidden = BattleDice::seeded([7; 32]);
    let mut visible = hidden.clone();
    let result = report.roll_detection(&mut hidden, 6.0, rules).unwrap();
    let expected = report
        .roll_detection(
            &mut visible,
            6.0,
            BattleDetectionRules {
                hidden: false,
                ..rules
            },
        )
        .unwrap();
    assert_eq!(result, expected);
    assert_eq!(hidden, visible);
    assert!(result.roll.is_some());
    assert!(result.threshold < 5050);
    let ordinary = BattleSensorReport {
        acquisition_factor: 100,
        ..report
    };
    assert!(
        !ordinary
            .roll_detection(&mut visible, 6.0, rules)
            .unwrap()
            .detected
    );
    assert!(
        BattleSensorReport {
            acquisition_factor: 102,
            ..report
        }
        .roll_detection(&mut visible, 6.0, rules)
        .is_err()
    );
}

use crate::support;

/// Fixed-installation reach uses integer 140% boundaries; merely stopping a mobile vehicle does not qualify.
#[tokio::test]
async fn stationary_probe_ranges_use_shared_bonus_and_preserve_boundaries() {
    for (probe, equipment, ordinary, fixed) in [
        (BattleActiveProbe::Beagle, "BeagleProbe", 6, 8),
        (BattleActiveProbe::Light, "Light_BAP", 3, 4),
        (BattleActiveProbe::Bloodhound, "BloodhoundProbe", 8, 11),
    ] {
        for stationary in [false, true] {
            let (_dir, config, mut world) = support::isolated_world().await;
            let map = world.create(&config, "Probe lane".into(), Kind::Room);
            create_battle_map(
                &mut world,
                map,
                "probe",
                BattleMapAsset::parse(&format!("1 14\n{}", ".0\n".repeat(14))).unwrap(),
            )
            .unwrap();
            let observer = world.create(&config, "Probe carrier".into(), Kind::Thing);
            let target = world.create(&config, "Probe target".into(), Kind::Thing);
            let mut definition = BattleVehicleTemplate::parse(if stationary {
                include_str!("../game/mechs/RadioTower")
            } else {
                include_str!("../game/mechs/Demolisher")
            })
            .unwrap();
            let front = definition
                .sections
                .get_mut(&BattleVehicleSection::Front)
                .unwrap();
            front.criticals.clear();
            front.criticals.insert(
                0,
                CriticalDefinition {
                    equipment: equipment.into(),
                    data: "-".into(),
                    modes: vec![],
                    brand: None,
                },
            );
            create_battle_vehicle(&mut world, observer, definition).unwrap();
            create_battle_unit(
                &mut world,
                target,
                BattleTemplate::parse(include_str!("../game/mechs/JR7-D")).unwrap(),
            )
            .unwrap();
            place_battle_unit(&mut world, observer, map, 0, 0).unwrap();
            let maximum = if stationary { fixed } else { ordinary };
            for distance in [ordinary, maximum, maximum + 1] {
                place_battle_unit(&mut world, target, map, 0, distance).unwrap();
                let before = world.btech.clone();
                let report = battle_active_probe_contact(&world, observer, target, probe).unwrap();
                assert_eq!(
                    report.eligible,
                    distance <= maximum,
                    "{probe:?} fixed={stationary} distance={distance}"
                );
                assert_eq!(world.btech, before);
            }
        }
    }
}
