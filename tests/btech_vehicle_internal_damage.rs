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
        crate::support::templates::fitted(VehicleTemplate::parse("test", template).unwrap()),
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

/// Seed only the selected vehicle's dice, keeping all material and equipment state unchanged.
fn seed(world: &mut World, id: ObjectId, value: u8) {
    world
        .btech
        .set_unit_dice(id, Dice::seeded([value; 32]))
        .unwrap();
}

/// Ordinary critical policy for standalone internal damage.
fn rules() -> VehicleCriticalRules {
    VehicleCriticalRules {
        extended_piloting: false,
        combat_safe: false,
        toughness: false,
    }
}

#[tokio::test]
async fn internal_damage_handles_structure_rounding_and_vehicle_local_overflow() {
    // Combat vehicles carry standard structure only (TechManual p. 98).
    for (special, structural, absorbed) in [("", 3, 3)] {
        let text = support::templates::with_flags(
            include_str!("../game/units/Demolisher.toml"),
            &[special],
        );
        let (_dir, config, mut world, id) = fixture(&text).await;
        // Entry roll, then a critical roll below six.
        let stream = matching_seed(|dice| {
            dice.two_d6();
            dice.two_d6() < 6
        });
        set_seed(&mut world, id, stream);
        let mut dice = Dice::seeded(stream);
        let expected = vec![dice.two_d6()];
        dice.two_d6();
        let report = resolve_battle_vehicle_internal_damage(
            &mut world,
            id,
            VehicleSection::Turret,
            3,
            rules(),
        )
        .unwrap();
        assert_eq!(report.rolls, expected);
        assert_eq!(report.structural_damage, structural);
        assert_eq!(report.absorbed, absorbed);
        assert_eq!(report.discarded, structural - u32::from(absorbed));
        assert!(!world.btech.vehicles()[&id].is_destroyed());
        assert_eq!(
            world.btech.vehicles()[&id].sections()[&VehicleSection::Front].armor,
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
async fn internal_damage_rolls_one_critical_after_the_structure_damage() {
    let (_dir, _config, mut world, id) =
        fixture(include_str!("../game/units/Demolisher.toml")).await;
    let value = (0..=255)
        .find(|value| {
            let mut dice = Dice::seeded([*value; 32]);
            dice.two_d6();
            dice.two_d6() == 6
        })
        .unwrap();
    seed(&mut world, id, value);
    let result =
        resolve_battle_vehicle_internal_damage(&mut world, id, VehicleSection::Front, 7, rules())
            .unwrap();
    assert_eq!(result.rolls.len(), 1);
    assert_eq!(result.criticals.len(), 1);
    assert_eq!(
        result.criticals[0].selection.effect,
        Some(VehicleCriticalEffect::Driver)
    );
    assert_eq!(world.btech.vehicles()[&id].piloting_damage(), 2);
    assert!(!result.unit_destroyed);
    assert_eq!(
        world.btech.vehicles()[&id].sections()[&VehicleSection::Front].internal,
        1
    );
    // A location that loses its last structure destroys the vehicle, so nothing is rolled.
    let (_dir, _config, mut world, id) =
        fixture(include_str!("../game/units/Demolisher.toml")).await;
    seed(&mut world, id, value);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let report = resolve_battle_vehicle_internal_damage(
        &mut world,
        id,
        VehicleSection::Turret,
        400,
        rules(),
    )
    .unwrap();
    assert!(report.unit_destroyed);
    assert!(report.criticals.is_empty());
    assert!(!world.btech.vehicles()[&id].crew_killed());
    assert_eq!(
        world.btech.vehicles()[&id].sections()[&VehicleSection::Turret].internal,
        0
    );
    assert_eq!(
        world.btech.vehicles()[&id].sections()[&VehicleSection::Front].internal,
        8
    );
    let mut dice = Dice::seeded([value; 32]);
    dice.two_d6();
    assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), vec![dice.d6()]);
}

#[tokio::test]
async fn weapon_explosions_disable_mount_before_damage_without_injuring_the_crew() {
    let text = include_str!("../game/units/Demolisher.toml").replace(
        "[sections.front_side]\n",
        "[sections.front_side]\nslots = [{ at = 1, item = \"IS.MagshotGaussRifle\" }]\n",
    );
    let (_dir, config, base, id) = fixture(&text).await;
    let value = (0..=255)
        .find(|value| {
            let mut dice = Dice::seeded([*value; 32]);
            if dice.two_d6() != 11 {
                return false;
            }
            dice.die(1).unwrap();
            dice.two_d6();
            dice.two_d6() < 6
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
        let result =
            resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Front, rules())
                .unwrap();
        assert_eq!(
            result.selection.effect,
            Some(VehicleCriticalEffect::WeaponDestroyed)
        );
        assert_eq!(world.btech.vehicles()[&id].lost_criticals().len(), 1);
        assert_eq!(result.internal_damage.len(), usize::from(!disabled));
        assert_eq!(world.btech.vehicles()[&id].pilot_injuries(), 0);
        assert_eq!(
            world.btech.vehicles()[&id].sections()[&VehicleSection::Front].internal,
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
fn matching_seed(predicate: impl Fn(&mut Dice) -> bool) -> [u8; 32] {
    for value in 0u32..100000 {
        let mut seed = [0; 32];
        seed[..4].copy_from_slice(&value.to_le_bytes());
        if predicate(&mut Dice::seeded(seed)) {
            return seed;
        }
    }
    panic!("No deterministic stream matched");
}

/// Change only the victim's random stream.
fn set_seed(world: &mut World, id: ObjectId, seed: [u8; 32]) {
    world.btech.set_unit_dice(id, Dice::seeded(seed)).unwrap();
}

#[tokio::test]
async fn ammunition_cascade_commits_complete_damage_or_rolls_back_spent_bins() {
    let (_dir, config, base, id) = fixture(include_str!("../game/units/Demolisher.toml")).await;
    let rules = rules();
    let success = matching_seed(|dice| dice.two_d6() == 11);
    let mut world = base.clone();
    set_seed(&mut world, id, success);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    let result =
        resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Turret, rules).unwrap();
    assert_eq!(
        result,
        resolve_battle_vehicle_critical(&mut loaded, id, VehicleSection::Turret, rules).unwrap()
    );
    assert_eq!(world.btech, loaded.btech);
    assert_eq!(result.ammunition_cascade.as_ref().unwrap().damage, 400);
    assert_eq!(result.internal_damage[0].absorbed, 8);
    assert_eq!(result.internal_damage[0].discarded, 392);
    // Blowing out the turret's structure destroys the vehicle (TW p. 128).
    assert!(world.btech.vehicles()[&id].is_destroyed());
    assert!(!world.btech.vehicles()[&id].crew_killed());
    assert_eq!(
        world.btech.vehicles()[&id].sections()[&VehicleSection::Rear].armor,
        20
    );
    assert!(
        world.btech.vehicles()[&id]
            .ammunition()
            .iter()
            .all(|rounds| *rounds == 0)
    );
    let mut world = base;
    set_seed(&mut world, id, success);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let before = world.btech.clone();
    let report =
        resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Rear, rules).unwrap();
    assert!(
        report
            .internal_damage
            .iter()
            .any(|damage| damage.unit_destroyed)
    );
    assert!(!world.btech.vehicles()[&id].crew_killed());
    world.btech = before;
    let report =
        resolve_battle_vehicle_internal_damage(&mut world, id, VehicleSection::Front, 100, rules)
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
        dice.two_d6() < 6
    });
    for (mode, rounds, lost_bin, disabled, expected) in [
        ("", 2, false, false, 5),
        ("", 0, false, false, 0),
        (", modes = [\"Sguided\"]", 2, false, false, 0),
        ("", 2, true, false, 0),
        ("", 2, false, true, 0),
    ] {
        let text = include_str!("../game/units/Demolisher.toml")
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
            .position(|mount| mount.weapon == Weapon::Lrm5)
            .unwrap();
        if lost_bin {
            destroy_battle_vehicle_critical(
                &mut world,
                id,
                VehicleCriticalLocation {
                    section: VehicleSection::Left,
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
        let rules = rules();
        let report =
            resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Front, rules).unwrap();
        assert_eq!(
            report,
            resolve_battle_vehicle_critical(&mut restored, id, VehicleSection::Front, rules)
                .unwrap()
        );
        assert_eq!(world.btech, restored.btech);
        let vehicle = &world.btech.vehicles()[&id];
        assert_eq!(
            vehicle.sections()[&VehicleSection::Front].internal,
            8 - expected
        );
        assert_eq!(vehicle.pilot_injuries(), 0);
        assert_eq!(vehicle.ammunition(), ammo);
        assert!(vehicle.critical_destroyed(VehicleCriticalLocation {
            section: VehicleSection::Front,
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
        let mut dice = Dice::seeded(stream);
        dice.two_d6();
        dice.die(1).unwrap();
        if expected > 0 {
            dice.two_d6();
            dice.two_d6();
        }
        assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
    }
}

/// A Weapon Destroyed critical on an unexpended B-Pod releases its 2 points of damage in the
/// pod's location, as it does on a Mech (Total Warfare p. 130).
#[tokio::test]
async fn a_destroyed_b_pod_explodes_in_its_location() {
    let stream = matching_seed(|dice| {
        if dice.two_d6() != 11 {
            return false;
        }
        dice.die(1).unwrap();
        dice.two_d6();
        dice.two_d6() < 6
    });
    let text = include_str!("../game/units/Demolisher.toml").replace(
        "[sections.front_side]\n",
        "[sections.front_side]\nslots = [{ at = 1, item = \"IS.B-Pod\" }]\n",
    );
    let (_dir, _config, mut world, id) = fixture(&text).await;
    set_seed(&mut world, id, stream);
    let report =
        resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Front, rules()).unwrap();
    let vehicle = &world.btech.vehicles()[&id];
    assert_eq!(vehicle.sections()[&VehicleSection::Front].internal, 8 - 2);
    assert!(vehicle.critical_destroyed(VehicleCriticalLocation {
        section: VehicleSection::Front,
        slot: 0
    }));
    assert_eq!(report.internal_damage.len(), 1);
    assert!(
        report
            .notices
            .iter()
            .any(|notice| notice.text.contains("B-Pod explodes for 2 points"))
    );
    assert!(
        report
            .broadcasts
            .iter()
            .any(|notice| notice.text.contains("rocked by a pod explosion"))
    );
}

#[tokio::test]
async fn hotloaded_hull_loss_preserves_surviving_ammunition() {
    let text = include_str!("../game/units/Demolisher.toml")
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
        dice.two_d6() == 12
    });
    set_seed(&mut world, id, stream);
    let ammo = world.btech.vehicles()[&id].ammunition().to_vec();
    let rules = rules();
    let report =
        resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Front, rules).unwrap();
    // The launcher's explosion guts the front, which wrecks the vehicle without a crew roll.
    assert!(report.internal_damage[0].unit_destroyed);
    assert!(report.internal_damage[0].criticals.is_empty());
    assert!(!world.btech.vehicles()[&id].crew_killed());
    assert_eq!(
        world.btech.vehicles()[&id].sections()[&VehicleSection::Front].internal,
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
        let text = include_str!("../game/units/Demolisher.toml").replace(
            "[sections.front_side]\n",
            &format!("[sections.front_side]\nslots = [{{ at = 1, {weapon} }}]\n"),
        );
        let (_dir, _config, mut world, id) = fixture(&text).await;
        let stream = matching_seed(|dice| dice.two_d6() == 11);
        set_seed(&mut world, id, stream);
        let report =
            resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Front, rules())
                .unwrap();
        assert!(report.internal_damage.is_empty());
        assert!(
            world.btech.vehicles()[&id].critical_destroyed(VehicleCriticalLocation {
                section: VehicleSection::Front,
                slot: 0
            })
        );
        assert_eq!(
            world.btech.vehicles()[&id].sections()[&VehicleSection::Front].internal,
            8
        );
    }
}
