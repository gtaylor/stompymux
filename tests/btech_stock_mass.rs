//! Shared stock names, integer cargo mass, propulsion limits and native/Lua transactions.
use crate::support;
use std::{cell::RefCell, collections::BTreeSet, rc::Rc};
use stompymux_rs::*;

/// Every supported movement class uses the same inventory and load projection.
fn templates() -> Vec<String> {
    let vehicle = include_str!("../game/mechs/Demolisher");
    vec![
        include_str!("../game/mechs/JR7-D").into(),
        include_str!("../game/mechs/GOL-1H").into(),
        vehicle.into(),
        vehicle.replace("Tracked", "Wheeled"),
        vehicle.replace("Tracked", "Hover"),
        vehicle.replace("Tracked", "None"),
        include_str!("../game/mechs/Kestrel").into(),
    ]
}

/// Construct and place one carrier, optionally enabling cargo technology without changing its stock.
async fn fixture(
    source: &str,
    cargo: bool,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Stock yard".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "stock",
        BattleMapAsset::parse(&format!("10 10\n{}", (".0".repeat(10) + "\n").repeat(10))).unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Carrier".into(), Kind::Thing);
    let mut template = BattleUnitTemplate::parse(source).unwrap();
    let attributes = match &mut template {
        BattleUnitTemplate::Mech(unit) => &mut unit.attributes,
        BattleUnitTemplate::Vehicle(unit) => &mut unit.attributes,
    };
    let mut flags = attributes
        .get("specials")
        .map_or("", String::as_str)
        .split_ascii_whitespace()
        .filter(|flag| *flag != "-" && !flag.eq_ignore_ascii_case("CargoTech"))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if cargo {
        flags.push("CargoTech".into());
    }
    attributes.insert(
        "specials".into(),
        if flags.is_empty() {
            "-".into()
        } else {
            flags.join(" ")
        },
    );
    template.create(&mut world, id).unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    (dir, config, world, map, id)
}

/// Explicit database IDs round-trip independently of declaration order and keep existing weapon facts.
#[test]
fn stock_identities_and_reference_mass_facts_are_stable() {
    let mut ids = BTreeSet::new();
    for &weapon in BattleWeapon::ALL {
        assert!(ids.insert(weapon.part_id()));
        assert_eq!(BattleWeapon::from_part_id(weapon.part_id()), Some(weapon));
        let part = BattlePart::from_id(weapon.part_id()).unwrap();
        assert_eq!(part.name, weapon.name());
        assert_eq!(part.mass, weapon.mass());
        assert_eq!(
            BattlePart::parse(&part.name.to_ascii_lowercase()).unwrap(),
            part
        );
        let ammunition = BattlePart::from_id(weapon.part_id() + 192).unwrap();
        assert_eq!(ammunition.kind, BattlePartKind::Ammunition);
        assert_eq!(ammunition.mass, 1024);
    }
    for (id, name, mass) in [
        (1, "CL.A-Pod", 512),
        (385, "Bomb_10_Inferno", 1020),
        (403, "HeatSink", 204),
        (442, "Light_BAP", 512),
        (443, "SplitCrit_Left", 0),
        (445, "Hardpoint", 0),
        (446, "Retractable_Blade", 1024),
        (449, "Wrecking_Ball", 819),
        (453, "Large_Vibroblade", 1792),
        (528, "Gold", 204),
        (529, "Natural_Extracts", 51),
        (534, "Ore", 51),
        (537, "Medical_Supplies", 8),
        (669, "SearchLight", 204),
    ] {
        let part = BattlePart::from_id(id).unwrap();
        assert_eq!((part.name.as_str(), part.mass), (name, mass));
        assert_eq!(BattlePart::parse(name).unwrap().part_id, id);
    }
    let mut names = BTreeSet::new();
    let parts: Vec<_> = (1..=669).filter_map(BattlePart::from_id).collect();
    assert_eq!(parts.len(), 591);
    let mut duplicates = BTreeSet::new();
    for part in parts {
        if !names.insert(part.name.to_ascii_lowercase()) {
            duplicates.insert(part.name.to_ascii_lowercase());
        }
    }
    assert_eq!(
        duplicates,
        BTreeSet::from(["case-ii".into(), "steel".into()])
    );
    for name in duplicates {
        assert!(
            BattlePart::parse(&name)
                .unwrap_err()
                .to_string()
                .contains("Ambiguous")
        );
    }
    for id in [-1, 0, 192, 384, 511, 670, i32::MAX] {
        assert!(BattlePart::from_id(id).is_none());
    }
    assert!(BattlePart::parse("Gold*").is_err());
}

/// Round once after summing stock and applying the chassis cargo factor; saved replay retains stock.
#[tokio::test]
async fn cargo_mass_and_throttle_are_shared_across_every_chassis() {
    for source in templates() {
        for cargo in [false, true] {
            let (_dir, config, mut world, _, id) = fixture(&source, cargo).await;
            let mech = world.btech.constructed_units().contains_key(&id);
            let baseline = battle_throttle_maximum(&world, id, true).unwrap();
            for (name, count) in [("Gold", 2), ("Natural_Extracts", 1), ("Ore", 1)] {
                set_battle_inventory_named(&mut world, ObjectId(1), id, name, 0, count).unwrap();
            }
            assert_eq!(battle_inventory_mass(&world, id).unwrap(), 510);
            let expected = match (mech, cargo) {
                (true, false) => 1020,
                (true, true) | (false, false) => 510,
                (false, true) => 255,
            };
            assert_eq!(
                battle_unit_load(&world, id, true).unwrap().carried_mass,
                expected
            );
            assert!(battle_throttle_maximum(&world, id, true).unwrap() <= baseline);
            set_battle_inventory_named(&mut world, ObjectId(1), id, "Bomb_10_Inferno", 0, 1)
                .unwrap();
            assert_eq!(battle_inventory_mass(&world, id).unwrap(), 4590);
            let before = world.btech.clone();
            assert!(
                set_battle_inventory_quantity(&mut world, ObjectId(1), id, i32::MAX, 0, 1).is_err()
            );
            assert_eq!(world.btech, before);
            world.validate(&config).unwrap();
            persistence::save(&config.database(), &world).await.unwrap();
            let loaded = persistence::load(&config.database()).await.unwrap();
            assert_eq!(loaded.btech, world.btech);
            assert_eq!(
                battle_throttle_maximum(&loaded, id, true).unwrap(),
                battle_throttle_maximum(&world, id, true).unwrap()
            );
            set_battle_inventory_named(&mut world, ObjectId(1), id, "Gold", 0, i32::MAX).unwrap();
            assert_eq!(battle_throttle_maximum(&world, id, true).unwrap(), 0.0);
            for name in ["Gold", "Natural_Extracts", "Ore", "Bomb_10_Inferno"] {
                set_battle_inventory_named(&mut world, ObjectId(1), id, name, 0, 0).unwrap();
            }
            assert_eq!(battle_throttle_maximum(&world, id, true).unwrap(), baseline);
            set_battle_inventory_named(&mut world, ObjectId(1), id, "SplitCrit_Left", 0, i32::MAX)
                .unwrap();
            assert_eq!(battle_inventory_mass(&world, id).unwrap(), 0);
            assert_eq!(battle_throttle_maximum(&world, id, true).unwrap(), baseline);
        }
    }
}

/// Named stock corrections share native and Lua edits, detached lookup and rollback.
#[tokio::test]
async fn named_stock_controls_and_inspection_agree() {
    let (_dir, config, world, _, id) = fixture(include_str!("../game/mechs/JR7-D"), false).await;
    let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let command = format!("@btech inventory-set #{} Gold 0 2", id.0);
    let callback = format!("btech.inventory.set_named(1,{},'gOlD',0,2)", id.0);
    let before = lua.world().btech.clone();
    assert!(
        lua.eval_callback::<()>(&format!("{callback}; error('abort')"))
            .is_err()
    );
    assert_eq!(lua.world().btech, before);
    let output = support::run_text(&native, &config, ObjectId(1), 1, &command);
    assert!(output.contains("quantity 2."), "{output}");
    lua.eval_callback::<()>(&callback).unwrap();
    assert_eq!(native.world().btech, lua.world().btech);
    let part: mlua::Table = lua
        .eval_callback("return btech.inventory.part('gold')")
        .unwrap();
    assert_eq!(part.get::<i32>("part_id").unwrap(), 528);
    part.set("mass", 999).unwrap();
    let part: mlua::Table = lua
        .eval_callback("return btech.inventory.part(528)")
        .unwrap();
    assert_eq!(part.get::<u32>("mass").unwrap(), 204);
    assert_eq!(
        lua.eval_callback::<u64>(&format!("return btech.inventory.mass({})", id.0))
            .unwrap(),
        408
    );
    let before = lua.world().btech.clone();
    for code in [
        format!("btech.inventory.set_named(4,{},'Gold',0,3)", id.0),
        format!("btech.inventory.set_named(1,{},'Unknown',0,3)", id.0),
    ] {
        assert!(lua.eval_callback::<()>(&code).is_err());
        assert_eq!(lua.world().btech, before);
    }
    let output = support::run_text(
        &native,
        &config,
        ObjectId(1),
        1,
        &format!("@btech inventory #{}", id.0),
    );
    assert!(output.contains("(Gold)"), "{output}");
}

/// Cargo composes with towing, and heartbeat movement clamps to the same loaded throttle ceiling.
#[tokio::test]
async fn carried_stock_affects_live_movement_and_adds_to_tow_load() {
    for source in templates() {
        let (_dir, config, mut world, map, id) = fixture(&source, false).await;
        let unloaded = battle_throttle_maximum(&world, id, true).unwrap();
        let target = world.create(&config, "Tow target".into(), Kind::Thing);
        BattleUnitTemplate::parse(include_str!("../game/mechs/JR7-D"))
            .unwrap()
            .create(&mut world, target)
            .unwrap();
        place_battle_unit(&mut world, target, map, 0, 0).unwrap();
        set_battle_tow(&mut world, id, Some(target)).unwrap();
        let empty = battle_unit_load(&world, id, true).unwrap().carried_mass;
        set_battle_inventory_named(&mut world, ObjectId(1), id, "Gold", 0, 50).unwrap();
        let mech = world.btech.constructed_units().contains_key(&id);
        assert_eq!(
            battle_unit_load(&world, id, true).unwrap().carried_mass - empty,
            if mech { 20400 } else { 10200 }
        );
        set_battle_tow(&mut world, id, None).unwrap();
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
        assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
        let maximum = battle_throttle_maximum(&world, id, true).unwrap();
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        let unit = &mut encoded[if mech { "constructed" } else { "vehicles" }][id.0.to_string()];
        unit["motion"]["heading"] = 90.0.into();
        unit["motion"]["desired_heading"] = 90.0.into();
        unit["motion"]["speed"] = unloaded.into();
        unit["motion"]["desired_speed"] = unloaded.into();
        if !unit["vtol_flight"].is_null() {
            unit["vtol_flight"]["phase"] = serde_json::json!({"kind":"airborne"});
            unit["vtol_flight"]["altitude"] = 20.0.into();
        }
        world.btech = serde_json::from_value(encoded).unwrap();
        advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
        let actual = if mech {
            world.btech.constructed_units()[&id].motion().unwrap()
        } else {
            world.btech.vehicles()[&id].motion().unwrap()
        };
        assert!(
            actual.speed.abs() <= maximum + 1e-6,
            "{source}: {actual:?} maximum {maximum}"
        );
        assert!(actual.desired_speed.abs() <= maximum + 1e-6);
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            advance_battle_motion(&mut replay, BattleMovementRules::STANDARD).unwrap(),
            advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap()
        );
        assert_eq!(world.btech, replay.btech);
    }
}

/// Constructing a stocked object checks its cargo before registering either anatomy.
#[tokio::test]
async fn construction_rejects_unknown_stock_before_mutation() {
    for source in templates() {
        let (_dir, config, mut world) = support::isolated_world().await;
        let id = world.create(&config, "Stocked shell".into(), Kind::Thing);
        set_battle_inventory_quantity(&mut world, ObjectId(1), id, 192, 0, 1).unwrap();
        let before = world.btech.clone();
        assert!(
            BattleUnitTemplate::parse(&source)
                .unwrap()
                .create(&mut world, id)
                .is_err()
        );
        assert_eq!(world.btech, before);
        set_battle_inventory_quantity(&mut world, ObjectId(1), id, 192, 0, 0).unwrap();
        set_battle_inventory_named(&mut world, ObjectId(1), id, "Gold", 0, 1).unwrap();
        BattleUnitTemplate::parse(&source)
            .unwrap()
            .create(&mut world, id)
            .unwrap();
        assert_eq!(battle_inventory_mass(&world, id).unwrap(), 204);
        world.validate(&config).unwrap();
    }
}
