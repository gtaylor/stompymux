//! Ground-vehicle critical table outcomes, suppression and saved random stream replay.
use crate::support;
use stompymux_rs::*;

/// A running vehicle with a present pilot, initially disconnected from a session.
async fn fixture(template: &str) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Test field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        BattleMapAsset::parse("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        BattleVehicleTemplate::parse("test", template).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    (dir, config, world, id)
}

/// Seed only the selected vehicle's dice, keeping all material and equipment state unchanged.
fn seed(world: &mut World, id: ObjectId, value: u8) {
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["vehicles"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([value; 32])).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
}

#[tokio::test]
async fn advanced_ground_tables_cover_every_face_and_roll() {
    use BattleVehicleSection as S;
    let (_dir, _config, base, id) = fixture(include_str!("../game/mechs/Demolisher.toml")).await;
    let rules = BattleVehicleCriticalRules {
        rotor_damage_divisor: 0,
        extended_piloting: false,
        vtol_table: None,
        toughness: false,
        table: BattleVehicleCriticalTable::Advanced,
        enabled: true,
        combat_safe: false,
    };
    for (section, golden) in [
        (
            S::Front,
            "driver weapon_jam stabilizer sensors commander weapon_destroyed crew_killed",
        ),
        (
            S::Left,
            "cargo weapon_jam crew_stunned stabilizer weapon_destroyed engine fuel_tank",
        ),
        (
            S::Right,
            "cargo weapon_jam crew_stunned stabilizer weapon_destroyed engine fuel_tank",
        ),
        (
            S::Rear,
            "weapon_jam cargo stabilizer weapon_destroyed engine ammunition fuel_tank",
        ),
        (
            S::Turret,
            "stabilizer turret_jam weapon_jam turret_lock weapon_destroyed turret_blown_off ammunition",
        ),
    ] {
        let golden: Vec<_> = golden.split_whitespace().collect();
        let mut seen = std::collections::BTreeSet::new();
        for value in 0..=255 {
            let mut world = base.clone();
            seed(&mut world, id, value);
            let mut dice = BattleDice::seeded([value; 32]);
            let roll = dice.two_d6();
            seen.insert(roll);
            let report = roll_battle_vehicle_critical(&mut world, id, section, rules).unwrap();
            assert_eq!(report.rolls, vec![roll]);
            assert_eq!(report.section, section);
            assert_eq!(report.table, rules.table);
            let effect = serde_json::to_value(report.effect).unwrap();
            assert_eq!(
                effect,
                if roll < 6 {
                    serde_json::Value::Null
                } else {
                    serde_json::json!(golden[usize::from(roll - 6)])
                }
            );
            assert_eq!(
                world.btech.vehicles()[&id].sections(),
                base.btech.vehicles()[&id].sections()
            );
            assert_eq!(
                roll_unit_dice(&mut world, id, 2).unwrap(),
                vec![dice.d6(), dice.d6()]
            );
        }
        assert_eq!(seen, (2..=12).collect());
    }
}

#[tokio::test]
async fn standard_branches_preserve_suppression_and_replay() {
    use BattleVehicleCriticalEffect as E;
    use BattleVehicleCriticalTable as T;
    let (_dir, config, base, id) = fixture(include_str!("../game/mechs/Demolisher.toml")).await;
    for table in [T::Standard] {
        for section in [BattleVehicleSection::Front, BattleVehicleSection::Turret] {
            for damaged in [false, true] {
                for value in 0..32 {
                    let mut world = base.clone();
                    if damaged {
                        damage_battle_vehicle_motive(
                            &mut world,
                            id,
                            BattleVehicleMotiveHit::Immobilize,
                        )
                        .unwrap();
                        lock_battle_vehicle_turret(&mut world, id).unwrap();
                    }
                    seed(&mut world, id, value);
                    let rules = BattleVehicleCriticalRules {
                        rotor_damage_divisor: 0,
                        extended_piloting: false,
                        vtol_table: None,
                        toughness: false,
                        table,
                        enabled: true,
                        combat_safe: false,
                    };
                    let mut dice = BattleDice::seeded([value; 32]);
                    let mut draws = Vec::new();
                    let early = if table == T::Standard {
                        let turret = section == BattleVehicleSection::Turret;
                        let roll = dice.die(if turret { 3 } else { 10 }).unwrap() as u8;
                        draws.push(roll);
                        if turret && roll == 2 {
                            Some((!damaged).then_some(E::TurretLock))
                        } else if !turret && roll <= 5 {
                            Some((!damaged).then_some(if roll == 5 {
                                E::Immobilize
                            } else {
                                E::MotiveSpeedLoss
                            }))
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    let effect = early.unwrap_or_else(|| {
                        let roll = dice.d6();
                        draws.push(roll);
                        Some(
                            [
                                E::CrewHit,
                                E::MainWeaponJam,
                                E::Engine,
                                E::CrewKilled,
                                E::FuelTank,
                                E::PowerPlant,
                            ][usize::from(roll - 1)],
                        )
                    });
                    let report =
                        roll_battle_vehicle_critical(&mut world, id, section, rules).unwrap();
                    assert_eq!(report.effect, effect);
                    assert_eq!(report.rolls, draws);
                    assert_eq!(
                        roll_unit_dice(&mut world, id, 2).unwrap(),
                        vec![dice.d6(), dice.d6()]
                    );
                }
            }
        }
    }
    let mut world = base;
    let rules = BattleVehicleCriticalRules {
        rotor_damage_divisor: 0,
        extended_piloting: false,
        vtol_table: None,
        toughness: false,
        table: T::Advanced,
        enabled: true,
        combat_safe: false,
    };
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    for _ in 0..5 {
        assert_eq!(
            roll_battle_vehicle_critical(&mut world, id, BattleVehicleSection::Rear, rules)
                .unwrap(),
            roll_battle_vehicle_critical(&mut loaded, id, BattleVehicleSection::Rear, rules)
                .unwrap()
        );
        assert_eq!(loaded.btech, world.btech);
    }
}

#[tokio::test]
async fn disabled_safe_critproof_and_stationary_criticals_preserve_draw_order() {
    use BattleVehicleCriticalTable as T;
    assert_eq!(T::from_settings(true), T::Advanced);
    assert_eq!(T::from_settings(false), T::Standard);
    for (template, critproof, stationary) in [
        (
            include_str!("../game/mechs/Demolisher.toml").to_owned(),
            false,
            false,
        ),
        (
            support::templates::with_flags(
                include_str!("../game/mechs/Demolisher.toml"),
                &["CritProof_Tech"],
            ),
            true,
            false,
        ),
        (
            include_str!("../game/mechs/Demolisher.toml")
                .replace("movement = \"track\"", "movement = \"none\"")
                .replace("walk_mp = 5", "walk_mp = 0"),
            false,
            true,
        ),
    ] {
        let (_dir, _config, base, id) = fixture(&template).await;
        for table in [T::Standard, T::Advanced] {
            for (enabled, combat_safe) in [(false, false), (true, true), (true, false)] {
                if !critproof && !stationary && enabled && !combat_safe {
                    continue;
                }
                let mut world = base.clone();
                seed(&mut world, id, 0);
                let before = world.btech.clone();
                let mut dice = BattleDice::seeded([0; 32]);
                let rolls = if stationary
                    && !critproof
                    && enabled
                    && !combat_safe
                    && table == T::Advanced
                {
                    vec![dice.two_d6()]
                } else {
                    Vec::new()
                };
                let report = roll_battle_vehicle_critical(
                    &mut world,
                    id,
                    BattleVehicleSection::Front,
                    BattleVehicleCriticalRules {
                        rotor_damage_divisor: 0,
                        extended_piloting: false,
                        vtol_table: None,
                        toughness: false,
                        table,
                        enabled,
                        combat_safe,
                    },
                )
                .unwrap();
                assert!(report.effect.is_none());
                assert_eq!(report.rolls, rolls);
                if rolls.is_empty() {
                    assert_eq!(world.btech, before);
                }
                assert_eq!(
                    roll_unit_dice(&mut world, id, 2).unwrap(),
                    vec![dice.d6(), dice.d6()]
                );
            }
        }
    }
}
