//! Internal vehicle damage orders criticals before structure and commits nested consequences atomically.
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
        BattleVehicleTemplate::parse("test", template).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    (dir, config, world, id)
}

/// Seed only the selected vehicle's dice, keeping all material and equipment state unchanged.
fn seed(world: &mut World, id: ObjectId, value: u8) {
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["dice"] = serde_json::to_value(BattleDice::seeded([value; 32])).unwrap();
        })
        .unwrap();
}

/// Isolated internal damage with criticals disabled still consumes its two diagnostic rolls.
fn rules() -> BattleVehicleCriticalRules {
    BattleVehicleCriticalRules {
        rotor_damage_divisor: 0,
        extended_piloting: false,
        vtol_table: None,
        table: BattleVehicleCriticalTable::Advanced,
        enabled: false,
        combat_safe: false,
        toughness: false,
    }
}

#[tokio::test]
async fn internal_damage_handles_structure_rounding_and_vehicle_local_overflow() {
    for (special, structural, absorbed) in [
        ("", 5, 5),
        ("ReinforcedInternal_Tech", 3, 3),
        ("CompositeInternal_Tech", 10, 8),
    ] {
        let text = support::templates::with_flags(
            include_str!("../game/mechs/Demolisher.toml"),
            &[special],
        );
        let (_dir, config, mut world, id) = fixture(&text).await;
        seed(&mut world, id, 31);
        let mut dice = BattleDice::seeded([31; 32]);
        let expected = vec![dice.two_d6(), dice.two_d6()];
        let report = resolve_battle_vehicle_internal_damage(
            &mut world,
            id,
            BattleVehicleSection::Turret,
            5,
            rules(),
        )
        .unwrap();
        assert_eq!(report.rolls, expected);
        assert_eq!(report.structural_damage, structural);
        assert_eq!(report.absorbed, absorbed);
        assert_eq!(report.discarded, structural - u32::from(absorbed));
        assert!(!world.btech.vehicles()[&id].is_destroyed());
        assert_eq!(
            world.btech.vehicles()[&id].sections()[&BattleVehicleSection::Front].armor,
            40
        );
        assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), vec![dice.d6()]);
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

#[tokio::test]
async fn internal_criticals_precede_structure_and_nested_errors_roll_back_all_damage() {
    let (_dir, _config, mut world, id) =
        fixture(include_str!("../game/mechs/Demolisher.toml")).await;
    let value = (0..=255)
        .find(|value| {
            let mut dice = BattleDice::seeded([*value; 32]);
            dice.two_d6();
            matches!(dice.two_d6(), 8 | 9) && dice.two_d6() == 6
        })
        .unwrap();
    seed(&mut world, id, value);
    let mut critical_rules = rules();
    critical_rules.enabled = true;
    let result = resolve_battle_vehicle_internal_damage(
        &mut world,
        id,
        BattleVehicleSection::Front,
        8,
        critical_rules,
    )
    .unwrap();
    assert_eq!(result.criticals.len(), 1);
    assert_eq!(
        result.criticals[0].selection.effect,
        Some(BattleVehicleCriticalEffect::Driver)
    );
    assert_eq!(world.btech.vehicles()[&id].piloting_damage(), 2);
    assert!(result.unit_destroyed);
    assert!(
        result
            .notices
            .last()
            .unwrap()
            .text
            .contains("has been destroyed")
    );
    let (_dir, _config, mut world, id) =
        fixture(include_str!("../game/mechs/Demolisher.toml")).await;
    let value = (0..=255)
        .find(|value| {
            let mut dice = BattleDice::seeded([*value; 32]);
            dice.two_d6();
            dice.two_d6() >= 8 && dice.die(3).unwrap() != 2 && dice.d6() >= 5
        })
        .unwrap();
    seed(&mut world, id, value);
    critical_rules.table = BattleVehicleCriticalTable::Standard;
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let report = resolve_battle_vehicle_internal_damage(
        &mut world,
        id,
        BattleVehicleSection::Turret,
        400,
        critical_rules,
    )
    .unwrap();
    assert!(report.unit_destroyed);
    assert!(world.btech.vehicles()[&id].crew_killed());
    assert!(
        world.btech.vehicles()[&id]
            .sections()
            .values()
            .all(|state| state.internal == 0)
    );
}

#[tokio::test]
async fn weapon_explosions_disable_mount_before_damage_and_injure_surviving_crew() {
    let text = include_str!("../game/mechs/Demolisher.toml").replace(
        "[sections.front_side]\n",
        "[sections.front_side]\nslots = [{ at = 1, item = \"IS.MagshotGaussRifle\" }]\n",
    );
    let (_dir, config, base, id) = fixture(&text).await;
    let value = (0..=255)
        .find(|value| {
            let mut dice = BattleDice::seeded([*value; 32]);
            if dice.two_d6() != 11 {
                return false;
            }
            dice.die(1).unwrap();
            dice.two_d6();
            dice.two_d6() < 8
        })
        .unwrap();
    for disabled in [false, true] {
        let mut world = base.clone();
        seed(&mut world, id, value);
        if disabled {
            world
                .btech
                .rewrite_unit_record(id, |record| {
                    record["weapon_failures"] = serde_json::json!({"0":"disabled"});
                })
                .unwrap();
        }
        let mut critical_rules = rules();
        critical_rules.enabled = true;
        let result = resolve_battle_vehicle_critical(
            &mut world,
            id,
            BattleVehicleSection::Front,
            critical_rules,
        )
        .unwrap();
        assert_eq!(
            result.selection.effect,
            Some(BattleVehicleCriticalEffect::WeaponDestroyed)
        );
        assert_eq!(world.btech.vehicles()[&id].lost_criticals().len(), 1);
        assert_eq!(result.internal_damage.len(), usize::from(!disabled));
        assert_eq!(
            world.btech.vehicles()[&id].pilot_injuries(),
            if disabled { 0 } else { 2 }
        );
        assert_eq!(
            world.btech.vehicles()[&id].sections()[&BattleVehicleSection::Front].internal,
            if disabled { 8 } else { 5 }
        );
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
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
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["dice"] = serde_json::to_value(BattleDice::seeded(seed)).unwrap();
        })
        .unwrap();
}

#[tokio::test]
async fn ammunition_cascade_commits_complete_damage_or_rolls_back_spent_bins() {
    let (_dir, config, base, id) = fixture(include_str!("../game/mechs/Demolisher.toml")).await;
    let mut rules = rules();
    rules.enabled = true;
    let success = matching_seed(|dice| {
        if dice.two_d6() != 12 {
            return false;
        }
        dice.two_d6();
        dice.two_d6() < 8
    });
    let mut world = base.clone();
    set_seed(&mut world, id, success);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    let result =
        resolve_battle_vehicle_critical(&mut world, id, BattleVehicleSection::Turret, rules)
            .unwrap();
    assert_eq!(
        result,
        resolve_battle_vehicle_critical(&mut loaded, id, BattleVehicleSection::Turret, rules)
            .unwrap()
    );
    assert_eq!(world.btech, loaded.btech);
    assert_eq!(result.ammunition_cascade.as_ref().unwrap().damage, 400);
    assert_eq!(result.internal_damage[0].absorbed, 8);
    assert_eq!(result.internal_damage[0].discarded, 392);
    assert!(!world.btech.vehicles()[&id].is_destroyed());
    assert_eq!(
        world.btech.vehicles()[&id].sections()[&BattleVehicleSection::Rear].armor,
        20
    );
    assert!(
        world.btech.vehicles()[&id]
            .ammunition()
            .iter()
            .all(|rounds| *rounds == 0)
    );
    let fail = matching_seed(|dice| {
        if dice.two_d6() != 11 {
            return false;
        }
        dice.two_d6();
        dice.two_d6() >= 8 && dice.two_d6() == 12
    });
    let mut world = base;
    set_seed(&mut world, id, fail);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let before = world.btech.clone();
    let report =
        resolve_battle_vehicle_critical(&mut world, id, BattleVehicleSection::Rear, rules).unwrap();
    assert!(
        report
            .internal_damage
            .iter()
            .any(|damage| damage.unit_destroyed)
    );
    assert!(world.btech.vehicles()[&id].crew_killed());
    world.btech = before;
    rules.enabled = false;
    let report = resolve_battle_vehicle_internal_damage(
        &mut world,
        id,
        BattleVehicleSection::Front,
        100,
        rules,
    )
    .unwrap();
    assert!(report.unit_destroyed);
    assert!(!world.btech.vehicles()[&id].crew_killed());
    assert_eq!(world.objects[&ObjectId(1)].location, Some(id));
}

#[tokio::test]
async fn hotloaded_vehicle_criticals_require_usable_normal_ammunition() {
    let stream = matching_seed(|dice| {
        if dice.two_d6() != 11 {
            return false;
        }
        dice.die(1).unwrap();
        dice.two_d6();
        dice.two_d6() < 8
    });
    for (mode, rounds, lost_bin, disabled, expected) in [
        ("", 2, false, false, 5),
        ("", 0, false, false, 0),
        (", modes = [\"Sguided\"]", 2, false, false, 0),
        ("", 2, true, false, 0),
        ("", 2, false, true, 0),
    ] {
        let text = include_str!("../game/mechs/Demolisher.toml")
            .replace(
                "[sections.front_side]\n",
                "[sections.front_side]\nslots = [{ at = 1, item = \"IS.LRM-5\", modes = [\"Hotload\"] }]\n",
            )
            .replace(
                "[sections.left_side]\n",
                &format!(
                    "[sections.left_side]\nslots = [{{ at = 1, item = \"Ammo_IS.LRM-5\", rounds = {rounds}{mode} }}]\n"
                ),
            );
        let (_dir, config, mut world, id) = fixture(&text).await;
        let index = world.btech.vehicles()[&id]
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|mount| mount.weapon == BattleWeapon::Lrm5)
            .unwrap();
        if lost_bin {
            destroy_battle_vehicle_critical(
                &mut world,
                id,
                VehicleCriticalLocation {
                    section: BattleVehicleSection::Left,
                    slot: 0,
                },
            )
            .unwrap();
        }
        if disabled {
            world
                .btech
                .rewrite_unit_record(id, |record| {
                    record["weapon_failures"] = serde_json::json!({index.to_string(): "disabled"});
                })
                .unwrap();
        }
        set_seed(&mut world, id, stream);
        let ammo = world.btech.vehicles()[&id].ammunition().to_vec();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        let mut rules = rules();
        rules.enabled = true;
        let report =
            resolve_battle_vehicle_critical(&mut world, id, BattleVehicleSection::Front, rules)
                .unwrap();
        assert_eq!(
            report,
            resolve_battle_vehicle_critical(&mut restored, id, BattleVehicleSection::Front, rules)
                .unwrap()
        );
        assert_eq!(world.btech, restored.btech);
        let vehicle = &world.btech.vehicles()[&id];
        assert_eq!(
            vehicle.sections()[&BattleVehicleSection::Front].internal,
            8 - expected
        );
        assert_eq!(vehicle.pilot_injuries(), 0);
        assert_eq!(vehicle.ammunition(), ammo);
        assert!(vehicle.critical_destroyed(VehicleCriticalLocation {
            section: BattleVehicleSection::Front,
            slot: 0
        }));
        assert_eq!(report.internal_damage.len(), usize::from(expected > 0));
        assert_eq!(
            report
                .notices
                .iter()
                .any(|notice| notice.text.contains("hotloaded launcher explodes")),
            expected > 0
        );
        let mut dice = BattleDice::seeded(stream);
        dice.two_d6();
        dice.die(1).unwrap();
        if expected > 0 {
            dice.two_d6();
            dice.two_d6();
        }
        assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
    }
}

#[tokio::test]
async fn incendiary_vehicle_criticals_require_recycling_and_matching_supply() {
    let stream = matching_seed(|dice| {
        if dice.two_d6() != 11 {
            return false;
        }
        dice.die(1).unwrap();
        dice.two_d6();
        dice.two_d6() < 8
    });
    for (recycling, rounds, bin_mode, expected) in [
        (true, 2, ", modes = [\"Incendiary\"]", 2),
        (false, 2, ", modes = [\"Incendiary\"]", 0),
        (true, 0, ", modes = [\"Incendiary\"]", 0),
        (true, 2, "", 0),
    ] {
        let text = include_str!("../game/mechs/Demolisher.toml")
            .replace(
                "[sections.front_side]\n",
                "[sections.front_side]\nslots = [{ at = 1, item = \"IS.AC/2\", modes = [\"Incendiary\"] }]\n",
            )
            .replace(
                "[sections.left_side]\n",
                &format!(
                    "[sections.left_side]\nslots = [{{ at = 1, item = \"Ammo_IS.AC/2\", rounds = {rounds}{bin_mode} }}]\n"
                ),
            );
        let (_dir, config, mut world, id) = fixture(&text).await;
        let index = world.btech.vehicles()[&id]
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|mount| mount.weapon == BattleWeapon::Ac2)
            .unwrap();
        if recycling {
            world
                .btech
                .rewrite_unit_record(id, |record| {
                    record["weapon_recycle"] = serde_json::json!({index.to_string(): 1});
                })
                .unwrap();
        }
        set_seed(&mut world, id, stream);
        let ammo = world.btech.vehicles()[&id].ammunition().to_vec();
        let mut rules = rules();
        rules.enabled = true;
        let report =
            resolve_battle_vehicle_critical(&mut world, id, BattleVehicleSection::Front, rules)
                .unwrap();
        let vehicle = &world.btech.vehicles()[&id];
        assert_eq!(
            vehicle.sections()[&BattleVehicleSection::Front].internal,
            8 - expected
        );
        assert_eq!(vehicle.pilot_injuries(), 0);
        assert_eq!(vehicle.ammunition(), ammo);
        assert!(!vehicle.weapon_recycle().contains_key(&index));
        assert_eq!(report.internal_damage.len(), usize::from(expected > 0));
        assert_eq!(
            report.notices.iter().any(|notice| notice
                .text
                .contains("incendiary ammunition in your launcher ignites")),
            expected > 0
        );
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            world.btech,
            persistence::load(&config.database()).await.unwrap().btech
        );
    }
}

#[tokio::test]
async fn hotloaded_nested_crew_and_hull_loss_preserves_surviving_ammunition() {
    let text = include_str!("../game/mechs/Demolisher.toml")
        .replace(
            "[sections.front_side]\n",
            "[sections.front_side]\nslots = [{ at = 1, item = \"IS.LRM-10\", modes = [\"Hotload\"] }]\n",
        )
        .replace(
            "[sections.left_side]\n",
            "[sections.left_side]\nslots = [{ at = 1, item = \"Ammo_IS.LRM-10\", rounds = 2 }]\n",
        );
    let (_dir, _config, mut world, id) = fixture(&text).await;
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let stream = matching_seed(|dice| {
        if dice.two_d6() != 11 {
            return false;
        }
        dice.die(1).unwrap();
        dice.two_d6();
        matches!(dice.two_d6(), 8 | 9) && dice.two_d6() == 12
    });
    set_seed(&mut world, id, stream);
    let ammo = world.btech.vehicles()[&id].ammunition().to_vec();
    let mut rules = rules();
    rules.enabled = true;
    let report =
        resolve_battle_vehicle_critical(&mut world, id, BattleVehicleSection::Front, rules)
            .unwrap();
    assert!(report.internal_damage[0].unit_destroyed);
    assert!(world.btech.vehicles()[&id].crew_killed());
    assert_eq!(
        world.btech.vehicles()[&id].sections()[&BattleVehicleSection::Front].internal,
        0
    );
    assert_eq!(world.btech.vehicles()[&id].ammunition(), ammo);
}

#[tokio::test]
async fn nonexplosive_vehicle_firing_modes_allow_weapon_destruction() {
    for weapon in [
        "item = \"IS.UltraAC/2\", modes = [\"UltraMode\"]",
        "item = \"IS.Flamer\", modes = [\"Heat\"]",
    ] {
        let text = include_str!("../game/mechs/Demolisher.toml").replace(
            "[sections.front_side]\n",
            &format!("[sections.front_side]\nslots = [{{ at = 1, {weapon} }}]\n"),
        );
        let (_dir, _config, mut world, id) = fixture(&text).await;
        let stream = matching_seed(|dice| dice.two_d6() == 11);
        set_seed(&mut world, id, stream);
        let mut rules = rules();
        rules.enabled = true;
        let report =
            resolve_battle_vehicle_critical(&mut world, id, BattleVehicleSection::Front, rules)
                .unwrap();
        assert!(report.internal_damage.is_empty());
        assert!(report.pilot_injury.is_none());
        assert!(
            world.btech.vehicles()[&id].critical_destroyed(VehicleCriticalLocation {
                section: BattleVehicleSection::Front,
                slot: 0
            })
        );
        assert_eq!(
            world.btech.vehicles()[&id].sections()[&BattleVehicleSection::Front].internal,
            8
        );
    }
}
