//! Tactical vehicle firing admission and friendly-fire preferences leave expenditure transactional.
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
        MapAsset::from_cells(&format!("1 5\n{tiles}")).unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for index in 0..4 {
        let id = world.create(&config, format!("Unit {index}"), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        if index < 2 {
            let mut definition =
                MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml"))
                    .unwrap();
            definition
                .attributes
                .insert("specials".into(), "FlipArms Searchlight".into());
            create_battle_unit(&mut world, id, definition).unwrap();
        } else {
            create_battle_vehicle(
                &mut world,
                id,
                VehicleTemplate::parse("test", vehicle).unwrap(),
            )
            .unwrap();
        }
        place_battle_unit(&mut world, id, map, 0, if index % 2 == 0 { 4 } else { 0 }).unwrap();
        ids.push(id);
    }
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    (dir, config, world, map, ids.try_into().unwrap())
}

/// Assign scenario power without introducing crew actions into sensor tests.
fn power(world: &mut World, ids: &[ObjectId], value: Power) {
    for id in ids {
        world.btech.set_unit_power(*id, value).unwrap();
    }
}

/// Ordinary conventional aim without range extensions or arc overrides.
fn rules() -> AimRules {
    AimRules {
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

/// Tactical conventional shot configuration shared by admission cases.
fn shot_rules() -> ShotRules {
    ShotRules {
        range_damage: false,
        tsm_tow_bonus: true,
        vehicle_impact: stompymux_rs::VehicleImpactRules::STANDARD,
        stacking: StackingRules::STANDARD,
        stagger: StaggerMode::Retain,
        glancing: GlancingMode::Disabled,
        aim: rules(),
        hit: HitRules {
            inferno_penalty: false,
            exile_stun_mode: 0,
        },
        hit_arc_mode: 0,
        extended_gunnery: false,
        extended_piloting: false,
        target_toughness: false,
    }
}

/// The shooter's turret points toward both target classes at a one-hex distance.
async fn engagement() -> (tempfile::TempDir, Config, World, ObjectId, [ObjectId; 4]) {
    let (dir, config, mut world, map, ids) = fixture(
        ".0\n.0\n.0\n.0\n.0\n",
        include_str!("../game/units/Demolisher.toml"),
    )
    .await;
    for id in ids {
        place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    }
    place_battle_unit(&mut world, ids[2], map, 0, 1).unwrap();
    power(&mut world, &ids, Power::Running);
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(ids[2]);
    assign_battle_pilot(&mut world, ids[2], ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    refresh_battle_contacts(&mut world, &[ids[2]]).unwrap();
    (dir, config, world, map, ids)
}

/// Vehicle fire checks crew, arcs, contacts, perception and weapon readiness without mutation.
#[tokio::test]
async fn vehicle_firing_checks_readiness_arcs_visibility_and_preserves_all_state() {
    let (_dir, _config, initial, map, [mech, _, shooter, vehicle]) = engagement().await;
    for target in [mech, vehicle] {
        let aim =
            check_battle_vehicle_shot(&initial, shooter, ObjectId(1), target, 0, shot_rules())
                .unwrap();
        assert_eq!(
            aim,
            battle_pilot_aim_modifiers(&initial, shooter, target, 0, false, rules()).unwrap()
        );
        let before = initial.btech.clone();
        assert!(
            check_battle_vehicle_shot(&initial, shooter, ObjectId(2), target, 0, shot_rules())
                .is_err()
        );
        assert!(
            check_battle_vehicle_shot(&initial, shooter, ObjectId(1), target, 999, shot_rules())
                .is_err()
        );
        assert!(
            check_battle_vehicle_shot(&initial, shooter, ObjectId(1), shooter, 0, shot_rules())
                .is_err()
        );
        assert_eq!(initial.btech, before);
        for condition in [
            "crew",
            "arc",
            "contacts",
            "unperceived",
            "character",
            "destroyed",
        ] {
            let mut world = initial.clone();
            if condition == "crew" {
                damage_battle_vehicle_controls(&mut world, shooter, VehicleControlHit::CrewStun)
                    .unwrap();
            } else if condition == "character" {
                world
                    .objects
                    .get_mut(&target)
                    .unwrap()
                    .flags
                    .insert(Flag::InCharacter);
            } else {
                let mut saved = serde_json::to_value(&world.btech).unwrap();
                match condition {
                    "arc" => {
                        saved["vehicles"][shooter.0.to_string()]["motion"]["heading"] =
                            serde_json::json!(180.0);
                        saved["vehicles"][shooter.0.to_string()]["motion"]["desired_heading"] =
                            serde_json::json!(180.0);
                    }
                    "contacts" => {
                        saved["vehicles"][shooter.0.to_string()]["contacts"] =
                            serde_json::json!({});
                    }
                    "unperceived" => {
                        // Neither the switched-off sensor band nor zero visibility reaches.
                        saved["maps"][map.0.to_string()]["sensor_flags"] = serde_json::json!(1);
                        saved["maps"][map.0.to_string()]["visibility"] = serde_json::json!(0);
                    }
                    "destroyed" => {
                        saved["vehicles"][shooter.0.to_string()]["weapon_failures"]["0"] =
                            serde_json::to_value(EquipmentFailure::Disabled).unwrap();
                    }
                    _ => unreachable!(),
                }
                world.btech = serde_json::from_value(saved).unwrap();
            }
            let before = world.btech.clone();
            assert!(
                check_battle_vehicle_shot(&world, shooter, ObjectId(1), target, 0, shot_rules())
                    .is_err(),
                "{condition}"
            );
            if condition == "arc" {
                let mut override_rules = shot_rules();
                override_rules.aim.override_weapon_arcs = true;
                assert!(
                    check_battle_vehicle_shot(
                        &world,
                        shooter,
                        ObjectId(1),
                        target,
                        0,
                        override_rules
                    )
                    .is_ok()
                );
            }
            assert_eq!(world.btech, before);
        }
    }
}

#[tokio::test]
async fn vehicle_friendly_fire_preferences_native_lua_and_map_policy_survive_restart() {
    let (_dir, config, initial, map, [target, _, shooter, _]) = engagement().await;
    for preference in [false, true] {
        for map_safety in [false, true] {
            for hostile in [false, true] {
                let mut world = initial.clone();
                set_battle_friendly_fire_safety(&mut world, shooter, ObjectId(1), preference)
                    .unwrap();
                set_battle_unit_signature(
                    &mut world,
                    target,
                    UnitSignature {
                        team: i32::from(hostile),
                        ..Default::default()
                    },
                )
                .unwrap();
                world
                    .btech
                    .rewrite_map_record(map, |record| {
                        record["flags"] = serde_json::json!(if map_safety { 256 } else { 0 });
                    })
                    .unwrap();
                let before = world.btech.clone();
                assert_eq!(
                    check_battle_vehicle_shot(
                        &world,
                        shooter,
                        ObjectId(1),
                        target,
                        0,
                        shot_rules()
                    )
                    .is_ok(),
                    hostile || !(preference || map_safety)
                );
                assert_eq!(world.btech, before);
                persistence::save(&config.database(), &world).await.unwrap();
                let loaded = persistence::load(&config.database()).await.unwrap();
                assert_eq!(
                    loaded.btech.vehicles()[&shooter].friendly_fire_safety(),
                    preference
                );
                assert_eq!(
                    check_battle_vehicle_shot(
                        &loaded,
                        shooter,
                        ObjectId(1),
                        target,
                        0,
                        shot_rules()
                    )
                    .is_ok(),
                    hostile || !(preference || map_safety)
                );
            }
        }
    }
    let scripts =
        Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(initial))).unwrap();
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "mechprefs"),
        "MWSafety: ON\r\nBTHDebug: OFF\r\nAutoFall: OFF\r\nFFSafety: OFF\r\nSLWarn: OFF\r\nAutoconShutdown: OFF\r\nArmorWarn: ON\r\nAmmoWarn: ON"
    );
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "mechprefs ffsafety on")
            .contains("ON")
    );
    assert!(scripts.world().btech.vehicles()[&shooter].friendly_fire_safety());
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.friendly_fire_safety({},1,false); error('abort')",
                shooter.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    scripts
        .eval_callback::<()>(&format!(
            "btech.unit.friendly_fire_safety({},1,false)",
            shooter.0
        ))
        .unwrap();
    assert!(!scripts.world().btech.vehicles()[&shooter].friendly_fire_safety());
    let value: bool = scripts
        .eval_callback(&format!(
            "return btech.unit.state({}).friendly_fire_safety",
            shooter.0
        ))
        .unwrap();
    assert!(!value);
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.friendly_fire_safety({},2,true)",
                shooter.0
            ))
            .is_err()
    );
}

#[tokio::test]
async fn vehicle_shot_checks_reject_submerged_weapons_and_depleted_ammunition() {
    let (_dir, _config, mut world, _map, [target, _, shooter, _]) = fixture(
        "~2\n~2\n~2\n~2\n~2\n",
        include_str!("../game/units/Demolisher.toml"),
    )
    .await;
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(shooter);
    assign_battle_pilot(&mut world, shooter, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    power(&mut world, &[shooter, target], Power::Running);
    let before = world.btech.clone();
    assert!(
        check_battle_vehicle_shot(&world, shooter, ObjectId(1), target, 0, shot_rules())
            .unwrap_err()
            .to_string()
            .contains("This weapon may not be fired underwater.")
    );
    assert_eq!(world.btech, before);
    let (_dir, _config, mut world, _, [target, _, shooter, _]) = engagement().await;
    world
        .btech
        .rewrite_unit_record(shooter, |record| {
            for rounds in record["ammunition"].as_array_mut().unwrap() {
                *rounds = serde_json::json!(0);
            }
        })
        .unwrap();
    let before = world.btech.clone();
    assert!(
        check_battle_vehicle_shot(&world, shooter, ObjectId(1), target, 0, shot_rules())
            .unwrap_err()
            .to_string()
            .contains("Weapon is not ready")
    );
    assert_eq!(world.btech, before);
}
