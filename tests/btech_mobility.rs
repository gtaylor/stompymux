//! Derived damage limits and their application to current movement and persistent controls.
use stompymux_rs::{
    BattleDamagePhase as Phase, BattleSection as Section, BattleTemplate, BattleUnit,
    CriticalLocation,
};

/// Reference Jenner with intact conventional biped equipment.
fn unit() -> BattleUnit {
    BattleUnit::from_template(
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap()
}

#[test]
fn hips_override_same_leg_actuators_and_hit_order_does_not_accumulate_penalties() {
    let mut a = unit();
    let mut b = a.clone();
    let hip = CriticalLocation {
        section: Section::LeftLeg,
        slot: 0,
    };
    let upper = CriticalLocation {
        section: Section::LeftLeg,
        slot: 1,
    };
    a.destroy_critical(upper).unwrap();
    assert_eq!(a.mobility().maximum_speed, 107.5);
    assert_eq!(a.mobility().piloting_modifier, 1);
    a.destroy_critical(hip).unwrap();
    b.destroy_critical(hip).unwrap();
    b.destroy_critical(upper).unwrap();
    assert_eq!(a.mobility(), b.mobility());
    assert_eq!(a.mobility().maximum_speed, 59.125);
    assert_eq!(a.mobility().piloting_modifier, 2);
    a.destroy_critical(CriticalLocation {
        section: Section::RightLeg,
        slot: 0,
    })
    .unwrap();
    assert_eq!(a.mobility().maximum_speed, 0.0);
    assert_eq!(a.mobility().piloting_modifier, 4);
}

#[test]
fn missing_legs_and_gyro_losses_have_distinct_mobility_and_piloting_effects() {
    let mut mech = unit();
    mech.damage_phase(Section::LeftLeg, 8, Phase::Internal);
    assert_eq!(mech.mobility().maximum_speed, 10.75);
    assert_eq!(mech.mobility().piloting_modifier, 5);
    mech.damage_phase(Section::RightLeg, 8, Phase::Internal);
    assert_eq!(mech.mobility().maximum_speed, 0.0);
    assert_eq!(mech.mobility().piloting_modifier, 10);
    let mut mech = unit();
    mech.destroy_critical(CriticalLocation {
        section: Section::CenterTorso,
        slot: 3,
    })
    .unwrap();
    assert_eq!(mech.mobility().maximum_speed, 118.25);
    assert_eq!(mech.mobility().piloting_modifier, 3);
    mech.destroy_critical(CriticalLocation {
        section: Section::CenterTorso,
        slot: 4,
    })
    .unwrap();
    assert_eq!(mech.mobility().maximum_speed, 0.0);
    assert!(!mech.is_destroyed());
    let mut mech = unit();
    mech.destroy_critical(CriticalLocation {
        section: Section::LeftArm,
        slot: 0,
    })
    .unwrap();
    assert_eq!(mech.mobility().maximum_speed, 118.25);
}

/// Construct the unchanged Scorpion asset for chassis-specific scenarios.
fn quad() -> BattleUnit {
    BattleUnit::from_template(
        BattleTemplate::parse("SCP-1N", include_str!("../game/mechs/SCP-1N.toml")).unwrap(),
    )
    .unwrap()
}

/// Every combination of missing or flooded support follows quad counts, not biped arm rules.
#[test]
fn quad_leg_loss_patterns_and_flooded_support_have_distinct_limits() {
    let quad = quad();
    let legs = quad.chassis().legs();
    for mask in 0u8..16 {
        let missing = mask.count_ones();
        for flooded in [false, true] {
            let mut encoded = serde_json::to_value(&quad).unwrap();
            let mut floods = Vec::new();
            for (i, leg) in legs.iter().enumerate() {
                if mask & (1 << i) == 0 {
                    continue;
                }
                if flooded {
                    floods.push(*leg);
                } else {
                    let key = serde_json::to_value(leg).unwrap();
                    encoded["sections"][key.as_str().unwrap()]["internal"] = 0.into();
                }
            }
            encoded["flooded_sections"] = serde_json::to_value(floods).unwrap();
            let damaged: BattleUnit = serde_json::from_value(encoded).unwrap();
            assert_eq!(damaged.validate_charge_support().is_ok(), missing <= 1);
            assert_eq!(damaged.unavailable_legs(), missing as usize);
            assert_eq!(damaged.airborne_support_lost(), missing >= 3);
            match missing {
                0 => assert!(!damaged.stand_requires_roll().unwrap()),
                1 | 2 => assert!(damaged.stand_requires_roll().unwrap()),
                _ => assert!(damaged.stand_requires_roll().is_err()),
            }
            let mobility = damaged.mobility();
            let (speed, penalty) = match missing {
                0 => (96.75, 0),
                1 => (86.0, 2),
                2 => (10.75, 7),
                _ => (0.0, 2),
            };
            assert_eq!(
                (mobility.maximum_speed, mobility.piloting_modifier),
                (speed, penalty),
                "mask={mask} flooded={flooded}"
            );
            assert_eq!(
                i16::from(penalty) + damaged.chassis().piloting_modifier(),
                match missing {
                    0 => -2,
                    2 => 5,
                    _ => 0,
                }
            );
        }
    }
}

/// Four hip hits continue to halve speed, and a hip replaces the same leg's lesser penalties.
#[test]
fn quad_actuators_hips_and_gyro_preserve_order_and_restart_calculation() {
    let quad = quad();
    let legs = quad.chassis().legs();
    for count in 1..=4 {
        let mut encoded = serde_json::to_value(&quad).unwrap();
        let losses: Vec<_> = legs[..count]
            .iter()
            .flat_map(|&section| {
                [
                    CriticalLocation { section, slot: 0 },
                    CriticalLocation { section, slot: 1 },
                ]
            })
            .collect();
        encoded["lost_criticals"] = serde_json::to_value(losses).unwrap();
        let damaged: BattleUnit = serde_json::from_value(encoded.clone()).unwrap();
        assert_eq!(
            damaged.mobility().maximum_speed,
            96.75 / 2f64.powi(count as i32)
        );
        assert_eq!(damaged.mobility().piloting_modifier, count as u8 * 2);
        let restored: BattleUnit =
            serde_json::from_value(serde_json::to_value(&damaged).unwrap()).unwrap();
        assert_eq!(restored.mobility(), damaged.mobility());
        assert!(!restored.airborne_support_lost());
        assert!(!restored.stand_requires_roll().unwrap());
        encoded["lost_criticals"].as_array_mut().unwrap().extend([
            serde_json::to_value(CriticalLocation {
                section: Section::CenterTorso,
                slot: 3,
            })
            .unwrap(),
            serde_json::to_value(CriticalLocation {
                section: Section::CenterTorso,
                slot: 4,
            })
            .unwrap(),
        ]);
        let disabled: BattleUnit = serde_json::from_value(encoded).unwrap();
        assert_eq!(disabled.mobility().maximum_speed, 0.0);
        assert!(disabled.stand_requires_roll().is_err());
    }
    for &section in legs {
        let mut encoded = serde_json::to_value(&quad).unwrap();
        encoded["lost_criticals"] =
            serde_json::to_value([1, 2, 3].map(|slot| CriticalLocation { section, slot })).unwrap();
        let damaged: BattleUnit = serde_json::from_value(encoded).unwrap();
        assert_eq!(
            (
                damaged.mobility().maximum_speed,
                damaged.mobility().piloting_modifier
            ),
            (64.5, 3)
        );
    }
    assert_eq!(quad.chassis().piloting_skill(true), "Piloting-Quad");
    assert_eq!(quad.chassis().piloting_skill(false), "Piloting-Battlemech");
    assert_eq!(unit().chassis().piloting_skill(true), "Piloting-Biped");
}

use crate::support;

/// Shallow water cools surviving sinks in all four quad legs, including the front pair.
#[tokio::test]
async fn quad_shallow_water_counts_front_leg_sinks_and_excludes_flooded_equipment() {
    use stompymux_rs::*;
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Water".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "water.map",
        MapAsset::from_cells("1 1\n~1\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let mut encoded = serde_json::to_value(quad()).unwrap();
    encoded["position"] = serde_json::to_value(BattlePosition { map, x: 0, y: 0 }).unwrap();
    encoded["ground_elevation"] = (-1).into();
    for section in [Section::LeftArm, Section::RightArm] {
        let key = serde_json::to_value(section).unwrap();
        for slot in [4, 5] {
            encoded["definition"]["sections"][key.as_str().unwrap()]["criticals"]
                [slot.to_string()] = serde_json::to_value(CriticalDefinition {
                equipment: "HeatSink".into(),
                data: "-".into(),
                modes: vec![],
            })
            .unwrap();
        }
    }
    let full: BattleUnit = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(full.heat_rates(&world).dissipation, 14.0);
    encoded["flooded_sections"] = serde_json::to_value([Section::LeftArm]).unwrap();
    let flooded: BattleUnit = serde_json::from_value(encoded).unwrap();
    // Flooding removes two base cooling points as well as their immersion bonus.
    assert_eq!(flooded.heat_rates(&world).dissipation, 10.0);
    assert_eq!(flooded.mobility().maximum_speed, 86.0);
}

/// Quad acceleration and lateral speed are derived separately from throttle and facing.
#[test]
fn quad_acceleration_lateral_speed_and_leg_loss_cancel_active_offset() {
    use stompymux_rs::*;
    let world = World::default();
    let base = quad();
    assert_eq!(base.ground_acceleration(&world), 9.675);
    assert_eq!(unit().ground_acceleration(&world), 5.9125);
    let mut encoded = serde_json::to_value(&base).unwrap();
    encoded["lateral"] = serde_json::to_value(BattleLateralState {
        active: BattleLateralMode::FrontLeft,
        ..Default::default()
    })
    .unwrap();
    let moving: BattleUnit = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(moving.lateral_speed(96.75), 86.0);
    assert!((moving.lateral_speed(-64.5) + 64.5 * 86.0 / 96.75).abs() < 1e-10);
    assert_eq!(unit().lateral_speed(-64.5), -64.5);
    for pending in [
        None,
        Some(BattleLateralMode::None),
        Some(BattleLateralMode::FrontRight),
    ] {
        encoded["lateral"] = serde_json::to_value(BattleLateralState {
            active: BattleLateralMode::FrontLeft,
            pending,
            remaining: if pending.is_some() { 3 } else { 0 },
        })
        .unwrap();
        let mut damaged: BattleUnit = serde_json::from_value(encoded.clone()).unwrap();
        damaged.damage_phase(Section::LeftArm, 100, Phase::Internal);
        assert_eq!(damaged.lateral().active, BattleLateralMode::None);
        assert_eq!(
            damaged.lateral().pending,
            pending.filter(|mode| *mode != BattleLateralMode::None)
        );
        let saved: BattleUnit =
            serde_json::from_value(serde_json::to_value(&damaged).unwrap()).unwrap();
        assert_eq!(saved.lateral(), damaged.lateral());
    }
}

/// Biped hips and missing legs retain their separate standing and landing rules.
#[test]
fn biped_support_and_quad_gyro_standing_rules() {
    let mut biped = unit();
    assert!(biped.stand_requires_roll().unwrap());
    assert!(biped.validate_charge_support().is_ok());
    assert!(!biped.airborne_support_lost());
    for section in [Section::LeftLeg, Section::RightLeg] {
        biped
            .destroy_critical(CriticalLocation { section, slot: 0 })
            .unwrap();
    }
    assert!(biped.airborne_support_lost());
    assert!(biped.stand_requires_roll().unwrap());
    biped.damage_phase(Section::LeftLeg, 100, Phase::Internal);
    assert!(biped.validate_charge_support().is_err());
    assert!(biped.stand_requires_roll().unwrap());
    biped.damage_phase(Section::RightLeg, 100, Phase::Internal);
    assert!(biped.stand_requires_roll().is_err());
    let mut encoded = serde_json::to_value(quad()).unwrap();
    encoded["lost_criticals"] = serde_json::to_value([CriticalLocation {
        section: Section::CenterTorso,
        slot: 3,
    }])
    .unwrap();
    let damaged: BattleUnit = serde_json::from_value(encoded).unwrap();
    assert!(!damaged.stand_requires_roll().unwrap());
}

/// Quad kicks use front legs, allow one missing support and reject hip damage or arm attacks.
#[test]
fn quad_physical_support_uses_selected_front_leg_and_surviving_hips() {
    use stompymux_rs::{BattleArm, BattleArmAttack, BattleLeg, BattlePhysicalAttack as Attack};
    let base = quad();
    for (leg, section) in [
        (BattleLeg::Left, Section::LeftArm),
        (BattleLeg::Right, Section::RightArm),
    ] {
        for attack in [Attack::Kick { leg }, Attack::Trip { leg }] {
            assert_eq!(attack.section(base.chassis()), section);
            assert!(attack.validate_chassis_support(&base).is_ok());
            for mask in 0u8..16 {
                let mut encoded = serde_json::to_value(&base).unwrap();
                let floods: Vec<_> = base
                    .chassis()
                    .legs()
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| mask & (1 << i) != 0)
                    .map(|(_, s)| *s)
                    .collect();
                let expected = floods.len() <= 1 && !floods.contains(&section);
                encoded["flooded_sections"] = serde_json::to_value(&floods).unwrap();
                let damaged: BattleUnit = serde_json::from_value(encoded).unwrap();
                assert_eq!(attack.validate_chassis_support(&damaged).is_ok(), expected);
            }
            for &hip in base.chassis().legs() {
                let mut encoded = serde_json::to_value(&base).unwrap();
                encoded["lost_criticals"] = serde_json::to_value([CriticalLocation {
                    section: hip,
                    slot: 0,
                }])
                .unwrap();
                let damaged: BattleUnit = serde_json::from_value(encoded).unwrap();
                assert!(attack.validate_chassis_support(&damaged).is_err());
            }
        }
    }
    for arm in [BattleArm::Left, BattleArm::Right] {
        for attack in [
            Attack::Punch { arm },
            Attack::Weapon {
                arm,
                weapon: BattleArmAttack::Axe,
            },
            Attack::Weapon {
                arm,
                weapon: BattleArmAttack::Flail,
            },
            Attack::Club,
        ] {
            assert!(attack.validate_chassis_support(&base).is_err());
            assert!(attack.validate_chassis_support(&unit()).is_ok());
        }
    }
}

/// Front-leg weapon accuracy uses leg actuator rules, including feet and hip precedence.
#[test]
fn quad_mounting_modifiers_use_all_four_leg_roles() {
    for &section in quad().chassis().legs() {
        for (slots, expected) in [
            (vec![1], 1),
            (vec![2], 1),
            (vec![3], 1),
            (vec![1, 2, 3], 3),
            (vec![0, 1, 2, 3], 0),
        ] {
            let mut encoded = serde_json::to_value(quad()).unwrap();
            encoded["lost_criticals"] = serde_json::to_value(
                slots
                    .into_iter()
                    .map(|slot| CriticalLocation { section, slot })
                    .collect::<Vec<_>>(),
            )
            .unwrap();
            let damaged: BattleUnit = serde_json::from_value(encoded).unwrap();
            assert_eq!(damaged.mounting_modifier(section), expected);
        }
    }
}

/// Intact prone quads need no prop; damaged ones follow the reference's two-leg cutoff and front props.
#[test]
fn quad_prone_mount_support_covers_all_sections_and_loss_patterns() {
    let mut encoded = serde_json::to_value(quad()).unwrap();
    encoded["posture"] = serde_json::to_value(stompymux_rs::BattlePosture::Prone).unwrap();
    for section in Section::ALL {
        let key = serde_json::to_value(section).unwrap();
        let mut part = encoded["definition"]["sections"]["LeftArm"]["criticals"]["0"].clone();
        part["equipment"] = "IS.MediumLaser".into();
        encoded["definition"]["sections"][key.as_str().unwrap()]["criticals"]["5"] = part;
    }
    for mask in 0u8..16 {
        let mut state = encoded.clone();
        for (i, &leg) in quad().chassis().legs().iter().enumerate() {
            if mask & (1 << i) != 0 {
                let key = serde_json::to_value(leg).unwrap();
                state["sections"][key.as_str().unwrap()]["internal"] = 0.into();
            }
        }
        let unit: BattleUnit = serde_json::from_value(state).unwrap();
        for (index, mount) in unit.loadout().unwrap().weapons.iter().enumerate() {
            let section = mount.criticals[0].section;
            let expected = match mask.count_ones() {
                0 => true,
                3.. => false,
                _ => match section {
                    Section::LeftLeg | Section::RightLeg => false,
                    Section::LeftArm => mask & 2 == 0,
                    Section::RightArm => mask & 1 == 0,
                    _ => mask & 3 != 3,
                },
            };
            assert_eq!(
                unit.weapon_readiness(index).unwrap().posture_ready,
                expected,
                "mask={mask} {section:?}"
            );
        }
    }
}

/// Quad armor and compact weapon exports use chassis labels without changing snapshot state.
#[tokio::test]
async fn quad_status_labels_front_and_rear_legs() {
    use stompymux_rs::*;
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Quad display".into(), Kind::Thing);
    let mut unit = serde_json::to_value(quad()).unwrap();
    let mut part = unit["definition"]["sections"]["LeftArm"]["criticals"]["0"].clone();
    part["equipment"] = "IS.MediumLaser".into();
    unit["definition"]["sections"]["LeftArm"]["criticals"]["5"] = part;
    let mut encoded = serde_json::to_value(&world).unwrap();
    encoded["btech"]["constructed"][id.0.to_string()] = unit;
    let world: World = serde_json::from_value(encoded).unwrap();
    let before = serde_json::to_value(&world).unwrap();
    let armor = battle_unit_status(&world, id, "a").unwrap();
    assert!(armor.contains("FRONT") && armor.contains("INTERNAL"));
    let weapons = battle_unit_status(&world, id, "w").unwrap();
    for name in ["FLLEG", "FRLEG", "RLLEG", "RRLEG"] {
        assert!(weapons.contains(name), "{weapons}");
    }
    let export = battle_unit_status(&world, id, "nw").unwrap();
    assert!(export.contains("MediumLaser|Front_Left_Leg"), "{export}");
    assert_eq!(before, serde_json::to_value(&world).unwrap());
}

/// Bootlegger readiness and penalties include both front legs and reject any unavailable support.
#[test]
fn quad_bootlegger_uses_four_legs_for_damage_and_recovery() {
    let base = quad();
    assert_eq!(base.bootlegger_leg_modifier().unwrap(), 0);
    for &section in base.chassis().legs() {
        for slots in [vec![1, 2, 3], vec![0, 1, 2, 3]] {
            let mut encoded = serde_json::to_value(&base).unwrap();
            encoded["lost_criticals"] = serde_json::to_value(
                slots
                    .iter()
                    .map(|&slot| CriticalLocation { section, slot })
                    .collect::<Vec<_>>(),
            )
            .unwrap();
            let unit: BattleUnit = serde_json::from_value(encoded).unwrap();
            assert_eq!(
                unit.bootlegger_leg_modifier().unwrap(),
                if slots.contains(&0) { 0 } else { 3 }
            );
        }
        for reason in ["flooded", "destroyed", "recovering"] {
            let mut encoded = serde_json::to_value(&base).unwrap();
            let key = serde_json::to_value(section).unwrap();
            match reason {
                "flooded" => encoded["flooded_sections"] = serde_json::to_value([section]).unwrap(),
                "destroyed" => encoded["sections"][key.as_str().unwrap()]["internal"] = 0.into(),
                _ => encoded["limb_recycle"][key.as_str().unwrap()] = 30.into(),
            }
            let unit: BattleUnit = serde_json::from_value(encoded).unwrap();
            assert!(
                unit.bootlegger_leg_modifier().is_err(),
                "{section:?} {reason}"
            );
        }
    }
}
