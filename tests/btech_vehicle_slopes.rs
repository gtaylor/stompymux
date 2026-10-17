//! Vehicle slopes share reverse control checks and retain deterministic fall and placement effects.
use crate::support;
use stompymux_rs::*;

/// A vehicle facing east beside a short sequence of one-level rises and descents.
async fn fixture(
    movement: VehicleMovement,
    reverse: bool,
) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Slopes".into(), Kind::Room);
    let row = ".0.0.0.1.2.2.1.0.0.0.0.0";
    create_battle_map(
        &mut world,
        map,
        "slopes",
        MapAsset::from_cells(&format!("12 3\n{row}\n{row}\n{row}\n")).unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    let mut template = VehicleTemplate::parse(
        "Flatbed_Truck",
        include_str!("../game/units/Flatbed_Truck.toml"),
    )
    .unwrap();
    // The flatbed's 6-ton cargo bay is its wheeled payload; the heavier tracked and hover
    // running gear needs that mass.
    template.attributes.remove(CARGO_TONS);
    template.movement = movement;
    template.restate_engine_rating();
    if movement == VehicleMovement::Hover {
        template.set_cruising_mp(4);
    }
    let maximum = template.max_speed;
    create_battle_vehicle(&mut world, id, template).unwrap();
    place_battle_unit(&mut world, id, map, if reverse { 8 } else { 2 }, 1).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0, true);
    }
    set_battle_heading(&mut world, id, ObjectId(1), 90.0).unwrap();
    for _ in 0..10 {
        advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
    }
    set_battle_speed(
        &mut world,
        id,
        ObjectId(1),
        if reverse {
            -maximum * 2.0 / 3.0
        } else {
            maximum
        },
    )
    .unwrap();
    (dir, config, world, id)
}

/// A ground vehicle pays two MP for each level it changes (Total Warfare pp. 48, 52). Under
/// the realtime scale that cost is charged like the hex's terrain cost: every second spent
/// crossing a hex it climbed or descended into is proposed with two extra movement points,
/// and the hexes beyond cost nothing extra.
#[tokio::test]
async fn vehicle_slopes_charge_two_movement_points_per_level_and_replay_mid_climb() {
    for (movement, reverse) in [
        (VehicleMovement::Tracked, false),
        (VehicleMovement::Wheeled, false),
        (VehicleMovement::Tracked, true),
    ] {
        let (_dir, config, mut world, id) = fixture(movement, reverse).await;
        let mut changes = 0;
        for _ in 0..250 {
            let old_height = battle_unit_elevation(&world, id).unwrap().unwrap();
            let unit = &world.btech.vehicles()[&id];
            let proposed = unit
                .definition()
                .ground_motion_step(
                    unit.motion().unwrap(),
                    Hex::new(Terrain::Clear, 0),
                    VehicleMotionRules {
                        extra_cost: unit.level_entry_cost(),
                        ..VehicleMotionRules::STANDARD
                    },
                )
                .unwrap();
            let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
            assert!(notices.is_empty(), "{movement:?} {reverse}: {notices:?}");
            let unit = &world.btech.vehicles()[&id];
            assert!((unit.motion().unwrap().speed - proposed.speed).abs() < 1e-10);
            let height = battle_unit_elevation(&world, id).unwrap().unwrap();
            if height != old_height {
                changes += 1;
                assert_eq!((height - old_height).abs(), 1);
                assert_eq!(unit.level_entry_cost(), 2);
                if changes == 1 {
                    persistence::save(&config.database(), &world).await.unwrap();
                    let restored = persistence::load(&config.database()).await.unwrap();
                    assert_eq!(restored.btech, world.btech);
                    world = restored;
                }
            }
            let x = world.btech.vehicles()[&id].position().unwrap().x;
            if (!reverse && x >= 8) || (reverse && x <= 2) {
                break;
            }
        }
        assert_eq!(changes, 4, "{movement:?} reverse={reverse}");
    }
}

#[tokio::test]
async fn reverse_slope_checks_replay_success_and_failed_climbs_and_descents() {
    for (movement, uphill, success) in [
        (VehicleMovement::Wheeled, true, true),
        (VehicleMovement::Wheeled, true, false),
        (VehicleMovement::Wheeled, false, false),
        (VehicleMovement::Hover, true, true),
        (VehicleMovement::Hover, true, false),
        (VehicleMovement::Hover, false, false),
    ] {
        let (_dir, config, mut world, id) = fixture(movement, true).await;
        let map = world.btech.vehicles()[&id].position().unwrap().map;
        let seed = (0..=255)
            .find(|seed| (Dice::seeded([*seed; 32]).two_d6() >= 6) == success)
            .unwrap();
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        saved["vehicles"][id.0.to_string()]["dice"] =
            serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
        for row in 0..3 {
            for x in 0..12 {
                crate::support::set_hex_elevation(
                    &mut saved["maps"][map.0.to_string()]["terrain"][row * 12 + x],
                    u8::from((x <= 6) == uphill),
                );
            }
        }
        world.btech = serde_json::from_value(saved).unwrap();
        let mut checked = false;
        for _ in 0..100 {
            let before = world.clone();
            let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
            if !notices
                .iter()
                .any(|notice| notice.text.contains("behind you!"))
            {
                continue;
            }
            checked = true;
            assert_eq!(
                notices
                    .iter()
                    .any(|notice| notice.text.contains("overcome the obstacle")),
                success
            );
            let unit = &world.btech.vehicles()[&id];
            if success {
                assert!(unit.motion().unwrap().active());
                assert_eq!(unit.position().unwrap().x, 6);
                let previous = &before.btech.vehicles()[&id];
                let proposed = previous
                    .ground_motion_step(
                        previous.motion().unwrap(),
                        Hex::new(Terrain::Clear, 0),
                        VehicleMotionRules::STANDARD,
                    )
                    .unwrap();
                assert_eq!(unit.motion().unwrap().speed, proposed.speed);
                let mut expected_dice = Dice::seeded([seed; 32]);
                expected_dice.two_d6();
                assert_eq!(
                    serde_json::to_value(unit).unwrap()["dice"],
                    serde_json::to_value(expected_dice).unwrap()
                );
            } else {
                assert!(!unit.motion().unwrap().active());
                assert_eq!(unit.position().unwrap().x, if uphill { 7 } else { 6 });
                // A vehicle fall injures no crew (Total Warfare p. 152).
                assert!(
                    !notices
                        .iter()
                        .any(|notice| notice.text.contains("personal"))
                );
                if uphill {
                    assert_eq!(
                        unit.motion().unwrap().point,
                        before.btech.vehicles()[&id].motion().unwrap().point
                    );
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
            world.validate(&config).unwrap();
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
            break;
        }
        assert!(checked);
    }
}

#[tokio::test]
async fn two_level_vehicle_cliffs_stop_before_entry() {
    let (_dir, _config, mut world, id) = fixture(VehicleMovement::Tracked, false).await;
    let map = world.btech.vehicles()[&id].position().unwrap().map;
    world
        .btech
        .rewrite_map_record(map, |record| {
            for row in 0..3 {
                crate::support::set_hex_elevation(&mut record["terrain"][row * 12 + 3], 2);
            }
        })
        .unwrap();
    let mut stopped = false;
    for _ in 0..100 {
        let old = world.btech.vehicles()[&id].motion().unwrap().point;
        let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
        if notices
            .iter()
            .any(|notice| notice.text.contains("too steep to drive"))
        {
            stopped = true;
            assert_eq!(world.btech.vehicles()[&id].motion().unwrap().point, old);
            break;
        }
    }
    assert!(stopped);
    assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(0));
    assert!(!world.btech.vehicles()[&id].motion().unwrap().active());
}

#[tokio::test]
async fn retained_height_applies_only_to_the_departure_hex() {
    let (_dir, config, mut world, id) = fixture(VehicleMovement::Tracked, false).await;
    let map = world.btech.vehicles()[&id].position().unwrap().map;
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][id.0.to_string()]["ground_elevation"] = 2.into();
    for row in 0..3 {
        for x in 3..12 {
            crate::support::set_hex_elevation(
                &mut saved["maps"][map.0.to_string()]["terrain"][row * 12 + x],
                3,
            );
        }
    }
    world.btech = serde_json::from_value(saved).unwrap();
    let mut entered = false;
    for _ in 0..100 {
        let unit = &world.btech.vehicles()[&id];
        let proposed = unit
            .ground_motion_step(
                unit.motion().unwrap(),
                Hex::new(Terrain::Clear, 0),
                VehicleMotionRules::STANDARD,
            )
            .unwrap();
        assert!(
            advance_battle_motion(&mut world, MovementRules::STANDARD)
                .unwrap()
                .is_empty()
        );
        let unit = &world.btech.vehicles()[&id];
        if unit.position().unwrap().x == 3 {
            entered = true;
            assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(3));
            // The climb from the retained level 2 is one level, paid as two MP while the
            // vehicle crosses the new hex rather than as a cut on entry.
            assert_eq!(unit.motion().unwrap().speed, proposed.speed);
            assert_eq!(unit.level_entry_cost(), 2);
            assert!(serde_json::to_value(unit).unwrap()["ground_elevation"].is_null());
            break;
        }
    }
    assert!(entered);
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn steep_slopes_stop_vehicles_at_the_edge_without_dice() {
    for (movement, downhill, water) in [
        (VehicleMovement::Tracked, false, false),
        (VehicleMovement::Wheeled, false, false),
        (VehicleMovement::Hover, true, false),
        (VehicleMovement::Tracked, true, false),
        (VehicleMovement::Wheeled, true, true),
        (VehicleMovement::Tracked, true, true),
    ] {
        let (_dir, config, mut world, id) = fixture(movement, false).await;
        let map = world.btech.vehicles()[&id].position().unwrap().map;
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        let unit = &mut saved["vehicles"][id.0.to_string()];
        unit["motion"]["speed"] = world.btech.vehicles()[&id].maximum_speed().into();
        let dice = unit["dice"].clone();
        for row in 0..3 {
            for x in 0..12 {
                let destination = x >= 3;
                let tile = &mut saved["maps"][map.0.to_string()]["terrain"][row * 12 + x];
                let elevation = if water {
                    1
                } else if destination == downhill {
                    0
                } else {
                    2
                };
                crate::support::set_hex_elevation(tile, elevation);
                if water && destination {
                    crate::support::set_hex_terrain(tile, Terrain::Water);
                }
            }
        }
        world.btech = serde_json::from_value(saved).unwrap();
        // A change of more than one level is prohibited terrain for a vehicle (Total Warfare
        // p. 48), as is deep water for anything but a hovercraft (p. 52): it stops at the
        // edge with no roll, no fall and no flooding.
        let mut encountered = false;
        for _ in 0..100 {
            let before = world.clone();
            let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
            if !notices.iter().any(|n| {
                n.text.contains("too steep to drive") || n.text.contains("cannot cross water")
            }) {
                continue;
            }
            encountered = true;
            let unit = &world.btech.vehicles()[&id];
            assert!(!unit.motion().unwrap().active());
            assert_eq!(unit.position().unwrap().x, 2);
            assert_eq!(
                unit.motion().unwrap().point,
                before.btech.vehicles()[&id].motion().unwrap().point
            );
            assert!(!unit.flooded());
            assert_eq!(unit.sections(), before.btech.vehicles()[&id].sections());
            assert_eq!(serde_json::to_value(unit).unwrap()["dice"], dice);
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
        assert!(
            encountered,
            "{movement:?} downhill={downhill} water={water}"
        );
    }
}

#[tokio::test]
async fn vehicle_auto_fall_changes_nothing_at_steep_slopes() {
    for (downhill, piloted) in [(true, true), (true, false), (false, true)] {
        let (_dir, config, mut world, id) = fixture(VehicleMovement::Tracked, false).await;
        set_battle_auto_fall(&mut world, id, ObjectId(1), true).unwrap();
        let map = world.btech.vehicles()[&id].position().unwrap().map;
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        let dice = saved["vehicles"][id.0.to_string()]["dice"].clone();
        if !piloted {
            saved["vehicles"][id.0.to_string()]["pilot"] = serde_json::Value::Null;
        }
        for row in 0..3 {
            for x in 0..12 {
                crate::support::set_hex_elevation(
                    &mut saved["maps"][map.0.to_string()]["terrain"][row * 12 + x],
                    if (x >= 3) == downhill { 0 } else { 2 },
                );
            }
        }
        world.btech = serde_json::from_value(saved).unwrap();
        let mut encountered = false;
        for _ in 0..100 {
            let before = world.clone();
            let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
            if !notices
                .iter()
                .any(|n| n.text.contains("too steep to drive"))
            {
                continue;
            }
            encountered = true;
            let unit = &world.btech.vehicles()[&id];
            assert_eq!(unit.position().unwrap().x, 2);
            assert!(!unit.motion().unwrap().active());
            assert!(unit.auto_fall());
            assert_eq!(serde_json::to_value(unit).unwrap()["dice"], dice);
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
        assert!(encountered);
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn vehicle_auto_fall_native_lua_and_storage_share_control_and_rollback() {
    let (_dir, config, world, id) = fixture(VehicleMovement::Tracked, false).await;
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "unitprefs").contains("AutoFall: OFF")
    );
    let initial = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!("btech.unit.auto_fall({},2,true)", id.0))
            .is_err()
    );
    assert_eq!(initial, scripts.world().btech);
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "unitprefs autofall on")
            .contains("ON")
    );
    assert!(scripts.world().btech.vehicles()[&id].auto_fall());
    let enabled = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.auto_fall({},1,false); error('abort')",
                id.0
            ))
            .is_err()
    );
    assert_eq!(enabled, scripts.world().btech);
    let value: bool = scripts
        .eval_callback(&format!("return btech.unit.state({}).auto_fall", id.0))
        .unwrap();
    assert!(value);
    scripts
        .eval_callback::<()>(&format!("btech.unit.auto_fall({},1,false)", id.0))
        .unwrap();
    assert_eq!(initial, scripts.world().btech);
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "unitprefs autofall").contains("ON")
    );
    let snapshot = scripts.world().clone();
    persistence::save(&config.database(), &snapshot)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        snapshot.btech
    );
}
