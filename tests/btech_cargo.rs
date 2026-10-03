//! Shared cargo command admission, all-or-nothing transfers and native/Lua replay across chassis.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Supported carrier constructions exercise shared rules rather than separate transfer implementations.
fn templates() -> Vec<String> {
    let ground = include_str!("../game/mechs/Demolisher.toml");
    vec![
        include_str!("../game/mechs/JR7-D.toml").into(),
        include_str!("../game/mechs/GOL-1H.toml").into(),
        ground.into(),
        ground.replace("movement = \"track\"", "movement = \"wheel\""),
        ground.replace("movement = \"track\"", "movement = \"hover\""),
        ground
            .replace("movement = \"track\"", "movement = \"none\"")
            .replace("walk_mp = 5", "walk_mp = 0"),
        include_str!("../game/mechs/Kestrel.toml").into(),
    ]
}

/// A powered CargoTech unit sits at its configured loading point with an assigned operator.
async fn fixture(source: &str) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Hangar".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "bay",
        BattleMapAsset::from_cells("3 2\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    set_battle_cargo_transfer_point(
        &mut world,
        ObjectId(1),
        map,
        Some(BattleCargoTransferPoint {
            x: 0,
            y: 0,
            reveal_hint: false,
        }),
    )
    .unwrap();
    let unit = world.create(&config, "Carrier".into(), Kind::Thing);
    let mut template = BattleUnitTemplate::parse("test", source).unwrap();
    let attributes = match &mut template {
        BattleUnitTemplate::Mech(unit) => &mut unit.attributes,
        BattleUnitTemplate::Vehicle(unit) => &mut unit.attributes,
    };
    let flags = attributes.entry("specials".into()).or_default();
    if flags == "-" {
        flags.clear();
    }
    if !flags
        .split_whitespace()
        .any(|flag| flag.eq_ignore_ascii_case("CargoTech"))
    {
        flags.push_str(" CargoTech");
    }
    template.create(&mut world, unit).unwrap();
    place_battle_unit(&mut world, unit, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(unit);
    assign_battle_pilot(&mut world, unit, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, unit, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    set_battle_inventory_named(&mut world, ObjectId(1), map, "Gold", 20).unwrap();
    (dir, config, world, map, unit)
}

/// Inspect quantities through the public stock model.
fn count(world: &World, holder: ObjectId, part: i32) -> i32 {
    battle_inventory(world, holder)
        .unwrap()
        .iter()
        .filter(|entry| entry.part_id == part)
        .map(|entry| entry.quantity)
        .sum()
}

/// Every chassis loads available stock and unloads without load-only startup, IC and point restrictions.
#[tokio::test]
async fn all_chassis_share_load_unload_gates_and_restart() {
    for source in templates() {
        let (_dir, config, mut world, map, unit) = fixture(&source).await;
        assert!(
            battle_cargo_manifest(&world, &config, ObjectId(1), false, "")
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            battle_cargo_manifest(&world, &config, ObjectId(1), true, "G?ld").unwrap()[0].quantity,
            20
        );
        let moved =
            transfer_battle_cargo(&mut world, &config, ObjectId(1), true, "G*", 25).unwrap();
        assert_eq!(moved[0].quantity, 20);
        assert_eq!((count(&world, map, 528), count(&world, unit, 528)), (0, 20));
        assert_eq!(battle_inventory_mass(&world, unit).unwrap(), 4080);
        persistence::save(&config.database(), &world).await.unwrap();
        world = persistence::load(&config.database()).await.unwrap();
        stop_battle_unit(
            &mut world,
            unit,
            ObjectId(1),
            BattleFallRules::configured(&config),
        )
        .unwrap();
        world
            .objects
            .get_mut(&map)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        set_battle_cargo_transfer_point(
            &mut world,
            ObjectId(1),
            map,
            Some(BattleCargoTransferPoint {
                x: 2,
                y: 1,
                reveal_hint: false,
            }),
        )
        .unwrap();
        let before = world.btech.clone();
        assert!(transfer_battle_cargo(&mut world, &config, ObjectId(1), true, "Gold", 1).is_err());
        assert_eq!(world.btech, before);
        assert!(battle_cargo_manifest(&world, &config, ObjectId(1), true, "").is_err());
        let moved =
            transfer_battle_cargo(&mut world, &config, ObjectId(1), false, "#528", i32::MAX)
                .unwrap();
        assert_eq!(moved[0].quantity, 20);
        assert_eq!((count(&world, map, 528), count(&world, unit, 528)), (20, 0));
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

/// Multiple stock edits roll back together on overflow or unknown carried mass, and requests cap at 50,000.
#[tokio::test]
async fn multi_part_transfers_are_atomic_and_quantity_bounded() {
    let (_dir, config, mut world, map, unit) =
        fixture(include_str!("../game/mechs/JR7-D.toml")).await;
    set_battle_inventory_named(&mut world, ObjectId(1), map, "Medical_Supplies", 3).unwrap();
    set_battle_inventory_named(&mut world, ObjectId(1), unit, "Medical_Supplies", i32::MAX)
        .unwrap();
    let before = world.btech.clone();
    assert!(
        transfer_battle_cargo(&mut world, &config, ObjectId(1), true, "*", 1)
            .unwrap_err()
            .to_string()
            .contains("overflow")
    );
    assert_eq!(world.btech, before);
    set_battle_inventory_named(&mut world, ObjectId(1), unit, "Medical_Supplies", 0).unwrap();
    set_battle_inventory_quantity(&mut world, ObjectId(1), map, i32::MAX, 1).unwrap();
    let before = world.btech.clone();
    assert!(transfer_battle_cargo(&mut world, &config, ObjectId(1), true, "*", 1).is_err());
    assert_eq!(world.btech, before);
    for quantity in [0, -1] {
        assert!(
            transfer_battle_cargo(&mut world, &config, ObjectId(1), true, "Gold", quantity)
                .is_err()
        );
        assert_eq!(world.btech, before);
    }
    set_battle_inventory_named(&mut world, ObjectId(1), map, "Gold", 50_001).unwrap();
    let moved =
        transfer_battle_cargo(&mut world, &config, ObjectId(1), true, "Gold", i32::MAX).unwrap();
    assert_eq!(moved[0].quantity, 50_000);
    assert_eq!(count(&world, map, 528), 1);
    assert_eq!(battle_throttle_maximum(&world, unit, true).unwrap(), 0.0);
    transfer_battle_cargo(&mut world, &config, ObjectId(1), false, "Gold", i32::MAX).unwrap();
    assert_eq!(count(&world, map, 528), 50_001);
}

/// Cargo access retains the IC pilot-only rule, OOC/Wizard exceptions, and the global command switch.
#[tokio::test]
async fn cargo_authority_and_location_gates_match_the_operation() {
    let (dir, config, mut world, map, unit) =
        fixture(include_str!("../game/mechs/JR7-D.toml")).await;
    let passenger = world.create(&config, "Passenger".into(), Kind::Player);
    world.objects.get_mut(&passenger).unwrap().location = Some(unit);
    world
        .objects
        .get_mut(&unit)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let before = world.btech.clone();
    assert!(transfer_battle_cargo(&mut world, &config, passenger, true, "Gold", 1).is_err());
    assert_eq!(world.btech, before);
    assert_eq!(
        battle_cargo_manifest(&world, &config, passenger, true, "").unwrap()[0].quantity,
        20
    );
    world
        .objects
        .get_mut(&unit)
        .unwrap()
        .flags
        .remove(Flag::InCharacter);
    transfer_battle_cargo(&mut world, &config, passenger, true, "Gold", 1).unwrap();
    set_battle_cargo_transfer_point(
        &mut world,
        ObjectId(1),
        map,
        Some(BattleCargoTransferPoint {
            x: 2,
            y: 1,
            reveal_hint: false,
        }),
    )
    .unwrap();
    let before = world.btech.clone();
    let error = transfer_battle_cargo(&mut world, &config, ObjectId(1), true, "Gold", 1)
        .unwrap_err()
        .to_string();
    assert!(!error.contains("2,1"));
    assert_eq!(world.btech, before);
    transfer_battle_cargo(&mut world, &config, passenger, false, "Gold", 1).unwrap();
    let path = dir.path().join("stompymux.toml");
    let source = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        source.replace(
            "allow_cargo_commands = true",
            "allow_cargo_commands = false",
        ),
    )
    .unwrap();
    let enabled_config = config;
    let config = Config::load(dir.path()).unwrap();
    let before = world.btech.clone();
    for load in [true, false] {
        assert!(transfer_battle_cargo(&mut world, &config, ObjectId(1), load, "Gold", 1).is_err());
        assert_eq!(world.btech, before);
    }
    for stores in [true, false] {
        assert!(battle_cargo_manifest(&world, &config, ObjectId(1), stores, "").is_err());
    }
    let config = enabled_config;
    world.objects.get_mut(&passenger).unwrap().location = Some(map);
    assert_eq!(
        battle_cargo_manifest(&world, &config, passenger, false, "Gold").unwrap()[0].quantity,
        20
    );
}

/// Native and Lua transfers publish identical stock and support late-callback rollback on every chassis.
#[tokio::test]
async fn cargo_commands_and_lua_share_state_and_callback_rollback() {
    for source in templates() {
        let (_dir, config, world, _map, unit) = fixture(&source).await;
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        for (command, callback, fragment) in [
            (
                "loadcargo Gold 3",
                "btech.cargo.load(1,'Gold',3)",
                "You load 3 Golds.",
            ),
            (
                "unloadcargo Gold 2",
                "btech.cargo.unload(1,'Gold',2)",
                "You unload 2 Golds.",
            ),
        ] {
            let before = lua.world().btech.clone();
            assert!(
                lua.eval_callback::<()>(&format!("{callback}; error('abort')"))
                    .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            let output = support::run_text(&native, &config, ObjectId(1), 1, command);
            assert!(output.contains(fragment), "{output}");
            lua.eval_callback::<mlua::Table>(&format!("return {callback}"))
                .unwrap();
            assert_eq!(lua.world().btech, native.world().btech);
        }
        assert_eq!(count(&lua.world(), unit, 528), 1);
        for (command, callback, quantity) in [
            ("manifest", "return btech.cargo.manifest(1)", 1),
            ("stores", "return btech.cargo.stores(1)", 19),
        ] {
            let output = support::run_text(&native, &config, ObjectId(1), 1, command);
            assert!(output.contains(&format!("{quantity} Gold")), "{output}");
            let rows: mlua::Table = lua.eval_callback(callback).unwrap();
            let row: mlua::Table = rows.get(1).unwrap();
            assert_eq!(row.get::<i32>("quantity").unwrap(), quantity);
            row.set("quantity", 999).unwrap();
            assert_eq!(lua.world().btech, native.world().btech);
        }
    }
}

/// Full and short names filter stock without duplicating equipment definitions.
#[tokio::test]
async fn cargo_patterns_select_multiple_part_types() {
    let (_dir, config, mut world, map, unit) =
        fixture(include_str!("../game/mechs/JR7-D.toml")).await;
    set_battle_inventory_named(&mut world, ObjectId(1), map, "IS.MediumLaser", 5).unwrap();
    let rows =
        transfer_battle_cargo(&mut world, &config, ObjectId(1), true, "IS.MediumLaser", 1).unwrap();
    assert_eq!(
        (rows.len(), rows[0].part_id, rows[0].quantity),
        (1, BattleWeapon::MediumLaser.part_id(), 1)
    );
    let rows =
        transfer_battle_cargo(&mut world, &config, ObjectId(1), true, "MediumLas?r", 20).unwrap();
    assert_eq!(rows.iter().map(|row| row.quantity).sum::<i32>(), 4);
    assert_eq!(count(&world, unit, BattleWeapon::MediumLaser.part_id()), 5);
    assert!(
        battle_cargo_manifest(&world, &config, ObjectId(1), false, "Gold\\*")
            .unwrap()
            .is_empty()
    );
    world.validate(&config).unwrap();
}

/// Transfers enforce actual motion and immediately reconcile shared throttle limits without a tick.
#[tokio::test]
async fn moving_load_is_rejected_and_loading_clamps_pending_throttle() {
    for source in templates() {
        let (_dir, config, mut world, map, unit) = fixture(&source).await;
        let maximum = battle_throttle_maximum(&world, unit, true).unwrap();
        if maximum == 0.0 {
            continue;
        }
        let mech = world.btech.constructed_units().contains_key(&unit);
        let registry = if mech { "constructed" } else { "vehicles" };
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        encoded[registry][unit.0.to_string()]["motion"]["speed"] = 1.0.into();
        world.btech = serde_json::from_value(encoded).unwrap();
        let before = world.btech.clone();
        let error = transfer_battle_cargo(&mut world, &config, ObjectId(1), true, "Gold", 1)
            .unwrap_err()
            .to_string();
        assert!(error.contains("moving"), "{error}");
        assert_eq!(world.btech, before);
        set_battle_inventory_named(&mut world, ObjectId(1), unit, "Gold", 1).unwrap();
        transfer_battle_cargo(&mut world, &config, ObjectId(1), false, "Gold", 1).unwrap();
        set_battle_inventory_named(&mut world, ObjectId(1), map, "Gold", 50_000).unwrap();
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        encoded[registry][unit.0.to_string()]["motion"]["speed"] = 0.0.into();
        encoded[registry][unit.0.to_string()]["motion"]["desired_speed"] = maximum.into();
        world.btech = serde_json::from_value(encoded).unwrap();
        transfer_battle_cargo(&mut world, &config, ObjectId(1), true, "Gold", 50_000).unwrap();
        let motion = if mech {
            world.btech.constructed_units()[&unit].motion().unwrap()
        } else {
            world.btech.vehicles()[&unit].motion().unwrap()
        };
        let loaded = battle_throttle_maximum(&world, unit, true).unwrap();
        assert!(loaded < maximum);
        assert!(motion.desired_speed <= loaded);
        assert_eq!(motion.speed, 0.0);
    }
}

/// Exact catalogue choices do not fall through to stocked aliases; both adapters share that priority.
#[tokio::test]
async fn cargo_abbreviations_resolve_before_stock() {
    for source in templates() {
        let (_dir, config, mut world, map, unit) = fixture(&source).await;
        let laser = BattleWeapon::MediumLaser.part_id();
        set_battle_inventory_quantity(&mut world, ObjectId(1), map, laser, 3).unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let output = support::run_text(&scripts, &config, ObjectId(1), 1, "loadcargo ml 1");
        assert!(output.contains("IS.MediumLaser"), "{output}");
        let rows: mlua::Table = scripts
            .eval_callback("return btech.cargo.load(1, 'IS.MediumLaser', 1)")
            .unwrap();
        assert_eq!(
            rows.get::<mlua::Table>(1)
                .unwrap()
                .get::<i32>("part_id")
                .unwrap(),
            laser
        );
        let rows: mlua::Table = scripts
            .eval_callback("return btech.cargo.unload(1, 'ML', 1)")
            .unwrap();
        assert_eq!(
            rows.get::<mlua::Table>(1)
                .unwrap()
                .get::<i32>("quantity")
                .unwrap(),
            1
        );
        let mut world = scripts.world().clone();
        let rows = transfer_battle_cargo(&mut world, &config, ObjectId(1), true, "MediumLas?r", 10)
            .unwrap();
        assert_eq!(
            (rows.len(), rows[0].part_id, rows[0].quantity),
            (1, laser, 2)
        );
        assert_eq!(count(&world, unit, laser), 3);
        set_battle_inventory_quantity(&mut world, ObjectId(1), map, 609, 3).unwrap();
        let before = world.btech.clone();
        assert!(transfer_battle_cargo(&mut world, &config, ObjectId(1), true, "Steel", 1).is_err());
        assert_eq!(world.btech, before);
        let rows =
            transfer_battle_cargo(&mut world, &config, ObjectId(1), true, "#609", 1).unwrap();
        assert_eq!(rows[0].part_id, 609);
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        let rows = transfer_battle_cargo(
            &mut loaded,
            &config,
            ObjectId(1),
            false,
            "IS.MediumLaser",
            3,
        )
        .unwrap();
        assert_eq!(
            (rows.len(), rows[0].part_id, rows[0].quantity),
            (1, laser, 3)
        );
    }
}

/// Diagnostic channels are ordinary administrator-created channels with normal listeners.
fn economy_channel(world: &mut World) {
    let mut channel = Channel::new("MechEconInfo".into());
    channel.users.push(stompymux_rs::communication::Membership {
        who: ObjectId(1),
        listening: true,
    });
    world.channels.insert(channel.name.clone(), channel);
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
}

/// Native and Lua transfers record both stock changes in order and roll back late callback failures.
#[tokio::test]
async fn cargo_economy_channels_share_transfer_order_and_callback_rollback() {
    for source in templates() {
        let (_dir, config, mut world, map, unit) = fixture(&source).await;
        economy_channel(&mut world);
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let text = support::run_text(&native, &config, ObjectId(1), 1, "loadcargo Gold 3");
        assert!(text.contains("MechEconInfo"), "{text}");
        lua.eval_callback::<()>("btech.cargo.load(1, 'Gold', 3)")
            .unwrap();
        support::run_text(&native, &config, ObjectId(1), 1, "unloadcargo Gold 2");
        lua.eval_callback::<()>("btech.cargo.unload(1, 'Gold', 2)")
            .unwrap();
        assert_eq!(native.world().btech, lua.world().btech);
        let expected = [
            format!("#1 added 3 Gold to #{}.", unit.0),
            format!("#1 removed 3 Gold from #{}.", map.0),
            format!("#1 removed 2 Gold from #{}.", unit.0),
            format!("#1 added 2 Gold to #{}.", map.0),
        ];
        for scripts in [&native, &lua] {
            let state = scripts.world();
            let channel = &state.channels["MechEconInfo"];
            assert_eq!(channel.messages, 4);
            assert_eq!(channel.history.len(), 4);
            for (message, expected) in channel.history.iter().zip(&expected) {
                assert!(message.message.contains(expected), "{}", message.message);
            }
        }
        lua.drain_outbox();
        let before = lua.world().clone();
        assert!(
            lua.eval_callback::<()>("btech.cargo.load(1, 'Gold', 1); error('abort cargo')")
                .is_err()
        );
        assert_eq!(lua.world().btech, before.btech);
        assert_eq!(
            serde_json::to_value(&lua.world().channels).unwrap(),
            serde_json::to_value(&before.channels).unwrap()
        );
        assert!(lua.drain_outbox().is_empty());
        lua.eval_callback::<()>("btech.cargo.manifest(1); btech.cargo.stores(1)")
            .unwrap();
        assert_eq!(lua.world().channels["MechEconInfo"].messages, 4);
        persistence::save(&config.database(), &before)
            .await
            .unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, before.btech);
        assert_eq!(loaded.channels["MechEconInfo"].messages, 4);
        assert_eq!(loaded.channels["MechEconInfo"].history.len(), 4);
    }
}

/// A failure on the second channel record restores stock, prior history and staged notifications.
#[tokio::test]
async fn cargo_economy_failure_is_atomic_for_native_lua_and_direct_actions() {
    for source in templates() {
        let (_dir, config, mut world, _map, _unit) = fixture(&source).await;
        economy_channel(&mut world);
        world.channels.get_mut("MechEconInfo").unwrap().messages = i64::MAX - 1;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let before = scripts.world().clone();
        assert!(
            transfer_battle_cargo_action(&scripts, &config, ObjectId(1), true, "Gold", 1).is_err()
        );
        assert_eq!(scripts.world().btech, before.btech);
        assert!(scripts.drain_outbox().is_empty());
        let output = support::run_text(&scripts, &config, ObjectId(1), 1, "loadcargo Gold 1");
        assert!(!output.contains("You load"), "{output}");
        assert!(!output.contains("#1 added"), "{output}");
        let code: String = scripts
            .eval_callback(
                "local ok,e=pcall(btech.cargo.load,1,'Gold',1); assert(not ok); return e.code",
            )
            .unwrap();
        assert_eq!(code, "btech.operation.failed");
        assert_eq!(scripts.world().btech, before.btech);
        assert_eq!(
            serde_json::to_value(&scripts.world().channels).unwrap(),
            serde_json::to_value(&before.channels).unwrap()
        );
        assert!(scripts.drain_outbox().is_empty());
    }
}

/// Loose tanks change capacity without filling themselves; surplus fuel survives unloading and restart.
#[tokio::test]
async fn vtol_auxiliary_tanks_share_capacity_mass_and_saved_surplus_fuel() {
    let (_dir, config, mut world, map, unit) =
        fixture(include_str!("../game/mechs/Kestrel.toml")).await;
    let original_speed = battle_throttle_maximum(&world, unit, true).unwrap();
    set_battle_inventory_quantity(&mut world, ObjectId(1), map, 422, 3).unwrap();
    transfer_battle_cargo(&mut world, &config, ObjectId(1), true, "#422", 3).unwrap();
    let fuel = battle_vtol_fuel_status(&world, unit).unwrap();
    assert_eq!(
        (
            fuel.original_capacity,
            fuel.capacity,
            fuel.remaining,
            fuel.auxiliary_tanks,
            fuel.excess_mass
        ),
        (4000, 10000, 4000, 3, 0)
    );
    assert_eq!(battle_inventory_mass(&world, unit).unwrap(), 306);
    assert_eq!(
        battle_unit_load(&world, unit, true).unwrap().carried_mass,
        153
    );
    let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let output = support::run_text(
        &native,
        &config,
        ObjectId(1),
        1,
        &format!("@btech fuel #{} 9000", unit.0),
    );
    assert!(output.contains("9000/10000"), "{output}");
    lua.eval_callback::<()>(&format!("btech.unit.set_fuel(1,{},9000)", unit.0))
        .unwrap();
    assert_eq!(native.world().btech, lua.world().btech);
    let reported: (u64, u64, i64) = lua.eval_callback(&format!("local f=btech.unit.fuel({}); return f.capacity,f.excess_mass,btech.unit.state({}).fuel.remaining",unit.0,unit.0)).unwrap();
    assert_eq!(reported, (10000, 5000, 9000));
    let output = support::run_text(&native, &config, ObjectId(1), 1, "status");
    assert!(output.contains("Fuel: 9000 (90.00 %)"), "{output}");
    let mut world = lua.world().clone();
    assert_eq!(
        battle_unit_load(&world, unit, true).unwrap().carried_mass,
        2653
    );
    transfer_battle_cargo(&mut world, &config, ObjectId(1), false, "#422", 3).unwrap();
    let fuel = battle_vtol_fuel_status(&world, unit).unwrap();
    assert_eq!(
        (fuel.capacity, fuel.remaining, fuel.excess_mass),
        (4000, 9000, 5000)
    );
    assert_eq!(battle_inventory_mass(&world, unit).unwrap(), 0);
    assert_eq!(
        battle_unit_load(&world, unit, true).unwrap().carried_mass,
        2500
    );
    assert!(battle_throttle_maximum(&world, unit, true).unwrap() < original_speed);
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    assert_eq!(battle_vtol_fuel_status(&loaded, unit).unwrap(), fuel);
    let mut aircraft = loaded.btech.vehicles()[&unit].clone();
    assert_eq!(
        aircraft.consume_vtol_fuel(21.5, 1, false, false).unwrap(),
        BattleVtolFuelUse::Consumed {
            amount: 1,
            remaining: 8999
        }
    );
    assert_eq!(aircraft.vtol_fuel().unwrap().excess_mass(), 4999);
    let replay: BattleVehicle =
        serde_json::from_value(serde_json::to_value(&aircraft).unwrap()).unwrap();
    assert_eq!(replay, aircraft);
    set_battle_vtol_fuel(&mut world, &config, ObjectId(1), unit, 4000).unwrap();
    assert_eq!(
        battle_throttle_maximum(&world, unit, true).unwrap(),
        original_speed
    );
}

/// Fuel corrections enforce authority and current capacity, preserve detached views and roll back callbacks.
#[tokio::test]
async fn vtol_fuel_corrections_are_bounded_authorized_and_transactional() {
    let (_dir, config, mut world, _map, unit) =
        fixture(include_str!("../game/mechs/Kestrel.toml")).await;
    let passenger = world.create(&config, "Passenger".into(), Kind::Player);
    let before = world.btech.clone();
    assert!(set_battle_vtol_fuel(&mut world, &config, passenger, unit, 1).is_err());
    assert!(set_battle_vtol_fuel(&mut world, &config, ObjectId(1), unit, 4001).is_err());
    assert_eq!(world.btech, before);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.set_fuel(1,{},1); error('abort refill')",
                unit.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    scripts
        .eval_callback::<()>(&format!(
            "local f=btech.unit.fuel({}); f.remaining=0; f.capacity=0",
            unit.0
        ))
        .unwrap();
    assert_eq!(scripts.world().btech, before);
    for amount in ["-1", "4294967296"] {
        assert!(
            scripts
                .eval_callback::<()>(&format!("btech.unit.set_fuel(1,{},{amount})", unit.0))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
    }
    for source in templates().into_iter().take(6) {
        let (_dir, config, mut world, _map, unit) = fixture(&source).await;
        set_battle_inventory_quantity(&mut world, ObjectId(1), unit, 422, 1).unwrap();
        assert_eq!(battle_inventory_mass(&world, unit).unwrap(), 102);
        assert!(battle_vtol_fuel_status(&world, unit).is_err());
        let before = world.btech.clone();
        assert!(set_battle_vtol_fuel(&mut world, &config, ObjectId(1), unit, 1).is_err());
        assert_eq!(world.btech, before);
    }
}

/// Large valid inventories derive capacity in wide arithmetic while saved fuel retains its bounded counter.
#[tokio::test]
async fn vtol_tank_capacity_uses_wide_inventory_totals() {
    let (_dir, config, mut world, _map, unit) =
        fixture(include_str!("../game/mechs/Kestrel.toml")).await;
    set_battle_inventory_quantity(&mut world, ObjectId(1), unit, 422, i32::MAX).unwrap();
    let fuel = battle_vtol_fuel_status(&world, unit).unwrap();
    assert_eq!(fuel.capacity, 4000 + i32::MAX as u64 * 2000);
    let fuel = set_battle_vtol_fuel(&mut world, &config, ObjectId(1), unit, u32::MAX).unwrap();
    assert_eq!(fuel.remaining, i64::from(u32::MAX));
    assert_eq!(battle_throttle_maximum(&world, unit, true).unwrap(), 0.0);
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(battle_vtol_fuel_status(&loaded, unit).unwrap(), fuel);
}

/// Absolute Wizard corrections audit their signed change and reconcile unit load without a movement tick.
#[tokio::test]
async fn wizard_stock_actions_share_audits_and_immediate_load_limits() {
    for source in templates() {
        let (_dir, config, mut world, _map, unit) = fixture(&source).await;
        economy_channel(&mut world);
        let maximum = battle_throttle_maximum(&world, unit, true).unwrap();
        let registry = if world.btech.vehicles().contains_key(&unit) {
            "vehicles"
        } else {
            "constructed"
        };
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        encoded[registry][unit.0.to_string()]["motion"]["speed"] = maximum.into();
        encoded[registry][unit.0.to_string()]["motion"]["desired_speed"] = maximum.into();
        world.btech = serde_json::from_value(encoded).unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let output = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("@btech inventory-set #{} Gold 50000", unit.0),
        );
        assert!(output.contains("added 50000 Gold"), "{output}");
        let encoded = serde_json::to_value(&scripts.world().btech).unwrap();
        assert_eq!(
            encoded[registry][unit.0.to_string()]["motion"]["speed"],
            0.0
        );
        assert_eq!(
            encoded[registry][unit.0.to_string()]["motion"]["desired_speed"],
            0.0
        );
        scripts
            .eval_callback::<()>(&format!("btech.inventory.set_named(1,{},'Gold',2)", unit.0))
            .unwrap();
        scripts
            .eval_callback::<()>(&format!("btech.inventory.set(1,{},528,2)", unit.0))
            .unwrap();
        assert_eq!(scripts.world().channels["MechEconInfo"].messages, 2);
        scripts
            .eval_callback::<()>(&format!("btech.inventory.set(1,{},528,0)", unit.0))
            .unwrap();
        let channel = scripts.world().channels["MechEconInfo"].clone();
        assert_eq!(channel.messages, 3);
        assert!(channel.history[1].message.contains("removed 49998 Gold"));
        assert!(channel.history[2].message.contains("removed 2 Gold"));
        assert_eq!(
            battle_throttle_maximum(&scripts.world(), unit, true).unwrap(),
            maximum
        );
        scripts.drain_outbox();
        let before = scripts.world().clone();
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.inventory.set_named(1,{},'Gold',50000); error('abort stock')",
                    unit.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before.btech);
        assert_eq!(
            serde_json::to_value(&scripts.world().channels).unwrap(),
            serde_json::to_value(&before.channels).unwrap()
        );
        assert!(scripts.drain_outbox().is_empty());
        persistence::save(&config.database(), &before)
            .await
            .unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, before.btech);
        assert_eq!(loaded.channels["MechEconInfo"].messages, 3);
    }
}

/// A failed economy record restores the inventory and throttle even when Lua catches the failure.
#[tokio::test]
async fn wizard_stock_publication_failure_restores_both_adapters() {
    for source in templates() {
        let (_dir, config, mut world, _map, unit) = fixture(&source).await;
        economy_channel(&mut world);
        world.channels.get_mut("MechEconInfo").unwrap().messages = i64::MAX;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let before = scripts.world().clone();
        let output = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("@btech inventory-set #{} Gold 50000", unit.0),
        );
        assert!(!output.contains("quantity 50000."), "{output}");
        let code: String = scripts.eval_callback(&format!("local ok,e=pcall(btech.inventory.set_named,1,{},'Gold',50000); assert(not ok); return e.code",unit.0)).unwrap();
        assert_eq!(code, "btech.operation.failed");
        assert_eq!(scripts.world().btech, before.btech);
        assert_eq!(
            serde_json::to_value(&scripts.world().channels).unwrap(),
            serde_json::to_value(&before.channels).unwrap()
        );
        assert!(scripts.drain_outbox().is_empty());
    }
}

/// Catalogue edits add absent stock, cap requested amounts, floor removals and clear through both adapters.
#[tokio::test]
async fn operator_stock_commands_and_lua_share_catalogue_edits() {
    for source in templates() {
        let (_dir, config, mut world, _map, unit) = fixture(&source).await;
        economy_channel(&mut world);
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        for (command, call) in [
            (
                "addstuff Gold 60000",
                format!("btech.inventory.add(1,{},'Gold',60000)", unit.0),
            ),
            (
                "removestuff Gold 49999",
                format!("btech.inventory.remove(1,{},'Gold',49999)", unit.0),
            ),
            (
                "removestuff Gold 10",
                format!("btech.inventory.remove(1,{},'Gold',10)", unit.0),
            ),
            (
                "addstuff ML 2",
                format!("btech.inventory.add(1,{},'ML',2)", unit.0),
            ),
            (
                "addstuff ShoulderOrHip 3",
                format!("btech.inventory.add(1,{},'ShoulderOrHip',3)", unit.0),
            ),
        ] {
            let output = support::run_text(&native, &config, ObjectId(1), 1, command);
            assert!(
                output.contains("You add") || output.contains("You remove"),
                "{output}"
            );
            lua.eval_callback::<()>(&call).unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
        }
        assert_eq!(count(&lua.world(), unit, 528), 0);
        assert_eq!(count(&lua.world(), unit, 548), 3);
        assert_eq!(count(&lua.world(), unit, 394), 0);
        assert!(
            battle_inventory(&lua.world(), unit)
                .unwrap()
                .iter()
                .any(|row| row.part_id == BattleWeapon::MediumLaser.part_id() && row.quantity == 2)
        );
        assert!(
            lua.world().channels["MechEconInfo"].history[0]
                .message
                .contains("added 50000 Gold")
        );
        assert!(
            lua.world().channels["MechEconInfo"].history[2]
                .message
                .contains("removed 10 Gold")
        );
        let output = support::run_text(&native, &config, ObjectId(1), 1, "clearstuff");
        assert!(output.contains("Inventory cleaned!"), "{output}");
        lua.eval_callback::<()>(&format!("btech.inventory.clear(1,{})", unit.0))
            .unwrap();
        assert_eq!(native.world().btech, lua.world().btech);
        assert!(battle_inventory(&lua.world(), unit).unwrap().is_empty());
        assert!(
            lua.world().channels["MechEconInfo"]
                .history
                .last()
                .unwrap()
                .message
                .contains(&format!("#1 reset #{}'s stuff.", unit.0))
        );
        let saved = lua.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
    }
}

/// Authority, catalogue match limits and multi-row publication failures cannot partially change stock.
#[tokio::test]
async fn operator_stock_limits_and_batch_rollback_are_shared() {
    let (_dir, config, mut world, map, unit) =
        fixture(include_str!("../game/mechs/Kestrel.toml")).await;
    economy_channel(&mut world);
    let wizard = world.create(&config, "StockWizard".into(), Kind::Player);
    world
        .objects
        .get_mut(&wizard)
        .unwrap()
        .flags
        .insert(Flag::Wizard);
    let ordinary = world.create(&config, "Ordinary".into(), Kind::Player);
    set_battle_inventory_quantity(&mut world, ObjectId(1), map, i32::MAX, 1).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for (actor, pattern, quantity) in [
        (ordinary, "Gold", 1),
        (wizard, "*", 1),
        (ObjectId(1), "Gold", 0),
        (ObjectId(1), "Gold", -1),
        (ObjectId(1), "NoSuchPart", 1),
    ] {
        let before = scripts.world().btech.clone();
        assert!(
            change_battle_inventory_action(
                &scripts,
                &config,
                actor,
                unit,
                BattleInventoryChange::Add {
                    pattern: pattern.into(),
                    quantity
                }
            )
            .is_err()
        );
        assert_eq!(scripts.world().btech, before);
    }
    scripts
        .world_mut()
        .channels
        .get_mut("MechEconInfo")
        .unwrap()
        .messages = i64::MAX;
    let before = scripts.world().clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.inventory.add(1,{},'MediumLas?r',1)",
                unit.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before.btech);
    assert_eq!(
        serde_json::to_value(&scripts.world().channels).unwrap(),
        serde_json::to_value(&before.channels).unwrap()
    );
    assert!(scripts.drain_outbox().is_empty());
    scripts
        .world_mut()
        .channels
        .get_mut("MechEconInfo")
        .unwrap()
        .messages = 0;
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.inventory.clear(1,{}); error('abort clear')",
                map.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    scripts
        .eval_callback::<()>(&format!("btech.inventory.clear(1,{})", map.0))
        .unwrap();
    assert!(battle_inventory(&scripts.world(), map).unwrap().is_empty());
    // The GOD operator is exempt from the 20-type limit; use a bounded family
    // so this check does not depend on the entire catalogue fitting the output budget.
    let rows = change_battle_inventory_action(
        &scripts,
        &config,
        ObjectId(1),
        map,
        BattleInventoryChange::Add {
            pattern: "IS.*Laser".into(),
            quantity: 1,
        },
    )
    .unwrap();
    assert!(rows.len() > 5);
}

/// The scripted store function adjusts one catalogue match and preserves its signed-count contract.
#[tokio::test]
async fn scripted_add_stores_uses_first_match_signed_counts_and_atomic_logging() {
    for source in templates() {
        let (_dir, config, mut world, _map, unit) = fixture(&source).await;
        economy_channel(&mut world);
        set_battle_inventory_quantity(&mut world, ObjectId(1), unit, 528, 80000).unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let added: bool = scripts
            .eval_callback(&format!(
                "return btech.inventory.add_stores(1,{},'IS.MediumLas?r',2147483647)",
                unit.0
            ))
            .unwrap();
        assert!(added);
        let rows = battle_inventory(&scripts.world(), unit).unwrap().to_vec();
        let lasers: Vec<_> = rows
            .iter()
            .filter(|row| row.part_id == BattleWeapon::MediumLaser.part_id())
            .collect();
        assert_eq!(lasers.len(), 1);
        assert_eq!(lasers[0].quantity, 50000);
        assert!(
            add_battle_stores_action(&scripts, &config, ObjectId(1), unit, "Gold", -70000).unwrap()
        );
        assert_eq!(count(&scripts.world(), unit, 528), 10000);
        assert!(
            scripts.world().channels["MechEconInfo"]
                .history
                .last()
                .unwrap()
                .message
                .contains("added -70000 Gold")
        );
        assert!(
            add_battle_stores_action(&scripts, &config, ObjectId(1), unit, "Gold", i32::MIN)
                .unwrap()
        );
        assert_eq!(count(&scripts.world(), unit, 528), 0);
        scripts.drain_outbox();
        let before = scripts.world().clone();
        assert!(
            !scripts
                .eval_callback::<bool>(&format!(
                    "return btech.inventory.add_stores(1,{},'NoSuchPart',1)",
                    unit.0
                ))
                .unwrap()
        );
        assert!(
            scripts
                .eval_callback::<bool>(&format!(
                    "return btech.inventory.add_stores(1,{},'NoSuchPart',0)",
                    unit.0
                ))
                .unwrap()
        );
        assert_eq!(scripts.world().btech, before.btech);
        assert_eq!(
            serde_json::to_value(&scripts.world().channels).unwrap(),
            serde_json::to_value(&before.channels).unwrap()
        );
        assert!(scripts.drain_outbox().is_empty());
        persistence::save(&config.database(), &before)
            .await
            .unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, before.btech);
    }
}

/// Single-store edits protect permission, name bounds and stock against callback and channel failures.
#[tokio::test]
async fn scripted_add_stores_guards_and_failure_rollback() {
    let (_dir, config, mut world, _map, unit) =
        fixture(include_str!("../game/mechs/Kestrel.toml")).await;
    economy_channel(&mut world);
    let ordinary = world.create(&config, "Ordinary".into(), Kind::Player);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    assert!(!add_battle_stores_action(&scripts, &config, ObjectId(1), unit, "", 1).unwrap());
    assert!(add_battle_stores_action(&scripts, &config, ObjectId(1), unit, "", 0).unwrap());
    let before = scripts.world().clone();
    assert!(add_battle_stores_action(&scripts, &config, ordinary, unit, "Gold", 0).is_err());
    assert!(
        add_battle_stores_action(
            &scripts,
            &config,
            ObjectId(1),
            ObjectId(i64::MAX),
            "Gold",
            0
        )
        .is_err()
    );
    assert!(
        add_battle_stores_action(&scripts, &config, ObjectId(1), unit, &"x".repeat(2048), 0)
            .is_err()
    );
    assert!(
        add_battle_stores_action(&scripts, &config, ObjectId(1), unit, &"x".repeat(2047), 0)
            .unwrap()
    );
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.inventory.add_stores(1,{},'Gold',1); error('abort store')",
                unit.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before.btech);
    assert!(scripts.drain_outbox().is_empty());
    scripts
        .world_mut()
        .channels
        .get_mut("MechEconInfo")
        .unwrap()
        .messages = i64::MAX;
    let before = scripts.world().clone();
    let code:String=scripts.eval_callback(&format!("local ok,e=pcall(btech.inventory.add_stores,1,{},'Gold',1); assert(not ok); return e.code",unit.0)).unwrap();
    assert_eq!(code, "btech.operation.failed");
    assert_eq!(scripts.world().btech, before.btech);
    assert_eq!(
        serde_json::to_value(&scripts.world().channels).unwrap(),
        serde_json::to_value(&before.channels).unwrap()
    );
    assert!(scripts.drain_outbox().is_empty());
}

/// Native and Lua cleanup share stock selection, load reconciliation and persistence on every chassis.
#[tokio::test]
async fn inventory_cleanup_preserves_installed_units_and_stock() {
    for source in templates() {
        let (_dir, config, mut world, _, unit) = fixture(&source).await;
        for part in [406, 407, 408, 428, 432, 433, 443, 444] {
            set_battle_inventory_quantity(&mut world, ObjectId(1), unit, part, 2).unwrap();
        }
        set_battle_inventory_named(&mut world, ObjectId(1), unit, "Gold", 7).unwrap();
        set_battle_inventory_quantity(&mut world, ObjectId(1), unit, 535, 3).unwrap();
        let installed = battle_weapon_specifications(&world, unit, true).unwrap();
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let before = lua.world().btech.clone();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.inventory.fix(1,{}); error('abort cleanup')",
                unit.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, before);
        assert!(lua.drain_outbox().is_empty());
        let report: mlua::Table = lua
            .eval_callback(&format!("return btech.inventory.fix(1,{})", unit.0))
            .unwrap();
        assert_eq!(report.get::<usize>("original_entries").unwrap(), 10);
        assert_eq!(report.get::<usize>("new_entries").unwrap(), 2);
        assert_eq!(report.get::<u64>("items").unwrap(), 10);
        let output = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            "fixstuff ignored arguments",
        );
        assert!(
            output.contains("Original entries: 10. New entries: 2."),
            "{output}"
        );
        assert!(
            output.contains("Items in new: 10. Unique items in new: 2."),
            "{output}"
        );
        assert_eq!(native.world().btech, lua.world().btech);
        assert_eq!(
            battle_weapon_specifications(&lua.world(), unit, true).unwrap(),
            installed
        );
        let stock = battle_inventory(&lua.world(), unit).unwrap().to_vec();
        assert_eq!(
            stock
                .iter()
                .map(|row| (row.part_id, row.quantity))
                .collect::<Vec<_>>(),
            vec![(528, 7), (535, 3)]
        );
        let saved = lua.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
        let repeated =
            clean_battle_inventory(&mut lua.world_mut(), &config, ObjectId(1), unit).unwrap();
        assert_eq!(repeated.original_entries, repeated.new_entries);
        assert_eq!(lua.world().btech, saved.btech);
    }
}

/// Room inventories can retain unknown imported IDs until cleanup; permissions and output rollback still apply.
#[tokio::test]
async fn inventory_cleanup_unknown_stock_authority_and_publication_rollback() {
    let (dir, config, mut world, room, unit) =
        fixture(include_str!("../game/mechs/JR7-D.toml")).await;
    release_battle_pilot(&mut world, unit, ObjectId(1)).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(room);
    set_battle_inventory_quantity(&mut world, ObjectId(1), room, i32::MAX, 4).unwrap();
    set_battle_inventory_quantity(&mut world, ObjectId(1), room, 406, 2).unwrap();
    let ordinary = world.create(&config, "Ordinary operator".into(), Kind::Player);
    let before = world.btech.clone();
    assert!(clean_battle_inventory(&mut world, &config, ordinary, room).is_err());
    assert!(clean_battle_inventory(&mut world, &config, ObjectId(1), ObjectId(-1)).is_err());
    assert_eq!(world.btech, before);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let text = support::run_text(&scripts, &config, ObjectId(1), 1, "fixstuff/invalid");
    assert!(text.contains("takes no switches"), "{text}");
    assert_eq!(scripts.world().btech, before);
    let report = clean_battle_inventory(&mut world, &config, ObjectId(1), room).unwrap();
    assert_eq!(
        (report.original_entries, report.new_entries, report.items),
        (3, 1, 20)
    );
    let empty = world.create(&config, "Empty store".into(), Kind::Room);
    assert_eq!(
        clean_battle_inventory(&mut world, &config, ObjectId(1), empty)
            .unwrap()
            .new_entries,
        0
    );
    world.validate(&config).unwrap();
    // A rejected native summary must roll back cleanup, just as a late Lua callback error does.
    let path = dir.path().join("stompymux.toml");
    let mut table: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
    table
        .entry("runtime")
        .or_insert(toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .unwrap()
        .insert("output_message_limit".into(), 1.into());
    std::fs::write(&path, toml::to_string(&table).unwrap()).unwrap();
    let constrained = Config::load(dir.path()).unwrap();
    let scripts =
        Scripts::new(&constrained, Rc::new(RefCell::new(scripts.world().clone()))).unwrap();
    let action = commands::run(&scripts, &constrained, ObjectId(1), 1, "fixstuff").unwrap();
    assert!(
        matches!(action, CommandAction::Report(CommandReport::Reply(message)) if message.contains("output limit"))
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
}
