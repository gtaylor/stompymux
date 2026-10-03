//! Ice and bridge breakage own terrain, neighboring falls and durable rollback together.
use crate::support;
use stompymux_rs::*;

/// Two running bipeds occupy the same ice tile, with independent persisted dice and pilots.
async fn fixture(depth: u8) -> (tempfile::TempDir, Config, World, ObjectId, [ObjectId; 2]) {
    fixture_surface(Terrain::Ice, depth).await
}

/// Build the same occupied field for either breakable surface.
async fn fixture_surface(
    terrain: Terrain,
    depth: u8,
) -> (tempfile::TempDir, Config, World, ObjectId, [ObjectId; 2]) {
    fixture_field(terrain, depth, 3).await
}

/// A configurable north/south field for terrain transitions and optical paths.
async fn fixture_field(
    terrain: Terrain,
    depth: u8,
    height: usize,
) -> (tempfile::TempDir, Config, World, ObjectId, [ObjectId; 2]) {
    let row = format!("{}{depth}", terrain.symbol()).repeat(3) + "\n";
    fixture_asset(
        BattleMapAsset::from_cells(&format!("3 {height}\n{}", row.repeat(height))).unwrap(),
    )
    .await
}

/// Construct two running bipeds on an isolated, caller-defined terrain asset.
async fn fixture_asset(
    asset: BattleMapAsset,
) -> (tempfile::TempDir, Config, World, ObjectId, [ObjectId; 2]) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Surface field".into(), Kind::Room);
    create_battle_map(&mut world, map, "surface.map", asset).unwrap();
    let mut units = Vec::new();
    for pilot in [ObjectId(1), ObjectId(2)] {
        let id = world.create(&config, format!("Surface unit {}", pilot.0), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        create_battle_unit(
            &mut world,
            id,
            BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml"))
                .unwrap(),
        )
        .unwrap();
        place_battle_unit(&mut world, id, map, 1, 1).unwrap();
        world.objects.get_mut(&pilot).unwrap().location = Some(id);
        assign_battle_pilot(&mut world, id, pilot).unwrap();
        start_battle_unit(&mut world, id, pilot, true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
        let mut state = serde_json::to_value(&world.btech).unwrap();
        let unit = &mut state["constructed"][id.0.to_string()];
        unit["dice"] = serde_json::to_value(BattleDice::seeded([19; 32])).unwrap();
        unit["crew_recovery"]["dice"] = serde_json::to_value(BattleDice::seeded([13; 32])).unwrap();
        state["recoveries"][pilot.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([11; 32])).unwrap();
        world.btech = serde_json::from_value(state).unwrap();
        units.push(id);
    }
    (dir, config, world, map, [units[0], units[1]])
}

/// Ordinary configured tactical fall semantics for deterministic fracture tests.
fn rules() -> BattleFallRules {
    BattleFallRules {
        vehicle_impact: stompymux_rs::BattleVehicleImpactRules::STANDARD,
        stacking: stompymux_rs::BattleStackingRules::STANDARD,
        stagger: BattleStaggerMode::Retain,
        hit: BattleHitRules {
            inferno_penalty: false,
            exile_stun_mode: 0,
        },
        extended_piloting: true,
        toughness: false,
    }
}

#[tokio::test]
async fn ice_fracture_drops_neighbors_before_trigger_and_replays_after_restart() {
    let (_dir, config, mut world, map, units) = fixture(3).await;
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restarted = persistence::load(&config.database()).await.unwrap();
    let coordinate = BattleHexCoordinate { x: 1, y: 1 };
    let report = break_battle_ice(&mut world, map, coordinate, Some(units[0]), rules()).unwrap();
    assert_eq!(
        report,
        break_battle_ice(&mut restarted, map, coordinate, Some(units[0]), rules()).unwrap()
    );
    assert_eq!(world.btech, restarted.btech);
    assert_eq!(
        report.falls.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        [units[1], units[0]]
    );
    assert!(report.falls.iter().all(|(_, fall)| fall.damage == 6));
    assert_eq!(
        world.btech.maps()[&map].hex(1, 1).unwrap(),
        BattleHex::new(Terrain::Water, 3)
    );
    for id in units {
        assert_eq!(
            world.btech.constructed_units()[&id].posture(),
            BattlePosture::Prone
        );
    }
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    let before = world.btech.clone();
    assert!(break_battle_ice(&mut world, map, coordinate, None, rules()).is_err());
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn fracture_failure_restores_terrain_neighbor_damage_and_dice() {
    let (_dir, config, mut world, map, units) = fixture(3).await;
    // The neighbor resolves first; the trigger then rejects unsupported character casualties.
    world
        .objects
        .get_mut(&units[0])
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let before = world.btech.clone();
    assert!(
        break_battle_ice(
            &mut world,
            map,
            BattleHexCoordinate { x: 1, y: 1 },
            Some(units[0]),
            rules()
        )
        .is_err()
    );
    assert_eq!(world.btech, before);
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn fracture_save_failure_keeps_occupied_terrain_and_units_atomic() {
    use sqlx::Connection;
    let (_dir, config, mut world, map, units) = fixture(3).await;
    persistence::save(&config.database(), &world).await.unwrap();
    let before = world.btech.clone();
    let report = break_battle_ice(
        &mut world,
        map,
        BattleHexCoordinate { x: 1, y: 1 },
        None,
        rules(),
    )
    .unwrap();
    assert_eq!(report.falls.len(), 2);
    let mut sql = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE TRIGGER reject_ice_effect BEFORE UPDATE ON btech_units WHEN NEW.dbref={} BEGIN SELECT RAISE(ABORT,'ice unit failure'); END",units[1].0))).execute(&mut sql).await.unwrap();
    assert!(persistence::save(&config.database(), &world).await.is_err());
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        before
    );
    sqlx::query("DROP TRIGGER reject_ice_effect")
        .execute(&mut sql)
        .await
        .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

#[tokio::test]
async fn zero_depth_ice_changes_terrain_without_fall_dice() {
    let (_dir, config, mut world, map, units) = fixture(0).await;
    let before: Vec<_> = units
        .iter()
        .map(|id| world.btech.constructed_units()[id].clone())
        .collect();
    let report = break_battle_ice(
        &mut world,
        map,
        BattleHexCoordinate { x: 1, y: 1 },
        None,
        rules(),
    )
    .unwrap();
    assert!(report.falls.is_empty());
    for (id, expected) in units.into_iter().zip(before) {
        assert_eq!(world.btech.constructed_units()[&id], expected);
    }
    world.validate(&config).unwrap();
}

/// Seed the next landing roll while leaving subsequent fall dice reproducible.
fn ice_seed(world: &mut World, id: ObjectId, fracture: bool, avoidance_first: bool) {
    let seed = (0..=255)
        .find(|seed| {
            let mut dice = BattleDice::seeded([*seed; 32]);
            if avoidance_first {
                dice.two_d6();
            }
            (dice.d6() == 1) == fracture
        })
        .unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
}

#[tokio::test]
async fn jump_onto_ice_uses_surface_height_and_commits_seeded_landing_after_restart() {
    for fracture in [false, true] {
        let (_dir, config, mut world, map, units) = fixture(3).await;
        let id = units[0];
        launch_battle_jump(&mut world, id, ObjectId(1), 0, 1.0).unwrap();
        let path = world.btech.constructed_units()[&id]
            .flight()
            .unwrap()
            .path();
        assert_eq!(path.sample(0.0, 5).unwrap().elevation, 0.0);
        assert_eq!(path.sample(1.0, 5).unwrap().elevation, 0.0);
        ice_seed(&mut world, id, fracture, false);
        for _ in 0..11 {
            advance_battle_jumps(
                &mut world,
                stompymux_rs::BattleMovementRules {
                    fall: stompymux_rs::BattleFallRules {
                        stacking: stompymux_rs::BattleStackingRules::STANDARD,
                        ..rules()
                    },
                    ..stompymux_rs::BattleMovementRules::STANDARD
                },
            )
            .unwrap();
        }
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restarted = persistence::load(&config.database()).await.unwrap();
        let notices = advance_battle_jumps(
            &mut world,
            stompymux_rs::BattleMovementRules {
                fall: stompymux_rs::BattleFallRules {
                    stacking: stompymux_rs::BattleStackingRules::STANDARD,
                    ..rules()
                },
                ..stompymux_rs::BattleMovementRules::STANDARD
            },
        )
        .unwrap();
        assert_eq!(
            notices,
            advance_battle_jumps(
                &mut restarted,
                stompymux_rs::BattleMovementRules {
                    fall: stompymux_rs::BattleFallRules {
                        stacking: stompymux_rs::BattleStackingRules::STANDARD,
                        ..rules()
                    },
                    ..stompymux_rs::BattleMovementRules::STANDARD
                }
            )
            .unwrap()
        );
        assert_eq!(world.btech, restarted.btech);
        let unit = &world.btech.constructed_units()[&id];
        assert!(unit.flight().is_none());
        assert_eq!(unit.jump_stabilization(), 12);
        assert_eq!(
            unit.posture(),
            if fracture {
                BattlePosture::Prone
            } else {
                BattlePosture::Standing
            }
        );
        assert_eq!(
            world.btech.maps()[&map].hex(1, 0).unwrap().terrain(),
            if fracture {
                Terrain::Water
            } else {
                Terrain::Ice
            }
        );
        assert_eq!(
            notices
                .iter()
                .filter(|notice| notice.text == "You break the ice!")
                .count(),
            usize::from(fracture)
        );
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            world.btech,
            persistence::load(&config.database()).await.unwrap().btech
        );
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn fall_onto_ice_reports_nested_fracture_and_uses_resulting_water_damage() {
    for fracture in [false, true] {
        let (_dir, config, mut world, map, units) = fixture(3).await;
        let id = units[0];
        ice_seed(&mut world, id, fracture, true);
        let report = resolve_battle_fall(&mut world, id, 1, rules()).unwrap();
        assert_eq!(report.ice_break.is_some(), fracture);
        assert_eq!(report.damage, if fracture { 2 } else { 4 });
        if let Some(ice) = &report.ice_break {
            assert_eq!(ice.falls.len(), 2);
            assert_eq!(ice.falls.last().unwrap().0, id);
            assert_eq!(ice.falls.last().unwrap().1.damage, 6);
        }
        assert_eq!(
            world.btech.maps()[&map].hex(1, 1).unwrap().terrain(),
            if fracture {
                Terrain::Water
            } else {
                Terrain::Ice
            }
        );
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn destination_fracture_during_flight_preserves_the_saved_launch_path() {
    let (_dir, config, mut world, map, units) = fixture(3).await;
    let id = units[0];
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 1.0).unwrap();
    for _ in 0..4 {
        advance_battle_jumps(
            &mut world,
            stompymux_rs::BattleMovementRules {
                fall: stompymux_rs::BattleFallRules {
                    stacking: stompymux_rs::BattleStackingRules::STANDARD,
                    ..rules()
                },
                ..stompymux_rs::BattleMovementRules::STANDARD
            },
        )
        .unwrap();
    }
    let path = world.btech.constructed_units()[&id]
        .flight()
        .unwrap()
        .path();
    let fracture = break_battle_ice(
        &mut world,
        map,
        BattleHexCoordinate { x: 1, y: 0 },
        None,
        rules(),
    )
    .unwrap();
    assert!(fracture.falls.is_empty());
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        loaded.btech.constructed_units()[&id]
            .flight()
            .unwrap()
            .path(),
        path
    );
    for _ in 0..8 {
        advance_battle_jumps(
            &mut loaded,
            stompymux_rs::BattleMovementRules {
                fall: stompymux_rs::BattleFallRules {
                    stacking: stompymux_rs::BattleStackingRules::STANDARD,
                    ..rules()
                },
                ..stompymux_rs::BattleMovementRules::STANDARD
            },
        )
        .unwrap();
    }
    assert!(loaded.btech.constructed_units()[&id].flight().is_none());
    // One unit is on intact ice at zero; the other landed on the water bottom at -3.
    let range = battle_unit_range(&loaded, id, units[1]).unwrap();
    assert!((range.spatial - 1.0_f64.hypot(0.6)).abs() < 1e-9);
    loaded.validate(&config).unwrap();
}

#[tokio::test]
async fn early_ice_landing_native_lua_parity_and_callback_rollback_include_the_map() {
    let (_dir, config, mut world, _map, units) = fixture(3).await;
    let id = units[0];
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 1.0).unwrap();
    for _ in 0..4 {
        advance_battle_jumps(
            &mut world,
            stompymux_rs::BattleMovementRules {
                fall: stompymux_rs::BattleFallRules {
                    stacking: stompymux_rs::BattleStackingRules::STANDARD,
                    ..rules()
                },
                ..stompymux_rs::BattleMovementRules::STANDARD
            },
        )
        .unwrap();
    }
    let seed = (0..=255)
        .find(|seed| {
            let mut dice = BattleDice::seeded([*seed; 32]);
            dice.two_d6() >= 6 && dice.d6() == 1
        })
        .unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.unit.land({},1); error('discard broken ice')",
            id.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, world.btech);
    let text = support::run_text(&native, &config, ObjectId(1), 1, "land");
    assert!(text.contains("You break the ice!"), "{text}");
    assert!(
        lua.eval_callback::<bool>(&format!("return btech.unit.land({},1)", id.0))
            .unwrap()
    );
    assert_eq!(native.world().btech, lua.world().btech);
    native.world().validate(&config).unwrap();
}

#[tokio::test]
async fn bridge_collapse_drops_deck_occupants_to_the_river_bed() {
    for height in [0, 1, 3, 9] {
        let (_dir, config, mut world, map, units) = fixture_surface(Terrain::Bridge, height).await;
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restarted = persistence::load(&config.database()).await.unwrap();
        let coordinate = BattleHexCoordinate { x: 1, y: 1 };
        let report = break_battle_bridge(&mut world, map, coordinate, rules()).unwrap();
        assert_eq!(
            report,
            break_battle_bridge(&mut restarted, map, coordinate, rules()).unwrap()
        );
        assert_eq!(world.btech, restarted.btech);
        assert_eq!(report.before, BattleHex::new(Terrain::Bridge, height));
        assert_eq!(report.after, BattleHex::new(Terrain::Water, 1));
        // Both units stand on the deck and fall past it into the depth-one water below.
        assert_eq!(report.fall_levels, height + 1);
        assert_eq!(
            report.falls.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            units.to_vec()
        );
        let damage = u32::from(height + 1) * (35 + 5) / 20;
        assert!(report.falls.iter().all(|(_, fall)| fall.damage == damage));
        assert_eq!(world.btech.maps()[&map].hex(1, 1).unwrap(), report.after);
        assert_eq!(world.btech.maps()[&map].hex(0, 0).unwrap(), report.before);
        for id in units {
            let state = serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap();
            assert_eq!(state["ground_elevation"], serde_json::Value::Null);
        }

        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

#[tokio::test]
async fn bridge_collapse_rejects_incomplete_effects_and_rolls_back_failed_writes() {
    use sqlx::Connection;
    let (_dir, config, mut world, map, units) = fixture_surface(Terrain::Bridge, 1).await;
    persistence::save(&config.database(), &world).await.unwrap();
    let before = world.btech.clone();
    world
        .objects
        .get_mut(&units[1])
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    assert!(
        break_battle_bridge(&mut world, map, BattleHexCoordinate { x: 1, y: 1 }, rules()).is_err()
    );
    assert_eq!(world.btech, before);
    world
        .objects
        .get_mut(&units[1])
        .unwrap()
        .flags
        .remove(Flag::InCharacter);
    let report =
        break_battle_bridge(&mut world, map, BattleHexCoordinate { x: 1, y: 1 }, rules()).unwrap();
    assert_eq!(report.falls.len(), 2);
    let mut sql = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE TRIGGER reject_bridge_effect BEFORE UPDATE ON btech_units WHEN NEW.dbref={} BEGIN SELECT RAISE(ABORT,'bridge effect failure'); END",units[1].0))).execute(&mut sql).await.unwrap();
    assert!(persistence::save(&config.database(), &world).await.is_err());
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        before
    );
    sqlx::query("DROP TRIGGER reject_bridge_effect")
        .execute(&mut sql)
        .await
        .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

#[tokio::test]
async fn retained_altitude_over_water_drives_range_launch_and_placement_after_restart() {
    let (_dir, config, mut world, map, units) = fixture_surface(Terrain::Water, 1).await;
    // The first unit keeps an altitude of +3 over the water; its neighbor stands on the bed.
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][units[0].0.to_string()]["ground_elevation"] = serde_json::json!(3.0);
    world.btech = serde_json::from_value(state).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    // The retained unit is at +3; its fallen neighbor is at -1 in the same hex.
    assert!(
        (battle_unit_range(&loaded, units[0], units[1])
            .unwrap()
            .spatial
            - 0.8)
            .abs()
            < 1e-9
    );
    let mut placed = loaded.clone();
    stop_battle_unit(
        &mut placed,
        units[0],
        ObjectId(1),
        stompymux_rs::BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    assert!(
        (battle_unit_range(&placed, units[0], units[1])
            .unwrap()
            .spatial
            - 0.8)
            .abs()
            < 1e-9
    );
    place_battle_unit(&mut placed, units[0], map, 1, 1).unwrap();
    assert_eq!(
        battle_unit_range(&placed, units[0], units[1])
            .unwrap()
            .spatial,
        0.0
    );
    placed.validate(&config).unwrap();

    launch_battle_jump(&mut loaded, units[0], ObjectId(1), 0, 1.0).unwrap();
    assert_eq!(
        loaded.btech.constructed_units()[&units[0]]
            .flight()
            .unwrap()
            .sample()
            .elevation,
        3.0
    );
    assert!(serde_json::to_value(&loaded.btech.constructed_units()[&units[0]]).unwrap()["ground_elevation"].is_null());
    for _ in 0..20 {
        advance_battle_jumps(
            &mut loaded,
            stompymux_rs::BattleMovementRules {
                fall: stompymux_rs::BattleFallRules {
                    stacking: stompymux_rs::BattleStackingRules::STANDARD,
                    ..rules()
                },
                ..stompymux_rs::BattleMovementRules::STANDARD
            },
        )
        .unwrap();
    }
    assert!(
        loaded.btech.constructed_units()[&units[0]]
            .flight()
            .is_none()
    );
    assert!(
        (battle_unit_range(&loaded, units[0], units[1])
            .unwrap()
            .spatial
            - 1.0)
            .abs()
            < 1e-9
    );
    loaded.validate(&config).unwrap();
    persistence::save(&config.database(), &loaded)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        loaded.btech
    );
}

#[tokio::test]
async fn ice_surface_contacts_and_fire_match_native_lua_after_restart() {
    for (depth, airborne) in [0, 1, 3, 9]
        .into_iter()
        .flat_map(|depth| [false, true].map(|airborne| (depth, airborne)))
    {
        let (_dir, config, mut world, map, units) = fixture(depth).await;
        let [shooter, target] = units;
        stop_battle_unit(
            &mut world,
            target,
            ObjectId(2),
            stompymux_rs::BattleMovementRules::STANDARD.fall,
        )
        .unwrap();
        place_battle_unit(&mut world, target, map, 1, 0).unwrap();
        assert!(
            !battle_unit_terrain_los(&world, shooter, target)
                .unwrap()
                .blocked
        );
        assert!(
            !battle_unit_terrain_los(&world, target, shooter)
                .unwrap()
                .blocked
        );
        let mut state = serde_json::to_value(&world.btech).unwrap();
        for (index, id) in units.into_iter().enumerate() {
            state["constructed"][id.0.to_string()]["dice"] =
                serde_json::to_value(BattleDice::seeded([index as u8; 32])).unwrap();
        }
        world.btech = serde_json::from_value(state).unwrap();
        if airborne {
            launch_battle_jump(&mut world, shooter, ObjectId(1), 0, 1.0).unwrap();
            for _ in 0..6 {
                advance_battle_jumps(
                    &mut world,
                    stompymux_rs::BattleMovementRules {
                        fall: stompymux_rs::BattleFallRules {
                            stacking: stompymux_rs::BattleStackingRules::STANDARD,
                            ..rules()
                        },
                        ..stompymux_rs::BattleMovementRules::STANDARD
                    },
                )
                .unwrap();
            }
        }
        refresh_battle_contacts(&mut world, &[shooter]).unwrap();
        assert!(
            world.btech.constructed_units()[&shooter]
                .contacts()
                .contains_key(&target)
        );
        persistence::save(&config.database(), &world).await.unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let lua = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(loaded))).unwrap();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.fire({},1,0,{}); error('abort ice shot')",
                shooter.0, target.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, world.btech);
        let text = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("fire 0 #{}", target.0),
        );
        let modifier: i64 = lua
            .eval_callback(&format!(
                "local r=btech.unit.fire({},1,0,{}); return r.aim.attacker_movement",
                shooter.0, target.0
            ))
            .unwrap();
        assert_eq!(modifier, if airborne { 3 } else { 0 });
        assert!(
            native.world().btech.constructed_units()[&shooter]
                .weapon_readiness(0)
                .unwrap()
                .recycle_remaining
                > 0,
            "{text}"
        );
        assert_eq!(native.world().btech, lua.world().btech);
        assert_eq!(
            native.world().btech.maps()[&map]
                .hex(1, 1)
                .unwrap()
                .terrain(),
            Terrain::Ice
        );
        native.world().validate(&config).unwrap();
        let saved = native.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            native.world().btech
        );
    }
}

#[tokio::test]
async fn ice_standing_native_lua_and_restart_cover_success_failure_and_fracture() {
    for (success, fracture, mode) in [
        (true, false, "normal"),
        (false, false, "normal"),
        (false, true, "normal"),
        (true, false, "careful"),
        (false, true, "careful"),
        (false, false, "anyway"),
    ] {
        let (_dir, config, mut world, map, units) = fixture(3).await;
        let id = units[0];
        // Begin with an actual non-injuring fall that leaves the ice intact.
        let seed = (0..=255)
            .find(|seed| {
                let mut dice = BattleDice::seeded([*seed; 32]);
                dice.two_d6() >= 7 && dice.d6() != 1
            })
            .unwrap();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][id.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        world.btech = serde_json::from_value(state).unwrap();
        let initial = resolve_battle_fall(&mut world, id, 1, rules()).unwrap();
        assert!(initial.avoidance.unwrap().success && initial.ice_break.is_none());
        let seed = (0..=255)
            .find(|seed| {
                let mut dice = BattleDice::seeded([*seed; 32]);
                if (dice.two_d6() >= if mode == "careful" { 4 } else { 6 }) != success {
                    return false;
                }
                success || (dice.two_d6() >= 7 && (dice.d6() == 1) == fracture)
            })
            .unwrap();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][id.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        state["constructed"][units[1].0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([19; 32])).unwrap();
        world.btech = serde_json::from_value(state).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let lua = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(
                persistence::load(&config.database()).await.unwrap(),
            )),
        )
        .unwrap();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.stand({},1,'{mode}'); error('abort ice stand')",
                id.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, world.btech);
        assert!(
            lua.eval_callback::<()>(&format!("btech.unit.stand({},1,'unknown')", id.0))
                .is_err()
        );
        assert_eq!(lua.world().btech, world.btech);
        let text = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            if mode == "normal" {
                "stand"
            } else if mode == "careful" {
                "stand careful"
            } else {
                "stand anyway"
            },
        );
        let actual: (bool, bool) = lua.eval_callback(&format!("local r=btech.unit.stand({},1,'{mode}'); return r.check.success, r.fall ~= nil and r.fall.ice_break ~= nil", id.0)).unwrap();
        assert_eq!(actual, (success, fracture), "{text}");
        assert_eq!(native.world().btech, lua.world().btech);
        let mut expected = native.world().clone();
        assert_eq!(
            expected.btech.maps()[&map].hex(1, 1).unwrap().terrain(),
            if fracture {
                Terrain::Water
            } else {
                Terrain::Ice
            }
        );
        assert_eq!(
            expected.btech.constructed_units()[&id].posture(),
            if success {
                BattlePosture::Standing
            } else {
                BattlePosture::Prone
            }
        );
        assert_eq!(
            expected.btech.constructed_units()[&units[1]].posture(),
            if fracture {
                BattlePosture::Prone
            } else {
                BattlePosture::Standing
            }
        );
        expected.validate(&config).unwrap();
        persistence::save(&config.database(), &expected)
            .await
            .unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        while expected.btech.constructed_units()[&id]
            .stand_timer()
            .is_some()
        {
            assert_eq!(
                advance_battle_standing(&mut expected),
                advance_battle_standing(&mut loaded)
            );
            assert_eq!(expected.btech, loaded.btech);
        }
        loaded.validate(&config).unwrap();
    }
}

/// Six water hexes break the clear line only once the target settles below the waterline,
/// using the retained altitude before and after a restart.
#[tokio::test]
async fn optical_water_attenuation_uses_retained_altitude_after_restart() {
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::Water, 1, 7).await;
    for (pilot, id, y) in [(ObjectId(1), units[0], 0), (ObjectId(2), units[1], 6)] {
        stop_battle_unit(
            &mut world,
            id,
            pilot,
            stompymux_rs::BattleMovementRules::STANDARD.fall,
        )
        .unwrap();
        place_battle_unit(&mut world, id, map, 1, y).unwrap();
    }
    // A saved prone unit retained at elevation zero has its eyes above the waterline.
    let mut state = serde_json::to_value(&world.btech).unwrap();
    for id in units {
        state["constructed"][id.0.to_string()]["posture"] = serde_json::json!("prone");
        state["constructed"][id.0.to_string()]["ground_elevation"] = serde_json::json!(0.0);
    }
    world.btech = serde_json::from_value(state).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    let check = |world: &World, perceived: bool| {
        let terrain = battle_unit_terrain_los(world, units[0], units[1]).unwrap();
        assert!(!terrain.blocked);
        assert_eq!(terrain.water, 6);
        let perception = battle_perceive(world, units[0], units[1]).unwrap();
        assert_eq!(
            perception.map(|perception| perception.channel),
            perceived.then_some(BattleDetectionChannel::Sensors)
        );
    };
    check(&world, true);
    check(&loaded, true);
    // Administrative placement settles both prone units onto the water bottom.
    for (id, y) in [(units[0], 0), (units[1], 6)] {
        place_battle_unit(&mut loaded, id, map, 1, y).unwrap();
    }
    check(&loaded, false);
    loaded.validate(&config).unwrap();
}

#[tokio::test]
async fn bridge_deck_contacts_refresh_after_collapse_and_restart() {
    for height in [0, 1, 3, 9] {
        let (_dir, config, mut world, map, units) = fixture_surface(Terrain::Bridge, height).await;
        let [observer, target] = units;
        stop_battle_unit(
            &mut world,
            target,
            ObjectId(2),
            stompymux_rs::BattleMovementRules::STANDARD.fall,
        )
        .unwrap();
        place_battle_unit(&mut world, target, map, 1, 0).unwrap();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        for (index, id) in units.into_iter().enumerate() {
            state["constructed"][id.0.to_string()]["dice"] =
                serde_json::to_value(BattleDice::seeded([index as u8; 32])).unwrap();
        }
        world.btech = serde_json::from_value(state).unwrap();
        assert!(battle_contact_observers(&world).contains(&observer));
        let events = refresh_battle_contacts(&mut world, &[observer]).unwrap();
        assert_eq!(events.len(), 1);
        assert!(events[0].acquired);
        let scripts = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let text = support::run_text(&scripts, &config, ObjectId(1), 1, "contacts");
        assert!(
            text.contains(&format!(
                "[{}]",
                stompymux_rs::visible_battle_contact(&world, observer, target)
                    .unwrap()
                    .unwrap()
                    .label
            )),
            "{text}"
        );
        let lua_target: i64 = scripts
            .eval_callback(&format!(
                "return btech.unit.contacts({})[1].target",
                observer.0
            ))
            .unwrap();
        assert_eq!(lua_target, target.0);
        assert_eq!(scripts.world().btech, world.btech);
        let collapse =
            break_battle_bridge(&mut world, map, BattleHexCoordinate { x: 1, y: 0 }, rules())
                .unwrap();
        // The target stood on the deck, so it falls into the water and drops out of sight.
        assert_eq!(collapse.falls.len(), 1);
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        let expected = refresh_battle_contacts(&mut world, &[observer]).unwrap();
        assert_eq!(
            refresh_battle_contacts(&mut loaded, &[observer]).unwrap(),
            expected
        );
        assert_eq!(world.btech, loaded.btech);
        assert_eq!(expected.len(), 1);
        assert!(!expected[0].acquired);
        assert!(
            visible_battle_contacts(&loaded, observer)
                .unwrap()
                .is_empty()
        );
        loaded.validate(&config).unwrap();
        persistence::save(&config.database(), &loaded)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            loaded.btech
        );
    }
}

/// Both inspection adapters expose the same derived altitude without mutating gameplay state.
fn assert_elevation_inspection(
    config: &Config,
    world: &World,
    id: ObjectId,
    elevation: Option<i32>,
) {
    assert_eq!(battle_unit_elevation(world, id).unwrap(), elevation);
    let scripts = Scripts::new(
        config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let actual: Option<i32> = scripts
        .eval_callback(&format!("return btech.unit.state({}).elevation", id.0))
        .unwrap();
    assert_eq!(actual, elevation);
    let text = support::run_text(
        &scripts,
        config,
        ObjectId(1),
        1,
        &format!("@btech inspect #{}", id.0),
    );
    if let Some(elevation) = elevation {
        assert!(
            text.lines()
                .any(|line| line == format!("Elevation {elevation}")),
            "{text}"
        );
    } else {
        assert!(!text.contains("Elevation "), "{text}");
    }
    scripts
        .eval_callback::<()>(&format!("btech.unit.state({}).elevation = 999", id.0))
        .unwrap();
    assert_eq!(scripts.world().btech, world.btech);
}

#[tokio::test]
async fn elevation_inspection_tracks_surface_collapse_flight_and_unplaced_state() {
    for (terrain, depth, elevation) in [
        (Terrain::Water, 3, -3),
        (Terrain::Ice, 9, 0),
        (Terrain::Grassland, 4, 4),
        (Terrain::Bridge, 3, 3),
    ] {
        let (_dir, config, mut world, map, units) = fixture_surface(terrain, depth).await;
        let id = units[0];
        assert_elevation_inspection(&config, &world, id, Some(elevation));
        if terrain == Terrain::Bridge {
            let report =
                break_battle_bridge(&mut world, map, BattleHexCoordinate { x: 1, y: 1 }, rules())
                    .unwrap();
            // Both units stood on the deck and fall to the bed of the depth-one water.
            assert_eq!(report.falls.len(), 2);
            persistence::save(&config.database(), &world).await.unwrap();
            world = persistence::load(&config.database()).await.unwrap();
            assert_elevation_inspection(&config, &world, id, Some(-1));
        }
        if terrain == Terrain::Grassland {
            launch_battle_jump(&mut world, id, ObjectId(1), 0, 1.0).unwrap();
            assert_elevation_inspection(&config, &world, id, Some(4));
            for _ in 0..6 {
                advance_battle_jumps(
                    &mut world,
                    stompymux_rs::BattleMovementRules {
                        fall: stompymux_rs::BattleFallRules {
                            stacking: stompymux_rs::BattleStackingRules::STANDARD,
                            ..rules()
                        },
                        ..stompymux_rs::BattleMovementRules::STANDARD
                    },
                )
                .unwrap();
                let sample = world.btech.constructed_units()[&id]
                    .flight()
                    .unwrap()
                    .sample();
                assert_elevation_inspection(
                    &config,
                    &world,
                    id,
                    Some((sample.elevation + 0.5).trunc() as i32),
                );
            }
            for _ in 0..20 {
                advance_battle_jumps(
                    &mut world,
                    stompymux_rs::BattleMovementRules {
                        fall: stompymux_rs::BattleFallRules {
                            stacking: stompymux_rs::BattleStackingRules::STANDARD,
                            ..rules()
                        },
                        ..stompymux_rs::BattleMovementRules::STANDARD
                    },
                )
                .unwrap();
            }
            assert_elevation_inspection(&config, &world, id, Some(4));
        }
        stop_battle_unit(
            &mut world,
            id,
            ObjectId(1),
            stompymux_rs::BattleMovementRules::STANDARD.fall,
        )
        .unwrap();
        remove_battle_unit(&mut world, id, ObjectId(config.home())).unwrap();
        assert_elevation_inspection(&config, &world, id, None);
        assert!(battle_unit_elevation(&world, ObjectId(-1)).is_err());
    }
}

#[tokio::test]
async fn bridge_falls_choose_deck_or_lower_surface_at_the_two_level_boundary() {
    for (deck, initial, final_height) in [
        (0, -3, -1),
        (0, -2, 0),
        (3, 0, -1),
        (3, 1, 3),
        (3, 3, 3),
        (9, 6, -1),
        (9, 7, 9),
    ] {
        let (_dir, config, mut world, map, units) = fixture_surface(Terrain::Bridge, deck).await;
        let id = units[0];
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][id.0.to_string()]["ground_elevation"] = serde_json::json!(initial);
        world.btech = serde_json::from_value(state).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        let fall = resolve_battle_fall(&mut world, id, 1, rules()).unwrap();
        assert_eq!(
            resolve_battle_fall(&mut loaded, id, 1, rules()).unwrap(),
            fall
        );
        assert_eq!(world.btech, loaded.btech);
        assert_eq!(fall.damage, if final_height < 0 { 2 } else { 4 });
        assert!(fall.ice_break.is_none());
        assert!(fall.flooding.is_empty());
        assert_eq!(
            battle_unit_elevation(&world, id).unwrap(),
            Some(final_height)
        );
        assert_eq!(
            world.btech.maps()[&map].hex(1, 1).unwrap().terrain(),
            Terrain::Bridge
        );
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

#[tokio::test]
async fn bridge_deck_fire_and_standing_share_native_lua_transactions() {
    let (_dir, config, mut world, map, units) = fixture_surface(Terrain::Bridge, 3).await;
    let [id, target] = units;
    stop_battle_unit(
        &mut world,
        target,
        ObjectId(2),
        stompymux_rs::BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    place_battle_unit(&mut world, target, map, 1, 0).unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([0; 32])).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    refresh_battle_contacts(&mut world, &[id]).unwrap();
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.unit.fire({},1,0,{}); error('abort bridge shot')",
            id.0, target.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, world.btech);
    let text = support::run_text(
        &native,
        &config,
        ObjectId(1),
        1,
        &format!("fire 0 #{}", target.0),
    );
    lua.eval_callback::<()>(&format!("btech.unit.fire({},1,0,{})", id.0, target.0))
        .unwrap();
    assert!(
        native.world().btech.constructed_units()[&id]
            .weapon_readiness(0)
            .unwrap()
            .recycle_remaining
            > 0,
        "{text}"
    );
    assert_eq!(native.world().btech, lua.world().btech);
    let safe_seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() >= 7)
        .unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([safe_seed; 32])).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    let fall = resolve_battle_fall(&mut world, id, 1, rules()).unwrap();
    assert!(fall.avoidance.unwrap().success);
    for success in [true, false] {
        let seed = (0..=255)
            .find(|seed| (BattleDice::seeded([*seed; 32]).two_d6() >= 6) == success)
            .unwrap();
        let mut candidate = world.clone();
        let mut state = serde_json::to_value(&candidate.btech).unwrap();
        state["constructed"][id.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        candidate.btech = serde_json::from_value(state).unwrap();
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(candidate.clone())),
        )
        .unwrap();
        let lua = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(candidate.clone())),
        )
        .unwrap();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.stand({},1); error('abort bridge stand')",
                id.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, candidate.btech);
        support::run_text(&native, &config, ObjectId(1), 1, "stand");
        let actual: bool = lua
            .eval_callback(&format!(
                "return btech.unit.stand({},1).check.success",
                id.0
            ))
            .unwrap();
        assert_eq!(actual, success);
        assert_eq!(native.world().btech, lua.world().btech);
        assert_eq!(battle_unit_elevation(&native.world(), id).unwrap(), Some(3));
        let saved = native.world().clone();
        saved.validate(&config).unwrap();
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
    }
}

#[tokio::test]
async fn below_bridge_cooling_and_fresh_breaches_use_actual_depth() {
    let (_dir, config, mut world, _map, units) = fixture_surface(Terrain::Bridge, 3).await;
    let id = units[0];
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][id.0.to_string()]["ground_elevation"] = serde_json::json!(0);
    state["constructed"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([3; 32])).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    let fall = resolve_battle_fall(&mut world, id, 1, rules()).unwrap();
    assert_eq!(fall.damage, 2);
    assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(-1));
    assert_eq!(
        world.btech.constructed_units()[&id]
            .heat_rates(&world)
            .dissipation,
        16.0
    );
    let section = BattleSection::LeftArm;
    let armor = world.btech.constructed_units()[&id].sections()[&section].armor;
    if armor > 0 {
        let _damage = apply_damage_phase(
            &mut world,
            id,
            section,
            armor,
            BattleDamagePhase::Armor { rear: false },
        )
        .unwrap();
    }
    // Reference whole-unit flooding uses the bridge's positive surface elevation.
    assert!(
        flood_battle_unit(&mut world, id, rules())
            .unwrap()
            .is_empty()
    );
    let impact = resolve_battle_tactical_impact(
        &mut world,
        id,
        BattleHit {
            section,
            rear_armor: false,
            through_armor_critical: false,
            crew_stun: false,
        },
        1,
        rules(),
    )
    .unwrap();
    assert!(!impact.impact.destroyed);
    assert!(
        world.btech.constructed_units()[&id]
            .flooded_sections()
            .contains(&section)
    );
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

#[tokio::test]
async fn bridge_jump_entry_underpass_and_interrupted_hex_update_replay_after_restart() {
    for (deck, rate, outcome) in [(9, 100, "clear"), (6, 100, "entry"), (5, 160, "span")] {
        let source = format!("3 5\n.0.0.0\n.0.0.0\n/{deck}/{deck}/{deck}\n.0.0.0\n.0.0.0\n");
        let (_dir, config, mut world, map, units) =
            fixture_asset(BattleMapAsset::from_cells(&source).unwrap()).await;
        let id = units[0];
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["maps"][map.0.to_string()]["movement_modifier"] = serde_json::json!(rate);
        // Keep the later resumed controls independent of random pilot recovery.
        let safe_seed = (0..=255)
            .find(|seed| {
                let mut dice = BattleDice::seeded([*seed; 32]);
                let protected = dice.two_d6() >= 7;
                dice.d6();
                protected && dice.two_d6() == 7
            })
            .unwrap();
        state["constructed"][id.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([safe_seed; 32])).unwrap();
        world.btech = serde_json::from_value(state).unwrap();
        launch_battle_jump(&mut world, id, ObjectId(1), 180, 2.0).unwrap();
        for _ in 0..3 {
            advance_battle_jumps(
                &mut world,
                stompymux_rs::BattleMovementRules {
                    fall: stompymux_rs::BattleFallRules {
                        stacking: stompymux_rs::BattleStackingRules::STANDARD,
                        ..rules()
                    },
                    ..stompymux_rs::BattleMovementRules::STANDARD
                },
            )
            .unwrap();
        }
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        let mut notices = Vec::new();
        while world.btech.constructed_units()[&id].flight().is_some() {
            let expected = advance_battle_jumps(
                &mut world,
                stompymux_rs::BattleMovementRules {
                    fall: stompymux_rs::BattleFallRules {
                        stacking: stompymux_rs::BattleStackingRules::STANDARD,
                        ..rules()
                    },
                    ..stompymux_rs::BattleMovementRules::STANDARD
                },
            )
            .unwrap();
            assert_eq!(
                advance_battle_jumps(
                    &mut loaded,
                    stompymux_rs::BattleMovementRules {
                        fall: stompymux_rs::BattleFallRules {
                            stacking: stompymux_rs::BattleStackingRules::STANDARD,
                            ..rules()
                        },
                        ..stompymux_rs::BattleMovementRules::STANDARD
                    }
                )
                .unwrap(),
                expected
            );
            assert_eq!(world.btech, loaded.btech);
            notices.extend(expected);
        }
        let unit = &world.btech.constructed_units()[&id];
        match outcome {
            "clear" => {
                assert_eq!(unit.position().unwrap().y, 3);
                assert_eq!(unit.posture(), BattlePosture::Standing);
                assert!(!unit.hex_sync_pending());
            }
            "entry" => {
                assert_eq!(unit.position().unwrap().y, 1);
                assert_eq!(unit.posture(), BattlePosture::Prone);
                assert!(
                    notices
                        .iter()
                        .any(|notice| notice.text.contains("too high"))
                );
                assert!(!unit.hex_sync_pending());
            }
            _ => {
                assert_eq!(unit.position().unwrap().y, 2);
                assert_eq!(unit.motion().unwrap().point.containing_hex().unwrap().y, 3);
                assert!(unit.hex_sync_pending());
                assert!(
                    (battle_unit_range(&world, id, units[1]).unwrap().horizontal - 1.6).abs()
                        < 1e-9
                );
                assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(5));
                assert_eq!(unit.jump_stabilization(), 12);
                assert!(
                    notices
                        .iter()
                        .any(|notice| notice.text == "CRASH! You crash into the bridge!")
                );
                let mut invalid = serde_json::to_value(&world.btech).unwrap();
                invalid["constructed"][id.0.to_string()]["hex_sync_pending"] =
                    serde_json::json!(false);
                let mut bad = world.clone();
                bad.btech = serde_json::from_value(invalid).unwrap();
                assert!(bad.validate(&config).is_err());
            }
        }
        world.validate(&config).unwrap();
        if outcome == "span" {
            use sqlx::Connection;
            let stored = persistence::load(&config.database()).await.unwrap();
            let mut sql = sqlx::SqliteConnection::connect_with(
                &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
            )
            .await
            .unwrap();
            sqlx::query("CREATE TRIGGER reject_bridge_collision BEFORE UPDATE ON btech_units BEGIN SELECT RAISE(ABORT,'bridge collision save failure'); END").execute(&mut sql).await.unwrap();
            assert!(persistence::save(&config.database(), &world).await.is_err());
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                stored.btech
            );
            sqlx::query("DROP TRIGGER reject_bridge_collision")
                .execute(&mut sql)
                .await
                .unwrap();
        }
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restarted = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restarted.btech, world.btech);
        if outcome == "span" {
            let scripts = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(restarted.clone())),
            )
            .unwrap();
            let pending: bool = scripts
                .eval_callback(&format!(
                    "return btech.unit.state({}).hex_sync_pending",
                    id.0
                ))
                .unwrap();
            assert!(pending);
            let mut resumed = restarted.clone();
            set_battle_heading(&mut resumed, id, ObjectId(1), 90.0).unwrap();
            advance_battle_motion(
                &mut resumed,
                BattleMovementRules {
                    fasa_turning: false,
                    slowdown: 0,
                    ..stompymux_rs::BattleMovementRules::STANDARD
                },
            )
            .unwrap();
            assert!(resumed.btech.constructed_units()[&id].hex_sync_pending());
            assert_eq!(battle_unit_elevation(&resumed, id).unwrap(), Some(5));
            for _ in 0..12 {
                advance_battle_jumps(
                    &mut resumed,
                    stompymux_rs::BattleMovementRules {
                        fall: stompymux_rs::BattleFallRules {
                            stacking: stompymux_rs::BattleStackingRules::STANDARD,
                            ..rules()
                        },
                        ..stompymux_rs::BattleMovementRules::STANDARD
                    },
                )
                .unwrap();
            }
            let seed = (0..=255)
                .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() >= 6)
                .unwrap();
            let mut state = serde_json::to_value(&resumed.btech).unwrap();
            state["constructed"][id.0.to_string()]["dice"] =
                serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
            resumed.btech = serde_json::from_value(state).unwrap();
            let stand = begin_battle_stand(
                &mut resumed,
                id,
                ObjectId(1),
                BattleStandMode::Normal,
                true,
                rules(),
            )
            .unwrap();
            assert!(stand.check.success);
            while resumed.btech.constructed_units()[&id]
                .stand_timer()
                .is_some()
            {
                advance_battle_standing(&mut resumed);
            }
            launch_battle_jump(&mut resumed, id, ObjectId(1), 180, 1.0).unwrap();
            resumed.validate(&config).unwrap();
            advance_battle_jumps(
                &mut resumed,
                stompymux_rs::BattleMovementRules {
                    fall: stompymux_rs::BattleFallRules {
                        stacking: stompymux_rs::BattleStackingRules::STANDARD,
                        ..rules()
                    },
                    ..stompymux_rs::BattleMovementRules::STANDARD
                },
            )
            .unwrap();
            assert!(!resumed.btech.constructed_units()[&id].hex_sync_pending());
            resumed.validate(&config).unwrap();
            stop_battle_unit(
                &mut restarted,
                id,
                ObjectId(1),
                stompymux_rs::BattleMovementRules::STANDARD.fall,
            )
            .unwrap();
            place_battle_unit(&mut restarted, id, map, 1, 3).unwrap();
            assert!(!restarted.btech.constructed_units()[&id].hex_sync_pending());
            restarted.validate(&config).unwrap();
        }
    }
}

#[tokio::test]
async fn bridge_jump_vertical_collision_inside_the_starting_hex_uses_the_deck() {
    let (_dir, config, mut world, map, units) = fixture_surface(Terrain::Bridge, 3).await;
    let id = units[0];
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["maps"][map.0.to_string()]["movement_modifier"] = serde_json::json!(50);
    state["constructed"][id.0.to_string()]["ground_elevation"] = serde_json::json!(-1);
    world.btech = serde_json::from_value(state).unwrap();
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 1.0).unwrap();
    let mut notices = Vec::new();
    for _ in 0..3 {
        notices.extend(
            advance_battle_jumps(
                &mut world,
                stompymux_rs::BattleMovementRules {
                    fall: stompymux_rs::BattleFallRules {
                        stacking: stompymux_rs::BattleStackingRules::STANDARD,
                        ..rules()
                    },
                    ..stompymux_rs::BattleMovementRules::STANDARD
                },
            )
            .unwrap(),
        );
    }
    let unit = &world.btech.constructed_units()[&id];
    assert!(unit.flight().is_none());
    assert!(!unit.hex_sync_pending());
    assert_eq!(unit.position().unwrap().y, 1);
    assert_eq!(unit.posture(), BattlePosture::Prone);
    assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(3));
    assert!(notices.iter().any(|notice| notice.text.contains("CRASH!")));
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn lost_jump_thrust_beneath_bridge_preserves_altitude_for_the_fall() {
    let source = "3 3\n.0.0.0\n/9/9/9\n.0.0.0\n";
    let (_dir, config, mut world, _map, units) =
        fixture_asset(BattleMapAsset::from_cells(source).unwrap()).await;
    let id = units[0];
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][id.0.to_string()]["ground_elevation"] = serde_json::json!(-1);
    world.btech = serde_json::from_value(state).unwrap();
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 1.0).unwrap();
    let jets: Vec<_> = world.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .systems
        .into_iter()
        .filter(|part| part.system == BattleSystem::JumpJet)
        .map(|part| part.location)
        .collect();
    for jet in jets {
        destroy_battle_critical(&mut world, id, jet).unwrap();
    }
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        advance_battle_jumps(
            &mut world,
            stompymux_rs::BattleMovementRules {
                fall: stompymux_rs::BattleFallRules {
                    stacking: stompymux_rs::BattleStackingRules::STANDARD,
                    ..rules()
                },
                ..stompymux_rs::BattleMovementRules::STANDARD
            }
        )
        .unwrap(),
        advance_battle_jumps(
            &mut loaded,
            stompymux_rs::BattleMovementRules {
                fall: stompymux_rs::BattleFallRules {
                    stacking: stompymux_rs::BattleStackingRules::STANDARD,
                    ..rules()
                },
                ..stompymux_rs::BattleMovementRules::STANDARD
            }
        )
        .unwrap()
    );
    assert_eq!(world.btech, loaded.btech);
    let unit = &world.btech.constructed_units()[&id];
    assert!(unit.flight().is_none());
    assert_eq!(unit.posture(), BattlePosture::Prone);
    assert_eq!(unit.jump_stabilization(), 12);
    assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(-1));
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn bridge_deck_jumps_and_early_landings_share_native_lua_state() {
    for deck in [0, 3, 9] {
        for early in [false, true] {
            let (_dir, config, world, _map, units) = fixture_surface(Terrain::Bridge, deck).await;
            let id = units[0];
            let native = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
            )
            .unwrap();
            let lua = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
            )
            .unwrap();
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.jump({},1,0,1); error('abort bridge jump')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, world.btech);
            let text = support::run_text(&native, &config, ObjectId(1), 1, "jump 0 1");
            assert!(text.contains("You engage your jump jets."), "{text}");
            lua.eval_callback::<()>(&format!("btech.unit.jump({},1,0,1)", id.0))
                .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            let mut current = native.world().clone();
            for _ in 0..6 {
                advance_battle_jumps(
                    &mut current,
                    stompymux_rs::BattleMovementRules {
                        fall: stompymux_rs::BattleFallRules {
                            stacking: stompymux_rs::BattleStackingRules::STANDARD,
                            ..rules()
                        },
                        ..stompymux_rs::BattleMovementRules::STANDARD
                    },
                )
                .unwrap();
            }
            persistence::save(&config.database(), &current)
                .await
                .unwrap();
            let mut loaded = persistence::load(&config.database()).await.unwrap();
            if early {
                let seed = (0..=255)
                    .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() >= 6)
                    .unwrap();
                let mut state = serde_json::to_value(&current.btech).unwrap();
                state["constructed"][id.0.to_string()]["dice"] =
                    serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
                current.btech = serde_json::from_value(state).unwrap();
                loaded.btech = current.btech.clone();
                let native = Scripts::new(
                    &config,
                    std::rc::Rc::new(std::cell::RefCell::new(current.clone())),
                )
                .unwrap();
                let lua = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(loaded)))
                    .unwrap();
                assert!(
                    lua.eval_callback::<()>(&format!(
                        "btech.unit.land({},1); error('abort bridge landing')",
                        id.0
                    ))
                    .is_err()
                );
                assert_eq!(lua.world().btech, current.btech);
                support::run_text(&native, &config, ObjectId(1), 1, "land");
                lua.eval_callback::<()>(&format!("btech.unit.land({},1)", id.0))
                    .unwrap();
                assert_eq!(native.world().btech, lua.world().btech);
                current = native.world().clone();
            } else {
                for _ in 0..6 {
                    assert_eq!(
                        advance_battle_jumps(
                            &mut current,
                            stompymux_rs::BattleMovementRules {
                                fall: stompymux_rs::BattleFallRules {
                                    stacking: stompymux_rs::BattleStackingRules::STANDARD,
                                    ..rules()
                                },
                                ..stompymux_rs::BattleMovementRules::STANDARD
                            }
                        )
                        .unwrap(),
                        advance_battle_jumps(
                            &mut loaded,
                            stompymux_rs::BattleMovementRules {
                                fall: stompymux_rs::BattleFallRules {
                                    stacking: stompymux_rs::BattleStackingRules::STANDARD,
                                    ..rules()
                                },
                                ..stompymux_rs::BattleMovementRules::STANDARD
                            }
                        )
                        .unwrap()
                    );
                }
                assert_eq!(current.btech, loaded.btech);
            }
            let unit = &current.btech.constructed_units()[&id];
            assert!(unit.flight().is_none());
            assert!(!unit.hex_sync_pending());
            assert_eq!(unit.posture(), BattlePosture::Standing);
            assert_eq!(
                battle_unit_elevation(&current, id).unwrap(),
                Some(i32::from(deck))
            );
            assert_eq!(unit.jump_stabilization(), 12);
            current.validate(&config).unwrap();
        }
    }
}

#[tokio::test]
async fn level_bridge_deck_motion_matches_native_lua_and_replays_both_directions() {
    for deck in [0, 3, 9] {
        for reverse in [false, true] {
            let row = format!(".{deck}").repeat(3) + "\n";
            let source = format!(
                "3 5\n{row}{row}{}\n{row}{row}",
                format!("/{deck}").repeat(3)
            );
            let (_dir, config, mut world, _map, units) =
                fixture_asset(BattleMapAsset::from_cells(&source).unwrap()).await;
            let id = units[0];
            let heading = if reverse { 0.0 } else { 180.0 };
            let speed = if reverse { -21.5 } else { 21.5 };
            let mut state = serde_json::to_value(&world.btech).unwrap();
            state["constructed"][id.0.to_string()]["motion"]["heading"] =
                serde_json::json!(heading);
            state["constructed"][id.0.to_string()]["motion"]["desired_heading"] =
                serde_json::json!(heading);
            world.btech = serde_json::from_value(state).unwrap();
            let dice = serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"]
                .clone();
            let native = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
            )
            .unwrap();
            let lua = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
            )
            .unwrap();
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.speed({},1,{speed}); error('abort deck movement')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, world.btech);
            let text =
                support::run_text(&native, &config, ObjectId(1), 1, &format!("speed {speed}"));
            lua.eval_callback::<()>(&format!("btech.unit.speed({},1,{speed})", id.0))
                .unwrap();
            assert_eq!(native.world().btech, lua.world().btech, "{text}");
            let mut expected = native.world().clone();
            let motion_rules = BattleMovementRules {
                fasa_turning: false,
                slowdown: 0,
                ..stompymux_rs::BattleMovementRules::STANDARD
            };
            for _ in 0..30 {
                assert!(
                    advance_battle_motion(&mut expected, motion_rules)
                        .unwrap()
                        .is_empty()
                );
            }
            assert_eq!(
                expected.btech.constructed_units()[&id]
                    .position()
                    .unwrap()
                    .y,
                2
            );
            assert_eq!(battle_unit_elevation(&expected, id).unwrap(), Some(deck));
            persistence::save(&config.database(), &expected)
                .await
                .unwrap();
            let mut loaded = persistence::load(&config.database()).await.unwrap();
            for _ in 0..40 {
                assert_eq!(
                    advance_battle_motion(&mut expected, motion_rules).unwrap(),
                    advance_battle_motion(&mut loaded, motion_rules).unwrap()
                );
                assert_eq!(expected.btech, loaded.btech);
            }
            let unit = &loaded.btech.constructed_units()[&id];
            assert_eq!(unit.position().unwrap().y, 3);
            assert_eq!(unit.motion().unwrap().speed, speed);
            assert_eq!(battle_unit_elevation(&loaded, id).unwrap(), Some(deck));
            assert_eq!(serde_json::to_value(unit).unwrap()["dice"], dice);
            loaded.validate(&config).unwrap();
            persistence::save(&config.database(), &loaded)
                .await
                .unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                loaded.btech
            );
        }
    }
}

#[tokio::test]
async fn bridge_motion_does_not_replace_a_retained_lower_altitude_with_deck_height() {
    let (_dir, config, mut world, _map, units) = fixture_surface(Terrain::Bridge, 3).await;
    let id = units[0];
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][id.0.to_string()]["ground_elevation"] = serde_json::json!(-1);
    world.btech = serde_json::from_value(state).unwrap();
    let point = world.btech.constructed_units()[&id].motion().unwrap().point;
    set_battle_speed(&mut world, id, ObjectId(1), 21.5).unwrap();
    let notices = advance_battle_motion(
        &mut world,
        BattleMovementRules {
            fasa_turning: false,
            slowdown: 0,
            ..stompymux_rs::BattleMovementRules::STANDARD
        },
    )
    .unwrap();
    assert!(notices.is_empty());
    assert_ne!(
        world.btech.constructed_units()[&id].motion().unwrap().point,
        point
    );
    assert!(world.btech.constructed_units()[&id].motion().unwrap().speed > 0.0);
    assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(-1));
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn forward_ground_steps_charge_each_height_change_and_replay_mid_slope() {
    for descending in [false, true] {
        for symbol in ['.', '/', '@', '='] {
            let heights = if descending {
                [3, 3, 2, 0, 0]
            } else {
                [0, 0, 1, 3, 3]
            };
            let rows: String = heights
                .into_iter()
                .enumerate()
                .map(|(y, height)| {
                    format!("{}{height}", if y == 2 { symbol } else { '.' }).repeat(3) + "\n"
                })
                .collect();
            let (_dir, config, mut world, _map, units) =
                fixture_asset(BattleMapAsset::from_cells(&format!("3 5\n{rows}")).unwrap()).await;
            let id = units[0];
            let mut state = serde_json::to_value(&world.btech).unwrap();
            state["constructed"][id.0.to_string()]["motion"]["heading"] = serde_json::json!(180.0);
            state["constructed"][id.0.to_string()]["motion"]["desired_heading"] =
                serde_json::json!(180.0);
            world.btech = serde_json::from_value(state).unwrap();
            set_battle_speed(&mut world, id, ObjectId(1), 21.5).unwrap();
            let dice = serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"]
                .clone();
            let motion_rules = BattleMovementRules {
                fasa_turning: false,
                slowdown: 0,
                ..stompymux_rs::BattleMovementRules::STANDARD
            };
            let mut crossed_first = false;
            for _ in 0..30 {
                let before = world.btech.constructed_units()[&id].position().unwrap().y;
                assert!(
                    advance_battle_motion(&mut world, motion_rules)
                        .unwrap()
                        .is_empty()
                );
                let unit = &world.btech.constructed_units()[&id];
                if unit.position().unwrap().y != before {
                    assert_eq!(unit.position().unwrap().y, 2);
                    assert_eq!(unit.motion().unwrap().speed, 10.75);
                    assert_eq!(unit.motion().unwrap().desired_speed, 21.5);
                    crossed_first = true;
                }
            }
            assert!(crossed_first);
            persistence::save(&config.database(), &world).await.unwrap();
            let mut loaded = persistence::load(&config.database()).await.unwrap();
            let mut crossed_second = false;
            for _ in 0..60 {
                let before = world.btech.constructed_units()[&id].position().unwrap().y;
                assert_eq!(
                    advance_battle_motion(&mut world, motion_rules).unwrap(),
                    advance_battle_motion(&mut loaded, motion_rules).unwrap()
                );
                assert_eq!(world.btech, loaded.btech);
                let unit = &world.btech.constructed_units()[&id];
                if unit.position().unwrap().y != before {
                    assert_eq!(unit.position().unwrap().y, 3);
                    assert_eq!(unit.motion().unwrap().speed, 0.0);
                    assert_eq!(unit.motion().unwrap().desired_speed, 21.5);
                    crossed_second = true;
                    break;
                }
            }
            assert!(crossed_second);
            assert_eq!(
                battle_unit_elevation(&loaded, id).unwrap(),
                Some(heights[3])
            );
            assert_eq!(
                serde_json::to_value(&loaded.btech.constructed_units()[&id]).unwrap()["dice"],
                dice
            );
            loaded.validate(&config).unwrap();
            persistence::save(&config.database(), &loaded)
                .await
                .unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                loaded.btech
            );
        }
    }
}

#[tokio::test]
async fn pending_hex_sync_completes_an_allowed_step_without_retaining_the_old_height() {
    let source = "3 4\n/5/5/5\n/5/5/5\n.4.4.4\n.4.4.4\n";
    let (_dir, config, mut world, _map, units) =
        fixture_asset(BattleMapAsset::from_cells(source).unwrap()).await;
    let id = units[0];
    let mut state = serde_json::to_value(&world.btech).unwrap();
    let unit = &mut state["constructed"][id.0.to_string()];
    unit["ground_elevation"] = serde_json::json!(5);
    unit["hex_sync_pending"] = serde_json::json!(true);
    unit["motion"]["point"] =
        serde_json::to_value(BattleHexCoordinate { x: 1, y: 2 }.center()).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    world.validate(&config).unwrap();
    set_battle_heading(&mut world, id, ObjectId(1), 90.0).unwrap();
    assert!(
        advance_battle_motion(
            &mut world,
            BattleMovementRules {
                fasa_turning: false,
                slowdown: 0,
                ..stompymux_rs::BattleMovementRules::STANDARD
            }
        )
        .unwrap()
        .is_empty()
    );
    assert!(!world.btech.constructed_units()[&id].hex_sync_pending());
    assert_eq!(
        world.btech.constructed_units()[&id].position().unwrap().y,
        2
    );
    assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(4));
    world.validate(&config).unwrap();
}

/// Place a unit just before a reverse crossing and choose an exact initial control roll.
fn prepare_reverse_step(world: &mut World, id: ObjectId, roll: u8) {
    let seed = (0..=u8::MAX)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == roll)
        .unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    let unit = &mut state["constructed"][id.0.to_string()];
    unit["motion"]["point"] = serde_json::to_value(BattlePoint {
        y: 1.49,
        ..BattleHexCoordinate { x: 1, y: 1 }.center()
    })
    .unwrap();
    unit["motion"]["speed"] = serde_json::json!(-21.5);
    unit["motion"]["desired_speed"] = serde_json::json!(-21.5);
    unit["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
}

/// One shard per terrain symbol; each elevation change owns one fixture that the
/// rule/success combinations clone, keeping the lockstep replay twin per combination.
async fn reverse_ground_steps_matrix(symbol: char) {
    for change in [-2i16, -1, 1, 2] {
        let destination = 3 + change;
        let row = format!("{symbol}{destination}").repeat(3);
        let source = format!("3 4\n.3.3.3\n.3.3.3\n{row}\n{row}\n");
        let (_dir, config, base, _, units) =
            fixture_asset(BattleMapAsset::from_cells(&source).unwrap()).await;
        let id = units[0];
        let mut probed_fidelity = [false; 3];
        for (shape, (enabled, success)) in [(false, true), (true, true), (true, false)]
            .into_iter()
            .enumerate()
        {
            let mut world = base.clone();
            let target = 6 + change.unsigned_abs() as u8 - 1;
            prepare_reverse_step(&mut world, id, if success { target } else { target - 1 });
            world.validate(&config).unwrap();
            let before = world.clone();
            persistence::save(&config.database(), &world).await.unwrap();
            let mut loaded = persistence::load(&config.database()).await.unwrap();
            let movement = BattleMovementRules {
                roll_on_backwalk: enabled,
                ..BattleMovementRules::STANDARD
            };
            let notices = advance_battle_motion(&mut world, movement).unwrap();
            assert_eq!(
                notices,
                advance_battle_motion(&mut loaded, movement).unwrap()
            );
            assert_eq!(world.btech, loaded.btech);
            let unit = &world.btech.constructed_units()[&id];
            if !enabled || success {
                assert_eq!(unit.position().unwrap().y, 2);
                assert_eq!(unit.posture(), BattlePosture::Standing);
                assert_eq!(unit.motion().unwrap().desired_speed, -21.5);
                assert_eq!(
                    unit.motion().unwrap().speed,
                    if enabled {
                        -21.5
                    } else {
                        -(21.5 - f64::from(change.unsigned_abs()) * 10.75)
                    }
                );
                if enabled {
                    assert!(
                        notices
                            .iter()
                            .any(|notice| notice.text == "You manage to overcome the obstacle.")
                    );
                    let mut expected = before.clone();
                    let check =
                        roll_battle_piloting(&mut expected, id, change.abs() - 1, true).unwrap();
                    assert_eq!(check.target, i32::from(target));
                    assert!(check.success);
                    assert_eq!(
                        serde_json::to_value(unit).unwrap()["dice"],
                        serde_json::to_value(&expected.btech.constructed_units()[&id]).unwrap()["dice"]
                    );
                } else {
                    assert!(notices.is_empty());
                    assert_eq!(
                        serde_json::to_value(unit).unwrap()["dice"],
                        serde_json::to_value(&before.btech.constructed_units()[&id]).unwrap()["dice"]
                    );
                }
            } else {
                assert_eq!(unit.posture(), BattlePosture::Prone);
                let protection = |unit: &BattleUnit| {
                    unit.sections()
                        .values()
                        .map(|section| {
                            u32::from(section.armor)
                                + u32::from(section.rear)
                                + u32::from(section.internal)
                        })
                        .sum::<u32>()
                };
                assert!(
                    protection(&before.btech.constructed_units()[&id]) - protection(unit)
                        >= u32::from(change.unsigned_abs()) * 4
                );
                assert_eq!(unit.motion().unwrap().speed, 0.0);
                assert_eq!(unit.motion().unwrap().desired_speed, 0.0);
                assert_eq!(unit.position().unwrap().y, if change > 0 { 1 } else { 2 });
                assert_eq!(
                    battle_unit_elevation(&world, id).unwrap(),
                    Some(if change > 0 {
                        3
                    } else {
                        i32::from(destination)
                    })
                );
                if change > 0 {
                    assert_eq!(
                        unit.motion().unwrap().point,
                        before.btech.constructed_units()[&id]
                            .motion()
                            .unwrap()
                            .point
                    );
                }
            }
            world.validate(&config).unwrap();
            // Final-state fidelity probe once per rule shape; the lockstep replay
            // above stays per combination.
            if !probed_fidelity[shape] {
                probed_fidelity[shape] = true;
                persistence::save(&config.database(), &world).await.unwrap();
                assert_eq!(
                    persistence::load(&config.database()).await.unwrap().btech,
                    world.btech
                );
            }
        }
    }
}

#[tokio::test]
async fn reverse_ground_steps_apply_configured_checks_falls_and_restart_replay_dot() {
    reverse_ground_steps_matrix('.').await;
}

#[tokio::test]
async fn reverse_ground_steps_apply_configured_checks_falls_and_restart_replay_slash() {
    reverse_ground_steps_matrix('/').await;
}

#[tokio::test]
async fn reverse_ground_steps_apply_configured_checks_falls_and_restart_replay_at() {
    reverse_ground_steps_matrix('@').await;
}

#[tokio::test]
async fn reverse_ground_steps_apply_configured_checks_falls_and_restart_replay_equals() {
    reverse_ground_steps_matrix('=').await;
}

#[tokio::test]
async fn failed_reverse_fall_restores_the_entire_movement_tick() {
    let source = "3 4\n.0.0.0\n.0.0.0\n.1.1.1\n.1.1.1\n";
    let (_dir, config, mut world, _, units) =
        fixture_asset(BattleMapAsset::from_cells(source).unwrap()).await;
    for id in units {
        prepare_reverse_step(&mut world, id, 2);
    }
    world
        .objects
        .get_mut(&units[1])
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let before = world.clone();
    assert!(
        advance_battle_motion(&mut world, BattleMovementRules::STANDARD)
            .unwrap_err()
            .to_string()
            .contains("tactical")
    );
    assert_eq!(world.btech, before.btech);
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn unpiloted_reverse_step_bypasses_control_dice_and_preserves_speed() {
    let source = "3 4\n.0.0.0\n.0.0.0\n.2.2.2\n.2.2.2\n";
    let (_dir, config, mut world, _, units) =
        fixture_asset(BattleMapAsset::from_cells(source).unwrap()).await;
    let id = units[0];
    prepare_reverse_step(&mut world, id, 2);
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][id.0.to_string()]["pilot"] = serde_json::Value::Null;
    world.btech = serde_json::from_value(state).unwrap();
    let dice = serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"].clone();
    let notices = advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
    let unit = &world.btech.constructed_units()[&id];
    assert_eq!(unit.position().unwrap().y, 2);
    assert_eq!(unit.motion().unwrap().speed, -21.5);
    assert_eq!(serde_json::to_value(unit).unwrap()["dice"], dice);
    assert!(
        notices
            .iter()
            .any(|notice| notice.text == "You manage to overcome the obstacle.")
    );
    world.validate(&config).unwrap();
}

/// One shard per terrain symbol and slope direction owns a single fixture; the
/// skid/speed/success combinations clone it and keep the lockstep replay twin.
async fn cliffs_apply_speed_checks_matrix(symbol: char, downhill: bool) {
    let (old, new) = if downhill { (3, 0) } else { (0, 3) };
    let row = format!("{symbol}{new}").repeat(3);
    let old_row = format!(".{old}").repeat(3);
    let source = format!("3 4\n{old_row}\n{old_row}\n{row}\n{row}\n");
    let (_dir, config, base, _, units) =
        fixture_asset(BattleMapAsset::from_cells(&source).unwrap()).await;
    let id = units[0];
    let mut probed_fidelity = [false; 2];
    for skid in [false, true] {
        for speed in [-53.75f64, -21.5, 21.5, 53.75] {
            for success in [false, true] {
                let mut world = base.clone();
                let modifier = if skid {
                    if speed.abs() < 30.0 { -1 } else { 1 }
                } else {
                    ((speed + 10.75).abs() / 10.75) as i16 / 3
                };
                let target = 6i16 + modifier;
                prepare_reverse_step(&mut world, id, (target - i16::from(!success)) as u8);
                let mut state = serde_json::to_value(&world.btech).unwrap();
                let motion = &mut state["constructed"][id.0.to_string()]["motion"];
                motion["speed"] = serde_json::json!(speed);
                motion["desired_speed"] = serde_json::json!(speed);
                motion["heading"] = serde_json::json!(if speed > 0.0 { 180.0 } else { 0.0 });
                motion["desired_heading"] = motion["heading"].clone();
                world.btech = serde_json::from_value(state).unwrap();
                world.validate(&config).unwrap();
                let before = world.clone();
                persistence::save(&config.database(), &world).await.unwrap();
                let mut loaded = persistence::load(&config.database()).await.unwrap();
                let movement = BattleMovementRules {
                    skid_cliff: skid,
                    ..BattleMovementRules::STANDARD
                };
                let notices = advance_battle_motion(&mut world, movement).unwrap();
                assert_eq!(
                    notices,
                    advance_battle_motion(&mut loaded, movement).unwrap()
                );
                assert_eq!(world.btech, loaded.btech);
                let unit = &world.btech.constructed_units()[&id];
                assert_eq!(unit.motion().unwrap().speed, 0.0);
                assert_eq!(unit.motion().unwrap().desired_speed, 0.0);
                assert_eq!(
                    unit.posture(),
                    if success {
                        BattlePosture::Standing
                    } else {
                        BattlePosture::Prone
                    }
                );
                let stayed = success || !downhill;
                assert_eq!(unit.position().unwrap().y, if stayed { 1 } else { 2 });
                assert_eq!(
                    battle_unit_elevation(&world, id).unwrap(),
                    Some(if stayed { old } else { new })
                );
                if stayed {
                    assert_eq!(
                        unit.motion().unwrap().point,
                        before.btech.constructed_units()[&id]
                            .motion()
                            .unwrap()
                            .point
                    );
                }
                if success {
                    assert!(
                        notices
                            .iter()
                            .any(|notice| { notice.text.starts_with("You manage to stop before") })
                    );
                    let mut expected = before.clone();
                    assert!(
                        roll_battle_piloting(&mut expected, id, modifier, true)
                            .unwrap()
                            .success
                    );
                    assert_eq!(
                        serde_json::to_value(unit).unwrap()["dice"],
                        serde_json::to_value(&expected.btech.constructed_units()[&id]).unwrap()["dice"]
                    );
                } else {
                    let protection = |unit: &BattleUnit| {
                        unit.sections()
                            .values()
                            .map(|section| {
                                u32::from(section.armor)
                                    + u32::from(section.internal)
                                    + u32::from(section.rear)
                            })
                            .sum::<u32>()
                    };
                    let lost =
                        protection(&before.btech.constructed_units()[&id]) - protection(unit);
                    let levels = if downhill {
                        3
                    } else if skid {
                        1
                    } else {
                        ((1.0 + speed / 10.75) as i16 / 4).max(0)
                    };
                    assert_eq!(lost, levels as u32 * 4);
                }
                world.validate(&config).unwrap();
                // Final-state fidelity probe once per outcome shape; the lockstep
                // replay above stays per combination.
                let shape = usize::from(success);
                if !probed_fidelity[shape] {
                    probed_fidelity[shape] = true;
                    persistence::save(&config.database(), &world).await.unwrap();
                    assert_eq!(
                        persistence::load(&config.database()).await.unwrap().btech,
                        world.btech
                    );
                }
            }
        }
    }
}

#[tokio::test]
async fn cliffs_apply_speed_checks_stop_or_fall_and_replay_after_restart_dot_down() {
    cliffs_apply_speed_checks_matrix('.', true).await;
}

#[tokio::test]
async fn cliffs_apply_speed_checks_stop_or_fall_and_replay_after_restart_dot_up() {
    cliffs_apply_speed_checks_matrix('.', false).await;
}

#[tokio::test]
async fn cliffs_apply_speed_checks_stop_or_fall_and_replay_after_restart_slash_down() {
    cliffs_apply_speed_checks_matrix('/', true).await;
}

#[tokio::test]
async fn cliffs_apply_speed_checks_stop_or_fall_and_replay_after_restart_slash_up() {
    cliffs_apply_speed_checks_matrix('/', false).await;
}

#[tokio::test]
async fn cliffs_apply_speed_checks_stop_or_fall_and_replay_after_restart_at_down() {
    cliffs_apply_speed_checks_matrix('@', true).await;
}

#[tokio::test]
async fn cliffs_apply_speed_checks_stop_or_fall_and_replay_after_restart_at_up() {
    cliffs_apply_speed_checks_matrix('@', false).await;
}

#[tokio::test]
async fn cliffs_apply_speed_checks_stop_or_fall_and_replay_after_restart_equals_down() {
    cliffs_apply_speed_checks_matrix('=', true).await;
}

#[tokio::test]
async fn cliffs_apply_speed_checks_stop_or_fall_and_replay_after_restart_equals_up() {
    cliffs_apply_speed_checks_matrix('=', false).await;
}

#[tokio::test]
async fn first_cliff_stops_unpiloted_fast_motion_without_consuming_dice() {
    for downhill in [false, true] {
        let (old, new) = if downhill { (9, 0) } else { (0, 9) };
        let old_row = format!(".{old}").repeat(3);
        let row = format!(".{new}").repeat(3);
        let source = format!("3 4\n{old_row}\n{old_row}\n{row}\n{row}\n");
        let (_dir, config, mut world, map, units) =
            fixture_asset(BattleMapAsset::from_cells(&source).unwrap()).await;
        let id = units[0];
        prepare_reverse_step(&mut world, id, 2);
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["maps"][map.0.to_string()]["movement_modifier"] = serde_json::json!(10000);
        state["constructed"][id.0.to_string()]["pilot"] = serde_json::Value::Null;
        world.btech = serde_json::from_value(state).unwrap();
        let before = world.clone();
        let notices = advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
        let unit = &world.btech.constructed_units()[&id];
        assert_eq!(
            unit.position(),
            before.btech.constructed_units()[&id].position()
        );
        assert_eq!(
            unit.motion().unwrap().point,
            before.btech.constructed_units()[&id]
                .motion()
                .unwrap()
                .point
        );
        assert_eq!(unit.motion().unwrap().speed, 0.0);
        assert_eq!(
            serde_json::to_value(unit).unwrap()["dice"],
            serde_json::to_value(&before.btech.constructed_units()[&id]).unwrap()["dice"]
        );
        assert!(
            notices
                .iter()
                .any(|notice| notice.text.starts_with("You manage to stop before"))
        );
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn autofall_native_lua_controls_are_atomic_and_survive_restart_and_shutdown() {
    let (_dir, config, world, _, units) = fixture_surface(Terrain::Grassland, 0).await;
    let id = units[0];
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert!(
        support::run_text(&native, &config, ObjectId(1), 1, "mechprefs").contains("AutoFall: OFF")
    );
    assert!(
        support::run_text(&native, &config, ObjectId(1), 1, "mechprefs aUtOfAlL on")
            .contains("toggled ON")
    );
    lua.eval_callback::<()>(&format!(
        "assert(btech.unit.auto_fall({}, 1, true)); assert(btech.unit.state({}).auto_fall)",
        id.0, id.0
    ))
    .unwrap();
    assert_eq!(native.world().btech, lua.world().btech);
    let before = lua.world().btech.clone();
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.unit.auto_fall({}, 1, false); error('undo preference')",
            id.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, before);
    assert!(
        lua.eval_callback::<()>(&format!("btech.unit.auto_fall({}, 2, false)", id.0))
            .is_err()
    );
    assert_eq!(lua.world().btech, before);
    for command in [
        "mechprefs AutoFall maybe",
        "mechprefs AutoFall on extra",
        "mechprefs Unknown on",
        "mechprefs/bad AutoFall off",
    ] {
        support::run_text(&native, &config, ObjectId(1), 1, command);
        assert_eq!(native.world().btech, before);
    }
    lua.eval_callback::<()>(&format!(
        "local state = btech.unit.state({}); state.auto_fall = false",
        id.0
    ))
    .unwrap();
    assert_eq!(lua.world().btech, before);
    let mut saved = native.world().clone();
    stop_battle_unit(
        &mut saved,
        id,
        ObjectId(1),
        stompymux_rs::BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    assert!(saved.btech.constructed_units()[&id].auto_fall());
    assign_battle_pilot(&mut saved, id, ObjectId(1)).unwrap();
    set_battle_auto_fall(&mut saved, id, ObjectId(1), false).unwrap();
    assert!(!saved.btech.constructed_units()[&id].auto_fall());
    set_battle_auto_fall(&mut saved, id, ObjectId(1), true).unwrap();
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
    assert!(
        support::run_text(&native, &config, ObjectId(1), 1, "mechprefs AutoFall")
            .contains("toggled OFF")
    );
    assert!(!native.world().btech.constructed_units()[&id].auto_fall());
    saved.validate(&config).unwrap();
}

#[tokio::test]
async fn autofall_skips_only_the_piloted_downhill_avoidance_roll() {
    for downhill in [false, true] {
        for piloted in [false, true] {
            let (old, new) = if downhill { (3, 0) } else { (0, 3) };
            let old_row = format!(".{old}").repeat(3);
            let row = format!(".{new}").repeat(3);
            let source = format!("3 4\n{old_row}\n{old_row}\n{row}\n{row}\n");
            let (_dir, config, mut world, _, units) =
                fixture_asset(BattleMapAsset::from_cells(&source).unwrap()).await;
            let id = units[0];
            prepare_reverse_step(&mut world, id, 12);
            set_battle_auto_fall(&mut world, id, ObjectId(1), true).unwrap();
            if !piloted {
                let mut state = serde_json::to_value(&world.btech).unwrap();
                state["constructed"][id.0.to_string()]["pilot"] = serde_json::Value::Null;
                world.btech = serde_json::from_value(state).unwrap();
            }
            persistence::save(&config.database(), &world).await.unwrap();
            let mut loaded = persistence::load(&config.database()).await.unwrap();
            let mut expected = world.clone();
            if downhill && piloted {
                let mut state = serde_json::to_value(&expected.btech).unwrap();
                let unit = &mut state["constructed"][id.0.to_string()];
                unit["position"]["y"] = serde_json::json!(2);
                let point = world.btech.constructed_units()[&id]
                    .motion()
                    .unwrap()
                    .point
                    .project(180.0, 21.5 / 645.0)
                    .unwrap();
                unit["motion"]["point"] = serde_json::to_value(point).unwrap();
                expected.btech = serde_json::from_value(state).unwrap();
                let report = resolve_battle_fall(&mut expected, id, 3, rules()).unwrap();
                assert_eq!(report.avoidance.unwrap().roll, Some(12));
            } else {
                let mut state = serde_json::to_value(&expected.btech).unwrap();
                state["constructed"][id.0.to_string()]["auto_fall"] = serde_json::json!(false);
                expected.btech = serde_json::from_value(state).unwrap();
                advance_battle_motion(&mut expected, BattleMovementRules::STANDARD).unwrap();
                let mut state = serde_json::to_value(&expected.btech).unwrap();
                state["constructed"][id.0.to_string()]["auto_fall"] = serde_json::json!(true);
                expected.btech = serde_json::from_value(state).unwrap();
            }
            let notices = advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
            assert_eq!(
                notices,
                advance_battle_motion(&mut loaded, BattleMovementRules::STANDARD).unwrap()
            );
            assert_eq!(world.btech, loaded.btech);
            assert_eq!(world.btech, expected.btech);
            world.validate(&config).unwrap();
        }
    }
}

#[tokio::test]
async fn water_hex_entry_uses_depth_checks_and_replays_success_and_falls() {
    for depth in [0u8, 1, 2, 3, 4, 9] {
        for success in [false, true] {
            let (_dir, config, mut world, _, units) = fixture_field(Terrain::Water, depth, 5).await;
            let id = units[0];
            let modifier = if depth > 3 { 1 } else { i16::from(depth) - 2 };
            prepare_reverse_step(&mut world, id, (6 + modifier - i16::from(!success)) as u8);
            let before = world.clone();
            persistence::save(&config.database(), &world).await.unwrap();
            let mut loaded = persistence::load(&config.database()).await.unwrap();
            let notices = advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
            assert_eq!(
                notices,
                advance_battle_motion(&mut loaded, BattleMovementRules::STANDARD).unwrap()
            );
            assert_eq!(world.btech, loaded.btech);
            let unit = &world.btech.constructed_units()[&id];
            assert_eq!(unit.position().unwrap().y, 2);
            assert_eq!(
                battle_unit_elevation(&world, id).unwrap(),
                Some(-i32::from(depth))
            );
            assert_eq!(
                unit.posture(),
                if success || depth == 0 {
                    BattlePosture::Standing
                } else {
                    BattlePosture::Prone
                }
            );
            if depth == 0 {
                assert!(notices.is_empty());
                assert_eq!(
                    serde_json::to_value(unit).unwrap()["dice"],
                    serde_json::to_value(&before.btech.constructed_units()[&id]).unwrap()["dice"]
                );
            } else if success {
                let mut expected = before.clone();
                let check = roll_battle_piloting(&mut expected, id, modifier, true).unwrap();
                assert!(check.success);
                assert_eq!(
                    serde_json::to_value(unit).unwrap()["dice"],
                    serde_json::to_value(&expected.btech.constructed_units()[&id]).unwrap()["dice"]
                );
                assert_eq!(unit.motion().unwrap().desired_speed, -21.5);
                // No second entry means no second control roll.
                let dice = serde_json::to_value(unit).unwrap()["dice"].clone();
                assert!(
                    advance_battle_motion(&mut world, BattleMovementRules::STANDARD)
                        .unwrap()
                        .is_empty()
                );
                assert_eq!(
                    serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"],
                    dice
                );
            } else {
                assert!(
                    notices
                        .iter()
                        .any(|notice| notice.text == "You slip in the water and fall down")
                );
                assert_eq!(unit.motion().unwrap().desired_speed, 0.0);
            }
            world.validate(&config).unwrap();
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
        }
    }
}

#[tokio::test]
async fn running_into_water_caps_throttle_and_adds_two_to_the_control_check() {
    for depth in [1, 2] {
        let row = format!("~{depth}").repeat(3);
        let source = format!("3 4\n.0.0.0\n.0.0.0\n{row}\n{row}\n");
        let (_dir, config, mut world, _, units) =
            fixture_asset(BattleMapAsset::from_cells(&source).unwrap()).await;
        let id = units[0];
        prepare_reverse_step(&mut world, id, 6 + depth);
        let maximum = world.btech.constructed_units()[&id]
            .mobility()
            .maximum_speed;
        let mut state = serde_json::to_value(&world.btech).unwrap();
        let motion = &mut state["constructed"][id.0.to_string()]["motion"];
        motion["heading"] = serde_json::json!(180.0);
        motion["desired_heading"] = serde_json::json!(180.0);
        motion["speed"] = serde_json::json!(maximum);
        motion["desired_speed"] = serde_json::json!(maximum);
        world.btech = serde_json::from_value(state).unwrap();
        let before = world.clone();
        let notices = advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
        assert!(
            notices
                .iter()
                .any(|notice| notice.text.contains("run into the water"))
        );
        let unit = &world.btech.constructed_units()[&id];
        assert_eq!(unit.posture(), BattlePosture::Standing);
        assert_eq!(
            unit.motion().unwrap().speed,
            maximum - f64::from(depth) * 10.75
        );
        assert_eq!(unit.motion().unwrap().desired_speed, maximum * 2.0 / 3.0);
        let mut expected = before;
        assert!(
            roll_battle_piloting(&mut expected, id, i16::from(depth), true)
                .unwrap()
                .success
        );
        assert_eq!(
            serde_json::to_value(unit).unwrap()["dice"],
            serde_json::to_value(&expected.btech.constructed_units()[&id]).unwrap()["dice"]
        );
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn entering_water_floods_a_breached_leg_and_keeps_the_fall_stopped() {
    let source = "3 4\n.0.0.0\n.0.0.0\n~1~1~1\n~1~1~1\n";
    let (_dir, config, mut world, _, units) =
        fixture_asset(BattleMapAsset::from_cells(source).unwrap()).await;
    let id = units[0];
    let leg = BattleSection::LeftLeg;
    let armor = world.btech.constructed_units()[&id].sections()[&leg].armor;
    let _damage = apply_damage_phase(
        &mut world,
        id,
        leg,
        armor,
        BattleDamagePhase::Armor { rear: false },
    )
    .unwrap();
    prepare_reverse_step(&mut world, id, 12);
    let notices = advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
    let unit = &world.btech.constructed_units()[&id];
    assert!(unit.flooded_sections().contains(&leg));
    assert_eq!(unit.posture(), BattlePosture::Prone);
    assert_eq!(unit.motion().unwrap().speed, 0.0);
    assert_eq!(unit.motion().unwrap().desired_speed, 0.0);
    assert_eq!(
        notices
            .iter()
            .filter(|notice| notice.text.contains("Water floods into your Left Leg"))
            .count(),
        1
    );
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn water_map_depth_preserves_requested_speed_before_a_hex_entry() {
    for depth in [0, 1, 2, 9] {
        for speed in [-21.5, 21.5] {
            let (_dir, config, mut world, _, units) = fixture_field(Terrain::Water, depth, 5).await;
            let id = units[0];
            set_battle_speed(&mut world, id, ObjectId(1), speed).unwrap();
            let dice = serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"]
                .clone();
            for _ in 0..4 {
                assert!(
                    advance_battle_motion(&mut world, BattleMovementRules::STANDARD)
                        .unwrap()
                        .is_empty()
                );
            }
            let unit = &world.btech.constructed_units()[&id];
            assert_eq!(unit.motion().unwrap().speed, speed);
            assert_eq!(unit.motion().unwrap().desired_speed, speed);
            assert_eq!(serde_json::to_value(unit).unwrap()["dice"], dice);
            world.validate(&config).unwrap();
        }
    }
}

#[tokio::test]
async fn invalid_water_entry_restores_position_throttle_and_dice() {
    let (_dir, config, mut world, _, units) = fixture_field(Terrain::Water, 1, 5).await;
    let id = units[0];
    prepare_reverse_step(&mut world, id, 12);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let before = world.clone();
    assert!(advance_battle_motion(&mut world, BattleMovementRules::STANDARD).is_err());
    assert_eq!(world.btech, before.btech);
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn leaving_shallow_water_restores_land_height_and_charges_the_upward_step() {
    let source = "3 4\n~1~1~1\n~1~1~1\n.0.0.0\n.0.0.0\n";
    let (_dir, config, mut world, _, units) =
        fixture_asset(BattleMapAsset::from_cells(source).unwrap()).await;
    let id = units[0];
    prepare_reverse_step(&mut world, id, 12);
    let mut state = serde_json::to_value(&world.btech).unwrap();
    let motion = &mut state["constructed"][id.0.to_string()]["motion"];
    motion["heading"] = serde_json::json!(180.0);
    motion["desired_heading"] = serde_json::json!(180.0);
    motion["speed"] = serde_json::json!(21.5);
    motion["desired_speed"] = serde_json::json!(21.5);
    world.btech = serde_json::from_value(state).unwrap();
    let before = world.clone();
    assert!(
        advance_battle_motion(&mut world, BattleMovementRules::STANDARD)
            .unwrap()
            .is_empty()
    );
    let unit = &world.btech.constructed_units()[&id];
    assert_eq!(unit.position().unwrap().y, 2);
    assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(0));
    assert_eq!(unit.motion().unwrap().desired_speed, 21.5);
    assert!((unit.motion().unwrap().speed - (21.5 - 10.75)).abs() < 1e-10);
    assert_eq!(
        serde_json::to_value(unit).unwrap()["dice"],
        serde_json::to_value(&before.btech.constructed_units()[&id]).unwrap()["dice"]
    );
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn bridge_ground_routes_select_lower_or_deck_surface_and_replay() {
    for reverse in [false, true] {
        for (old_deck, old_height, terrain, new_deck, height) in [
            (3, -1, Terrain::Bridge, 3, -1),
            (3, -1, Terrain::Bridge, 4, -1),
            (3, -1, Terrain::Bridge, 1, 1),
            (0, -1, Terrain::Bridge, 1, 1),
            (3, 3, Terrain::Bridge, 4, 4),
            (1, -1, Terrain::Water, 1, -1),
        ] {
            let old_row = format!("/{old_deck}").repeat(3);
            let row = format!("{}{new_deck}", terrain.symbol()).repeat(3);
            let source = format!("3 4\n{old_row}\n{old_row}\n{row}\n{row}\n");
            let (_dir, config, mut world, _, units) =
                fixture_asset(BattleMapAsset::from_cells(&source).unwrap()).await;
            let id = units[0];
            prepare_reverse_step(&mut world, id, 12);
            let mut state = serde_json::to_value(&world.btech).unwrap();
            let unit = &mut state["constructed"][id.0.to_string()];
            unit["ground_elevation"] = if old_height == old_deck {
                serde_json::Value::Null
            } else {
                serde_json::json!(old_height)
            };
            if !reverse {
                unit["motion"]["heading"] = serde_json::json!(180.0);
                unit["motion"]["desired_heading"] = serde_json::json!(180.0);
                unit["motion"]["speed"] = serde_json::json!(21.5);
                unit["motion"]["desired_speed"] = serde_json::json!(21.5);
            }
            world.btech = serde_json::from_value(state).unwrap();
            let before = world.clone();
            persistence::save(&config.database(), &world).await.unwrap();
            let mut loaded = persistence::load(&config.database()).await.unwrap();
            let notices = advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
            assert_eq!(
                notices,
                advance_battle_motion(&mut loaded, BattleMovementRules::STANDARD).unwrap()
            );
            assert_eq!(world.btech, loaded.btech);
            let unit = &world.btech.constructed_units()[&id];
            assert_eq!(unit.position().unwrap().y, 2);
            assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(height));
            assert_eq!(unit.posture(), BattlePosture::Standing);
            let map_height = if terrain == Terrain::Water {
                -new_deck
            } else {
                new_deck
            };
            let change: i32 = map_height - old_deck;
            let reverse_check = reverse && change != 0;
            let water_check = height < 0 && !reverse_check;
            let mut expected = before.clone();
            if reverse_check || water_check {
                let modifier = if reverse_check {
                    change.abs() as i16 - 1
                } else {
                    -1
                };
                assert!(
                    roll_battle_piloting(&mut expected, id, modifier, true)
                        .unwrap()
                        .success
                );
            }
            assert_eq!(
                serde_json::to_value(unit).unwrap()["dice"],
                serde_json::to_value(&expected.btech.constructed_units()[&id]).unwrap()["dice"]
            );
            let cost = if !reverse_check && height == map_height {
                f64::from(change.abs()) * 10.75
            } else {
                0.0
            };
            assert_eq!(unit.motion().unwrap().speed.abs(), (21.5 - cost).max(0.0));
            world.validate(&config).unwrap();
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
        }
    }
}

#[tokio::test]
async fn below_bridge_water_check_failure_falls_on_the_lower_surface() {
    let (_dir, config, mut world, _, units) = fixture_field(Terrain::Bridge, 9, 5).await;
    let id = units[0];
    prepare_reverse_step(&mut world, id, 4);
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][id.0.to_string()]["ground_elevation"] = serde_json::json!(-1);
    world.btech = serde_json::from_value(state).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    let notices = advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
    assert_eq!(
        notices,
        advance_battle_motion(&mut loaded, BattleMovementRules::STANDARD).unwrap()
    );
    assert_eq!(world.btech, loaded.btech);
    assert!(
        notices
            .iter()
            .any(|notice| notice.text == "You slip in the water and fall down")
    );
    let unit = &world.btech.constructed_units()[&id];
    assert_eq!(unit.position().unwrap().y, 2);
    assert_eq!(unit.posture(), BattlePosture::Prone);
    assert_eq!(unit.motion().unwrap().speed, 0.0);
    assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(-1));
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn exiting_below_bridge_retains_mapped_cliff_checks_and_lower_rollback() {
    for success in [false, true] {
        let source = "3 4\n/3/3/3\n/3/3/3\n~1~1~1\n~1~1~1\n";
        let (_dir, config, mut world, _, units) =
            fixture_asset(BattleMapAsset::from_cells(source).unwrap()).await;
        let id = units[0];
        prepare_reverse_step(&mut world, id, if success { 6 } else { 5 });
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][id.0.to_string()]["ground_elevation"] = serde_json::json!(-1);
        world.btech = serde_json::from_value(state).unwrap();
        let before = world.clone();
        let notices = advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
        let unit = &world.btech.constructed_units()[&id];
        assert_eq!(unit.position().unwrap().y, if success { 1 } else { 2 });
        assert_eq!(
            unit.posture(),
            if success {
                BattlePosture::Standing
            } else {
                BattlePosture::Prone
            }
        );
        assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(-1));
        assert_eq!(unit.motion().unwrap().speed, 0.0);
        if success {
            assert_eq!(
                unit.motion().unwrap().point,
                before.btech.constructed_units()[&id]
                    .motion()
                    .unwrap()
                    .point
            );
        }
        assert!(
            notices
                .iter()
                .any(|notice| notice.text == "You notice a large drop in front of you")
        );
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

#[tokio::test]
async fn ground_ice_entry_fractures_neighbors_and_replays_without_repeated_checks() {
    for depth in [0, 1, 3, 9] {
        for fracture in [false, true] {
            for reverse in [false, true] {
                let (_dir, config, mut world, map, units) =
                    fixture_field(Terrain::Ice, depth, 5).await;
                // Isolate ordinary fracture from the separately tested reactor-blast aftermath.
                configure_battle_reactor_policy(&mut world, false, false);
                let id = units[0];
                prepare_reverse_step(&mut world, id, 12);
                let mut state = serde_json::to_value(&world.btech).unwrap();
                if !reverse {
                    let motion = &mut state["constructed"][id.0.to_string()]["motion"];
                    motion["heading"] = serde_json::json!(180.0);
                    motion["desired_heading"] = serde_json::json!(180.0);
                    motion["speed"] = serde_json::json!(21.5);
                    motion["desired_speed"] = serde_json::json!(21.5);
                }
                let neighbor = &mut state["constructed"][units[1].0.to_string()];
                neighbor["dice"] = serde_json::to_value(BattleDice::seeded([19; 32])).unwrap();
                neighbor["position"]["y"] = serde_json::json!(2);
                neighbor["motion"]["point"] =
                    serde_json::to_value(BattleHexCoordinate { x: 1, y: 2 }.center()).unwrap();
                world.btech = serde_json::from_value(state).unwrap();
                ice_seed(&mut world, id, fracture, false);
                let before = world.clone();
                persistence::save(&config.database(), &world).await.unwrap();
                let mut loaded = persistence::load(&config.database()).await.unwrap();
                configure_battle_reactor_policy(&mut loaded, false, false);
                let notices =
                    advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
                assert_eq!(
                    notices,
                    advance_battle_motion(&mut loaded, BattleMovementRules::STANDARD).unwrap()
                );
                assert_eq!(world.btech, loaded.btech);
                assert_eq!(
                    world.btech.maps()[&map].hex(1, 2).unwrap().terrain(),
                    if fracture {
                        Terrain::Water
                    } else {
                        Terrain::Ice
                    }
                );
                for unit in units {
                    assert_eq!(
                        world.btech.constructed_units()[&unit].position().unwrap().y,
                        2
                    );
                    assert_eq!(
                        world.btech.constructed_units()[&unit].posture(),
                        if fracture && depth > 0 {
                            BattlePosture::Prone
                        } else {
                            BattlePosture::Standing
                        }
                    );
                    assert_eq!(
                        battle_unit_elevation(&world, unit).unwrap(),
                        Some(if fracture { -i32::from(depth) } else { 0 })
                    );
                }
                if !fracture || depth == 0 {
                    let mut dice: BattleDice = serde_json::from_value(serde_json::to_value(&before.btech.constructed_units()[&id]).unwrap()["dice"].clone()).unwrap();
                    assert_eq!(dice.d6() == 1, fracture);
                    assert_eq!(
                        serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"],
                        serde_json::to_value(&dice).unwrap()
                    );
                    assert!(
                        advance_battle_motion(&mut world, BattleMovementRules::STANDARD)
                            .unwrap()
                            .is_empty()
                    );
                    assert_eq!(
                        serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"],
                        serde_json::to_value(&dice).unwrap()
                    );
                }
                world.validate(&config).unwrap();
                persistence::save(&config.database(), &world).await.unwrap();
                let mut saved = persistence::load(&config.database()).await.unwrap();
                configure_battle_reactor_policy(&mut saved, false, false);
                assert_eq!(saved.btech, world.btech);
            }
        }
    }
}

#[tokio::test]
async fn submerged_ice_routes_use_bottom_depth_and_depth_one_surface_transition() {
    for (old_terrain, old_depth, new_terrain, new_depth, height, speed, modifier) in [
        (Terrain::Water, 3, Terrain::Ice, 3, -3, 21.5, Some(1)),
        (Terrain::Ice, 3, Terrain::Ice, 1, -1, 0.0, Some(-1)),
        (Terrain::Ice, 1, Terrain::Ice, 1, 0, 21.5, None),
        (Terrain::Water, 1, Terrain::Ice, 1, 0, 21.5, None),
        (Terrain::Ice, 3, Terrain::Ice, 0, 0, 21.5, None),
        (Terrain::Ice, 3, Terrain::Water, 2, -2, 10.75, Some(0)),
    ] {
        let old_row = format!("{}{old_depth}", old_terrain.symbol()).repeat(3);
        let row = format!("{}{new_depth}", new_terrain.symbol()).repeat(3);
        let source = format!("3 4\n{old_row}\n{old_row}\n{row}\n{row}\n");
        let (_dir, config, mut world, map, units) =
            fixture_asset(BattleMapAsset::from_cells(&source).unwrap()).await;
        let id = units[0];
        prepare_reverse_step(&mut world, id, 12);
        let mut state = serde_json::to_value(&world.btech).unwrap();
        let unit = &mut state["constructed"][id.0.to_string()];
        if old_terrain == Terrain::Ice {
            unit["ground_elevation"] = serde_json::json!(-old_depth);
        }
        unit["motion"]["heading"] = serde_json::json!(180.0);
        unit["motion"]["desired_heading"] = serde_json::json!(180.0);
        unit["motion"]["speed"] = serde_json::json!(21.5);
        unit["motion"]["desired_speed"] = serde_json::json!(21.5);
        world.btech = serde_json::from_value(state).unwrap();
        let before = world.clone();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        let notices = advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
        assert_eq!(
            notices,
            advance_battle_motion(&mut loaded, BattleMovementRules::STANDARD).unwrap()
        );
        assert_eq!(world.btech, loaded.btech);
        assert_eq!(
            world.btech.maps()[&map].hex(1, 2).unwrap().terrain(),
            new_terrain
        );
        assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(height));
        assert_eq!(
            world.btech.constructed_units()[&id].motion().unwrap().speed,
            speed
        );
        let mut expected = before;
        if let Some(modifier) = modifier {
            assert!(
                roll_battle_piloting(&mut expected, id, modifier, true)
                    .unwrap()
                    .success
            );
        }
        assert_eq!(
            serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"],
            serde_json::to_value(&expected.btech.constructed_units()[&id]).unwrap()["dice"]
        );
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn failed_under_ice_control_keeps_the_bottom_and_allows_standing() {
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::Ice, 3, 5).await;
    let id = units[0];
    prepare_reverse_step(&mut world, id, 6);
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][id.0.to_string()]["ground_elevation"] = serde_json::json!(-3);
    // Fail water control but protect the pilot, then hit a torso rather than the head.
    let fall_seed = (0..=255)
        .find(|seed| {
            let mut dice = BattleDice::seeded([*seed; 32]);
            dice.two_d6() == 6 && dice.two_d6() >= 7 && {
                dice.d6();
                dice.two_d6() == 7
            }
        })
        .unwrap();
    state["constructed"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([fall_seed; 32])).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    let notices = advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
    assert!(
        notices
            .iter()
            .any(|notice| notice.text == "You slip in the water and fall down")
    );
    assert_eq!(
        world.btech.constructed_units()[&id].posture(),
        BattlePosture::Prone
    );
    assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(-3));
    assert_eq!(
        world.btech.maps()[&map].hex(1, 2).unwrap().terrain(),
        Terrain::Ice
    );
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
        .unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    let report = begin_battle_stand(
        &mut world,
        id,
        ObjectId(1),
        BattleStandMode::Normal,
        false,
        rules(),
    )
    .unwrap();
    assert!(report.check.success);
    for _ in 0..60 {
        advance_battle_standing(&mut world);
    }
    assert_eq!(
        world.btech.constructed_units()[&id].posture(),
        BattlePosture::Standing
    );
    assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(-3));
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

#[tokio::test]
async fn failed_neighbor_fall_during_ground_ice_fracture_restores_the_tick() {
    let (_dir, config, mut world, _, units) = fixture_field(Terrain::Ice, 3, 5).await;
    let id = units[0];
    prepare_reverse_step(&mut world, id, 12);
    let mut state = serde_json::to_value(&world.btech).unwrap();
    let neighbor = &mut state["constructed"][units[1].0.to_string()];
    neighbor["position"]["y"] = serde_json::json!(2);
    neighbor["motion"]["point"] =
        serde_json::to_value(BattleHexCoordinate { x: 1, y: 2 }.center()).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    world
        .objects
        .get_mut(&units[1])
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    ice_seed(&mut world, id, true, false);
    let before = world.clone();
    assert!(advance_battle_motion(&mut world, BattleMovementRules::STANDARD).is_err());
    assert_eq!(world.btech, before.btech);
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn descending_jump_breaks_previous_ice_before_finishing_horizontal_entry() {
    let source = "3 5\n.0.0.0\n.0.0.0\n-3-3-3\n~5~5~5\n~5~5~5\n";
    let (_dir, config, mut world, map, units) =
        fixture_asset(BattleMapAsset::from_cells(source).unwrap()).await;
    let id = units[0];
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["maps"][map.0.to_string()]["movement_modifier"] = serde_json::json!(800);
    state["constructed"][units[1].0.to_string()]["position"]["y"] = serde_json::json!(2);
    state["constructed"][units[1].0.to_string()]["motion"]["point"] =
        serde_json::to_value(BattleHexCoordinate { x: 1, y: 2 }.center()).unwrap();
    // Protect both pilots and avoid random critical cascades in this geometry fixture.
    let safe = (0..=255)
        .find(|seed| {
            let mut dice = BattleDice::seeded([*seed; 32]);
            let protects = dice.two_d6() >= 7;
            dice.d6();
            protects && dice.two_d6() == 7 && dice.two_d6() >= 7
        })
        .unwrap();
    for unit in units {
        state["constructed"][unit.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([safe; 32])).unwrap();
    }
    world.btech = serde_json::from_value(state).unwrap();
    launch_battle_jump(&mut world, id, ObjectId(1), 180, 2.0).unwrap();
    for _ in 0..2 {
        assert!(
            advance_battle_jumps(
                &mut world,
                stompymux_rs::BattleMovementRules {
                    fall: stompymux_rs::BattleFallRules {
                        stacking: stompymux_rs::BattleStackingRules::STANDARD,
                        ..rules()
                    },
                    ..stompymux_rs::BattleMovementRules::STANDARD
                }
            )
            .unwrap()
            .is_empty()
        );
    }
    assert!(world.btech.constructed_units()[&id].flight().is_some());
    assert_eq!(
        world.btech.constructed_units()[&id].position().unwrap().y,
        2
    );
    let mut rejected = world.clone();
    rejected
        .objects
        .get_mut(&units[1])
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let rejected_before = rejected.btech.clone();
    assert!(
        advance_battle_jumps(
            &mut rejected,
            stompymux_rs::BattleMovementRules {
                fall: stompymux_rs::BattleFallRules {
                    stacking: stompymux_rs::BattleStackingRules::STANDARD,
                    ..rules()
                },
                ..stompymux_rs::BattleMovementRules::STANDARD
            }
        )
        .is_err()
    );
    assert_eq!(rejected.btech, rejected_before);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    let notices = advance_battle_jumps(
        &mut world,
        stompymux_rs::BattleMovementRules {
            fall: stompymux_rs::BattleFallRules {
                stacking: stompymux_rs::BattleStackingRules::STANDARD,
                ..rules()
            },
            ..stompymux_rs::BattleMovementRules::STANDARD
        },
    )
    .unwrap();
    assert_eq!(
        notices,
        advance_battle_jumps(
            &mut loaded,
            stompymux_rs::BattleMovementRules {
                fall: stompymux_rs::BattleFallRules {
                    stacking: stompymux_rs::BattleStackingRules::STANDARD,
                    ..rules()
                },
                ..stompymux_rs::BattleMovementRules::STANDARD
            }
        )
        .unwrap()
    );
    assert_eq!(world.btech, loaded.btech);
    assert!(
        notices
            .iter()
            .any(|notice| notice.text == "You break the ice!")
    );
    assert_eq!(
        world.btech.maps()[&map].hex(1, 2).unwrap().terrain(),
        Terrain::Water
    );
    for unit in units {
        assert_eq!(
            world.btech.constructed_units()[&unit].posture(),
            BattlePosture::Prone
        );
    }
    let unit = &world.btech.constructed_units()[&id];
    assert!(unit.flight().is_none());
    assert!(!unit.hex_sync_pending());
    assert_eq!(unit.position().unwrap().y, 3);
    assert_eq!(
        unit.motion().unwrap().point,
        BattleHexCoordinate { x: 1, y: 3 }.center()
    );
    assert_eq!(unit.jump_stabilization(), 12);
    assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(-3));
    assert_eq!(battle_unit_elevation(&world, units[1]).unwrap(), Some(-3));
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

#[tokio::test]
async fn ascending_saved_flight_breaks_ice_without_falling_or_spending_its_dice() {
    let (_dir, config, mut world, map, units) = airborne_under_ice_fixture().await;
    let observer = fracture_observer(&mut world, &config, map, units);
    let id = units[0];
    let dice = serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"].clone();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    let notices = advance_battle_jumps(
        &mut world,
        stompymux_rs::BattleMovementRules {
            fall: stompymux_rs::BattleFallRules {
                stacking: stompymux_rs::BattleStackingRules::STANDARD,
                ..rules()
            },
            ..stompymux_rs::BattleMovementRules::STANDARD
        },
    )
    .unwrap();
    assert_eq!(
        notices,
        advance_battle_jumps(
            &mut loaded,
            stompymux_rs::BattleMovementRules {
                fall: stompymux_rs::BattleFallRules {
                    stacking: stompymux_rs::BattleStackingRules::STANDARD,
                    ..rules()
                },
                ..stompymux_rs::BattleMovementRules::STANDARD
            }
        )
        .unwrap()
    );
    assert_eq!(world.btech, loaded.btech);
    assert_eq!(notices[0].text, "You break through the ice!");
    let observed: Vec<_> = notices
        .iter()
        .filter(|notice| notice.unit == observer)
        .collect();
    // The crossing cursor is still submerged; only the surface neighbor is visible.
    assert_eq!(observed.len(), 1);
    assert!(observed[0].text.ends_with("goes swimming!"));
    assert_eq!(
        world.btech.maps()[&map].hex(1, 3).unwrap().terrain(),
        Terrain::Water
    );
    assert!(world.btech.constructed_units()[&id].flight().is_some());
    assert_eq!(
        world.btech.constructed_units()[&id].posture(),
        BattlePosture::Standing
    );
    assert_eq!(
        serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"],
        dice
    );
    assert_eq!(
        world.btech.constructed_units()[&units[1]].posture(),
        BattlePosture::Prone
    );
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

/// A valid saved flight below ice after a capacity change, with a surface neighbor.
async fn airborne_under_ice_fixture() -> (tempfile::TempDir, Config, World, ObjectId, [ObjectId; 2])
{
    let source = "3 5\n.0.0.0\n.0.0.0\n.0.0.0\n~5~5~5\n~5~5~5\n";
    let (_dir, config, mut world, map, units) =
        fixture_asset(BattleMapAsset::from_cells(source).unwrap()).await;
    let id = units[0];
    launch_battle_jump(&mut world, id, ObjectId(1), 180, 2.0).unwrap();
    // A lower-capacity sample followed by restored gravity can cross the ice plane upward.
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["maps"][map.0.to_string()]["gravity"] = serde_json::json!(150);
    state["maps"][map.0.to_string()]["movement_modifier"] = serde_json::json!(3240);
    world.btech = serde_json::from_value(state).unwrap();
    advance_battle_jumps(
        &mut world,
        stompymux_rs::BattleMovementRules {
            fall: stompymux_rs::BattleFallRules {
                stacking: stompymux_rs::BattleStackingRules::STANDARD,
                ..rules()
            },
            ..stompymux_rs::BattleMovementRules::STANDARD
        },
    )
    .unwrap();
    assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(-2));
    assert!(world.btech.constructed_units()[&id].flight().is_some());
    // Represent a saved field with ice formed above that already airborne unit.
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["maps"][map.0.to_string()]["gravity"] = serde_json::json!(100);
    state["maps"][map.0.to_string()]["movement_modifier"] = serde_json::json!(20);
    state["maps"][map.0.to_string()]["terrain"][10] =
        serde_json::to_value(BattleHex::new(Terrain::Ice, 5)).unwrap();
    state["constructed"][units[1].0.to_string()]["position"]["y"] = serde_json::json!(3);
    state["constructed"][units[1].0.to_string()]["motion"]["point"] =
        serde_json::to_value(BattleHexCoordinate { x: 1, y: 3 }.center()).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    world.validate(&config).unwrap();
    (_dir, config, world, map, units)
}

#[tokio::test]
async fn landing_in_existing_ice_precedes_the_final_upward_breakout() {
    for fracture in [false, true] {
        let (_dir, config, mut world, map, units) = airborne_under_ice_fixture().await;
        let observer = fracture_observer(&mut world, &config, map, units);
        let id = units[0];
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["maps"][map.0.to_string()]["movement_modifier"] = serde_json::json!(1000);
        world.btech = serde_json::from_value(state).unwrap();
        ice_seed(&mut world, id, fracture, false);
        // Keep the neighbor's fall reproducible: a random reactor breach can add steam
        // and change the jumper's landing damage, independently of ice-break ordering.
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][units[1].0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([42; 32])).unwrap();
        world.btech = serde_json::from_value(state).unwrap();
        let before = world.clone();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        let notices = advance_battle_jumps(
            &mut world,
            stompymux_rs::BattleMovementRules {
                fall: stompymux_rs::BattleFallRules {
                    stacking: stompymux_rs::BattleStackingRules::STANDARD,
                    ..rules()
                },
                ..stompymux_rs::BattleMovementRules::STANDARD
            },
        )
        .unwrap();
        assert_eq!(
            notices,
            advance_battle_jumps(
                &mut loaded,
                stompymux_rs::BattleMovementRules {
                    fall: stompymux_rs::BattleFallRules {
                        stacking: stompymux_rs::BattleStackingRules::STANDARD,
                        ..rules()
                    },
                    ..stompymux_rs::BattleMovementRules::STANDARD
                }
            )
            .unwrap()
        );
        assert_eq!(world.btech, loaded.btech);
        assert_eq!(
            notices
                .iter()
                .filter(|notice| notice.unit == observer
                    && notice.text.ends_with("breaks through the ice!"))
                .count(),
            usize::from(!fracture)
        );
        assert_eq!(
            world.btech.maps()[&map].hex(1, 3).unwrap().terrain(),
            Terrain::Water,
            "fracture={fracture}; before={:?}; after={:?}; notices={notices:?}",
            before.btech.maps()[&map].decoration(BattleHexCoordinate { x: 1, y: 3 }),
            world.btech.maps()[&map].decoration(BattleHexCoordinate { x: 1, y: 3 }),
        );
        let unit = &world.btech.constructed_units()[&id];
        assert!(unit.flight().is_none());
        assert_eq!(unit.jump_stabilization(), 12);
        assert_eq!(
            unit.posture(),
            if fracture {
                BattlePosture::Prone
            } else {
                BattlePosture::Standing
            }
        );
        assert_eq!(
            battle_unit_elevation(&world, id).unwrap(),
            Some(if fracture { -5 } else { 0 })
        );
        assert_eq!(
            world.btech.constructed_units()[&units[1]].posture(),
            BattlePosture::Prone
        );
        assert_eq!(
            notices
                .iter()
                .any(|notice| notice.text == "You break through the ice!"),
            !fracture
        );
        if !fracture {
            let mut dice: BattleDice = serde_json::from_value(
                serde_json::to_value(&before.btech.constructed_units()[&id]).unwrap()["dice"]
                    .clone(),
            )
            .unwrap();
            assert_ne!(dice.d6(), 1);
            assert_eq!(
                serde_json::to_value(unit).unwrap()["dice"],
                serde_json::to_value(dice).unwrap()
            );
        }
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

#[tokio::test]
async fn wet_running_controls_match_native_lua_and_preserve_rejected_throttle() {
    for (terrain, depth, altitude, wet) in [
        (Terrain::Water, 1, None, true),
        (Terrain::Water, 0, None, false),
        (Terrain::Ice, 3, None, false),
        (Terrain::Ice, 3, Some(-3), true),
        (Terrain::Bridge, 3, None, false),
        (Terrain::Bridge, 3, Some(-1), true),
    ] {
        let (_dir, config, mut world, _, units) = fixture_field(terrain, depth, 5).await;
        let id = units[0];
        let maximum = world.btech.constructed_units()[&id]
            .mobility()
            .maximum_speed;
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][id.0.to_string()]["ground_elevation"] =
            serde_json::to_value(altitude).unwrap();
        world.btech = serde_json::from_value(state).unwrap();
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let lua = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let text = support::run_text(&native, &config, ObjectId(1), 1, "speed run");
        let result = lua.eval_callback::<()>(&format!("btech.unit.speed({}, 1, {maximum})", id.0));
        assert_eq!(result.is_err(), wet);
        assert_eq!(native.world().btech, lua.world().btech);
        if wet {
            assert!(text.contains("You can't run through water!"));
            assert_eq!(native.world().btech, world.btech);
        }
        let walking = maximum * 2.0 / 3.0;
        set_battle_speed(&mut world, id, ObjectId(1), walking + 0.1).unwrap();
        let before = world.btech.clone();
        assert_eq!(
            set_battle_speed(&mut world, id, ObjectId(1), walking + 0.2).is_err(),
            wet
        );
        if wet {
            assert_eq!(world.btech, before);
        }
        set_battle_speed(&mut world, id, ObjectId(1), -walking).unwrap();
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn mapped_surface_jump_routes_land_at_height_and_replay() {
    for terrain in [Terrain::Building, Terrain::Wall] {
        for height in [0, 3, 9] {
            let (_dir, config, mut world, map, units) = fixture_field(terrain, height, 5).await;
            let id = units[0];
            let mut state = serde_json::to_value(&world.btech).unwrap();
            state["maps"][map.0.to_string()]["movement_modifier"] = serde_json::json!(800);
            world.btech = serde_json::from_value(state).unwrap();
            launch_battle_jump(&mut world, id, ObjectId(1), 180, 1.0).unwrap();
            assert!(
                advance_battle_jumps(
                    &mut world,
                    stompymux_rs::BattleMovementRules {
                        fall: stompymux_rs::BattleFallRules {
                            stacking: stompymux_rs::BattleStackingRules::STANDARD,
                            ..rules()
                        },
                        ..stompymux_rs::BattleMovementRules::STANDARD
                    }
                )
                .unwrap()
                .is_empty()
            );
            persistence::save(&config.database(), &world).await.unwrap();
            let mut loaded = persistence::load(&config.database()).await.unwrap();
            assert_eq!(
                advance_battle_jumps(
                    &mut world,
                    stompymux_rs::BattleMovementRules {
                        fall: stompymux_rs::BattleFallRules {
                            stacking: stompymux_rs::BattleStackingRules::STANDARD,
                            ..rules()
                        },
                        ..stompymux_rs::BattleMovementRules::STANDARD
                    }
                )
                .unwrap(),
                advance_battle_jumps(
                    &mut loaded,
                    stompymux_rs::BattleMovementRules {
                        fall: stompymux_rs::BattleFallRules {
                            stacking: stompymux_rs::BattleStackingRules::STANDARD,
                            ..rules()
                        },
                        ..stompymux_rs::BattleMovementRules::STANDARD
                    }
                )
                .unwrap()
            );
            assert_eq!(world.btech, loaded.btech);
            assert!(world.btech.constructed_units()[&id].flight().is_none());
            assert_eq!(
                world.btech.constructed_units()[&id].posture(),
                BattlePosture::Standing
            );
            assert_eq!(
                battle_unit_elevation(&world, id).unwrap(),
                Some(i32::from(height))
            );
            world.validate(&config).unwrap();
        }
    }
}

#[tokio::test]
async fn structure_jump_controls_land_and_collide_with_saved_replay() {
    for symbol in ['@', '='] {
        for height in [0, 3, 9] {
            let row = format!("{symbol}{height}").repeat(3);
            let source = format!("3 5\n.0.0.0\n.0.0.0\n{row}\n.0.0.0\n.0.0.0\n");
            let (_dir, config, mut world, map, units) =
                fixture_asset(BattleMapAsset::from_cells(&source).unwrap()).await;
            let id = units[0];
            let mut state = serde_json::to_value(&world.btech).unwrap();
            state["maps"][map.0.to_string()]["movement_modifier"] = serde_json::json!(400);
            state["constructed"][id.0.to_string()]["dice"] =
                serde_json::to_value(BattleDice::seeded([3; 32])).unwrap();
            world.btech = serde_json::from_value(state).unwrap();
            let native = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
            )
            .unwrap();
            let lua = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
            )
            .unwrap();
            support::run_text(&native, &config, ObjectId(1), 1, "jump 180 2");
            lua.eval_callback::<()>(&format!("btech.unit.jump({},1,180,2)", id.0))
                .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            world = native.world().clone();
            assert!(world.btech.constructed_units()[&id].flight().is_some());
            persistence::save(&config.database(), &world).await.unwrap();
            let mut loaded = persistence::load(&config.database()).await.unwrap();
            let mut notices = Vec::new();
            for _ in 0..6 {
                let tick = advance_battle_jumps(
                    &mut world,
                    stompymux_rs::BattleMovementRules {
                        fall: stompymux_rs::BattleFallRules {
                            stacking: stompymux_rs::BattleStackingRules::STANDARD,
                            ..rules()
                        },
                        ..stompymux_rs::BattleMovementRules::STANDARD
                    },
                )
                .unwrap();
                assert_eq!(
                    tick,
                    advance_battle_jumps(
                        &mut loaded,
                        stompymux_rs::BattleMovementRules {
                            fall: stompymux_rs::BattleFallRules {
                                stacking: stompymux_rs::BattleStackingRules::STANDARD,
                                ..rules()
                            },
                            ..stompymux_rs::BattleMovementRules::STANDARD
                        }
                    )
                    .unwrap()
                );
                notices.extend(tick);
                assert_eq!(world.btech, loaded.btech);
            }
            let unit = &world.btech.constructed_units()[&id];
            assert!(unit.flight().is_none());
            assert_eq!(
                unit.posture(),
                if height == 9 {
                    BattlePosture::Prone
                } else {
                    BattlePosture::Standing
                }
            );
            assert_eq!(unit.position().unwrap().y, if height == 9 { 1 } else { 3 });
            assert_eq!(
                notices
                    .iter()
                    .any(|notice| notice.text.contains("too high")),
                height == 9
            );
            world.validate(&config).unwrap();
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
        }
    }
}

#[tokio::test]
async fn live_fire_and_smoke_tiles_allow_ground_crossings_without_control_dice() {
    for terrain in [Terrain::Fire, Terrain::Smoke] {
        let (_dir, config, mut world, map, units) = fixture_field(Terrain::Grassland, 0, 5).await;
        let id = units[0];
        prepare_reverse_step(&mut world, id, 6);
        let kind = match terrain {
            Terrain::Fire => BattleDecorationKind::Fire,
            _ => BattleDecorationKind::Smoke,
        };
        set_map_decoration(
            &mut world,
            map,
            BattleHexCoordinate { x: 1, y: 2 },
            Some(BattleDecoration::new(kind, 0, None)),
        )
        .unwrap();
        let before = serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        assert!(
            advance_battle_motion(&mut world, BattleMovementRules::STANDARD)
                .unwrap()
                .is_empty()
        );
        assert!(
            advance_battle_motion(&mut loaded, BattleMovementRules::STANDARD)
                .unwrap()
                .is_empty()
        );
        assert_eq!(world.btech, loaded.btech);
        let unit = &world.btech.constructed_units()[&id];
        assert_eq!(unit.position().unwrap().y, 2);
        assert_eq!(unit.motion().unwrap().speed, -21.5);
        assert_eq!(before["dice"], serde_json::to_value(unit).unwrap()["dice"]);
        assert_eq!(
            world.btech.maps()[&map].hex(1, 2).unwrap().terrain(),
            terrain
        );
        world.validate(&config).unwrap();
    }
}

/// A running observer outside the fractured hex with saved prior contacts to both occupants.
fn fracture_observer(
    world: &mut World,
    config: &Config,
    map: ObjectId,
    units: [ObjectId; 2],
) -> ObjectId {
    let observer = world.create(config, "Fracture observer".into(), Kind::Thing);
    create_battle_unit(
        world,
        observer,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    place_battle_unit(world, observer, map, 2, 1).unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    let viewer = &mut state["constructed"][observer.0.to_string()];
    viewer["power"] = serde_json::json!({"state":"running"});
    for unit in units {
        viewer["contacts"][unit.0.to_string()] = serde_json::json!({"identified":false});
    }
    world.btech = serde_json::from_value(state).unwrap();
    observer
}

/// Fracture notices preserve surface visibility and trigger/neighbor order through atomic replay.
#[tokio::test]
async fn fracture_observers_capture_breaker_and_occupants_before_submersion() {
    for (terrain, depth) in [
        (Terrain::Ice, 0),
        (Terrain::Ice, 3),
        (Terrain::Bridge, 1),
        (Terrain::Bridge, 4),
    ] {
        let (_dir, config, mut base, map, units) = fixture_surface(terrain, depth).await;
        let observer = fracture_observer(&mut base, &config, map, units);
        for triggered in [false, true] {
            if terrain == Terrain::Bridge && triggered {
                continue;
            }
            for visible in [false, true] {
                let mut world = base.clone();
                if !visible {
                    let mut state = serde_json::to_value(&world.btech).unwrap();
                    state["constructed"][observer.0.to_string()]["power"] =
                        serde_json::json!({"state":"off"});
                    world.btech = serde_json::from_value(state).unwrap();
                }
                // Deck occupants always fall; ice occupants fall only into deeper water.
                let expected_falls = terrain == Terrain::Bridge || depth > 0;
                let mut expected = Vec::new();
                let mut append = |id, text| {
                    expected.extend(
                        battle_observer_messages(&world, id, text)
                            .into_iter()
                            .filter(|(recipient, _)| *recipient == observer)
                            .map(|(_, text)| text),
                    )
                };
                if triggered {
                    append(units[0], "breaks the ice!");
                    if depth > 0 {
                        append(units[0], "vanishes into the waters!");
                    }
                }
                if expected_falls {
                    for unit in units {
                        if triggered && unit == units[0] {
                            continue;
                        }
                        append(
                            unit,
                            if terrain == Terrain::Bridge {
                                "goes swimming as the bridge is blown apart!"
                            } else if triggered {
                                "goes swimming!"
                            } else {
                                "goes swimming as ice breaks!"
                            },
                        );
                    }
                }
                assert_eq!(
                    !expected.is_empty(),
                    visible && (triggered || expected_falls)
                );
                let resolve = |world: &mut World| {
                    let coordinate = BattleHexCoordinate { x: 1, y: 1 };
                    if terrain == Terrain::Bridge {
                        break_battle_bridge(world, map, coordinate, rules())
                    } else {
                        break_battle_ice(
                            world,
                            map,
                            coordinate,
                            triggered.then_some(units[0]),
                            rules(),
                        )
                    }
                };
                if expected_falls {
                    let mut rejected = world.clone();
                    rejected
                        .objects
                        .get_mut(&units[0])
                        .unwrap()
                        .flags
                        .insert(Flag::InCharacter);
                    let before = rejected.btech.clone();
                    assert!(resolve(&mut rejected).is_err());
                    assert_eq!(rejected.btech, before);
                }
                persistence::save(&config.database(), &world).await.unwrap();
                let mut restored = persistence::load(&config.database()).await.unwrap();
                let report = resolve(&mut world).unwrap();
                assert_eq!(resolve(&mut restored).unwrap(), report);
                assert_eq!(world.btech, restored.btech);
                let observed: Vec<_> = report
                    .notices
                    .iter()
                    .filter(|notice| notice.unit == observer)
                    .map(|notice| notice.text.clone())
                    .collect();
                assert_eq!(observed, expected);
                let before = world.btech.clone();
                assert!(resolve(&mut world).is_err());
                assert_eq!(world.btech, before);
            }
        }
    }
}

/// Explicit surface actions support mixed crews while ordinary callers retain casualty guards.
#[tokio::test]
async fn character_surface_actions_evacuate_and_roll_back_terrain() {
    for (terrain, depth) in [(Terrain::Ice, 2), (Terrain::Bridge, 1)] {
        let (_dir, config, mut world, map, units) = fixture_surface(terrain, depth).await;
        let victim = units[1];
        world
            .objects
            .get_mut(&ObjectId(2))
            .unwrap()
            .flags
            .remove(Flag::Wizard);
        world
            .objects
            .get_mut(&victim)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        set_battle_character(
            &mut world,
            ObjectId(2),
            BattleCharacter {
                build: 5,
                reflexes: 5,
                intuition: 5,
                learn: 5,
                charisma: 5,
                bruise: 0,
                lethal: 0,
            },
        )
        .unwrap();
        apply_damage_phase(
            &mut world,
            victim,
            BattleSection::Head,
            7,
            BattleDamagePhase::Armor { rear: false },
        )
        .unwrap();
        let coordinate = BattleHexCoordinate { x: 1, y: 1 };
        for pilot in [ObjectId(1), ObjectId(2)] {
            world
                .objects
                .get_mut(&pilot)
                .unwrap()
                .flags
                .insert(Flag::Connected);
        }
        let passenger = world.create(&config, "Surface passenger".into(), Kind::Player);
        world.objects.get_mut(&passenger).unwrap().location = Some(units[0]);
        world
            .objects
            .get_mut(&passenger)
            .unwrap()
            .flags
            .insert(Flag::Connected);
        let baseline = world.clone();
        let ordinary = if terrain == Terrain::Ice {
            break_battle_ice(&mut world, map, coordinate, None, rules())
        } else {
            break_battle_bridge(&mut world, map, coordinate, rules())
        };
        assert!(ordinary.is_err());
        assert_eq!(world.btech, baseline.btech);
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let afterlife = ObjectId(config.battletech.afterlife_dbref);
        scripts.world_mut().objects.remove(&afterlife);
        assert!(
            break_battle_surface_action(
                &scripts,
                &config,
                map,
                coordinate,
                BattleSurface::of(BattleHex::new(terrain, 1)).unwrap(),
                rules()
            )
            .is_err()
        );
        assert_eq!(scripts.world().btech, baseline.btech);
        assert_eq!(scripts.world().objects[&ObjectId(2)].location, Some(victim));
        assert!(scripts.drain_outbox().is_empty());
        *scripts.world_mut() = baseline;
        let report = break_battle_surface_action(
            &scripts,
            &config,
            map,
            coordinate,
            BattleSurface::of(BattleHex::new(terrain, 1)).unwrap(),
            rules(),
        )
        .unwrap();
        assert_eq!(report.falls.len(), 2);
        let output = scripts.drain_outbox();
        let mut checked = 0;
        for (_, fall) in &report.falls {
            let Some(check) = fall.avoidance.filter(|check| check.roll.is_some()) else {
                continue;
            };
            let pilot = fall.pilot.unwrap();
            let messages: Vec<_> = output
                .iter()
                .filter(|(who, _)| *who == pilot)
                .map(|(_, message)| message.source())
                .collect();
            let expected = format!(
                "Modified Pilot Skill: BTH {}\tRoll: {}",
                check.target,
                check.roll.unwrap()
            );
            let index = messages
                .iter()
                .position(|message| *message == expected)
                .unwrap();
            assert!(index > 0, "the roll introduction precedes its result");
            assert_eq!(messages[index - 1], "You make a piloting skill roll!");
            checked += 1;
        }
        assert!(checked > 0);
        assert!(output.iter().any(|(who, _)| *who == passenger));
        assert!(!output.iter().any(|(who, message)| *who == passenger
            && (message.source().starts_with("Modified Pilot Skill:")
                || message.source() == "You make a piloting skill roll!")));

        assert_eq!(report.after.terrain(), Terrain::Water);
        assert_eq!(
            scripts.world().objects[&ObjectId(2)].location,
            Some(afterlife)
        );
        assert_eq!(
            scripts.world().objects[&ObjectId(1)].location,
            Some(units[0])
        );
        let candidate = scripts.world().clone();
        assert!(candidate.btech.constructed_units()[&victim].is_destroyed());
        assert!(
            candidate.btech.constructed_units()[&victim].sections()[&BattleSection::Head].internal
                > 0
        );
        candidate.validate(&config).unwrap();
        persistence::save(&config.database(), &candidate)
            .await
            .unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, candidate.btech);
        assert_eq!(loaded.objects[&ObjectId(2)].location, Some(afterlife));
    }
}

/// An in-character fall can fracture ice, drop neighbors first and publish the nested casualty.
#[tokio::test]
async fn character_fall_fractures_ice_with_nested_evacuation() {
    let (_dir, config, mut world, map, units) = fixture(2).await;
    let trigger = units[1];
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    world
        .objects
        .get_mut(&trigger)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    set_battle_character(
        &mut world,
        ObjectId(2),
        BattleCharacter {
            build: 5,
            reflexes: 5,
            intuition: 5,
            learn: 5,
            charisma: 5,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    apply_damage_phase(
        &mut world,
        trigger,
        BattleSection::Head,
        7,
        BattleDamagePhase::Armor { rear: false },
    )
    .unwrap();
    let seed = (0..=255)
        .find(|seed| {
            let mut dice = BattleDice::seeded([*seed; 32]);
            dice.two_d6();
            dice.d6() == 1
        })
        .unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][trigger.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let report = fall_battle_unit_action(&scripts, &config, trigger, 1, rules()).unwrap();
    let fracture = report.ice_break.unwrap();
    assert_eq!(
        fracture.falls.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        vec![units[0], trigger]
    );
    assert_eq!(
        scripts.world().btech.maps()[&map]
            .hex(1, 1)
            .unwrap()
            .terrain(),
        Terrain::Water
    );
    assert_eq!(
        scripts.world().objects[&ObjectId(2)].location,
        Some(ObjectId(config.battletech.afterlife_dbref))
    );
    assert!(scripts.world().btech.constructed_units()[&trigger].is_destroyed());
    scripts.world().validate(&config).unwrap();
}

/// Upward fracture excludes the breaker and publishes character neighbor losses atomically.
#[tokio::test]
async fn upward_character_breakout_preserves_breaker_and_rolls_back() {
    for (terrain, depth) in [(Terrain::Ice, 2), (Terrain::Ice, 3)] {
        let (_dir, config, mut world, map, units) = fixture_surface(terrain, depth).await;
        let victim = units[1];
        for pilot in [ObjectId(1), ObjectId(2)] {
            world
                .objects
                .get_mut(&pilot)
                .unwrap()
                .flags
                .insert(Flag::Connected);
        }
        world
            .objects
            .get_mut(&ObjectId(2))
            .unwrap()
            .flags
            .remove(Flag::Wizard);
        world
            .objects
            .get_mut(&victim)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        set_battle_character(
            &mut world,
            ObjectId(2),
            BattleCharacter {
                build: 5,
                reflexes: 5,
                intuition: 5,
                learn: 5,
                charisma: 5,
                bruise: 0,
                lethal: 0,
            },
        )
        .unwrap();
        apply_damage_phase(
            &mut world,
            victim,
            BattleSection::Head,
            7,
            BattleDamagePhase::Armor { rear: false },
        )
        .unwrap();
        let coordinate = BattleHexCoordinate { x: 1, y: 1 };
        let baseline = world.clone();
        let breaker_before = world.btech.constructed_units()[&units[0]].clone();
        let breaker_height = battle_unit_elevation(&world, units[0]).unwrap();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let afterlife = ObjectId(config.battletech.afterlife_dbref);
        scripts.world_mut().objects.remove(&afterlife);
        assert!(
            break_battle_ice_upward_action(&scripts, &config, map, coordinate, units[0], rules())
                .is_err()
        );
        assert_eq!(scripts.world().btech, baseline.btech);
        assert_eq!(scripts.world().objects[&ObjectId(2)].location, Some(victim));
        assert!(scripts.drain_outbox().is_empty());
        persistence::save(&config.database(), &baseline)
            .await
            .unwrap();
        let replay = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(
                persistence::load(&config.database()).await.unwrap(),
            )),
        )
        .unwrap();
        for pilot in [ObjectId(1), ObjectId(2)] {
            replay
                .world_mut()
                .objects
                .get_mut(&pilot)
                .unwrap()
                .flags
                .insert(Flag::Connected);
        }
        *scripts.world_mut() = baseline;
        let report =
            break_battle_ice_upward_action(&scripts, &config, map, coordinate, units[0], rules())
                .unwrap();
        assert_eq!(
            report.falls.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            vec![victim]
        );
        assert_eq!(
            battle_unit_elevation(&scripts.world(), units[0]).unwrap(),
            breaker_height
        );
        assert_eq!(
            scripts.world().btech.constructed_units()[&units[0]].posture(),
            breaker_before.posture()
        );
        assert_eq!(
            scripts.world().btech.constructed_units()[&units[0]].sections(),
            breaker_before.sections()
        );
        let repeated =
            break_battle_ice_upward_action(&replay, &config, map, coordinate, units[0], rules())
                .unwrap();
        assert_eq!(report, repeated);
        let output = scripts.drain_outbox();
        assert_eq!(output, replay.drain_outbox());
        let check = report.falls[0].1.avoidance.unwrap();
        let expected = format!(
            "Modified Pilot Skill: BTH {}\tRoll: {}",
            check.target,
            check
                .roll
                .expect("the neighboring pilot rolls for protection")
        );
        let intro = output
            .iter()
            .position(|(who, message)| {
                *who == ObjectId(2) && message.source() == "You make a piloting skill roll!"
            })
            .unwrap();
        assert_eq!(output[intro + 1].0, ObjectId(2));
        assert_eq!(output[intro + 1].1.source(), expected);
        assert!(
            output[..intro]
                .iter()
                .any(|(_, message)| { message.source() == "You break through the ice!" }),
            "upward-break notices precede the neighboring fall roll"
        );
        assert!(!output.iter().any(|(who, message)| {
            *who != ObjectId(2) && message.source().starts_with("Modified Pilot Skill:")
        }));
        assert_eq!(scripts.world().btech, replay.world().btech);
        assert_eq!(report.after.terrain(), Terrain::Water);
        assert_eq!(
            scripts.world().objects[&ObjectId(2)].location,
            Some(afterlife)
        );
        assert_eq!(
            scripts.world().objects[&ObjectId(1)].location,
            Some(units[0])
        );
        let candidate = scripts.world().clone();
        assert!(candidate.btech.constructed_units()[&victim].is_destroyed());
        assert!(
            candidate.btech.constructed_units()[&victim].sections()[&BattleSection::Head].internal
                > 0
        );
        candidate.validate(&config).unwrap();
        persistence::save(&config.database(), &candidate)
            .await
            .unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, candidate.btech);
        assert_eq!(loaded.objects[&ObjectId(2)].location, Some(afterlife));
    }
}

/// Landing fracture and upward crossing publish neighboring character casualties in the live tick.
#[tokio::test]
async fn airborne_ice_action_evacuates_neighbors_and_replays() {
    for (modifier, fracture) in [(20, false), (1000, false), (1000, true)] {
        let (_dir, config, mut world, map, units) = airborne_under_ice_fixture().await;
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["maps"][map.0.to_string()]["movement_modifier"] = modifier.into();
        world.btech = serde_json::from_value(state).unwrap();
        ice_seed(&mut world, units[0], fracture, false);
        let movement = BattleMovementRules {
            fall: rules(),
            ..BattleMovementRules::STANDARD
        };
        let victim = units[1];
        world
            .objects
            .get_mut(&ObjectId(2))
            .unwrap()
            .flags
            .remove(Flag::Wizard);
        world
            .objects
            .get_mut(&victim)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        set_battle_character(
            &mut world,
            ObjectId(2),
            BattleCharacter {
                build: 5,
                reflexes: 5,
                intuition: 5,
                learn: 5,
                charisma: 5,
                bruise: 0,
                lethal: 0,
            },
        )
        .unwrap();
        apply_damage_phase(
            &mut world,
            victim,
            BattleSection::Head,
            7,
            BattleDamagePhase::Armor { rear: false },
        )
        .unwrap();
        let baseline = world.clone();
        assert!(advance_battle_jumps(&mut world, movement).is_err());
        assert_eq!(world.btech, baseline.btech);
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let afterlife = ObjectId(config.battletech.afterlife_dbref);
        scripts.world_mut().objects.remove(&afterlife);
        assert!(advance_battle_jumps_action(&scripts, &config, movement).is_err());
        assert_eq!(scripts.world().btech, baseline.btech);
        assert_eq!(scripts.world().objects[&ObjectId(2)].location, Some(victim));
        assert!(scripts.drain_outbox().is_empty());
        persistence::save(&config.database(), &baseline)
            .await
            .unwrap();
        let replay = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(
                persistence::load(&config.database()).await.unwrap(),
            )),
        )
        .unwrap();
        *scripts.world_mut() = baseline;
        advance_battle_jumps_action(&scripts, &config, movement).unwrap();
        advance_battle_jumps_action(&replay, &config, movement).unwrap();
        assert_eq!(scripts.world().btech, replay.world().btech);
        assert_eq!(
            scripts.world().btech.maps()[&map]
                .hex(1, 3)
                .unwrap()
                .terrain(),
            Terrain::Water
        );
        if !fracture {
            assert_eq!(
                scripts.world().btech.constructed_units()[&units[0]].posture(),
                BattlePosture::Standing
            );
        }
        assert_eq!(
            scripts.world().objects[&ObjectId(2)].location,
            Some(afterlife)
        );
        assert_eq!(
            scripts.world().objects[&ObjectId(1)].location,
            Some(units[0])
        );
        let candidate = scripts.world().clone();
        assert!(candidate.btech.constructed_units()[&victim].is_destroyed());
        assert!(
            candidate.btech.constructed_units()[&victim].sections()[&BattleSection::Head].internal
                > 0
        );
        candidate.validate(&config).unwrap();
        persistence::save(&config.database(), &candidate)
            .await
            .unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, candidate.btech);
        assert_eq!(loaded.objects[&ObjectId(2)].location, Some(afterlife));
    }
}

/// Character falls preserve horizontal synchronization and neighboring evacuation on ice descent.
#[tokio::test]
async fn character_interrupted_jump_finishes_water_entry_atomically() {
    let source = "3 5\n.0.0.0\n.0.0.0\n-3-3-3\n~5~5~5\n~5~5~5\n";
    let (_dir, config, mut world, map, units) =
        fixture_asset(BattleMapAsset::from_cells(source).unwrap()).await;
    let id = units[0];
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["maps"][map.0.to_string()]["movement_modifier"] = serde_json::json!(800);
    state["constructed"][units[1].0.to_string()]["position"]["y"] = serde_json::json!(2);
    state["constructed"][units[1].0.to_string()]["motion"]["point"] =
        serde_json::to_value(BattleHexCoordinate { x: 1, y: 2 }.center()).unwrap();
    // Protect both pilots and avoid random critical cascades in this geometry fixture.
    let safe = (0..=255)
        .find(|seed| {
            let mut dice = BattleDice::seeded([*seed; 32]);
            let protects = dice.two_d6() >= 7;
            dice.d6();
            protects && dice.two_d6() == 7 && dice.two_d6() >= 7
        })
        .unwrap();
    for unit in units {
        state["constructed"][unit.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([safe; 32])).unwrap();
    }
    world.btech = serde_json::from_value(state).unwrap();
    launch_battle_jump(&mut world, id, ObjectId(1), 180, 2.0).unwrap();
    for _ in 0..2 {
        assert!(
            advance_battle_jumps(
                &mut world,
                stompymux_rs::BattleMovementRules {
                    fall: stompymux_rs::BattleFallRules {
                        stacking: stompymux_rs::BattleStackingRules::STANDARD,
                        ..rules()
                    },
                    ..stompymux_rs::BattleMovementRules::STANDARD
                }
            )
            .unwrap()
            .is_empty()
        );
    }
    assert!(world.btech.constructed_units()[&id].flight().is_some());
    assert_eq!(
        world.btech.constructed_units()[&id].position().unwrap().y,
        2
    );
    let movement = BattleMovementRules {
        fall: rules(),
        ..BattleMovementRules::STANDARD
    };
    for (unit, pilot) in [(units[0], ObjectId(1)), (units[1], ObjectId(2))] {
        world
            .objects
            .get_mut(&unit)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        set_battle_character(
            &mut world,
            pilot,
            BattleCharacter {
                build: 5,
                reflexes: 5,
                intuition: 5,
                learn: 5,
                charisma: 5,
                bruise: 0,
                lethal: 0,
            },
        )
        .unwrap();
    }
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    apply_damage_phase(
        &mut world,
        units[1],
        BattleSection::Head,
        7,
        BattleDamagePhase::Armor { rear: false },
    )
    .unwrap();
    let baseline = world.clone();
    assert!(advance_battle_jumps(&mut world, movement).is_err());
    assert_eq!(world.btech, baseline.btech);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let afterlife = ObjectId(config.battletech.afterlife_dbref);
    scripts.world_mut().objects.remove(&afterlife);
    assert!(advance_battle_jumps_action(&scripts, &config, movement).is_err());
    assert_eq!(scripts.world().btech, baseline.btech);
    assert_eq!(
        scripts.world().objects[&ObjectId(2)].location,
        Some(units[1])
    );
    assert!(scripts.drain_outbox().is_empty());
    persistence::save(&config.database(), &baseline)
        .await
        .unwrap();
    let replay = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(
            persistence::load(&config.database()).await.unwrap(),
        )),
    )
    .unwrap();
    *scripts.world_mut() = baseline;
    advance_battle_jumps_action(&scripts, &config, movement).unwrap();
    advance_battle_jumps_action(&replay, &config, movement).unwrap();
    assert_eq!(scripts.world().btech, replay.world().btech);
    let candidate = scripts.world().clone();
    let unit = &candidate.btech.constructed_units()[&id];
    assert!(unit.flight().is_none());
    assert!(!unit.hex_sync_pending());
    assert_eq!(unit.position().unwrap().y, 3);
    assert_eq!(
        unit.motion().unwrap().point,
        BattleHexCoordinate { x: 1, y: 3 }.center()
    );
    assert_eq!(unit.posture(), BattlePosture::Prone);
    assert_eq!(battle_unit_elevation(&candidate, id).unwrap(), Some(-3));
    assert_eq!(
        candidate.btech.maps()[&map].hex(1, 2).unwrap().terrain(),
        Terrain::Water
    );
    assert!(candidate.btech.constructed_units()[&units[1]].is_destroyed());
    assert_eq!(candidate.objects[&ObjectId(2)].location, Some(afterlife));
    candidate.validate(&config).unwrap();
    persistence::save(&config.database(), &candidate)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        candidate.btech
    );
}

/// The ground heartbeat publishes water-entry character falls and retries failed crew evacuation.
#[tokio::test]
async fn character_ground_water_entry_replays_and_rolls_back() {
    for fatal in [false, true] {
        let (_dir, config, mut world, _, units) = fixture_field(Terrain::Water, 2, 5).await;
        let id = units[0];
        let pilot = ObjectId(2);
        release_battle_pilot(&mut world, units[1], pilot).unwrap();
        release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        world
            .objects
            .get_mut(&pilot)
            .unwrap()
            .flags
            .remove(Flag::Wizard);
        world.objects.get_mut(&pilot).unwrap().location = Some(id);
        assign_battle_pilot(&mut world, id, pilot).unwrap();
        world
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        set_battle_character(
            &mut world,
            pilot,
            BattleCharacter {
                build: 5,
                reflexes: 5,
                intuition: 5,
                learn: 5,
                charisma: 5,
                bruise: 0,
                lethal: 0,
            },
        )
        .unwrap();
        if fatal {
            apply_damage_phase(
                &mut world,
                id,
                BattleSection::Head,
                7,
                BattleDamagePhase::Armor { rear: false },
            )
            .unwrap();
        }
        prepare_reverse_step(&mut world, id, 2);
        let baseline = world.clone();
        let rules = BattleMovementRules::STANDARD;
        assert!(advance_battle_motion(&mut world, rules).is_err());
        assert_eq!(world.btech, baseline.btech);
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let afterlife = ObjectId(config.battletech.afterlife_dbref);
        if fatal {
            scripts.world_mut().objects.remove(&afterlife);
            assert!(advance_battle_motion_action(&scripts, &config, rules).is_err());
            assert_eq!(scripts.world().btech, baseline.btech);
            assert_eq!(scripts.world().objects[&pilot].location, Some(id));
            assert!(scripts.drain_outbox().is_empty());
            *scripts.world_mut() = baseline.clone();
        }
        persistence::save(&config.database(), &baseline)
            .await
            .unwrap();
        let replay = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(
                persistence::load(&config.database()).await.unwrap(),
            )),
        )
        .unwrap();
        advance_battle_motion_action(&scripts, &config, rules).unwrap();
        advance_battle_motion_action(&replay, &config, rules).unwrap();
        assert_eq!(scripts.world().btech, replay.world().btech);
        let candidate = scripts.world().clone();
        let unit = &candidate.btech.constructed_units()[&id];
        assert_eq!(unit.position().unwrap().y, 2);
        assert_eq!(unit.posture(), BattlePosture::Prone);
        assert_eq!(unit.is_destroyed(), fatal);
        assert_eq!(battle_unit_elevation(&candidate, id).unwrap(), Some(-2));
        assert_eq!(
            candidate.objects[&pilot].location,
            Some(if fatal { afterlife } else { id })
        );
        candidate.validate(&config).unwrap();
        persistence::save(&config.database(), &candidate)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            candidate.btech
        );
    }
}

#[tokio::test]
async fn woodland_clearing_on_occupied_map_is_durable_and_rejects_stale_results() {
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::HeavyForest, 2, 3).await;
    persistence::save(&config.database(), &world).await.unwrap();
    let coordinate = BattleHexCoordinate { x: 1, y: 1 };
    let original = world.clone();
    let before = world.btech.maps()[&map].hex(1, 1).unwrap();
    assert!(
        apply_woodland_clearing(
            &mut world,
            map,
            coordinate,
            before,
            BattleWoodlandClearing::CutToClear
        )
        .is_err()
    );
    assert!(
        apply_woodland_clearing(
            &mut world,
            map,
            BattleHexCoordinate { x: -1, y: 1 },
            before,
            BattleWoodlandClearing::ThinToLight
        )
        .is_err()
    );
    assert_eq!(world.btech, original.btech);
    let report = apply_woodland_clearing(
        &mut world,
        map,
        coordinate,
        before,
        BattleWoodlandClearing::ThinToLight,
    )
    .unwrap();
    assert_eq!(report.before, before);
    assert_eq!(report.after, BattleHex::new(Terrain::LightForest, 2));
    assert!(
        apply_woodland_clearing(
            &mut world,
            map,
            coordinate,
            before,
            BattleWoodlandClearing::ThinToLight
        )
        .is_err()
    );
    for id in units {
        assert_eq!(
            world.btech.constructed_units()[&id],
            original.btech.constructed_units()[&id]
        );
    }
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    let report = apply_woodland_clearing(
        &mut world,
        map,
        coordinate,
        report.after,
        BattleWoodlandClearing::CutToRough,
    )
    .unwrap();
    assert_eq!(report.after.level(), 2);
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    let unchanged = world.btech.clone();
    assert!(
        apply_woodland_clearing(
            &mut world,
            map,
            coordinate,
            report.after,
            BattleWoodlandClearing::CutToClear
        )
        .is_err()
    );
    assert_eq!(world.btech, unchanged);
    // Two reductions in a single saved transaction are also valid with occupants present.
    let coordinate = BattleHexCoordinate { x: 0, y: 0 };
    let first = apply_woodland_clearing(
        &mut world,
        map,
        coordinate,
        before,
        BattleWoodlandClearing::ThinToLight,
    )
    .unwrap();
    let _second = apply_woodland_clearing(
        &mut world,
        map,
        coordinate,
        first.after,
        BattleWoodlandClearing::CutToClear,
    )
    .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    assert_eq!(original.btech.maps()[&map].hex(1, 1).unwrap(), before);
}

#[tokio::test]
async fn map_decorations_preserve_base_terrain_checkpoints_and_saved_state() {
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::HeavyForest, 2, 3).await;
    persistence::save(&config.database(), &world).await.unwrap();
    let original = world.clone();
    let coordinate = BattleHexCoordinate { x: 1, y: 1 };
    let fire = BattleDecoration::new(BattleDecorationKind::Fire, 120, Some(60));
    set_map_decoration(&mut world, map, coordinate, Some(fire)).unwrap();
    assert_eq!(
        world.btech.maps()[&map].hex(1, 1).unwrap(),
        BattleHex::new(Terrain::HeavyForest, 2).with_overlay(Some(BattleDecorationKind::Fire))
    );
    assert_eq!(
        world.btech.maps()[&map].base_hex(1, 1).unwrap().terrain(),
        Terrain::HeavyForest
    );
    assert_eq!(
        world.btech.maps()[&map].decoration(coordinate).unwrap(),
        Some(fire)
    );
    assert_eq!(
        original.btech.maps()[&map].hex(1, 1).unwrap().terrain(),
        Terrain::HeavyForest
    );
    for id in units {
        assert_eq!(
            world.btech.constructed_units()[&id],
            original.btech.constructed_units()[&id]
        );
    }
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    let before = world.btech.clone();
    assert!(
        set_map_decoration(
            &mut world,
            map,
            coordinate,
            Some(BattleDecoration {
                remaining: 0,
                ..fire
            })
        )
        .is_err()
    );
    assert!(
        set_map_decoration(
            &mut world,
            map,
            BattleHexCoordinate { x: 3, y: 1 },
            Some(fire)
        )
        .is_err()
    );
    assert_eq!(world.btech, before);
    let smoke = BattleDecoration::new(BattleDecorationKind::Smoke, 90, None);
    set_map_decoration(&mut world, map, coordinate, Some(smoke)).unwrap();
    assert_eq!(
        world.btech.maps()[&map].hex(1, 1).unwrap().terrain(),
        Terrain::Smoke
    );
    assert_eq!(
        world.btech.maps()[&map].base_hex(1, 1).unwrap().terrain(),
        Terrain::HeavyForest
    );
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    set_map_decoration(&mut world, map, coordinate, None).unwrap();
    assert_eq!(
        world.btech.maps()[&map].hex(1, 1).unwrap(),
        original.btech.maps()[&map].hex(1, 1).unwrap()
    );
    assert_eq!(
        world.btech.constructed_units(),
        original.btech.constructed_units()
    );
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

#[tokio::test]
async fn smoke_expiration_restores_terrain_and_resumes_only_saved_seconds() {
    let (_dir, config, mut world, map, _units) = fixture_field(Terrain::HeavyForest, 2, 3).await;
    let coordinate = BattleHexCoordinate { x: 1, y: 1 };
    let fire_coordinate = BattleHexCoordinate { x: 0, y: 0 };
    let smoke = BattleDecoration::new(BattleDecorationKind::Smoke, 3, None);
    let fire = BattleDecoration::new(BattleDecorationKind::Fire, 60, Some(60));
    set_map_decoration(&mut world, map, coordinate, Some(smoke)).unwrap();
    set_map_decoration(&mut world, map, fire_coordinate, Some(fire)).unwrap();
    let initial = world.clone();
    assert!(map_smoke_pending(&world));
    advance_map_smoke(&mut world);
    assert_eq!(
        world.btech.maps()[&map]
            .decoration(coordinate)
            .unwrap()
            .unwrap()
            .remaining,
        2
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let saved = world.clone();
    advance_map_smoke(&mut world);
    world = persistence::load(&config.database()).await.unwrap();
    assert_eq!(world.btech, saved.btech);
    advance_map_smoke(&mut world);
    assert!(map_smoke_pending(&world));
    assert_eq!(
        world.btech.maps()[&map].hex(1, 1).unwrap().terrain(),
        Terrain::Smoke
    );
    advance_map_smoke(&mut world);
    assert!(!map_smoke_pending(&world));
    assert_eq!(
        world.btech.maps()[&map].hex(1, 1).unwrap(),
        BattleHex::new(Terrain::HeavyForest, 2)
    );
    assert_eq!(
        world.btech.maps()[&map]
            .decoration(fire_coordinate)
            .unwrap(),
        Some(fire)
    );
    let finished = world.btech.clone();
    advance_map_smoke(&mut world);
    assert_eq!(world.btech, finished);
    assert_eq!(
        initial.btech.maps()[&map].decoration(coordinate).unwrap(),
        Some(smoke)
    );
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

#[tokio::test]
async fn calm_fire_spreads_smoke_then_burns_out_with_saved_replay() {
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::HeavyForest, 2, 3).await;
    let coordinate = BattleHexCoordinate { x: 1, y: 1 };
    set_map_decoration(
        &mut world,
        map,
        coordinate,
        Some(BattleDecoration::new(BattleDecorationKind::Fire, 60, None)),
    )
    .unwrap();
    let original = world.clone();
    for _ in 0..30 {
        advance_map_fire(&mut world).unwrap();
    }
    let effect = world.btech.maps()[&map]
        .decoration(coordinate)
        .unwrap()
        .unwrap();
    assert_eq!((effect.remaining, effect.next_spread), (60, Some(30)));
    persistence::save(&config.database(), &world).await.unwrap();
    let mut replay = persistence::load(&config.database()).await.unwrap();
    for _ in 0..30 {
        advance_map_smoke(&mut world);
        advance_map_fire(&mut world).unwrap();
        advance_map_smoke(&mut replay);
        advance_map_fire(&mut replay).unwrap();
        assert_eq!(world.btech, replay.btech);
    }
    let pending = world.btech.maps()[&map]
        .decoration(coordinate)
        .unwrap()
        .unwrap();
    assert_eq!((pending.remaining, pending.next_spread), (1, None));
    for candidate in [&mut world, &mut replay] {
        advance_map_smoke(candidate);
        advance_map_fire(candidate).unwrap();
    }
    assert_eq!(world.btech, replay.btech);
    assert!(!map_fire_pending(&world));
    assert!(map_smoke_pending(&world));
    // Heavy woods thin to light woods when the fire burns out.
    let burnt = world.btech.maps()[&map].hex(1, 1).unwrap();
    assert_eq!(burnt, BattleHex::new(Terrain::LightForest, 2));
    for x in 0..3 {
        let smoke = world.btech.maps()[&map]
            .decoration(BattleHexCoordinate { x, y: 0 })
            .unwrap()
            .unwrap();
        assert_eq!(smoke.kind, BattleDecorationKind::Smoke);
        assert!((89..=149).contains(&smoke.remaining));
        assert_eq!(smoke.next_spread, None);
    }
    for id in units {
        assert_eq!(
            world.btech.constructed_units()[&id],
            original.btech.constructed_units()[&id]
        );
    }
    assert_eq!(
        original.btech.maps()[&map].hex(1, 1).unwrap().terrain(),
        Terrain::Fire
    );
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    for _ in 0..150 {
        advance_map_smoke(&mut world);
    }
    assert!(!map_smoke_pending(&world));
    assert_eq!(
        world.btech.maps()[&map].hex(1, 0).unwrap().terrain(),
        Terrain::HeavyForest
    );
}

#[tokio::test]
async fn strong_wind_fire_replays_new_ignition_and_retains_scheduled_delay() {
    let (_dir, config, mut world, map, _units) = fixture_field(Terrain::LightForest, 1, 5).await;
    set_map_wind(&mut world, map, 0, 0).unwrap();
    let coordinate = BattleHexCoordinate { x: 1, y: 3 };
    set_map_decoration(
        &mut world,
        map,
        coordinate,
        Some(BattleDecoration::new(BattleDecorationKind::Fire, 120, None)),
    )
    .unwrap();
    // Select a reproducible map stream whose first spread roll ignites the forward cell.
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() >= 9)
        .unwrap();
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["maps"][map.0.to_string()]["fire_dice"] =
        serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    world.btech = serde_json::from_value(encoded).unwrap();
    for _ in 0..10 {
        advance_map_fire(&mut world).unwrap();
    }
    // Wind changes affect subsequent scheduling, not a check already counting down.
    set_map_wind(&mut world, map, 0, 60).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut replay = persistence::load(&config.database()).await.unwrap();
    for _ in 0..50 {
        advance_map_fire(&mut world).unwrap();
        advance_map_fire(&mut replay).unwrap();
    }
    assert_eq!(world.btech, replay.btech);
    let original = world.btech.maps()[&map]
        .decoration(coordinate)
        .unwrap()
        .unwrap();
    assert_eq!((original.remaining, original.next_spread), (100, Some(20)));
    let forward = world.btech.maps()[&map]
        .decoration(BattleHexCoordinate { x: 1, y: 2 })
        .unwrap()
        .unwrap();
    assert_eq!(forward.kind, BattleDecorationKind::Fire);
    assert!((60..=180).contains(&forward.remaining));
    assert_eq!(forward.next_spread, Some(20));
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

#[tokio::test]
async fn smoke_over_water_preserves_altitude_cooling_los_range_and_flooding() {
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::Water, 3, 3).await;
    let _shutdown = stop_battle_unit(&mut world, units[1], ObjectId(2), rules()).unwrap();
    place_battle_unit(&mut world, units[1], map, 1, 0).unwrap();
    let id = units[0];
    let range = battle_unit_range(&world, id, units[1]).unwrap();
    let los = battle_unit_terrain_los(&world, id, units[1]).unwrap();
    let cooling = world.btech.constructed_units()[&id].heat_rates(&world);
    assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(-3));
    let mut clear = world.clone();
    set_map_decoration(
        &mut world,
        map,
        BattleHexCoordinate { x: 1, y: 1 },
        Some(BattleDecoration::new(
            BattleDecorationKind::Smoke,
            120,
            None,
        )),
    )
    .unwrap();
    assert_eq!(
        world.btech.maps()[&map].hex(1, 1).unwrap().terrain(),
        Terrain::Smoke
    );
    assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(-3));
    assert_eq!(battle_unit_range(&world, id, units[1]).unwrap(), range);
    assert_eq!(battle_unit_terrain_los(&world, id, units[1]).unwrap(), los);
    assert_eq!(
        world.btech.constructed_units()[&id].heat_rates(&world),
        cooling
    );
    let leg = BattleSection::LeftLeg;
    let armor = world.btech.constructed_units()[&id].sections()[&leg].armor;
    for candidate in [&mut world, &mut clear] {
        let _damage = apply_damage_phase(
            candidate,
            id,
            leg,
            armor,
            BattleDamagePhase::Armor { rear: false },
        )
        .unwrap();
    }
    let flooded = flood_battle_unit(&mut world, id, rules()).unwrap();
    let expected = flood_battle_unit(&mut clear, id, rules()).unwrap();
    assert_eq!(flooded, expected);
    assert!(flooded.iter().any(|report| report.section == leg));
    // Flooding a standing leg causes a fall, whose critical damage may destroy that leg.
    let unit = &world.btech.constructed_units()[&id];
    assert!(unit.flooded_sections().contains(&leg) || unit.sections()[&leg].internal == 0);
    assert_eq!(
        world.btech.constructed_units(),
        clear.btech.constructed_units()
    );
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

#[tokio::test]
async fn smoke_keeps_ice_surface_height_and_bridge_fracture_available() {
    for (terrain, depth) in [(Terrain::Ice, 3), (Terrain::Bridge, 2)] {
        let (_dir, config, mut world, map, units) = fixture_field(terrain, depth, 3).await;
        let coordinate = BattleHexCoordinate { x: 1, y: 1 };
        let elevation = battle_unit_elevation(&world, units[0]).unwrap();
        set_map_decoration(
            &mut world,
            map,
            coordinate,
            Some(BattleDecoration::new(
                BattleDecorationKind::Smoke,
                120,
                None,
            )),
        )
        .unwrap();
        assert_eq!(battle_unit_elevation(&world, units[0]).unwrap(), elevation);
        let report = if terrain == Terrain::Ice {
            break_battle_ice(&mut world, map, coordinate, None, rules()).unwrap()
        } else {
            break_battle_bridge(&mut world, map, coordinate, rules()).unwrap()
        };
        assert_eq!(report.before.terrain(), terrain);
        assert_eq!(report.after.terrain(), Terrain::Water);
        assert_eq!(
            world.btech.maps()[&map].base_hex(1, 1).unwrap(),
            report.after
        );
        assert_eq!(
            world.btech.maps()[&map].hex(1, 1).unwrap().terrain(),
            Terrain::Smoke
        );
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

#[tokio::test]
async fn entering_smoke_covered_water_keeps_movement_and_immersion_checks() {
    let source = "3 4\n.0.0.0\n.0.0.0\n~1~1~1\n~1~1~1\n";
    let (_dir, config, mut world, map, units) =
        fixture_asset(BattleMapAsset::from_cells(source).unwrap()).await;
    let id = units[0];
    let leg = BattleSection::LeftLeg;
    let armor = world.btech.constructed_units()[&id].sections()[&leg].armor;
    let _damage = apply_damage_phase(
        &mut world,
        id,
        leg,
        armor,
        BattleDamagePhase::Armor { rear: false },
    )
    .unwrap();
    prepare_reverse_step(&mut world, id, 12);
    let mut clear = world.clone();
    for y in 2..4 {
        for x in 0..3 {
            set_map_decoration(
                &mut world,
                map,
                BattleHexCoordinate { x, y },
                Some(BattleDecoration::new(
                    BattleDecorationKind::Smoke,
                    120,
                    None,
                )),
            )
            .unwrap();
        }
    }
    let notices = advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
    let expected = advance_battle_motion(&mut clear, BattleMovementRules::STANDARD).unwrap();
    assert_eq!(notices, expected);
    assert_eq!(
        world.btech.constructed_units(),
        clear.btech.constructed_units()
    );
    assert!(
        world.btech.constructed_units()[&id]
            .flooded_sections()
            .contains(&leg)
    );
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn woodland_impacts_commit_dice_terrain_and_notices_with_restart_replay() {
    for intent in [
        BattleWoodlandIntent::Ignite,
        BattleWoodlandIntent::Clear,
        BattleWoodlandIntent::Incidental,
    ] {
        let (_dir, config, mut world, map, units) = fixture_field(Terrain::HeavyForest, 2, 3).await;
        let shooter = units[0];
        let seed = (0..=255)
            .find(|seed| {
                let roll = BattleDice::seeded([*seed; 32]).two_d6();
                if intent == BattleWoodlandIntent::Ignite {
                    roll >= 4
                } else {
                    roll > 5
                }
            })
            .unwrap();
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        encoded["constructed"][shooter.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        world.btech = serde_json::from_value(encoded).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        let original = world.clone();
        let request = BattleWoodlandAttack {
            shooter,
            coordinate: BattleHexCoordinate { x: 1, y: 0 },
            weapon: if intent == BattleWoodlandIntent::Ignite {
                BattleWeapon::Flamer
            } else {
                BattleWeapon::GaussRifle
            },
            ammunition: BattleAmmunitionMode::Normal,
            damage: 15,
            intent,
        };
        if intent == BattleWoodlandIntent::Ignite {
            let mut incomplete = original.clone();
            let mut encoded = serde_json::to_value(&incomplete.btech).unwrap();
            encoded["maps"][map.0.to_string()]["fire_dice"] = serde_json::Value::Null;
            incomplete.btech = serde_json::from_value(encoded).unwrap();
            let checkpoint = incomplete.btech.clone();
            assert!(resolve_woodland_attack(&mut incomplete, request).is_err());
            assert_eq!(incomplete.btech, checkpoint);
        }
        let report = resolve_woodland_attack(&mut world, request).unwrap();
        assert_eq!(
            resolve_woodland_attack(&mut replay, request).unwrap(),
            report
        );
        assert_eq!(world.btech, replay.btech);
        assert_eq!(report.map, map);
        let expected = if intent == BattleWoodlandIntent::Ignite {
            Terrain::Fire
        } else {
            Terrain::LightForest
        };
        assert_eq!(
            world.btech.maps()[&map].hex(1, 0).unwrap().terrain(),
            expected
        );
        let text = match intent {
            BattleWoodlandIntent::Ignite => "You ignite 1,0.",
            BattleWoodlandIntent::Clear => "You clear 1,0.",
            BattleWoodlandIntent::Incidental => "You accidentally clear 1,0!",
        };
        assert_eq!(report.notices[0].text, text);
        let mut expected_unit =
            serde_json::to_value(&original.btech.constructed_units()[&shooter]).unwrap();
        let actual_unit = serde_json::to_value(&world.btech.constructed_units()[&shooter]).unwrap();
        expected_unit["dice"] = actual_unit["dice"].clone();
        assert_eq!(expected_unit, actual_unit);
        assert_eq!(
            original.btech.maps()[&map].hex(1, 0).unwrap().terrain(),
            Terrain::HeavyForest
        );
        let before_invalid = world.btech.clone();
        assert!(
            resolve_woodland_attack(
                &mut world,
                BattleWoodlandAttack {
                    coordinate: BattleHexCoordinate { x: -1, y: 0 },
                    ..request
                }
            )
            .is_err()
        );
        assert_eq!(world.btech, before_invalid);
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
        if intent == BattleWoodlandIntent::Ignite {
            for _ in 0..60 {
                advance_map_smoke(&mut world);
                advance_map_fire(&mut world).unwrap();
                advance_map_smoke(&mut replay);
                advance_map_fire(&mut replay).unwrap();
            }
            assert_eq!(world.btech, replay.btech);
        }
    }
}

#[tokio::test]
async fn building_integrity_configuration_persistence_and_rejection() {
    use sqlx::Connection;
    let (_dir, config, mut world, map, units) = fixture(1).await;
    let before = world.clone();
    let building = BattleBuildingState {
        integrity: 73,
        maximum_integrity: 120,
        flags: 31,
        regeneration: 7,
    };
    assert!(building.is_command_center() && building.is_complex() && building.is_hidden());
    assert!(building.is_invisible() && building.is_safe() && building.is_dropship());
    set_building_state(&mut world, map, building).unwrap();
    assert_eq!(
        world.btech.constructed_units(),
        before.btech.constructed_units()
    );
    assert_eq!(
        world.btech.maps()[&map].base_hex(1, 1).unwrap(),
        before.btech.maps()[&map].base_hex(1, 1).unwrap()
    );
    let configured = world.clone();
    for invalid in [
        BattleBuildingState {
            integrity: -1,
            ..building
        },
        BattleBuildingState {
            integrity: 121,
            ..building
        },
        BattleBuildingState {
            maximum_integrity: 32768,
            ..building
        },
        BattleBuildingState {
            flags: 256,
            ..building
        },
        BattleBuildingState {
            regeneration: i64::MAX,
            ..building
        },
    ] {
        assert!(set_building_state(&mut world, map, invalid).is_err());
        assert_eq!(world.btech, configured.btech);
    }
    assert!(set_building_state(&mut world, ObjectId(-1), building).is_err());
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    let mut sql = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TRIGGER reject_building_update BEFORE UPDATE OF cf ON btech_maps BEGIN SELECT RAISE(ABORT,'building failure'); END").execute(&mut sql).await.unwrap();
    set_building_state(
        &mut world,
        map,
        BattleBuildingState {
            integrity: 0,
            ..building
        },
    )
    .unwrap();
    assert!(persistence::save(&config.database(), &world).await.is_err());
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        configured.btech
    );
    sqlx::query("DROP TRIGGER reject_building_update")
        .execute(&mut sql)
        .await
        .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    for id in units {
        let pilot = world.btech.constructed_units()[&id].pilot().unwrap();
        stop_battle_unit(&mut world, id, pilot, BattleMovementRules::STANDARD.fall).unwrap();
        remove_battle_unit(&mut world, id, ObjectId(config.home())).unwrap();
    }
    reload_battle_map(
        &mut world,
        map,
        "interior.map",
        BattleMapAsset::from_cells("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    assert_eq!(
        world.btech.maps()[&map].building,
        BattleBuildingState {
            integrity: 0,
            ..building
        }
    );
    persistence::save(&config.database(), &world).await.unwrap();
    sqlx::query("UPDATE btech_maps SET cf=cf_max+1 WHERE dbref=?")
        .bind(map.0)
        .execute(&mut sql)
        .await
        .unwrap();
    assert!(persistence::load(&config.database()).await.is_err());
}

#[tokio::test]
async fn building_entrances_preserve_order_identity_and_unowned_data() {
    use sqlx::Connection;
    let (_dir, config, mut world, map, _units) = fixture(1).await;
    let interior = world.create(&config, "Interior".into(), Kind::Room);
    create_battle_map(
        &mut world,
        interior,
        "inside.map",
        BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let point = BattleHexCoordinate { x: 1, y: 1 };
    let entrance = BattleBuildingEntrance {
        coordinate: point,
        interior,
        data_char: 0,
        data_short: 0,
        data_int: 0,
    };
    let later = BattleBuildingEntrance {
        coordinate: point,
        interior: map,
        data_char: 0,
        data_short: 0,
        data_int: 0,
    };
    set_building_entrance(&mut world, map, 9, Some(later)).unwrap();
    set_building_entrance(&mut world, map, 2, Some(entrance)).unwrap();
    assert_eq!(
        world.btech.maps()[&map].building_at(point).unwrap(),
        Some(entrance)
    );
    assert_eq!(
        world.btech.maps()[&map]
            .building_entrances()
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        vec![2, 9]
    );
    let before = world.clone();
    for invalid in [
        BattleBuildingEntrance {
            coordinate: BattleHexCoordinate { x: -1, y: 1 },
            ..entrance
        },
        BattleBuildingEntrance {
            interior: ObjectId(-1),
            ..entrance
        },
    ] {
        assert!(set_building_entrance(&mut world, map, 2, Some(invalid)).is_err());
        assert_eq!(world.btech, before.btech);
    }
    persistence::save(&config.database(), &world).await.unwrap();
    let mut sql = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE btech_map_objects SET data_char=87,data_short=23,data_int=91 WHERE map_dbref=? AND object_type=4 AND ordinal=2").bind(map.0).execute(&mut sql).await.unwrap();
    sqlx::query("INSERT INTO btech_map_objects VALUES(?,1,0,0,0,-1,0,12,13)")
        .bind(map.0)
        .execute(&mut sql)
        .await
        .unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    let mut expected = before.clone();
    let retained_entrance = BattleBuildingEntrance {
        data_char: 87,
        data_short: 23,
        data_int: 91,
        ..entrance
    };
    set_building_entrance(&mut expected, map, 2, Some(retained_entrance)).unwrap();
    set_battle_static_decoration(
        &mut expected,
        map,
        BattleStaticDecorationKind::Smoke,
        0,
        Some(BattleStaticDecoration {
            coordinate: BattleHexCoordinate { x: 0, y: 0 },
            restored_terrain: None,
            object: ObjectId(-1),
            duration: 12,
            scalar: 13,
        }),
    )
    .unwrap();
    assert_eq!(world.btech, expected.btech);
    set_building_entrance(
        &mut world,
        map,
        2,
        Some(BattleBuildingEntrance {
            coordinate: BattleHexCoordinate { x: 0, y: 0 },
            ..retained_entrance
        }),
    )
    .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let data: (i64,i64,i64) = sqlx::query_as("SELECT data_char,data_short,data_int FROM btech_map_objects WHERE map_dbref=? AND object_type=4 AND ordinal=2").bind(map.0).fetch_one(&mut sql).await.unwrap();
    assert_eq!(data, (87, 23, 91));
    let other: i64 = sqlx::query_scalar(
        "SELECT data_int FROM btech_map_objects WHERE map_dbref=? AND object_type=1",
    )
    .bind(map.0)
    .fetch_one(&mut sql)
    .await
    .unwrap();
    assert_eq!(other, 13);
    assert_eq!(
        world.btech.maps()[&map].building_at(point).unwrap(),
        Some(later)
    );
    let saved = world.clone();
    sqlx::query("CREATE TRIGGER reject_entrance_delete BEFORE DELETE ON btech_map_objects WHEN OLD.object_type=4 BEGIN SELECT RAISE(ABORT,'entrance failure'); END").execute(&mut sql).await.unwrap();
    set_building_entrance(&mut world, map, 9, None).unwrap();
    assert!(persistence::save(&config.database(), &world).await.is_err());
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
    sqlx::query("DROP TRIGGER reject_entrance_delete")
        .execute(&mut sql)
        .await
        .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    assert!(
        world.btech.maps()[&map]
            .building_at(point)
            .unwrap()
            .is_none()
    );
    sqlx::query("UPDATE btech_map_objects SET object_dbref=-1 WHERE map_dbref=? AND object_type=4")
        .bind(map.0)
        .execute(&mut sql)
        .await
        .unwrap();
    assert!(persistence::load(&config.database()).await.is_err());
}

#[tokio::test]
async fn minefields_persist_all_kinds_and_survive_woodland_clearing() {
    use sqlx::Connection;
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::HeavyForest, 0, 3).await;
    let coordinate = BattleHexCoordinate { x: 1, y: 1 };
    for (ordinal, kind) in [
        BattleMineKind::Standard,
        BattleMineKind::Inferno,
        BattleMineKind::Command,
        BattleMineKind::Vibra,
        BattleMineKind::Trigger,
    ]
    .into_iter()
    .enumerate()
    {
        set_minefield(
            &mut world,
            map,
            ordinal as u32,
            Some(BattleMinefield {
                coordinate,
                kind,
                strength: 10 + ordinal as i16,
                extra: 20 + ordinal as i32,
                owner: ObjectId(1),
            }),
        )
        .unwrap();
    }
    let checkpoint = world.clone();
    let mine = world.btech.maps()[&map].minefields()[&0];
    for invalid in [
        BattleMinefield {
            coordinate: BattleHexCoordinate { x: 3, y: 1 },
            ..mine
        },
        BattleMinefield {
            owner: ObjectId(-2),
            ..mine
        },
    ] {
        assert!(set_minefield(&mut world, map, 0, Some(invalid)).is_err());
        assert_eq!(world.btech, checkpoint.btech);
    }
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    let mut cleared = None;
    for seed in 0..=255 {
        let mut candidate = world.clone();
        let mut encoded = serde_json::to_value(&candidate.btech).unwrap();
        encoded["constructed"][units[0].0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        candidate.btech = serde_json::from_value(encoded).unwrap();
        let report = resolve_woodland_attack(
            &mut candidate,
            BattleWoodlandAttack {
                shooter: units[0],
                coordinate,
                weapon: BattleWeapon::MediumLaser,
                ammunition: BattleAmmunitionMode::Normal,
                damage: 12,
                intent: BattleWoodlandIntent::Clear,
            },
        )
        .unwrap();
        if matches!(report.effect, BattleWoodlandEffect::Clear { .. }) {
            cleared = Some(candidate);
            break;
        }
    }
    world = cleared.unwrap();
    assert_eq!(
        world.btech.maps()[&map].base_hex(1, 1).unwrap().terrain(),
        Terrain::LightForest
    );
    assert_eq!(
        world.btech.maps()[&map].minefields(),
        checkpoint.btech.maps()[&map].minefields()
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let mut sql = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query("ALTER TABLE btech_map_objects ADD COLUMN mine_note TEXT NOT NULL DEFAULT 'keep'")
        .execute(&mut sql)
        .await
        .unwrap();
    sqlx::query("INSERT INTO btech_map_objects (map_dbref,object_type,ordinal,x,y,object_dbref,data_char,data_short,data_int) VALUES(?,1,0,0,0,-1,0,12,13)").bind(map.0).execute(&mut sql).await.unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    set_minefield(
        &mut world,
        map,
        0,
        Some(BattleMinefield {
            strength: 21,
            ..mine
        }),
    )
    .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let note: String = sqlx::query_scalar(
        "SELECT mine_note FROM btech_map_objects WHERE map_dbref=? AND object_type=3 AND ordinal=0",
    )
    .bind(map.0)
    .fetch_one(&mut sql)
    .await
    .unwrap();
    assert_eq!(note, "keep");
    let unrelated: i64 = sqlx::query_scalar(
        "SELECT data_int FROM btech_map_objects WHERE map_dbref=? AND object_type=1",
    )
    .bind(map.0)
    .fetch_one(&mut sql)
    .await
    .unwrap();
    assert_eq!(unrelated, 13);
    let saved = world.clone();
    set_minefield(&mut world, map, 0, None).unwrap();
    sqlx::query("CREATE TRIGGER reject_mine_delete BEFORE DELETE ON btech_map_objects WHEN OLD.object_type=3 BEGIN SELECT RAISE(ABORT,'mine failure'); END").execute(&mut sql).await.unwrap();
    assert!(persistence::save(&config.database(), &world).await.is_err());
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
    sqlx::query("DROP TRIGGER reject_mine_delete")
        .execute(&mut sql)
        .await
        .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    sqlx::query("UPDATE btech_map_objects SET data_char=99 WHERE map_dbref=? AND object_type=3 AND ordinal=1").bind(map.0).execute(&mut sql).await.unwrap();
    assert!(persistence::load(&config.database()).await.is_err());
}

#[tokio::test]
async fn mine_activation_queries_preserve_state_and_saved_order() {
    let (_dir, config, mut world, map, units) = fixture(3).await;
    let unit = units[0];
    let coordinate = BattleHexCoordinate { x: 1, y: 1 };
    let tons = (world.btech.constructed_units()[&unit].mass().unwrap().total / 1024) as i16;
    let field = BattleMinefield {
        coordinate,
        kind: BattleMineKind::Standard,
        strength: tons,
        extra: 0,
        owner: ObjectId(1),
    };
    set_minefield(&mut world, map, 9, Some(field)).unwrap();
    set_minefield(
        &mut world,
        map,
        2,
        Some(BattleMinefield {
            kind: BattleMineKind::Trigger,
            extra: -1,
            ..field
        }),
    )
    .unwrap();
    set_minefield(
        &mut world,
        map,
        5,
        Some(BattleMinefield {
            kind: BattleMineKind::Command,
            extra: 42,
            ..field
        }),
    )
    .unwrap();
    // The standard field supplies map coverage; a colocated trigger still uses its weight gate.
    let before = world.btech.clone();
    let selected = mine_activations(&world, unit, BattleMineTriggerReason::Step).unwrap();
    assert_eq!(
        selected
            .iter()
            .map(|event| (event.ordinal, event.response))
            .collect::<Vec<_>>(),
        vec![
            (2, BattleMineResponse::Trigger),
            (5, BattleMineResponse::Spotted),
            (9, BattleMineResponse::Explode)
        ]
    );
    assert_eq!(
        mine_activations(&world, unit, BattleMineTriggerReason::Fall)
            .unwrap()
            .iter()
            .map(|event| event.ordinal)
            .collect::<Vec<_>>(),
        vec![5, 9]
    );
    assert_eq!(world.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        mine_activations(&loaded, unit, BattleMineTriggerReason::Step).unwrap(),
        selected
    );
    set_map_decoration(
        &mut world,
        map,
        coordinate,
        Some(BattleDecoration::new(BattleDecorationKind::Smoke, 30, None)),
    )
    .unwrap();
    assert_eq!(
        mine_activations(&world, unit, BattleMineTriggerReason::Step).unwrap(),
        selected
    );
    set_minefield(
        &mut world,
        map,
        2,
        Some(BattleMinefield {
            kind: BattleMineKind::Trigger,
            strength: tons + 1,
            ..field
        }),
    )
    .unwrap();
    assert_eq!(
        mine_activations(&world, unit, BattleMineTriggerReason::Step)
            .unwrap()
            .iter()
            .map(|event| event.ordinal)
            .collect::<Vec<_>>(),
        vec![5, 9]
    );
    let mut elevated = world.clone();
    let mut encoded = serde_json::to_value(&elevated.btech).unwrap();
    encoded["constructed"][unit.0.to_string()]["ground_elevation"] = 1.into();
    elevated.btech = serde_json::from_value(encoded).unwrap();
    assert!(
        mine_activations(&elevated, unit, BattleMineTriggerReason::Land)
            .unwrap()
            .is_empty()
    );
    assert!(
        world.btech.maps()[&map]
            .mine_coverage(BattleHexCoordinate { x: -1, y: 1 })
            .is_err()
    );
    assert!(mine_activations(&world, ObjectId(-1), BattleMineTriggerReason::Step).is_err());
}

#[tokio::test]
async fn conventional_mine_blasts_packets_neighbors_removal_and_restart() {
    for (kind, strength) in [
        (BattleMineKind::Standard, 4),
        (BattleMineKind::Standard, 6),
        (BattleMineKind::Command, 6),
        (BattleMineKind::Vibra, 6),
    ] {
        let (_dir, config, mut world, map, units) = fixture_field(Terrain::HeavyForest, 0, 3).await;
        let coordinate = BattleHexCoordinate { x: 1, y: 1 };
        let neighbor = BattleHexCoordinate { x: 1, y: 0 };
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        encoded["constructed"][units[1].0.to_string()]["position"]["y"] = 0.into();
        encoded["constructed"][units[1].0.to_string()]["motion"]["point"] =
            serde_json::to_value(neighbor.center()).unwrap();
        world.btech = serde_json::from_value(encoded).unwrap();
        let mine = BattleMinefield {
            coordinate,
            kind,
            strength,
            extra: 0,
            owner: ObjectId(1),
        };
        set_minefield(&mut world, map, 0, Some(mine)).unwrap();
        set_minefield(
            &mut world,
            map,
            1,
            Some(BattleMinefield {
                kind: BattleMineKind::Trigger,
                ..mine
            }),
        )
        .unwrap();
        let before = world.clone();
        let report = resolve_mine_blast(&mut world, map, 0, rules()).unwrap();
        assert_eq!(report.hits[0].unit, units[0]);
        assert_eq!(report.hits[0].damage, strength as u16);
        assert_eq!(
            report.hits[0].impacts.len(),
            (strength as usize).div_ceil(5)
        );
        let area = kind != BattleMineKind::Standard;
        assert_eq!(report.hits.len(), if area { 2 } else { 1 });
        if area {
            assert_eq!((report.hits[1].unit, report.hits[1].damage), (units[1], 3));
        } else {
            assert_eq!(
                world.btech.constructed_units()[&units[1]],
                before.btech.constructed_units()[&units[1]]
            );
        }
        assert_eq!(report.ignited.len(), if area { 6 } else { 0 });
        assert_eq!(
            report.removed,
            if area || strength < 5 {
                vec![0, 1]
            } else {
                vec![]
            }
        );
        if !area && strength >= 5 {
            assert_eq!(world.btech.maps()[&map].minefields()[&0], mine);
        }
        let mut replay = before.clone();
        assert_eq!(
            resolve_mine_blast(&mut replay, map, 0, rules()).unwrap(),
            report
        );
        assert_eq!(replay.btech, world.btech);
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
        if area {
            let mut invalid = before;
            let mut encoded = serde_json::to_value(&invalid.btech).unwrap();
            encoded["maps"][map.0.to_string()]["fire_dice"] = serde_json::Value::Null;
            invalid.btech = serde_json::from_value(encoded).unwrap();
            let checkpoint = invalid.clone();
            assert!(resolve_mine_blast(&mut invalid, map, 0, rules()).is_err());
            assert_eq!(invalid.btech, checkpoint.btech);
        }
    }
}

#[tokio::test]
async fn mine_blast_character_action_and_late_rejection_are_atomic() {
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::Grassland, 0, 3).await;
    set_minefield(
        &mut world,
        map,
        0,
        Some(BattleMinefield {
            coordinate: BattleHexCoordinate { x: 1, y: 1 },
            kind: BattleMineKind::Standard,
            strength: 4,
            extra: 0,
            owner: ObjectId(1),
        }),
    )
    .unwrap();
    world
        .objects
        .get_mut(&units[1])
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let checkpoint = world.clone();
    assert!(resolve_mine_blast(&mut world, map, 0, rules()).is_err());
    assert_eq!(world.btech, checkpoint.btech);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let report = resolve_mine_blast_action(&scripts, &config, map, 0, rules()).unwrap();
    assert_eq!(report.hits.len(), 2);
    assert_eq!(report.removed, vec![0]);
    assert!(
        scripts
            .drain_outbox()
            .iter()
            .any(|(_, message)| message.source().contains("blast of shrapnel"))
    );
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
}

#[tokio::test]
async fn mine_activation_water_uses_bottom_depth_and_blast_height_bounds() {
    let (_dir, _config, mut world, map, units) = fixture_field(Terrain::Water, 3, 3).await;
    set_minefield(
        &mut world,
        map,
        0,
        Some(BattleMinefield {
            coordinate: BattleHexCoordinate { x: 1, y: 1 },
            kind: BattleMineKind::Standard,
            strength: 4,
            extra: 0,
            owner: ObjectId(1),
        }),
    )
    .unwrap();
    assert!(
        !mine_activations(&world, units[0], BattleMineTriggerReason::Step)
            .unwrap()
            .is_empty()
    );
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["constructed"][units[0].0.to_string()]["ground_elevation"] = (-2).into();
    encoded["constructed"][units[1].0.to_string()]["ground_elevation"] = (-1).into();
    world.btech = serde_json::from_value(encoded).unwrap();
    assert!(
        mine_activations(&world, units[0], BattleMineTriggerReason::Step)
            .unwrap()
            .is_empty()
    );
    let report = resolve_mine_blast(&mut world, map, 0, rules()).unwrap();
    assert_eq!(
        report.hits.iter().map(|hit| hit.unit).collect::<Vec<_>>(),
        vec![units[0]]
    );
}

#[tokio::test]
async fn inferno_duration_cooling_extension_and_saved_expiry() {
    let (_dir, config, mut world, _map, units) = fixture_field(Terrain::Grassland, 0, 3).await;
    let id = units[0];
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["constructed"][units[1].0.to_string()]["position"]["y"] = 0.into();
    encoded["constructed"][units[1].0.to_string()]["motion"]["point"] =
        serde_json::to_value(BattleHexCoordinate { x: 1, y: 0 }.center()).unwrap();
    world.btech = serde_json::from_value(encoded).unwrap();
    let baseline = world.btech.constructed_units()[&id].heat_rates(&world);
    let heat = world.btech.constructed_units()[&id].heat();
    apply_inferno_burn(&mut world, id, 2).unwrap();
    assert!(battle_unit_illuminated(&world, id));
    assert!(battle_unit_illuminated(&world, units[1]));
    let rates = world.btech.constructed_units()[&id].heat_rates(&world);
    assert_eq!(rates.production, baseline.production);
    assert_eq!(rates.dissipation, (baseline.dissipation - 6.0).max(0.0));
    assert_eq!(world.btech.constructed_units()[&id].heat(), heat);
    assert!(advance_inferno_burns(&mut world).is_empty());
    apply_inferno_burn(&mut world, id, 2).unwrap();
    assert_eq!(world.btech.constructed_units()[&id].inferno_remaining(), 3);
    let checkpoint = world.clone();
    assert!(apply_inferno_burn(&mut world, id, 0).is_err());
    assert!(apply_inferno_burn(&mut world, id, u32::MAX).is_err());
    assert_eq!(world.btech, checkpoint.btech);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut replay = persistence::load(&config.database()).await.unwrap();
    for second in 1..=3 {
        let notices = advance_inferno_burns(&mut world);
        assert_eq!(notices, advance_inferno_burns(&mut replay));
        assert_eq!(notices.len(), usize::from(second == 3));
        assert_eq!(world.btech, replay.btech);
    }
    assert_eq!(
        world.btech.constructed_units()[&id].heat_rates(&world),
        baseline
    );
    assert!(advance_inferno_burns(&mut world).is_empty());
    assert!(!battle_unit_illuminated(&world, id));
    assert!(!battle_unit_illuminated(&world, units[1]));
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["constructed"][id.0.to_string()]["inferno_remaining"] = u32::MAX.into();
    world.btech = serde_json::from_value(encoded).unwrap();
    assert!(world.validate(&config).is_err());
}

#[tokio::test]
async fn inferno_mines_split_damage_and_burn_duration_atomically() {
    for strength in [1, 4, 6] {
        let (_dir, config, mut world, map, units) = fixture_field(Terrain::Grassland, 0, 3).await;
        set_minefield(
            &mut world,
            map,
            0,
            Some(BattleMinefield {
                coordinate: BattleHexCoordinate { x: 1, y: 1 },
                kind: BattleMineKind::Inferno,
                strength,
                extra: 0,
                owner: ObjectId(1),
            }),
        )
        .unwrap();
        let before = world.clone();
        let report = resolve_mine_blast(&mut world, map, 0, rules()).unwrap();
        assert_eq!(report.hits.len(), 2);
        for hit in &report.hits {
            assert_eq!(hit.damage, strength as u16 / 3);
            assert_eq!(hit.burn_seconds, i64::from(strength) * 6);
            assert_eq!(
                world.btech.constructed_units()[&hit.unit].inferno_remaining(),
                hit.burn_seconds as u32
            );
            assert_eq!(hit.impacts.len(), usize::from(strength >= 3));
        }
        assert_eq!(report.removed, if strength < 5 { vec![0] } else { vec![] });
        assert!(
            report
                .notices
                .iter()
                .any(|notice| notice.text.contains("flaming gel"))
        );
        let mut replay = before.clone();
        assert_eq!(
            resolve_mine_blast(&mut replay, map, 0, rules()).unwrap(),
            report
        );
        assert_eq!(replay.btech, world.btech);
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
        let mut overflow = before;
        apply_inferno_burn(&mut overflow, units[1], i32::MAX as u32).unwrap();
        let checkpoint = overflow.clone();
        assert!(resolve_mine_blast(&mut overflow, map, 0, rules()).is_err());
        assert_eq!(overflow.btech, checkpoint.btech);
    }
}

#[tokio::test]
async fn inferno_water_extinction_and_fall_publish_saved_steam() {
    for depth in [1, 2] {
        let (_dir, config, mut world, map, units) = fixture_field(Terrain::Water, depth, 3).await;
        let id = units[0];
        apply_inferno_burn(&mut world, id, 60).unwrap();
        let before = world.clone();
        let notices = extinguish_inferno_in_water(&mut world, id).unwrap();
        if depth == 1 {
            assert!(notices.is_empty());
            assert_eq!(world.btech, before.btech);
            let report = resolve_battle_fall(&mut world, id, 1, rules()).unwrap();
            assert!(
                report
                    .notices(id)
                    .iter()
                    .any(|notice| notice.text.contains("roar of steam"))
            );
        } else {
            assert!(
                notices
                    .iter()
                    .any(|notice| notice.text.contains("roar of steam"))
            );
        }
        assert_eq!(world.btech.constructed_units()[&id].inferno_remaining(), 0);
        assert_eq!(
            world.btech.maps()[&map]
                .decoration(BattleHexCoordinate { x: 1, y: 1 })
                .unwrap()
                .unwrap()
                .remaining,
            120
        );
        assert_eq!(
            world.btech.maps()[&map].hex(1, 1).unwrap().terrain(),
            Terrain::Smoke
        );
        assert_eq!(
            world.btech.maps()[&map].base_hex(1, 1).unwrap().terrain(),
            Terrain::Water
        );
        assert!(advance_inferno_burns(&mut world).is_empty());
        assert!(
            extinguish_inferno_in_water(&mut world, id)
                .unwrap()
                .is_empty()
        );
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

#[tokio::test]
async fn inferno_missile_exposure_rounds_pairs_and_replays_without_damage_or_dice() {
    let (_dir, config, mut base, _map, units) = fixture_field(Terrain::Grassland, 0, 3).await;
    let target = units[0];
    update_battle_contact(
        &mut base,
        units[1],
        target,
        BattleContactRules {
            hostile: true,
            hidden: false,
            perception: 7,
            acquire: true,
        },
    )
    .unwrap();
    for (missiles, seconds) in [(1, 180), (2, 180), (3, 360), (6, 540), (65535, 5_898_240)] {
        let mut world = base.clone();
        let report = resolve_inferno_hit(&mut world, target, missiles).unwrap();
        assert_eq!(report.target, target);
        assert_eq!(report.missiles, missiles);
        assert_eq!(report.burn_seconds, seconds);
        assert!(!report.extinguished);
        assert!(
            report
                .notices
                .iter()
                .any(|n| n.unit == target && n.text.contains("sprayed"))
        );
        assert!(
            report
                .notices
                .iter()
                .any(|n| n.unit == units[1] && n.text.contains("bursts into flames"))
        );
        let mut expected = serde_json::to_value(&base.btech).unwrap();
        expected["constructed"][target.0.to_string()]["inferno_remaining"] = seconds.into();
        assert_eq!(serde_json::to_value(&world.btech).unwrap(), expected);
        let checkpoint = world.clone();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        let more = resolve_inferno_hit(&mut world, target, 1).unwrap();
        assert_eq!(resolve_inferno_hit(&mut replay, target, 1).unwrap(), more);
        assert_eq!(world.btech, replay.btech);
        assert_eq!(
            world.btech.constructed_units()[&target].inferno_remaining(),
            seconds + 180
        );
        assert!(
            more.notices
                .iter()
                .any(|n| n.unit == target && n.text.contains("More burning jelly"))
        );
        assert!(
            more.notices
                .iter()
                .any(|n| n.unit == units[1] && n.text.contains("more brightly"))
        );
        assert_eq!(
            world.btech.constructed_units()[&target].heat(),
            checkpoint.btech.constructed_units()[&target].heat()
        );
    }
}

#[tokio::test]
async fn inferno_missile_immersion_extinguishes_after_ignition_and_rolls_back_late_failure() {
    for (terrain, depth, prone, extinguished) in [
        (Terrain::Grassland, 0, false, false),
        (Terrain::Water, 1, false, false),
        (Terrain::Water, 1, true, true),
        (Terrain::Water, 2, false, true),
        (Terrain::Ice, 2, false, false),
    ] {
        let (_dir, config, mut world, map, units) = fixture_field(terrain, depth, 3).await;
        let target = units[0];
        if prone {
            let _ = resolve_battle_fall(&mut world, target, 1, rules()).unwrap();
        }
        let before = world.clone();
        let report = resolve_inferno_hit(&mut world, target, 2).unwrap();
        assert_eq!(report.extinguished, extinguished);
        assert_eq!(report.burn_seconds, 180);
        assert_eq!(
            world.btech.constructed_units()[&target].inferno_remaining(),
            if extinguished { 0 } else { 180 }
        );
        assert_eq!(
            world.btech.constructed_units()[&target].heat(),
            before.btech.constructed_units()[&target].heat()
        );
        let cockpit: Vec<_> = report.notices.iter().filter(|n| n.unit == target).collect();
        assert!(cockpit[0].text.contains("sprayed"));
        if extinguished {
            assert!(cockpit[1].text.contains("roar of steam"));
            assert_eq!(
                world.btech.maps()[&map]
                    .decoration(BattleHexCoordinate { x: 1, y: 1 })
                    .unwrap()
                    .unwrap()
                    .remaining,
                120
            );
            assert!(advance_inferno_burns(&mut world).is_empty());
        }
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
        // Corrupt a different unit so validation fails after this target ignites and makes steam.
        // Per-operation validation runs only in debug builds; release relies on the commit check.
        if !cfg!(debug_assertions) {
            continue;
        }
        let mut invalid = before;
        let mut encoded = serde_json::to_value(&invalid.btech).unwrap();
        encoded["constructed"][units[1].0.to_string()]["inferno_remaining"] = u32::MAX.into();
        invalid.btech = serde_json::from_value(encoded).unwrap();
        let checkpoint = invalid.clone();
        assert!(resolve_inferno_hit(&mut invalid, target, 2).is_err());
        assert_eq!(invalid.btech, checkpoint.btech);
    }
}

#[tokio::test]
async fn inferno_missile_rejects_invalid_exposures_without_mutation() {
    let (_dir, _config, mut world, _map, units) = fixture_field(Terrain::Grassland, 0, 3).await;
    let target = units[0];
    let checkpoint = world.clone();
    assert!(resolve_inferno_hit(&mut world, target, 0).is_err());
    assert!(resolve_inferno_hit(&mut world, ObjectId(-1), 1).is_err());
    assert_eq!(world.btech, checkpoint.btech);
    world
        .objects
        .get_mut(&target)
        .unwrap()
        .flags
        .insert(Flag::Going);
    assert!(resolve_inferno_hit(&mut world, target, 1).is_err());
    assert_eq!(world.btech, checkpoint.btech);
    world
        .objects
        .get_mut(&target)
        .unwrap()
        .flags
        .remove(Flag::Going);
    apply_inferno_burn(&mut world, target, i32::MAX as u32 - 179).unwrap();
    let checkpoint = world.clone();
    assert!(resolve_inferno_hit(&mut world, target, 1).is_err());
    assert_eq!(world.btech, checkpoint.btech);
}

#[tokio::test]
async fn inferno_ammunition_explosion_halves_damage_and_applies_configured_heat() {
    for (terrain, depth) in [(Terrain::Grassland, 0), (Terrain::Water, 2)] {
        for penalty in [false, true] {
            let (_dir, config, mut world, _map, units) = fixture_field(terrain, depth, 3).await;
            let id = units[0];
            let mut encoded = serde_json::to_value(&world.btech).unwrap();
            encoded["constructed"][id.0.to_string()]["definition"]["sections"]["RightTorso"]["criticals"]
                ["0"]["modes"] = serde_json::json!(["Inferno"]);
            encoded["constructed"][id.0.to_string()]["ammunition"][0] = 1.into();
            world.btech = serde_json::from_value(encoded).unwrap();
            let mut rules = rules();
            rules.hit.inferno_penalty = penalty;
            let before = world.clone();
            let report = explode_battle_ammunition(&mut world, id, 0, rules).unwrap();
            assert_eq!(
                report.impact.phases.iter().map(|p| p.absorbed).sum::<u16>(),
                4
            );
            assert_eq!(world.btech.constructed_units()[&id].ammunition()[0], 0);
            assert_eq!(
                world.btech.constructed_units()[&id].heat().stored,
                before.btech.constructed_units()[&id].heat().stored
                    + if penalty { 30.0 } else { 0.0 }
            );
            assert_eq!(
                world.btech.constructed_units()[&id].inferno_remaining(),
                if depth == 2 { 0 } else { 180 }
            );
            assert!(report.notices.iter().any(|n| n.text.contains("sprayed")));
            assert_eq!(
                world.btech.constructed_units()[&id].pilot_injuries(),
                before.btech.constructed_units()[&id].pilot_injuries() + 2
            );
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
            let mut overflow = before;
            apply_inferno_burn(&mut overflow, id, i32::MAX as u32).unwrap();
            let checkpoint = overflow.clone();
            assert!(explode_battle_ammunition(&mut overflow, id, 0, rules).is_err());
            assert_eq!(overflow.btech, checkpoint.btech);
        }
    }
}

#[tokio::test]
async fn mine_event_ground_entry_spotting_burning_removal_and_restart() {
    for kind in [
        BattleMineKind::Standard,
        BattleMineKind::Inferno,
        BattleMineKind::Command,
    ] {
        let (_dir, config, mut world, map, units) = fixture_field(Terrain::Grassland, 0, 4).await;
        let id = units[0];
        prepare_reverse_step(&mut world, id, 7);
        set_minefield(
            &mut world,
            map,
            0,
            Some(BattleMinefield {
                coordinate: BattleHexCoordinate { x: 1, y: 2 },
                kind,
                strength: 4,
                extra: 0,
                owner: ObjectId(1),
            }),
        )
        .unwrap();
        let before = world.clone();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        let notices = advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
        assert_eq!(
            notices,
            advance_battle_motion(&mut replay, BattleMovementRules::STANDARD).unwrap()
        );
        assert_eq!(world.btech, replay.btech);
        assert_eq!(
            world.btech.constructed_units()[&id].position().unwrap().y,
            2
        );
        if kind == BattleMineKind::Command {
            assert!(notices.iter().any(|n| n.text.contains("small bomblets")));
            assert_eq!(
                world.btech.constructed_units()[&id].sections(),
                before.btech.constructed_units()[&id].sections()
            );
            assert_eq!(world.btech.maps()[&map].minefields().len(), 1);
        } else {
            assert!(
                notices
                    .iter()
                    .any(|n| n.text == "As you move to 1,2, you trigger a mine!")
            );
            assert_ne!(
                world.btech.constructed_units()[&id].sections(),
                before.btech.constructed_units()[&id].sections()
            );
            assert!(world.btech.maps()[&map].minefields().is_empty());
            assert_eq!(
                world.btech.constructed_units()[&id].inferno_remaining(),
                if kind == BattleMineKind::Inferno {
                    24
                } else {
                    0
                }
            );
        }
        let later = advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
        assert!(!later.iter().any(|n| n.text.contains("trigger a mine")));
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

#[tokio::test]
async fn mine_event_fall_and_jump_landing_activate_at_surface_once() {
    for jump in [false, true] {
        let (_dir, config, mut world, map, units) = fixture_field(Terrain::Grassland, 0, 4).await;
        let id = units[0];
        let coordinate = BattleHexCoordinate {
            x: 1,
            y: if jump { 2 } else { 1 },
        };
        set_minefield(
            &mut world,
            map,
            0,
            Some(BattleMinefield {
                coordinate,
                kind: BattleMineKind::Inferno,
                strength: 4,
                extra: 0,
                owner: ObjectId(1),
            }),
        )
        .unwrap();
        if jump {
            launch_battle_jump(&mut world, id, ObjectId(1), 180, 1.0).unwrap();
            let mut notices = Vec::new();
            for _ in 0..60 {
                notices.extend(
                    advance_battle_jumps(&mut world, BattleMovementRules::STANDARD).unwrap(),
                );
                if !world.btech.constructed_units()[&id].airborne() {
                    break;
                }
                assert_eq!(world.btech.constructed_units()[&id].inferno_remaining(), 0);
            }
            assert!(!world.btech.constructed_units()[&id].airborne());
            assert_eq!(
                notices
                    .iter()
                    .filter(|n| n.unit == id && n.text == "You trigger a mine!")
                    .count(),
                1
            );
        } else {
            let report = resolve_battle_fall(&mut world, id, 1, rules()).unwrap();
            assert_eq!(report.mines.reason, BattleMineTriggerReason::Fall);
            assert_eq!(report.mines.blasts.len(), 1);
            assert!(
                report
                    .notices(id)
                    .iter()
                    .any(|n| n.text == "You trigger a mine!")
            );
        }
        assert_eq!(world.btech.constructed_units()[&id].inferno_remaining(), 24);
        assert!(world.btech.maps()[&map].minefields().is_empty());
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

#[tokio::test]
async fn mine_event_scripted_entry_and_late_callback_failure_are_atomic() {
    let (dir, config, mut world, map, units) = fixture_field(Terrain::Grassland, 0, 4).await;
    let id = units[0];
    prepare_reverse_step(&mut world, id, 7);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    world.objects.get_mut(&id).unwrap().lua_parent = "mine_trigger.lua".into();
    std::fs::write(
        dir.path().join("lua/object_logic/mine_trigger.lua"),
        r#"return {events={on_mech_mine_trigger=function(ctx)
      assert(ctx.object==ctx.enactor and ctx.cause==ctx.object)
      assert(ctx.operation=='mine_trigger' and ctx.event=='on_mech_mine_trigger')
      local state=mux.world.object(ctx.object):state('mine')
      state:set('count',(state:get('count') or 0)+1)
      if mine_fail then error('mine callback abort') end
    end}}"#,
    )
    .unwrap();
    for (ordinal, kind, strength) in [
        (0, BattleMineKind::Inferno, 6),
        (1, BattleMineKind::Trigger, 0),
    ] {
        set_minefield(
            &mut world,
            map,
            ordinal,
            Some(BattleMinefield {
                coordinate: BattleHexCoordinate { x: 1, y: 2 },
                kind,
                strength,
                extra: 0,
                owner: ObjectId(1),
            }),
        )
        .unwrap();
    }
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    scripts.inspect_lua().load("mine_fail=true").exec().unwrap();
    let error =
        advance_battle_motion_action(&scripts, &config, BattleMovementRules::STANDARD).unwrap_err();
    assert!(
        format!("{error:#}").contains("mine callback abort"),
        "{error:#}"
    );
    assert_eq!(scripts.world().btech, world.btech);
    assert_eq!(
        serde_json::to_value(&*scripts.world()).unwrap(),
        serde_json::to_value(&world).unwrap()
    );
    assert!(scripts.drain_outbox().is_empty());
    scripts
        .inspect_lua()
        .load("mine_fail=false")
        .exec()
        .unwrap();
    advance_battle_motion_action(&scripts, &config, BattleMovementRules::STANDARD).unwrap();
    assert_eq!(
        scripts.world().btech.constructed_units()[&id].inferno_remaining(),
        36
    );
    assert_eq!(
        scripts
            .eval_callback::<i64>(&format!(
                "return mux.world.object({}):state('mine'):get('count')",
                id.0
            ))
            .unwrap(),
        1
    );
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
}

#[tokio::test]
async fn mine_event_nested_support_falls_and_deleted_definitions_are_ordered() {
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::Grassland, 0, 3).await;
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    for id in units {
        for leg in ["LeftLeg", "RightLeg"] {
            encoded["constructed"][id.0.to_string()]["sections"][leg]["armor"] = 0.into();
            encoded["constructed"][id.0.to_string()]["sections"][leg]["internal"] = 1.into();
        }
    }
    world.btech = serde_json::from_value(encoded).unwrap();
    let mine = BattleMinefield {
        coordinate: BattleHexCoordinate { x: 1, y: 1 },
        kind: BattleMineKind::Standard,
        strength: 6,
        extra: 0,
        owner: ObjectId(1),
    };
    set_minefield(&mut world, map, 0, Some(mine)).unwrap();
    let before = world.clone();
    let report =
        activate_mines(&mut world, units[0], BattleMineTriggerReason::Step, rules()).unwrap();
    assert_eq!(report.blasts.len(), 1);
    assert!(
        serde_json::to_string(&report)
            .unwrap()
            .contains("\"reason\":\"fall\"")
    );
    assert!(
        world
            .btech
            .constructed_units()
            .values()
            .all(|u| u.is_destroyed() || u.posture() == BattlePosture::Prone)
    );
    world.validate(&config).unwrap();
    let mut replay = before.clone();
    assert_eq!(
        activate_mines(
            &mut replay,
            units[0],
            BattleMineTriggerReason::Step,
            rules()
        )
        .unwrap(),
        report
    );
    assert_eq!(replay.btech, world.btech);
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    let mut weak = before;
    set_minefield(
        &mut weak,
        map,
        0,
        Some(BattleMinefield {
            strength: 1,
            ..mine
        }),
    )
    .unwrap();
    set_minefield(
        &mut weak,
        map,
        1,
        Some(BattleMinefield {
            kind: BattleMineKind::Trigger,
            strength: 0,
            ..mine
        }),
    )
    .unwrap();
    let report =
        activate_mines(&mut weak, units[0], BattleMineTriggerReason::Step, rules()).unwrap();
    assert_eq!(report.triggers, 0);
    assert!(weak.btech.maps()[&map].minefields().is_empty());
}

#[tokio::test]
async fn mine_event_late_blast_failure_restores_the_whole_ground_step() {
    let (_dir, _config, mut world, map, units) = fixture_field(Terrain::Grassland, 0, 4).await;
    let id = units[0];
    prepare_reverse_step(&mut world, id, 7);
    let mine = BattleMinefield {
        coordinate: BattleHexCoordinate { x: 1, y: 2 },
        kind: BattleMineKind::Inferno,
        strength: 6,
        extra: 0,
        owner: ObjectId(1),
    };
    set_minefield(&mut world, map, 0, Some(mine)).unwrap();
    set_minefield(
        &mut world,
        map,
        1,
        Some(BattleMinefield {
            kind: BattleMineKind::Vibra,
            strength: 2,
            ..mine
        }),
    )
    .unwrap();
    support::fail_mine_ignition(&mut world, map, 2);
    let before = world.clone();
    assert!(advance_battle_motion(&mut world, BattleMovementRules::STANDARD).is_err());
    assert_eq!(world.btech, before.btech);
}

#[tokio::test]
async fn mine_event_remote_vibra_reports_visible_explosion_and_neighbor_damage() {
    let (_dir, _config, mut world, map, units) = fixture_field(Terrain::Grassland, 0, 3).await;
    set_minefield(
        &mut world,
        map,
        0,
        Some(BattleMinefield {
            coordinate: BattleHexCoordinate { x: 1, y: 0 },
            kind: BattleMineKind::Vibra,
            strength: 6,
            extra: 0,
            owner: ObjectId(1),
        }),
    )
    .unwrap();
    let event =
        activate_mines(&mut world, units[0], BattleMineTriggerReason::Step, rules()).unwrap();
    assert_eq!(event.blasts.len(), 1);
    assert_eq!(event.blasts[0].hits.len(), 2);
    assert!(event.blasts[0].hits.iter().all(|h| h.damage == 3));
    assert!(
        event
            .notices
            .iter()
            .any(|n| n.unit == units[0]
                && n.text == "A mine explodes in [fg=yellow bold]1,0[reset]!")
    );
    assert!(world.btech.maps()[&map].minefields().is_empty());
}

#[tokio::test]
async fn command_mines_match_frequency_map_and_order_with_saved_replay() {
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::Grassland, 0, 7).await;
    let sender = units[0];
    let other = world.create(&config, "Other mine map".into(), Kind::Room);
    create_battle_map(
        &mut world,
        other,
        "other-mine.map",
        BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let mine = BattleMinefield {
        coordinate: BattleHexCoordinate { x: 1, y: 1 },
        kind: BattleMineKind::Command,
        strength: 4,
        extra: 42,
        owner: ObjectId(2),
    };
    for (ordinal, field) in [
        (2, mine),
        (3, mine),
        (
            4,
            BattleMinefield {
                kind: BattleMineKind::Standard,
                ..mine
            },
        ),
        (
            5,
            BattleMinefield {
                coordinate: BattleHexCoordinate { x: 0, y: 5 },
                extra: 77,
                ..mine
            },
        ),
        (
            6,
            BattleMinefield {
                coordinate: BattleHexCoordinate { x: 2, y: 4 },
                kind: BattleMineKind::Inferno,
                ..mine
            },
        ),
        (
            9,
            BattleMinefield {
                coordinate: BattleHexCoordinate { x: 1, y: 5 },
                ..mine
            },
        ),
    ] {
        set_minefield(&mut world, map, ordinal, Some(field)).unwrap();
    }
    set_minefield(
        &mut world,
        other,
        0,
        Some(BattleMinefield {
            coordinate: BattleHexCoordinate { x: 0, y: 0 },
            ..mine
        }),
    )
    .unwrap();
    let before = world.clone();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut replay = persistence::load(&config.database()).await.unwrap();
    let report = detonate_command_mines(&mut world, sender, 42, rules()).unwrap();
    assert_eq!(report.sender, sender);
    assert_eq!(report.map, map);
    assert_eq!(report.frequency, 42);
    assert_eq!(
        report
            .blasts
            .iter()
            .map(|b| b.mine.coordinate.y)
            .collect::<Vec<_>>(),
        vec![1, 5]
    );
    assert_eq!(report.blasts[0].removed, vec![2, 3, 4]);
    assert_eq!(report.blasts[1].removed, vec![9]);
    assert_eq!(
        world.btech.maps()[&map]
            .minefields()
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        vec![5, 6]
    );
    assert_eq!(world.btech.maps()[&other], before.btech.maps()[&other]);
    assert!(
        report
            .notices
            .iter()
            .any(|n| n.unit == sender
                && n.text == "A mine explodes in [fg=red bold]YOUR HEX[reset]!")
    );
    assert!(
        !report
            .notices
            .iter()
            .any(|n| n.text.contains("trigger a mine"))
    );
    assert_eq!(
        detonate_command_mines(&mut replay, sender, 42, rules()).unwrap(),
        report
    );
    assert_eq!(world.btech, replay.btech);
    let after = world.clone();
    let repeat = detonate_command_mines(&mut world, sender, 42, rules()).unwrap();
    assert!(repeat.blasts.is_empty() && repeat.notices.is_empty());
    assert_eq!(world.btech, after.btech);
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

#[tokio::test]
async fn command_mines_character_publication_and_late_rejection_are_atomic() {
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::Grassland, 0, 7).await;
    let sender = units[0];
    let target = units[1];
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["constructed"][target.0.to_string()]["position"]["y"] = 5.into();
    encoded["constructed"][target.0.to_string()]["motion"]["point"] =
        serde_json::to_value(BattleHexCoordinate { x: 1, y: 5 }.center()).unwrap();
    world.btech = serde_json::from_value(encoded).unwrap();
    world
        .objects
        .get_mut(&target)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let mine = BattleMinefield {
        coordinate: BattleHexCoordinate { x: 1, y: 1 },
        kind: BattleMineKind::Command,
        strength: 4,
        extra: 5,
        owner: ObjectId(1),
    };
    set_minefield(&mut world, map, 0, Some(mine)).unwrap();
    set_minefield(
        &mut world,
        map,
        1,
        Some(BattleMinefield {
            coordinate: BattleHexCoordinate { x: 1, y: 5 },
            ..mine
        }),
    )
    .unwrap();
    let before = world.clone();
    assert!(detonate_command_mines(&mut world, sender, 5, rules()).is_err());
    assert_eq!(world.btech, before.btech);
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let report = detonate_command_mines_action(&scripts, &config, sender, 5, rules()).unwrap();
    assert_eq!(report.blasts.len(), 2);
    assert_ne!(
        scripts.world().btech.constructed_units()[&sender].sections(),
        before.btech.constructed_units()[&sender].sections()
    );
    assert_ne!(
        scripts.world().btech.constructed_units()[&target].sections(),
        before.btech.constructed_units()[&target].sections()
    );
    assert!(scripts.world().btech.maps()[&map].minefields().is_empty());
    assert!(
        scripts
            .drain_outbox()
            .iter()
            .any(|(_, d)| d.source().contains("shrapnel"))
    );
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
    let mut invalid = before;
    set_minefield(
        &mut invalid,
        map,
        1,
        Some(BattleMinefield {
            coordinate: BattleHexCoordinate { x: 1, y: 5 },
            strength: 2,
            ..mine
        }),
    )
    .unwrap();
    support::fail_mine_ignition(&mut invalid, map, 5);
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(invalid.clone())),
    )
    .unwrap();
    assert!(detonate_command_mines_action(&scripts, &config, sender, 5, rules()).is_err());
    assert_eq!(scripts.world().btech, invalid.btech);
    assert!(scripts.drain_outbox().is_empty());
}

#[tokio::test]
async fn command_mines_frequency_values_and_invalid_senders_do_not_spend_dice() {
    let (_dir, _config, base, map, units) = fixture_field(Terrain::Grassland, 0, 3).await;
    let sender = units[0];
    for frequency in [0, -1, i32::MAX] {
        let mut world = base.clone();
        set_minefield(
            &mut world,
            map,
            0,
            Some(BattleMinefield {
                coordinate: BattleHexCoordinate { x: 0, y: 0 },
                kind: BattleMineKind::Command,
                strength: 0,
                extra: frequency,
                owner: ObjectId(2),
            }),
        )
        .unwrap();
        let before = world.clone();
        assert!(detonate_command_mines(&mut world, ObjectId(-999), frequency, rules()).is_err());
        assert_eq!(world.btech, before.btech);
        world
            .objects
            .get_mut(&sender)
            .unwrap()
            .flags
            .insert(Flag::Going);
        assert!(detonate_command_mines(&mut world, sender, frequency, rules()).is_err());
        assert_eq!(world.btech, before.btech);
        world
            .objects
            .get_mut(&sender)
            .unwrap()
            .flags
            .remove(Flag::Going);
        let report = detonate_command_mines(&mut world, sender, frequency, rules()).unwrap();
        assert_eq!(report.blasts.len(), 1);
        assert!(world.btech.maps()[&map].minefields().is_empty());
        assert_eq!(
            world.btech.constructed_units(),
            before.btech.constructed_units()
        );
    }
}

/// Artillery commits ordered center/neighbor packets and saved map/unit dice together.
#[tokio::test]
async fn artillery_world_damage_and_restart() {
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::Grassland, 0, 3).await;
    let center = BattleHexCoordinate { x: 1, y: 1 };
    let neighbor = BattleHexCoordinate { x: 1, y: 0 };
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["constructed"][units[1].0.to_string()]["position"]["y"] = 0.into();
    encoded["constructed"][units[1].0.to_string()]["motion"]["point"] =
        serde_json::to_value(neighbor.center()).unwrap();
    for (id, seed) in units.into_iter().zip([3, 4]) {
        encoded["constructed"][id.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    }
    world.btech = serde_json::from_value(encoded).unwrap();
    let mut flight = BattleArtilleryFlight::new(
        center,
        center,
        BattleWeapon::LongTom,
        BattleArtilleryMode::Standard,
        true,
    )
    .unwrap();
    let before = world.btech.clone();
    for _ in 0..9 {
        assert!(
            advance_artillery_flight(&mut world, map, &mut flight, rules())
                .unwrap()
                .is_none()
        );
    }
    assert_eq!(world.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut replay = persistence::load(&config.database()).await.unwrap();
    let mut saved_flight = serde_json::from_value(serde_json::to_value(&flight).unwrap()).unwrap();
    let report = advance_artillery_flight(&mut world, map, &mut flight, rules())
        .unwrap()
        .unwrap();
    assert_eq!(
        report
            .hits
            .iter()
            .map(|hit| (hit.unit, hit.impacts.len()))
            .collect::<Vec<_>>(),
        [(units[0], 4), (units[1], 2)]
    );
    assert_eq!(report.hits[0].coordinate, center);
    assert_eq!(report.hits[1].coordinate, neighbor);
    assert!(
        report
            .notices
            .iter()
            .any(|notice| notice.text == "You receive a direct hit!")
    );
    assert!(
        report
            .notices
            .iter()
            .any(|notice| notice.text == "You are hit by fragments!")
    );
    assert_eq!(
        advance_artillery_flight(&mut replay, map, &mut saved_flight, rules())
            .unwrap()
            .unwrap(),
        report
    );
    assert_eq!(replay.btech, world.btech);
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

/// Smoke covers the pattern except where a fire burns, and survives storage; delivered mines
/// neither reinforce nor replace existing fields.
#[tokio::test]
async fn artillery_world_smoke_and_mines() {
    let (_dir, config, mut world, map, _) = fixture_field(Terrain::Grassland, 0, 3).await;
    let center = BattleHexCoordinate { x: 1, y: 1 };
    set_map_decoration(
        &mut world,
        map,
        center,
        Some(BattleDecoration::new(BattleDecorationKind::Fire, 200, None)),
    )
    .unwrap();
    for mode in [
        BattleArtilleryMode::Smoke,
        BattleArtilleryMode::Mine,
        BattleArtilleryMode::Mine,
    ] {
        let mut flight =
            BattleArtilleryFlight::new(center, center, BattleWeapon::LongTom, mode, true).unwrap();
        let mut report = None;
        for _ in 0..10 {
            report = advance_artillery_flight(&mut world, map, &mut flight, rules()).unwrap();
        }
        let report = report.unwrap();
        assert!(report.hits.is_empty());
        if mode == BattleArtilleryMode::Smoke {
            for cell in &report.pattern.cells {
                let decoration = world.btech.maps()[&map]
                    .decoration(cell.position)
                    .unwrap()
                    .unwrap();
                if cell.position == center {
                    assert_eq!(decoration.kind, BattleDecorationKind::Fire);
                    continue;
                }
                assert_eq!(decoration.kind, BattleDecorationKind::Smoke);
                assert!((90..=150).contains(&decoration.remaining));
                assert_eq!(
                    world.btech.maps()[&map]
                        .base_hex(i64::from(cell.position.x), i64::from(cell.position.y))
                        .unwrap()
                        .terrain(),
                    Terrain::Grassland
                );
            }
        } else {
            assert_eq!(world.btech.maps()[&map].minefields().len(), 1);
            let mut field = world.btech.maps()[&map].minefields()[&0];
            if field.kind == BattleMineKind::Standard {
                assert_eq!(report.mines, [0]);
                assert_eq!(field.strength, 20);
                field.kind = BattleMineKind::Inferno;
                field.strength = 7;
                set_minefield(&mut world, map, 0, Some(field)).unwrap();
            } else {
                assert!(report.mines.is_empty());
                assert_eq!(field.strength, 7);
            }
        }
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

/// A late character guard restores earlier packets, map randomness and the flight cursor; the host action publishes them.
#[tokio::test]
async fn artillery_character_arrival_is_atomic() {
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::Grassland, 0, 3).await;
    for pilot in [ObjectId(1), ObjectId(2)] {
        set_battle_character(
            &mut world,
            pilot,
            BattleCharacter {
                build: 5,
                reflexes: 4,
                intuition: 3,
                learn: 2,
                charisma: 1,
                bruise: 0,
                lethal: 0,
            },
        )
        .unwrap();
    }
    let center = BattleHexCoordinate { x: 1, y: 1 };
    world
        .objects
        .get_mut(&units[1])
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let mut flight = BattleArtilleryFlight::new(
        center,
        center,
        BattleWeapon::Thumper,
        BattleArtilleryMode::Standard,
        true,
    )
    .unwrap();
    for _ in 0..9 {
        assert!(
            advance_artillery_flight(&mut world, map, &mut flight, rules())
                .unwrap()
                .is_none()
        );
    }
    let before = world.btech.clone();
    let cursor = flight.clone();
    assert!(
        advance_artillery_flight(&mut world, map, &mut flight, rules())
            .unwrap_err()
            .to_string()
            .contains("character consequence")
    );
    assert_eq!(world.btech, before);
    assert_eq!(flight, cursor);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let report = advance_artillery_flight_action(&scripts, &config, map, &mut flight, rules())
        .unwrap()
        .unwrap();
    assert_eq!(report.hits.len(), 2);
    assert_eq!(flight.remaining(), 0);
    assert!(
        scripts
            .drain_outbox()
            .iter()
            .any(|(_, document)| document.source().contains("direct hit"))
    );
    scripts.world().validate(&config).unwrap();
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
}

/// Artillery reaches three levels underwater but excludes four; its upper height limit is ten levels.
#[tokio::test]
async fn artillery_blast_height_limits() {
    for (terrain, depth, first, second) in
        [(Terrain::Water, 4, -3, -4), (Terrain::Grassland, 0, 9, 10)]
    {
        let (_dir, config, mut world, map, units) = fixture_field(terrain, depth, 3).await;
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        // Isolate the primary blast's height boundary from secondary reactor
        // explosions, which have their own reach and are covered separately.
        for unit in &units {
            encoded["constructed"][unit.0.to_string()]["power"] =
                serde_json::to_value(BattlePower::Off).unwrap();
        }
        encoded["constructed"][units[0].0.to_string()]["ground_elevation"] = first.into();
        if second == 10 {
            encoded["constructed"][units[1].0.to_string()]["free_fall"] =
                serde_json::to_value(BattleFreeFall::new(10)).unwrap();
        } else {
            encoded["constructed"][units[1].0.to_string()]["ground_elevation"] = second.into();
        }
        world.btech = serde_json::from_value(encoded).unwrap();
        let untouched = world.btech.constructed_units()[&units[1]].clone();
        let center = BattleHexCoordinate { x: 1, y: 1 };
        let mut flight = BattleArtilleryFlight::new(
            center,
            center,
            BattleWeapon::Thumper,
            BattleArtilleryMode::Standard,
            true,
        )
        .unwrap();
        let mut report = None;
        for _ in 0..10 {
            report = advance_artillery_flight(&mut world, map, &mut flight, rules()).unwrap();
        }
        assert_eq!(
            report
                .unwrap()
                .hits
                .iter()
                .map(|hit| hit.unit)
                .collect::<Vec<_>>(),
            [units[0]]
        );
        assert_eq!(world.btech.constructed_units()[&units[1]], untouched);
        world.validate(&config).unwrap();
    }
}

/// Occupants across the complete cluster area receive two-point punch packets; failed character admission restores scatter dice.
#[tokio::test]
async fn artillery_cluster_world_packets_and_random_rollback() {
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::Grassland, 0, 3).await;
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["constructed"][units[1].0.to_string()]["position"]["x"] = 0.into();
    encoded["constructed"][units[1].0.to_string()]["position"]["y"] = 0.into();
    encoded["constructed"][units[1].0.to_string()]["motion"]["point"] =
        serde_json::to_value(BattleHexCoordinate { x: 0, y: 0 }.center()).unwrap();
    encoded["maps"][map.0.to_string()]["fire_dice"] =
        serde_json::to_value(BattleDice::seeded([7; 32])).unwrap();
    world.btech = serde_json::from_value(encoded).unwrap();
    for x in 0..3 {
        for y in 0..3 {
            if (x, y) == (0, 0) || (x, y) == (1, 1) {
                continue;
            }
            let id = world.create(&config, format!("Cluster target {x},{y}"), Kind::Thing);
            world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
            create_battle_unit(
                &mut world,
                id,
                BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml"))
                    .unwrap(),
            )
            .unwrap();
            place_battle_unit(&mut world, id, map, x, y).unwrap();
        }
    }
    let center = BattleHexCoordinate { x: 1, y: 1 };
    let mut flight = BattleArtilleryFlight::new(
        center,
        center,
        BattleWeapon::LongTom,
        BattleArtilleryMode::Cluster,
        true,
    )
    .unwrap();
    for _ in 0..9 {
        assert!(
            advance_artillery_flight(&mut world, map, &mut flight, rules())
                .unwrap()
                .is_none()
        );
    }
    let before = world.clone();
    let cursor = flight.clone();
    let report = advance_artillery_flight(&mut world, map, &mut flight, rules())
        .unwrap()
        .unwrap();
    assert_eq!(
        report
            .hits
            .iter()
            .map(|hit| hit.impacts.len())
            .sum::<usize>(),
        20
    );
    for cell in &report.pattern.cells {
        let BattleArtilleryEffect::Damage {
            total,
            packet_size: 2,
            table: BattleHitTable::Punch,
        } = cell.effect
        else {
            panic!("Expected cluster packet")
        };
        let hit = report
            .hits
            .iter()
            .find(|hit| hit.coordinate == cell.position)
            .unwrap();
        assert_eq!(hit.impacts.len(), usize::from(total / 2));
    }
    world.validate(&config).unwrap();
    let rejected_unit = report.hits.last().unwrap().unit;
    let mut invalid = before;
    invalid
        .objects
        .get_mut(&rejected_unit)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let checkpoint = invalid.btech.clone();
    let mut retry = cursor.clone();
    assert!(advance_artillery_flight(&mut invalid, map, &mut retry, rules()).is_err());
    assert_eq!(invalid.btech, checkpoint);
    assert_eq!(retry, cursor);
}

/// Launch order and remaining seconds survive storage, and rounds outlive a pending-removal shooter.
#[tokio::test]
async fn artillery_queue_order_and_database_replay() {
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::Grassland, 0, 3).await;
    let center = BattleHexCoordinate { x: 1, y: 1 };
    for (index, mode) in [BattleArtilleryMode::Smoke, BattleArtilleryMode::Mine]
        .into_iter()
        .enumerate()
    {
        let flight =
            BattleArtilleryFlight::new(center, center, BattleWeapon::LongTom, mode, true).unwrap();
        assert_eq!(
            enqueue_artillery(&mut world, map, units[0], flight).unwrap(),
            index as u32
        );
    }
    assert!(artillery_pending(&world));
    world
        .objects
        .get_mut(&units[0])
        .unwrap()
        .flags
        .insert(Flag::Going);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    for _ in 0..5 {
        assert!(
            advance_artillery_action(&scripts, &config, rules())
                .unwrap()
                .is_empty()
        );
    }
    let midpoint = scripts.world().clone();
    assert!(
        midpoint.btech.maps()[&map]
            .artillery_shots()
            .values()
            .all(|shot| shot.flight.remaining() == 5)
    );
    persistence::save(&config.database(), &midpoint)
        .await
        .unwrap();
    let replay = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(
            persistence::load(&config.database()).await.unwrap(),
        )),
    )
    .unwrap();
    for _ in 0..5 {
        let first = advance_artillery_action(&scripts, &config, rules()).unwrap();
        assert_eq!(
            first,
            advance_artillery_action(&replay, &config, rules()).unwrap()
        );
        assert_eq!(scripts.world().btech, replay.world().btech);
        if !first.is_empty() {
            assert_eq!(first.len(), 2);
            assert!(matches!(
                first[0].pattern.cells[0].effect,
                BattleArtilleryEffect::Smoke { .. }
            ));
            assert!(matches!(
                first[1].pattern.cells[0].effect,
                BattleArtilleryEffect::Mine { .. }
            ));
        }
    }
    assert!(!artillery_pending(&scripts.world()));
    let final_world = scripts.world().clone();
    persistence::save(&config.database(), &final_world)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        final_world.btech
    );
}

/// A rejected later round rolls back earlier smoke, queue removal, random draws and published notices.
#[tokio::test]
async fn artillery_queue_late_arrival_rolls_back_all_shots() {
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::Grassland, 0, 3).await;
    let center = BattleHexCoordinate { x: 1, y: 1 };
    world
        .objects
        .get_mut(&units[1])
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let seed = (0..=255)
        .find(|&seed| BattleDice::seeded([seed; 32]).two_d6() == 12)
        .unwrap();
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["constructed"][units[1].0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    world.btech = serde_json::from_value(encoded).unwrap();
    for mode in [BattleArtilleryMode::Smoke, BattleArtilleryMode::Standard] {
        enqueue_artillery(
            &mut world,
            map,
            units[0],
            BattleArtilleryFlight::new(center, center, BattleWeapon::Thumper, mode, true).unwrap(),
        )
        .unwrap();
    }
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    for _ in 0..9 {
        assert!(
            advance_artillery_action(&scripts, &config, rules())
                .unwrap()
                .is_empty()
        );
    }
    scripts.drain_outbox();
    let before = scripts.world().btech.clone();
    assert!(
        advance_artillery_action(&scripts, &config, rules())
            .unwrap_err()
            .to_string()
            .contains("Character state")
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
}

/// A flight counting down with the simulation clock is never rewritten; only its arrival
/// touches the row.
#[tokio::test]
async fn artillery_flight_in_progress_leaves_its_row_unchanged() {
    tokio::task::LocalSet::new().run_until(async {
        use sqlx::Connection;
        let (_dir, config, mut world, map, units) = fixture_field(Terrain::Grassland, 0, 3).await;
        for (unit, pilot) in units.into_iter().zip([ObjectId(1), ObjectId(2)]) {
            stop_battle_unit(&mut world, unit, pilot, rules()).unwrap();
        }
        let center = BattleHexCoordinate { x: 1, y: 1 };
        enqueue_artillery(&mut world, map, units[0], BattleArtilleryFlight::new(center, center, BattleWeapon::LongTom, BattleArtilleryMode::Mine, true).unwrap()).unwrap();
        let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        for _ in 0..7 { assert!(advance_artillery_action(&scripts, &config, rules()).unwrap().is_empty()); }
        let world_snapshot = scripts.world().clone();
        persistence::save(&config.database(), &world_snapshot).await.unwrap();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::raw_sql("CREATE TABLE artillery_updates(n INTEGER); CREATE TRIGGER count_artillery_updates AFTER UPDATE ON btech_artillery BEGIN INSERT INTO artillery_updates VALUES(1); END;").execute(&mut sql).await.unwrap();
        let (_address, shutdown, task, _lua) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        tokio::time::timeout(std::time::Duration::from_secs(6), async {
            loop {
                let loaded = persistence::load(&config.database()).await.unwrap();
                if loaded.btech.maps()[&map].artillery_shots().is_empty() {
                    assert_eq!(loaded.btech.maps()[&map].minefields().len(), 1);
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }).await.unwrap();
        let updates: i64 = sqlx::query_scalar("SELECT count(*) FROM artillery_updates").fetch_one(&mut sql).await.unwrap();
        assert_eq!(updates, 0);
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

/// The real server restores queued arrivals and their effects when a database delete fails, then retries once.
#[tokio::test]
async fn artillery_queue_server_save_failure_and_retry() {
    tokio::task::LocalSet::new().run_until(async {
        use sqlx::Connection;
        let (_dir, config, mut world, map, units) = fixture_field(Terrain::Grassland, 0, 3).await;
        for (unit, pilot) in units.into_iter().zip([ObjectId(1), ObjectId(2)]) {
                let _notices = stop_battle_unit(&mut world, unit, pilot, rules()).unwrap();
                assert_eq!(world.btech.constructed_units()[&unit].power(), BattlePower::Off);
            }
            let center = BattleHexCoordinate { x: 1, y: 1 };
        enqueue_artillery(&mut world, map, units[0], BattleArtilleryFlight::new(center, center, BattleWeapon::LongTom, BattleArtilleryMode::Mine, true).unwrap()).unwrap();
        let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        for _ in 0..9 { assert!(advance_artillery_action(&scripts, &config, rules()).unwrap().is_empty()); }
        let before = scripts.world().clone();
        persistence::save(&config.database(), &before).await.unwrap();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::query("CREATE TRIGGER deny_artillery BEFORE DELETE ON btech_artillery BEGIN SELECT RAISE(ABORT,'artillery failure'); END").execute(&mut sql).await.unwrap();
        let (_address, shutdown, task, _lua) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        tokio::time::sleep(std::time::Duration::from_millis(1400)).await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech, before.btech);
        sqlx::query("DROP TRIGGER deny_artillery").execute(&mut sql).await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let saved = persistence::load(&config.database()).await.unwrap();
                if saved.btech.maps()[&map].artillery_shots().is_empty() {
                    assert_eq!(saved.btech.maps()[&map].minefields().len(), 1);
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }).await.unwrap();
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

/// The schema refuses a completed cursor, and loading rejects one longer than its launch allows.
#[tokio::test]
async fn artillery_queue_rejects_corrupt_saved_cursor() {
    use sqlx::Connection;
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::Grassland, 0, 3).await;
    let center = BattleHexCoordinate { x: 1, y: 1 };
    enqueue_artillery(
        &mut world,
        map,
        units[0],
        BattleArtilleryFlight::new(
            center,
            center,
            BattleWeapon::LongTom,
            BattleArtilleryMode::Mine,
            true,
        )
        .unwrap(),
    )
    .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut sql = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    assert!(
        sqlx::query("UPDATE btech_artillery SET arrives_at=0 WHERE map_dbref=?")
            .bind(map.0)
            .execute(&mut sql)
            .await
            .is_err()
    );
    sqlx::query("UPDATE btech_artillery SET arrives_at=60000 WHERE map_dbref=?")
        .bind(map.0)
        .execute(&mut sql)
        .await
        .unwrap();
    let error = persistence::load(&config.database()).await.unwrap_err();
    assert!(
        format!("{error:#}").contains("Invalid artillery countdown"),
        "{error:#}"
    );
}

/// Artillery always reads its dedicated connected-pilot skill and otherwise uses the default target of eight.
#[tokio::test]
async fn artillery_gunnery_uses_dedicated_skill_without_mutation() {
    let (_dir, _config, mut world, _, units) = fixture_field(Terrain::Grassland, 0, 3).await;
    let pilot = ObjectId(1);
    world
        .objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .remove(Flag::Connected);
    assert_eq!(unit_artillery_gunnery_target(&world, units[0]).unwrap(), 8);
    world
        .objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    assert_eq!(unit_artillery_gunnery_target(&world, units[0]).unwrap(), 18);
    set_battle_character(
        &mut world,
        pilot,
        BattleCharacter {
            build: 5,
            reflexes: 4,
            intuition: 3,
            learn: 2,
            charisma: 1,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    for (skill, value) in [
        ("Gunnery-Artillery", 5),
        ("Gunnery-Battlemech", 20),
        ("Gunnery-Missile", 20),
    ] {
        set_battle_character_value(
            &mut world,
            pilot,
            skill,
            BattleCharacterValue {
                value,
                experience: 0,
                last_used: 0,
            },
        )
        .unwrap();
    }
    let before = world.btech.clone();
    assert_eq!(unit_artillery_gunnery_target(&world, units[0]).unwrap(), 6);
    assert_eq!(world.btech, before);
    world.objects.get_mut(&pilot).unwrap().location = None;
    assert_eq!(unit_artillery_gunnery_target(&world, units[0]).unwrap(), 8);
}

/// A surface collapse finishes admitted falls after another occupant's reactor blast.
async fn fracture_cascade_matrix(vehicle: bool, trigger_last: bool) {
    let (_dir, config, mut world, map, units) = fixture(2).await;
    let aircraft = vehicle.then(|| {
        let id = world.create(&config, "Surface aircraft".into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        create_battle_vehicle(
            &mut world,
            id,
            BattleVehicleTemplate::parse("Kestrel", include_str!("../game/mechs/Kestrel.toml"))
                .unwrap(),
        )
        .unwrap();
        place_battle_unit(&mut world, id, map, 1, 1).unwrap();
        assert!(!world.btech.vehicles()[&id].is_destroyed());
        id
    });
    let mut state = serde_json::to_value(&world.btech).unwrap();
    for id in units {
        state["constructed"][id.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([14; 32])).unwrap();
    }
    if let Some(id) = aircraft {
        state["vehicles"][id.0.to_string()]["vtol_flight"] =
            serde_json::to_value(BattleVtolFlight::default()).unwrap();
        state["vehicles"][id.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([14; 32])).unwrap();
    }
    world.btech = serde_json::from_value(state).unwrap();
    // Select a collapse whose first fall detonates a reactor and interrupts the next fall.
    let trigger = trigger_last.then_some(units[0]);
    let coordinate = BattleHexCoordinate { x: 1, y: 1 };
    let template = serde_json::to_value(&world.btech).unwrap();
    let seeded = |state: &mut serde_json::Value, bytes: [u8; 32]| {
        for id in units {
            state["constructed"][id.0.to_string()]["dice"] =
                serde_json::to_value(BattleDice::seeded(bytes)).unwrap();
        }
    };
    let selected = (0u32..10_000)
        .find_map(|seed| {
            let mut bytes = [0; 32];
            bytes[..4].copy_from_slice(&seed.to_le_bytes());
            let mut state = template.clone();
            seeded(&mut state, bytes);
            let mut candidate = world.clone();
            candidate.btech = serde_json::from_value(state).unwrap();
            let report =
                break_battle_ice(&mut candidate, map, coordinate, trigger, rules()).unwrap();
            let blast = report
                .falls
                .first()?
                .1
                .groups
                .iter()
                .flat_map(|group| &group.flooding)
                .find_map(|flood| flood.reactor_explosion.as_ref())?;
            (report.falls.len() == 2
                && blast.hits.iter().any(|hit| hit.unit == report.falls[1].0)
                && report.falls[1].1.groups.is_empty())
            .then_some(seed)
        })
        .expect("collapse with an interrupted pending fall");
    let mut state = template.clone();
    let mut bytes = [0; 32];
    bytes[..4].copy_from_slice(&selected.to_le_bytes());
    seeded(&mut state, bytes);
    let mut selected = world.clone();
    selected.btech = serde_json::from_value(state).unwrap();
    world = selected;
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restarted = persistence::load(&config.database()).await.unwrap();
    let report = break_battle_ice(&mut world, map, coordinate, trigger, rules()).unwrap();
    assert_eq!(
        report,
        break_battle_ice(&mut restarted, map, coordinate, trigger, rules()).unwrap()
    );
    assert_eq!(world.btech, restarted.btech);
    assert_eq!(report.falls.len(), 2);
    let blast = report.falls[0]
        .1
        .groups
        .iter()
        .flat_map(|group| &group.flooding)
        .find_map(|flood| flood.reactor_explosion.as_ref())
        .unwrap();
    assert!(blast.hits.iter().any(|hit| hit.unit == report.falls[1].0));
    assert!(report.falls[1].1.groups.is_empty());
    for id in units {
        let unit = &world.btech.constructed_units()[&id];
        assert!(unit.is_destroyed());
        assert_eq!(unit.posture(), BattlePosture::Prone);
    }
    if let Some(id) = aircraft {
        assert!(world.btech.vehicles()[&id].is_destroyed());
        assert_eq!(report.vehicle_falls.len(), 1);
        assert_eq!(report.vehicle_falls[0].0, id);
        assert!(!report.flooded_vehicles.contains(&id));
        assert!(resolve_battle_vehicle_fall(&mut world, id, 1, rules()).is_err());
    }
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    let before = world.btech.clone();
    assert!(resolve_battle_fall(&mut world, units[0], 1, rules()).is_err());
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn fracture_cascade_finishes_pending_wreck_falls_and_replays_mech_first() {
    fracture_cascade_matrix(false, false).await;
}

#[tokio::test]
async fn fracture_cascade_finishes_pending_wreck_falls_and_replays_mech_last() {
    fracture_cascade_matrix(false, true).await;
}

#[tokio::test]
async fn fracture_cascade_finishes_pending_wreck_falls_and_replays_vtol_first() {
    fracture_cascade_matrix(true, false).await;
}

#[tokio::test]
async fn fracture_cascade_finishes_pending_wreck_falls_and_replays_vtol_last() {
    fracture_cascade_matrix(true, true).await;
}
