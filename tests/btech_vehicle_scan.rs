//! Mixed-class detailed unit scans preserve range, disclosure and native/Lua transaction rules.
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

/// A close mixed formation guarantees contact acquisition without a random fixture dependency.
async fn formation() -> (tempfile::TempDir, Config, World, [ObjectId; 4]) {
    let (dir, config, mut world, map, ids) = fixture(
        ".0\n.0\n.0\n.0\n.0\n",
        include_str!("../game/units/Demolisher.toml"),
    )
    .await;
    for id in ids {
        place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    }
    power(&mut world, &ids, Power::Running);
    (dir, config, world, ids)
}

#[tokio::test]
async fn mixed_unit_scans_match_native_lua_and_preserve_ordinary_disclosure() {
    let (_dir, config, initial, [a, b, c, d]) = formation().await;
    for (observer, target, kind) in [(a, d, "VEHICLE"), (c, b, "MECH"), (c, d, "VEHICLE")] {
        let mut world = initial.clone();
        refresh_battle_contacts(&mut world, &[a, b, c, d]).unwrap();
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(observer);
        assign_battle_pilot(&mut world, observer, ObjectId(1)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        let before = world.btech.clone();
        let text = scan_battle_unit(&world, observer, ObjectId(1), target, "").unwrap();
        assert!(text.contains(&format!("Type: {kind}")));
        assert!(text.contains("WEAPON SYSTEMS"));
        if kind == "VEHICLE" {
            assert!(stompymux_rs::text::plain(&text).contains(",`.OO,'."));
            assert!(
                stompymux_rs::text::plain(&text)
                    .lines()
                    .any(|line| line.starts_with("Key"))
            );
            assert!(!text.contains("Front_Side: 40/8"));
            assert!(
                stompymux_rs::text::plain(&text)
                    .contains(" AC/20              [ 0]  Turret        Ready")
            );
            let mut damaged = world.clone();
            damaged
                .btech
                .rewrite_unit_record(target, |record| {
                    record["weapon_failures"]["0"] = serde_json::json!("disabled");
                })
                .unwrap();
            let damaged_text =
                scan_battle_unit(&damaged, observer, ObjectId(1), target, "W").unwrap();
            assert!(
                stompymux_rs::text::plain(&damaged_text)
                    .contains(" AC/20              [ 0]  Turret        *****")
            );
            assert!(
                stompymux_rs::text::plain(&damaged_text)
                    .contains(" AC/20              [ 1]  Turret        Ready")
            );
        }
        assert_eq!(world.btech, before);
        assert!(scan_battle_unit(&world, observer, ObjectId(2), target, "").is_err());
        assert!(scan_battle_unit(&world, observer, ObjectId(1), target, "X").is_err());
        let label = if let Some(vehicle) = world.btech.vehicles().get(&target) {
            vehicle.battlefield_id().unwrap()
        } else {
            world.btech.constructed_units()[&target]
                .battlefield_id()
                .unwrap()
        };
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let native = support::run_text(&scripts, &config, ObjectId(1), 1, &format!("scan {label}"));
        assert!(native.contains(&text), "{native}");
        let lua: String = scripts
            .eval_callback(&format!(
                "return btech.unit.scan({},1,{})",
                observer.0, target.0
            ))
            .unwrap();
        assert_eq!(lua, text);
        let before = scripts.world().clone();
        scripts.drain_outbox();
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.scan({},1,{}); error('abort')",
                    observer.0, target.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before.btech);
        assert!(scripts.drain_outbox().is_empty());
        persistence::save(&config.database(), &before)
            .await
            .unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            scan_battle_unit(&restored, observer, ObjectId(1), target, "").unwrap(),
            text
        );
    }
}

#[tokio::test]
async fn vehicle_scan_ranges_and_observer_disclosure_follow_current_state() {
    let (_dir, _config, mut world, [a, b, c, d]) = formation().await;
    refresh_battle_contacts(&mut world, &[a, b, c, d]).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(a);
    assign_battle_pilot(&mut world, a, ObjectId(1)).unwrap();
    set_battle_observer(&mut world, a, true).unwrap();
    let exact = scan_battle_unit(&world, a, ObjectId(1), d, "A").unwrap();
    assert!(
        stompymux_rs::text::plain(&exact)
            .contains("         ,`.40,'.                              ,`. 8,'."),
        "{exact}"
    );
    assert!(exact.contains(&battle_unit_status(&world, d, "armor").unwrap()));
    assert!(!exact.contains("Weapons ("));
    let (_dir, _config, mut world, map, [a, b, c, d]) = fixture(
        ".0\n.0\n.0\n.0\n.0\n",
        include_str!("../game/units/Demolisher.toml"),
    )
    .await;
    power(&mut world, &[a, b, c, d], Power::Running);
    set_battle_map_visibility(&mut world, map, Light::Day, 30).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(c);
    assign_battle_pilot(&mut world, c, ObjectId(1)).unwrap();
    refresh_battle_contacts(&mut world, &[c]).unwrap();
    assert!(world.btech.vehicles()[&c].contacts().contains_key(&d));
    world
        .btech
        .rewrite_unit_record(c, |record| {
            record["definition"]["attributes"]["scan_range"] = serde_json::json!("3");
        })
        .unwrap();
    let before = world.btech.clone();
    assert!(
        scan_battle_unit(&world, c, ObjectId(1), d, "")
            .unwrap_err()
            .to_string()
            .contains("out of scanner range")
    );
    assert!(report_battle_unit(&world, c, ObjectId(1), d).is_ok());
    assert_eq!(world.btech, before);
    world
        .btech
        .rewrite_unit_record(c, |record| {
            record["hardware"]["scan"] = serde_json::json!({"value": 0, "sensor_hits": 0});
        })
        .unwrap();
    assert_eq!(world.btech.vehicles()[&c].sensor_ranges().scan, 0);
    assert!(
        report_battle_unit(&world, c, ObjectId(1), d)
            .unwrap_err()
            .to_string()
            .contains("inoperational")
    );
}

#[tokio::test]
async fn coordinate_scans_choose_visible_mixed_occupants_in_battlefield_order() {
    let (_dir, config, mut initial, [observer, mech, vehicle, other]) = formation().await;
    // Membership, rather than class or object identity, decides which occupant is inspected.
    let mut saved = serde_json::to_value(&initial.btech).unwrap();
    saved["constructed"][mech.0.to_string()]["map_slot"] = serde_json::json!(2);
    saved["vehicles"][vehicle.0.to_string()]["map_slot"] = serde_json::json!(1);
    initial.btech = serde_json::from_value(saved).unwrap();
    initial.objects.get_mut(&ObjectId(1)).unwrap().location = Some(observer);
    assign_battle_pilot(&mut initial, observer, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut initial, ObjectId(1), support::FIXTURE_DICE_SEED);
    refresh_battle_contacts(&mut initial, &[observer]).unwrap();
    initial.validate(&config).unwrap();
    for selected in [Some(vehicle), Some(mech), None] {
        let mut world = initial.clone();
        world
            .btech
            .rewrite_unit_record(observer, |record| {
                let contacts = record["contacts"].as_object_mut().unwrap();
                contacts.remove(&other.0.to_string());
                if selected != Some(vehicle) {
                    contacts.remove(&vehicle.0.to_string());
                }
                if selected.is_none() {
                    contacts.clear();
                }
            })
            .unwrap();
        let expected = selected.map_or_else(
            || "You see nobody in the hex!".to_owned(),
            |target| scan_battle_unit(&world, observer, ObjectId(1), target, "").unwrap(),
        );
        let report = selected
            .map(|target| report_battle_unit(&world, observer, ObjectId(1), target).unwrap());
        let before = world.btech.clone();
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
        let native = support::run_text(&scripts, &config, ObjectId(1), 1, "scan 0 0");
        assert!(native.contains(&expected), "{native}");
        let lua: String = scripts
            .eval_callback(&format!("return btech.unit.scan_hex({},1,0,0)", observer.0))
            .unwrap();
        assert_eq!(lua, expected);
        let native_report = support::run_text(&scripts, &config, ObjectId(1), 2, "report 0 0");
        assert!(native_report.contains(report.as_deref().unwrap_or("No target found.")));
        assert_eq!(scripts.world().btech, before);
        scripts.drain_outbox();
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.scan_hex({},1,0,0); error('abort')",
                    observer.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
    }
}

/// A nearby structure shares a coordinate with a minefield without triggering either during setup.
fn terrain_targets(world: &mut World, config: &Config, map: ObjectId) -> ObjectId {
    let interior = world.create(config, "Hangar".into(), Kind::Room);
    create_battle_map(
        world,
        interior,
        "hangar",
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(world, interior, support::FIXTURE_DICE_SEED);
    set_building_state(
        world,
        interior,
        BuildingState {
            integrity: 31,
            maximum_integrity: 50,
            flags: 0,
            regeneration: 1,
        },
    )
    .unwrap();
    let coordinate = HexCoordinate { x: 0, y: 1 };
    set_building_entrance(
        world,
        map,
        0,
        Some(BuildingEntrance {
            coordinate,
            interior,
            data_char: 0,
            data_short: 0,
            data_int: 0,
        }),
    )
    .unwrap();
    set_minefield(
        world,
        map,
        0,
        Some(Minefield {
            coordinate,
            kind: MineKind::Command,
            strength: 12,
            extra: 123,
            owner: ObjectId(1),
        }),
    )
    .unwrap();
    interior
}

/// Coordinate, structure and minefield scans share admission across native and Lua callers,
/// and need the hex to be perceived.
#[tokio::test]
async fn vehicle_coordinate_and_structure_scans_share_native_lua_admission() {
    let (_dir, config, mut world, [mech, _, observer, _]) = formation().await;
    let map = world.btech.vehicles()[&observer].position().unwrap().map;
    terrain_targets(&mut world, &config, map);
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(observer);
    assign_battle_pilot(&mut world, observer, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    refresh_battle_contacts(&mut world, &[observer]).unwrap();
    let coordinate = HexCoordinate { x: 0, y: 1 };
    assert!(battle_hex_visible(&world, observer, coordinate).unwrap());
    let expected = scan_battle_unit(&world, observer, ObjectId(1), mech, "").unwrap();
    let before = world.btech.clone();
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert!(support::run_text(&scripts, &config, ObjectId(1), 1, "scan 0 0").contains(&expected));
    let lua: String = scripts
        .eval_callback(&format!("return btech.unit.scan_hex({},1,0,0)", observer.0))
        .unwrap();
    assert_eq!(lua, expected);
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 2, "scan 0 1 B"),
        "The Hangar's CF is 31."
    );
    let lua: String = scripts
        .eval_callback(&format!(
            "return btech.unit.scan_building({},1,0,1).text",
            observer.0
        ))
        .unwrap();
    assert_eq!(lua, "The Hangar's CF is 31.");
    assert_eq!(scripts.world().btech, before);
    assert!(scan_battle_building(&mut world, observer, ObjectId(2), coordinate, 1000).is_err());
    assert!(
        scan_battle_building(
            &mut world,
            observer,
            ObjectId(1),
            HexCoordinate { x: 1, y: 9 },
            1000
        )
        .is_err()
    );
    assert_eq!(world.btech, before);
    world
        .btech
        .rewrite_unit_record(observer, |record| {
            record["definition"]["attributes"]["scan_range"] = serde_json::json!("1");
        })
        .unwrap();
    let limited = world.btech.clone();
    assert!(
        scan_battle_building(
            &mut world,
            observer,
            ObjectId(1),
            HexCoordinate { x: 0, y: 4 },
            1000
        )
        .unwrap_err()
        .to_string()
        .contains("out of scanner range")
    );
    assert_eq!(world.btech, limited);
    // The sensor band sees the adjacent hex in any weather until the battlefield disables it.
    set_battle_map_visibility(&mut world, map, Light::Day, 0).unwrap();
    assert_eq!(
        battle_hex_detection(&world, observer, coordinate).unwrap(),
        Some(DetectionChannel::Sensors)
    );
    set_battle_map_perception(&mut world, map, MapPerceptionFlag::Sensors, false).unwrap();
    assert!(!battle_hex_visible(&world, observer, coordinate).unwrap());
    let disabled = world.btech.clone();
    assert!(scan_battle_mines(&mut world, observer, ObjectId(1), coordinate, 1000).is_err());
    assert_eq!(world.btech, disabled);
}

#[tokio::test]
async fn vehicle_terrain_perception_owns_dice_and_rolls_back_experience() {
    let (_dir, config, mut world, [_, _, observer, _]) = formation().await;
    let map = world.btech.vehicles()[&observer].position().unwrap().map;
    let interior = terrain_targets(&mut world, &config, map);
    let mut building = world.btech.maps()[&interior].building;
    building.flags = 4;
    set_building_state(&mut world, interior, building).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(observer);
    assign_battle_pilot(&mut world, observer, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    let coordinate = HexCoordinate { x: 0, y: 1 };
    let before = world.btech.clone();
    assert!(
        scan_battle_building(&mut world, observer, ObjectId(1), coordinate, 1000)
            .unwrap()
            .text
            .contains("no building")
    );
    assert_eq!(world.btech, before);
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    let mut dice: Dice =
        serde_json::from_value(saved["vehicles"][observer.0.to_string()]["dice"].clone()).unwrap();
    dice.die(8).unwrap();
    saved["vehicles"][observer.0.to_string()]["dice"] = serde_json::to_value(dice).unwrap();
    assert!(
        !scan_battle_mines(&mut world, observer, ObjectId(1), coordinate, 1000)
            .unwrap()
            .found
    );
    assert_eq!(serde_json::to_value(&world.btech).unwrap(), saved);
    world
        .objects
        .get_mut(&observer)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    set_battle_character(
        &mut world,
        ObjectId(1),
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
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    world
        .btech
        .rewrite_unit_record(observer, |record| {
            record["scanner_perception"] = serde_json::json!(18);
        })
        .unwrap();
    let mut expected = world.clone();
    roll_unit_dice(&mut expected, observer, 2).unwrap();
    assert!(
        scan_battle_building(&mut world, observer, ObjectId(1), coordinate, 1000)
            .unwrap()
            .text
            .contains("no building")
    );
    assert_eq!(world.btech, expected.btech);
    world
        .btech
        .rewrite_unit_record(observer, |record| {
            record["scanner_perception"] = serde_json::json!(-10);
        })
        .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    restored
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let report = scan_battle_building(&mut world, observer, ObjectId(1), coordinate, 1000).unwrap();
    assert_eq!(report.text, "The Hangar's CF is 31.");
    assert_eq!(report.experience_messages.len(), 1);
    assert_eq!(
        scan_battle_building(&mut restored, observer, ObjectId(1), coordinate, 1000).unwrap(),
        report
    );
    let mines = scan_battle_mines(&mut world, observer, ObjectId(1), coordinate, 1031).unwrap();
    assert!(mines.found);
    assert_eq!(mines.experience_messages.len(), 1);
    assert!(!mines.text.contains("123") && !mines.text.contains("Command"));
    assert_eq!(
        scan_battle_mines(&mut restored, observer, ObjectId(1), coordinate, 1031).unwrap(),
        mines
    );
    // Restart restores the referenced damaged building's missing repair interval.
    let mut expected = serde_json::to_value(&world.btech).unwrap();
    expected["maps"][interior.0.to_string()]["building_repair"] = 120.into();
    assert_eq!(serde_json::to_value(&restored.btech).unwrap(), expected);
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.scan_terrain({},1,0,1); error('abort')",
                observer.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    let native = support::run_text(&scripts, &config, ObjectId(1), 1, "scan 0 1 H");
    assert!(
        native.contains("The Hangar's CF is 31.") && native.contains("Small bomblets"),
        "{native}"
    );
    let found: bool = scripts
        .eval_callback(&format!(
            "return btech.unit.scan_terrain({},1,0,1).mines.found",
            observer.0
        ))
        .unwrap();
    assert!(found);
}

/// Building contacts admit passengers, follow display preferences and consult identification
/// locks only for buildings the unit currently perceives.
#[tokio::test]
async fn vehicle_building_contacts_honor_passengers_preferences_and_identification_locks() {
    let (_dir, config, mut world, [_, _, observer, _]) = formation().await;
    let map = world.btech.vehicles()[&observer].position().unwrap().map;
    let interior = terrain_targets(&mut world, &config, map);
    // Contact displays admit passengers; detailed scan actions still require the assigned pilot.
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(observer);
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    let before = scripts.world().btech.clone();
    let contacts = battle_building_contacts(&scripts, observer, ObjectId(1)).unwrap();
    assert_eq!(contacts.len(), 1);
    assert_eq!(contacts[0].name, "Hangar");
    assert_eq!(
        (contacts[0].integrity, contacts[0].maximum_integrity),
        (31, 50)
    );
    assert_eq!(contacts[0].weapon_arc, ContactArc::Rear);
    assert_eq!(contacts[0].elevation, 1);
    assert!(contacts[0].identified);
    for command in ["contacts", "contacts b"] {
        let native = support::run_text(&scripts, &config, ObjectId(1), 1, command);
        assert!(
            native.contains(&contacts[0].styled_text()),
            "{command}: {native}"
        );
    }
    let lua: mlua::Table = scripts
        .eval_callback(&format!(
            "return btech.unit.building_contacts({},1)",
            observer.0
        ))
        .unwrap();
    assert_eq!(
        serde_json::to_value(lua).unwrap(),
        serde_json::to_value(&contacts).unwrap()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    assert!(!support::run_text(&scripts, &config, ObjectId(1), 1, "contacts +").contains("Hangar"));
    set_battle_contact_preferences(
        &mut shared.borrow_mut(),
        ObjectId(1),
        ContactPreferences {
            buildings: BuildingContactMode::FollowBrief,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(support::run_text(&scripts, &config, ObjectId(1), 1, "contacts +").contains("Hangar"));
    assert!(battle_building_contacts(&scripts, observer, ObjectId(2)).is_err());
    support::run_text(&scripts, &config, ObjectId(1), 1, "brief C2");
    assert!(!support::run_text(&scripts, &config, ObjectId(1), 1, "contacts").contains("Hangar"));
    assert!(support::run_text(&scripts, &config, ObjectId(1), 1, "contacts b").contains("Hangar"));
    support::run_text(&scripts, &config, ObjectId(1), 1, "brief C1");
    scripts.drain_outbox();
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    let restarted =
        Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
    assert_eq!(
        battle_building_contacts(&restarted, observer, ObjectId(1)).unwrap(),
        contacts
    );
    let parents: mlua::Table = scripts
        .inspect_lua()
        .named_registry_value("mux.parents")
        .unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("_parents", parents)
        .unwrap();
    scripts.eval_callback::<()>(&format!(
        "calls=0; _parents['default_room.lua'].locks={{identify_building=function(ctx) calls=calls+1; assert(ctx.object=={} and ctx.enactor==1 and ctx.cause=={} and ctx.silent); return false end}}",
        interior.0, observer.0
    )).unwrap();
    let denied = battle_building_contacts(&scripts, observer, ObjectId(1)).unwrap();
    assert_eq!(denied[0].status, 'x');
    assert!(!denied[0].identified);
    let mut building = scripts.world().btech.maps()[&interior].building;
    building.flags = 4;
    set_building_state(&mut shared.borrow_mut(), interior, building).unwrap();
    assert!(
        battle_building_contacts(&scripts, observer, ObjectId(1))
            .unwrap()
            .is_empty()
    );
    scripts.eval_callback::<()>("_parents['default_room.lua'].locks.identify_building=function(ctx) calls=calls+1; return true end").unwrap();
    let hidden = battle_building_contacts(&scripts, observer, ObjectId(1)).unwrap();
    assert!(hidden[0].hidden && hidden[0].identified);
    let calls: i64 = scripts.eval_callback("return calls").unwrap();
    building.flags = 16;
    set_building_state(&mut shared.borrow_mut(), interior, building).unwrap();
    assert!(
        battle_building_contacts(&scripts, observer, ObjectId(1))
            .unwrap()
            .is_empty()
    );
    assert_eq!(scripts.eval_callback::<i64>("return calls").unwrap(), calls);
    building.flags = 0;
    set_building_state(&mut shared.borrow_mut(), interior, building).unwrap();
    scripts.eval_callback::<()>(&format!(
        "_parents['default_room.lua'].locks.identify_building=function(ctx) btech.unit.brief({},1,'C2'); error('lock failed') end", observer.0
    )).unwrap();
    let before = scripts.world().btech.clone();
    assert!(battle_building_contacts(&scripts, observer, ObjectId(1)).is_err());
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    // Only sight can lose the adjacent building, so leave it to weather.
    set_battle_map_perception(
        &mut shared.borrow_mut(),
        map,
        MapPerceptionFlag::Sensors,
        false,
    )
    .unwrap();
    scripts.eval_callback::<()>(&format!(
        "_parents['default_room.lua'].locks.identify_building=function(ctx) btech.map.conditions({},btech.map.light_levels.DAY,0); return true end", map.0
    )).unwrap();
    assert!(
        battle_building_contacts(&scripts, observer, ObjectId(1))
            .unwrap()
            .is_empty()
    );
    assert_eq!(scripts.world().btech.maps()[&map].visibility, 0);
    scripts.eval_callback::<()>("_parents['default_room.lua'].locks.identify_building=function(ctx) error('must not run') end").unwrap();
    set_battle_map_visibility(&mut shared.borrow_mut(), map, Light::Day, 0).unwrap();
    // No visible candidate means the deliberately failing lock must not run.
    assert!(
        battle_building_contacts(&scripts, observer, ObjectId(1))
            .unwrap()
            .is_empty()
    );
}
