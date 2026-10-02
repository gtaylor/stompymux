//! Vehicle launch rolls and expenditure are atomic and preserve saved random-stream ordering.
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

/// Install deterministic shooter dice while keeping the rest of construction unchanged.
fn seed(world: &mut World, id: ObjectId, value: u8) {
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([value; 32])).unwrap();
    world.btech = serde_json::from_value(saved).unwrap();
}

/// Give every slot entry for `item` in a template document the listed modes.
fn with_modes(template: &str, item: &str, modes: &[&str]) -> String {
    let modes = modes
        .iter()
        .map(|mode| format!("\"{mode}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let pattern = format!("item = \"{item}\"");
    assert!(template.contains(&pattern), "{item} is not in the template");
    template
        .lines()
        .map(|line| match line.find(&pattern) {
            Some(start) => {
                let end = start + pattern.len();
                format!(
                    "{}{}, modes = [{modes}]{}",
                    &line[..start],
                    pattern,
                    &line[end..]
                )
            }
            None => line.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// The Demolisher with its turret AC/20 replaced by `weapon`, including the turret ammunition.
fn demolisher_with(weapon: &str) -> String {
    include_str!("../game/mechs/Demolisher.toml").replace("IS.AC/20", weapon)
}

/// Ordinary admitted direct-shot inputs; the launch stage is independent of target damage.
fn request(id: ObjectId) -> BattleVehicleLaunchRequest {
    BattleVehicleLaunchRequest {
        shooter: id,
        pilot: ObjectId(1),
        weapon_index: 0,
        distance: 1.0,
        target_number: Some(6),
        streak_confused: false,
        glancing: BattleGlancingMode::Disabled,
        critical_rules: BattleVehicleCriticalRules {
            rotor_damage_divisor: 0,
            extended_piloting: false,
            vtol_table: None,
            table: BattleVehicleCriticalTable::Standard,
            enabled: false,
            combat_safe: false,
            toughness: false,
        },
    }
}

#[tokio::test]
async fn vehicle_launch_glancing_misses_and_out_of_range_attempts_replay_expenditure() {
    let (_dir, config, mut base, id) = fixture(include_str!("../game/mechs/Demolisher.toml")).await;
    seed(&mut base, id, 17);
    for kind in ["distance", "glancing", "pilot"] {
        let mut world = base.clone();
        let before = world.btech.clone();
        let mut invalid = request(id);
        match kind {
            "distance" => invalid.distance = f64::NAN,
            "glancing" => {
                invalid.target_number = Some(i32::MIN);
                invalid.glancing = BattleGlancingMode::BelowTarget;
            }
            "pilot" => invalid.pilot = ObjectId(2),
            _ => unreachable!(),
        }
        assert!(launch_battle_vehicle_weapon(&mut world, invalid).is_err());
        assert_eq!(world.btech, before);
    }
    let mut dice = BattleDice::seeded([17; 32]);
    let roll = dice.two_d6();
    for (mode, target, hit, glanced) in [
        (
            BattleGlancingMode::Disabled,
            Some(i32::from(roll)),
            true,
            false,
        ),
        (
            BattleGlancingMode::AtTarget,
            Some(i32::from(roll)),
            true,
            true,
        ),
        (
            BattleGlancingMode::BelowTarget,
            Some(i32::from(roll) + 1),
            true,
            true,
        ),
        (BattleGlancingMode::Disabled, Some(13), false, false),
        (BattleGlancingMode::Disabled, None, false, false),
    ] {
        let mut world = base.clone();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        let mut request = request(id);
        request.glancing = mode;
        request.target_number = target;
        let report = launch_battle_vehicle_weapon(&mut world, request).unwrap();
        assert_eq!(
            (report.roll, report.hit, report.glancing),
            (roll, hit, glanced)
        );
        assert!(report.expenditure.launched);
        assert_eq!(
            report.expenditure.heat,
            report.expenditure.weapon.profile().heat
        );
        assert_eq!(
            world.btech.vehicles()[&id].weapon_heat(),
            f64::from(report.expenditure.heat)
        );
        assert_eq!(
            report
                .expenditure
                .ammunition
                .iter()
                .map(|draw| draw.rounds)
                .sum::<u16>(),
            1
        );
        assert_eq!(
            launch_battle_vehicle_weapon(&mut restored, request).unwrap(),
            report
        );
        assert_eq!(world.btech, restored.btech);
        let before = world.btech.clone();
        assert!(launch_battle_vehicle_weapon(&mut world, request).is_err());
        assert_eq!(world.btech, before);
        assert_eq!(
            roll_unit_dice(&mut world, id, 1).unwrap(),
            [dice.clone().d6()]
        );
    }
}

#[tokio::test]
async fn vehicle_streak_failure_recycles_without_ammunition_and_confusion_allows_misses() {
    let (_dir, _config, mut base, id) =
        fixture(include_str!("../game/mechs/Svantovit-Streak.toml")).await;
    seed(&mut base, id, 17);
    let index = base.btech.vehicles()[&id]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon.is_streak())
        .unwrap();
    let mut dice = BattleDice::seeded([17; 32]);
    let roll = dice.two_d6();
    for (target, confused, launched, hit) in [
        (Some(13), false, false, false),
        (None, false, false, false),
        (Some(13), true, true, false),
        (Some(i32::from(roll)), false, true, true),
    ] {
        let mut world = base.clone();
        let before = world.btech.vehicles()[&id].ammunition().to_vec();
        let mut request = request(id);
        request.weapon_index = index;
        request.target_number = target;
        request.streak_confused = confused;
        request.glancing = BattleGlancingMode::AtTarget;
        let report = launch_battle_vehicle_weapon(&mut world, request).unwrap();
        assert_eq!(report.roll, roll);
        assert_eq!(report.hit, hit);
        assert_eq!(report.expenditure.launched, launched);
        assert_eq!(world.btech.vehicles()[&id].fired_recently(), launched);
        let heat = if launched {
            report.expenditure.weapon.profile().heat
        } else {
            0
        };
        assert_eq!(report.expenditure.heat, heat);
        assert_eq!(world.btech.vehicles()[&id].weapon_heat(), f64::from(heat));
        assert!(!report.glancing);
        assert!(
            world.btech.vehicles()[&id]
                .weapon_recycle()
                .contains_key(&index)
        );
        if !launched {
            assert!(report.expenditure.ammunition.is_empty());
            assert_eq!(world.btech.vehicles()[&id].ammunition(), before);
        }
    }
}

#[tokio::test]
async fn vehicle_gatling_preparation_precedes_attack_and_burst_supply_falls_back() {
    let template = with_modes(
        &demolisher_with("IS.MachineGun"),
        "IS.MachineGun",
        &["Gattling"],
    );
    let (_dir, _config, mut world, id) = fixture(&template).await;
    seed(&mut world, id, 17);
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][id.0.to_string()]["ammunition"] = serde_json::json!([5, 0, 0, 0]);
    world.btech = serde_json::from_value(saved).unwrap();
    let mut dice = BattleDice::seeded([17; 32]);
    dice.d6();
    let expected_roll = dice.two_d6();
    let report = launch_battle_vehicle_weapon(&mut world, request(id)).unwrap();
    assert_eq!(report.roll, expected_roll);
    assert_eq!(report.expenditure.gatling_damage, Some(1));
    assert_eq!(report.expenditure.heat, 1);
    assert_eq!(world.btech.vehicles()[&id].weapon_heat(), 1.0);
    assert_eq!(
        report
            .expenditure
            .ammunition
            .iter()
            .map(|draw| draw.rounds)
            .sum::<u16>(),
        3
    );
    assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
    let template = with_modes(
        include_str!("../game/mechs/Demolisher.toml"),
        "IS.AC/20",
        &["RapidFire"],
    );
    let (_dir, _config, mut world, id) = fixture(&template).await;
    seed(&mut world, id, 17);
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][id.0.to_string()]["ammunition"] = serde_json::json!([1, 0, 0, 0]);
    world.btech = serde_json::from_value(saved).unwrap();
    let report = launch_battle_vehicle_weapon(&mut world, request(id)).unwrap();
    assert_eq!(report.expenditure.fire_mode, BattleFireMode::Normal);
    assert_eq!(
        report
            .expenditure
            .ammunition
            .iter()
            .map(|draw| draw.rounds)
            .sum::<u16>(),
        1
    );
    assert_eq!(
        world.btech.vehicles()[&id].fire_mode(0).unwrap(),
        BattleFireMode::Normal
    );
}

#[tokio::test]
async fn vehicle_ultra_launches_and_loader_loss_replay_without_affecting_other_mounts() {
    let template = with_modes(
        &demolisher_with("IS.UltraAC/2"),
        "IS.UltraAC/2",
        &["UltraMode"],
    );
    let (_dir, config, base, id) = fixture(&template).await;
    let mut covered = std::collections::BTreeSet::new();
    for value in 0..=255 {
        let mut expected = BattleDice::seeded([value; 32]);
        let roll = expected.two_d6();
        if !covered.insert(roll) {
            continue;
        }
        let mut world = base.clone();
        seed(&mut world, id, value);
        let ammunition = world.btech.vehicles()[&id].ammunition().to_vec();
        let saved = serde_json::to_value(&world.btech).unwrap();
        let mut replay = world.clone();
        replay.btech = serde_json::from_value(saved).unwrap();
        let report = launch_battle_vehicle_weapon(&mut world, request(id)).unwrap();
        assert_eq!(
            report,
            launch_battle_vehicle_weapon(&mut replay, request(id)).unwrap()
        );
        assert_eq!(world.btech, replay.btech);
        assert_eq!(report.roll, roll);
        assert_eq!(report.loader_destroyed, roll == 2);
        assert_eq!(report.expenditure.launched, roll != 2);
        assert_eq!(report.hit, roll >= 6);
        assert!(!report.glancing);
        assert_eq!(report.expenditure.fire_mode, BattleFireMode::Ultra);
        let unit = &world.btech.vehicles()[&id];
        assert!(unit.weapon_readiness(1).unwrap().ready);
        if roll == 2 {
            assert_eq!(unit.ammunition(), ammunition);
            assert!(unit.weapon_recycle().is_empty());
            assert!(!unit.weapon_readiness(0).unwrap().intact);
            assert!(!unit.is_destroyed());
            assert!(
                unit.loadout().unwrap().weapons[0]
                    .criticals
                    .iter()
                    .all(|location| unit.critical_destroyed(*location))
            );
        } else {
            assert!(unit.weapon_readiness(0).unwrap().intact);
            assert!(unit.weapon_readiness(0).unwrap().recycle_remaining > 0);
            assert_eq!(
                report
                    .expenditure
                    .ammunition
                    .iter()
                    .map(|draw| draw.rounds)
                    .sum::<u16>(),
                2
            );
        }
        let after = world.btech.clone();
        assert!(launch_battle_vehicle_weapon(&mut world, request(id)).is_err());
        assert_eq!(world.btech, after);
        world.validate(&config).unwrap();
        let restored: BtechState =
            serde_json::from_value(serde_json::to_value(&world.btech).unwrap()).unwrap();
        assert_eq!(restored, world.btech);
        assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [expected.d6()]);
    }
    assert_eq!(covered, (2..=12).collect());
}

#[tokio::test]
async fn vehicle_feed_jams_follow_burst_thresholds_and_survive_ticks_and_reload() {
    for (weapon, flag, threshold, rounds) in [
        ("IS.RotaryAC/2", "Rotary_TwoShot", 2, 2),
        ("IS.RotaryAC/2", "Rotary_FourShot", 3, 4),
        ("IS.RotaryAC/2", "Rotary_SixShot", 4, 6),
        ("IS.LRM-5", "Hotload", 3, 1),
    ] {
        let template = with_modes(&demolisher_with(weapon), weapon, &[flag]);
        let (_dir, config, base, id) = fixture(&template).await;
        let mut covered = std::collections::BTreeSet::new();
        for value in 0..=255 {
            let mut dice = BattleDice::seeded([value; 32]);
            let roll = dice.two_d6();
            if !covered.insert(roll) {
                continue;
            }
            let mut world = base.clone();
            seed(&mut world, id, value);
            let before = world.btech.vehicles()[&id].ammunition().to_vec();
            let report = launch_battle_vehicle_weapon(&mut world, request(id)).unwrap();
            assert_eq!(report.roll, roll);
            assert_eq!(report.jammed, roll <= threshold);
            assert!(!report.loader_destroyed);
            assert_eq!(report.expenditure.launched, !report.jammed);
            assert_eq!(report.hit, !report.jammed && roll >= 6);
            assert!(
                world.btech.vehicles()[&id]
                    .weapon_readiness(0)
                    .unwrap()
                    .intact
            );
            assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
            if !report.jammed {
                assert_eq!(
                    report
                        .expenditure
                        .ammunition
                        .iter()
                        .map(|draw| draw.rounds)
                        .sum::<u16>(),
                    rounds
                );
                continue;
            }
            assert!(report.expenditure.ammunition.is_empty());
            assert_eq!(world.btech.vehicles()[&id].ammunition(), before);
            assert!(world.btech.vehicles()[&id].weapon_recycle().is_empty());
            let saved = serde_json::to_value(&world.btech).unwrap();
            let mut replay = world.clone();
            replay.btech = serde_json::from_value(saved.clone()).unwrap();
            for _ in 0..130 {
                assert_eq!(
                    advance_battle_units(&mut world, 0),
                    advance_battle_units(&mut replay, 0)
                );
            }
            assert_eq!(world.btech, replay.btech);
            let ready = world.btech.vehicles()[&id].weapon_readiness(0).unwrap();
            assert!(ready.intact && ready.jammed && !ready.ready);
            assert!(
                world.btech.vehicles()[&id]
                    .weapon_readiness(1)
                    .unwrap()
                    .ready
            );
            let before = world.btech.clone();
            assert!(launch_battle_vehicle_weapon(&mut world, request(id)).is_err());
            assert_eq!(world.btech, before);
            assert!(battle_weapon_status(&world, id).unwrap().contains("jammed"));
            for index in [99, 999] {
                let mut bad = saved.clone();
                bad["vehicles"][id.0.to_string()]["jammed_weapons"] = serde_json::json!([index]);
                assert!(serde_json::from_value::<BtechState>(bad).is_err());
            }
            let location = world.btech.vehicles()[&id].loadout().unwrap().weapons[0].criticals[0];
            destroy_battle_vehicle_critical(&mut world, id, location).unwrap();
            assert!(!world.btech.vehicles()[&id].weapon_jammed(0).unwrap());
            world.validate(&config).unwrap();
            let restored: BtechState =
                serde_json::from_value(serde_json::to_value(&world.btech).unwrap()).unwrap();
            assert_eq!(restored, world.btech);
        }
        assert_eq!(covered, (2..=12).collect());
    }
}

#[tokio::test]
async fn vehicle_misloads_and_propellant_checks_replay_damage_supply_and_dice() {
    for (rapid, caseless) in [(true, false), (false, true), (true, true)] {
        let flags: &[&str] = match (rapid, caseless) {
            (true, true) => &["RapidFire", "Caseless"],
            (true, false) => &["RapidFire"],
            _ => &["Caseless"],
        };
        let mut template = with_modes(&demolisher_with("IS.AC/2"), "IS.AC/2", flags);
        if caseless {
            template = with_modes(&template, "Ammo_IS.AC/2", &["Caseless"]);
        }
        let (_dir, config, base, id) = fixture(&template).await;
        let mut covered = std::collections::BTreeSet::new();
        for value in 0..=255 {
            let mut dice = BattleDice::seeded([value; 32]);
            let roll = dice.two_d6();
            let propellant = (caseless && roll <= 3).then(|| dice.two_d6());
            if !covered.insert((roll, propellant)) {
                continue;
            }
            let destroyed = propellant.map_or(rapid && roll == 2, |roll| roll > 7);
            let jammed = propellant.map_or(rapid && (3..=4).contains(&roll), |roll| roll <= 7);
            let expected_damage_rolls = if destroyed {
                vec![dice.two_d6(), dice.two_d6()]
            } else {
                Vec::new()
            };
            let mut world = base.clone();
            seed(&mut world, id, value);
            let before_ammo: u16 = world.btech.vehicles()[&id].ammunition().iter().sum();
            let mut replay = world.clone();
            replay.btech =
                serde_json::from_value(serde_json::to_value(&world.btech).unwrap()).unwrap();
            let report = launch_battle_vehicle_weapon(&mut world, request(id)).unwrap();
            assert_eq!(
                report,
                launch_battle_vehicle_weapon(&mut replay, request(id)).unwrap()
            );
            assert_eq!(world.btech, replay.btech);
            assert_eq!(report.roll, roll);
            assert_eq!(report.propellant_roll, propellant);
            assert_eq!(report.loader_destroyed, destroyed);
            assert_eq!(report.jammed, jammed);
            assert_eq!(report.expenditure.launched, !destroyed && !jammed);
            let heat = if destroyed || jammed {
                0
            } else if rapid {
                2
            } else {
                1
            };
            assert_eq!(report.expenditure.heat, heat);
            assert_eq!(world.btech.vehicles()[&id].weapon_heat(), f64::from(heat));
            assert_eq!(report.hit, !destroyed && !jammed && roll >= 6);
            assert_eq!(report.misload.is_some(), destroyed);
            if let Some(damage) = report.misload {
                assert_eq!(damage.rolls, expected_damage_rolls);
                assert_eq!(damage.absorbed, 2);
                assert_eq!(damage.section, BattleVehicleSection::Turret);
                assert_eq!(
                    world.btech.vehicles()[&id].sections()[&BattleVehicleSection::Turret].internal,
                    6
                );
            }
            let spent = if jammed {
                0
            } else if rapid {
                2
            } else {
                1
            };
            assert_eq!(
                report
                    .expenditure
                    .ammunition
                    .iter()
                    .map(|draw| draw.rounds)
                    .sum::<u16>(),
                spent
            );
            assert_eq!(
                world.btech.vehicles()[&id].ammunition().iter().sum::<u16>(),
                before_ammo - spent
            );
            assert_eq!(
                world.btech.vehicles()[&id].weapon_recycle().is_empty(),
                destroyed || jammed
            );
            assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
            world.validate(&config).unwrap();
            let restored: BtechState =
                serde_json::from_value(serde_json::to_value(&world.btech).unwrap()).unwrap();
            assert_eq!(restored, world.btech);
        }
        assert!(covered.iter().any(|(roll, _)| *roll == 2));
        assert!(covered.iter().any(|(roll, _)| *roll == 12));
        if caseless {
            assert!(
                covered
                    .iter()
                    .any(|(_, propellant)| propellant.is_some_and(|roll| roll > 7))
            );
            assert!(
                covered
                    .iter()
                    .any(|(_, propellant)| propellant.is_some_and(|roll| roll <= 7))
            );
        }
    }
}

#[tokio::test]
async fn vehicle_misloads_clamp_destroyed_bins_and_spend_surviving_supply_after_hull_loss() {
    let seed_value = (0..=255)
        .find(|value| BattleDice::seeded([*value; 32]).two_d6() == 2)
        .unwrap();
    for front in [false, true] {
        let mut template = with_modes(
            include_str!("../game/mechs/Demolisher.toml"),
            "IS.AC/20",
            &["RapidFire"],
        );
        if front {
            template = template
                .lines()
                .filter(|line| !line.contains("item = \"IS.AC/20\""))
                .collect::<Vec<_>>()
                .join("\n")
                + "\n";
            template = template.replace(
                "[sections.front_side]\n",
                "[sections.front_side]\nslots = [{ at = 1, item = \"IS.AC/20\", modes = [\"RapidFire\"] }]\n",
            );
        }
        let (_dir, config, mut world, id) = fixture(&template).await;
        seed(&mut world, id, seed_value);
        let mut character = world.clone();
        character
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        let before_ammo: u16 = world.btech.vehicles()[&id].ammunition().iter().sum();
        let report = launch_battle_vehicle_weapon(&mut world, request(id)).unwrap();
        assert!(report.loader_destroyed && report.misload.is_some());
        assert_eq!(world.btech.vehicles()[&id].is_destroyed(), front);
        let ammo: u16 = world.btech.vehicles()[&id].ammunition().iter().sum();
        assert_eq!(ammo, if front { before_ammo - 2 } else { 0 });
        assert_eq!(
            report
                .expenditure
                .ammunition
                .iter()
                .map(|draw| draw.rounds)
                .sum::<u16>(),
            if front { 2 } else { 0 }
        );
        world.validate(&config).unwrap();
        if front {
            let character_report =
                launch_battle_vehicle_weapon(&mut character, request(id)).unwrap();
            assert_eq!(character_report, report);
            assert_eq!(character.btech, world.btech);
            assert!(!character.btech.vehicles()[&id].crew_killed());
            assert_eq!(character.objects[&ObjectId(1)].location, Some(id));
        }
    }
}

#[tokio::test]
async fn vehicle_misload_policy_and_caseless_short_supply_are_retained() {
    let template = with_modes(
        &with_modes(
            &demolisher_with("IS.AC/2"),
            "IS.AC/2",
            &["RapidFire", "Caseless"],
        ),
        "Ammo_IS.AC/2",
        &["Caseless"],
    );
    let (_dir, config, base, id) = fixture(&template).await;
    for ignition in [false, true] {
        let value = (0..=255)
            .find(|value| {
                let mut dice = BattleDice::seeded([*value; 32]);
                dice.two_d6() <= 3 && (dice.two_d6() > 7) == ignition
            })
            .unwrap();
        let mut world = base.clone();
        seed(&mut world, id, value);
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        saved["vehicles"][id.0.to_string()]["ammunition"] = serde_json::json!([1, 0, 0, 0]);
        world.btech = serde_json::from_value(saved).unwrap();
        let report = launch_battle_vehicle_weapon(&mut world, request(id)).unwrap();
        assert_eq!(report.loader_destroyed, ignition);
        assert_eq!(report.jammed, !ignition);
        assert_eq!(report.expenditure.fire_mode, BattleFireMode::Normal);
        assert_eq!(
            world.btech.vehicles()[&id].fire_mode(0).unwrap(),
            BattleFireMode::Normal
        );
        assert_eq!(
            world.btech.vehicles()[&id].ammunition()[0],
            u16::from(!ignition)
        );
        world.validate(&config).unwrap();
    }
    let template = with_modes(&demolisher_with("IS.AC/2"), "IS.AC/2", &["RapidFire"]);
    let (_dir, _config, mut base, id) = fixture(&template).await;
    let value = (0..=255)
        .find(|value| {
            let mut dice = BattleDice::seeded([*value; 32]);
            let attack = dice.two_d6();
            dice.two_d6();
            attack == 2 && dice.two_d6() >= 8
        })
        .unwrap();
    seed(&mut base, id, value);
    for safe in [false, true] {
        let mut world = base.clone();
        let mut rules = request(id);
        rules.critical_rules.enabled = true;
        rules.critical_rules.combat_safe = safe;
        let report = launch_battle_vehicle_weapon(&mut world, rules).unwrap();
        let impact = report.misload.unwrap();
        assert_eq!(impact.incoming, 2);
        assert_eq!(impact.structural_damage, 2);
        // A secondary fuel/power explosion can consume the section before the original packet.
        assert!(impact.absorbed <= 2);
        if safe {
            assert_eq!(impact.absorbed, 0);
        }
        assert_eq!(impact.criticals.is_empty(), safe);
        let consumed = report
            .expenditure
            .ammunition
            .iter()
            .map(|draw| draw.rounds)
            .sum::<u16>();
        assert!(consumed <= 2);
        if safe {
            assert_eq!(consumed, 2);
        }
        let mut replay = base.clone();
        let expected = launch_battle_vehicle_weapon(&mut replay, rules).unwrap();
        assert_eq!(expected.misload.unwrap(), impact);
        assert_eq!(world.btech, replay.btech);
    }
}
