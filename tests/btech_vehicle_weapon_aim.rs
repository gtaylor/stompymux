//! Vehicle weapon aim shares numeric terms without mutating combat state or authorizing fire.
use crate::support;
use stompymux_rs::*;

/// Two Mechs and two vehicles at opposite ends of a north/south lane.
async fn fixture(
    tiles: &str,
    vehicle: &str,
) -> (tempfile::TempDir, Config, World, ObjectId, [ObjectId; 4]) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Sight lane".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "sight",
        BattleMapAsset::parse(&format!("1 5\n{tiles}")).unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for index in 0..4 {
        let id = world.create(&config, format!("Unit {index}"), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        if index < 2 {
            let mut definition =
                BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap();
            definition
                .attributes
                .insert("specials".into(), "FlipArms Searchlight".into());
            create_battle_unit(&mut world, id, definition).unwrap();
        } else {
            create_battle_vehicle(
                &mut world,
                id,
                BattleVehicleTemplate::parse(vehicle).unwrap(),
            )
            .unwrap();
        }
        place_battle_unit(&mut world, id, map, 0, if index % 2 == 0 { 4 } else { 0 }).unwrap();
        ids.push(id);
    }
    (dir, config, world, map, ids.try_into().unwrap())
}

/// Assign scenario power without introducing crew actions into sensor tests.
fn power(world: &mut World, ids: &[ObjectId], value: BattlePower) {
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    for id in ids {
        let class = if world.btech.vehicles().contains_key(id) {
            "vehicles"
        } else {
            "constructed"
        };
        saved[class][id.0.to_string()]["power"] = serde_json::to_value(value).unwrap();
    }
    world.btech = serde_json::from_value(saved).unwrap();
}

/// A close mixed formation guarantees contact acquisition without a random fixture dependency.
async fn formation() -> (tempfile::TempDir, Config, World, [ObjectId; 4]) {
    let (dir, config, mut world, map, ids) = fixture(
        ".0\n.0\n.0\n.0\n.0\n",
        include_str!("../game/mechs/Demolisher"),
    )
    .await;
    for id in ids {
        place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    }
    power(&mut world, &ids, BattlePower::Running);
    (dir, config, world, ids)
}

/// Ordinary conventional aim without range extensions or arc overrides.
fn rules() -> BattleAimRules {
    BattleAimRules {
        woods_damage: false,
        dig_bonus: 3,
        dig_only_front: false,
        hit_arc_mode: 0,
        fasa_turning: false,
        extended_movement: false,
        extended_ranges: false,
        hotload_half_minimum: false,
        override_weapon_arcs: false,
    }
}

#[tokio::test]
async fn vehicle_aim_combines_mixed_targets_controls_locks_and_saved_replay() {
    let (_dir, config, initial, [mech, _, shooter, vehicle]) = formation().await;
    for target in [mech, vehicle] {
        let mut world = initial.clone();
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(shooter);
        assign_battle_pilot(&mut world, shooter, ObjectId(1)).unwrap();
        refresh_optical_scanners(&mut world, &[shooter]).unwrap();
        let before = world.btech.clone();
        let aim = battle_pilot_aim_modifiers(&world, shooter, target, 0, false, rules()).unwrap();
        assert_eq!(aim.gunnery, 6);
        assert_eq!(aim.distance, 0.0);
        assert_eq!(aim.range.unwrap().modifier, 0);
        assert_eq!(aim.target_lock, 2);
        assert_eq!(aim.attacker_movement, 0);
        assert_eq!(aim.sensors, 0);
        assert_eq!(aim.control_damage, 0);
        assert!(aim.optical.is_some());
        assert!(aim.subtotal().is_some());
        assert_eq!(world.btech, before);
        let supplied = battle_aim_modifiers(&world, shooter, target, 0, 3, rules()).unwrap();
        assert_eq!(supplied.subtotal(), aim.subtotal().map(|n| n - 3));
        select_battle_target(&mut world, shooter, ObjectId(1), Some(target)).unwrap();
        for _ in 0..8 {
            advance_battle_target_locks(&mut world);
        }
        let settled =
            battle_pilot_aim_modifiers(&world, shooter, target, 0, false, rules()).unwrap();
        assert_eq!(settled.target_lock, 0);
        assert_eq!(settled.subtotal(), aim.subtotal().map(|n| n - 2));
        damage_battle_vehicle_controls(&mut world, shooter, BattleVehicleControlHit::Sensors)
            .unwrap();
        damage_battle_vehicle_controls(
            &mut world,
            shooter,
            BattleVehicleControlHit::Stabilizers {
                section: BattleVehicleSection::Turret,
            },
        )
        .unwrap();
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        saved["vehicles"][shooter.0.to_string()]["motion"]["speed"] = serde_json::json!(10.0);
        saved["vehicles"][shooter.0.to_string()]["motion"]["desired_speed"] =
            serde_json::json!(10.0);
        world.btech = serde_json::from_value(saved).unwrap();
        let damaged =
            battle_pilot_aim_modifiers(&world, shooter, target, 0, false, rules()).unwrap();
        assert_eq!(damaged.control_damage, 1);
        assert_eq!(damaged.sensors, 0);
        assert_eq!(damaged.attacker_movement, 2);
        assert_eq!(damaged.subtotal(), settled.subtotal().map(|n| n + 3));
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            battle_pilot_aim_modifiers(&restored, shooter, target, 0, false, rules()).unwrap(),
            damaged
        );
    }
}

#[tokio::test]
async fn vehicle_aim_rechecks_contact_sensors_and_does_not_spend_candidate_dice() {
    let (_dir, _config, mut world, [target, _, shooter, _]) = formation().await;
    let before = world.btech.clone();
    assert!(
        battle_aim_modifiers(&world, shooter, target, 0, 6, rules())
            .unwrap()
            .subtotal()
            .is_none()
    );
    assert_eq!(world.btech, before);
    refresh_optical_scanners(&mut world, &[shooter]).unwrap();
    let before = world.btech.clone();
    assert!(battle_aim_modifiers(&world, shooter, target, 99, 6, rules()).is_err());
    assert_eq!(world.btech, before);
    let map = world.btech.vehicles()[&shooter].position().unwrap().map;
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["maps"][map.0.to_string()]["sensor_flags"] = serde_json::json!(1);
    world.btech = serde_json::from_value(saved).unwrap();
    let before = world.btech.clone();
    assert!(
        battle_aim_modifiers(&world, shooter, target, 0, 6, rules())
            .unwrap()
            .subtotal()
            .is_none()
    );
    assert_eq!(world.btech, before);
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][shooter.0.to_string()]["sensor_selection"]["active"]["secondary"] =
        serde_json::json!("electromagnetic");
    world.btech = serde_json::from_value(saved).unwrap();
    let before = world.btech.clone();
    let aim = battle_aim_modifiers(&world, shooter, target, 0, 6, rules()).unwrap();
    let optical = aim.optical.unwrap();
    assert_eq!(optical.sensor, BattleSensorMode::Electromagnetic);
    assert!(optical.secondary);
    assert!(aim.subtotal().is_some());
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn vehicle_aim_applies_computer_and_ammunition_accuracy_without_fire_admission() {
    let template = include_str!("../game/mechs/Demolisher").replace(
        "Front_Side\n",
        "Front_Side\n CRIT_1 { TargetingComputer - - }\n",
    );
    let (_dir, _config, mut world, map, ids) = fixture(".0\n.0\n.0\n.0\n.0\n", &template).await;
    let [target, _, shooter, _] = ids;
    for id in ids {
        place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    }
    power(&mut world, &ids, BattlePower::Running);
    refresh_optical_scanners(&mut world, &[shooter]).unwrap();
    let equipped = battle_aim_modifiers(&world, shooter, target, 0, 6, rules()).unwrap();
    assert_eq!(equipped.targeting_computer, -1);
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][shooter.0.to_string()]["ammunition_modes"]["0"] =
        serde_json::to_value(BattleAmmunitionMode::ArmorPiercing).unwrap();
    world.btech = serde_json::from_value(saved).unwrap();
    let ap = battle_aim_modifiers(&world, shooter, target, 0, 6, rules()).unwrap();
    assert_eq!(ap.ammunition_accuracy, 1);
    assert_eq!(ap.subtotal(), equipped.subtotal().map(|n| n + 1));
    assert!(
        !world.btech.vehicles()[&shooter]
            .weapon_readiness(0)
            .unwrap()
            .ready
    );
    let before = world.btech.clone();
    assert_eq!(
        battle_aim_modifiers(&world, shooter, target, 0, 6, rules()).unwrap(),
        ap
    );
    assert_eq!(world.btech, before);
    destroy_battle_vehicle_critical(
        &mut world,
        shooter,
        VehicleCriticalLocation {
            section: BattleVehicleSection::Front,
            slot: 0,
        },
    )
    .unwrap();
    let damaged = battle_aim_modifiers(&world, shooter, target, 0, 6, rules()).unwrap();
    assert_eq!(damaged.targeting_computer, 0);
    assert_eq!(damaged.subtotal(), ap.subtotal().map(|n| n + 1));
}

/// Every shooter uses the same vehicle movement and beacon terms, including after restart.
#[tokio::test]
async fn mech_and_vehicle_aim_share_vehicle_target_terms_without_spending_dice() {
    let template = include_str!("../game/mechs/Demolisher").replace("IS.AC/20", "IS.SRM-4");
    let (_dir, config, mut world, map, ids) = fixture(".0\n.0\n.0\n.0\n.0\n", &template).await;
    let [mech, _, vehicle, target] = ids;
    for id in ids {
        place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    }
    power(&mut world, &ids, BattlePower::Running);
    refresh_optical_scanners(&mut world, &[mech, vehicle]).unwrap();
    let mech_mount = world.btech.constructed_units()[&mech]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == BattleWeapon::Srm4)
        .unwrap();
    assert_eq!(
        world.btech.vehicles()[&vehicle].loadout().unwrap().weapons[0].weapon,
        BattleWeapon::Srm4
    );
    for (speed, power, expected_movement) in [
        (0.0, BattlePower::Running, 0),
        (43.001, BattlePower::Running, 2),
        (-21.501, BattlePower::Running, 1),
        (0.0, BattlePower::Off, -4),
    ] {
        for homing in [false, true] {
            let mut saved = serde_json::to_value(&world.btech).unwrap();
            let defender = &mut saved["vehicles"][target.0.to_string()];
            defender["motion"]["speed"] = serde_json::json!(speed);
            defender["power"] = serde_json::to_value(power).unwrap();
            defender["beacons"] = if homing {
                serde_json::json!({"turret":["homing"]})
            } else {
                serde_json::json!({})
            };
            for (class, shooter, index) in
                [("constructed", mech, mech_mount), ("vehicles", vehicle, 0)]
            {
                saved[class][shooter.0.to_string()]["ammunition_modes"][index.to_string()] =
                    serde_json::to_value(BattleAmmunitionMode::Narc).unwrap();
            }
            world.btech = serde_json::from_value(saved).unwrap();
            let before = world.btech.clone();
            for (shooter, index) in [(mech, mech_mount), (vehicle, 0)] {
                let aim = battle_aim_modifiers(&world, shooter, target, index, 6, rules()).unwrap();
                assert_eq!(aim.target_movement, expected_movement);
                assert_eq!(aim.beacon_accuracy, -i8::from(homing));
                assert!(aim.optical.is_some());
                assert!(aim.subtotal().is_some());
            }
            assert_eq!(world.btech, before);
        }
    }
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_aim_modifiers(&restored, mech, target, mech_mount, 6, rules()).unwrap(),
        battle_aim_modifiers(&world, mech, target, mech_mount, 6, rules()).unwrap()
    );
    world
        .objects
        .get_mut(&target)
        .unwrap()
        .flags
        .insert(Flag::Going);
    let before = world.btech.clone();
    assert!(battle_aim_modifiers(&world, mech, target, mech_mount, 6, rules()).is_err());
    assert_eq!(world.btech, before);
}

/// Infrared aims at both chassis through the same rules and ignores vehicle weapon-heat storage.
#[tokio::test]
async fn infrared_aim_uses_mixed_targets_without_spending_dice() {
    let (_dir, config, mut world, [mech, _, vehicle, _]) = formation().await;
    refresh_optical_scanners(&mut world, &[mech, vehicle]).unwrap();
    for shooter in [mech, vehicle] {
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        let class = if shooter == mech {
            "constructed"
        } else {
            "vehicles"
        };
        saved[class][shooter.0.to_string()]["sensor_selection"]["active"] =
            serde_json::to_value(BattleSensorPair {
                primary: BattleSensorMode::Infrared,
                secondary: BattleSensorMode::Infrared,
            })
            .unwrap();
        // This expenditure reservoir is distinct from production and cooling rates.
        saved["vehicles"][vehicle.0.to_string()]["weapon_heat"] = serde_json::json!(100.0);
        world.btech = serde_json::from_value(saved).unwrap();
    }
    let before = world.btech.clone();
    for (shooter, target) in [(mech, vehicle), (vehicle, mech)] {
        let aim = battle_aim_modifiers(&world, shooter, target, 0, 6, rules()).unwrap();
        let optical = aim.optical.unwrap();
        assert_eq!(optical.sensor, BattleSensorMode::Infrared);
        assert!(!optical.secondary);
        assert_eq!(optical.modifier, 2);
        assert!(aim.subtotal().is_some());
    }
    assert_eq!(world.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    for (shooter, target) in [(mech, vehicle), (vehicle, mech)] {
        assert_eq!(
            battle_aim_modifiers(&restored, shooter, target, 0, 6, rules()).unwrap(),
            battle_aim_modifiers(&world, shooter, target, 0, 6, rules()).unwrap()
        );
    }
}

/// Ammunition-fed flamer variants receive the same computer assistance on either target chassis.
#[tokio::test]
async fn explicit_vehicle_links_share_targeting_computer_aim_and_critical_loss() {
    for weapon in [
        BattleWeapon::HeavyFlamer,
        BattleWeapon::VehicleFlamer,
        BattleWeapon::VehicleHeavyFlamer,
        BattleWeapon::MachineGun,
        BattleWeapon::ClanMachineGun,
    ] {
        let template = include_str!("../game/mechs/Demolisher")
            .replace("IS.AC/20 - -", &format!("{} - OnTC", weapon.name()))
            .replace("Ammo_IS.AC/20", &format!("Ammo_{}", weapon.name()))
            .replace(
                "Front_Side\n",
                "Front_Side\n CRIT_1 { TargetingComputer - - }\n",
            );
        let (_dir, config, mut world, map, ids) = fixture(".0\n.0\n.0\n.0\n.0\n", &template).await;
        let shooter = ids[2];
        for id in ids {
            place_battle_unit(&mut world, id, map, 0, 0).unwrap();
        }
        power(&mut world, &ids, BattlePower::Running);
        refresh_optical_scanners(&mut world, &[shooter]).unwrap();
        for target in [ids[0], ids[3]] {
            let before = world.btech.clone();
            let assisted = battle_aim_modifiers(&world, shooter, target, 0, 6, rules()).unwrap();
            assert_eq!(assisted.targeting_computer, -1);
            assert_eq!(world.btech, before);
            let mut damaged = world.clone();
            destroy_battle_vehicle_critical(
                &mut damaged,
                shooter,
                VehicleCriticalLocation {
                    section: BattleVehicleSection::Front,
                    slot: 0,
                },
            )
            .unwrap();
            let unaided = battle_aim_modifiers(&damaged, shooter, target, 0, 6, rules()).unwrap();
            assert_eq!(unaided.targeting_computer, 0);
            assert_eq!(
                unaided.subtotal(),
                assisted.subtotal().map(|value| value + 1)
            );
        }
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, world.btech);
        assert_eq!(
            battle_aim_modifiers(&restored, shooter, ids[0], 0, 6, rules())
                .unwrap()
                .targeting_computer,
            -1
        );
    }
}
