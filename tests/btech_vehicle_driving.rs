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
        include_str!("../game/mechs/Flatbed_Truck.toml"),
    )
    .unwrap();
    template.movement = movement;
    if movement == VehicleMovement::Hover {
        template.max_speed = 64.5;
    }
    create_battle_vehicle(&mut world, id, template).unwrap();
    place_battle_unit(&mut world, id, map, 2, 1).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
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

/// Authored building, wall and high-water tiles use ordinary ground elevation rules.
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

/// A saved high-speed straight segment crosses several burning cells in one movement update.
async fn burning_corridor() -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world, id, map) = fast_corridor(10000).await;
    let point = world.btech.vehicles()[&id].motion().unwrap().point;
    for x in 0..20 {
        for y in 0..3 {
            let hex = HexCoordinate { x, y };
            if hex != point.containing_hex().unwrap() {
                set_map_decoration(
                    &mut world,
                    map,
                    hex,
                    Some(Decoration::new(DecorationKind::Fire, 120, Some(30))),
                )
                .unwrap();
            }
        }
    }
    (dir, config, world, id, map)
}

/// Put a prescribed fire/motive pair first in the driver's private stream.
fn terrain_seed(world: &mut World, id: ObjectId, fire: u8, motive: u8) -> Dice {
    let seed = (0u32..100000)
        .find_map(|number| {
            let mut seed = [0; 32];
            seed[..4].copy_from_slice(&number.to_le_bytes());
            let mut dice = Dice::seeded(seed);
            (dice.two_d6() == fire && dice.two_d6() == motive).then_some(seed)
        })
        .unwrap();
    world.btech.set_unit_dice(id, Dice::seeded(seed)).unwrap();
    Dice::seeded(seed)
}

#[tokio::test]
async fn terrain_fire_is_configured_and_stops_at_the_first_disabling_crossing() {
    let (_dir, config, mut base, id, _map) = burning_corridor().await;
    let mut dice = terrain_seed(&mut base, id, 6, 10); // Wheeled: fire 8, motive 12.
    let original = base.btech.vehicles()[&id].motion().unwrap();
    let mut safe = base.clone();
    advance_battle_motion(&mut safe, MovementRules::STANDARD).unwrap();
    assert!(safe.btech.vehicles()[&id].motion().unwrap().point.x > original.point.x + 1.0);
    assert_eq!(
        roll_unit_dice(&mut safe, id, 1).unwrap(),
        [dice.clone().d6()]
    );
    let rules = MovementRules {
        fall: FallRules {
            vehicle_impact: VehicleImpactRules {
                advanced_fire: true,
                ..VehicleImpactRules::STANDARD
            },
            ..MovementRules::STANDARD.fall
        },
        ..MovementRules::STANDARD
    };
    persistence::save(&config.database(), &base).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    let notices = advance_battle_motion(&mut base, rules).unwrap();
    assert_eq!(
        notices,
        advance_battle_motion(&mut restored, rules).unwrap()
    );
    assert_eq!(base.btech, restored.btech);
    let unit = &base.btech.vehicles()[&id];
    assert!(unit.immobilized());
    assert_eq!(unit.motion().unwrap().speed, 0.0);
    assert_ne!(
        unit.position().unwrap(),
        safe.btech.vehicles()[&id].position().unwrap()
    );
    assert!(unit.motion().unwrap().point.x > original.point.x);
    assert_eq!(unit.motion().unwrap().point.y, original.point.y);
    assert!(
        notices
            .iter()
            .any(|notice| notice.text.contains("motive system is destroyed"))
    );
    dice.two_d6();
    dice.two_d6();
    assert_eq!(roll_unit_dice(&mut base, id, 1).unwrap(), [dice.d6()]);
    base.validate(&config).unwrap();
}

#[tokio::test]
async fn fire_exposure_requires_entry_and_hull_loss_preserves_occupants() {
    let (_dir, config, mut world, id, map) = burning_corridor().await;
    let rules = MovementRules {
        fall: FallRules {
            vehicle_impact: VehicleImpactRules {
                advanced_fire: true,
                ..VehicleImpactRules::STANDARD
            },
            ..MovementRules::STANDARD.fall
        },
        ..MovementRules::STANDARD
    };
    let mut quiet = world.clone();
    let mut saved = serde_json::to_value(&quiet.btech).unwrap();
    saved["vehicles"][id.0.to_string()]["motion"]["speed"] = 1.0.into();
    saved["vehicles"][id.0.to_string()]["motion"]["desired_speed"] = 1.0.into();
    saved["maps"][map.0.to_string()]["movement_modifier"] = 100.into();
    quiet.btech = serde_json::from_value(saved).unwrap();
    let hex = quiet.btech.vehicles()[&id]
        .motion()
        .unwrap()
        .point
        .containing_hex()
        .unwrap();
    set_map_decoration(
        &mut quiet,
        map,
        hex,
        Some(Decoration::new(DecorationKind::Fire, 120, Some(30))),
    )
    .unwrap();
    let before = serde_json::to_value(&quiet.btech.vehicles()[&id]).unwrap();
    assert!(advance_battle_motion(&mut quiet, rules).unwrap().is_empty());
    assert_eq!(
        serde_json::to_value(&quiet.btech.vehicles()[&id]).unwrap()["dice"],
        before["dice"]
    );
    assert_eq!(
        quiet.btech.vehicles()[&id].position().unwrap().x,
        hex.x as u16
    );
    terrain_seed(&mut world, id, 8, 7); // Wheeled fire 10, sweep on fragile truck.
    world
        .btech
        .rewrite_unit_record(id, |record| {
            for section in record["sections"].as_object_mut().unwrap().values_mut() {
                section["armor"] = 0.into();
            }
        })
        .unwrap();
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let occupants: Vec<_> = world
        .objects
        .iter()
        .filter(|(_, object)| object.location == Some(id))
        .map(|(&id, _)| id)
        .collect();
    advance_battle_motion(&mut world, rules).unwrap();
    assert!(world.btech.vehicles()[&id].is_destroyed());
    assert!(!world.btech.vehicles()[&id].crew_killed());
    for occupant in occupants {
        assert_eq!(world.objects[&occupant].location, Some(id));
    }
    world.validate(&config).unwrap();
    quiet.validate(&config).unwrap();
}

#[tokio::test]
async fn every_crossed_fire_hex_checks_once_without_adding_stationary_exposure() {
    let (_dir, config, mut world, id, map) = burning_corridor().await;
    world
        .btech
        .rewrite_map_record(map, |record| {
            record["movement_modifier"] = 2000.into();
        })
        .unwrap();
    let start = world.btech.vehicles()[&id].motion().unwrap().point;
    let mut ordinary = world.clone();
    advance_battle_motion(&mut ordinary, MovementRules::STANDARD).unwrap();
    let end = ordinary.btech.vehicles()[&id].motion().unwrap().point;
    let crossings = start.trace(end).unwrap().len() - 1;
    assert!(crossings >= 2);
    let seed = (0u32..100000)
        .find_map(|number| {
            let mut seed = [0; 32];
            seed[..4].copy_from_slice(&number.to_le_bytes());
            let mut dice = Dice::seeded(seed);
            (0..crossings).all(|_| dice.two_d6() <= 5).then_some(seed)
        })
        .unwrap();
    world.btech.set_unit_dice(id, Dice::seeded(seed)).unwrap();
    let rules = MovementRules {
        fall: FallRules {
            vehicle_impact: VehicleImpactRules {
                advanced_fire: true,
                ..VehicleImpactRules::STANDARD
            },
            ..MovementRules::STANDARD.fall
        },
        ..MovementRules::STANDARD
    };
    let notices = advance_battle_motion(&mut world, rules).unwrap();
    assert!(notices.is_empty());
    assert_eq!(
        world.btech.vehicles()[&id].motion(),
        ordinary.btech.vehicles()[&id].motion()
    );
    assert_eq!(
        world.btech.vehicles()[&id].sections(),
        ordinary.btech.vehicles()[&id].sections()
    );
    let mut dice = Dice::seeded(seed);
    for _ in 0..crossings {
        dice.two_d6();
    }
    assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
    world.validate(&config).unwrap();
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
    let mut rules = MovementRules::STANDARD;
    rules.fall.vehicle_impact.criticals.enabled = false;
    rules.fall.vehicle_impact.hit.critical_mode = 0;
    rules
}

/// Place a scripted trigger before each disposable mine, so field removal retains its queued callback.
fn seed_corridor_mines(
    world: &mut World,
    id: ObjectId,
    map: ObjectId,
    entries: &[HexCoordinate],
) -> Dice {
    let seed = (0u32..100000)
        .find_map(|number| {
            let mut bytes = [0; 32];
            bytes[..4].copy_from_slice(&number.to_le_bytes());
            let initial = Dice::seeded(bytes);
            let mut dice = initial.clone();
            entries
                .iter()
                .all(|_| {
                    dice.two_d6();
                    dice.two_d6();
                    dice.two_d6() <= 8
                })
                .then_some(initial)
        })
        .unwrap();
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
    for _ in &entries {
        dice.two_d6();
        dice.two_d6();
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
async fn disabling_mine_heat_precedes_terrain_fire_and_stops_at_entry() {
    let (_dir, config, mut world, id, map) = fast_corridor(2000).await;
    let entries = corridor_entries(&world, id);
    let first = entries[0];
    set_minefield(
        &mut world,
        map,
        0,
        Some(Minefield {
            coordinate: first,
            kind: MineKind::Inferno,
            strength: 2,
            extra: 0,
            owner: ObjectId(1),
        }),
    )
    .unwrap();
    set_map_decoration(
        &mut world,
        map,
        first,
        Some(Decoration::new(DecorationKind::Fire, 120, Some(30))),
    )
    .unwrap();
    let mut dice = (0u32..100000)
        .find_map(|number| {
            let mut bytes = [0; 32];
            bytes[..4].copy_from_slice(&number.to_le_bytes());
            let initial = Dice::seeded(bytes);
            let mut dice = initial.clone();
            (dice.two_d6() == 6 && dice.two_d6() == 10 && dice.two_d6() <= 5).then_some(initial)
        })
        .unwrap();
    world.btech.set_unit_dice(id, dice.clone()).unwrap();
    let before = world.clone();
    let mut rules = mine_movement_rules();
    rules.fall.vehicle_impact.advanced_fire = true;
    let notices = advance_battle_motion(&mut world, rules).unwrap();
    let unit = &world.btech.vehicles()[&id];
    assert!(unit.immobilized());
    assert!(!unit.is_destroyed());
    assert_eq!(
        unit.motion().unwrap().point.containing_hex().unwrap(),
        first
    );
    assert_eq!(unit.motion().unwrap().speed, 0.0);
    assert!(
        notices
            .iter()
            .any(|notice| notice.unit == id && notice.text.contains("you trigger a mine"))
    );
    assert!(world.btech.maps()[&map].minefields().is_empty());
    assert!(unit.burning_sections().is_empty());
    dice.two_d6();
    dice.two_d6();
    dice.two_d6();
    assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
    // Unsupported character damage must restore entry position, field removal and every draw.
    let mut character = before;
    character
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let checkpoint = character.btech.clone();
    assert!(advance_battle_motion(&mut character, rules).is_err());
    assert_eq!(character.btech, checkpoint);
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn mine_blast_disables_a_later_vehicle_before_its_scheduled_movement() {
    let (_dir, config, mut world, id, map) = fast_corridor(2000).await;
    let entries = corridor_entries(&world, id);
    let first = entries[0];
    let neighbor = first
        .neighbors()
        .unwrap()
        .into_iter()
        .find(|hex| hex.y != first.y && hex.x >= 0 && hex.y >= 0 && hex.y < 3 && hex.x < 20)
        .unwrap();
    let other = world.create(&config, "Second truck".into(), Kind::Thing);
    world.objects.get_mut(&other).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        other,
        VehicleTemplate::parse(
            "Flatbed_Truck",
            include_str!("../game/mechs/Flatbed_Truck.toml"),
        )
        .unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, other, support::FIXTURE_DICE_SEED);
    place_battle_unit(
        &mut world,
        other,
        map,
        i64::from(neighbor.x),
        i64::from(neighbor.y),
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
            kind: MineKind::Vibra,
            strength: 8,
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

/// A map edge farther down the traced route cannot suppress a mine crossed before it.
#[tokio::test]
async fn later_boundary_does_not_bypass_an_earlier_mine() {
    let (_dir, config, mut world, id, map) = fast_corridor(50000).await;
    let origin = world.btech.vehicles()[&id].motion().unwrap().point;
    let first = origin.trace(origin.project(90.0, 2.0).unwrap()).unwrap()[1];
    set_minefield(
        &mut world,
        map,
        0,
        Some(Minefield {
            coordinate: first,
            kind: MineKind::Inferno,
            strength: 2,
            extra: 0,
            owner: ObjectId(1),
        }),
    )
    .unwrap();
    let mut dice = terrain_seed(&mut world, id, 6, 10);
    let mut rules = mine_movement_rules();
    rules.fall.vehicle_impact.advanced_fire = true;
    let notices = advance_battle_motion(&mut world, rules).unwrap();
    assert!(world.btech.vehicles()[&id].immobilized());
    assert_eq!(
        world.btech.vehicles()[&id]
            .motion()
            .unwrap()
            .point
            .containing_hex()
            .unwrap(),
        first
    );
    assert!(
        !notices
            .iter()
            .any(|notice| notice.text.contains("Map edge"))
    );
    assert!(world.btech.maps()[&map].minefields().is_empty());
    dice.two_d6();
    dice.two_d6();
    assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
    world.validate(&config).unwrap();
}

/// Failed mine deletion rolls back the vehicle's movement and damage before the heartbeat retries.
#[tokio::test]
async fn vehicle_mine_movement_retries_a_failed_server_save() {
    tokio::task::LocalSet::new().run_until(async {
        use sqlx::Connection;
        let (_dir, config, mut world, id, map) = fast_corridor(2000).await;
        let first = corridor_entries(&world, id)[0];
        set_minefield(&mut world, map, 0, Some(Minefield {
            coordinate: first, kind: MineKind::Vibra, strength: 8, extra: 0, owner: ObjectId(1),
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

/// Fatal mine heat does not skip the entered terrain's independent fire check.
#[tokio::test]
async fn fatal_mine_heat_still_checks_terrain_fire_before_stopping() {
    let (_dir, config, mut world, id, map) = burning_corridor().await;
    reinforce_truck(&mut world, id);
    let first = corridor_entries(&world, id)[0];
    set_minefield(
        &mut world,
        map,
        0,
        Some(Minefield {
            coordinate: first,
            kind: MineKind::Inferno,
            strength: 2,
            extra: 0,
            owner: ObjectId(1),
        }),
    )
    .unwrap();
    let (initial, mut expected) = (0u32..100000)
        .find_map(|number| {
            let mut bytes = [0; 32];
            bytes[..4].copy_from_slice(&number.to_le_bytes());
            let initial = Dice::seeded(bytes);
            let mut dice = initial.clone();
            if dice.two_d6() != 8 {
                return None;
            }
            dice.d6();
            dice.two_d6();
            if dice.two_d6() > 7 {
                return None;
            }
            for _ in 0..3 {
                dice.d6();
                dice.two_d6();
            }
            for _ in 0..4 {
                dice.d6();
            }
            if dice.two_d6() > 5 {
                return None;
            }
            Some((initial, dice))
        })
        .unwrap();
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["dice"] = serde_json::to_value(initial).unwrap();
            record["sections"]["left"]["armor"] = 0.into();
            record["sections"]["left"]["internal"] = 1.into();
        })
        .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    let mut rules = mine_movement_rules();
    rules.fall.vehicle_impact.advanced_fire = true;
    assert_eq!(
        advance_battle_motion(&mut world, rules).unwrap(),
        advance_battle_motion(&mut restored, rules).unwrap()
    );
    assert_eq!(world.btech, restored.btech);
    let unit = &world.btech.vehicles()[&id];
    assert!(unit.is_destroyed());
    assert_eq!(
        unit.motion().unwrap().point.containing_hex().unwrap(),
        first
    );
    assert_eq!(unit.motion().unwrap().speed, 0.0);
    assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [expected.d6()]);
    world.validate(&config).unwrap();
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
                    include_str!("../game/mechs/Flatbed_Truck.toml"),
                )
                .unwrap(),
            )
            .unwrap();
            support::seed_object_dice(&mut world, other, support::FIXTURE_DICE_SEED);
        } else {
            create_battle_unit(
                &mut world,
                other,
                MechTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml"))
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
async fn vehicle_water_entry_replays_avoidance_flooding_and_exemptions() {
    for (movement, success, waterproof, depth, piloted) in [
        (VehicleMovement::Wheeled, true, false, 1, true),
        (VehicleMovement::Wheeled, false, false, 1, true),
        (VehicleMovement::Tracked, true, false, 1, true),
        (VehicleMovement::Tracked, false, false, 1, true),
        (VehicleMovement::Tracked, true, true, 1, true),
        (VehicleMovement::Wheeled, true, false, 0, true),
        (VehicleMovement::Wheeled, true, false, 1, false),
    ] {
        let (_dir, config, mut world, id, map) = fixture_movement(movement).await;
        set_battle_heading(&mut world, id, ObjectId(1), 90.0).unwrap();
        for _ in 0..10 {
            advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
        }
        set_battle_speed(&mut world, id, ObjectId(1), 86.0).unwrap();
        let seed = (0..=255)
            .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 12)
            .unwrap();
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        let unit = &mut saved["vehicles"][id.0.to_string()];
        unit["piloting_damage"] = if success { 0 } else { 100 }.into();
        unit["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
        if !piloted {
            unit["pilot"] = serde_json::Value::Null;
        }
        if waterproof {
            unit["definition"]["attributes"]["specials"] = "Waterproof_Tech".into();
        }
        for row in 0..3 {
            for x in 3..20 {
                let tile = &mut saved["maps"][map.0.to_string()]["terrain"][row * 20 + x];
                crate::support::set_hex(tile, Terrain::Water, depth);
            }
        }
        saved["maps"][map.0.to_string()]["decorations"]["23"] =
            serde_json::json!({"kind":"smoke", "remaining":120, "object_duration":120, "order":-1});
        world.btech = serde_json::from_value(saved).unwrap();
        let mut encountered = false;
        for _ in 0..100 {
            let before = world.clone();
            let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
            let unit = &world.btech.vehicles()[&id];
            let checked = notices.iter().any(|n| n.text.contains("body of water"));
            if !checked && unit.position().unwrap().x < 3 {
                continue;
            }
            encountered = true;
            assert_eq!(checked, depth > 0 && !waterproof);
            assert_eq!(unit.flooded(), checked && !success);
            assert_eq!(unit.sections(), before.btech.vehicles()[&id].sections());
            assert_eq!(unit.ammunition(), before.btech.vehicles()[&id].ammunition());
            assert_eq!(unit.pilot_injuries(), 0);
            if checked {
                assert!(!unit.motion().unwrap().active());
                assert_eq!(unit.position().unwrap().x, if success { 2 } else { 3 });
                if success {
                    assert_eq!(
                        unit.motion().unwrap().point,
                        before.btech.vehicles()[&id].motion().unwrap().point
                    );
                }
            } else {
                assert!(unit.motion().unwrap().active());
            }
            let mut dice = Dice::seeded([seed; 32]);
            if checked && piloted {
                dice.two_d6();
            }
            assert_eq!(
                serde_json::to_value(unit).unwrap()["dice"],
                serde_json::to_value(dice).unwrap()
            );
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
        assert!(encountered);
    }
}

#[tokio::test]
async fn vehicle_obstacles_share_checks_falls_configuration_and_replay() {
    for (movement, terrain, enabled, success, checked) in [
        (
            VehicleMovement::Tracked,
            Terrain::HeavyForest,
            true,
            false,
            true,
        ),
        (
            VehicleMovement::Tracked,
            Terrain::HeavyForest,
            true,
            true,
            true,
        ),
        (
            VehicleMovement::Tracked,
            Terrain::HeavyForest,
            false,
            true,
            false,
        ),
        (
            VehicleMovement::Tracked,
            Terrain::LightForest,
            true,
            true,
            false,
        ),
        (
            VehicleMovement::Wheeled,
            Terrain::LightForest,
            true,
            false,
            true,
        ),
        (
            VehicleMovement::Wheeled,
            Terrain::HeavyForest,
            true,
            true,
            true,
        ),
        (VehicleMovement::Wheeled, Terrain::Rough, true, false, true),
        (VehicleMovement::Wheeled, Terrain::Rough, false, true, false),
        (
            VehicleMovement::Hover,
            Terrain::HeavyForest,
            false,
            false,
            true,
        ),
        (
            VehicleMovement::Hover,
            Terrain::LightForest,
            false,
            true,
            true,
        ),
    ] {
        let (_dir, config, mut world, id, map) = fixture_movement(movement).await;
        let maximum = world.btech.vehicles()[&id].maximum_speed();
        let seed = (0..=255)
            .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 12)
            .unwrap();
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        let unit = &mut saved["vehicles"][id.0.to_string()];
        unit["motion"]["heading"] = 90.0.into();
        unit["motion"]["desired_heading"] = 90.0.into();
        unit["motion"]["speed"] = maximum.into();
        unit["motion"]["desired_speed"] = maximum.into();
        unit["piloting_damage"] = if success { 0 } else { 100 }.into();
        unit["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
        for row in 0..3 {
            for x in 3..20 {
                let tile = &mut saved["maps"][map.0.to_string()]["terrain"][row * 20 + x];
                crate::support::set_hex(tile, terrain, 1);
            }
        }
        saved["maps"][map.0.to_string()]["decorations"]["23"] =
            serde_json::json!({"kind":"smoke", "remaining":120, "object_duration":120, "order":-1});
        world.btech = serde_json::from_value(saved).unwrap();
        let rules = MovementRules {
            new_terrain: enabled,
            ..MovementRules::STANDARD
        };
        let mut entered = false;
        for _ in 0..100 {
            let before = world.clone();
            let notices = advance_battle_motion(&mut world, rules).unwrap();
            let unit = &world.btech.vehicles()[&id];
            if unit.position().unwrap().x < 3 {
                continue;
            }
            entered = true;
            assert_eq!(
                notices.iter().any(|n| n.text.contains("try to dodge")
                    || n.text.contains("try to avoid the rocks")),
                checked
            );
            assert_eq!(unit.motion().unwrap().active(), success);
            assert_eq!(unit.position().unwrap().x, 3);
            assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(1));
            let mut expected = before.clone();
            if checked {
                let _check =
                    roll_battle_piloting(&mut expected, id, 0, rules.fall.extended_piloting)
                        .unwrap();
            }
            if success {
                assert_eq!(unit.sections(), before.btech.vehicles()[&id].sections());
                assert_eq!(unit.motion().unwrap().speed, maximum - 21.5);
            } else {
                assert_ne!(unit.sections(), before.btech.vehicles()[&id].sections());
                let levels = (maximum / 10.75 / 2.0).sqrt().max(1.0) as u8;
                let _fall =
                    resolve_battle_vehicle_fall(&mut expected, id, levels, rules.fall).unwrap();
                let mut restricted = before.clone();
                restricted
                    .objects
                    .get_mut(&id)
                    .unwrap()
                    .flags
                    .insert(Flag::InCharacter);
                let snapshot = restricted.btech.clone();
                assert!(advance_battle_motion(&mut restricted, rules).is_err());
                assert_eq!(snapshot, restricted.btech);
            }
            assert_eq!(
                serde_json::to_value(unit).unwrap()["dice"],
                serde_json::to_value(&expected.btech.vehicles()[&id]).unwrap()["dice"]
            );
            persistence::save(&config.database(), &before)
                .await
                .unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(
                notices,
                advance_battle_motion(&mut restored, rules).unwrap()
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
        let seed = (0..=255)
            .find(|seed| (Dice::seeded([*seed; 32]).d6() == 1) == fracture)
            .unwrap();
        let neighbor = world.create(&config, "Waterproof neighbor".into(), Kind::Thing);
        world.objects.get_mut(&neighbor).unwrap().home = Some(ObjectId(config.home()));
        let mut template = VehicleTemplate::parse(
            "Flatbed_Truck",
            include_str!("../game/mechs/Flatbed_Truck.toml"),
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
            MechTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
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
                let neighbor_notice = notices
                    .iter()
                    .position(|n| {
                        n.unit == neighbor && n.text == "Water renders your vehicle inoperable."
                    })
                    .unwrap();
                let trigger_notice = notices
                    .iter()
                    .position(|n| {
                        n.unit == id && n.text == "You try to avoid taking personal damage."
                    })
                    .unwrap();
                assert!(neighbor_notice < trigger_notice);
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

#[tokio::test]
async fn waterproof_vehicle_stays_below_ice_without_a_surface_fracture_roll() {
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
                Hex::new(if x < 3 { Terrain::Water } else { Terrain::Ice }, 2),
            )
            .unwrap();
        }
    }
    world.btech = serde_json::from_value(saved).unwrap();
    let mut entered = false;
    for _ in 0..100 {
        assert!(
            advance_battle_motion(&mut world, MovementRules::STANDARD)
                .unwrap()
                .is_empty()
        );
        if world.btech.vehicles()[&id].position().unwrap().x < 3 {
            continue;
        }
        entered = true;
        assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(-2));
        assert_eq!(
            serde_json::to_value(&world.btech.vehicles()[&id]).unwrap()["dice"],
            dice
        );
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
        break;
    }
    assert!(entered);
}

#[tokio::test]
async fn terrain_fire_crew_death_evacuates_in_the_movement_checkpoint() {
    use std::{cell::RefCell, rc::Rc};
    let (_dir, config, mut world, id, _map) = burning_corridor().await;
    let seed = (0u32..100_000)
        .find_map(|value| {
            let mut seed = [0; 32];
            seed[..4].copy_from_slice(&value.to_le_bytes());
            let mut dice = Dice::seeded(seed);
            if dice.two_d6() != 8 {
                return None;
            }
            dice.d6();
            dice.two_d6();
            if !matches!(dice.two_d6(), 8 | 9) || dice.die(10).unwrap() <= 5 || dice.d6() != 4 {
                return None;
            }
            Some(seed)
        })
        .unwrap();
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["dice"] = serde_json::to_value(Dice::seeded(seed)).unwrap();
            for section in record["sections"].as_object_mut().unwrap().values_mut() {
                section["armor"] = 0.into();
            }
        })
        .unwrap();
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let mut rules = MovementRules::STANDARD;
    rules.fall.vehicle_impact.advanced_fire = true;
    rules.fall.vehicle_impact.criticals.table = VehicleCriticalTable::Standard;
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(id);
    let before = world.clone();
    assert!(advance_battle_motion(&mut world, rules).is_err());
    assert_eq!(world.btech, before.btech);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let afterlife = ObjectId(config.battletech.afterlife_dbref);
    scripts.world_mut().objects.remove(&afterlife);
    assert!(advance_battle_motion_action(&scripts, &config, rules).is_err());
    assert_eq!(scripts.world().btech, before.btech);
    assert_eq!(scripts.world().objects[&ObjectId(2)].location, Some(id));
    *scripts.world_mut() = before;
    advance_battle_motion_action(&scripts, &config, rules).unwrap();
    let result = scripts.world().clone();
    assert!(result.btech.vehicles()[&id].crew_killed());
    assert_eq!(result.objects[&ObjectId(2)].location, Some(afterlife));
    assert!(!result.btech.vehicles()[&id].motion().unwrap().active());
    persistence::save(&config.database(), &result)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        result.btech
    );
}

#[tokio::test]
async fn character_movement_collisions_publish_falls_and_rollback_casualties() {
    use std::{cell::RefCell, rc::Rc};
    for hazard in ["forest", "cliff", "bridge", "ice", "water_cliff"] {
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
                        ("forest", true) => (Terrain::HeavyForest, 0),
                        ("cliff", true) => (Terrain::Grassland, 3),
                        ("ice", true) => (Terrain::Ice, 1),
                        ("water_cliff", false) => (Terrain::Grassland, 2),
                        ("water_cliff", true) => (Terrain::Water, 1),
                        _ => (Terrain::Grassland, 0),
                    };
                    crate::support::set_hex(tile, terrain, elevation);
                }
            }
            world.btech = serde_json::from_value(saved).unwrap();
            let rules = MovementRules {
                new_terrain: true,
                ..MovementRules::STANDARD
            };
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let afterlife = ObjectId(config.battletech.afterlife_dbref);
            let mut collided = false;
            for _ in 0..100 {
                let before = scripts.world().clone();
                scripts.drain_outbox();
                advance_battle_motion_action(&scripts, &config, rules).unwrap();
                let output = scripts.drain_outbox();
                if !scripts.world().btech.vehicles()[&id]
                    .character_pilot_status()
                    .is_some_and(|status| status.killed || status.injuries > u16::from(initial))
                {
                    continue;
                }
                collided = true;
                if hazard != "ice" {
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
                        assert!(
                            !pilot_output
                                .iter()
                                .any(|text| text.starts_with("You make a piloting")
                                    || text.starts_with("Modified Pilot Skill:"))
                        );
                    } else {
                        let roll = pilot_output
                            .iter()
                            .position(|text| *text == "You make a piloting skill roll!")
                            .unwrap_or_else(|| panic!("{hazard}/{initial}: {pilot_output:?}"));
                        assert!(roll > 0);
                        assert_eq!(
                            pilot_output[roll - 1],
                            match hazard {
                                "forest" => "You try to dodge the larger trees..",
                                "cliff" => "You attempt to climb a hill too steep for you.",
                                "bridge" =>
                                    "You notice the underside of the bridge in front of you!",
                                "water_cliff" => "You notice a large drop in front of you",
                                _ => unreachable!(),
                            }
                        );
                        assert!(pilot_output[roll + 1].starts_with("Modified Pilot Skill: BTH "));
                    }
                    assert!(!output.iter().any(|(who, text)| *who != ObjectId(2)
                        && (text.source().starts_with("You make a piloting")
                            || text.source().starts_with("Modified Pilot Skill:"))));
                }

                let result = scripts.world().clone();
                assert_eq!(
                    u16::from(result.btech.vehicles()[&id].pilot_injuries()),
                    result.btech.vehicles()[&id]
                        .character_pilot_status()
                        .unwrap()
                        .injuries
                );
                assert!(!result.btech.vehicles()[&id].motion().unwrap().active());
                assert_eq!(
                    result.objects[&ObjectId(2)].location,
                    Some(if initial == 9 { afterlife } else { id })
                );
                let mut raw = before.clone();
                assert!(advance_battle_motion(&mut raw, rules).is_err());
                assert_eq!(raw.btech, before.btech);
                if initial == 9 {
                    *scripts.world_mut() = before.clone();
                    scripts.world_mut().objects.remove(&afterlife);
                    assert!(advance_battle_motion_action(&scripts, &config, rules).is_err());
                    assert_eq!(scripts.world().btech, before.btech);
                    assert_eq!(scripts.world().objects[&ObjectId(2)].location, Some(id));
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
            assert!(collided, "{hazard}/{initial} never collided");
        }
    }
}

#[tokio::test]
async fn character_water_entry_preserves_occupants_and_replays_flooding() {
    use std::{cell::RefCell, rc::Rc};
    for movement in [VehicleMovement::Tracked, VehicleMovement::Wheeled] {
        for success in [false, true] {
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
            let seed = (0..=255)
                .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 12)
                .unwrap();
            let mut saved = serde_json::to_value(&world.btech).unwrap();
            let unit = &mut saved["vehicles"][id.0.to_string()];
            unit["motion"]["heading"] = 90.0.into();
            unit["motion"]["desired_heading"] = 90.0.into();
            unit["motion"]["speed"] = 86.0.into();
            unit["motion"]["desired_speed"] = 86.0.into();
            unit["piloting_damage"] = if success { 0 } else { 100 }.into();
            unit["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
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
                let pilot_output: Vec<_> = output
                    .iter()
                    .filter(|(who, _)| *who == ObjectId(2))
                    .map(|(_, text)| text.source())
                    .collect();
                let warning = pilot_output
                    .iter()
                    .position(|text| *text == "You notice a body of water in front of you")
                    .unwrap();
                assert_eq!(pilot_output[warning + 1], "You make a piloting skill roll!");
                assert!(pilot_output[warning + 2].starts_with("Modified Pilot Skill: BTH "));
                assert!(pilot_output[warning + 2].ends_with("\tRoll: 12"));
                assert_eq!(
                    pilot_output[warning + 3],
                    if success {
                        "You manage to stop before falling in."
                    } else {
                        "You drive into the water and your vehicle becomes inoperable."
                    }
                );
                assert!(output.iter().any(|(who, text)| {
                    *who == ObjectId(1)
                        && text.source() == "You notice a body of water in front of you"
                }));
                assert!(!output.iter().any(|(who, text)| {
                    *who != ObjectId(2)
                        && (text.source().starts_with("You make a piloting")
                            || text.source().starts_with("Modified Pilot Skill:"))
                }));
                assert_eq!(unit.flooded(), !success);
                assert_eq!(unit.position().unwrap().x, if success { 2 } else { 3 });
                assert_eq!(unit.sections(), before.btech.vehicles()[&id].sections());
                assert_eq!(
                    unit.character_pilot_status(),
                    before.btech.vehicles()[&id].character_pilot_status()
                );
                assert_eq!(result.btech.characters(), before.btech.characters());
                for pilot in [ObjectId(1), ObjectId(2)] {
                    assert_eq!(result.objects[&pilot].location, Some(id));
                }
                if !success {
                    let mut raw = before.clone();
                    assert!(advance_battle_motion(&mut raw, rules).is_err());
                    assert_eq!(raw.btech, before.btech);
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
            assert!(encountered, "{movement:?}/{success}");
        }
    }
}
