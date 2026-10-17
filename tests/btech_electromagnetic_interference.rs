//! Electromagnetic interference (Tactical Operations: Advanced Rules p. 53): a map condition
//! set through the map flags, from `@SETMAP`, Lua or a map file, that adds two to every ranged
//! weapon attack, takes two from every Cluster Hits Table roll, renders active probes useless
//! and doubles the reach of ECM suites.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Switch electromagnetic interference over `map` on or off, leaving its other flags alone.
fn interference(world: &mut World, map: ObjectId, enabled: bool) {
    world
        .btech
        .rewrite_map_record(map, |record| {
            let flags = record["flags"].as_i64().unwrap();
            record["flags"] = MapFlag::ElectromagneticInterference
                .apply(flags, enabled)
                .into();
        })
        .unwrap();
}

/// The battlefield a placed unit stands on.
fn map_of(world: &World, id: ObjectId) -> ObjectId {
    world.btech.unit(id).unwrap().position().unwrap().map
}

/// Direct-fire rules for these shots.
fn shot_rules() -> ShotRules {
    ShotRules {
        tsm_tow_bonus: true,
        vehicle_impact: VehicleImpactRules::STANDARD,
        aim: AimRules::STANDARD,
        extended_gunnery: false,
        extended_piloting: false,
        target_toughness: false,
    }
}

/// `@SETMAP flags` and Lua's `btech.map.set_flag` switch the condition like any map flag,
/// `@VIEWMAP` and `btech.map.has_flag` report it, and it survives a restart.
#[tokio::test]
async fn interference_is_a_map_flag_that_survives_restart() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Interference field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "interference",
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let lua_has = |scripts: &Scripts| -> bool {
        scripts
            .eval_callback(&format!(
                "return btech.map.has_flag({}, btech.map.flags.ELECTROMAGNETIC_INTERFERENCE)",
                map.0
            ))
            .unwrap()
    };
    assert!(!lua_has(&scripts));
    let output = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        "@setmap flags dark electromagnetic_interference",
    );
    assert!(output.is_empty(), "{output}");
    assert!(
        scripts.world().btech.maps()[&map].electromagnetic_interference()
            && scripts.world().btech.maps()[&map].has_flag(MapFlag::Dark)
    );
    assert!(lua_has(&scripts));
    let report =
        view_battle_map_fields_action(&scripts, &config, ObjectId(1), map, "flags").unwrap();
    assert_eq!(
        report.fields[0].value.as_deref(),
        Some("dark electromagnetic_interference")
    );
    let set = |enabled: bool| {
        scripts
            .eval_callback::<()>(&format!(
                "btech.map.set_flag(1, {}, btech.map.flags.ELECTROMAGNETIC_INTERFERENCE, {enabled})",
                map.0
            ))
            .unwrap()
    };
    set(false);
    assert!(!lua_has(&scripts));
    assert!(scripts.world().btech.maps()[&map].has_flag(MapFlag::Dark));
    set(true);
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert!(restored.btech.maps()[&map].electromagnetic_interference());
    assert_eq!(restored.btech, saved.btech);
}

/// A map file declares the condition among its flags; loading it puts the condition on the
/// live map and an export writes it back by name.
#[tokio::test]
async fn a_map_file_declares_interference() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Loaded field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "loaded",
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let root = config.path(&config.database.map_database);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("storm.map.toml"),
        "flags = [\"electromagnetic_interference\"]\nterrain = '''\n..\n'''\nlevel = '''\n00\n'''\n",
    )
    .unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    assert!(!scripts.world().btech.maps()[&map].electromagnetic_interference());
    load_battle_map_action(&scripts, &config, ObjectId(1), map, "storm.map").unwrap();
    let world = scripts.world();
    let loaded = &world.btech.maps()[&map];
    assert!(loaded.electromagnetic_interference());
    assert_eq!(loaded.width, 2);
    let export = loaded.export_asset().unwrap();
    assert!(
        export.contains("flags = [\"electromagnetic_interference\"]"),
        "{export}"
    );
}

/// Every ranged weapon attack on the map takes +2 as its own term of the to-hit breakdown, for
/// every chassis and at units and hexes alike, while a punch takes nothing.
#[tokio::test]
async fn interference_adds_two_to_ranged_attacks_and_nothing_to_punches() {
    for source in firing::templates() {
        let (_dir, _config, mut world, shooter, target, index) =
            firing::fixture_with_target(&source, None, include_str!("../game/units/JR7-D.toml"))
                .await;
        let map = map_of(&world, shooter);
        let aim = |world: &World| {
            battle_aim_modifiers(world, shooter, target, index, 4, AimRules::STANDARD).unwrap()
        };
        let hex = HexCoordinate { x: 0, y: 9 };
        let hex_aim = |world: &World| battle_hex_aim_modifiers(world, shooter, hex, index, 4);
        let clear = aim(&world);
        let clear_hex = hex_aim(&world);
        assert_eq!(clear.interference, 0);
        interference(&mut world, map, true);
        let interfered = aim(&world);
        assert_eq!(interfered.interference, 2);
        assert!(clear.subtotal().is_some());
        assert_eq!(interfered.subtotal(), clear.subtotal().map(|n| n + 2));
        assert_eq!(
            AimModifiers {
                interference: 0,
                ..interfered
            },
            clear,
            "nothing else changes"
        );
        let Ok(clear_hex) = clear_hex else {
            continue;
        };
        let interfered_hex = hex_aim(&world).unwrap();
        assert_eq!(interfered_hex.modifiers.interference, 2);
        assert_eq!(
            interfered_hex.subtotal(),
            clear_hex.subtotal().map(|n| n + 2)
        );
    }
    let (_dir, _config, mut world, shooter, target, _) = firing::fixture_with_target(
        include_str!("../game/units/JR7-D.toml"),
        None,
        include_str!("../game/units/JR7-D.toml"),
    )
    .await;
    // A physical attack reaches a unit sharing the attacker's hex.
    let map = map_of(&world, shooter);
    world.btech.set_unit_power(target, Power::Off).unwrap();
    place_battle_unit(&mut world, target, map, 0, 11).unwrap();
    world.btech.set_unit_power(target, Power::Running).unwrap();
    refresh_battle_contacts(&mut world, &[shooter]).unwrap();
    let rules = PhysicalRules {
        fall: MovementRules::STANDARD.fall,
    };
    let punch = |world: &World| {
        battle_punch_profile(world, shooter, ObjectId(1), target, Arm::Right, rules)
            .unwrap()
            .target_number
    };
    let clear = punch(&world);
    interference(&mut world, map, true);
    assert_eq!(punch(&world), clear, "physical attacks are not ranged");
}

/// A Jenner's SRM-4 fires under interference at a to-hit number two higher; the reported
/// Cluster Hits roll is the dice's, and the missiles that land follow the row two lower, never
/// below 2.
#[tokio::test]
async fn interference_lowers_cluster_rolls_to_the_floor() {
    let (_dir, config, base, shooter, target, _) = firing::fixture_with_target(
        include_str!("../game/units/JR7-D.toml"),
        None,
        include_str!("../game/units/JR7-D.toml"),
    )
    .await;
    let index = base.btech.constructed_units()[&shooter]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == Weapon::Srm4)
        .unwrap();
    let map = map_of(&base, shooter);
    let salvo = |report: &MechShotReport| match &report.salvo {
        Some(TargetSalvo::Mech(salvo)) => salvo.clone(),
        other => panic!("a Mech salvo: {other:?}"),
    };
    // The shooter's first roll is 12, a hit at either to-hit number; the target's dice roll
    // the Cluster Hits Table.
    let hit = (0..=255)
        .find(|value| Dice::seeded([*value; 32]).two_d6() == 12)
        .unwrap();
    let (mut floored, mut lowered) = (false, false);
    for value in 0..=63u8 {
        let mut clear = base.clone();
        clear
            .btech
            .set_unit_dice(shooter, Dice::seeded([hit; 32]))
            .unwrap();
        clear
            .btech
            .set_unit_dice(target, Dice::seeded([value; 32]))
            .unwrap();
        let mut interfered = clear.clone();
        interference(&mut interfered, map, true);
        let fire = |world: &mut World| {
            resolve_battle_shot(world, shooter, ObjectId(1), target, index, shot_rules()).unwrap()
        };
        let plain = fire(&mut clear);
        let emi = fire(&mut interfered);
        assert_eq!(emi.roll, plain.roll);
        assert_eq!(emi.target_number, plain.target_number.map(|n| n + 2));
        assert!(plain.hit && emi.hit && !plain.glancing && !emi.glancing);
        let (plain, emi) = (salvo(&plain), salvo(&emi));
        let roll = plain.cluster_roll.unwrap();
        assert_eq!(
            emi.cluster_roll,
            Some(roll),
            "the reported roll is the dice's"
        );
        assert_eq!(
            plain.groups.len(),
            usize::from(Weapon::Srm4.missile_hits(roll).unwrap())
        );
        let row = roll.saturating_sub(2).max(2);
        assert_eq!(
            emi.groups.len(),
            usize::from(Weapon::Srm4.missile_hits(row).unwrap()),
            "roll {roll}"
        );
        floored |= roll <= 4;
        lowered |= roll >= 5;
        clear.validate(&config).unwrap();
        interfered.validate(&config).unwrap();
    }
    assert!(floored && lowered, "both a floored and a lowered roll");
}

/// A running Demolisher with its Guardian ECM switched on at the head of a sixteen-hex lane and
/// a hostile, parked Jenner in the lane.
async fn ecm_lane() -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "ECM lane".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "ecm",
        MapAsset::from_cells(&format!("1 16\n{}", ".0\n".repeat(16))).unwrap(),
    )
    .unwrap();
    let emitter = world.create(&config, "Emitter".into(), Kind::Thing);
    let target = world.create(&config, "Target".into(), Kind::Thing);
    for id in [emitter, target] {
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    }
    let mut template = support::templates::fitted(
        VehicleTemplate::parse("Demolisher", include_str!("../game/units/Demolisher.toml"))
            .unwrap(),
    );
    template
        .sections
        .get_mut(&VehicleSection::Front)
        .unwrap()
        .criticals
        .insert(
            0,
            CriticalDefinition {
                equipment: "Ecm".into(),
                data: "-".into(),
                modes: vec![],
            },
        );
    create_battle_vehicle(&mut world, emitter, support::templates::fitted(template)).unwrap();
    create_battle_unit(
        &mut world,
        target,
        MechTemplate::parse("JR7-D", include_str!("../game/units/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, emitter, map, 0, 0).unwrap();
    place_battle_unit(&mut world, target, map, 0, 5).unwrap();
    set_battle_unit_signature(
        &mut world,
        target,
        UnitSignature {
            team: 2,
            ..Default::default()
        },
    )
    .unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(emitter);
    assign_battle_pilot(&mut world, emitter, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, emitter, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0, true);
    }
    toggle_battle_electronics(
        &mut world,
        emitter,
        ObjectId(1),
        ElectronicSuite::Guardian,
        ElectronicMode::Ecm,
    )
    .unwrap();
    (dir, config, world, emitter, target)
}

/// A Guardian suite disturbs enemies within six hexes, and within twelve under
/// electromagnetic interference; the doubled bubble survives a restart.
#[tokio::test]
async fn interference_doubles_the_ecm_bubble() {
    let (_dir, config, mut world, emitter, target) = ecm_lane().await;
    let map = map_of(&world, emitter);
    assert_eq!(battle_ecm_range(false), ECM_RANGE);
    assert_eq!(battle_ecm_range(true), 2.0 * ECM_RANGE);
    let disturbed = |world: &World| battle_electronic_field(world, target).unwrap().disturbed;
    for (y, ordinary, interfered) in [
        (5, true, true),
        (9, false, true),
        (11, false, true),
        (14, false, false),
    ] {
        place_battle_unit(&mut world, target, map, 0, y).unwrap();
        interference(&mut world, map, false);
        assert_eq!(disturbed(&world), ordinary, "{y} hexes");
        interference(&mut world, map, true);
        assert_eq!(
            disturbed(&world),
            interfered,
            "{y} hexes under interference"
        );
    }
    place_battle_unit(&mut world, target, map, 0, 9).unwrap();
    let notices = refresh_battle_electronic_fields(&mut world).unwrap();
    assert!(
        notices.iter().any(|notice| notice.unit == target
            && notice.text == "Half your screens are suddenly filled with static!"),
        "{notices:?}"
    );
    assert!(
        world.btech.constructed_units()[&target]
            .electronics()
            .field
            .blocks_outgoing_guidance()
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert!(disturbed(&restored));
    interference(&mut world, map, false);
    let notices = refresh_battle_electronic_fields(&mut world).unwrap();
    assert!(
        notices.iter().any(|notice| notice.unit == target
            && notice.text == "All your systems are back to normal again!"),
        "{notices:?}"
    );
}
