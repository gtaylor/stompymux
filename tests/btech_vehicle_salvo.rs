//! Vehicle salvos preserve packet ordering, defense counts, target dice and atomic damage.
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
        BattleVehicleTemplate::parse("test",template).unwrap(),
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

/// Find a deterministic stream for a specific multi-stage explosion path.
fn matching_seed(predicate: impl Fn(&mut BattleDice) -> bool) -> [u8; 32] {
    for value in 0u32..100000 {
        let mut seed = [0; 32];
        seed[..4].copy_from_slice(&value.to_le_bytes());
        if predicate(&mut BattleDice::seeded(seed)) {
            return seed;
        }
    }
    panic!("No deterministic stream matched");
}

/// Change only the victim's random stream.
fn set_seed(world: &mut World, id: ObjectId, seed: [u8; 32]) {
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["vehicles"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded(seed)).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
}

/// Explicit policy for the hit table and critical consequences.
fn rules(table: BattleVehicleCriticalTable) -> BattleVehicleImpactRules {
    BattleVehicleImpactRules {
        advanced_fire: false,
        criticals: BattleVehicleCriticalRules {
            rotor_damage_divisor: 0,
            extended_piloting: false,
            vtol_table: None,
            table,
            enabled: false,
            combat_safe: false,
            toughness: false,
        },
        hit: BattleVehicleHitRules {
            critical_mode: 1,
            critical_level: 40,
        },
    }
}

/// Ordinary direct packet inputs supplied after the firing action has admitted a hit.
fn request(weapon: BattleWeapon) -> BattleVehicleSalvoRequest {
    BattleVehicleSalvoRequest {
        range_damage: false,
        damage_penalty: 0,
        weapon,
        ammunition: BattleAmmunitionMode::Normal,
        fire_mode: BattleFireMode::Normal,
        gatling_damage: None,
        distance: 1.0,
        glancing: false,
        guidance_blocked: false,
        angel_blocked: false,
        intercepted: 0,
    }
}

#[tokio::test]
async fn vehicle_salvos_share_packet_rules_and_order_every_impact() {
    let template = include_str!("../game/mechs/Demolisher.toml")
        .replace("armor = 30\n", "armor = 200\n")
        .replace("armor = 40\n", "armor = 200\n")
        .replace("armor = 20\n", "armor = 200\n");
    let (_dir, config, base, id) = fixture(&template).await;
    let seed = matching_seed(|dice| dice.two_d6() == 12);
    for (weapon, mode, ammunition, expected) in [
        (
            BattleWeapon::Ac20,
            BattleFireMode::Normal,
            BattleAmmunitionMode::Normal,
            vec![20],
        ),
        (
            BattleWeapon::Lrm20,
            BattleFireMode::Normal,
            BattleAmmunitionMode::Normal,
            vec![5; 4],
        ),
        (
            BattleWeapon::Srm6,
            BattleFireMode::Normal,
            BattleAmmunitionMode::Normal,
            vec![2; 6],
        ),
        (
            BattleWeapon::Lbx10,
            BattleFireMode::Normal,
            BattleAmmunitionMode::Cluster,
            vec![1; 10],
        ),
        (
            BattleWeapon::UltraAc2,
            BattleFireMode::Ultra,
            BattleAmmunitionMode::Normal,
            vec![2; 2],
        ),
        (
            BattleWeapon::RotaryAc2,
            BattleFireMode::Rotary6,
            BattleAmmunitionMode::Normal,
            vec![2; 6],
        ),
    ] {
        let mut world = base.clone();
        set_seed(&mut world, id, seed);
        let mut replay = world.clone();
        replay.btech = serde_json::from_value(serde_json::to_value(&world.btech).unwrap()).unwrap();
        let mut input = request(weapon);
        input.fire_mode = mode;
        input.ammunition = ammunition;
        let report = resolve_battle_vehicle_salvo(
            &mut world,
            id,
            BattleHitArc::Front,
            input,
            rules(BattleVehicleCriticalTable::Standard),
        )
        .unwrap();
        assert_eq!(
            report
                .groups
                .iter()
                .map(|group| group.damage)
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            report.cluster_roll,
            (weapon != BattleWeapon::Ac20).then_some(12)
        );
        let mut dice = BattleDice::seeded(seed);
        if report.cluster_roll.is_some() {
            dice.two_d6();
        }
        let mut saved = serde_json::to_value(&replay.btech).unwrap();
        saved["vehicles"][id.0.to_string()]["dice"] = serde_json::to_value(dice).unwrap();
        replay.btech = serde_json::from_value(saved).unwrap();
        for (group, amount) in report.groups.iter().zip(expected) {
            let expected = resolve_battle_vehicle_impact(
                &mut replay,
                id,
                BattleHitArc::Front,
                u32::from(amount),
                None,
                rules(BattleVehicleCriticalTable::Standard),
            )
            .unwrap();
            assert_eq!(group.impact, expected);
        }
        assert_eq!(world.btech, replay.btech);
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn vehicle_salvos_apply_interception_glancing_and_streak_confusion() {
    let (_dir, _config, base, id) = fixture(include_str!("../game/mechs/Demolisher.toml")).await;
    for intercepted in [6, 20] {
        let mut world = base.clone();
        let seed = matching_seed(|dice| dice.two_d6() == 12);
        set_seed(&mut world, id, seed);
        let mut input = request(BattleWeapon::Lrm20);
        input.intercepted = intercepted;
        let report = resolve_battle_vehicle_salvo(
            &mut world,
            id,
            BattleHitArc::Front,
            input,
            rules(BattleVehicleCriticalTable::Standard),
        )
        .unwrap();
        assert_eq!(report.missiles_before_defense, Some(20));
        assert_eq!(
            report
                .groups
                .iter()
                .map(|group| group.damage)
                .collect::<Vec<_>>(),
            if intercepted == 20 {
                vec![]
            } else {
                vec![5, 5, 4]
            }
        );
        if intercepted == 20 {
            let mut dice = BattleDice::seeded(seed);
            dice.two_d6();
            assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
            assert_eq!(
                world.btech.vehicles()[&id].sections(),
                base.btech.vehicles()[&id].sections()
            );
        }
    }
    for confused in [false, true] {
        let mut world = base.clone();
        set_seed(&mut world, id, matching_seed(|dice| dice.two_d6() == 2));
        let mut input = request(BattleWeapon::StreakSrm6);
        input.angel_blocked = confused;
        let report = resolve_battle_vehicle_salvo(
            &mut world,
            id,
            BattleHitArc::Front,
            input,
            rules(BattleVehicleCriticalTable::Standard),
        )
        .unwrap();
        assert_eq!(
            report.missiles_before_defense,
            Some(if confused { 2 } else { 6 })
        );
    }
    for gatling in [false, true] {
        let mut world = base.clone();
        let mut input = request(if gatling {
            BattleWeapon::MachineGun
        } else {
            BattleWeapon::MediumLaser
        });
        input.glancing = true;
        if gatling {
            input.fire_mode = BattleFireMode::Gatling;
            input.gatling_damage = Some(5);
        }
        let report = resolve_battle_vehicle_salvo(
            &mut world,
            id,
            BattleHitArc::Front,
            input,
            rules(BattleVehicleCriticalTable::Standard),
        )
        .unwrap();
        assert_eq!(report.groups[0].damage, 3);
        assert_eq!(report.cluster_roll, None);
    }
}

#[tokio::test]
async fn vehicle_salvos_finish_after_hull_loss_and_reject_invalid_effects_atomically() {
    let (_dir, config, mut base, id) = fixture(include_str!("../game/mechs/Demolisher.toml")).await;
    let mut saved = serde_json::to_value(&base.btech).unwrap();
    saved["vehicles"][id.0.to_string()]["sections"]["front"]["armor"] = serde_json::json!(0);
    saved["vehicles"][id.0.to_string()]["sections"]["front"]["internal"] = serde_json::json!(1);
    base.btech = serde_json::from_value(saved).unwrap();
    set_seed(
        &mut base,
        id,
        matching_seed(|dice| dice.two_d6() == 12 && dice.two_d6() == 7),
    );
    let mut world = base.clone();
    let report = resolve_battle_vehicle_salvo(
        &mut world,
        id,
        BattleHitArc::Front,
        request(BattleWeapon::Srm6),
        rules(BattleVehicleCriticalTable::Standard),
    )
    .unwrap();
    assert!(world.btech.vehicles()[&id].is_destroyed());
    assert_eq!(report.groups.len(), 6);
    assert!(report.groups.iter().all(|group| group.damage == 2));
    let mut replay = base.clone();
    assert_eq!(
        resolve_battle_vehicle_salvo(
            &mut replay,
            id,
            BattleHitArc::Front,
            request(BattleWeapon::Srm6),
            rules(BattleVehicleCriticalTable::Standard)
        )
        .unwrap(),
        report
    );
    assert_eq!(replay.btech, world.btech);
    world.validate(&config).unwrap();
    for case in ["interception", "gatling", "range", "going"] {
        let mut world = base.clone();
        let mut input = request(BattleWeapon::Srm6);
        match case {
            "interception" => input.intercepted = 7,
            "gatling" => input.gatling_damage = Some(5),
            "range" => input.distance = f64::NAN,
            _ => {
                world
                    .objects
                    .get_mut(&id)
                    .unwrap()
                    .flags
                    .insert(Flag::Going);
            }
        }
        let before = world.btech.clone();
        assert!(
            resolve_battle_vehicle_salvo(
                &mut world,
                id,
                BattleHitArc::Front,
                input,
                rules(BattleVehicleCriticalTable::Standard)
            )
            .is_err()
        );
        assert_eq!(world.btech, before);
    }
}
