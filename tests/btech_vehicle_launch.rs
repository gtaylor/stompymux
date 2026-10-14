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
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        VehicleTemplate::parse("test", template).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0, true);
    }
    (dir, config, world, id)
}

/// Install deterministic shooter dice while keeping the rest of construction unchanged.
fn seed(world: &mut World, id: ObjectId, value: u8) {
    world
        .btech
        .set_unit_dice(id, Dice::seeded([value; 32]))
        .unwrap();
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
    include_str!("../game/units/Demolisher.toml").replace("IS.AC/20", weapon)
}

/// Ordinary admitted direct-shot inputs; the launch stage is independent of target damage.
fn request(id: ObjectId) -> VehicleLaunchRequest {
    VehicleLaunchRequest {
        shooter: id,
        pilot: ObjectId(1),
        weapon_index: 0,
        distance: 1.0,
        target_number: Some(6),
        streak_confused: false,
        critical_rules: VehicleCriticalRules {
            extended_piloting: false,
            combat_safe: false,
            toughness: false,
        },
    }
}

#[tokio::test]
async fn vehicle_launch_glancing_misses_and_out_of_range_attempts_replay_expenditure() {
    let (_dir, config, mut base, id) = fixture(include_str!("../game/units/Demolisher.toml")).await;
    seed(&mut base, id, 17);
    for kind in ["distance", "pilot"] {
        let mut world = base.clone();
        let before = world.btech.clone();
        let mut invalid = request(id);
        match kind {
            "distance" => invalid.distance = f64::NAN,
            "pilot" => invalid.pilot = ObjectId(2),
            _ => unreachable!(),
        }
        assert!(launch_battle_vehicle_weapon(&mut world, invalid).is_err());
        assert_eq!(world.btech, before);
    }
    let mut dice = Dice::seeded([17; 32]);
    let roll = dice.two_d6();
    // Tactical Operations glancing blows: a roll exactly equal to the target number glances.
    for (target, hit, glanced) in [
        (Some(i32::from(roll)), true, true),
        (Some(i32::from(roll) - 1), true, false),
        (Some(13), false, false),
        (None, false, false),
    ] {
        let mut world = base.clone();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        let mut request = request(id);
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
async fn vehicle_streak_failure_recycles_without_ammunition_even_when_confused() {
    let (_dir, _config, mut base, id) =
        fixture(include_str!("../game/units/Svantovit-Streak.toml")).await;
    seed(&mut base, id, 17);
    let index = base.btech.vehicles()[&id]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon.is_streak())
        .unwrap();
    let mut dice = Dice::seeded([17; 32]);
    let roll = dice.two_d6();
    // A confused Streak launcher still refuses to launch on a failed roll (TacOps: AUE p. 91).
    for (target, confused, launched, hit) in [
        (Some(13), false, false, false),
        (None, false, false, false),
        (Some(13), true, false, false),
        (Some(i32::from(roll)), false, true, true),
        (Some(i32::from(roll)), true, true, true),
    ] {
        let mut world = base.clone();
        let before = world.btech.vehicles()[&id].ammunition().to_vec();
        let mut request = request(id);
        request.weapon_index = index;
        request.target_number = target;
        request.streak_confused = confused;
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
        // A confused launcher resolves as a standard launcher, so its boundary roll glances.
        assert_eq!(report.glancing, confused && hit);
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
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["ammunition"] = serde_json::json!([5, 0, 0, 0]);
        })
        .unwrap();
    let mut dice = Dice::seeded([17; 32]);
    dice.d6();
    let expected_roll = dice.two_d6();
    let report = launch_battle_vehicle_weapon(&mut world, request(id)).unwrap();
    assert_eq!(report.roll, expected_roll);
    assert_eq!(report.expenditure.gatling_damage, Some(1));
    assert_eq!(report.expenditure.heat, 1);
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
        include_str!("../game/units/Demolisher.toml"),
        "IS.AC/20",
        &["RapidFire"],
    );
    let (_dir, _config, mut world, id) = fixture(&template).await;
    seed(&mut world, id, 17);
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["ammunition"] = serde_json::json!([1, 0, 0, 0]);
        })
        .unwrap();
    let report = launch_battle_vehicle_weapon(&mut world, request(id)).unwrap();
    assert_eq!(report.expenditure.fire_mode, FireMode::Normal);
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
        FireMode::Normal
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
        let mut expected = Dice::seeded([value; 32]);
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
        // A roll of 2 fires both shots, then leaves the weapon useless for the rest of the
        // game (Total Warfare p. 114): an intact mount with a feed the crew cannot clear.
        assert!(report.expenditure.launched);
        assert!(!report.loader_destroyed);
        assert_eq!(report.jammed, roll == 2);
        assert_eq!(report.feed_broken, roll == 2);
        assert_eq!(report.feed_critical_roll, None);
        assert_eq!(report.hit, roll >= 6);
        assert_eq!(report.glancing, roll == 6);
        assert_eq!(report.expenditure.fire_mode, FireMode::Ultra);
        assert_eq!(
            report
                .expenditure
                .ammunition
                .iter()
                .map(|draw| draw.rounds)
                .sum::<u16>(),
            2
        );
        let unit = &world.btech.vehicles()[&id];
        assert!(unit.weapon_readiness(1).unwrap().ready);
        assert!(unit.weapon_readiness(0).unwrap().intact);
        assert!(unit.weapon_readiness(0).unwrap().recycle_remaining > 0);
        assert_eq!(
            unit.ammunition().iter().sum::<u16>() + 2,
            ammunition.iter().sum::<u16>()
        );
        assert!(!unit.is_destroyed());
        assert_eq!(unit.weapon_jammed(0).unwrap(), roll == 2);
        assert_eq!(unit.weapon_feed_broken(0).unwrap(), roll == 2);
        if roll == 2 {
            assert!(unit.weapon_readiness(0).unwrap().jammed);
            assert!(battle_weapon_status(&world, id).unwrap().contains("jammed"));
            let mut attempt = world.clone();
            for _ in 0..130 {
                advance_battle_recycle(&mut attempt);
            }
            assert_eq!(
                begin_battle_unjam(&mut attempt, id, ObjectId(1), 0)
                    .unwrap_err()
                    .to_string(),
                "The ammo feed on that weapon is broken beyond field repair."
            );
            // A saved record cannot mark a feed broken without its jam.
            let mut bad = serde_json::to_value(&world.btech).unwrap();
            bad["vehicles"][id.0.to_string()]["jammed_weapons"] = serde_json::json!([]);
            assert!(serde_json::from_value::<BtechState>(bad).is_err());
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
        ("IS.LRM-5", "Hotload", 1, 1),
    ] {
        let template = with_modes(&demolisher_with(weapon), weapon, &[flag]);
        let (_dir, config, base, id) = fixture(&template).await;
        let mut covered = std::collections::BTreeSet::new();
        for value in 0..=255 {
            let mut dice = Dice::seeded([value; 32]);
            let roll = dice.two_d6();
            if !covered.insert(roll) {
                continue;
            }
            let mut world = base.clone();
            seed(&mut world, id, value);
            let before = world.btech.vehicles()[&id].ammunition().to_vec();
            let report = launch_battle_vehicle_weapon(&mut world, request(id)).unwrap();
            assert_eq!(report.roll, roll);
            // The burst that jams is still fired (Total Warfare p. 114); hot-loaded launchers
            // never jam (Tactical Operations p. 101).
            assert_eq!(report.jammed, roll <= threshold);
            assert!(!report.loader_destroyed && !report.feed_broken);
            assert!(report.expenditure.launched);
            assert_eq!(report.hit, roll >= 6);
            assert!(
                world.btech.vehicles()[&id]
                    .weapon_readiness(0)
                    .unwrap()
                    .intact
            );
            assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
            assert_eq!(
                report
                    .expenditure
                    .ammunition
                    .iter()
                    .map(|draw| draw.rounds)
                    .sum::<u16>(),
                rounds
            );
            assert_eq!(
                world.btech.vehicles()[&id].ammunition().iter().sum::<u16>() + rounds,
                before.iter().sum::<u16>()
            );
            assert!(!world.btech.vehicles()[&id].weapon_recycle().is_empty());
            if !report.jammed {
                continue;
            }
            let saved = serde_json::to_value(&world.btech).unwrap();
            let mut replay = world.clone();
            replay.btech = serde_json::from_value(saved.clone()).unwrap();
            for _ in 0..130 {
                assert_eq!(
                    advance_battle_units(&mut world, 0, true),
                    advance_battle_units(&mut replay, 0, true)
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
async fn vehicle_rapid_fire_explosions_and_caseless_breakdowns_fire_first() {
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
            let mut dice = Dice::seeded([value; 32]);
            let roll = dice.two_d6();
            if !covered.insert(roll) {
                continue;
            }
            // Rapid-fire rounds explode in the barrel on a 2 (Tactical Operations p. 98); a
            // vehicle's caseless breakdown on a 2 destroys the weapon outright (TacOps: AUE
            // p. 165). Either way the shot is fired first.
            let exploded = rapid && roll == 2;
            let breakdown = caseless && roll == 2;
            let destroyed = exploded || breakdown;
            let jammed = rapid && (3..=4).contains(&roll);
            let expected_damage_rolls = if exploded {
                vec![dice.two_d6()]
            } else {
                Vec::new()
            };
            // The structure damage then rolls once on the turret column.
            let critical = exploded.then(|| dice.two_d6());
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
            assert_eq!(report.feed_critical_roll, None);
            assert_eq!(report.loader_destroyed, destroyed);
            assert_eq!(report.jammed, jammed);
            assert!(!report.feed_broken);
            assert!(report.expenditure.launched);
            let heat = if rapid { 2 } else { 1 };
            assert_eq!(report.expenditure.heat, heat);
            assert_eq!(report.hit, !breakdown && roll >= 6);
            assert_eq!(report.misload.is_some(), exploded);
            if let Some(damage) = report.misload {
                assert_eq!(damage.rolls, expected_damage_rolls);
                assert_eq!(damage.absorbed, 2);
                assert_eq!(damage.section, VehicleSection::Turret);
                assert_eq!(damage.criticals.len(), 1);
                assert_eq!(damage.criticals[0].selection.rolls, vec![critical.unwrap()]);
                if critical.unwrap() < 6 {
                    assert_eq!(
                        world.btech.vehicles()[&id].sections()[&VehicleSection::Turret].internal,
                        6
                    );
                }
            }
            let unit = &world.btech.vehicles()[&id];
            assert_eq!(unit.weapon_readiness(0).unwrap().intact, !destroyed);
            assert_eq!(unit.weapon_jammed(0).unwrap(), jammed);
            // A critical from the explosion's structure damage has its own consequences; the
            // bookkeeping below holds whenever the roll was too low to apply one.
            if critical.is_none_or(|roll| roll < 6) {
                let spent = if rapid { 2 } else { 1 };
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
                // Destroying the mount retires its recycle clock.
                assert_eq!(
                    world.btech.vehicles()[&id].weapon_recycle().is_empty(),
                    destroyed
                );
                assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
                world.validate(&config).unwrap();
                let restored: BtechState =
                    serde_json::from_value(serde_json::to_value(&world.btech).unwrap()).unwrap();
                assert_eq!(restored, world.btech);
            }
        }
        assert_eq!(covered, (2..=12).collect());
    }
}

#[tokio::test]
async fn vehicle_rapid_fire_explosions_spend_the_burst_before_hull_loss() {
    let seed_value = (0..=255)
        .find(|value| Dice::seeded([*value; 32]).two_d6() == 2)
        .unwrap();
    for front in [false, true] {
        let mut template = with_modes(
            include_str!("../game/units/Demolisher.toml"),
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
        // The burst is spent before the explosion; either explosion guts a location, and any
        // lost location wrecks the vehicle (TW p. 128).
        assert!(world.btech.vehicles()[&id].is_destroyed());
        let ammo: u16 = world.btech.vehicles()[&id].ammunition().iter().sum();
        assert_eq!(ammo, if front { before_ammo - 2 } else { 0 });
        assert_eq!(
            report
                .expenditure
                .ammunition
                .iter()
                .map(|draw| draw.rounds)
                .sum::<u16>(),
            2
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
    // With one round left the burst falls back to a single shot; only a roll of 2 breaks the
    // caseless feed, and a vehicle then loses the weapon outright.
    for roll in [2, 3] {
        let value = (0..=255)
            .find(|value| Dice::seeded([*value; 32]).two_d6() == roll)
            .unwrap();
        let mut world = base.clone();
        seed(&mut world, id, value);
        world
            .btech
            .rewrite_unit_record(id, |record| {
                record["ammunition"] = serde_json::json!([1, 0, 0, 0]);
            })
            .unwrap();
        let report = launch_battle_vehicle_weapon(&mut world, request(id)).unwrap();
        assert_eq!(report.loader_destroyed, roll == 2);
        assert!(!report.jammed && !report.feed_broken);
        assert_eq!(report.feed_critical_roll, None);
        assert!(report.misload.is_none());
        assert!(report.expenditure.launched);
        assert_eq!(report.expenditure.fire_mode, FireMode::Normal);
        assert_eq!(
            world.btech.vehicles()[&id].fire_mode(0).unwrap(),
            FireMode::Normal
        );
        assert_eq!(world.btech.vehicles()[&id].ammunition()[0], 0);
        assert_eq!(
            world.btech.vehicles()[&id]
                .weapon_readiness(0)
                .unwrap()
                .intact,
            roll != 2
        );
        world.validate(&config).unwrap();
    }
    let template = with_modes(&demolisher_with("IS.AC/2"), "IS.AC/2", &["RapidFire"]);
    let (_dir, _config, mut base, id) = fixture(&template).await;
    let value = (0..=255)
        .find(|value| {
            let mut dice = Dice::seeded([*value; 32]);
            let attack = dice.two_d6();
            dice.two_d6();
            attack == 2 && dice.two_d6() >= 8
        })
        .unwrap();
    seed(&mut base, id, value);
    for safe in [false, true] {
        let mut world = base.clone();
        let mut rules = request(id);
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
        // The burst is spent before the rounds explode.
        assert_eq!(
            report
                .expenditure
                .ammunition
                .iter()
                .map(|draw| draw.rounds)
                .sum::<u16>(),
            2
        );
        let mut replay = base.clone();
        let expected = launch_battle_vehicle_weapon(&mut replay, rules).unwrap();
        assert_eq!(expected.misload.unwrap(), impact);
        assert_eq!(world.btech, replay.btech);
    }
}
