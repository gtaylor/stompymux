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
    fixture_asset(MapAsset::from_cells(&format!("3 {height}\n{}", row.repeat(height))).unwrap())
        .await
}

/// Construct two running bipeds on an isolated, caller-defined terrain asset.
async fn fixture_asset(
    asset: MapAsset,
) -> (tempfile::TempDir, Config, World, ObjectId, [ObjectId; 2]) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Surface field".into(), Kind::Room);
    create_battle_map(&mut world, map, "surface.map", asset).unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let mut units = Vec::new();
    for pilot in [ObjectId(1), ObjectId(2)] {
        let id = world.create(&config, format!("Surface unit {}", pilot.0), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        create_battle_unit(
            &mut world,
            id,
            MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap(),
        )
        .unwrap();
        support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
        place_battle_unit(&mut world, id, map, 1, 1).unwrap();
        world.objects.get_mut(&pilot).unwrap().location = Some(id);
        assign_battle_pilot(&mut world, id, pilot).unwrap();
        start_battle_unit(&mut world, id, pilot, true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0, true);
        }
        let mut state = serde_json::to_value(&world.btech).unwrap();
        let unit = &mut state["constructed"][id.0.to_string()];
        unit["dice"] = serde_json::to_value(Dice::seeded([19; 32])).unwrap();
        unit["crew_recovery"]["dice"] = serde_json::to_value(Dice::seeded([13; 32])).unwrap();
        state["recoveries"][pilot.0.to_string()]["dice"] =
            serde_json::to_value(Dice::seeded([11; 32])).unwrap();
        world.btech = serde_json::from_value(state).unwrap();
        units.push(id);
    }
    (dir, config, world, map, [units[0], units[1]])
}

/// Ordinary configured tactical fall semantics for deterministic fracture tests.
fn rules() -> FallRules {
    FallRules {
        vehicle_impact: stompymux_rs::VehicleImpactRules::STANDARD,
        extended_piloting: true,
        toughness: false,
    }
}

/// Advance a jump one quiet second at a time until its next event is the landing. A jump
/// movement point covers a hex a minute, so even a short jump spans many events.
fn fly_until_landing_is_next(world: &mut World, id: ObjectId) {
    loop {
        let unit = &world.btech.constructed_units()[&id];
        let map = unit.position().unwrap().map;
        let mut flight = unit.flight().unwrap();
        let capacity = unit
            .jump_capacity(world.btech.maps()[&map].gravity)
            .unwrap();
        if flight.advance(capacity).unwrap().outcome == JumpOutcome::Landing {
            return;
        }
        assert!(
            advance_battle_jumps(
                world,
                stompymux_rs::MovementRules {
                    fall: rules(),
                    ..stompymux_rs::MovementRules::STANDARD
                }
            )
            .unwrap()
            .is_empty()
        );
    }
}

#[tokio::test]
async fn ice_fracture_drops_neighbors_before_trigger_and_replays_after_restart() {
    let (_dir, config, mut world, map, units) = fixture(3).await;
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restarted = persistence::load(&config.database()).await.unwrap();
    let coordinate = HexCoordinate { x: 1, y: 1 };
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
        Hex::new(Terrain::Water, 3)
    );
    for id in units {
        assert_eq!(
            world.btech.constructed_units()[&id].posture(),
            Posture::Prone
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
            HexCoordinate { x: 1, y: 1 },
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
    let report =
        break_battle_ice(&mut world, map, HexCoordinate { x: 1, y: 1 }, None, rules()).unwrap();
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
    let report =
        break_battle_ice(&mut world, map, HexCoordinate { x: 1, y: 1 }, None, rules()).unwrap();
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
            let mut dice = Dice::seeded([*seed; 32]);
            if avoidance_first {
                dice.two_d6();
            }
            // Ice that holds is followed by a +0 footing roll, which these seeds pass, and a
            // landing beside the neighbour on the sheet misses its accidental fall from above.
            (dice.d6() == 1) == fracture
                && (fracture
                    || (dice.two_d6() >= 11 && dice.die(1).unwrap() == 1 && dice.two_d6() < 7))
        })
        .unwrap();
    world
        .btech
        .set_unit_dice(id, Dice::seeded([seed; 32]))
        .unwrap();
}

/// Seed the dice for a jump landing on ice: the sheet breaks on a four or better, ice that
/// holds is followed by a +4 footing roll these seeds pass, and the landing beside the
/// neighbour on the sheet misses its accidental fall from above (Total Warfare p. 152).
fn ice_landing_seed(world: &mut World, id: ObjectId, fracture: bool) {
    let seed = (0..=255)
        .find(|seed| {
            let mut dice = Dice::seeded([*seed; 32]);
            (dice.d6() <= 3) == fracture
                && (fracture
                    || (dice.two_d6() >= 11 && dice.die(1).unwrap() == 1 && dice.two_d6() < 7))
        })
        .unwrap();
    world
        .btech
        .set_unit_dice(id, Dice::seeded([seed; 32]))
        .unwrap();
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
        ice_landing_seed(&mut world, id, fracture);
        for _ in 0..11 {
            advance_battle_jumps(
                &mut world,
                stompymux_rs::MovementRules {
                    fall: rules(),
                    ..stompymux_rs::MovementRules::STANDARD
                },
            )
            .unwrap();
        }
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restarted = persistence::load(&config.database()).await.unwrap();
        let notices = advance_battle_jumps(
            &mut world,
            stompymux_rs::MovementRules {
                fall: rules(),
                ..stompymux_rs::MovementRules::STANDARD
            },
        )
        .unwrap();
        assert_eq!(
            notices,
            advance_battle_jumps(
                &mut restarted,
                stompymux_rs::MovementRules {
                    fall: rules(),
                    ..stompymux_rs::MovementRules::STANDARD
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
                Posture::Prone
            } else {
                Posture::Standing
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
            stompymux_rs::MovementRules {
                fall: rules(),
                ..stompymux_rs::MovementRules::STANDARD
            },
        )
        .unwrap();
    }
    let path = world.btech.constructed_units()[&id]
        .flight()
        .unwrap()
        .path();
    let fracture =
        break_battle_ice(&mut world, map, HexCoordinate { x: 1, y: 0 }, None, rules()).unwrap();
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
            stompymux_rs::MovementRules {
                fall: rules(),
                ..stompymux_rs::MovementRules::STANDARD
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
            stompymux_rs::MovementRules {
                fall: rules(),
                ..stompymux_rs::MovementRules::STANDARD
            },
        )
        .unwrap();
    }
    let seed = (0..=255)
        .find(|seed| {
            let mut dice = Dice::seeded([*seed; 32]);
            dice.two_d6() >= 6 && dice.d6() == 1
        })
        .unwrap();
    world
        .btech
        .set_unit_dice(id, Dice::seeded([seed; 32]))
        .unwrap();
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
        let coordinate = HexCoordinate { x: 1, y: 1 };
        let report = break_battle_bridge(&mut world, map, coordinate, rules()).unwrap();
        assert_eq!(
            report,
            break_battle_bridge(&mut restarted, map, coordinate, rules()).unwrap()
        );
        assert_eq!(world.btech, restarted.btech);
        assert_eq!(report.before, Hex::new(Terrain::Bridge, height));
        assert_eq!(report.after, Hex::new(Terrain::Water, 1));
        // Both units stand on the deck and fall past it into the depth-one water below.
        assert_eq!(report.fall_levels, height + 1);
        assert_eq!(
            report.falls.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            units.to_vec()
        );
        // A 35-ton Jenner falls for 4 a multiplier. A fall that starts above the water's
        // surface takes half for striking the surface and half again, with the depth, for
        // the bottom (Total Warfare p. 57); one from the surface takes a single halved fall.
        let multiplier = u32::from(height) + 1;
        let damage = if multiplier > 2 {
            4 * (multiplier - 1) / 2 + 4 * 2 / 2
        } else {
            4 * multiplier / 2
        };
        assert!(
            report.falls.iter().all(|(_, fall)| fall.damage == damage),
            "{height}"
        );
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
    assert!(break_battle_bridge(&mut world, map, HexCoordinate { x: 1, y: 1 }, rules()).is_err());
    assert_eq!(world.btech, before);
    world
        .objects
        .get_mut(&units[1])
        .unwrap()
        .flags
        .remove(Flag::InCharacter);
    let report =
        break_battle_bridge(&mut world, map, HexCoordinate { x: 1, y: 1 }, rules()).unwrap();
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
    world
        .btech
        .rewrite_unit_record(units[0], |record| {
            record["ground_elevation"] = serde_json::json!(3.0);
        })
        .unwrap();
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
    stop_battle_unit(&mut placed, units[0], ObjectId(1)).unwrap();
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
            stompymux_rs::MovementRules {
                fall: rules(),
                ..stompymux_rs::MovementRules::STANDARD
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
        stop_battle_unit(&mut world, target, ObjectId(2)).unwrap();
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
                serde_json::to_value(Dice::seeded([index as u8; 32])).unwrap();
        }
        world.btech = serde_json::from_value(state).unwrap();
        if airborne {
            launch_battle_jump(&mut world, shooter, ObjectId(1), 0, 1.0).unwrap();
            for _ in 0..6 {
                advance_battle_jumps(
                    &mut world,
                    stompymux_rs::MovementRules {
                        fall: rules(),
                        ..stompymux_rs::MovementRules::STANDARD
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
                let mut dice = Dice::seeded([*seed; 32]);
                dice.two_d6() >= 7 && dice.d6() != 1
            })
            .unwrap();
        world
            .btech
            .set_unit_dice(id, Dice::seeded([seed; 32]))
            .unwrap();
        let initial = resolve_battle_fall(&mut world, id, 1, rules()).unwrap();
        assert!(initial.avoidance.unwrap().success && initial.ice_break.is_none());
        let seed = (0..=255)
            .find(|seed| {
                let mut dice = Dice::seeded([*seed; 32]);
                // Standing on ice takes the four-point ice modifier.
                if (dice.two_d6() >= if mode == "careful" { 8 } else { 10 }) != success {
                    return false;
                }
                success || (dice.two_d6() >= 7 && (dice.d6() == 1) == fracture)
            })
            .unwrap();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][id.0.to_string()]["dice"] =
            serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
        state["constructed"][units[1].0.to_string()]["dice"] =
            serde_json::to_value(Dice::seeded([19; 32])).unwrap();
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
                Posture::Standing
            } else {
                Posture::Prone
            }
        );
        assert_eq!(
            expected.btech.constructed_units()[&units[1]].posture(),
            if fracture {
                Posture::Prone
            } else {
                Posture::Standing
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
/// using the retained altitude before and after a restart. Sweeps of 12, which find nothing,
/// leave sight alone to answer.
#[tokio::test]
async fn optical_water_attenuation_uses_retained_altitude_after_restart() {
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::Water, 1, 8).await;
    for (pilot, id, y) in [(ObjectId(1), units[0], 0), (ObjectId(2), units[1], 7)] {
        stop_battle_unit(&mut world, id, pilot).unwrap();
        place_battle_unit(&mut world, id, map, 1, y).unwrap();
    }
    // A saved prone unit retained at elevation zero has its eyes above the waterline.
    let mut state = serde_json::to_value(&world.btech).unwrap();
    for id in units {
        state["constructed"][id.0.to_string()]["posture"] = serde_json::json!("prone");
        state["constructed"][id.0.to_string()]["ground_elevation"] = serde_json::json!(0.0);
        state["constructed"][id.0.to_string()]["sensor_sweep"] = serde_json::json!(12);
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
            perception.map(|perception| (perception.channel, perception.identified)),
            perceived.then_some((DetectionChannel::Sight, true))
        );
    };
    check(&world, true);
    check(&loaded, true);
    // Administrative placement settles both prone units onto the water bottom.
    for (id, y) in [(units[0], 0), (units[1], 7)] {
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
        stop_battle_unit(&mut world, target, ObjectId(2)).unwrap();
        place_battle_unit(&mut world, target, map, 1, 0).unwrap();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        for (index, id) in units.into_iter().enumerate() {
            state["constructed"][id.0.to_string()]["dice"] =
                serde_json::to_value(Dice::seeded([index as u8; 32])).unwrap();
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
            break_battle_bridge(&mut world, map, HexCoordinate { x: 1, y: 0 }, rules()).unwrap();
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
        (Terrain::Clear, 4, 4),
        (Terrain::Bridge, 3, 3),
    ] {
        let (_dir, config, mut world, map, units) = fixture_surface(terrain, depth).await;
        let id = units[0];
        assert_elevation_inspection(&config, &world, id, Some(elevation));
        if terrain == Terrain::Bridge {
            let report =
                break_battle_bridge(&mut world, map, HexCoordinate { x: 1, y: 1 }, rules())
                    .unwrap();
            // Both units stood on the deck and fall to the bed of the depth-one water.
            assert_eq!(report.falls.len(), 2);
            persistence::save(&config.database(), &world).await.unwrap();
            world = persistence::load(&config.database()).await.unwrap();
            assert_elevation_inspection(&config, &world, id, Some(-1));
        }
        if terrain == Terrain::Clear {
            launch_battle_jump(&mut world, id, ObjectId(1), 0, 1.0).unwrap();
            assert_elevation_inspection(&config, &world, id, Some(4));
            for _ in 0..6 {
                advance_battle_jumps(
                    &mut world,
                    stompymux_rs::MovementRules {
                        fall: rules(),
                        ..stompymux_rs::MovementRules::STANDARD
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
                    stompymux_rs::MovementRules {
                        fall: rules(),
                        ..stompymux_rs::MovementRules::STANDARD
                    },
                )
                .unwrap();
            }
            assert_elevation_inspection(&config, &world, id, Some(4));
        }
        stop_battle_unit(&mut world, id, ObjectId(1)).unwrap();
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
        world
            .btech
            .rewrite_unit_record(id, |record| {
                record["ground_elevation"] = serde_json::json!(initial);
            })
            .unwrap();
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
        assert!(fall.feedback.flooding.is_empty());
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
    stop_battle_unit(&mut world, target, ObjectId(2)).unwrap();
    place_battle_unit(&mut world, target, map, 1, 0).unwrap();
    world
        .btech
        .set_unit_dice(id, Dice::seeded([0; 32]))
        .unwrap();
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
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() >= 7)
        .unwrap();
    world
        .btech
        .set_unit_dice(id, Dice::seeded([safe_seed; 32]))
        .unwrap();
    let fall = resolve_battle_fall(&mut world, id, 1, rules()).unwrap();
    assert!(fall.avoidance.unwrap().success);
    for success in [true, false] {
        let seed = (0..=255)
            .find(|seed| (Dice::seeded([*seed; 32]).two_d6() >= 6) == success)
            .unwrap();
        let mut candidate = world.clone();
        candidate
            .btech
            .set_unit_dice(id, Dice::seeded([seed; 32]))
            .unwrap();
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
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["ground_elevation"] = serde_json::json!(0);
            record["dice"] = serde_json::to_value(Dice::seeded([3; 32])).unwrap();
        })
        .unwrap();
    let fall = resolve_battle_fall(&mut world, id, 1, rules()).unwrap();
    assert_eq!(fall.damage, 2);
    assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(-1));
    assert_eq!(
        world.btech.constructed_units()[&id]
            .heat_rates(&world)
            .dissipation,
        16.0
    );
    let section = MechSection::LeftArm;
    let armor = world.btech.constructed_units()[&id].sections()[&section].armor;
    if armor > 0 {
        let _damage = apply_damage_phase(
            &mut world,
            id,
            section,
            armor,
            DamagePhase::Armor { rear: false },
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
        Hit {
            section,
            rear_armor: false,
            through_armor_critical: false,
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

/// A bridge deck is the crossed hex's surface: a deck above Jumping MP plus the takeoff
/// level refuses the jump at launch (Total Warfare p. 52), and a deck exactly that high is
/// cleared by the displayed flight and landed beyond.
#[tokio::test]
async fn bridge_decks_bound_jump_routes_at_launch_and_legal_spans_are_cleared() {
    for (deck, clears) in [(9, false), (6, false), (5, true)] {
        let source = format!("3 5\n.0.0.0\n.0.0.0\n/{deck}/{deck}/{deck}\n.0.0.0\n.0.0.0\n");
        let (_dir, config, mut world, _, units) =
            fixture_asset(MapAsset::from_cells(&source).unwrap()).await;
        let id = units[0];
        let before = world.btech.clone();
        let launch = launch_battle_jump(&mut world, id, ObjectId(1), 180, 2.0);
        if !clears {
            let error = launch.unwrap_err();
            assert!(format!("{error:#}").contains("higher than your jets can clear"));
            assert_eq!(world.btech, before);
            continue;
        }
        launch.unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        let mut crossed = false;
        while world.btech.constructed_units()[&id].flight().is_some() {
            let expected = advance_battle_jumps(
                &mut world,
                stompymux_rs::MovementRules {
                    fall: rules(),
                    ..stompymux_rs::MovementRules::STANDARD
                },
            )
            .unwrap();
            assert_eq!(
                advance_battle_jumps(
                    &mut loaded,
                    stompymux_rs::MovementRules {
                        fall: rules(),
                        ..stompymux_rs::MovementRules::STANDARD
                    }
                )
                .unwrap(),
                expected
            );
            assert_eq!(world.btech, loaded.btech);
            let unit = &world.btech.constructed_units()[&id];
            if unit.position().unwrap().y == 2
                && let Some(flight) = unit.flight()
            {
                crossed = true;
                assert!(flight.sample().elevation >= f64::from(deck));
                assert!(battle_unit_elevation(&world, id).unwrap().unwrap() >= deck);
            }
        }
        assert!(crossed);
        let unit = &world.btech.constructed_units()[&id];
        assert_eq!(unit.position().unwrap().y, 3);
        assert_eq!(unit.posture(), Posture::Standing);
        assert!(!unit.hex_sync_pending());
        world.validate(&config).unwrap();
    }
}

/// A 'Mech standing beneath a bridge deck has a solid obstacle directly overhead and is
/// refused a jump before it leaves the ground.
#[tokio::test]
async fn jumps_are_refused_beneath_a_bridge_deck() {
    let (_dir, config, mut world, _, units) = fixture_surface(Terrain::Bridge, 3).await;
    let id = units[0];
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][id.0.to_string()]["ground_elevation"] = serde_json::json!(-1);
    world.btech = serde_json::from_value(state).unwrap();
    let before = world.btech.clone();
    let error = launch_battle_jump(&mut world, id, ObjectId(1), 0, 1.0).unwrap_err();
    assert!(format!("{error:#}").contains("bridge overhead"));
    assert_eq!(world.btech, before);
    assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(-1));
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn bridge_deck_jumps_and_early_landings_share_native_lua_state() {
    for deck in [0, 3, 9] {
        for early in [false, true] {
            let (_dir, config, mut world, _map, units) =
                fixture_surface(Terrain::Bridge, deck).await;
            let id = units[0];
            // An early landing comes down in the starting hex, which only one 'Mech may hold.
            world
                .btech
                .rewrite_unit_record(units[1], |record| {
                    record["position"]["x"] = 0.into();
                    record["position"]["y"] = 0.into();
                    record["motion"]["point"] =
                        serde_json::to_value(HexCoordinate { x: 0, y: 0 }.center()).unwrap();
                })
                .unwrap();
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
                    stompymux_rs::MovementRules {
                        fall: rules(),
                        ..stompymux_rs::MovementRules::STANDARD
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
                    .find(|seed| Dice::seeded([*seed; 32]).two_d6() >= 6)
                    .unwrap();
                current
                    .btech
                    .set_unit_dice(id, Dice::seeded([seed; 32]))
                    .unwrap();
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
                            stompymux_rs::MovementRules {
                                fall: rules(),
                                ..stompymux_rs::MovementRules::STANDARD
                            }
                        )
                        .unwrap(),
                        advance_battle_jumps(
                            &mut loaded,
                            stompymux_rs::MovementRules {
                                fall: rules(),
                                ..stompymux_rs::MovementRules::STANDARD
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
            assert_eq!(unit.posture(), Posture::Standing);
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
                fixture_asset(MapAsset::from_cells(&source).unwrap()).await;
            let id = units[0];
            let heading = if reverse { 0.0 } else { 180.0 };
            let speed = if reverse { -21.5 } else { 21.5 };
            world
                .btech
                .rewrite_unit_record(id, |record| {
                    record["motion"]["heading"] = serde_json::json!(heading);
                    record["motion"]["desired_heading"] = serde_json::json!(heading);
                })
                .unwrap();
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
            let motion_rules = MovementRules::STANDARD;
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
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["ground_elevation"] = serde_json::json!(-1);
        })
        .unwrap();
    let point = world.btech.constructed_units()[&id].motion().unwrap().point;
    set_battle_speed(&mut world, id, ObjectId(1), 21.5).unwrap();
    let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
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
        // Buildings are entered, not climbed; see btech_building_interior for them.
        for symbol in ['.', '/', '='] {
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
                fixture_asset(MapAsset::from_cells(&format!("3 5\n{rows}")).unwrap()).await;
            let id = units[0];
            world
                .btech
                .rewrite_unit_record(id, |record| {
                    record["motion"]["heading"] = serde_json::json!(180.0);
                    record["motion"]["desired_heading"] = serde_json::json!(180.0);
                })
                .unwrap();
            set_battle_speed(&mut world, id, ObjectId(1), 21.5).unwrap();
            let dice = serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"]
                .clone();
            let motion_rules = MovementRules::STANDARD;
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
                    // One level costs one MP, charged like the hex's own terrain cost: the
                    // unit keeps its speed and crosses the hex a hex-width slower.
                    assert_eq!(unit.position().unwrap().y, 2);
                    assert_eq!(unit.motion().unwrap().speed, 21.5);
                    assert_eq!(unit.motion().unwrap().desired_speed, 21.5);
                    assert_eq!(unit.level_entry_cost(), 1);
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
                    assert_eq!(unit.motion().unwrap().desired_speed, 21.5);
                    assert_eq!(unit.level_entry_cost(), 2);
                    // The two-level step costs two hex-widths of travel: a third of the
                    // throttle through a clear hex.
                    let proposal = propose_battle_mech_ground_motion(
                        &world,
                        id,
                        unit.motion().unwrap(),
                        HexCoordinate { x: 1, y: 3 },
                        motion_rules,
                    )
                    .unwrap();
                    assert!(proposal.motion.speed < unit.motion().unwrap().speed);
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
        fixture_asset(MapAsset::from_cells(source).unwrap()).await;
    let id = units[0];
    world
        .btech
        .rewrite_unit_record(id, |record| {
            let unit = record;
            unit["ground_elevation"] = serde_json::json!(5);
            unit["hex_sync_pending"] = serde_json::json!(true);
            unit["motion"]["point"] =
                serde_json::to_value(HexCoordinate { x: 1, y: 2 }.center()).unwrap();
        })
        .unwrap();
    world.validate(&config).unwrap();
    set_battle_heading(&mut world, id, ObjectId(1), 90.0).unwrap();
    assert!(
        advance_battle_motion(&mut world, MovementRules::STANDARD)
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
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == roll)
        .unwrap();
    world
        .btech
        .rewrite_unit_record(id, |record| {
            let unit = record;
            unit["motion"]["point"] = serde_json::to_value(Point {
                y: 1.49,
                ..HexCoordinate { x: 1, y: 1 }.center()
            })
            .unwrap();
            unit["motion"]["speed"] = serde_json::json!(-21.5);
            unit["motion"]["desired_speed"] = serde_json::json!(-21.5);
            unit["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
        })
        .unwrap();
}

/// One shard per terrain symbol; each elevation change owns one fixture that the
/// rule/success combinations clone, keeping the lockstep replay twin per combination.
async fn reverse_ground_steps_matrix(symbol: char) {
    for change in [-2i16, -1, 1, 2] {
        let destination = 3 + change;
        let row = format!("{symbol}{destination}").repeat(3);
        let source = format!("3 4\n.3.3.3\n.3.3.3\n{row}\n{row}\n");
        let (_dir, config, base, _, units) =
            fixture_asset(MapAsset::from_cells(&source).unwrap()).await;
        let id = units[0];
        let mut probed_fidelity = [false; 2];
        for (shape, success) in [true, false].into_iter().enumerate() {
            let mut world = base.clone();
            let target = 6 + change.unsigned_abs() as u8 - 1;
            prepare_reverse_step(&mut world, id, if success { target } else { target - 1 });
            world.validate(&config).unwrap();
            let before = world.clone();
            persistence::save(&config.database(), &world).await.unwrap();
            let mut loaded = persistence::load(&config.database()).await.unwrap();
            let movement = MovementRules::STANDARD;
            let notices = advance_battle_motion(&mut world, movement).unwrap();
            assert_eq!(
                notices,
                advance_battle_motion(&mut loaded, movement).unwrap()
            );
            assert_eq!(world.btech, loaded.btech);
            let unit = &world.btech.constructed_units()[&id];
            if success {
                assert_eq!(unit.position().unwrap().y, 2);
                assert_eq!(unit.posture(), Posture::Standing);
                assert_eq!(unit.motion().unwrap().desired_speed, -21.5);
                assert_eq!(unit.motion().unwrap().speed, -21.5);
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
                assert_eq!(unit.posture(), Posture::Prone);
                let protection = |unit: &Mech| {
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
async fn reverse_ground_steps_apply_configured_checks_falls_and_restart_replay_equals() {
    reverse_ground_steps_matrix('=').await;
}

#[tokio::test]
async fn failed_reverse_fall_restores_the_entire_movement_tick() {
    let source = "3 4\n.0.0.0\n.0.0.0\n.1.1.1\n.1.1.1\n";
    let (_dir, config, mut world, _, units) =
        fixture_asset(MapAsset::from_cells(source).unwrap()).await;
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
        advance_battle_motion(&mut world, MovementRules::STANDARD)
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
        fixture_asset(MapAsset::from_cells(source).unwrap()).await;
    let id = units[0];
    prepare_reverse_step(&mut world, id, 2);
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["pilot"] = serde_json::Value::Null;
        })
        .unwrap();
    let dice = serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"].clone();
    let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
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

/// One shard per terrain symbol and slope direction owns a single fixture; every speed clones
/// it and keeps the lockstep replay twin. A change of more than two levels is prohibited
/// terrain (Total Warfare p. 48): the 'Mech stops at the edge without a roll.
async fn cliffs_apply_speed_checks_matrix(symbol: char, downhill: bool) {
    let (old, new) = if downhill { (3, 0) } else { (0, 3) };
    let row = format!("{symbol}{new}").repeat(3);
    let old_row = format!(".{old}").repeat(3);
    let source = format!("3 4\n{old_row}\n{old_row}\n{row}\n{row}\n");
    let (_dir, config, base, _, units) =
        fixture_asset(MapAsset::from_cells(&source).unwrap()).await;
    let id = units[0];
    let mut probed_fidelity = false;
    for speed in [-53.75f64, -21.5, 21.5, 53.75] {
        let mut world = base.clone();
        prepare_reverse_step(&mut world, id, 12);
        world
            .btech
            .rewrite_unit_record(id, |record| {
                let motion = &mut record["motion"];
                motion["speed"] = serde_json::json!(speed);
                motion["desired_speed"] = serde_json::json!(speed);
                motion["heading"] = serde_json::json!(if speed > 0.0 { 180.0 } else { 0.0 });
                motion["desired_heading"] = motion["heading"].clone();
            })
            .unwrap();
        world.validate(&config).unwrap();
        let before = world.clone();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
        assert_eq!(
            notices,
            advance_battle_motion(&mut loaded, MovementRules::STANDARD).unwrap()
        );
        assert_eq!(world.btech, loaded.btech);
        let unit = &world.btech.constructed_units()[&id];
        assert!(
            notices.iter().any(|notice| notice.text
                == if downhill {
                    "The drop ahead is too steep to walk down."
                } else {
                    "The slope ahead is too steep to climb."
                }),
            "{symbol}{new} downhill={downhill} speed={speed}: {notices:?}"
        );
        assert_eq!(unit.posture(), Posture::Standing);
        assert_eq!(unit.position().unwrap().y, 1);
        assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(old));
        assert_eq!(
            unit.motion().unwrap().point,
            before.btech.constructed_units()[&id]
                .motion()
                .unwrap()
                .point
        );
        assert_eq!(unit.motion().unwrap().speed, 0.0);
        assert_eq!(unit.motion().unwrap().desired_speed, 0.0);
        assert_eq!(
            serde_json::to_value(unit).unwrap()["dice"],
            serde_json::to_value(&before.btech.constructed_units()[&id]).unwrap()["dice"]
        );
        assert_eq!(
            unit.sections(),
            before.btech.constructed_units()[&id].sections()
        );
        world.validate(&config).unwrap();
        if !probed_fidelity {
            probed_fidelity = true;
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
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

/// A span three levels overhead is not a cliff: a 'Mech on the bank walks beneath it onto
/// the river bed one level down, taking the water check, or the reverse-slope check when it
/// backs in.
#[tokio::test]
async fn tall_bridge_approached_from_the_bank_is_an_underpass_not_a_cliff() {
    let source = "3 4\n.0.0.0\n.0.0.0\n/3/3/3\n/3/3/3\n";
    let (_dir, config, base, _, units) = fixture_asset(MapAsset::from_cells(source).unwrap()).await;
    let id = units[0];
    for speed in [-53.75f64, -21.5, 21.5, 53.75] {
        let mut world = base.clone();
        prepare_reverse_step(&mut world, id, 12);
        world
            .btech
            .rewrite_unit_record(id, |record| {
                let motion = &mut record["motion"];
                motion["speed"] = serde_json::json!(speed);
                motion["desired_speed"] = serde_json::json!(speed);
                motion["heading"] = serde_json::json!(if speed > 0.0 { 180.0 } else { 0.0 });
                motion["desired_heading"] = motion["heading"].clone();
            })
            .unwrap();
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
        assert_eq!(
            notices,
            advance_battle_motion(&mut loaded, MovementRules::STANDARD).unwrap()
        );
        assert_eq!(world.btech, loaded.btech);
        let expected = if speed < 0.0 {
            "You manage to overcome the obstacle."
        } else {
            "You use your piloting skill to maneuver through the water."
        };
        assert!(
            notices.iter().any(|notice| notice.text == expected),
            "speed={speed}: {notices:?}"
        );
        let unit = &world.btech.constructed_units()[&id];
        assert_eq!(unit.position().unwrap().y, 2, "speed={speed}");
        assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(-1));
        assert_eq!(unit.posture(), Posture::Standing);
        assert_eq!(unit.motion().unwrap().speed, speed);
        world.validate(&config).unwrap();
    }
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
async fn a_cliff_stops_unpiloted_motion_without_consuming_dice() {
    for downhill in [false, true] {
        let (old, new) = if downhill { (9, 0) } else { (0, 9) };
        let old_row = format!(".{old}").repeat(3);
        let row = format!(".{new}").repeat(3);
        let source = format!("3 4\n{old_row}\n{old_row}\n{row}\n{row}\n");
        let (_dir, config, mut world, _, units) =
            fixture_asset(MapAsset::from_cells(&source).unwrap()).await;
        let id = units[0];
        prepare_reverse_step(&mut world, id, 2);
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][id.0.to_string()]["pilot"] = serde_json::Value::Null;
        world.btech = serde_json::from_value(state).unwrap();
        let before = world.clone();
        let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
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
                .any(|notice| notice.text.contains("too steep"))
        );
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn autofall_native_lua_controls_are_atomic_and_survive_restart_and_shutdown() {
    let (_dir, config, world, _, units) = fixture_surface(Terrain::Clear, 0).await;
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
        support::run_text(&native, &config, ObjectId(1), 1, "unitprefs").contains("AutoFall: OFF")
    );
    assert!(
        support::run_text(&native, &config, ObjectId(1), 1, "unitprefs aUtOfAlL on")
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
        "unitprefs AutoFall maybe",
        "unitprefs AutoFall on extra",
        "unitprefs Unknown on",
        "unitprefs/bad AutoFall off",
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
    stop_battle_unit(&mut saved, id, ObjectId(1)).unwrap();
    assert!(saved.btech.constructed_units()[&id].auto_fall());
    assign_battle_pilot(&mut saved, id, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut saved, ObjectId(1), support::FIXTURE_DICE_SEED);
    set_battle_auto_fall(&mut saved, id, ObjectId(1), false).unwrap();
    assert!(!saved.btech.constructed_units()[&id].auto_fall());
    set_battle_auto_fall(&mut saved, id, ObjectId(1), true).unwrap();
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
    assert!(
        support::run_text(&native, &config, ObjectId(1), 1, "unitprefs AutoFall")
            .contains("toggled OFF")
    );
    assert!(!native.world().btech.constructed_units()[&id].auto_fall());
    saved.validate(&config).unwrap();
}

#[tokio::test]
async fn autofall_changes_nothing_at_a_slope_too_steep_to_enter() {
    for downhill in [false, true] {
        for piloted in [false, true] {
            let (old, new) = if downhill { (3, 0) } else { (0, 3) };
            let old_row = format!(".{old}").repeat(3);
            let row = format!(".{new}").repeat(3);
            let source = format!("3 4\n{old_row}\n{old_row}\n{row}\n{row}\n");
            let (_dir, config, mut world, _, units) =
                fixture_asset(MapAsset::from_cells(&source).unwrap()).await;
            let id = units[0];
            prepare_reverse_step(&mut world, id, 12);
            set_battle_auto_fall(&mut world, id, ObjectId(1), true).unwrap();
            if !piloted {
                world
                    .btech
                    .rewrite_unit_record(id, |record| {
                        record["pilot"] = serde_json::Value::Null;
                    })
                    .unwrap();
            }
            persistence::save(&config.database(), &world).await.unwrap();
            let mut loaded = persistence::load(&config.database()).await.unwrap();
            // With the preference off the outcome is the same: a stop at the edge.
            let mut expected = world.clone();
            expected
                .btech
                .rewrite_unit_record(id, |record| {
                    record["auto_fall"] = serde_json::json!(false);
                })
                .unwrap();
            advance_battle_motion(&mut expected, MovementRules::STANDARD).unwrap();
            expected
                .btech
                .rewrite_unit_record(id, |record| {
                    record["auto_fall"] = serde_json::json!(true);
                })
                .unwrap();
            let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
            assert_eq!(
                notices,
                advance_battle_motion(&mut loaded, MovementRules::STANDARD).unwrap()
            );
            assert_eq!(world.btech, loaded.btech);
            assert_eq!(world.btech, expected.btech);
            let unit = &world.btech.constructed_units()[&id];
            assert_eq!(unit.posture(), Posture::Standing);
            assert_eq!(unit.position().unwrap().y, 1);
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
            let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
            assert_eq!(
                notices,
                advance_battle_motion(&mut loaded, MovementRules::STANDARD).unwrap()
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
                    Posture::Standing
                } else {
                    Posture::Prone
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
                    advance_battle_motion(&mut world, MovementRules::STANDARD)
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
async fn running_into_water_stops_at_the_edge() {
    for depth in [1, 2] {
        let row = format!("~{depth}").repeat(3);
        let source = format!("3 4\n.0.0.0\n.0.0.0\n{row}\n{row}\n");
        let (_dir, config, mut world, _, units) =
            fixture_asset(MapAsset::from_cells(&source).unwrap()).await;
        let id = units[0];
        prepare_reverse_step(&mut world, id, 6 + depth);
        let maximum = world.btech.constructed_units()[&id]
            .mobility()
            .maximum_speed;
        world
            .btech
            .rewrite_unit_record(id, |record| {
                let motion = &mut record["motion"];
                motion["heading"] = serde_json::json!(180.0);
                motion["desired_heading"] = serde_json::json!(180.0);
                motion["speed"] = serde_json::json!(maximum);
                motion["desired_speed"] = serde_json::json!(maximum);
            })
            .unwrap();
        let before = world.clone();
        let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
        // A running 'Mech may not enter water (Total Warfare p. 54); it stops at the bank.
        assert!(
            notices
                .iter()
                .any(|notice| notice.text == "You cannot run into the water!")
        );
        let unit = &world.btech.constructed_units()[&id];
        assert_eq!(unit.posture(), Posture::Standing);
        assert_eq!(unit.position().unwrap().y, 1);
        assert_eq!(unit.motion().unwrap().speed, 0.0);
        assert_eq!(unit.motion().unwrap().desired_speed, 0.0);
        assert_eq!(
            serde_json::to_value(unit).unwrap()["dice"],
            serde_json::to_value(&before.btech.constructed_units()[&id]).unwrap()["dice"]
        );
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn entering_water_floods_a_breached_leg_and_keeps_the_fall_stopped() {
    let source = "3 4\n.0.0.0\n.0.0.0\n~1~1~1\n~1~1~1\n";
    let (_dir, config, mut world, _, units) =
        fixture_asset(MapAsset::from_cells(source).unwrap()).await;
    let id = units[0];
    let leg = MechSection::LeftLeg;
    let armor = world.btech.constructed_units()[&id].sections()[&leg].armor;
    let _damage = apply_damage_phase(
        &mut world,
        id,
        leg,
        armor,
        DamagePhase::Armor { rear: false },
    )
    .unwrap();
    prepare_reverse_step(&mut world, id, 12);
    let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
    let unit = &world.btech.constructed_units()[&id];
    assert!(unit.flooded_sections().contains(&leg));
    assert_eq!(unit.posture(), Posture::Prone);
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
                    advance_battle_motion(&mut world, MovementRules::STANDARD)
                        .unwrap()
                        .is_empty()
                );
            }
            let unit = &world.btech.constructed_units()[&id];
            // Water a level deep costs one extra movement point and deeper water three.
            let cost = match depth {
                0 => 1.0,
                1 => 2.0,
                _ => 4.0,
            };
            assert_eq!(unit.motion().unwrap().speed, speed / cost);
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
    assert!(advance_battle_motion(&mut world, MovementRules::STANDARD).is_err());
    assert_eq!(world.btech, before.btech);
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn leaving_shallow_water_restores_land_height_and_charges_the_upward_step() {
    let source = "3 4\n~1~1~1\n~1~1~1\n.0.0.0\n.0.0.0\n";
    let (_dir, config, mut world, _, units) =
        fixture_asset(MapAsset::from_cells(source).unwrap()).await;
    let id = units[0];
    prepare_reverse_step(&mut world, id, 12);
    world
        .btech
        .rewrite_unit_record(id, |record| {
            let motion = &mut record["motion"];
            motion["heading"] = serde_json::json!(180.0);
            motion["desired_heading"] = serde_json::json!(180.0);
            motion["speed"] = serde_json::json!(21.5);
            motion["desired_speed"] = serde_json::json!(21.5);
        })
        .unwrap();
    let before = world.clone();
    assert!(
        advance_battle_motion(&mut world, MovementRules::STANDARD)
            .unwrap()
            .is_empty()
    );
    let unit = &world.btech.constructed_units()[&id];
    assert_eq!(unit.position().unwrap().y, 2);
    assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(0));
    assert_eq!(unit.motion().unwrap().desired_speed, 21.5);
    // The water hex's cost slows the step; the climb out is a level charged on the new hex.
    let proposed = propose_battle_mech_ground_motion(
        &before,
        id,
        before.btech.constructed_units()[&id].motion().unwrap(),
        HexCoordinate { x: 1, y: 1 },
        MovementRules::STANDARD,
    )
    .unwrap();
    assert!((unit.motion().unwrap().speed - proposed.motion.speed).abs() < 1e-10);
    assert_eq!(unit.level_entry_cost(), 1);
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
                fixture_asset(MapAsset::from_cells(&source).unwrap()).await;
            let id = units[0];
            prepare_reverse_step(&mut world, id, 12);
            world
                .btech
                .rewrite_unit_record(id, |record| {
                    let unit = record;
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
                })
                .unwrap();
            let before = world.clone();
            persistence::save(&config.database(), &world).await.unwrap();
            let mut loaded = persistence::load(&config.database()).await.unwrap();
            let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
            assert_eq!(
                notices,
                advance_battle_motion(&mut loaded, MovementRules::STANDARD).unwrap()
            );
            assert_eq!(world.btech, loaded.btech);
            let row = format!("{reverse} {old_deck} {old_height} {terrain:?} {new_deck} {height}");
            let unit = &world.btech.constructed_units()[&id];
            assert_eq!(unit.position().unwrap().y, 2, "{row}: {notices:?}");
            assert_eq!(
                battle_unit_elevation(&world, id).unwrap(),
                Some(height),
                "{row}"
            );
            assert_eq!(unit.posture(), Posture::Standing, "{row}");
            let map_height = if terrain == Terrain::Water {
                -new_deck
            } else {
                new_deck
            };
            // Beneath a span the unit stands on the river bed, so the change is measured
            // between the heights it stands at, not between the decks overhead.
            let change: i32 = height - old_height;
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
            // Levels climbed to the hex surface are charged on the new hex, forward or backward;
            // a change of river-bed height beneath a span is not a level change of the hex.
            let levels = if height == map_height {
                change.unsigned_abs() as u8
            } else {
                0
            };
            assert_eq!(unit.motion().unwrap().speed.abs(), 21.5, "{row}");
            assert_eq!(unit.level_entry_cost(), levels, "{row}");
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
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["ground_elevation"] = serde_json::json!(-1);
        })
        .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
    assert_eq!(
        notices,
        advance_battle_motion(&mut loaded, MovementRules::STANDARD).unwrap()
    );
    assert_eq!(world.btech, loaded.btech);
    assert!(
        notices
            .iter()
            .any(|notice| notice.text == "You slip in the water and fall down")
    );
    let unit = &world.btech.constructed_units()[&id];
    assert_eq!(unit.position().unwrap().y, 2);
    assert_eq!(unit.posture(), Posture::Prone);
    assert_eq!(unit.motion().unwrap().speed, 0.0);
    assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(-1));
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn exiting_below_bridge_into_water_of_the_same_depth_takes_only_the_water_check() {
    let source = "3 4\n/3/3/3\n/3/3/3\n~1~1~1\n~1~1~1\n";
    let (_dir, config, mut world, _, units) =
        fixture_asset(MapAsset::from_cells(source).unwrap()).await;
    let id = units[0];
    prepare_reverse_step(&mut world, id, 12);
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["ground_elevation"] = serde_json::json!(-1);
        })
        .unwrap();
    let before = world.clone();
    let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
    // The deck above is not the surface the unit stands on, so leaving the span is level
    // ground and only the water it enters is checked.
    let unit = &world.btech.constructed_units()[&id];
    assert_eq!(unit.position().unwrap().y, 2);
    assert_eq!(unit.posture(), Posture::Standing);
    assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(-1));
    assert!(
        notices
            .iter()
            .any(|notice| notice.text
                == "You use your piloting skill to maneuver through the water.")
    );
    assert!(
        !notices
            .iter()
            .any(|notice| notice.text.contains("behind you"))
    );
    let mut expected = before;
    assert!(
        roll_battle_piloting(&mut expected, id, -1, true)
            .unwrap()
            .success
    );
    assert_eq!(
        serde_json::to_value(unit).unwrap()["dice"],
        serde_json::to_value(&expected.btech.constructed_units()[&id]).unwrap()["dice"]
    );
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn ground_ice_entry_fractures_neighbors_and_replays_without_repeated_checks() {
    for depth in [0, 1, 3, 9] {
        for fracture in [false, true] {
            for reverse in [false, true] {
                let (_dir, config, mut world, map, units) =
                    fixture_field(Terrain::Ice, depth, 5).await;
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
                neighbor["dice"] = serde_json::to_value(Dice::seeded([19; 32])).unwrap();
                neighbor["position"]["y"] = serde_json::json!(2);
                neighbor["motion"]["point"] =
                    serde_json::to_value(HexCoordinate { x: 1, y: 2 }.center()).unwrap();
                world.btech = serde_json::from_value(state).unwrap();
                ice_seed(&mut world, id, fracture, false);
                let before = world.clone();
                persistence::save(&config.database(), &world).await.unwrap();
                let mut loaded = persistence::load(&config.database()).await.unwrap();
                let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
                assert_eq!(
                    notices,
                    advance_battle_motion(&mut loaded, MovementRules::STANDARD).unwrap()
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
                            Posture::Prone
                        } else {
                            Posture::Standing
                        }
                    );
                    assert_eq!(
                        battle_unit_elevation(&world, unit).unwrap(),
                        Some(if fracture { -i32::from(depth) } else { 0 })
                    );
                }
                if !fracture || depth == 0 {
                    let mut dice: Dice = serde_json::from_value(serde_json::to_value(&before.btech.constructed_units()[&id]).unwrap()["dice"].clone()).unwrap();
                    assert_eq!(dice.d6() == 1, fracture);
                    if !fracture {
                        // The ice held, so the 'Mech rolled at +0 to keep its feet on it.
                        dice.two_d6();
                    }
                    assert_eq!(
                        serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"],
                        serde_json::to_value(&dice).unwrap()
                    );
                    assert!(
                        advance_battle_motion(&mut world, MovementRules::STANDARD)
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
                let saved = persistence::load(&config.database()).await.unwrap();
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
            fixture_asset(MapAsset::from_cells(&source).unwrap()).await;
        let id = units[0];
        prepare_reverse_step(&mut world, id, 12);
        world
            .btech
            .rewrite_unit_record(id, |record| {
                let unit = record;
                if old_terrain == Terrain::Ice {
                    unit["ground_elevation"] = serde_json::json!(-old_depth);
                }
                unit["motion"]["heading"] = serde_json::json!(180.0);
                unit["motion"]["desired_heading"] = serde_json::json!(180.0);
                unit["motion"]["speed"] = serde_json::json!(21.5);
                unit["motion"]["desired_speed"] = serde_json::json!(21.5);
            })
            .unwrap();
        let before = world.clone();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
        assert_eq!(
            notices,
            advance_battle_motion(&mut loaded, MovementRules::STANDARD).unwrap()
        );
        assert_eq!(world.btech, loaded.btech);
        assert_eq!(
            world.btech.maps()[&map].hex(1, 2).unwrap().terrain(),
            new_terrain
        );
        assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(height));
        // Ice and deep water cost movement now; the climb between beds costs a level.
        let proposed = propose_battle_mech_ground_motion(
            &before,
            id,
            before.btech.constructed_units()[&id].motion().unwrap(),
            HexCoordinate { x: 1, y: 1 },
            MovementRules::STANDARD,
        )
        .unwrap();
        // The speed column encodes the levels climbed between beds, charged on the new hex;
        // ice and water costs come from the proposal.
        let levels = ((21.5 - speed) / 10.75_f64).round() as u8;
        let tile = world.btech.maps()[&map].base_hex(1, 2).unwrap();
        assert!(
            (world.btech.constructed_units()[&id].motion().unwrap().speed - proposed.motion.speed)
                .abs()
                < 1e-10,
            "{old_terrain:?}{old_depth} -> {new_terrain:?}{new_depth}"
        );
        assert_eq!(
            world.btech.constructed_units()[&id].level_entry_cost(),
            levels,
            "{old_terrain:?}{old_depth} -> {new_terrain:?}{new_depth}"
        );
        let mut expected = before;
        if let Some(modifier) = modifier {
            assert!(
                roll_battle_piloting(&mut expected, id, modifier, true)
                    .unwrap()
                    .success
            );
        }
        if height >= i32::from(tile.water_line()) && tile.is_ice() {
            // Stepping onto the ice sheet rolls at +0 against slipping.
            let _footing = roll_battle_piloting(&mut expected, id, 0, true).unwrap();
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
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["ground_elevation"] = serde_json::json!(-3);
            // Fail water control but protect the pilot, then hit a torso rather than the head.
            let fall_seed = (0..=255)
                .find(|seed| {
                    let mut dice = Dice::seeded([*seed; 32]);
                    dice.two_d6() == 6 && dice.two_d6() >= 7 && {
                        dice.d6();
                        dice.two_d6() == 7
                    }
                })
                .unwrap();
            record["dice"] = serde_json::to_value(Dice::seeded([fall_seed; 32])).unwrap();
        })
        .unwrap();
    let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
    assert!(
        notices
            .iter()
            .any(|notice| notice.text == "You slip in the water and fall down")
    );
    assert_eq!(
        world.btech.constructed_units()[&id].posture(),
        Posture::Prone
    );
    assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(-3));
    assert_eq!(
        world.btech.maps()[&map].hex(1, 2).unwrap().terrain(),
        Terrain::Ice
    );
    let seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 12)
        .unwrap();
    world
        .btech
        .set_unit_dice(id, Dice::seeded([seed; 32]))
        .unwrap();
    let report =
        begin_battle_stand(&mut world, id, ObjectId(1), StandMode::Normal, rules()).unwrap();
    assert!(report.check.success);
    for _ in 0..60 {
        advance_battle_standing(&mut world);
    }
    assert_eq!(
        world.btech.constructed_units()[&id].posture(),
        Posture::Standing
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
    world
        .btech
        .rewrite_unit_record(units[1], |record| {
            let neighbor = record;
            neighbor["position"]["y"] = serde_json::json!(2);
            neighbor["motion"]["point"] =
                serde_json::to_value(HexCoordinate { x: 1, y: 2 }.center()).unwrap();
        })
        .unwrap();
    world
        .objects
        .get_mut(&units[1])
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    ice_seed(&mut world, id, true, false);
    let before = world.clone();
    assert!(advance_battle_motion(&mut world, MovementRules::STANDARD).is_err());
    assert_eq!(world.btech, before.btech);
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn descending_jump_breaks_previous_ice_before_finishing_horizontal_entry() {
    let source = "3 5\n.0.0.0\n.0.0.0\n-3-3-3\n~5~5~5\n~5~5~5\n";
    let (_dir, config, mut world, map, units) =
        fixture_asset(MapAsset::from_cells(source).unwrap()).await;
    let id = units[0];
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][units[1].0.to_string()]["position"]["y"] = serde_json::json!(2);
    state["constructed"][units[1].0.to_string()]["motion"]["point"] =
        serde_json::to_value(HexCoordinate { x: 1, y: 2 }.center()).unwrap();
    // Protect both pilots and avoid random critical cascades in this geometry fixture.
    let safe = (0..=255)
        .find(|seed| {
            let mut dice = Dice::seeded([*seed; 32]);
            let protects = dice.two_d6() >= 7;
            dice.d6();
            protects && dice.two_d6() == 7 && dice.two_d6() >= 7
        })
        .unwrap();
    for unit in units {
        state["constructed"][unit.0.to_string()]["dice"] =
            serde_json::to_value(Dice::seeded([safe; 32])).unwrap();
    }
    world.btech = serde_json::from_value(state).unwrap();
    launch_battle_jump(&mut world, id, ObjectId(1), 180, 2.0).unwrap();
    // At a twelfth of standard gravity the jets carry the 'Mech over two-thirds of a hex each
    // second, so its landing second starts above the ice and comes down in the water beyond.
    world
        .btech
        .rewrite_map_record(map, |record| record["gravity"] = 12.into())
        .unwrap();
    for _ in 0..2 {
        assert!(
            advance_battle_jumps(
                &mut world,
                stompymux_rs::MovementRules {
                    fall: rules(),
                    ..stompymux_rs::MovementRules::STANDARD
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
            stompymux_rs::MovementRules {
                fall: rules(),
                ..stompymux_rs::MovementRules::STANDARD
            }
        )
        .is_err()
    );
    assert_eq!(rejected.btech, rejected_before);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    let notices = advance_battle_jumps(
        &mut world,
        stompymux_rs::MovementRules {
            fall: rules(),
            ..stompymux_rs::MovementRules::STANDARD
        },
    )
    .unwrap();
    assert_eq!(
        notices,
        advance_battle_jumps(
            &mut loaded,
            stompymux_rs::MovementRules {
                fall: rules(),
                ..stompymux_rs::MovementRules::STANDARD
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
            Posture::Prone
        );
    }
    let unit = &world.btech.constructed_units()[&id];
    assert!(unit.flight().is_none());
    assert!(!unit.hex_sync_pending());
    assert_eq!(unit.position().unwrap().y, 3);
    assert_eq!(
        unit.motion().unwrap().point,
        HexCoordinate { x: 1, y: 3 }.center()
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
    let (_dir, config, mut world, map, units) = airborne_under_ice_fixture(UnderIce::Rising).await;
    let observer = fracture_observer(&mut world, &config, map, units);
    let id = units[0];
    let dice = serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"].clone();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    let notices = advance_battle_jumps(
        &mut world,
        stompymux_rs::MovementRules {
            fall: rules(),
            ..stompymux_rs::MovementRules::STANDARD
        },
    )
    .unwrap();
    assert_eq!(
        notices,
        advance_battle_jumps(
            &mut loaded,
            stompymux_rs::MovementRules {
                fall: rules(),
                ..stompymux_rs::MovementRules::STANDARD
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
        Posture::Standing
    );
    assert_eq!(
        serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"],
        dice
    );
    assert_eq!(
        world.btech.constructed_units()[&units[1]].posture(),
        Posture::Prone
    );
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

/// What the under-ice fixture's next jump second does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnderIce {
    /// The flight climbs back through the ice without landing.
    Rising,
    /// The flight lands in the iced hex.
    Landing,
}

/// A valid saved flight below ice, with a surface neighbor.
async fn airborne_under_ice_fixture(
    next: UnderIce,
) -> (tempfile::TempDir, Config, World, ObjectId, [ObjectId; 2]) {
    let source = "3 5\n.0.0.0\n.0.0.0\n.0.0.0\n~5~5~5\n~5~5~5\n";
    let (_dir, config, mut world, map, units) =
        fixture_asset(MapAsset::from_cells(source).unwrap()).await;
    let id = units[0];
    launch_battle_jump(&mut world, id, ObjectId(1), 180, 2.0).unwrap();
    match next {
        UnderIce::Landing => fly_until_landing_is_next(&mut world, id),
        UnderIce::Rising => {
            // Twenty-one seconds of standard gravity bring the 'Mech down over the water.
            for _ in 0..21 {
                assert!(
                    advance_battle_jumps(
                        &mut world,
                        stompymux_rs::MovementRules {
                            fall: rules(),
                            ..stompymux_rs::MovementRules::STANDARD
                        }
                    )
                    .unwrap()
                    .is_empty()
                );
            }
            // A lower-capacity sample followed by restored gravity can cross the ice plane
            // upward: at 2.5 G the jets give two MP, which flattens the sampled curve below
            // the water line, and the next sample at standard gravity is higher again.
            world
                .btech
                .rewrite_map_record(map, |record| record["gravity"] = 250.into())
                .unwrap();
            advance_battle_jumps(
                &mut world,
                stompymux_rs::MovementRules {
                    fall: rules(),
                    ..stompymux_rs::MovementRules::STANDARD
                },
            )
            .unwrap();
        }
    }
    assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(-2));
    assert!(world.btech.constructed_units()[&id].flight().is_some());
    // Represent a saved field with ice formed above that already airborne unit.
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["maps"][map.0.to_string()]["gravity"] = serde_json::json!(100);
    state["maps"][map.0.to_string()]["terrain"][10] =
        serde_json::to_value(Hex::new(Terrain::Ice, 5)).unwrap();
    state["constructed"][units[1].0.to_string()]["position"]["y"] = serde_json::json!(3);
    state["constructed"][units[1].0.to_string()]["motion"]["point"] =
        serde_json::to_value(HexCoordinate { x: 1, y: 3 }.center()).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    world.validate(&config).unwrap();
    (_dir, config, world, map, units)
}

#[tokio::test]
async fn landing_in_existing_ice_precedes_the_final_upward_breakout() {
    for fracture in [false, true] {
        let (_dir, config, mut world, map, units) =
            airborne_under_ice_fixture(UnderIce::Landing).await;
        let observer = fracture_observer(&mut world, &config, map, units);
        let id = units[0];
        ice_landing_seed(&mut world, id, fracture);
        // Keep the neighbor's fall reproducible independently of ice-break ordering.
        world
            .btech
            .rewrite_unit_record(units[1], |record| {
                record["dice"] = serde_json::to_value(Dice::seeded([42; 32])).unwrap();
            })
            .unwrap();
        let before = world.clone();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        let notices = advance_battle_jumps(
            &mut world,
            stompymux_rs::MovementRules {
                fall: rules(),
                ..stompymux_rs::MovementRules::STANDARD
            },
        )
        .unwrap();
        assert_eq!(
            notices,
            advance_battle_jumps(
                &mut loaded,
                stompymux_rs::MovementRules {
                    fall: rules(),
                    ..stompymux_rs::MovementRules::STANDARD
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
            before.btech.maps()[&map].decoration(HexCoordinate { x: 1, y: 3 }),
            world.btech.maps()[&map].decoration(HexCoordinate { x: 1, y: 3 }),
        );
        let unit = &world.btech.constructed_units()[&id];
        assert!(unit.flight().is_none());
        assert_eq!(unit.jump_stabilization(), 12);
        assert_eq!(
            unit.posture(),
            if fracture {
                Posture::Prone
            } else {
                Posture::Standing
            }
        );
        assert_eq!(
            battle_unit_elevation(&world, id).unwrap(),
            Some(if fracture { -5 } else { 0 })
        );
        assert_eq!(
            world.btech.constructed_units()[&units[1]].posture(),
            Posture::Prone
        );
        assert_eq!(
            notices
                .iter()
                .any(|notice| notice.text == "You break through the ice!"),
            !fracture
        );
        if !fracture {
            let mut dice: Dice = serde_json::from_value(
                serde_json::to_value(&before.btech.constructed_units()[&id]).unwrap()["dice"]
                    .clone(),
            )
            .unwrap();
            // A landing breaks ice on a four or better; ice that holds takes a footing roll, and
            // the landing beside the neighbour picks it and misses its accidental fall from above.
            assert!(dice.d6() > 3);
            dice.two_d6();
            assert_eq!(dice.die(1).unwrap(), 1);
            assert!(dice.two_d6() < 7);
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
        world
            .btech
            .rewrite_unit_record(id, |record| {
                record["ground_elevation"] = serde_json::to_value(altitude).unwrap();
            })
            .unwrap();
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
        let walking = world.btech.constructed_units()[&id].walking_speed();
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
            let (_dir, config, mut world, _, units) = fixture_field(terrain, height, 5).await;
            let id = units[0];
            launch_battle_jump(&mut world, id, ObjectId(1), 180, 1.0).unwrap();
            fly_until_landing_is_next(&mut world, id);
            persistence::save(&config.database(), &world).await.unwrap();
            let mut loaded = persistence::load(&config.database()).await.unwrap();
            assert_eq!(
                advance_battle_jumps(
                    &mut world,
                    stompymux_rs::MovementRules {
                        fall: rules(),
                        ..stompymux_rs::MovementRules::STANDARD
                    }
                )
                .unwrap(),
                advance_battle_jumps(
                    &mut loaded,
                    stompymux_rs::MovementRules {
                        fall: rules(),
                        ..stompymux_rs::MovementRules::STANDARD
                    }
                )
                .unwrap()
            );
            assert_eq!(world.btech, loaded.btech);
            assert!(world.btech.constructed_units()[&id].flight().is_none());
            assert_eq!(
                world.btech.constructed_units()[&id].posture(),
                Posture::Standing
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
async fn structure_jump_controls_land_or_refuse_the_route_with_saved_replay() {
    for symbol in ['@', '='] {
        for height in [0, 3, 9] {
            let row = format!("{symbol}{height}").repeat(3);
            let source = format!("3 5\n.0.0.0\n.0.0.0\n{row}\n.0.0.0\n.0.0.0\n");
            let (_dir, config, mut world, _, units) =
                fixture_asset(MapAsset::from_cells(&source).unwrap()).await;
            let id = units[0];
            let mut state = serde_json::to_value(&world.btech).unwrap();
            state["constructed"][id.0.to_string()]["dice"] =
                serde_json::to_value(Dice::seeded([3; 32])).unwrap();
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
            let text = support::run_text(&native, &config, ObjectId(1), 1, "jump 180 2");
            let launch = lua.eval_callback::<()>(&format!("btech.unit.jump({},1,180,2)", id.0));
            assert_eq!(native.world().btech, lua.world().btech);
            if height == 9 {
                // A structure above Jumping MP plus the takeoff level refuses the route at
                // launch (Total Warfare p. 52) in both adapters.
                assert!(text.contains("higher than your jets can clear"), "{text}");
                assert!(launch.is_err());
                assert_eq!(native.world().btech, world.btech);
                continue;
            }
            launch.unwrap();
            world = native.world().clone();
            assert!(world.btech.constructed_units()[&id].flight().is_some());
            persistence::save(&config.database(), &world).await.unwrap();
            let mut loaded = persistence::load(&config.database()).await.unwrap();
            let mut notices = Vec::new();
            while world.btech.constructed_units()[&id].flight().is_some() {
                let tick = advance_battle_jumps(
                    &mut world,
                    stompymux_rs::MovementRules {
                        fall: rules(),
                        ..stompymux_rs::MovementRules::STANDARD
                    },
                )
                .unwrap();
                assert_eq!(
                    tick,
                    advance_battle_jumps(
                        &mut loaded,
                        stompymux_rs::MovementRules {
                            fall: rules(),
                            ..stompymux_rs::MovementRules::STANDARD
                        }
                    )
                    .unwrap()
                );
                notices.extend(tick);
                assert_eq!(world.btech, loaded.btech);
            }
            let unit = &world.btech.constructed_units()[&id];
            assert!(unit.flight().is_none());
            assert_eq!(unit.posture(), Posture::Standing);
            assert_eq!(unit.position().unwrap().y, 3);
            assert!(
                !notices
                    .iter()
                    .any(|notice| notice.text.contains("too high"))
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
        let (_dir, config, mut world, map, units) = fixture_field(Terrain::Clear, 0, 5).await;
        let id = units[0];
        prepare_reverse_step(&mut world, id, 6);
        let kind = match terrain {
            Terrain::Fire => DecorationKind::Fire,
            _ => DecorationKind::Smoke,
        };
        set_map_decoration(
            &mut world,
            map,
            HexCoordinate { x: 1, y: 2 },
            Some(Decoration::new(kind, 0, None)),
        )
        .unwrap();
        let before = serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        assert!(
            advance_battle_motion(&mut world, MovementRules::STANDARD)
                .unwrap()
                .is_empty()
        );
        assert!(
            advance_battle_motion(&mut loaded, MovementRules::STANDARD)
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
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(world, observer, support::FIXTURE_DICE_SEED);
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
                    let coordinate = HexCoordinate { x: 1, y: 1 };
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
            Character {
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
        support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
        apply_damage_phase(
            &mut world,
            victim,
            MechSection::Head,
            7,
            DamagePhase::Armor { rear: false },
        )
        .unwrap();
        let coordinate = HexCoordinate { x: 1, y: 1 };
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
                Surface::of(Hex::new(terrain, 1)).unwrap(),
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
            Surface::of(Hex::new(terrain, 1)).unwrap(),
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
            candidate.btech.constructed_units()[&victim].sections()[&MechSection::Head].internal
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
        Character {
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
    support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
    apply_damage_phase(
        &mut world,
        trigger,
        MechSection::Head,
        7,
        DamagePhase::Armor { rear: false },
    )
    .unwrap();
    let seed = (0..=255)
        .find(|seed| {
            let mut dice = Dice::seeded([*seed; 32]);
            dice.two_d6();
            dice.d6() == 1
        })
        .unwrap();
    world
        .btech
        .set_unit_dice(trigger, Dice::seeded([seed; 32]))
        .unwrap();
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
            Character {
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
        support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
        apply_damage_phase(
            &mut world,
            victim,
            MechSection::Head,
            7,
            DamagePhase::Armor { rear: false },
        )
        .unwrap();
        let coordinate = HexCoordinate { x: 1, y: 1 };
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
            candidate.btech.constructed_units()[&victim].sections()[&MechSection::Head].internal
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
    for (next, fracture) in [
        (UnderIce::Rising, false),
        (UnderIce::Landing, false),
        (UnderIce::Landing, true),
    ] {
        let (_dir, config, mut world, map, units) = airborne_under_ice_fixture(next).await;
        ice_seed(&mut world, units[0], fracture, false);
        let movement = MovementRules {
            fall: rules(),
            ..MovementRules::STANDARD
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
            Character {
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
        support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
        apply_damage_phase(
            &mut world,
            victim,
            MechSection::Head,
            7,
            DamagePhase::Armor { rear: false },
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
                Posture::Standing
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
            candidate.btech.constructed_units()[&victim].sections()[&MechSection::Head].internal
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
        fixture_asset(MapAsset::from_cells(source).unwrap()).await;
    let id = units[0];
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][units[1].0.to_string()]["position"]["y"] = serde_json::json!(2);
    state["constructed"][units[1].0.to_string()]["motion"]["point"] =
        serde_json::to_value(HexCoordinate { x: 1, y: 2 }.center()).unwrap();
    // Protect both pilots and avoid random critical cascades in this geometry fixture.
    let safe = (0..=255)
        .find(|seed| {
            let mut dice = Dice::seeded([*seed; 32]);
            let protects = dice.two_d6() >= 7;
            dice.d6();
            protects && dice.two_d6() == 7 && dice.two_d6() >= 7
        })
        .unwrap();
    for unit in units {
        state["constructed"][unit.0.to_string()]["dice"] =
            serde_json::to_value(Dice::seeded([safe; 32])).unwrap();
    }
    world.btech = serde_json::from_value(state).unwrap();
    launch_battle_jump(&mut world, id, ObjectId(1), 180, 2.0).unwrap();
    // At a twelfth of standard gravity the jets carry the 'Mech over two-thirds of a hex each
    // second, so its landing second starts above the ice and comes down in the water beyond.
    world
        .btech
        .rewrite_map_record(map, |record| record["gravity"] = 12.into())
        .unwrap();
    for _ in 0..2 {
        assert!(
            advance_battle_jumps(
                &mut world,
                stompymux_rs::MovementRules {
                    fall: rules(),
                    ..stompymux_rs::MovementRules::STANDARD
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
    let movement = MovementRules {
        fall: rules(),
        ..MovementRules::STANDARD
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
            Character {
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
        support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
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
        MechSection::Head,
        7,
        DamagePhase::Armor { rear: false },
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
        HexCoordinate { x: 1, y: 3 }.center()
    );
    assert_eq!(unit.posture(), Posture::Prone);
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
        support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
        world
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        set_battle_character(
            &mut world,
            pilot,
            Character {
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
        support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
        if fatal {
            apply_damage_phase(
                &mut world,
                id,
                MechSection::Head,
                7,
                DamagePhase::Armor { rear: false },
            )
            .unwrap();
        }
        prepare_reverse_step(&mut world, id, 2);
        let baseline = world.clone();
        let rules = MovementRules::STANDARD;
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
        assert_eq!(unit.posture(), Posture::Prone);
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
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::HeavyWoods, 2, 3).await;
    persistence::save(&config.database(), &world).await.unwrap();
    let coordinate = HexCoordinate { x: 1, y: 1 };
    let original = world.clone();
    let before = world.btech.maps()[&map].hex(1, 1).unwrap();
    // Forty points leave heavy woods standing, so there is nothing to clear.
    assert!(apply_woodland_clearing(&mut world, map, coordinate, before, 40).is_err());
    assert!(
        apply_woodland_clearing(&mut world, map, HexCoordinate { x: -1, y: 1 }, before, 41)
            .is_err()
    );
    assert_eq!(world.btech, original.btech);
    let report = apply_woodland_clearing(&mut world, map, coordinate, before, 41).unwrap();
    assert_eq!(report.before, before);
    assert_eq!(report.after, Hex::new(Terrain::LightWoods, 2));
    // The light woods keep the one point spent past the heavy woods' threshold.
    assert_eq!(
        world.btech.maps()[&map]
            .terrain_damage_at(coordinate)
            .unwrap(),
        1
    );
    assert!(apply_woodland_clearing(&mut world, map, coordinate, before, 41).is_err());
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
    let report = apply_woodland_clearing(&mut world, map, coordinate, report.after, 50).unwrap();
    assert_eq!(report.after.level(), 2);
    assert_eq!(report.after.terrain(), Terrain::Rough);
    assert_eq!(
        world.btech.maps()[&map]
            .terrain_damage_at(coordinate)
            .unwrap(),
        0
    );
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    let unchanged = world.btech.clone();
    assert!(apply_woodland_clearing(&mut world, map, coordinate, report.after, 200).is_err());
    assert_eq!(world.btech, unchanged);
    // Two reductions in a single saved transaction are also valid with occupants present.
    let coordinate = HexCoordinate { x: 0, y: 0 };
    let first = apply_woodland_clearing(&mut world, map, coordinate, before, 41).unwrap();
    let _second = apply_woodland_clearing(&mut world, map, coordinate, first.after, 50).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    assert_eq!(original.btech.maps()[&map].hex(1, 1).unwrap(), before);
}

#[tokio::test]
async fn map_decorations_preserve_base_terrain_checkpoints_and_saved_state() {
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::HeavyWoods, 2, 3).await;
    persistence::save(&config.database(), &world).await.unwrap();
    let original = world.clone();
    let coordinate = HexCoordinate { x: 1, y: 1 };
    let fire = Decoration::new(DecorationKind::Fire, 120, Some(60));
    set_map_decoration(&mut world, map, coordinate, Some(fire)).unwrap();
    assert_eq!(
        world.btech.maps()[&map].hex(1, 1).unwrap(),
        Hex::new(Terrain::HeavyWoods, 2).with_overlay(Some(DecorationKind::Fire))
    );
    assert_eq!(
        world.btech.maps()[&map].base_hex(1, 1).unwrap().terrain(),
        Terrain::HeavyWoods
    );
    assert_eq!(
        world.btech.maps()[&map].decoration(coordinate).unwrap(),
        Some(fire)
    );
    assert_eq!(
        original.btech.maps()[&map].hex(1, 1).unwrap().terrain(),
        Terrain::HeavyWoods
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
            Some(Decoration {
                remaining: 0,
                ..fire
            })
        )
        .is_err()
    );
    assert!(set_map_decoration(&mut world, map, HexCoordinate { x: 3, y: 1 }, Some(fire)).is_err());
    assert_eq!(world.btech, before);
    let smoke = Decoration::new(DecorationKind::Smoke, 90, None);
    set_map_decoration(&mut world, map, coordinate, Some(smoke)).unwrap();
    assert_eq!(
        world.btech.maps()[&map].hex(1, 1).unwrap().terrain(),
        Terrain::Smoke
    );
    assert_eq!(
        world.btech.maps()[&map].base_hex(1, 1).unwrap().terrain(),
        Terrain::HeavyWoods
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
    let (_dir, config, mut world, map, _units) = fixture_field(Terrain::HeavyWoods, 2, 3).await;
    let coordinate = HexCoordinate { x: 1, y: 1 };
    let fire_coordinate = HexCoordinate { x: 0, y: 0 };
    let smoke = Decoration::new(DecorationKind::Smoke, 3, None);
    let fire = Decoration::new(DecorationKind::Fire, 60, Some(60));
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
        Hex::new(Terrain::HeavyWoods, 2)
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
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::HeavyWoods, 2, 3).await;
    let coordinate = HexCoordinate { x: 1, y: 1 };
    set_map_decoration(
        &mut world,
        map,
        coordinate,
        Some(Decoration::new(DecorationKind::Fire, 60, None)),
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
    assert_eq!(burnt, Hex::new(Terrain::LightWoods, 2));
    // The map's wind blows from the north (bearing 0), so smoke lies downwind to the south:
    // the hex below the fire and the two hexes 60 degrees either side of it (Tactical
    // Operations pp. 43-45).
    for (x, y) in [(1, 2), (0, 1), (2, 1)] {
        let smoke = world.btech.maps()[&map]
            .decoration(HexCoordinate { x, y })
            .unwrap()
            .unwrap();
        assert_eq!(smoke.kind, DecorationKind::Smoke);
        assert!((89..=149).contains(&smoke.remaining));
        assert_eq!(smoke.next_spread, None);
    }
    for x in 0..3 {
        assert!(
            world.btech.maps()[&map]
                .decoration(HexCoordinate { x, y: 0 })
                .unwrap()
                .is_none(),
            "no smoke upwind at {x},0"
        );
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
        world.btech.maps()[&map].hex(1, 2).unwrap().terrain(),
        Terrain::HeavyWoods
    );
}

#[tokio::test]
async fn strong_wind_fire_replays_new_ignition_and_retains_scheduled_delay() {
    let (_dir, config, mut world, map, _units) = fixture_field(Terrain::LightWoods, 1, 5).await;
    // A wind from the south (bearing 180) carries the fire north, toward row 2.
    set_map_wind(&mut world, map, 180, 0).unwrap();
    let coordinate = HexCoordinate { x: 1, y: 3 };
    set_map_decoration(
        &mut world,
        map,
        coordinate,
        Some(Decoration::new(DecorationKind::Fire, 120, None)),
    )
    .unwrap();
    // Select a reproducible map stream whose first spread roll ignites the forward cell.
    let seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() >= 9)
        .unwrap();
    world
        .btech
        .rewrite_map_record(map, |record| {
            record["fire_dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
        })
        .unwrap();
    for _ in 0..10 {
        advance_map_fire(&mut world).unwrap();
    }
    // Wind changes affect subsequent scheduling, not a check already counting down.
    set_map_wind(&mut world, map, 180, 60).unwrap();
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
        .decoration(HexCoordinate { x: 1, y: 2 })
        .unwrap()
        .unwrap();
    assert_eq!(forward.kind, DecorationKind::Fire);
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
    let _shutdown = stop_battle_unit(&mut world, units[1], ObjectId(2)).unwrap();
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
        HexCoordinate { x: 1, y: 1 },
        Some(Decoration::new(DecorationKind::Smoke, 120, None)),
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
    let leg = MechSection::LeftLeg;
    let armor = world.btech.constructed_units()[&id].sections()[&leg].armor;
    for candidate in [&mut world, &mut clear] {
        let _damage = apply_damage_phase(
            candidate,
            id,
            leg,
            armor,
            DamagePhase::Armor { rear: false },
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
        let coordinate = HexCoordinate { x: 1, y: 1 };
        let elevation = battle_unit_elevation(&world, units[0]).unwrap();
        set_map_decoration(
            &mut world,
            map,
            coordinate,
            Some(Decoration::new(DecorationKind::Smoke, 120, None)),
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
        fixture_asset(MapAsset::from_cells(source).unwrap()).await;
    let id = units[0];
    let leg = MechSection::LeftLeg;
    let armor = world.btech.constructed_units()[&id].sections()[&leg].armor;
    let _damage = apply_damage_phase(
        &mut world,
        id,
        leg,
        armor,
        DamagePhase::Armor { rear: false },
    )
    .unwrap();
    prepare_reverse_step(&mut world, id, 12);
    let mut clear = world.clone();
    for y in 2..4 {
        for x in 0..3 {
            set_map_decoration(
                &mut world,
                map,
                HexCoordinate { x, y },
                Some(Decoration::new(DecorationKind::Smoke, 120, None)),
            )
            .unwrap();
        }
    }
    let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
    let expected = advance_battle_motion(&mut clear, MovementRules::STANDARD).unwrap();
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
        WoodlandIntent::Ignite,
        WoodlandIntent::Clear,
        WoodlandIntent::Absorb,
        WoodlandIntent::Incidental,
    ] {
        let (_dir, config, mut world, map, units) = fixture_field(Terrain::HeavyWoods, 2, 3).await;
        let shooter = units[0];
        let seed = (0..=255)
            .find(|seed| Dice::seeded([*seed; 32]).two_d6() >= 4)
            .unwrap();
        // Heavy woods have a terrain factor of ninety and become light woods once forty-one
        // is spent (Total Warfare p. 112); thirty is already spent, so one fifteen-point slug
        // aimed at the hex thins them without felling them.
        world
            .btech
            .rewrite_map_record(map, |record| {
                record["terrain_damage"] = serde_json::json!({ "1": 30 });
            })
            .unwrap();
        world
            .btech
            .set_unit_dice(shooter, Dice::seeded([seed; 32]))
            .unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        let original = world.clone();
        let request = WoodlandAttack {
            shooter,
            coordinate: HexCoordinate { x: 1, y: 0 },
            weapon: if intent == WoodlandIntent::Ignite {
                Weapon::Flamer
            } else {
                Weapon::GaussRifle
            },
            ammunition: AmmunitionMode::Normal,
            damage: 15,
            intent,
        };
        if intent == WoodlandIntent::Ignite {
            let mut incomplete = original.clone();
            incomplete
                .btech
                .rewrite_map_record(map, |record| {
                    record["fire_dice"] = serde_json::Value::Null;
                })
                .unwrap();
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
        // A stray slug neither clears nor, from a Gauss rifle, ignites anything.
        let expected = match intent {
            WoodlandIntent::Ignite => Terrain::Fire,
            WoodlandIntent::Clear | WoodlandIntent::Absorb => Terrain::LightWoods,
            WoodlandIntent::Incidental => Terrain::HeavyWoods,
        };
        assert_eq!(
            world.btech.maps()[&map].hex(1, 0).unwrap().terrain(),
            expected
        );
        match intent {
            WoodlandIntent::Ignite => assert_eq!(report.notices[0].text, "You ignite 1,0."),
            WoodlandIntent::Clear | WoodlandIntent::Absorb => {
                assert_eq!(report.notices[0].text, "You clear 1,0.")
            }
            WoodlandIntent::Incidental => {
                assert_eq!(report.effect, WoodlandEffect::None);
                assert!(report.notices.is_empty());
            }
        }
        let mut expected_unit =
            serde_json::to_value(&original.btech.constructed_units()[&shooter]).unwrap();
        let actual_unit = serde_json::to_value(&world.btech.constructed_units()[&shooter]).unwrap();
        expected_unit["dice"] = actual_unit["dice"].clone();
        assert_eq!(expected_unit, actual_unit);
        assert_eq!(
            original.btech.maps()[&map].hex(1, 0).unwrap().terrain(),
            Terrain::HeavyWoods
        );
        let before_invalid = world.btech.clone();
        assert!(
            resolve_woodland_attack(
                &mut world,
                WoodlandAttack {
                    coordinate: HexCoordinate { x: -1, y: 0 },
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
        if intent == WoodlandIntent::Ignite {
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
    let building = BuildingState {
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
        BuildingState {
            integrity: -1,
            ..building
        },
        BuildingState {
            integrity: 121,
            ..building
        },
        BuildingState {
            maximum_integrity: 32768,
            ..building
        },
        BuildingState {
            flags: 256,
            ..building
        },
        BuildingState {
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
        BuildingState {
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
        stop_battle_unit(&mut world, id, pilot).unwrap();
        remove_battle_unit(&mut world, id, ObjectId(config.home())).unwrap();
    }
    reload_battle_map(
        &mut world,
        map,
        "interior.map",
        MapAsset::from_cells("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    assert_eq!(
        world.btech.maps()[&map].building,
        BuildingState {
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
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let point = HexCoordinate { x: 1, y: 1 };
    let entrance = BuildingEntrance {
        coordinate: point,
        interior,
        data_char: 0,
        data_short: 0,
        data_int: 0,
    };
    let later = BuildingEntrance {
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
        BuildingEntrance {
            coordinate: HexCoordinate { x: -1, y: 1 },
            ..entrance
        },
        BuildingEntrance {
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
    let retained_entrance = BuildingEntrance {
        data_char: 87,
        data_short: 23,
        data_int: 91,
        ..entrance
    };
    set_building_entrance(&mut expected, map, 2, Some(retained_entrance)).unwrap();
    set_battle_static_decoration(
        &mut expected,
        map,
        StaticDecorationKind::Smoke,
        0,
        Some(StaticDecoration {
            coordinate: HexCoordinate { x: 0, y: 0 },
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
        Some(BuildingEntrance {
            coordinate: HexCoordinate { x: 0, y: 0 },
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
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::HeavyWoods, 0, 3).await;
    let coordinate = HexCoordinate { x: 1, y: 1 };
    for (ordinal, kind) in [
        MineKind::Standard,
        MineKind::Inferno,
        MineKind::Command,
        MineKind::Vibra,
        MineKind::Trigger,
    ]
    .into_iter()
    .enumerate()
    {
        set_minefield(
            &mut world,
            map,
            ordinal as u32,
            Some(Minefield {
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
        Minefield {
            coordinate: HexCoordinate { x: 3, y: 1 },
            ..mine
        },
        Minefield {
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
        candidate
            .btech
            .rewrite_unit_record(units[0], |record| {
                record["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
            })
            .unwrap();
        candidate
            .btech
            .rewrite_map_record(map, |record| {
                let index = (record["width"].as_i64().unwrap() + 1).to_string();
                record["terrain_damage"] = serde_json::json!({ index: 30 });
            })
            .unwrap();
        let report = resolve_woodland_attack(
            &mut candidate,
            WoodlandAttack {
                shooter: units[0],
                coordinate,
                weapon: Weapon::MediumLaser,
                ammunition: AmmunitionMode::Normal,
                damage: 12,
                intent: WoodlandIntent::Clear,
            },
        )
        .unwrap();
        if matches!(report.effect, WoodlandEffect::Clear { .. }) {
            cleared = Some(candidate);
            break;
        }
    }
    world = cleared.unwrap();
    assert_eq!(
        world.btech.maps()[&map].base_hex(1, 1).unwrap().terrain(),
        Terrain::LightWoods
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
        Some(Minefield {
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
    let coordinate = HexCoordinate { x: 1, y: 1 };
    let tons = (world.btech.constructed_units()[&unit].mass().unwrap().total / 1024) as i16;
    let field = Minefield {
        coordinate,
        kind: MineKind::Standard,
        strength: tons,
        extra: 0,
        owner: ObjectId(1),
    };
    set_minefield(&mut world, map, 9, Some(field)).unwrap();
    set_minefield(
        &mut world,
        map,
        2,
        Some(Minefield {
            kind: MineKind::Trigger,
            extra: -1,
            ..field
        }),
    )
    .unwrap();
    set_minefield(
        &mut world,
        map,
        5,
        Some(Minefield {
            kind: MineKind::Command,
            extra: 42,
            ..field
        }),
    )
    .unwrap();
    // The standard field supplies map coverage; a colocated trigger still uses its weight gate.
    let before = world.btech.clone();
    let selected = mine_activations(&world, unit, MineTriggerReason::Step).unwrap();
    assert_eq!(
        selected
            .iter()
            .map(|event| (event.ordinal, event.response))
            .collect::<Vec<_>>(),
        vec![
            (2, MineResponse::Trigger),
            (5, MineResponse::Spotted),
            (9, MineResponse::Explode)
        ]
    );
    assert_eq!(
        mine_activations(&world, unit, MineTriggerReason::Fall)
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
        mine_activations(&loaded, unit, MineTriggerReason::Step).unwrap(),
        selected
    );
    set_map_decoration(
        &mut world,
        map,
        coordinate,
        Some(Decoration::new(DecorationKind::Smoke, 30, None)),
    )
    .unwrap();
    assert_eq!(
        mine_activations(&world, unit, MineTriggerReason::Step).unwrap(),
        selected
    );
    set_minefield(
        &mut world,
        map,
        2,
        Some(Minefield {
            kind: MineKind::Trigger,
            strength: tons + 1,
            ..field
        }),
    )
    .unwrap();
    assert_eq!(
        mine_activations(&world, unit, MineTriggerReason::Step)
            .unwrap()
            .iter()
            .map(|event| event.ordinal)
            .collect::<Vec<_>>(),
        vec![5, 9]
    );
    let mut elevated = world.clone();
    elevated
        .btech
        .rewrite_unit_record(unit, |record| {
            record["ground_elevation"] = 1.into();
        })
        .unwrap();
    assert!(
        mine_activations(&elevated, unit, MineTriggerReason::Land)
            .unwrap()
            .is_empty()
    );
    assert!(
        world.btech.maps()[&map]
            .mine_coverage(HexCoordinate { x: -1, y: 1 })
            .is_err()
    );
    assert!(mine_activations(&world, ObjectId(-1), MineTriggerReason::Step).is_err());
}

/// Dice for `id` under which `accept` holds for the action it runs on a copy of the world; a
/// conventional field goes off only on a high enough roll (Tactical Operations p. 176), so a
/// scenario that needs the explosion searches for it.
fn dice_for(world: &World, id: ObjectId, accept: impl Fn(&mut World) -> bool) -> Dice {
    (0..=u16::MAX)
        .map(|value| {
            let mut seed = [0; 32];
            seed[..2].copy_from_slice(&value.to_le_bytes());
            Dice::seeded(seed)
        })
        .find(|dice| {
            let mut probe = world.clone();
            probe.btech.set_unit_dice(id, dice.clone()).unwrap();
            accept(&mut probe)
        })
        .expect("dice that produce the requested outcome")
}

#[tokio::test]
async fn conventional_mine_blasts_packets_neighbors_removal_and_restart() {
    for (kind, strength) in [
        (MineKind::Standard, 4),
        (MineKind::Standard, 6),
        (MineKind::Command, 6),
        (MineKind::Vibra, 6),
    ] {
        let (_dir, config, mut world, map, units) = fixture_field(Terrain::HeavyWoods, 0, 3).await;
        let coordinate = HexCoordinate { x: 1, y: 1 };
        let neighbor = HexCoordinate { x: 1, y: 0 };
        world
            .btech
            .rewrite_unit_record(units[1], |record| {
                record["position"]["y"] = 0.into();
                record["motion"]["point"] = serde_json::to_value(neighbor.center()).unwrap();
            })
            .unwrap();
        let mine = Minefield {
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
            Some(Minefield {
                kind: MineKind::Trigger,
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
        // Only a command-detonated field reaches the neighbouring hexes (Tactical Operations
        // p. 177); every unit takes the blast on its front.
        let area = kind == MineKind::Command;
        assert!(report.hits.iter().all(|hit| hit.arc == HitArc::Front));
        assert_eq!(report.hits.len(), if area { 2 } else { 1 });
        if area {
            assert_eq!((report.hits[1].unit, report.hits[1].damage), (units[1], 3));
        } else {
            assert_eq!(
                world.btech.constructed_units()[&units[1]],
                before.btech.constructed_units()[&units[1]]
            );
        }
        // Conventional, command and vibra blasts start no fires (TO:AR p. 176–177).
        assert!(report.ignited.is_empty());
        // A conventional field thins on a density roll and never below five points; the
        // others drop a step and are spent once that would leave less than five.
        let conventional = kind == MineKind::Standard;
        assert_eq!(
            report.removed,
            if !conventional || strength < 5 {
                vec![0, 1]
            } else {
                vec![]
            }
        );
        if conventional && strength >= 5 {
            let roll = report.depletion_roll.unwrap();
            let remaining = if roll >= 9 { 5 } else { strength };
            assert_eq!(report.remaining_strength, Some(remaining));
            assert_eq!(
                world.btech.maps()[&map].minefields()[&0],
                Minefield {
                    strength: remaining,
                    ..mine
                }
            );
        } else {
            assert_eq!(report.depletion_roll, None);
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
        // A conventional field's depletion roll needs the map's random stream; its loss
        // rejects the whole blast after the damage was applied.
        if conventional && strength >= 5 {
            let mut invalid = before;
            invalid
                .btech
                .rewrite_map_record(map, |record| {
                    record["fire_dice"] = serde_json::Value::Null;
                })
                .unwrap();
            let checkpoint = invalid.clone();
            assert!(resolve_mine_blast(&mut invalid, map, 0, rules()).is_err());
            assert_eq!(invalid.btech, checkpoint.btech);
        }
    }
}

#[tokio::test]
async fn mine_blast_character_action_and_late_rejection_are_atomic() {
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::Clear, 0, 3).await;
    set_minefield(
        &mut world,
        map,
        0,
        Some(Minefield {
            coordinate: HexCoordinate { x: 1, y: 1 },
            kind: MineKind::Standard,
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
        Some(Minefield {
            coordinate: HexCoordinate { x: 1, y: 1 },
            kind: MineKind::Standard,
            strength: 4,
            extra: 0,
            owner: ObjectId(1),
        }),
    )
    .unwrap();
    assert!(
        !mine_activations(&world, units[0], MineTriggerReason::Step)
            .unwrap()
            .is_empty()
    );
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["constructed"][units[0].0.to_string()]["ground_elevation"] = (-2).into();
    encoded["constructed"][units[1].0.to_string()]["ground_elevation"] = (-1).into();
    world.btech = serde_json::from_value(encoded).unwrap();
    assert!(
        mine_activations(&world, units[0], MineTriggerReason::Step)
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
async fn inferno_mines_split_damage_and_burn_duration_atomically() {
    for strength in [1, 4, 6] {
        let (_dir, config, mut world, map, _units) = fixture_field(Terrain::Clear, 0, 3).await;
        set_minefield(
            &mut world,
            map,
            0,
            Some(Minefield {
                coordinate: HexCoordinate { x: 1, y: 1 },
                kind: MineKind::Inferno,
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
            // A 'Mech is hit by 1d6 infernos: two heat points each and no burn (Tactical
            // Operations p. 42, Total Warfare p. 159).
            assert_eq!(hit.burn_seconds, 0);
            assert!((2..=12).contains(&hit.heat), "{}", hit.heat);
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
    }
}

#[tokio::test]
async fn inferno_missile_exposure_adds_two_heat_per_missile_within_the_turn_cap() {
    let (_dir, config, mut base, _map, units) = fixture_field(Terrain::Clear, 0, 3).await;
    let target = units[0];
    update_battle_contact(&mut base, units[1], target).unwrap();
    // Total Warfare p. 141 and p. 159: two heat per missile, at most fifteen outside heat a turn.
    for (missiles, heat) in [(1, 2), (2, 4), (3, 6), (6, 12), (8, 15), (65535, 15)] {
        let mut world = base.clone();
        let report = resolve_inferno_hit(&mut world, target, missiles).unwrap();
        assert_eq!(report.target, target);
        assert_eq!(report.missiles, missiles);
        assert_eq!(report.heat, heat);
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
                .any(|n| n.unit == units[1] && n.text.contains("splashed"))
        );
        let mut expected = serde_json::to_value(&base.btech).unwrap();
        let unit = &mut expected["constructed"][target.0.to_string()];
        unit["heat"]["stored"] =
            (unit["heat"]["stored"].as_f64().unwrap() + f64::from(heat)).into();
        unit["overheat_clock"]["outside_heat"] = heat.into();
        assert_eq!(serde_json::to_value(&world.btech).unwrap(), expected);
        persistence::save(&config.database(), &world).await.unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        let more = resolve_inferno_hit(&mut world, target, 1).unwrap();
        assert_eq!(resolve_inferno_hit(&mut replay, target, 1).unwrap(), more);
        assert_eq!(world.btech, replay.btech);
        assert_eq!(more.heat, 2.min(15 - heat));
        assert_eq!(
            world.btech.constructed_units()[&target]
                .overheat_clock()
                .outside_heat,
            heat + more.heat
        );
    }
}

#[tokio::test]
async fn inferno_missile_rejects_invalid_exposures_without_mutation() {
    let (_dir, _config, mut world, _map, units) = fixture_field(Terrain::Clear, 0, 3).await;
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
}

#[tokio::test]
async fn inferno_ammunition_explosion_deals_full_damage_and_adds_heat() {
    for (terrain, depth) in [(Terrain::Clear, 0), (Terrain::Water, 2)] {
        let (_dir, config, mut world, _map, units) = fixture_field(terrain, depth, 3).await;
        let id = units[0];
        world
            .btech
            .rewrite_unit_record(id, |record| {
                record["definition"]["sections"]["RightTorso"]["criticals"]["0"]["modes"] =
                    serde_json::json!(["Inferno"]);
                record["ammunition"][0] = 1.into();
            })
            .unwrap();
        let rules = rules();
        let before = world.clone();
        let report = explode_battle_ammunition(&mut world, id, 0, rules).unwrap();
        // Total Warfare p. 141: the inferno bin explodes for its full damage and heats the
        // carrying unit by two per missile in the bin.
        let missiles = u16::from(
            world.btech.constructed_units()[&id]
                .loadout()
                .unwrap()
                .ammunition[0]
                .weapon
                .profile()
                .missiles,
        )
        .max(1);
        assert_eq!(
            report.impact.phases.iter().map(|p| p.absorbed).sum::<u16>(),
            8
        );
        assert_eq!(world.btech.constructed_units()[&id].ammunition()[0], 0);
        assert_eq!(
            world.btech.constructed_units()[&id].heat().stored,
            before.btech.constructed_units()[&id].heat().stored + f64::from((2 * missiles).min(30))
        );
        assert_eq!(
            world.btech.constructed_units()[&id].pilot_injuries(),
            before.btech.constructed_units()[&id].pilot_injuries() + 2
        );
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

#[tokio::test]
async fn mine_event_ground_entry_spotting_burning_removal_and_restart() {
    for kind in [MineKind::Standard, MineKind::Inferno, MineKind::Command] {
        let (_dir, config, mut world, map, units) = fixture_field(Terrain::Clear, 0, 4).await;
        let id = units[0];
        prepare_reverse_step(&mut world, id, 7);
        set_minefield(
            &mut world,
            map,
            0,
            Some(Minefield {
                coordinate: HexCoordinate { x: 1, y: 2 },
                kind,
                strength: 4,
                extra: 0,
                owner: ObjectId(1),
            }),
        )
        .unwrap();
        if kind != MineKind::Command {
            let dice = dice_for(&world, id, |probe| {
                advance_battle_motion(probe, MovementRules::STANDARD)
                    .unwrap()
                    .iter()
                    .any(|n| n.text.contains("trigger a mine"))
            });
            world.btech.set_unit_dice(id, dice).unwrap();
        }
        let before = world.clone();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
        assert_eq!(
            notices,
            advance_battle_motion(&mut replay, MovementRules::STANDARD).unwrap()
        );
        assert_eq!(world.btech, replay.btech);
        assert_eq!(
            world.btech.constructed_units()[&id].position().unwrap().y,
            2
        );
        if kind == MineKind::Command {
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
        }
        let later = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
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
        let (_dir, config, mut world, map, units) = fixture_field(Terrain::Clear, 0, 4).await;
        let id = units[0];
        let coordinate = HexCoordinate {
            x: 1,
            y: if jump { 2 } else { 1 },
        };
        set_minefield(
            &mut world,
            map,
            0,
            Some(Minefield {
                coordinate,
                kind: MineKind::Inferno,
                strength: 4,
                extra: 0,
                owner: ObjectId(1),
            }),
        )
        .unwrap();
        // A landing sets the field off on a nine or better, a fall only on a twelve
        // (Tactical Operations pp. 176–177); the dice are chosen so that it does.
        let land = |world: &mut World| {
            let mut notices = Vec::new();
            for _ in 0..60 {
                notices.extend(advance_battle_jumps(world, MovementRules::STANDARD).unwrap());
                if !world.btech.constructed_units()[&id].airborne() {
                    break;
                }
            }
            notices
        };
        if jump {
            launch_battle_jump(&mut world, id, ObjectId(1), 180, 1.0).unwrap();
            let dice = dice_for(&world, id, |probe| {
                land(probe)
                    .iter()
                    .any(|n| n.unit == id && n.text == "You trigger a mine!")
            });
            world.btech.set_unit_dice(id, dice).unwrap();
            let notices = land(&mut world);
            assert!(!world.btech.constructed_units()[&id].airborne());
            assert_eq!(
                notices
                    .iter()
                    .filter(|n| n.unit == id && n.text == "You trigger a mine!")
                    .count(),
                1
            );
        } else {
            let dice = dice_for(&world, id, |probe| {
                resolve_battle_fall(probe, id, 1, rules())
                    .unwrap()
                    .mines
                    .blasts
                    .len()
                    == 1
            });
            world.btech.set_unit_dice(id, dice).unwrap();
            let report = resolve_battle_fall(&mut world, id, 1, rules()).unwrap();
            assert_eq!(report.mines.reason, MineTriggerReason::Fall);
            assert_eq!(report.mines.blasts.len(), 1);
            assert!(
                report
                    .notices(id)
                    .iter()
                    .any(|n| n.text == "You trigger a mine!")
            );
        }
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
    let (dir, config, mut world, map, units) = fixture_field(Terrain::Clear, 0, 4).await;
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
    for (ordinal, kind, strength) in [(0, MineKind::Inferno, 6), (1, MineKind::Trigger, 0)] {
        set_minefield(
            &mut world,
            map,
            ordinal,
            Some(Minefield {
                coordinate: HexCoordinate { x: 1, y: 2 },
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
        advance_battle_motion_action(&scripts, &config, MovementRules::STANDARD).unwrap_err();
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
    advance_battle_motion_action(&scripts, &config, MovementRules::STANDARD).unwrap();
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
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::Clear, 0, 3).await;
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    for id in units {
        for leg in ["LeftLeg", "RightLeg"] {
            encoded["constructed"][id.0.to_string()]["sections"][leg]["armor"] = 0.into();
            encoded["constructed"][id.0.to_string()]["sections"][leg]["internal"] = 1.into();
        }
    }
    world.btech = serde_json::from_value(encoded).unwrap();
    let mine = Minefield {
        coordinate: HexCoordinate { x: 1, y: 1 },
        kind: MineKind::Standard,
        strength: 6,
        extra: 0,
        owner: ObjectId(1),
    };
    set_minefield(&mut world, map, 0, Some(mine)).unwrap();
    let dice = dice_for(&world, units[0], |probe| {
        activate_mines(probe, units[0], MineTriggerReason::Step, rules())
            .unwrap()
            .blasts
            .len()
            == 1
    });
    world.btech.set_unit_dice(units[0], dice).unwrap();
    let before = world.clone();
    let report = activate_mines(&mut world, units[0], MineTriggerReason::Step, rules()).unwrap();
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
            .all(|u| u.is_destroyed() || u.posture() == Posture::Prone)
    );
    world.validate(&config).unwrap();
    let mut replay = before.clone();
    assert_eq!(
        activate_mines(&mut replay, units[0], MineTriggerReason::Step, rules()).unwrap(),
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
        Some(Minefield {
            strength: 1,
            ..mine
        }),
    )
    .unwrap();
    set_minefield(
        &mut weak,
        map,
        1,
        Some(Minefield {
            kind: MineKind::Trigger,
            strength: 0,
            ..mine
        }),
    )
    .unwrap();
    let report = activate_mines(&mut weak, units[0], MineTriggerReason::Step, rules()).unwrap();
    assert_eq!(report.triggers, 0);
    assert!(weak.btech.maps()[&map].minefields().is_empty());
}

#[tokio::test]
async fn mine_event_late_blast_failure_restores_the_whole_ground_step() {
    let (_dir, _config, mut world, map, units) = fixture_field(Terrain::Clear, 0, 4).await;
    let id = units[0];
    prepare_reverse_step(&mut world, id, 7);
    let mine = Minefield {
        coordinate: HexCoordinate { x: 1, y: 2 },
        kind: MineKind::Inferno,
        strength: 6,
        extra: 0,
        owner: ObjectId(1),
    };
    set_minefield(&mut world, map, 0, Some(mine)).unwrap();
    set_minefield(
        &mut world,
        map,
        1,
        Some(Minefield {
            kind: MineKind::Vibra,
            strength: 2,
            ..mine
        }),
    )
    .unwrap();
    support::fail_mine_ignition(&mut world, map, 2);
    let dice = dice_for(&world, id, |probe| {
        advance_battle_motion(probe, MovementRules::STANDARD).is_err()
    });
    world.btech.set_unit_dice(id, dice).unwrap();
    let before = world.clone();
    assert!(advance_battle_motion(&mut world, MovementRules::STANDARD).is_err());
    assert_eq!(world.btech, before.btech);
}

#[tokio::test]
async fn mine_event_remote_vibra_reports_visible_explosion_without_neighbor_damage() {
    let (_dir, _config, mut world, map, units) = fixture_field(Terrain::Clear, 0, 3).await;
    set_minefield(
        &mut world,
        map,
        0,
        Some(Minefield {
            coordinate: HexCoordinate { x: 1, y: 0 },
            kind: MineKind::Vibra,
            strength: 6,
            extra: 0,
            owner: ObjectId(1),
        }),
    )
    .unwrap();
    let event = activate_mines(&mut world, units[0], MineTriggerReason::Step, rules()).unwrap();
    // A vibrabomb set off from a hex away damages only its own, empty hex (Tactical
    // Operations p. 177).
    assert_eq!(event.blasts.len(), 1);
    assert!(event.blasts[0].hits.is_empty());
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
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::Clear, 0, 7).await;
    let sender = units[0];
    let other = world.create(&config, "Other mine map".into(), Kind::Room);
    create_battle_map(
        &mut world,
        other,
        "other-mine.map",
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, other, support::FIXTURE_DICE_SEED);
    let mine = Minefield {
        coordinate: HexCoordinate { x: 1, y: 1 },
        kind: MineKind::Command,
        strength: 4,
        extra: 42,
        owner: ObjectId(2),
    };
    for (ordinal, field) in [
        (2, mine),
        (3, mine),
        (
            4,
            Minefield {
                kind: MineKind::Standard,
                ..mine
            },
        ),
        (
            5,
            Minefield {
                coordinate: HexCoordinate { x: 0, y: 5 },
                extra: 77,
                ..mine
            },
        ),
        (
            6,
            Minefield {
                coordinate: HexCoordinate { x: 2, y: 4 },
                kind: MineKind::Inferno,
                ..mine
            },
        ),
        (
            9,
            Minefield {
                coordinate: HexCoordinate { x: 1, y: 5 },
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
        Some(Minefield {
            coordinate: HexCoordinate { x: 0, y: 0 },
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
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::Clear, 0, 7).await;
    let sender = units[0];
    let target = units[1];
    world
        .btech
        .rewrite_unit_record(target, |record| {
            record["position"]["y"] = 5.into();
            record["motion"]["point"] =
                serde_json::to_value(HexCoordinate { x: 1, y: 5 }.center()).unwrap();
        })
        .unwrap();
    world
        .objects
        .get_mut(&target)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let mine = Minefield {
        coordinate: HexCoordinate { x: 1, y: 1 },
        kind: MineKind::Command,
        strength: 4,
        extra: 5,
        owner: ObjectId(1),
    };
    set_minefield(&mut world, map, 0, Some(mine)).unwrap();
    set_minefield(
        &mut world,
        map,
        1,
        Some(Minefield {
            coordinate: HexCoordinate { x: 1, y: 5 },
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
}

#[tokio::test]
async fn command_mines_frequency_values_and_invalid_senders_do_not_spend_dice() {
    let (_dir, _config, base, map, units) = fixture_field(Terrain::Clear, 0, 3).await;
    let sender = units[0];
    for frequency in [0, -1, i32::MAX] {
        let mut world = base.clone();
        set_minefield(
            &mut world,
            map,
            0,
            Some(Minefield {
                coordinate: HexCoordinate { x: 0, y: 0 },
                kind: MineKind::Command,
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
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::Clear, 0, 3).await;
    let center = HexCoordinate { x: 1, y: 1 };
    let neighbor = HexCoordinate { x: 1, y: 0 };
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["constructed"][units[1].0.to_string()]["position"]["y"] = 0.into();
    encoded["constructed"][units[1].0.to_string()]["motion"]["point"] =
        serde_json::to_value(neighbor.center()).unwrap();
    for (id, seed) in units.into_iter().zip([3, 4]) {
        encoded["constructed"][id.0.to_string()]["dice"] =
            serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
    }
    world.btech = serde_json::from_value(encoded).unwrap();
    let mut flight = ArtilleryFlight::new(
        center,
        center,
        Weapon::LongTom,
        ArtilleryMode::Standard,
        ArtilleryOutcome::Hit,
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
        // A Long Tom's 25 in the target hex and 15 in the ring around it.
        [(units[0], 5), (units[1], 3)]
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

/// Smoke covers the target hex and its neighbors except where a fire burns, and survives
/// storage; delivered mines neither reinforce nor replace existing weapon-delivered fields.
#[tokio::test]
async fn artillery_world_smoke_and_mines() {
    let (_dir, config, mut world, map, _) = fixture_field(Terrain::Clear, 0, 3).await;
    let center = HexCoordinate { x: 1, y: 1 };
    set_map_decoration(
        &mut world,
        map,
        center,
        Some(Decoration::new(DecorationKind::Fire, 200, None)),
    )
    .unwrap();
    for mode in [
        ArtilleryMode::Smoke,
        ArtilleryMode::Mine,
        ArtilleryMode::Mine,
    ] {
        let mut flight =
            ArtilleryFlight::new(center, center, Weapon::LongTom, mode, ArtilleryOutcome::Hit)
                .unwrap();
        let mut report = None;
        for _ in 0..10 {
            report = advance_artillery_flight(&mut world, map, &mut flight, rules()).unwrap();
        }
        let report = report.unwrap();
        assert!(report.hits.is_empty());
        if mode == ArtilleryMode::Smoke {
            for cell in &report.pattern.cells {
                let decoration = world.btech.maps()[&map]
                    .decoration(cell.position)
                    .unwrap()
                    .unwrap();
                if cell.position == center {
                    assert_eq!(decoration.kind, DecorationKind::Fire);
                    continue;
                }
                // Heavy smoke until the End Phase three turns later (Tactical Operations:
                // Advanced Units & Equipment p. 168).
                assert_eq!(decoration.kind, DecorationKind::HeavySmoke);
                assert!((91..=120).contains(&decoration.remaining));
                assert_eq!(
                    world.btech.maps()[&map]
                        .base_hex(i64::from(cell.position.x), i64::from(cell.position.y))
                        .unwrap()
                        .terrain(),
                    Terrain::Clear
                );
            }
        } else {
            assert_eq!(world.btech.maps()[&map].minefields().len(), 1);
            let mut field = world.btech.maps()[&map].minefields()[&0];
            if field.kind == MineKind::Standard {
                assert_eq!(report.mines, [0]);
                assert_eq!(field.strength, 25);
                field.kind = MineKind::Inferno;
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
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::Clear, 0, 3).await;
    for pilot in [ObjectId(1), ObjectId(2)] {
        set_battle_character(
            &mut world,
            pilot,
            Character {
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
        support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
    }
    let center = HexCoordinate { x: 1, y: 1 };
    world
        .objects
        .get_mut(&units[1])
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let mut flight = ArtilleryFlight::new(
        center,
        center,
        Weapon::Thumper,
        ArtilleryMode::Standard,
        ArtilleryOutcome::Hit,
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
    let report =
        advance_artillery_flight_action(&scripts, &config, map, (&mut flight, units[0]), rules())
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

/// In its own hex a shell's damage climbs and sinks ten points a level (Tactical Operations
/// p. 183): a Long Tom's 25 still reaches two levels away and nothing three levels away.
#[tokio::test]
async fn artillery_blast_height_limits() {
    for (terrain, depth, first, second) in [(Terrain::Water, 4, -2, -3), (Terrain::Clear, 0, 2, 10)]
    {
        let (_dir, config, mut world, map, units) = fixture_field(terrain, depth, 3).await;
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        encoded["constructed"][units[0].0.to_string()]["ground_elevation"] = first.into();
        if second == 10 {
            encoded["constructed"][units[1].0.to_string()]["free_fall"] =
                serde_json::to_value(FreeFall::new(10)).unwrap();
        } else {
            encoded["constructed"][units[1].0.to_string()]["ground_elevation"] = second.into();
        }
        world.btech = serde_json::from_value(encoded).unwrap();
        let untouched = world.btech.constructed_units()[&units[1]].clone();
        let center = HexCoordinate { x: 1, y: 1 };
        let mut flight = ArtilleryFlight::new(
            center,
            center,
            Weapon::LongTom,
            ArtilleryMode::Standard,
            ArtilleryOutcome::Hit,
        )
        .unwrap();
        let mut report = None;
        for _ in 0..10 {
            report = advance_artillery_flight(&mut world, map, &mut flight, rules()).unwrap();
        }
        let report = report.unwrap();
        assert_eq!(
            report.hits.iter().map(|hit| hit.unit).collect::<Vec<_>>(),
            [units[0]]
        );
        // Two levels from the burst: five points left of the twenty-five.
        assert_eq!(report.hits[0].impacts.len(), 1);
        assert_eq!(world.btech.constructed_units()[&units[1]], untouched);
        world.validate(&config).unwrap();
    }
}

/// Occupants across the cluster area receive 5-point groups; failed character admission restores
/// the world and the flight cursor.
#[tokio::test]
async fn artillery_cluster_world_packets_and_random_rollback() {
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::Clear, 0, 3).await;
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["constructed"][units[1].0.to_string()]["position"]["x"] = 0.into();
    encoded["constructed"][units[1].0.to_string()]["position"]["y"] = 0.into();
    encoded["constructed"][units[1].0.to_string()]["motion"]["point"] =
        serde_json::to_value(HexCoordinate { x: 0, y: 0 }.center()).unwrap();
    encoded["maps"][map.0.to_string()]["fire_dice"] =
        serde_json::to_value(Dice::seeded([7; 32])).unwrap();
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
                MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml"))
                    .unwrap(),
            )
            .unwrap();
            support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
            place_battle_unit(&mut world, id, map, x, y).unwrap();
        }
    }
    let center = HexCoordinate { x: 1, y: 1 };
    let mut flight = ArtilleryFlight::new(
        center,
        center,
        Weapon::LongTom,
        ArtilleryMode::Cluster,
        ArtilleryOutcome::Hit,
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
    // A Long Tom cluster round is twenty points in its hex and ten in each neighbor, in 5-point
    // groups on the Weapon table (Tactical Operations: Advanced Rules p. 152).
    assert_eq!(report.pattern.cells.len(), 7);
    for cell in &report.pattern.cells {
        let ArtilleryEffect::Damage {
            total,
            packet_size: 5,
            table: HitTable::Weapon,
        } = cell.effect
        else {
            panic!("Expected cluster packet")
        };
        assert_eq!(total, if cell.direct { 20 } else { 10 });
        for hit in report
            .hits
            .iter()
            .filter(|hit| hit.coordinate == cell.position)
        {
            assert_eq!(hit.impacts.len(), usize::from(total / 5));
        }
    }
    assert!(report.hits.len() >= 7);
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
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::Clear, 0, 3).await;
    let center = HexCoordinate { x: 1, y: 1 };
    for (index, mode) in [ArtilleryMode::Smoke, ArtilleryMode::Mine]
        .into_iter()
        .enumerate()
    {
        let flight =
            ArtilleryFlight::new(center, center, Weapon::LongTom, mode, ArtilleryOutcome::Hit)
                .unwrap();
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
                ArtilleryEffect::Smoke
            ));
            assert!(matches!(
                first[1].pattern.cells[0].effect,
                ArtilleryEffect::Mine { .. }
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
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::Clear, 0, 3).await;
    let center = HexCoordinate { x: 1, y: 1 };
    world
        .objects
        .get_mut(&units[1])
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let seed = (0..=255)
        .find(|&seed| Dice::seeded([seed; 32]).two_d6() == 12)
        .unwrap();
    world
        .btech
        .rewrite_unit_record(units[1], |record| {
            record["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
        })
        .unwrap();
    for mode in [ArtilleryMode::Smoke, ArtilleryMode::Standard] {
        enqueue_artillery(
            &mut world,
            map,
            units[0],
            ArtilleryFlight::new(center, center, Weapon::Thumper, mode, ArtilleryOutcome::Hit)
                .unwrap(),
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
        let (_dir, config, mut world, map, units) = fixture_field(Terrain::Clear, 0, 3).await;
        for (unit, pilot) in units.into_iter().zip([ObjectId(1), ObjectId(2)]) {
            stop_battle_unit(&mut world, unit, pilot).unwrap();
        }
        let center = HexCoordinate { x: 1, y: 1 };
        enqueue_artillery(&mut world, map, units[0], ArtilleryFlight::new(center, center, Weapon::LongTom, ArtilleryMode::Mine, ArtilleryOutcome::Hit).unwrap()).unwrap();
        let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        for _ in 0..7 { assert!(advance_artillery_action(&scripts, &config, rules()).unwrap().is_empty()); }
        let world_snapshot = scripts.world().clone();
        persistence::save(&config.database(), &world_snapshot).await.unwrap();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::raw_sql("CREATE TABLE artillery_updates(n INTEGER); CREATE TRIGGER count_artillery_updates AFTER UPDATE ON btech_artillery BEGIN INSERT INTO artillery_updates VALUES(1); END;").execute(&mut sql).await.unwrap();
        let (_address, shutdown, task, _lua, mut heartbeats) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        let loaded = heartbeats.until_saved(&config, 6, |loaded| loaded.btech.maps()[&map].artillery_shots().is_empty()).await;
        assert_eq!(loaded.btech.maps()[&map].minefields().len(), 1);
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
        let (_dir, config, mut world, map, units) = fixture_field(Terrain::Clear, 0, 3).await;
        for (unit, pilot) in units.into_iter().zip([ObjectId(1), ObjectId(2)]) {
                let _notices = stop_battle_unit(&mut world, unit, pilot).unwrap();
                assert_eq!(world.btech.constructed_units()[&unit].power(), Power::Off);
            }
            let center = HexCoordinate { x: 1, y: 1 };
        enqueue_artillery(&mut world, map, units[0], ArtilleryFlight::new(center, center, Weapon::LongTom, ArtilleryMode::Mine, ArtilleryOutcome::Hit).unwrap()).unwrap();
        let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        for _ in 0..9 { assert!(advance_artillery_action(&scripts, &config, rules()).unwrap().is_empty()); }
        let before = scripts.world().clone();
        persistence::save(&config.database(), &before).await.unwrap();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::query("CREATE TRIGGER deny_artillery BEFORE DELETE ON btech_artillery BEGIN SELECT RAISE(ABORT,'artillery failure'); END").execute(&mut sql).await.unwrap();
        let (_address, shutdown, task, _lua, mut heartbeats) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        heartbeats.attempt().await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech, before.btech);
        sqlx::query("DROP TRIGGER deny_artillery").execute(&mut sql).await.unwrap();
        let saved = heartbeats.until_saved(&config, 5, |saved| saved.btech.maps()[&map].artillery_shots().is_empty()).await;
        assert_eq!(saved.btech.maps()[&map].minefields().len(), 1);
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

/// The schema refuses a completed cursor, and loading rejects one longer than its launch allows.
#[tokio::test]
async fn artillery_queue_rejects_corrupt_saved_cursor() {
    use sqlx::Connection;
    let (_dir, config, mut world, map, units) = fixture_field(Terrain::Clear, 0, 3).await;
    let center = HexCoordinate { x: 1, y: 1 };
    enqueue_artillery(
        &mut world,
        map,
        units[0],
        ArtilleryFlight::new(
            center,
            center,
            Weapon::LongTom,
            ArtilleryMode::Mine,
            ArtilleryOutcome::Hit,
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
    let (_dir, _config, mut world, _, units) = fixture_field(Terrain::Clear, 0, 3).await;
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
        Character {
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
            CharacterValue {
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
