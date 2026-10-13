//! Aircraft impacts share armor and critical transactions: one location roll, rotor footnotes
//! and the one-tenth weapon damage against rotors.
use crate::support;
use stompymux_rs::*;

/// Install a Kestrel variant with seeded dice directly into the world.
fn install(world: &mut World, id: ObjectId, unit: &Vehicle, seed: u8) {
    let mut saved = serde_json::to_value(unit).unwrap();
    saved["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["vehicles"][id.0.to_string()] = saved;
    world.btech = serde_json::from_value(state).unwrap();
}

#[tokio::test]
async fn aircraft_impacts_apply_location_rotor_and_armor_effects_with_saved_replay() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Aircraft impact".into(), Kind::Thing);

    for proof in [false, true] {
        let source = if proof {
            include_str!("../game/units/Kestrel.toml").replacen(
                "\"CargoTech\"",
                "\"CargoTech\", \"CritProof_Tech\"",
                1,
            )
        } else {
            include_str!("../game/units/Kestrel.toml").into()
        };
        let unit = Vehicle::new(VehicleTemplate::parse("Kestrel", &source).unwrap()).unwrap();
        for arc in [HitArc::Front, HitArc::Rear, HitArc::Left, HitArc::Right] {
            for roll in 2..=12 {
                let seed = (0..=255)
                    .find(|seed| Dice::seeded([*seed; 32]).two_d6() == roll)
                    .unwrap();
                install(&mut world, id, &unit, seed);
                let mut replay = world.clone();
                let rules = VehicleImpactRules::STANDARD;
                let selected = unit.vtol_hit(arc, roll).unwrap();
                let report =
                    resolve_battle_vehicle_impact(&mut world, id, arc, 1, None, rules).unwrap();
                assert_eq!(report.hit, Some(selected.hit));
                assert_eq!(report.rolls, vec![roll]);
                assert_eq!(
                    resolve_battle_vehicle_impact(&mut replay, id, arc, 1, None, rules).unwrap(),
                    report
                );
                assert_eq!(world.btech, replay.btech);
                let changed = &world.btech.vehicles()[&id];
                assert_eq!(report.damage.as_ref().unwrap().absorbed, 1);
                // A Rotors result costs one Cruising MP on top of its damage, and a rotor
                // critical of Rotor Damage one more.
                let rotor_criticals = report
                    .damage
                    .as_ref()
                    .unwrap()
                    .criticals
                    .iter()
                    .filter(|critical| {
                        critical.selection.effect
                            == Some(VehicleCriticalEffect::Rotor(RotorHit::Damage))
                    })
                    .count() as f64;
                assert_eq!(
                    changed.maximum_speed(),
                    193.5 - 10.75 * (f64::from(u8::from(selected.rotor)) + rotor_criticals)
                );
                assert_eq!(
                    report.damage.as_ref().unwrap().criticals.len(),
                    usize::from(selected.hit.through_armor_critical)
                );
                assert_eq!(
                    serde_json::from_value::<Vehicle>(serde_json::to_value(changed).unwrap())
                        .unwrap(),
                    *changed
                );
            }
        }
    }
}

#[tokio::test]
async fn weapon_damage_against_rotors_counts_one_point_per_ten() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Rotor divisor".into(), Kind::Thing);
    let unit = Vehicle::new(
        VehicleTemplate::parse("Kestrel", include_str!("../game/units/Kestrel.toml")).unwrap(),
    )
    .unwrap();
    let seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 3)
        .unwrap();
    for (amount, scaled) in [(1, 1), (9, 1), (10, 1), (11, 2), (25, 3)] {
        install(&mut world, id, &unit, seed);
        let report = resolve_battle_vehicle_impact(
            &mut world,
            id,
            HitArc::Front,
            amount,
            None,
            VehicleImpactRules::STANDARD,
        )
        .unwrap();
        assert_eq!(report.hit.unwrap().section, VehicleSection::Rotor);
        let damage = report.damage.as_ref().unwrap();
        assert_eq!(damage.incoming, scaled);
        assert_eq!(damage.absorbed, scaled.min(2) as u16);
        assert_eq!(world.btech.vehicles()[&id].maximum_speed(), 182.75);
    }
}

#[tokio::test]
async fn non_weapon_damage_against_rotors_is_not_reduced() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Rotor armor".into(), Kind::Thing);
    let unit = Vehicle::new(
        VehicleTemplate::parse("Kestrel", include_str!("../game/units/Kestrel.toml")).unwrap(),
    )
    .unwrap();
    install(&mut world, id, &unit, 0);
    let rules = VehicleImpactRules::STANDARD.criticals;
    let report = resolve_battle_vehicle_armor_damage(
        &mut world,
        id,
        VehicleArmorHit {
            damage_class: DamageClass::Ordinary,
            section: VehicleSection::Rotor,
            amount: 9,
            through_armor_critical: false,
            armor_piercing: None,
        },
        rules,
    )
    .unwrap();
    assert_eq!(report.incoming, 9);
    assert_eq!(report.absorbed, 2);
    assert_eq!(report.overflow, 7);
}

#[tokio::test]
async fn safe_aircraft_impacts_preserve_material_and_rejected_input_rolls_back() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Protected aircraft".into(), Kind::Thing);
    let unit = Vehicle::new(
        VehicleTemplate::parse("Kestrel", include_str!("../game/units/Kestrel.toml")).unwrap(),
    )
    .unwrap();
    install(&mut world, id, &unit, 7);
    let before = world.btech.clone();
    let mut rules = VehicleImpactRules::STANDARD;
    assert!(resolve_battle_vehicle_impact(&mut world, id, HitArc::Front, 0, None, rules).is_err());
    assert_eq!(world.btech, before);
    rules.criticals.combat_safe = true;
    let report =
        resolve_battle_vehicle_impact(&mut world, id, HitArc::Left, 1, None, rules).unwrap();
    assert!(report.hit.is_none());
    assert_eq!(report.rolls.len(), 1);
    let changed = &world.btech.vehicles()[&id];
    assert_eq!(changed.sections(), unit.sections());
    assert_eq!(changed.lost_criticals(), unit.lost_criticals());
    assert_eq!(changed.maximum_speed(), unit.maximum_speed());
    assert_eq!(changed.vtol_flight(), unit.vtol_flight());
}
