//! Shot outcomes compared with and without validation shortcuts, plus failure-atomicity checks.
use super::*;
use crate::{BattleDice, BattleMapAsset, BattlePower, BattleUnitTemplate, Config, Kind, ObjectId};
use std::sync::Arc;

fn fixture(source: &str, recipient: &str, seed: u8) -> (Config, World, ObjectId, ObjectId) {
    let config = Config::load("tests/fixtures/game").unwrap();
    let mut world = World::default();
    let map = world.create(&config, "Shot lane".into(), Kind::Room);
    crate::create_battle_map(
        &mut world,
        map,
        "shot-lane",
        BattleMapAsset::parse(&format!("1 12\n{}", ".0\n".repeat(12))).unwrap(),
    )
    .unwrap();
    let shooter = world.create(&config, "Shooter".into(), Kind::Thing);
    let target = world.create(&config, "Recipient".into(), Kind::Thing);
    for (id, template, y, team) in [(shooter, source, 8, 1), (target, recipient, 5, 2)] {
        BattleUnitTemplate::parse("unit", template)
            .unwrap()
            .create(&mut world, id)
            .unwrap();
        crate::place_battle_unit(&mut world, id, map, 0, y).unwrap();
        if let Some(unit) = world.btech.constructed.get_mut(&id) {
            unit.power = BattlePower::Running;
            unit.dice = BattleDice::seeded([seed; 32]);
            unit.signature.team = team;
        } else {
            let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
            vehicle.power = BattlePower::Running;
            vehicle.dice = BattleDice::seeded([seed; 32]);
            vehicle.signature.team = team;
        }
    }
    world
        .btech
        .controllers
        .insert(shooter, crate::btech::autopilot::AutopilotController::new());
    crate::refresh_battle_contacts(&mut world, &[shooter]).unwrap();
    crate::btech::targeting::select_target_autopilot(&mut world, shooter, Some(target)).unwrap();
    if matches!(seed, 17 | 42) {
        if let Some(unit) = world.btech.constructed.get_mut(&target) {
            for section in unit.sections.values_mut() {
                section.armor = 0;
                section.rear = 0;
                section.internal = 1;
            }
        } else {
            for section in world
                .btech
                .vehicles
                .get_mut(&target)
                .unwrap()
                .sections
                .values_mut()
            {
                section.armor = 0;
                section.rear = 0;
                section.internal = 1;
            }
        }
    }
    if seed == 88 {
        let pilot = world.create(&config, "Pilot".into(), Kind::Player);
        world.objects.get_mut(&pilot).unwrap().location = Some(shooter);
        crate::assign_battle_pilot(&mut world, shooter, pilot).unwrap();
    }
    world.btech.validate(&world).unwrap();
    (config, world, shooter, target)
}

/// Serialize the complete report, ordered notices and world after each attempt.
fn fire(
    config: &Config,
    world: &mut World,
    shooter: ObjectId,
    target: ObjectId,
    index: usize,
) -> Result<(serde_json::Value, serde_json::Value), String> {
    let rules = crate::btech::BattleShotRules::configured(&config.battletech, false);
    if world.btech.constructed_units().contains_key(&shooter) {
        let result = if let Some(pilot) = world.btech.constructed_units()[&shooter].pilot() {
            crate::btech::shot::resolve_shot(world, shooter, pilot, target, index, rules)
        } else {
            crate::btech::shot::resolve_shot_autopilot(world, shooter, target, index, rules)
        };
        result
            .map(|r| {
                (
                    serde_json::to_value(&r).unwrap(),
                    serde_json::to_value(r.notices()).unwrap(),
                )
            })
            .map_err(|e| format!("{e:#}"))
    } else {
        let vehicle_rules = crate::btech::BattleVehicleShotRules {
            shot: rules,
            shooter_criticals: crate::btech::BattleVehicleImpactRules::configured(
                &config.battletech,
                false,
            )
            .criticals,
        };
        let result = if let Some(pilot) = world.btech.vehicles()[&shooter].pilot() {
            crate::btech::vehicle_fire::fire_vehicle_shot(
                world,
                shooter,
                pilot,
                target,
                index,
                vehicle_rules,
            )
        } else {
            crate::btech::vehicle_fire::fire_vehicle_shot_autopilot(
                world,
                shooter,
                target,
                index,
                vehicle_rules,
            )
        };
        result
            .map(|r| {
                (
                    serde_json::to_value(&r).unwrap(),
                    serde_json::to_value(r.notices()).unwrap(),
                )
            })
            .map_err(|e| format!("{e:#}"))
    }
}

#[test]
fn shot_transactions_match_reference_across_chassis_and_rejections() {
    let mech = include_str!("../../game/mechs/JR7-D.toml");
    let tracked = include_str!("../../game/mechs/Demolisher.toml");
    let mut accepted = 0;
    let mut rejected = 0;
    let mut hits = 0;
    for source in [
        mech.to_owned(),
        tracked.to_owned(),
        tracked.replace("movement = \"track\"", "movement = \"wheel\""),
        tracked.replace("movement = \"track\"", "movement = \"hover\""),
    ] {
        for recipient in [mech, tracked] {
            for seed in [3, 17, 42, 88] {
                let (config, initial, shooter, target) = fixture(&source, recipient, seed);
                let mut optimized = initial.clone();
                let mut reference = initial;
                let _equipment = crate::btech::equipment_context::Scope::begin(&optimized.btech);
                let _cache = crate::btech::validation_context::Scope::begin(&optimized.btech);
                let mounts = if let Some(unit) = optimized.btech.constructed_units().get(&shooter) {
                    unit.loadout().unwrap().weapons.len()
                } else {
                    optimized.btech.vehicles()[&shooter]
                        .loadout()
                        .unwrap()
                        .weapons
                        .len()
                };
                for index in (0..mounts).chain([0, usize::MAX]) {
                    let a = fire(&config, &mut optimized, shooter, target, index);
                    let b = {
                        let _scope = ReferenceScope::begin();
                        let _uncached = crate::btech::validation_context::Scope::disabled();
                        let _equipment = crate::btech::equipment_context::Scope::disabled();
                        fire(&config, &mut reference, shooter, target, index)
                    };
                    assert_eq!(a, b, "seed {seed}, mount {index}");
                    match &a {
                        Ok((report, _)) => {
                            accepted += 1;
                            hits += usize::from(!report["salvo"].is_null());
                        }
                        Err(_) => rejected += 1,
                    }
                    assert_eq!(
                        optimized.battle_roll_statistics().unwrap(),
                        reference.battle_roll_statistics().unwrap()
                    );
                    assert_eq!(
                        serde_json::to_value(&optimized).unwrap(),
                        serde_json::to_value(&reference).unwrap()
                    );
                }
            }
        }
    }
    assert!(accepted > 0 && rejected > 0 && hits > 0);
}

#[test]
fn shot_transactions_discard_expenditure_damage_and_validation_failures() {
    for source in [
        include_str!("../../game/mechs/JR7-D.toml"),
        include_str!("../../game/mechs/Demolisher.toml"),
    ] {
        for recipient in [
            include_str!("../../game/mechs/JR7-D.toml"),
            include_str!("../../game/mechs/Demolisher.toml"),
        ] {
            let (config, initial, shooter, target) = fixture(source, recipient, 42);
            for point in [
                FailurePoint::Expenditure,
                FailurePoint::Damage,
                FailurePoint::Validation,
            ] {
                for reference in [false, true] {
                    let mut world = initial.clone();
                    let _reference = reference.then(ReferenceScope::begin);
                    let _equipment = crate::btech::equipment_context::Scope::begin(&world.btech);
                    let _cache = crate::btech::validation_context::Scope::begin(&world.btech);
                    world.btech.validate(&world).unwrap();
                    assert!(crate::btech::validation_context::retained().1 > 0);
                    let _failure = FailureScope::begin(point);
                    let error = fire(&config, &mut world, shooter, target, 0).unwrap_err();
                    assert!(error.contains("Injected shot failure"), "{error}");
                    assert_eq!(crate::btech::validation_context::retained(), (0, 0));
                    assert_eq!(crate::btech::equipment_context::retained(), (0, 0));
                    assert_eq!(
                        world.battle_roll_statistics().unwrap(),
                        initial.battle_roll_statistics().unwrap()
                    );
                    assert_eq!(
                        serde_json::to_value(&world).unwrap(),
                        serde_json::to_value(&initial).unwrap()
                    );
                }
            }
        }
    }
}

/// Local reuse must not bypass unrelated world references or changed local inputs.
#[test]
fn validation_reuse_matches_full_checks_after_mutations_and_scope_exit() {
    use crate::btech::validation_context::{Scope, retained};
    let (_, initial, shooter, target) = fixture(
        include_str!("../../game/mechs/JR7-D.toml"),
        include_str!("../../game/mechs/JR7-D.toml"),
        42,
    );
    let scope = Scope::begin(&initial.btech);
    initial.btech.validate(&initial).unwrap();
    assert_eq!(retained(), (2, 1));
    for mutation in 0..30 {
        let mut changed = initial.clone();
        let map = changed.btech.constructed_units()[&shooter]
            .position()
            .unwrap()
            .map;
        match mutation % 10 {
            0 => {
                changed
                    .btech
                    .constructed
                    .get_mut(&target)
                    .unwrap()
                    .hide_elapsed = Some(u16::from(mutation as u8) * 10)
            }
            1 => {
                changed
                    .btech
                    .constructed
                    .get_mut(&shooter)
                    .unwrap()
                    .hide_elapsed = Some(101)
            }
            2 => changed.objects.get_mut(&target).unwrap().location = None,
            3 => {
                Arc::make_mut(&mut changed.btech.registrations).remove(&target);
            }
            4 => {
                Arc::make_mut(&mut changed.btech.tows).insert(shooter, ObjectId(99999));
            }
            5 => {
                let unit = changed.btech.constructed.get_mut(&shooter).unwrap();
                let contact = unit.contacts.remove(&target).unwrap();
                unit.contacts.insert(ObjectId(99999), contact);
            }
            6 => changed.btech.maps.get_mut(&map).unwrap().width = 0,
            8 => {
                let map = changed.btech.maps.get_mut(&map).unwrap();
                Arc::make_mut(map.terrain.as_mut().unwrap())[0].elevation = 10;
            }
            9 => changed.btech.maps.get_mut(&map).unwrap().temperature = 128,
            _ => {
                let actor = changed.create(
                    &Config::load("tests/fixtures/game").unwrap(),
                    "Crew".into(),
                    Kind::Player,
                );
                changed.objects.get_mut(&actor).unwrap().location = Some(shooter);
                crate::assign_battle_pilot(&mut changed, shooter, actor).unwrap();
                changed.objects.get_mut(&actor).unwrap().location = Some(map);
            }
        }
        let cached = changed
            .btech
            .validate(&changed)
            .map_err(|e| format!("{e:#}"));
        let full = {
            let _reference = ReferenceScope::begin();
            let _disabled = Scope::disabled();
            changed
                .btech
                .validate(&changed)
                .map_err(|e| format!("{e:#}"))
        };
        assert_eq!(cached, full, "mutation {mutation}");
        initial.btech.validate(&initial).unwrap();
        assert_eq!(retained(), (2, 1));
    }
    drop(scope);
    assert_eq!(retained(), (0, 0));
}

#[test]
fn special_shot_effects_match_reference_validation() {
    let mech = include_str!("../../game/mechs/JR7-D.toml");
    let vehicle = include_str!("../../game/mechs/Demolisher.toml");
    let cases = [
        (
            mech.replace(
                "item = \"Ammo_IS.SRM-4\", rounds = 25 }",
                "item = \"Ammo_IS.SRM-4\", rounds = 25, modes = [\"Inferno\"] }",
            ),
            crate::BattleWeapon::Srm4,
        ),
        (
            vehicle.replace(
                "item = \"IS.AC/20\" }",
                "item = \"IS.Flamer\", modes = [\"Heat\"] }",
            ),
            crate::BattleWeapon::Flamer,
        ),
        (
            include_str!("../../game/mechs/AS7-S2.toml").to_owned(),
            crate::BattleWeapon::HeavyGaussRifle,
        ),
    ];
    for (source, weapon) in cases {
        for target_template in [mech, vehicle] {
            let (config, mut initial, shooter, target) = fixture(&source, target_template, 3);
            let index = if let Some(unit) = initial.btech.constructed_units().get(&shooter) {
                unit.loadout()
                    .unwrap()
                    .weapons
                    .iter()
                    .position(|mount| mount.weapon == weapon)
                    .unwrap()
            } else {
                initial.btech.vehicles()[&shooter]
                    .loadout()
                    .unwrap()
                    .weapons
                    .iter()
                    .position(|mount| mount.weapon == weapon)
                    .unwrap()
            };
            if weapon == crate::BattleWeapon::Srm4 {
                initial
                    .btech
                    .constructed
                    .get_mut(&shooter)
                    .unwrap()
                    .ammunition_modes
                    .insert(index, crate::BattleAmmunitionMode::Inferno);
            }
            let mut optimized = initial.clone();
            let mut reference = initial;
            let _equipment = crate::btech::equipment_context::Scope::begin(&optimized.btech);
            let _cache = crate::btech::validation_context::Scope::begin(&optimized.btech);
            let a = fire(&config, &mut optimized, shooter, target, index);
            let b = {
                let _reference = ReferenceScope::begin();
                let _disabled = crate::btech::validation_context::Scope::disabled();
                let _equipment = crate::btech::equipment_context::Scope::disabled();
                fire(&config, &mut reference, shooter, target, index)
            };
            assert!(a.is_ok(), "{weapon:?}: {a:?}");
            assert_eq!(a, b);
            assert_eq!(
                serde_json::to_value(&optimized).unwrap(),
                serde_json::to_value(&reference).unwrap()
            );
            assert_eq!(
                optimized.battle_roll_statistics().unwrap(),
                reference.battle_roll_statistics().unwrap()
            );
        }
    }
}

#[test]
fn validation_projection_requires_exact_equipment_and_roster_membership() {
    use crate::btech::validation_context::{Scope, cached_loadout, retained, unit};
    let (_, world, shooter, _) = fixture(
        include_str!("../../game/mechs/JR7-D.toml"),
        include_str!("../../game/mechs/JR7-D.toml"),
        3,
    );
    let _scope = Scope::begin(&world.btech);
    world.btech.validate(&world).unwrap();
    let original = &world.btech.constructed_units()[&shooter];
    let projection = original.loadout().unwrap();
    assert_eq!(cached_loadout(shooter, original), Some(projection.clone()));
    let mut definition = original.definition().clone();
    for section in definition.sections.values_mut() {
        for critical in section.criticals.values_mut() {
            if critical.equipment.ends_with("MediumLaser") {
                critical.equipment = critical.equipment.replace("MediumLaser", "Flamer");
            }
        }
    }
    let replacement = crate::BattleUnit::from_template(definition).unwrap();
    assert!(cached_loadout(shooter, &replacement).is_none());
    unit(shooter, &replacement).unwrap();
    let changed = cached_loadout(shooter, &replacement).unwrap();
    assert_ne!(changed, projection);
    assert_eq!(changed, replacement.loadout().unwrap());
    assert!(cached_loadout(shooter, original).is_none());
    unit(ObjectId(99999), original).unwrap();
    assert!(cached_loadout(ObjectId(99999), original).is_none());
    assert_eq!(retained(), (2, 1));
}

#[test]
fn contact_position_index_handles_missing_unplaced_sparse_and_duplicate_records() {
    use crate::btech::validation_contacts::Positions;
    let (_, mut world, shooter, target) = fixture(
        include_str!("../../game/mechs/JR7-D.toml"),
        include_str!("../../game/mechs/Demolisher.toml"),
        3,
    );
    let positions = Positions::prepare(&world.btech).unwrap();
    assert_eq!(
        positions.get(shooter),
        Some(world.btech.constructed_units()[&shooter].position())
    );
    assert_eq!(
        positions.get(target),
        Some(world.btech.vehicles()[&target].position())
    );
    assert_eq!(positions.get(ObjectId(i64::MIN)), None);
    assert_eq!(positions.get(ObjectId(i64::MAX)), None);
    let unit = world.btech.constructed_units()[&shooter].clone();
    let unplaced = crate::BattleUnit::from_template(unit.definition().clone()).unwrap();
    world
        .btech
        .constructed
        .insert(ObjectId(target.0 + 1), unplaced);
    assert_eq!(
        Positions::prepare(&world.btech)
            .unwrap()
            .get(ObjectId(target.0 + 1)),
        Some(None)
    );
    let duplicate = world.btech.vehicles()[&target].clone();
    world.btech.vehicles.insert(shooter, duplicate);
    assert_eq!(
        Positions::prepare(&world.btech).unwrap().get(shooter),
        Some(world.btech.vehicles()[&target].position())
    );
    world
        .btech
        .constructed
        .insert(ObjectId(i64::MAX), unit.clone());
    assert!(Positions::prepare(&world.btech).is_none());
    world.btech.constructed.insert(ObjectId(i64::MIN), unit);
    assert!(Positions::prepare(&world.btech).is_none());
}

#[test]
fn missile_defenses_match_reference_for_both_chassis() {
    let mech = include_str!("../../game/mechs/JR7-D.toml");
    let vehicle = include_str!("../../game/mechs/Goblin-58.toml");
    let mut defenses = 0;
    for source in [mech, vehicle] {
        for recipient in [include_str!("../../game/mechs/Daishi-A.toml"), vehicle] {
            let (config, mut initial, shooter, target) = fixture(source, recipient, 3);
            // Guarantee an admitted missile hit so every pairing exercises defense expenditure.
            let dice = (0u8..=255)
                .map(|seed| BattleDice::seeded([seed; 32]))
                .find(|dice| dice.clone().two_d6() == 12)
                .unwrap();
            if let Some(unit) = initial.btech.constructed.get_mut(&shooter) {
                unit.dice = dice;
            } else {
                initial.btech.vehicles.get_mut(&shooter).unwrap().dice = dice;
            }

            if let Some(unit) = initial.btech.constructed.get_mut(&target) {
                unit.ams_enabled = true;
            } else {
                initial.btech.vehicles.get_mut(&target).unwrap().ams_enabled = true;
            }
            let index = if let Some(unit) = initial.btech.constructed_units().get(&shooter) {
                unit.loadout()
                    .unwrap()
                    .weapons
                    .iter()
                    .position(|mount| mount.weapon == crate::BattleWeapon::Srm4)
                    .unwrap()
            } else {
                initial.btech.vehicles()[&shooter]
                    .loadout()
                    .unwrap()
                    .weapons
                    .iter()
                    .position(|mount| mount.weapon == crate::BattleWeapon::Srm6)
                    .unwrap()
            };
            let mut optimized = initial.clone();
            let mut reference = initial;
            let _equipment = crate::btech::equipment_context::Scope::begin(&optimized.btech);
            let _cache = crate::btech::validation_context::Scope::begin(&optimized.btech);
            let a = fire(&config, &mut optimized, shooter, target, index);
            let b = {
                let _reference = ReferenceScope::begin();
                let _disabled = crate::btech::validation_context::Scope::disabled();
                let _equipment = crate::btech::equipment_context::Scope::disabled();
                fire(&config, &mut reference, shooter, target, index)
            };
            let (report, _) = a.as_ref().unwrap();
            defenses += usize::from(!report["ams"].is_null());
            assert_eq!(a, b);
            assert_eq!(
                serde_json::to_value(&optimized).unwrap(),
                serde_json::to_value(&reference).unwrap()
            );
            assert_eq!(
                optimized.battle_roll_statistics().unwrap(),
                reference.battle_roll_statistics().unwrap()
            );
        }
    }
    assert_eq!(defenses, 4, "each chassis pairing must exercise AMS");
}
