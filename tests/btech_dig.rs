//! Digging cover shares timers, controls, firing readiness, target aim and hit routing.
use crate::support;
use stompymux_rs::*;
const VEHICLE: &str = include_str!("../game/mechs/Demolisher.toml");
const MECH: &str = include_str!("fixtures/btech/mechs/JR7-D.toml");

/// A piloted target and an optional uncrewed running shooter with an acquired contact.
async fn fixture(
    target_source: &str,
    shooter_source: &str,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Cover field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "cover",
        BattleMapAsset::from_cells("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    let target = world.create(&config, "Defender".into(), Kind::Thing);
    let shooter = world.create(&config, "Shooter".into(), Kind::Thing);
    for (id, source, y) in [(target, target_source, 1), (shooter, shooter_source, 0)] {
        BattleUnitTemplate::parse("test", source)
            .unwrap()
            .create(&mut world, id)
            .unwrap();
        place_battle_unit(&mut world, id, map, 1, y).unwrap();
    }
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(target);
    assign_battle_pilot(&mut world, target, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, target, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    world
        .btech
        .rewrite_unit_record(shooter, |record| {
            record["power"] = serde_json::json!({"state":"running"});
            record["contacts"][target.0.to_string()] = serde_json::json!({"identified": true});
        })
        .unwrap();
    (dir, config, world, target, shooter)
}

fn complete(world: &mut World, id: ObjectId) {
    dig_battle_unit(world, id, ObjectId(1)).unwrap();
    for _ in 0..20 {
        advance_battle_units(world, 0);
    }
    assert_eq!(
        world.btech.vehicles()[&id].dig_state(),
        BattleDigState::covered()
    );
}

fn aim_rules() -> BattleAimRules {
    BattleAimRules {
        woods_damage: false,
        dig_bonus: 3,
        dig_only_front: false,
        hit_arc_mode: 1,
        fasa_turning: false,
        extended_movement: false,
        extended_ranges: false,
        hotload_half_minimum: false,
        override_weapon_arcs: false,
    }
}

#[tokio::test]
async fn dig_timer_native_lua_controls_shutdown_and_persistence_share_state() {
    for source in [
        VEHICLE.to_owned(),
        VEHICLE.replace("movement = \"track\"", "movement = \"wheel\""),
    ] {
        let (_dir, config, world, id, _) = fixture(&source, MECH).await;
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let lua = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let output = support::run_text(&native, &config, ObjectId(1), 1, "dig");
        assert!(output.contains("start digging"), "{output}");
        lua.eval_callback::<()>(&format!("btech.unit.dig({},1)", id.0))
            .unwrap();
        assert_eq!(native.world().btech, lua.world().btech);
        for _ in 0..19 {
            advance_battle_units(&mut lua.world_mut(), 0);
        }
        assert_eq!(
            lua.world().btech.vehicles()[&id].dig_state(),
            BattleDigState::preparing(1)
        );
        let snapshot = lua.world().clone();
        persistence::save(&config.database(), &snapshot)
            .await
            .unwrap();
        let mut restarted = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            advance_battle_units(&mut lua.world_mut(), 0),
            advance_battle_units(&mut restarted, 0)
        );
        assert_eq!(lua.world().btech, restarted.btech);
        assert!(
            battle_unit_status(&lua.world(), id, "info")
                .unwrap()
                .contains("DUG IN")
        );
        let before = lua.world().btech.clone();
        assert!(
            lua.eval_callback::<()>(&format!("btech.unit.heading({},1,90)", id.0))
                .is_err()
        );
        assert_eq!(lua.world().btech, before);
        lua.eval_callback::<()>(&format!("btech.unit.speed({},1,'stop')", id.0))
            .unwrap();
        assert_eq!(
            lua.world().btech.vehicles()[&id].dig_state(),
            BattleDigState::covered()
        );
        lua.eval_callback::<()>(&format!("btech.unit.speed({},1,1)", id.0))
            .unwrap();
        assert_eq!(
            lua.world().btech.vehicles()[&id].dig_state(),
            BattleDigState::default()
        );
        lua.eval_callback::<()>(&format!(
            "btech.unit.speed({},1,'stop'); btech.unit.dig({},1)",
            id.0, id.0
        ))
        .unwrap();
        lua.eval_callback::<()>(&format!("btech.unit.heading({},1,90)", id.0))
            .unwrap();
        assert_eq!(
            lua.world().btech.vehicles()[&id].dig_state(),
            BattleDigState::default()
        );
        lua.eval_callback::<()>(&format!(
            "btech.unit.heading({},1,0); btech.unit.dig({},1); btech.unit.stop({},1)",
            id.0, id.0, id.0
        ))
        .unwrap();
        assert_eq!(
            lua.world().btech.vehicles()[&id].dig_state(),
            BattleDigState::default()
        );
        lua.world().validate(&config).unwrap();
    }
}

#[tokio::test]
async fn dug_in_cover_is_shared_by_mech_and_vehicle_aim_with_arc_and_height_gates() {
    for source in [MECH, VEHICLE] {
        let (_dir, config, mut world, target, shooter) = fixture(VEHICLE, source).await;
        let baseline = battle_aim_modifiers(&world, shooter, target, 0, 4, aim_rules()).unwrap();
        complete(&mut world, target);
        let aim = battle_aim_modifiers(&world, shooter, target, 0, 4, aim_rules()).unwrap();
        assert_eq!(aim.dug_in, 3);
        assert_eq!(aim.subtotal().unwrap(), baseline.subtotal().unwrap() + 3);
        let rules = BattleAimRules {
            woods_damage: false,
            dig_bonus: 7,
            dig_only_front: true,
            ..aim_rules()
        };
        assert_eq!(
            battle_aim_modifiers(&world, shooter, target, 0, 4, rules)
                .unwrap()
                .dug_in,
            7
        );
        // At sixty degrees the default vehicle rules expose the side, while mode two
        // still grants front cover. Both shooter types must observe the target's arcs.
        for (mode, heading, expected) in [
            (0, 30.0, 7),
            (0, 30.001, 0),
            (0, 60.0, 0),
            (1, 45.0, 7),
            (1, 45.001, 0),
            (2, 60.0, 7),
            (2, 90.0, 7),
            (2, 90.001, 0),
        ] {
            world
                .btech
                .rewrite_unit_record(target, |record| {
                    record["motion"]["heading"] = heading.into();
                    record["motion"]["desired_heading"] = heading.into();
                })
                .unwrap();
            assert_eq!(
                battle_aim_modifiers(
                    &world,
                    shooter,
                    target,
                    0,
                    4,
                    BattleAimRules {
                        hit_arc_mode: mode,
                        ..rules
                    }
                )
                .unwrap()
                .dug_in,
                expected,
                "mode={mode}, heading={heading}"
            );
        }
        world
            .btech
            .rewrite_unit_record(target, |record| {
                record["motion"]["heading"] = 180.0.into();
                record["motion"]["desired_heading"] = 180.0.into();
            })
            .unwrap();
        assert_eq!(
            battle_aim_modifiers(&world, shooter, target, 0, 4, rules)
                .unwrap()
                .dug_in,
            0
        );
        world
            .btech
            .rewrite_unit_record(shooter, |record| {
                record["ground_elevation"] = 1.into();
            })
            .unwrap();
        assert_eq!(
            battle_aim_modifiers(&world, shooter, target, 0, 4, aim_rules())
                .unwrap()
                .dug_in,
            0
        );
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn cover_blocks_hull_weapons_at_the_shared_reservation_boundary() {
    let source = VEHICLE.replace(
        "[sections.front_side]\n",
        "[sections.front_side]\nslots = [{ at = 1, item = \"IS.MediumLaser\" }]\n",
    );
    let (_dir, _, mut world, id, _) = fixture(&source, MECH).await;
    let loadout = world.btech.vehicles()[&id].loadout().unwrap();
    let hull = loadout
        .weapons
        .iter()
        .position(|mount| mount.criticals[0].section == BattleVehicleSection::Front)
        .unwrap();
    let turret = loadout
        .weapons
        .iter()
        .position(|mount| mount.criticals[0].section == BattleVehicleSection::Turret)
        .unwrap();
    complete(&mut world, id);
    assert!(
        !world.btech.vehicles()[&id]
            .weapon_readiness(hull)
            .unwrap()
            .ready
    );
    assert!(
        world.btech.vehicles()[&id]
            .weapon_readiness(turret)
            .unwrap()
            .ready
    );
    let before = world.btech.clone();
    assert!(reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), hull, true).is_err());
    assert_eq!(world.btech, before);
    let _ = reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), turret, true).unwrap();
}

#[tokio::test]
async fn dug_in_turret_routing_uses_the_41_42_boundary_for_each_hit_table() {
    for table in [
        BattleVehicleCriticalTable::Standard,
        BattleVehicleCriticalTable::Advanced,
    ] {
        for percentage in [41, 42] {
            let (_dir, _, mut world, id, _) = fixture(VEHICLE, MECH).await;
            complete(&mut world, id);
            let seed = (0u32..100000)
                .find_map(|n| {
                    let mut seed = [0; 32];
                    seed[..4].copy_from_slice(&n.to_le_bytes());
                    let mut dice = BattleDice::seeded(seed);
                    let mut roll = dice.two_d6();
                    if table != BattleVehicleCriticalTable::Standard {
                        roll = dice.two_d6();
                    }
                    (roll == 7 && dice.die(100).unwrap() == percentage).then_some(seed)
                })
                .unwrap();
            world
                .btech
                .rewrite_unit_record(id, |record| {
                    record["dice"] = serde_json::to_value(BattleDice::seeded(seed)).unwrap();
                })
                .unwrap();
            let mut replay = world.clone();
            let mut rules = BattleVehicleImpactRules::STANDARD;
            rules.criticals.table = table;
            let report =
                resolve_battle_vehicle_impact(&mut world, id, BattleHitArc::Front, 1, None, rules)
                    .unwrap();
            assert_eq!(
                report,
                resolve_battle_vehicle_impact(&mut replay, id, BattleHitArc::Front, 1, None, rules)
                    .unwrap()
            );
            assert_eq!(
                report.hit.unwrap().section,
                if percentage == 42 {
                    BattleVehicleSection::Turret
                } else {
                    BattleVehicleSection::Front
                }
            );
            assert_eq!(world.btech, replay.btech);
        }
    }
}

#[tokio::test]
async fn unsupported_chassis_and_invalid_saved_countdowns_are_rejected() {
    for source in [
        MECH,
        include_str!("../game/mechs/J_Edgar.toml"),
        include_str!("../game/mechs/Kestrel.toml"),
    ] {
        let (_dir, _, mut world, id, _) = fixture(source, MECH).await;
        let before = world.btech.clone();
        assert!(dig_battle_unit(&mut world, id, ObjectId(1)).is_err());
        assert_eq!(world.btech, before);
    }
    let (_dir, config, mut world, id, _) = fixture(VEHICLE, MECH).await;
    dig_battle_unit(&mut world, id, ObjectId(1)).unwrap();
    let saved = serde_json::to_value(&world.btech).unwrap();
    for remaining in [21, 256] {
        let mut invalid = saved.clone();
        invalid["vehicles"][id.0.to_string()]["dig"]["completion"] = serde_json::json!([remaining]);
        assert!(serde_json::from_value::<BtechState>(invalid).is_err());
    }
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    scripts
        .eval_callback::<()>(&format!("btech.unit.heading({},1,0)", id.0))
        .unwrap();
    scripts.drain_outbox();
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!("btech.unit.dig({},1); error('abort cover')", id.0))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
}

#[tokio::test]
async fn pickup_clears_completed_cover_but_shutdown_preserves_it() {
    let (_dir, config, mut world, target, carrier) =
        fixture(VEHICLE, include_str!("fixtures/btech/mechs/AS7-D.toml")).await;
    complete(&mut world, target);
    stop_battle_unit(
        &mut world,
        target,
        ObjectId(1),
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    assert_eq!(
        world.btech.vehicles()[&target].dig_state(),
        BattleDigState::covered()
    );
    let position = world.btech.vehicles()[&target].position().unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(carrier);
    assign_battle_pilot(&mut world, carrier, ObjectId(1)).unwrap();
    let _ = stop_battle_unit(
        &mut world,
        carrier,
        ObjectId(1),
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    place_battle_unit(
        &mut world,
        carrier,
        position.map,
        i64::from(position.x),
        i64::from(position.y),
    )
    .unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(carrier);
    assign_battle_pilot(&mut world, carrier, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, carrier, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    set_battle_towable(&mut world, target, true).unwrap();
    // Administrative placement clears acquired contacts; restore the carrier's explicit acquisition.
    world
        .btech
        .rewrite_unit_record(carrier, |record| {
            record["contacts"][target.0.to_string()] = serde_json::json!({"identified": true});
        })
        .unwrap();
    let before = world.btech.clone();
    assert!(set_battle_tow(&mut world, carrier, Some(target)).is_err());
    assert_eq!(world.btech, before);
    let _ = pickup_battle_unit(
        &mut world,
        carrier,
        ObjectId(1),
        target,
        BattleMovementRules::STANDARD.fall,
        true,
    )
    .unwrap();
    assert_eq!(
        world.btech.vehicles()[&target].dig_state(),
        BattleDigState::default()
    );
    world.validate(&config).unwrap();
}

/// Surface admission must reject hard cover without spending time or changing state.
#[tokio::test]
async fn digging_checks_the_current_surface_before_starting() {
    for terrain in [
        Terrain::Road,
        Terrain::Bridge,
        Terrain::Building,
        Terrain::Wall,
        Terrain::Water,
        Terrain::Ice,
        Terrain::Grassland,
    ] {
        let (_dir, _, mut world, id, _) = fixture(VEHICLE, MECH).await;
        let map = world.btech.vehicles()[&id].position().unwrap().map;
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        crate::support::set_hex_terrain(
            &mut saved["maps"][map.0.to_string()]["terrain"][4],
            terrain,
        );
        world.btech = serde_json::from_value(saved).unwrap();
        let before = world.btech.clone();
        let result = dig_battle_unit(&mut world, id, ObjectId(1));
        if matches!(terrain, Terrain::Ice | Terrain::Grassland) {
            result.unwrap();
            assert_eq!(
                world.btech.vehicles()[&id].dig_state(),
                BattleDigState::preparing(20)
            );
        } else {
            assert!(result.is_err(), "{terrain:?}");
            assert_eq!(world.btech, before);
        }
    }
}

/// Raw flags preserve older dig events when a new native preparation is started.
#[tokio::test]
async fn raw_dig_flags_preserve_overlapping_completion_deadlines() {
    use std::{cell::RefCell, rc::Rc};
    for source in [
        VEHICLE.to_owned(),
        VEHICLE.replace("movement = \"track\"", "movement = \"wheel\""),
    ] {
        let (_dir, config, mut world, id, _) = fixture(&source, MECH).await;
        dig_battle_unit(&mut world, id, ObjectId(1)).unwrap();
        for _ in 0..3 {
            advance_battle_units(&mut world, 0);
        }
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "tankcritstatus", "-")
            .unwrap();
        assert_eq!(
            scripts.world().btech.vehicles()[&id]
                .dig_state()
                .remaining(),
            17
        );
        dig_battle_unit(&mut scripts.world_mut(), id, ObjectId(1)).unwrap();
        let saved = scripts.world().clone();
        assert_eq!(
            saved.btech.vehicles()[&id].dig_state().completion,
            [17, 20].into_iter().collect()
        );
        persistence::save(&config.database(), &saved).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, saved.btech);
        for _ in 0..17 {
            advance_battle_units(&mut loaded, 0);
        }
        let dig = loaded.btech.vehicles()[&id].dig_state();
        assert!(dig.dug_in && !dig.digging);
        assert_eq!(dig.remaining(), 3);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
        set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "tankcritstatus", "d")
            .unwrap();
        for _ in 0..3 {
            advance_battle_units(&mut scripts.world_mut(), 0);
        }
        assert_eq!(
            scripts.world().btech.vehicles()[&id].dig_state(),
            BattleDigState::covered()
        );
    }
}
