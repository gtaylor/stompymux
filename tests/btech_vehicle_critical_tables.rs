//! The Ground Combat Vehicle Critical Hits Table: columns, move-down for inapplicable results,
//! roll modifiers, suppression and saved random stream replay.
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

/// Seed only the selected vehicle's dice, keeping all material and equipment state unchanged.
fn seed(world: &mut World, id: ObjectId, value: u8) {
    world
        .btech
        .set_unit_dice(id, Dice::seeded([value; 32]))
        .unwrap();
}

fn rules() -> VehicleCriticalRules {
    VehicleCriticalRules {
        extended_piloting: false,
        toughness: false,
        combat_safe: false,
    }
}

/// Rows 6–12 for a pristine Demolisher (weapons and ammunition only in the turret, ICE engine)
/// after inapplicable results move down the column (TW p. 194).
fn demolisher_rows(section: VehicleSection) -> [&'static str; 7] {
    use VehicleSection as S;
    match section {
        S::Front => [
            "driver",
            "stabilizer",
            "stabilizer",
            "sensors",
            "commander",
            "crew_killed",
            "crew_killed",
        ],
        S::Left | S::Right => [
            "crew_stunned",
            "crew_stunned",
            "crew_stunned",
            "stabilizer",
            "engine",
            "engine",
            "fuel_tank",
        ],
        S::Rear => [
            "stabilizer",
            "stabilizer",
            "stabilizer",
            "engine",
            "engine",
            "ammunition",
            "fuel_tank",
        ],
        S::Turret => [
            "stabilizer",
            "turret_jam",
            "weapon_jam",
            "turret_lock",
            "weapon_destroyed",
            "ammunition",
            "turret_blown_off",
        ],
        S::Rotor => unreachable!("ground vehicles have no rotor"),
    }
}

#[tokio::test]
async fn ground_columns_move_down_past_inapplicable_results() {
    use VehicleSection as S;
    let (_dir, _config, base, id) = fixture(include_str!("../game/units/Demolisher.toml")).await;
    for section in [S::Front, S::Left, S::Right, S::Rear, S::Turret] {
        let golden = demolisher_rows(section);
        let mut seen = std::collections::BTreeSet::new();
        for value in 0..=255 {
            let mut world = base.clone();
            seed(&mut world, id, value);
            let mut dice = Dice::seeded([value; 32]);
            let roll = dice.two_d6();
            seen.insert(roll);
            let report = roll_battle_vehicle_critical(&mut world, id, section, rules()).unwrap();
            assert_eq!(report.rolls, vec![roll]);
            assert_eq!(report.section, section);
            assert_eq!(report.modifier, 0);
            let effect = serde_json::to_value(report.effect).unwrap();
            assert_eq!(
                effect,
                if roll < 6 {
                    serde_json::Value::Null
                } else {
                    serde_json::json!(golden[usize::from(roll - 6)])
                },
                "{section:?}/{roll}"
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
async fn fusion_fuel_tank_is_an_engine_hit_and_empty_bins_become_weapon_destroyed() {
    use VehicleSection as S;
    let fusion = support::templates::without_flags(
        include_str!("../game/units/Demolisher.toml"),
        &["ICEEngine_Tech"],
    );
    let (_dir, _config, base, id) = fixture(&fusion).await;
    let twelve = (0..=255)
        .find(|value| Dice::seeded([*value; 32]).two_d6() == 12)
        .unwrap();
    let mut world = base.clone();
    seed(&mut world, id, twelve);
    let report = roll_battle_vehicle_critical(&mut world, id, S::Left, rules()).unwrap();
    assert_eq!(report.effect, Some(VehicleCriticalEffect::Engine));
    // With the engine already gone, a side 12 wraps to the top of the column and becomes
    // the first applicable result, Crew Stunned.
    let eleven = (0..=255)
        .find(|value| Dice::seeded([*value; 32]).two_d6() == 11)
        .unwrap();
    let mut world = base.clone();
    seed(&mut world, id, eleven);
    let _ = resolve_battle_vehicle_critical(&mut world, id, S::Left, rules()).unwrap();
    assert!(world.btech.vehicles()[&id].engine_destroyed());
    seed(&mut world, id, twelve);
    let report = roll_battle_vehicle_critical(&mut world, id, S::Left, rules()).unwrap();
    assert_eq!(report.effect, Some(VehicleCriticalEffect::CrewStunned));
    // An Ammunition result without live ammunition is Weapon Destroyed.
    let mut world = base.clone();
    world
        .btech
        .rewrite_unit_record(id, |record| {
            let bins = record["ammunition"].as_array().unwrap().len();
            record["ammunition"] = serde_json::json!(vec![0u16; bins]);
        })
        .unwrap();
    seed(&mut world, id, eleven);
    let report = roll_battle_vehicle_critical(&mut world, id, S::Turret, rules()).unwrap();
    assert_eq!(report.effect, Some(VehicleCriticalEffect::WeaponDestroyed));
}

#[tokio::test]
async fn roll_modifiers_shift_the_row_and_a_taken_column_ignores_the_hit() {
    use VehicleSection as S;
    let (_dir, _config, base, id) = fixture(include_str!("../game/units/Demolisher.toml")).await;
    let eight = (0..=255)
        .find(|value| Dice::seeded([*value; 32]).two_d6() == 8)
        .unwrap();
    // Armor-piercing −2 turns a turret 8 (Weapon Malfunction) into a 6 (Stabilizer); −3 misses.
    let mut world = base.clone();
    seed(&mut world, id, eight);
    let report =
        roll_battle_vehicle_critical_modified(&mut world, id, S::Turret, rules(), -2).unwrap();
    assert_eq!(report.modifier, -2);
    assert_eq!(report.rolls, vec![8]);
    assert_eq!(report.effect, Some(VehicleCriticalEffect::Stabilizer));
    let mut world = base.clone();
    seed(&mut world, id, eight);
    let report =
        roll_battle_vehicle_critical_modified(&mut world, id, S::Turret, rules(), -3).unwrap();
    assert_eq!(report.effect, None);
    // Positive modifiers cap at the bottom row.
    let mut world = base.clone();
    seed(&mut world, id, eight);
    let report =
        roll_battle_vehicle_critical_modified(&mut world, id, S::Turret, rules(), 6).unwrap();
    assert_eq!(report.effect, Some(VehicleCriticalEffect::TurretBlownOff));
    // Stabilizer once, then the row moves down to the next untaken result.
    let mut world = base.clone();
    let six = (0..=255)
        .find(|value| Dice::seeded([*value; 32]).two_d6() == 6)
        .unwrap();
    seed(&mut world, id, six);
    let _ = resolve_battle_vehicle_critical(&mut world, id, S::Rear, rules()).unwrap();
    assert!(
        world.btech.vehicles()[&id]
            .lost_stabilizers()
            .contains(&S::Rear)
    );
    seed(&mut world, id, six);
    let report = roll_battle_vehicle_critical(&mut world, id, S::Rear, rules()).unwrap();
    assert_eq!(report.effect, Some(VehicleCriticalEffect::Engine));
}

#[tokio::test]
async fn safe_critproof_and_stationary_criticals_draw_no_dice_and_replay() {
    for (template, suppressed) in [
        (
            include_str!("../game/units/Demolisher.toml").to_owned(),
            false,
        ),
        (
            support::templates::with_flags(
                include_str!("../game/units/Demolisher.toml"),
                &["CritProof_Tech"],
            ),
            true,
        ),
        (
            include_str!("../game/units/Demolisher.toml")
                .replace("movement = \"track\"", "movement = \"none\"")
                .replace("walk_mp = 5", "walk_mp = 0"),
            true,
        ),
    ] {
        let (_dir, config, base, id) = fixture(&template).await;
        for combat_safe in [false, true] {
            let mut world = base.clone();
            seed(&mut world, id, 0);
            let before = world.btech.clone();
            let mut dice = Dice::seeded([0; 32]);
            let report = roll_battle_vehicle_critical(
                &mut world,
                id,
                VehicleSection::Front,
                VehicleCriticalRules {
                    combat_safe,
                    ..rules()
                },
            )
            .unwrap();
            if suppressed || combat_safe {
                assert!(report.effect.is_none());
                assert!(report.rolls.is_empty());
                assert_eq!(world.btech, before);
            } else {
                assert_eq!(report.rolls, vec![dice.two_d6()]);
            }
            assert_eq!(
                roll_unit_dice(&mut world, id, 2).unwrap(),
                vec![dice.d6(), dice.d6()]
            );
        }
        let mut world = base;
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        for _ in 0..5 {
            assert_eq!(
                roll_battle_vehicle_critical(&mut world, id, VehicleSection::Rear, rules())
                    .unwrap(),
                roll_battle_vehicle_critical(&mut loaded, id, VehicleSection::Rear, rules())
                    .unwrap()
            );
            assert_eq!(loaded.btech, world.btech);
        }
    }
}
