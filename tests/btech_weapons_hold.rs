//! Operator weapons hold: admission across chassis, saved state, and atomic rejected edits.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Edit isolated scenario facts without bypassing final world validation.
fn edit(world: &mut World, id: ObjectId, change: impl FnOnce(&mut serde_json::Value)) {
    world.btech.rewrite_unit_record(id, change).unwrap();
}

/// One running wizard-piloted unit in light woods, with deterministic owned dice.
async fn fixture(source: &str) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Firing range".into(), Kind::Room);
    let row = format!("{}0", Terrain::LightWoods.symbol()).repeat(3) + "\n";
    create_battle_map(
        &mut world,
        map,
        "range",
        MapAsset::from_cells(&format!("3 3\n{}", row.repeat(3))).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let id = world.create(&config, "Held unit".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    crate::support::templates::legal_unit(UnitTemplate::parse("test", source).unwrap())
        .create(&mut world, id)
        .unwrap();
    support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, id, map, 1, 2).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0, true);
    }
    edit(&mut world, id, |unit| {
        unit["dice"] = serde_json::to_value(Dice::seeded([42; 32])).unwrap()
    });
    world.validate(&config).unwrap();
    (dir, config, world, map, id)
}

/// Operator hold rejects native, Lua and Rust firing before parsing, across every chassis.
#[tokio::test]
async fn weapons_hold_controls_admission_and_saved_state() {
    let tracked = include_str!("../game/units/Demolisher.toml");
    let chassis = [
        include_str!("../game/units/JR7-D.toml").to_owned(),
        include_str!("../game/units/GOL-1H.toml").to_owned(),
        tracked.to_owned(),
        tracked.replace("movement = \"track\"", "movement = \"wheel\""),
        crate::support::templates::hovercraft(tracked),
        tracked
            .replace("movement = \"track\"", "movement = \"none\"")
            .replace("walk_mp = 3", "walk_mp = 0"),
        include_str!("../game/units/Kestrel.toml").to_owned(),
    ];
    for source in chassis {
        let (_dir, config, mut world, _, id) = fixture(&source).await;
        edit_battle_tic(&mut world, id, ObjectId(1), 0, TicEdit::Add(vec![0])).unwrap();
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        assert!(!weapons_hold(&world, id).unwrap());
        let text = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("@btech unit-weapons-hold #{}=on", id.0),
        );
        assert!(text.contains("weapons hold enabled"), "{text}");
        let call = format!("btech.unit.weapons_hold({},true)", id.0);
        assert!(
            lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                .is_err()
        );
        assert_eq!(lua.world().btech, world.btech);
        assert!(
            lua.eval_callback::<bool>(&format!("return {call}"))
                .unwrap()
        );
        assert_eq!(native.world().btech, lua.world().btech);
        assert!(
            lua.eval_callback::<bool>(&format!(
                "return btech.unit.weapons_hold({}) and btech.unit.state({}).weapons_hold",
                id.0, id.0
            ))
            .unwrap()
        );
        let held = native.world().btech.clone();
        let ready = support::run_text(&native, &config, ObjectId(1), 1, "weapons");
        for command in [
            "fire",
            "fire nonsense",
            "fire 99999 invalid",
            "fire 0 1 2 3",
            "firetic",
            "firetic nonsense",
            "firetic 99999 invalid",
            "firetic 0 invalid",
        ] {
            let text = support::run_text(&native, &config, ObjectId(1), 1, command);
            assert!(
                text.contains("Currently in weapons hold."),
                "{command}: {text}"
            );
            assert_eq!(native.world().btech, held);
        }
        for call in [
            format!("btech.unit.fire({},1,999999,999999)", id.0),
            format!("btech.unit.tic_fire({},1,{{}},999999)", id.0),
        ] {
            let error = lua.eval_callback::<()>(&call).unwrap_err();
            assert!(
                error.to_string().contains("Currently in weapons hold."),
                "{error}"
            );
            assert_eq!(lua.world().btech, held);
            assert!(lua.drain_outbox().is_empty());
        }
        let error = fire_battle_tics(&native, &config, id, ObjectId(1), vec![], None).unwrap_err();
        assert!(error.to_string().contains("Currently in weapons hold."));
        assert_eq!(native.world().btech, held);
        let status = support::run_text(&native, &config, ObjectId(1), 1, "status info");
        assert!(status.contains("WEAPONS HOLD"), "{status}");
        let snapshot = native.world().clone();
        persistence::save(&config.database(), &snapshot)
            .await
            .unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, held);
        stop_battle_unit(&mut restored, id, ObjectId(1)).unwrap();
        assert!(weapons_hold(&restored, id).unwrap());
        set_battle_weapons_hold(&mut restored, id, false).unwrap();
        assert!(!weapons_hold(&restored, id).unwrap());
        let text = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("@btech unit-weapons-hold #{}=off", id.0),
        );
        assert!(text.contains("weapons hold disabled"), "{text}");
        assert_eq!(
            ready,
            support::run_text(&native, &config, ObjectId(1), 1, "weapons")
        );
        assert_eq!(native.world().btech, world.btech);
        let text = support::run_text(&native, &config, ObjectId(1), 1, "fire 999999 invalid");
        assert!(!text.contains("Currently in weapons hold."), "{text}");
    }
}

/// Cockpit authority and startup precede hold, while administrative edits reject invalid subjects.
#[tokio::test]
async fn weapons_hold_authority_and_rejected_edits_are_atomic() {
    for source in [
        include_str!("../game/units/JR7-D.toml"),
        include_str!("../game/units/Demolisher.toml"),
        include_str!("../game/units/Kestrel.toml"),
    ] {
        let (_dir, config, mut world, map, id) = fixture(source).await;
        world
            .objects
            .get_mut(&ObjectId(2))
            .unwrap()
            .flags
            .remove(Flag::Wizard);
        world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(id);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let command = format!("@btech unit-weapons-hold #{}=on", id.0);
        support::run_text(&scripts, &config, ObjectId(2), 2, &command);
        assert_eq!(scripts.world().btech, world.btech);
        let text = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("@btech unit-weapons-hold #{}=maybe", id.0),
        );
        assert!(text.contains("Usage:"), "{text}");
        assert_eq!(scripts.world().btech, world.btech);
        for subject in [map, ObjectId(999999)] {
            assert!(set_battle_weapons_hold(&mut scripts.world_mut(), subject, true).is_err());
            assert_eq!(scripts.world().btech, world.btech);
        }
        let mut going = world.clone();
        going
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::Going);
        assert!(set_battle_weapons_hold(&mut going, id, true).is_err());
        assert_eq!(going.btech, world.btech);
        set_battle_weapons_hold(&mut scripts.world_mut(), id, true).unwrap();
        let held = scripts.world().btech.clone();
        for command in ["fire nonsense", "firetic nonsense"] {
            let text = support::run_text(&scripts, &config, ObjectId(2), 2, command);
            assert!(!text.contains("Currently in weapons hold."), "{text}");
            assert_eq!(scripts.world().btech, held);
        }
        stop_battle_unit(&mut scripts.world_mut(), id, ObjectId(1)).unwrap();
        assign_battle_pilot(&mut scripts.world_mut(), id, ObjectId(1)).unwrap();
        support::seed_object_dice(
            &mut scripts.world_mut(),
            ObjectId(1),
            support::FIXTURE_DICE_SEED,
        );
        let stopped = scripts.world().btech.clone();
        for command in ["fire nonsense", "firetic nonsense"] {
            let text = support::run_text(&scripts, &config, ObjectId(1), 1, command);
            assert!(text.contains("Unit must be started"), "{text}");
            assert!(!text.contains("Currently in weapons hold."));
            assert_eq!(scripts.world().btech, stopped);
        }
    }
}
