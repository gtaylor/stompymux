//! Live vehicle controls, continuous travel, boundary stops, and deterministic persisted replay.
use crate::support;
use stompymux_rs::*;

/// A running wheeled vehicle in a long, level corridor.
async fn fixture() -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    fixture_movement(VehicleMovement::Wheeled).await
}

/// Build either ground drivetrain for shared terrain-entry checks.
async fn fixture_movement(
    movement: VehicleMovement,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Road".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "road",
        MapAsset::from_cells(&format!(
            "20 3\n{}\n{}\n{}\n",
            ".0".repeat(20),
            ".0".repeat(20),
            ".0".repeat(20)
        ))
        .unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Truck".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    let mut template = VehicleTemplate::parse(
        "Flatbed_Truck",
        include_str!("../game/units/Flatbed_Truck.toml"),
    )
    .unwrap();
    template.movement = movement;
    if movement == VehicleMovement::Hover {
        template.set_cruising_mp(4);
    }
    create_battle_vehicle(&mut world, id, template).unwrap();
    place_battle_unit(&mut world, id, map, 2, 1).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0, true);
    }
    (dir, config, world, id, map)
}

#[tokio::test]
async fn vehicle_driving_replays_motion_and_coordinates_through_shared_controls() {
    let (_dir, config, mut world, id, map) = fixture().await;
    set_battle_heading(&mut world, id, ObjectId(1), 90.0).unwrap();
    for _ in 0..10 {
        advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
    }
    assert_eq!(world.btech.vehicles()[&id].motion().unwrap().heading, 90.0);
    set_battle_speed(&mut world, id, ObjectId(1), 86.0).unwrap();
    let before = world.clone();
    assert!(set_battle_speed(&mut world, id, ObjectId(2), 1.0).is_err());
    assert!(set_battle_speed(&mut world, id, ObjectId(1), f64::NAN).is_err());
    assert_eq!(world.btech, before.btech);
    for _ in 0..20 {
        advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
    }
    assert_eq!(world.btech.vehicles()[&id].motion().unwrap().speed, 86.0);
    assert!(world.btech.vehicles()[&id].position().unwrap().x > 2);
    assert_eq!(world.objects[&id].location, Some(map));
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    for _ in 0..10 {
        assert_eq!(
            advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap(),
            advance_battle_motion(&mut restored, MovementRules::STANDARD).unwrap()
        );
    }
    assert_eq!(world.btech, restored.btech);
    let mut edge = false;
    for _ in 0..200 {
        edge |= advance_battle_motion(&mut world, MovementRules::STANDARD)
            .unwrap()
            .iter()
            .any(|notice| notice.text.contains("Map edge"));
    }
    assert!(edge);
    assert_eq!(world.btech.vehicles()[&id].motion().unwrap().speed, 0.0);
    assert!(world.btech.vehicles()[&id].position().unwrap().x < 20);
}

#[tokio::test]
async fn native_and_lua_vehicle_controls_share_rollback_and_shutdown() {
    let (_dir, config, world, id, _) = fixture().await;
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let output = support::run_text(&scripts, &config, ObjectId(1), 1, "heading 90");
    assert!(output.contains("Desired heading"), "{output}");
    let output = support::run_text(&scripts, &config, ObjectId(1), 1, "speed cruise");
    assert!(output.contains("Desired speed"), "{output}");
    scripts
        .eval_callback::<()>(&format!("btech.unit.speed({},1,'flank')", id.0))
        .unwrap();
    assert_eq!(
        scripts.world().btech.vehicles()[&id]
            .motion()
            .unwrap()
            .desired_speed,
        86.0
    );
    let before = scripts.world().clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.speed({},1,'stop'); error('abort')",
                id.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before.btech);
    let output = support::run_text(&scripts, &config, ObjectId(1), 1, "speed");
    assert!(output.contains("speed"), "{output}");
    let output = support::run_text(&scripts, &config, ObjectId(1), 1, "shutdown");
    assert!(output.contains("shut down"), "{output}");
    assert!(
        !scripts.world().btech.vehicles()[&id]
            .motion()
            .unwrap()
            .active()
    );
}

/// Authored wall and high-water tiles use ordinary ground elevation rules; buildings are entered.
#[tokio::test]
async fn vehicle_authored_terrain_shares_elevation_hazards_and_replay() {
    for movement in [
        VehicleMovement::Tracked,
        VehicleMovement::Wheeled,
        VehicleMovement::Hover,
    ] {
        for terrain in [Terrain::Building, Terrain::Wall] {
            for height in [0, 1, 3] {
                let (_dir, config, mut world, id, map) = fixture_movement(movement).await;
                set_battle_heading(&mut world, id, ObjectId(1), 90.0).unwrap();
                for _ in 0..30 {
                    advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
                }
                set_battle_speed(&mut world, id, ObjectId(1), 64.5).unwrap();
                let mut saved = serde_json::to_value(&world.btech).unwrap();
                for cell in (0..3).flat_map(|y| (3..9).map(move |x| y * 20 + x)) {
                    crate::support::set_hex_elevation(
                        &mut saved["maps"][map.0.to_string()]["terrain"][cell],
                        height,
                    );
                }
                // Ordinary grass hills are the independent comparison for the same height profile.
                world.btech = serde_json::from_value(saved.clone()).unwrap();
                let mut ordinary = world.clone();
                for cell in (0..3)
                    .flat_map(|y| ((if height == 0 { 2 } else { 3 })..9).map(move |x| y * 20 + x))
                {
                    crate::support::set_hex_terrain(
                        &mut saved["maps"][map.0.to_string()]["terrain"][cell],
                        terrain,
                    );
                }
                saved["maps"][map.0.to_string()]["decorations"]["23"] = serde_json::json!({"kind":"smoke", "remaining":120, "object_duration":120, "order":-1});
                world.btech = serde_json::from_value(saved).unwrap();
                if terrain == Terrain::Building && height > 0 {
                    // A building below roof height is entered, not climbed (Total Warfare
                    // p. 167): the drive pushes through its walls at ground level, rolling
                    // against the medium class and costing the hex a tenth of its tonnage.
                    let tons = world.btech.vehicles()[&id].definition().tons;
                    let mut entered = false;
                    let mut breached = false;
                    for tick in 0..45 {
                        let notices =
                            advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
                        entered |= world.btech.vehicles()[&id].position().unwrap().x >= 3;
                        breached |= notices
                            .iter()
                            .any(|notice| notice.text.contains("push through the walls"));
                        if tick == 20 {
                            persistence::save(&config.database(), &world).await.unwrap();
                            let restored = persistence::load(&config.database()).await.unwrap();
                            assert_eq!(restored.btech, world.btech);
                            world = restored;
                        }
                    }
                    assert!(entered && breached);
                    // The truck stays on the ground floor of the first hex it broke into; a
                    // failed roll can wreck so light a vehicle, so its material is not compared.
                    assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(0));
                    let inside = battle_unit_inside_building(&world, id).unwrap().unwrap();
                    assert_eq!(inside.level, 0);
                    assert_eq!(
                        inside.cf(),
                        ConstructionClass::Medium.construction_factor() - tons.div_ceil(10)
                    );
                    assert!(!world.btech.vehicles()[&id].flooded());
                    world.validate(&config).unwrap();
                    continue;
                }
                let material = world.btech.vehicles()[&id].sections().clone();
                let mut entered = false;
                let mut hazard = false;
                for tick in 0..45 {
                    let notices =
                        advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
                    let expected =
                        advance_battle_motion(&mut ordinary, MovementRules::STANDARD).unwrap();
                    assert_eq!(notices, expected);
                    assert_eq!(world.btech.vehicles()[&id], ordinary.btech.vehicles()[&id]);
                    entered |= world.btech.vehicles()[&id].position().unwrap().x >= 3;
                    hazard |= notices.iter().any(|notice| {
                        notice.text.contains("steep") || notice.text.contains("cliff")
                    });
                    if tick == 20 {
                        persistence::save(&config.database(), &world).await.unwrap();
                        let restored = persistence::load(&config.database()).await.unwrap();
                        assert_eq!(restored.btech, world.btech);
                        world = restored;
                    }
                }
                if height <= 1 {
                    assert!(entered);
                    assert_eq!(world.btech.vehicles()[&id].sections(), &material);
                } else {
                    assert!(!entered);
                    assert!(hazard);
                }
                assert!(!world.btech.vehicles()[&id].flooded());
                world.validate(&config).unwrap();
            }
        }
    }
}

/// A saved straight segment with a caller-selected movement rate.
async fn fast_corridor(rate: i64) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world, id, map) = fixture().await;
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    let unit = &mut saved["vehicles"][id.0.to_string()];
    unit["motion"]["heading"] = 90.0.into();
    unit["motion"]["desired_heading"] = 90.0.into();
    unit["motion"]["speed"] = 86.0.into();
    unit["motion"]["desired_speed"] = 86.0.into();
    saved["maps"][map.0.to_string()]["movement_modifier"] = rate.into();
    world.btech = serde_json::from_value(saved).unwrap();
    (dir, config, world, id, map)
}

/// Candidate travel supplies a fixture route; mine assertions independently track packet expenditure.
fn corridor_entries(world: &World, id: ObjectId) -> Vec<HexCoordinate> {
    let origin = world.btech.vehicles()[&id].motion().unwrap().point;
    let mut clear = world.clone();
    advance_battle_motion(&mut clear, MovementRules::STANDARD).unwrap();
    let end = clear.btech.vehicles()[&id].motion().unwrap().point;
    origin.trace(end).unwrap().into_iter().skip(1).collect()
}

/// Prevent incidental critical cascades from obscuring the per-crossing mine packet checks.
fn mine_movement_rules() -> MovementRules {
    MovementRules::STANDARD
}

/// A conventional field of fewer than five points: it needs a 9 or more to go off
/// (Tactical Operations p. 176) and is cleared, with everything colocated, after one blast.
const DISPOSABLE_MINE_TARGET: u8 = 9;

/// Search the numbered seeds for a stream whose opening draws satisfy `accept`.
fn search_dice(accept: impl Fn(&mut Dice) -> bool) -> Dice {
    (0u32..100000)
        .find_map(|number| {
            let mut bytes = [0; 32];
            bytes[..4].copy_from_slice(&number.to_le_bytes());
            let initial = Dice::seeded(bytes);
            accept(&mut initial.clone()).then_some(initial)
        })
        .unwrap()
}

/// Pin the truck's dice so its first 2d6 sets off a field below the five-point minimum.
fn seed_detonation(world: &mut World, id: ObjectId) {
    let dice = search_dice(|dice| dice.two_d6() >= DISPOSABLE_MINE_TARGET);
    world.btech.set_unit_dice(id, dice).unwrap();
}

/// Unit dice one disposable mine crossing consumes: the detonation roll, the hit location and
/// the armor stage's roll.
const CROSSING_ROLLS: usize = 3;

/// Place a scripted trigger before each disposable mine, so field removal retains its queued callback.
/// Every crossing's detonation roll succeeds and its one-point blast lands on a front row with
/// neither a motive roll nor a through-armor critical, so the truck keeps its speed.
fn seed_corridor_mines(
    world: &mut World,
    id: ObjectId,
    map: ObjectId,
    entries: &[HexCoordinate],
) -> Dice {
    let seed = search_dice(|dice| {
        entries.iter().all(|_| {
            if dice.two_d6() < DISPOSABLE_MINE_TARGET {
                return false;
            }
            let plain = matches!(dice.two_d6(), 6..=8 | 10 | 11);
            dice.two_d6();
            plain
        })
    });
    world.btech.set_unit_dice(id, seed.clone()).unwrap();
    for (index, &coordinate) in entries.iter().enumerate() {
        for (offset, kind, strength) in [(0, MineKind::Trigger, 0), (1, MineKind::Standard, 1)] {
            set_minefield(
                world,
                map,
                index as u32 * 2 + offset,
                Some(Minefield {
                    coordinate,
                    kind,
                    strength,
                    extra: 0,
                    owner: ObjectId(1),
                }),
            )
            .unwrap();
        }
    }
    seed
}

/// Give the test truck enough protection for several independent weak blasts.
fn reinforce_truck(world: &mut World, id: ObjectId) {
    world
        .btech
        .rewrite_unit_record(id, |record| {
            for section in ["left", "right", "front", "rear"] {
                record["definition"]["sections"][section]["armor"] = 100.into();
                record["sections"][section]["armor"] = 100.into();
            }
        })
        .unwrap();
}

#[tokio::test]
async fn vehicle_movement_checks_each_mined_crossing_and_replays_expenditure() {
    let (_dir, config, mut world, id, map) = fast_corridor(2000).await;
    reinforce_truck(&mut world, id);
    let entries = corridor_entries(&world, id);
    assert!(entries.len() > 1);
    let mut expected = world.clone();
    advance_battle_motion(&mut expected, mine_movement_rules()).unwrap();
    let seed = seed_corridor_mines(&mut world, id, map, &entries);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    let notices = advance_battle_motion(&mut world, mine_movement_rules()).unwrap();
    assert_eq!(
        advance_battle_motion(&mut loaded, mine_movement_rules()).unwrap(),
        notices
    );
    assert_eq!(loaded.btech, world.btech);
    assert_eq!(
        world.btech.vehicles()[&id].motion(),
        expected.btech.vehicles()[&id].motion()
    );
    assert!(world.btech.maps()[&map].minefields().is_empty());
    assert_eq!(
        notices
            .iter()
            .filter(|notice| notice.unit == id && notice.text.contains("you trigger a mine"))
            .count(),
        entries.len()
    );
    let armor: u16 = world.btech.vehicles()[&id]
        .sections()
        .values()
        .map(|section| section.armor)
        .sum();
    assert_eq!(400 - armor, entries.len() as u16);
    let mut dice = seed;
    for _ in 0..entries.len() * CROSSING_ROLLS {
        dice.two_d6();
    }
    assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn vehicle_mine_callbacks_publish_once_and_restore_failed_movement() {
    let (dir, config, mut world, id, map) = fast_corridor(2000).await;
    reinforce_truck(&mut world, id);
    let entries = corridor_entries(&world, id);
    seed_corridor_mines(&mut world, id, map, &entries);
    world.objects.get_mut(&id).unwrap().lua_parent = "vehicle_mines.lua".into();
    std::fs::write(
        dir.path().join("lua/object_logic/vehicle_mines.lua"),
        r#"return {events={on_mech_mine_trigger=function(ctx)
        assert(ctx.object==ctx.enactor and ctx.cause==ctx.object)
        assert(ctx.operation=='mine_trigger')
        local s=mux.world.object(ctx.object):state('mine')
        s:set('count',(s:get('count') or 0)+1)
        if mine_fail then error('vehicle mine callback abort') end
    end}}"#,
    )
    .unwrap();
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    scripts.inspect_lua().load("mine_fail=true").exec().unwrap();
    let error = advance_battle_motion_action(&scripts, &config, mine_movement_rules()).unwrap_err();
    assert!(format!("{error:#}").contains("vehicle mine callback abort"));
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
    advance_battle_motion_action(&scripts, &config, mine_movement_rules()).unwrap();
    let count: usize = scripts
        .eval_callback(&format!(
            "return mux.world.object({}):state('mine'):get('count')",
            id.0
        ))
        .unwrap();
    assert_eq!(count, entries.len());
    let committed = scripts.world().clone();
    persistence::save(&config.database(), &committed)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        committed.btech
    );
}

#[tokio::test]
async fn unrelated_mines_and_motion_inside_one_hex_do_not_trigger_or_stop_vehicles() {
    let (_dir, config, mut world, id, map) = fast_corridor(100).await;
    let before = world.clone();
    set_minefield(
        &mut world,
        map,
        0,
        Some(Minefield {
            coordinate: HexCoordinate { x: 19, y: 2 },
            kind: MineKind::Standard,
            strength: 10,
            extra: 0,
            owner: ObjectId(1),
        }),
    )
    .unwrap();
    let mut clear = before;
    let expected = advance_battle_motion(&mut clear, mine_movement_rules()).unwrap();
    assert_eq!(
        advance_battle_motion(&mut world, mine_movement_rules()).unwrap(),
        expected
    );
    assert_eq!(world.btech.vehicles()[&id], clear.btech.vehicles()[&id]);
    // Start from the center at very low speed so this update never enters another hex.
    let coordinate = world.btech.vehicles()[&id]
        .motion()
        .unwrap()
        .point
        .containing_hex()
        .unwrap();
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["motion"]["point"] = serde_json::to_value(coordinate.center()).unwrap();
            record["motion"]["speed"] = 1.0.into();
            record["motion"]["desired_speed"] = 1.0.into();
        })
        .unwrap();
    set_minefield(
        &mut world,
        map,
        1,
        Some(Minefield {
            coordinate,
            kind: MineKind::Standard,
            strength: 10,
            extra: 0,
            owner: ObjectId(1),
        }),
    )
    .unwrap();
    let before = world.btech.vehicles()[&id].clone();
    assert!(
        advance_battle_motion(&mut world, mine_movement_rules())
            .unwrap()
            .is_empty()
    );
    assert_eq!(world.btech.vehicles()[&id].position(), before.position());
    assert_eq!(world.btech.vehicles()[&id].sections(), before.sections());
    assert_ne!(
        world.btech.vehicles()[&id].motion().unwrap().point,
        before.motion().unwrap().point
    );
    assert_eq!(world.btech.maps()[&map].minefields().len(), 2);
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn mine_blast_disables_a_later_vehicle_before_its_scheduled_movement() {
    let (_dir, config, mut world, id, map) = fast_corridor(2000).await;
    seed_detonation(&mut world, id);
    let entries = corridor_entries(&world, id);
    let first = entries[0];
    // Only a command-detonated field reaches the neighbouring hexes, so the second truck
    // waits in the mined hex itself.
    let other = world.create(&config, "Second truck".into(), Kind::Thing);
    world.objects.get_mut(&other).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        other,
        VehicleTemplate::parse(
            "Flatbed_Truck",
            include_str!("../game/units/Flatbed_Truck.toml"),
        )
        .unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, other, support::FIXTURE_DICE_SEED);
    place_battle_unit(
        &mut world,
        other,
        map,
        i64::from(first.x),
        i64::from(first.y),
    )
    .unwrap();
    world
        .btech
        .rewrite_unit_record(other, |record| {
            record["power"] = serde_json::to_value(Power::Running).unwrap();
            record["motion"]["speed"] = 86.0.into();
            record["motion"]["desired_speed"] = 86.0.into();
        })
        .unwrap();
    let second_point = world.btech.vehicles()[&other].motion().unwrap().point;
    set_minefield(
        &mut world,
        map,
        0,
        Some(Minefield {
            coordinate: first,
            kind: MineKind::Standard,
            strength: 4,
            extra: 0,
            owner: ObjectId(1),
        }),
    )
    .unwrap();
    let before = world.clone();
    let notices = advance_battle_motion(&mut world, mine_movement_rules()).unwrap();
    for target in [id, other] {
        assert!(world.btech.vehicles()[&target].is_destroyed());
        assert_eq!(world.btech.vehicles()[&target].power(), Power::Off);
    }
    assert_eq!(
        world.btech.vehicles()[&other].motion().unwrap().point,
        second_point
    );
    assert_eq!(
        world.btech.vehicles()[&id]
            .motion()
            .unwrap()
            .point
            .containing_hex()
            .unwrap(),
        first
    );
    assert!(world.btech.maps()[&map].minefields().is_empty());
    let mut replay = before;
    assert_eq!(
        advance_battle_motion(&mut replay, mine_movement_rules()).unwrap(),
        notices
    );
    assert_eq!(replay.btech, world.btech);
    world.validate(&config).unwrap();
}

/// Failed mine deletion rolls back the vehicle's movement and damage before the heartbeat retries.
#[tokio::test]
async fn vehicle_mine_movement_retries_a_failed_server_save() {
    tokio::task::LocalSet::new().run_until(async {
        use sqlx::Connection;
        let (_dir, config, mut world, id, map) = fast_corridor(2000).await;
        seed_detonation(&mut world, id);
        let first = corridor_entries(&world, id)[0];
        set_minefield(&mut world, map, 0, Some(Minefield {
            coordinate: first, kind: MineKind::Standard, strength: 4, extra: 0, owner: ObjectId(1),
        })).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::raw_sql("CREATE TRIGGER deny_vehicle_mine_delete BEFORE DELETE ON btech_map_objects WHEN OLD.object_type=3 BEGIN SELECT RAISE(ABORT,'vehicle mine save failure'); END;").execute(&mut sql).await.unwrap();
        let (_address, shutdown, server, _lua, mut heartbeats) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        heartbeats.attempt().await;
        let failed = persistence::load(&config.database()).await.unwrap();
        assert_eq!(failed.btech.vehicles()[&id], world.btech.vehicles()[&id]);
        assert_eq!(failed.btech.maps()[&map].minefields(), world.btech.maps()[&map].minefields());
        sqlx::raw_sql("DROP TRIGGER deny_vehicle_mine_delete;").execute(&mut sql).await.unwrap();
        let committed = heartbeats.until_saved(&config, 5, |committed| committed.btech.maps()[&map].minefields().is_empty()).await;
        let unit = &committed.btech.vehicles()[&id];
        assert!(unit.is_destroyed());
        assert_eq!(unit.motion().unwrap().point.containing_hex().unwrap(), first);
        assert_eq!(unit.motion().unwrap().speed, 0.0);
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        server.await.unwrap().unwrap();
    }).await;
}

#[tokio::test]
async fn ground_vehicles_cross_crowded_hexes_without_stacking_effects() {
    let (_dir, config, mut world, id, map) = fixture().await;
    set_battle_heading(&mut world, id, ObjectId(1), 90.0).unwrap();
    for _ in 0..10 {
        advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
    }
    set_battle_speed(&mut world, id, ObjectId(1), 86.0).unwrap();
    let mut empty = world.clone();
    let mut occupants = Vec::new();
    for index in 0..8 {
        let other = world.create(&config, format!("Occupant {index}"), Kind::Thing);
        world.objects.get_mut(&other).unwrap().home = Some(ObjectId(config.home()));
        if index % 2 == 0 {
            create_battle_vehicle(
                &mut world,
                other,
                VehicleTemplate::parse(
                    "Flatbed_Truck",
                    include_str!("../game/units/Flatbed_Truck.toml"),
                )
                .unwrap(),
            )
            .unwrap();
            support::seed_object_dice(&mut world, other, support::FIXTURE_DICE_SEED);
        } else {
            create_battle_unit(
                &mut world,
                other,
                MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml"))
                    .unwrap(),
            )
            .unwrap();
            support::seed_object_dice(&mut world, other, support::FIXTURE_DICE_SEED);
        }
        place_battle_unit(&mut world, other, map, 3, 1).unwrap();
        occupants.push(other);
    }
    let before = world.btech.clone();
    let mut passed = false;
    for _ in 0..100 {
        assert_eq!(
            advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap(),
            advance_battle_motion(&mut empty, MovementRules::STANDARD).unwrap()
        );
        assert_eq!(world.btech.vehicles()[&id], empty.btech.vehicles()[&id]);
        if world.btech.vehicles()[&id].position().unwrap().x > 3 {
            passed = true;
            break;
        }
    }
    assert!(passed);
    for other in occupants {
        assert_eq!(
            world.btech.vehicles().get(&other),
            before.vehicles().get(&other)
        );
        assert_eq!(
            world.btech.constructed_units().get(&other),
            before.constructed_units().get(&other)
        );
    }
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

#[tokio::test]
async fn tracked_and_wheeled_vehicles_stop_at_deep_water_and_hovercraft_cross() {
    for (movement, waterproof, depth) in [
        (VehicleMovement::Wheeled, false, 1),
        (VehicleMovement::Tracked, false, 1),
        (VehicleMovement::Tracked, true, 1),
        (VehicleMovement::Wheeled, false, 0),
        (VehicleMovement::Hover, false, 2),
    ] {
        let (_dir, config, mut world, id, map) = fixture_movement(movement).await;
        set_battle_heading(&mut world, id, ObjectId(1), 90.0).unwrap();
        for _ in 0..10 {
            advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
        }
        let maximum = world.btech.vehicles()[&id].maximum_speed();
        set_battle_speed(&mut world, id, ObjectId(1), maximum.min(86.0)).unwrap();
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        let unit = &mut saved["vehicles"][id.0.to_string()];
        let dice = unit["dice"].clone();
        if waterproof {
            unit["definition"]["attributes"]["specials"] = "Waterproof_Tech".into();
        }
        for row in 0..3 {
            for x in 3..20 {
                let tile = &mut saved["maps"][map.0.to_string()]["terrain"][row * 20 + x];
                crate::support::set_hex(tile, Terrain::Water, depth);
            }
        }
        world.btech = serde_json::from_value(saved).unwrap();
        // Water a level or more deep is prohibited terrain for tracks and wheels (Total
        // Warfare p. 52): the vehicle stops at its edge without a roll. Hovercraft and
        // amphibious vehicles cross on the surface.
        let stops = depth > 0 && !waterproof && movement != VehicleMovement::Hover;
        let mut encountered = false;
        for _ in 0..100 {
            let before = world.clone();
            let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
            let unit = &world.btech.vehicles()[&id];
            let stopped = notices
                .iter()
                .any(|n| n.text.contains("cannot cross water that deep"));
            if !stopped && unit.position().unwrap().x < 3 {
                continue;
            }
            encountered = true;
            assert_eq!(stopped, stops, "{movement:?} depth={depth}");
            assert!(!unit.flooded());
            assert_eq!(unit.sections(), before.btech.vehicles()[&id].sections());
            assert_eq!(unit.pilot_injuries(), 0);
            assert_eq!(serde_json::to_value(unit).unwrap()["dice"], dice);
            if stops {
                assert!(!unit.motion().unwrap().active());
                assert_eq!(unit.position().unwrap().x, 2);
                assert_eq!(
                    unit.motion().unwrap().point,
                    before.btech.vehicles()[&id].motion().unwrap().point
                );
            } else {
                assert!(unit.motion().unwrap().active());
                assert_eq!(unit.position().unwrap().x, 3);
            }
            persistence::save(&config.database(), &before)
                .await
                .unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(
                notices,
                advance_battle_motion(&mut restored, MovementRules::STANDARD).unwrap()
            );
            assert_eq!(world.btech, restored.btech);
            world.validate(&config).unwrap();
            break;
        }
        assert!(encountered, "{movement:?} depth={depth}");
    }
}

#[tokio::test]
async fn prohibited_terrain_stops_vehicles_at_its_edge_without_dice() {
    for (movement, terrain, stops) in [
        (VehicleMovement::Tracked, Terrain::HeavyWoods, true),
        (VehicleMovement::Tracked, Terrain::LightWoods, false),
        (VehicleMovement::Tracked, Terrain::Rough, false),
        (VehicleMovement::Tracked, Terrain::LightJungle, true),
        (VehicleMovement::Tracked, Terrain::UltraRough, true),
        (VehicleMovement::Wheeled, Terrain::LightWoods, true),
        (VehicleMovement::Wheeled, Terrain::HeavyWoods, true),
        (VehicleMovement::Wheeled, Terrain::Rough, true),
        (VehicleMovement::Wheeled, Terrain::Rubble, true),
        // Ultra rubble inherits rubble's restrictions (Tactical Operations p. 32).
        (VehicleMovement::Wheeled, Terrain::UltraRubble, true),
        (VehicleMovement::Wheeled, Terrain::DeepSnow, true),
        (VehicleMovement::Wheeled, Terrain::Sand, false),
        (VehicleMovement::Hover, Terrain::HeavyWoods, true),
        (VehicleMovement::Hover, Terrain::LightWoods, true),
        (VehicleMovement::Hover, Terrain::Rough, false),
    ] {
        let (_dir, config, mut world, id, map) = fixture_movement(movement).await;
        let maximum = world.btech.vehicles()[&id].maximum_speed();
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        let unit = &mut saved["vehicles"][id.0.to_string()];
        unit["motion"]["heading"] = 90.0.into();
        unit["motion"]["desired_heading"] = 90.0.into();
        unit["motion"]["speed"] = maximum.into();
        unit["motion"]["desired_speed"] = maximum.into();
        let dice = unit["dice"].clone();
        for row in 0..3 {
            for x in 3..20 {
                let tile = &mut saved["maps"][map.0.to_string()]["terrain"][row * 20 + x];
                crate::support::set_hex(tile, terrain, 0);
            }
        }
        world.btech = serde_json::from_value(saved).unwrap();
        let mut entered = false;
        for _ in 0..100 {
            let before = world.clone();
            let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
            let unit = &world.btech.vehicles()[&id];
            let stopped = notices
                .iter()
                .any(|n| n.text.starts_with("You cannot enter "));
            if !stopped && unit.position().unwrap().x < 3 {
                continue;
            }
            entered = true;
            assert_eq!(stopped, stops, "{movement:?} into {terrain:?}: {notices:?}");
            assert_eq!(unit.sections(), before.btech.vehicles()[&id].sections());
            if terrain == Terrain::Sand {
                // Sand is soft ground: entering it rolls against bogging down.
                let mut expected: Dice = serde_json::from_value(dice.clone()).unwrap();
                expected.two_d6();
                assert_eq!(
                    serde_json::to_value(unit).unwrap()["dice"],
                    serde_json::to_value(expected).unwrap()
                );
            } else {
                assert_eq!(serde_json::to_value(unit).unwrap()["dice"], dice);
            }
            if stops {
                assert!(!unit.motion().unwrap().active());
                assert_eq!(unit.position().unwrap().x, 2);
                assert_eq!(
                    unit.motion().unwrap().point,
                    before.btech.vehicles()[&id].motion().unwrap().point
                );
            } else {
                assert!(unit.motion().unwrap().active() || terrain == Terrain::Sand);
                assert_eq!(unit.position().unwrap().x, 3);
            }
            persistence::save(&config.database(), &before)
                .await
                .unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(
                notices,
                advance_battle_motion(&mut restored, MovementRules::STANDARD).unwrap()
            );
            assert_eq!(world.btech, restored.btech);
            world.validate(&config).unwrap();
            break;
        }
        assert!(entered, "{movement:?} into {terrain:?}");
    }
}

#[tokio::test]
async fn tracked_and_wheeled_ice_entry_replays_shared_fracture_and_waterproof_roles() {
    for (movement, fracture, depth, waterproof) in [
        (VehicleMovement::Tracked, false, 3, false),
        (VehicleMovement::Wheeled, false, 3, false),
        (VehicleMovement::Tracked, true, 1, false),
        (VehicleMovement::Wheeled, true, 1, true),
        (VehicleMovement::Tracked, true, 0, false),
    ] {
        let (_dir, config, mut world, id, map) = fixture_movement(movement).await;
        // The fracture die comes first; a vehicle that stays on the ice then rolls at +0 to keep
        // its grip, and these seeds keep it.
        let seed = (0..=255)
            .find(|seed| {
                let mut dice = Dice::seeded([*seed; 32]);
                (dice.d6() == 1) == fracture && (fracture || dice.two_d6() >= 11)
            })
            .unwrap();
        let neighbor = world.create(&config, "Waterproof neighbor".into(), Kind::Thing);
        world.objects.get_mut(&neighbor).unwrap().home = Some(ObjectId(config.home()));
        let mut template = VehicleTemplate::parse(
            "Flatbed_Truck",
            include_str!("../game/units/Flatbed_Truck.toml"),
        )
        .unwrap();
        template
            .attributes
            .insert("specials".into(), "Waterproof_Tech".into());
        create_battle_vehicle(&mut world, neighbor, template).unwrap();
        support::seed_object_dice(&mut world, neighbor, support::FIXTURE_DICE_SEED);
        place_battle_unit(&mut world, neighbor, map, 3, 2).unwrap();
        let mech = world.create(&config, "Neighbor Mech".into(), Kind::Thing);
        world.objects.get_mut(&mech).unwrap().home = Some(ObjectId(config.home()));
        create_battle_unit(
            &mut world,
            mech,
            MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap(),
        )
        .unwrap();
        support::seed_object_dice(&mut world, mech, support::FIXTURE_DICE_SEED);
        place_battle_unit(&mut world, mech, map, 3, 2).unwrap();
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        let unit = &mut saved["vehicles"][id.0.to_string()];
        unit["motion"]["heading"] = 120.0.into();
        unit["motion"]["desired_heading"] = 120.0.into();
        unit["motion"]["speed"] = 86.0.into();
        unit["motion"]["desired_speed"] = 86.0.into();
        unit["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
        if waterproof {
            unit["definition"]["attributes"]["specials"] = "Waterproof_Tech".into();
        }
        saved["maps"][map.0.to_string()]["terrain"][43] =
            serde_json::to_value(Hex::new(Terrain::Ice, depth)).unwrap();
        saved["maps"][map.0.to_string()]["decorations"]["43"] =
            serde_json::json!({"kind":"smoke", "remaining":120, "object_duration":120, "order":-1});
        world.btech = serde_json::from_value(saved).unwrap();
        let mut entered = false;
        for _ in 0..100 {
            let before = world.clone();
            let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
            let unit = &world.btech.vehicles()[&id];
            if unit.position().unwrap().x < 3 {
                continue;
            }
            entered = true;
            assert_eq!(
                unit.position().unwrap().y,
                2,
                "entered {:?}, old {:?}, notices {notices:?}",
                unit.position(),
                before.btech.vehicles()[&id].position()
            );
            assert_eq!(
                world.btech.maps()[&map].base_hex(3, 2).unwrap().terrain(),
                if fracture {
                    Terrain::Water
                } else {
                    Terrain::Ice
                }
            );
            assert_eq!(
                notices
                    .iter()
                    .any(|n| n.unit == id && n.text == "You break the ice!"),
                fracture
            );
            assert_eq!(unit.flooded(), fracture && depth > 0 && !waterproof);
            assert_eq!(
                world.btech.vehicles()[&neighbor].flooded(),
                fracture && depth > 0
            );
            assert_eq!(unit.motion().unwrap().active(), !(fracture && depth > 0));
            if !fracture || depth == 0 {
                let mut dice = Dice::seeded([seed; 32]);
                dice.d6();
                if !fracture {
                    dice.two_d6();
                }
                assert_eq!(
                    serde_json::to_value(unit).unwrap()["dice"],
                    serde_json::to_value(dice).unwrap()
                );
                assert_eq!(
                    world.btech.vehicles()[&neighbor],
                    before.btech.vehicles()[&neighbor]
                );
                assert_eq!(
                    world.btech.constructed_units()[&mech],
                    before.btech.constructed_units()[&mech]
                );
            } else {
                assert_eq!(
                    world.btech.constructed_units()[&mech].posture(),
                    Posture::Prone
                );
                assert!(notices.iter().any(|n| {
                    n.unit == neighbor && n.text == "Water renders your vehicle inoperable."
                }));
                // The falling vehicle's crew rolls nothing (Total Warfare p. 152).
                assert!(
                    !notices
                        .iter()
                        .any(|n| n.unit == id && n.text.contains("personal"))
                );
                let mut rejected = before.clone();
                rejected
                    .objects
                    .get_mut(&id)
                    .unwrap()
                    .flags
                    .insert(Flag::InCharacter);
                let snapshot = rejected.btech.clone();
                assert!(advance_battle_motion(&mut rejected, MovementRules::STANDARD).is_err());
                assert_eq!(snapshot, rejected.btech);
            }
            persistence::save(&config.database(), &before)
                .await
                .unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(
                notices,
                advance_battle_motion(&mut restored, MovementRules::STANDARD).unwrap()
            );
            assert_eq!(world.btech, restored.btech);
            world.validate(&config).unwrap();
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
            break;
        }
        assert!(entered);
    }
}

/// An amphibious vehicle crosses deep water on the surface at two points per hex (Total
/// Warfare p. 52 note 7, p. 56): it rides at the water line, is never immersed, and moves at
/// half speed, so it steps onto an ice sheet from the surface like any other vehicle.
#[tokio::test]
async fn amphibious_vehicle_rides_the_water_surface_at_half_speed() {
    let (_dir, config, mut world, id, map) = fixture_movement(VehicleMovement::Tracked).await;
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    let unit = &mut saved["vehicles"][id.0.to_string()];
    unit["definition"]["attributes"]["specials"] = "Waterproof_Tech".into();
    unit["motion"]["heading"] = 90.0.into();
    unit["motion"]["desired_heading"] = 90.0.into();
    unit["motion"]["speed"] = 86.0.into();
    unit["motion"]["desired_speed"] = 86.0.into();
    let dice = unit["dice"].clone();
    for row in 0..3 {
        for x in 0..20 {
            saved["maps"][map.0.to_string()]["terrain"][row * 20 + x] = serde_json::to_value(
                Hex::new(if x < 6 { Terrain::Water } else { Terrain::Ice }, 2),
            )
            .unwrap();
        }
    }
    world.btech = serde_json::from_value(saved).unwrap();
    let mut entered = false;
    for _ in 0..100 {
        let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
        let unit = &world.btech.vehicles()[&id];
        if unit.position().unwrap().x < 6 {
            assert!(notices.is_empty(), "{notices:?}");
            assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(0));
            assert!(!unit.flooded());
            // Two points per water hex: the throttle settles at half the 86 kph requested.
            if unit.position().unwrap().x >= 4 {
                assert!((unit.motion().unwrap().speed - 43.0).abs() < 1e-9);
            }
            assert_eq!(serde_json::to_value(unit).unwrap()["dice"], dice);
            continue;
        }
        entered = true;
        // Stepping from the surface onto the ice sheet takes the ordinary fracture and
        // driving rolls, so the dice have moved on.
        assert_ne!(serde_json::to_value(unit).unwrap()["dice"], dice);
        assert!(notices.iter().any(|n| n.text.contains("ease onto the ice")));
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
        break;
    }
    assert!(entered);
}

/// Bridge and ice collisions drop an in-character vehicle and publish the fall; the crew is
/// never injured by the fall itself (Total Warfare p. 152).
#[tokio::test]
async fn character_movement_collisions_publish_falls_without_crew_injury() {
    use std::{cell::RefCell, rc::Rc};
    for hazard in ["bridge", "ice"] {
        for initial in [0, 9] {
            let movement = if hazard == "bridge" {
                VehicleMovement::Hover
            } else {
                VehicleMovement::Wheeled
            };
            let (_dir, config, mut world, id, map) = fixture_movement(movement).await;
            release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
            world
                .objects
                .get_mut(&id)
                .unwrap()
                .flags
                .insert(Flag::InCharacter);
            world
                .objects
                .get_mut(&ObjectId(2))
                .unwrap()
                .flags
                .remove(Flag::Wizard);
            world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(id);
            set_battle_character(
                &mut world,
                ObjectId(2),
                Character {
                    bruise: 0,
                    lethal: 0,
                    build: 5,
                    reflexes: 5,
                    intuition: 5,
                    learn: 5,
                    charisma: 5,
                },
            )
            .unwrap();
            support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
            assign_battle_pilot(&mut world, id, ObjectId(2)).unwrap();
            support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
            if initial > 0 {
                injure_battle_character_pilot(&mut world, id, initial, false).unwrap();
            }
            let maximum = world.btech.vehicles()[&id].maximum_speed();
            let mut saved = serde_json::to_value(&world.btech).unwrap();
            let unit = &mut saved["vehicles"][id.0.to_string()];
            unit["motion"]["heading"] = 90.0.into();
            unit["motion"]["desired_heading"] = 90.0.into();
            unit["motion"]["speed"] = maximum.into();
            unit["motion"]["desired_speed"] = maximum.into();
            unit["piloting_damage"] = 100.into();
            unit["definition"]["attributes"]["specials"] = "ICEEngine_Tech CritProof_Tech".into();
            if hazard == "ice" {
                let seed = (0..=255)
                    .find(|seed| Dice::seeded([*seed; 32]).d6() == 1)
                    .unwrap();
                unit["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
            }
            if hazard == "bridge" {
                unit["under_bridge"] = true.into();
            }
            for row in 0..3 {
                for x in 0..20 {
                    let tile = &mut saved["maps"][map.0.to_string()]["terrain"][row * 20 + x];
                    let (terrain, elevation) = match (hazard, x >= 3) {
                        ("bridge", false) => (Terrain::Bridge, 3),
                        ("bridge", true) => (Terrain::Bridge, 1),
                        ("ice", true) => (Terrain::Ice, 1),
                        _ => (Terrain::Clear, 0),
                    };
                    crate::support::set_hex(tile, terrain, elevation);
                }
            }
            world.btech = serde_json::from_value(saved).unwrap();
            let rules = MovementRules {
                ..MovementRules::STANDARD
            };
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let mut collided = false;
            for _ in 0..100 {
                let before = scripts.world().clone();
                scripts.drain_outbox();
                advance_battle_motion_action(&scripts, &config, rules).unwrap();
                let output = scripts.drain_outbox();
                // The collision drops the vehicle, which stops it; its crew is never injured
                // by the fall (Total Warfare p. 152).
                if scripts.world().btech.vehicles()[&id]
                    .motion()
                    .unwrap()
                    .active()
                {
                    continue;
                }
                collided = true;
                // The vehicle fall itself rolls nothing; only the bridge underside's control
                // check reaches the driver, and only the driver.
                let roll_message = |text: &str| {
                    text.starts_with("You make a piloting")
                        || text.starts_with("Modified Pilot Skill:")
                };
                if hazard == "bridge" {
                    let pilot_output: Vec<_> = output
                        .iter()
                        .filter(|(who, _)| *who == ObjectId(2))
                        .map(|(_, text)| text.source())
                        .collect();
                    let mut probe = before.clone();
                    let check =
                        roll_battle_piloting(&mut probe, id, 0, rules.fall.extended_piloting)
                            .unwrap();
                    if check.roll.is_none() {
                        assert!(!pilot_output.iter().any(|text| roll_message(text)));
                    } else {
                        let roll = pilot_output
                            .iter()
                            .position(|text| *text == "You make a piloting skill roll!")
                            .unwrap_or_else(|| panic!("{hazard}/{initial}: {pilot_output:?}"));
                        assert!(roll > 0);
                        assert_eq!(
                            pilot_output[roll - 1],
                            "You notice the underside of the bridge in front of you!"
                        );
                        assert!(pilot_output[roll + 1].starts_with("Modified Pilot Skill: BTH "));
                    }
                    assert!(
                        !output
                            .iter()
                            .any(|(who, text)| *who != ObjectId(2) && roll_message(text.source()))
                    );
                } else {
                    assert!(!output.iter().any(|(_, text)| roll_message(text.source())));
                }
                let result = scripts.world().clone();
                assert_eq!(result.btech.vehicles()[&id].pilot_injuries(), initial);
                assert_eq!(
                    result.btech.vehicles()[&id]
                        .character_pilot_status()
                        .map_or(0, |status| status.injuries),
                    u16::from(initial)
                );
                assert!(!result.btech.vehicles()[&id].motion().unwrap().active());
                assert_eq!(result.objects[&ObjectId(2)].location, Some(id));
                let mut raw = before.clone();
                assert!(advance_battle_motion(&mut raw, rules).is_err());
                assert_eq!(raw.btech, before.btech);
                *scripts.world_mut() = before;
                advance_battle_motion_action(&scripts, &config, rules).unwrap();
                assert_eq!(scripts.world().btech, result.btech);
                persistence::save(&config.database(), &result)
                    .await
                    .unwrap();
                assert_eq!(
                    persistence::load(&config.database()).await.unwrap().btech,
                    result.btech
                );
                break;
            }
            assert!(collided, "{hazard}/{initial} never collided");
        }
    }
}

#[tokio::test]
async fn character_vehicles_stop_at_deep_water_without_a_roll() {
    use std::{cell::RefCell, rc::Rc};
    for movement in [VehicleMovement::Tracked, VehicleMovement::Wheeled] {
        let (_dir, config, mut world, id, map) = fixture_movement(movement).await;
        world
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        world
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        world
            .objects
            .get_mut(&ObjectId(2))
            .unwrap()
            .flags
            .remove(Flag::Wizard);
        world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(id);
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
        assign_battle_pilot(&mut world, id, ObjectId(2)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        let unit = &mut saved["vehicles"][id.0.to_string()];
        unit["motion"]["heading"] = 90.0.into();
        unit["motion"]["desired_heading"] = 90.0.into();
        unit["motion"]["speed"] = 86.0.into();
        unit["motion"]["desired_speed"] = 86.0.into();
        let dice = unit["dice"].clone();
        for row in 0..3 {
            for x in 3..20 {
                let tile = &mut saved["maps"][map.0.to_string()]["terrain"][row * 20 + x];
                crate::support::set_hex(tile, Terrain::Water, 1);
            }
        }
        world.btech = serde_json::from_value(saved).unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let rules = MovementRules::STANDARD;
        let mut encountered = false;
        for _ in 0..100 {
            let before = scripts.world().clone();
            scripts.drain_outbox();
            advance_battle_motion_action(&scripts, &config, rules).unwrap();
            let output = scripts.drain_outbox();
            let result = scripts.world().clone();
            let unit = &result.btech.vehicles()[&id];
            if unit.motion().unwrap().active() {
                continue;
            }
            encountered = true;
            assert!(output.iter().any(|(who, text)| {
                *who == ObjectId(2) && text.source() == "Your vehicle cannot cross water that deep."
            }));
            assert!(!output.iter().any(|(_, text)| {
                text.source().starts_with("You make a piloting")
                    || text.source().starts_with("Modified Pilot Skill:")
            }));
            assert!(!unit.flooded());
            assert_eq!(unit.position().unwrap().x, 2);
            assert_eq!(unit.sections(), before.btech.vehicles()[&id].sections());
            assert_eq!(serde_json::to_value(unit).unwrap()["dice"], dice);
            assert_eq!(
                unit.character_pilot_status(),
                before.btech.vehicles()[&id].character_pilot_status()
            );
            assert_eq!(result.btech.characters(), before.btech.characters());
            for pilot in [ObjectId(1), ObjectId(2)] {
                assert_eq!(result.objects[&pilot].location, Some(id));
            }
            *scripts.world_mut() = before;
            advance_battle_motion_action(&scripts, &config, rules).unwrap();
            assert_eq!(scripts.world().btech, result.btech);
            persistence::save(&config.database(), &result)
                .await
                .unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                result.btech
            );
            break;
        }
        assert!(encountered, "{movement:?}");
    }
}

/// A hovercraft that changes facing at flank speed risks a sideslip on any ground, not only
/// on pavement (Total Warfare pp. 62, 67): an unmodified driving roll, and on a failure a drift
/// along its old course for as many hexes as it missed by; at cruising speed there is no roll.
#[tokio::test]
async fn hovercraft_sideslips_after_a_flank_speed_turn_on_clear_ground() {
    for (roll, flank) in [(2, true), (12, true), (12, false)] {
        let (_dir, config, mut world, id, _map) = fixture_movement(VehicleMovement::Hover).await;
        let maximum = world.btech.vehicles()[&id].maximum_speed();
        let speed = if flank { maximum } else { maximum * 2.0 / 3.0 };
        let seed = (0..=255)
            .find(|seed| Dice::seeded([*seed; 32]).two_d6() == roll)
            .unwrap();
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        let unit = &mut saved["vehicles"][id.0.to_string()];
        unit["motion"]["heading"] = 90.0.into();
        unit["motion"]["desired_heading"] = 150.0.into();
        unit["motion"]["speed"] = speed.into();
        unit["motion"]["desired_speed"] = speed.into();
        unit["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
        world.btech = serde_json::from_value(saved).unwrap();
        let start = world.btech.vehicles()[&id].position().unwrap();
        let mut checked = false;
        for _ in 0..40 {
            let before = world.clone();
            let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
            if !notices.iter().any(|n| n.text.contains("hold your line")) {
                continue;
            }
            checked = true;
            let unit = &world.btech.vehicles()[&id];
            assert_ne!(unit.motion().unwrap().heading, 90.0);
            if roll == 12 {
                assert!(notices.iter().any(|n| n.text == "You hold your course."));
                assert!(unit.motion().unwrap().active());
                assert_eq!(unit.sections(), before.btech.vehicles()[&id].sections());
            } else {
                assert!(
                    notices
                        .iter()
                        .any(|n| n.text == "You lose your line and sideslip!")
                );
                assert!(notices.iter().any(|n| n.text.starts_with("You drift into")));
                // The drift carries it on along its old eastward course, undamaged.
                assert!(unit.position().unwrap().x > start.x);
                assert_eq!(unit.position().unwrap().y, start.y);
                assert_eq!(unit.motion().unwrap().speed, 0.0);
                assert_eq!(unit.sections(), before.btech.vehicles()[&id].sections());
                assert!(!unit.immobilized());
            }
            persistence::save(&config.database(), &before)
                .await
                .unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(
                notices,
                advance_battle_motion(&mut restored, MovementRules::STANDARD).unwrap()
            );
            assert_eq!(world.btech, restored.btech);
            break;
        }
        assert_eq!(checked, flank, "roll {roll} flank {flank}");
        world.validate(&config).unwrap();
    }
}

/// A wheeled vehicle that skids on pavement charges any unit in its path (Total Warfare
/// p. 63, with the +3 unintentional-charge modifier) and is left immobile in terrain its
/// drive could never enter.
#[tokio::test]
async fn a_skidding_vehicle_charges_units_and_is_stuck_by_prohibited_terrain() {
    for obstacle in ["unit", "rough"] {
        let (_dir, config, mut world, id, map) = fixture_movement(VehicleMovement::Wheeled).await;
        // Parked Mechs fill the column the skid slides into, whichever row it takes.
        let mut mechs = Vec::new();
        for y in 0..3 {
            let mech = world.create(&config, format!("Parked Mech {y}"), Kind::Thing);
            world.objects.get_mut(&mech).unwrap().home = Some(ObjectId(config.home()));
            create_battle_unit(
                &mut world,
                mech,
                MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml"))
                    .unwrap(),
            )
            .unwrap();
            support::seed_object_dice(&mut world, mech, support::FIXTURE_DICE_SEED);
            place_battle_unit(
                &mut world,
                mech,
                map,
                if obstacle == "unit" { 3 } else { 15 },
                y,
            )
            .unwrap();
            mechs.push(mech);
        }
        let maximum = world.btech.vehicles()[&id].maximum_speed();
        // The first roll is the skid check: a two always fails it.
        let seed = (0..=255)
            .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 2)
            .unwrap();
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        let unit = &mut saved["vehicles"][id.0.to_string()];
        unit["motion"]["heading"] = 90.0.into();
        unit["motion"]["desired_heading"] = 150.0.into();
        unit["motion"]["speed"] = maximum.into();
        unit["motion"]["desired_speed"] = maximum.into();
        unit["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
        for row in 0..3 {
            for x in 0..20 {
                let terrain = if x < 3 {
                    Terrain::Pavement
                } else if obstacle == "rough" {
                    Terrain::Rough
                } else {
                    Terrain::Clear
                };
                crate::support::set_hex(
                    &mut saved["maps"][map.0.to_string()]["terrain"][row * 20 + x],
                    terrain,
                    0,
                );
            }
        }
        world.btech = serde_json::from_value(saved).unwrap();
        let mut skidded = false;
        for _ in 0..40 {
            let before = world.clone();
            let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
            if !notices
                .iter()
                .any(|n| n.text == "You lose control and skid!")
            {
                continue;
            }
            skidded = true;
            let unit = &world.btech.vehicles()[&id];
            assert_eq!(unit.motion().unwrap().speed, 0.0);
            if obstacle == "rough" {
                assert!(
                    notices
                        .iter()
                        .any(|n| n.text == "You crash into the rough!")
                );
                assert!(notices.iter().any(|n| n.text.contains("stuck fast")));
                assert!(unit.immobilized());
                assert_eq!(unit.maximum_speed(), 0.0);
                assert_eq!(unit.position().unwrap().x, 3);
            } else {
                let charge = notices
                    .iter()
                    .find(|n| n.unit == id && n.text.starts_with("Charge: BTH "))
                    .unwrap_or_else(|| panic!("the skid should charge the Mech: {notices:?}"));
                let (bth, roll) = charge.text["Charge: BTH ".len()..]
                    .split_once("\tRoll: ")
                    .unwrap();
                let hit = roll.parse::<i32>().unwrap() >= bth.parse::<i32>().unwrap();
                let struck = mechs
                    .iter()
                    .filter(|mech| {
                        world.btech.constructed_units()[mech].sections()
                            != before.btech.constructed_units()[mech].sections()
                    })
                    .count();
                if hit {
                    assert!(
                        notices
                            .iter()
                            .any(|n| n.text == "SMASH!!! You crash into your target!")
                    );
                    assert_eq!(struck, 1);
                    assert_ne!(unit.sections(), before.btech.vehicles()[&id].sections());
                    assert_eq!(unit.position().unwrap().x, 2);
                } else {
                    assert!(
                        notices
                            .iter()
                            .any(|n| n.text == "You slide past your target!")
                    );
                    assert_eq!(struck, 0);
                    assert!(unit.position().unwrap().x >= 3);
                }
            }
            persistence::save(&config.database(), &before)
                .await
                .unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(
                notices,
                advance_battle_motion(&mut restored, MovementRules::STANDARD).unwrap()
            );
            assert_eq!(world.btech, restored.btech);
            break;
        }
        assert!(skidded, "{obstacle}");
        world.validate(&config).unwrap();
    }
}
