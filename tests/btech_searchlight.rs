//! Searchlight switch persistence, command transactions, live geometry and damage dice.
use crate::support;
use stompymux_rs::*;

/// Build a running lamp carrier and an unpowered target on open terrain.
async fn fixture() -> (
    tempfile::TempDir,
    Config,
    World,
    ObjectId,
    ObjectId,
    ObjectId,
) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Lamp field".into(), Kind::Room);
    let terrain = format!("5 40\n{}", ".0.0.0.0.0\n".repeat(40));
    create_battle_map(
        &mut world,
        map,
        "lamp.map",
        BattleMapAsset::from_cells(&terrain).unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for name in ["Lamp", "Target"] {
        let id = world.create(&config, name.into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        let mut definition =
            BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml"))
                .unwrap();
        definition
            .attributes
            .insert("specials".into(), "FlipArms Searchlight".into());
        create_battle_unit(&mut world, id, definition).unwrap();
        place_battle_unit(&mut world, id, map, 2, if ids.is_empty() { 35 } else { 34 }).unwrap();
        ids.push(id);
    }
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(ids[0]);
    assign_battle_pilot(&mut world, ids[0], ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, ids[0], ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    (dir, config, world, ids[0], ids[1], map)
}

/// Change one persisted field to exercise ownership validation and deterministic scenarios.
fn field(world: &mut World, id: ObjectId, key: &str, value: serde_json::Value) {
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record[key] = value;
        })
        .unwrap();
}

#[tokio::test]
async fn switches_resume_after_restart_and_commands_rollback() {
    let (_dir, config, mut world, lamp, _, _) = fixture().await;
    assert!(toggle_battle_searchlight(&mut world, lamp, ObjectId(2)).is_err());
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert!(
        scripts
            .eval_callback::<()>(&format!("btech.unit.slite({},1); error('abort')", lamp.0))
            .is_err()
    );
    assert_eq!(scripts.world().btech, world.btech);
    assert!(scripts.drain_outbox().is_empty());
    support::run_text(&scripts, &config, ObjectId(1), 1, "slite");
    assert_eq!(
        scripts.world().btech.constructed_units()[&lamp]
            .searchlight()
            .remaining,
        5
    );
    toggle_battle_searchlight(&mut world, lamp, ObjectId(1)).unwrap();
    assert_eq!(scripts.world().btech, world.btech);
    for _ in 0..2 {
        assert!(advance_battle_searchlights(&mut world).is_empty());
    }
    assert!(
        toggle_battle_searchlight(&mut world, lamp, ObjectId(1))
            .unwrap()
            .text
            .contains("already")
    );
    assert_eq!(
        world.btech.constructed_units()[&lamp]
            .searchlight()
            .remaining,
        3
    );
    persistence::save(&config.database(), &world).await.unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    for _ in 0..2 {
        assert!(advance_battle_searchlights(&mut world).is_empty());
    }
    assert!(!world.btech.constructed_units()[&lamp].searchlight().on);
    assert!(
        advance_battle_searchlights(&mut world)[0]
            .text
            .contains("full power")
    );
    toggle_battle_searchlight(&mut world, lamp, ObjectId(1)).unwrap();
    stop_battle_unit(
        &mut world,
        lamp,
        ObjectId(1),
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    for _ in 0..5 {
        assert!(advance_battle_searchlights(&mut world).is_empty());
    }
    assert!(!world.btech.constructed_units()[&lamp].searchlight().on);
    assert_eq!(
        world.btech.constructed_units()[&lamp]
            .searchlight()
            .remaining,
        0
    );
    for invalid in [
        serde_json::json!({"on":false,"destroyed":false,"remaining":6}),
        serde_json::json!({"on":true,"destroyed":true,"remaining":0}),
    ] {
        let mut corrupt = world.clone();
        field(&mut corrupt, lamp, "searchlight", invalid);
        assert!(corrupt.validate(&config).is_err());
    }
}

#[tokio::test]
async fn illumination_tracks_geometry_range_and_live_objects() {
    let (_dir, config, mut world, lamp, target, map) = fixture().await;
    assert!(!battle_unit_illuminated(&world, target));
    toggle_battle_searchlight(&mut world, lamp, ObjectId(1)).unwrap();
    for _ in 0..5 {
        advance_battle_searchlights(&mut world);
    }
    assert!(battle_unit_illuminated(&world, lamp));
    assert!(battle_unit_illuminated(&world, target));
    place_battle_unit(&mut world, target, map, 2, 36).unwrap();
    assert!(!battle_unit_illuminated(&world, target));
    place_battle_unit(&mut world, target, map, 2, 5).unwrap();
    assert!(battle_unit_illuminated(&world, target));
    place_battle_unit(&mut world, target, map, 2, 4).unwrap();
    assert!(!battle_unit_illuminated(&world, target));
    place_battle_unit(&mut world, target, map, 2, 34).unwrap();
    world
        .objects
        .get_mut(&lamp)
        .unwrap()
        .flags
        .insert(Flag::Going);
    assert!(!battle_unit_illuminated(&world, target));
    world
        .objects
        .get_mut(&lamp)
        .unwrap()
        .flags
        .remove(Flag::Going);
    toggle_battle_searchlight(&mut world, lamp, ObjectId(1)).unwrap();
    for _ in 0..5 {
        advance_battle_searchlights(&mut world);
    }
    assert!(!battle_unit_illuminated(&world, target));
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn front_torso_damage_uses_lamp_state_and_exact_dice() {
    let (_dir, _config, baseline, lamp, _, _) = fixture().await;
    for on in [false, true] {
        for section in [
            BattleSection::LeftTorso,
            BattleSection::CenterTorso,
            BattleSection::RightTorso,
            BattleSection::LeftArm,
        ] {
            for rear in [false, true] {
                for seed in 0..32 {
                    let mut world = baseline.clone();
                    field(
                        &mut world,
                        lamp,
                        "searchlight",
                        serde_json::json!({"on":on,"destroyed":false,"remaining":3}),
                    );
                    let mut dice = BattleDice::seeded([seed; 32]);
                    field(
                        &mut world,
                        lamp,
                        "dice",
                        serde_json::to_value(&dice).unwrap(),
                    );
                    let exposed = !rear && section != BattleSection::LeftArm;
                    dice.two_d6(); // Material entry precedes the searchlight strike.
                    let destroyed = exposed && dice.two_d6() > 6 && (on || dice.two_d6() > 5);
                    let report = resolve_battle_impact(
                        &mut world,
                        lamp,
                        BattleHit {
                            section,
                            rear_armor: rear,
                            through_armor_critical: false,
                            crew_stun: false,
                        },
                        1,
                    )
                    .unwrap();
                    assert_eq!(report.searchlight_destroyed, destroyed);
                    assert_eq!(
                        world.btech.constructed_units()[&lamp]
                            .searchlight()
                            .destroyed,
                        destroyed
                    );
                    let state = serde_json::to_value(&world.btech).unwrap();
                    assert_eq!(
                        state["constructed"][lamp.0.to_string()]["dice"],
                        serde_json::to_value(dice).unwrap()
                    );
                    if destroyed {
                        assert_eq!(
                            world.btech.constructed_units()[&lamp]
                                .searchlight()
                                .remaining,
                            0
                        );
                        assert!(!battle_unit_illuminated(&world, lamp));
                        assert!(toggle_battle_searchlight(&mut world, lamp, ObjectId(1)).is_err());
                    }
                }
            }
        }
    }
}

/// At night the sensor band ignores darkness, while sight needs light: an unlit target costs +1
/// within visibility, and a searchlight beam lets sight reach three times as far without it.
#[tokio::test]
async fn searchlights_extend_night_sight_to_lit_targets() {
    let (_dir, _config, mut world, lamp, target, map) = fixture().await;
    place_battle_unit(&mut world, target, map, 2, 31).unwrap();
    set_battle_map_visibility(&mut world, map, BattleLight::Night, 3).unwrap();
    let perceived = |world: &World| {
        battle_perceive(world, lamp, target)
            .unwrap()
            .map(|perception| (perception.channel, perception.aim_modifier))
    };
    assert_eq!(
        perceived(&world),
        Some((BattleDetectionChannel::Sensors, 0))
    );
    set_battle_map_perception(&mut world, map, BattleMapPerceptionFlag::Sensors, false).unwrap();
    assert_eq!(perceived(&world), None);
    place_battle_unit(&mut world, target, map, 2, 33).unwrap();
    assert_eq!(perceived(&world), Some((BattleDetectionChannel::Sight, 1)));
    toggle_battle_searchlight(&mut world, lamp, ObjectId(1)).unwrap();
    for _ in 0..5 {
        advance_battle_searchlights(&mut world);
    }
    assert!(battle_unit_illuminated(&world, target));
    assert_eq!(perceived(&world), Some((BattleDetectionChannel::Sight, 0)));
    for (y, expected) in [
        (31, Some((BattleDetectionChannel::Sight, 0))),
        (26, Some((BattleDetectionChannel::Sight, 0))),
        (25, None),
    ] {
        place_battle_unit(&mut world, target, map, 2, y).unwrap();
        assert!(battle_unit_illuminated(&world, target));
        assert_eq!(perceived(&world), expected, "y={y}");
    }
}

#[tokio::test]
async fn illumination_warnings_are_opt_in_transactional_and_restart_safe() {
    let (_dir, config, mut world, lamp, source, map) = fixture().await;
    place_battle_unit(&mut world, source, map, 2, 36).unwrap();
    field(
        &mut world,
        source,
        "searchlight",
        serde_json::json!({"on":true,"destroyed":false,"remaining":0}),
    );
    assert!(battle_illumination_pending(&world));
    assert!(refresh_battle_illumination(&mut world).is_empty());
    assert!(!battle_illumination_pending(&world));
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.searchlight_warning({},1,true); error('abort')",
                lamp.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, world.btech);
    assert!(scripts.drain_outbox().is_empty());
    let message = support::run_text(&scripts, &config, ObjectId(1), 1, "mechprefs SLWarn ON");
    assert!(message.contains("warning when lit by searchlight is now ON"));
    set_battle_searchlight_warning(&mut world, lamp, ObjectId(1), true).unwrap();
    assert_eq!(scripts.world().btech, world.btech);
    assert!(refresh_battle_illumination(&mut world).is_empty());
    // A second beam prevents a false exit when one emitter goes away.
    let second = world.create(&config, "Second lamp".into(), Kind::Thing);
    world.objects.get_mut(&second).unwrap().home = Some(ObjectId(config.home()));
    let definition = world.btech.constructed_units()[&source]
        .definition()
        .clone();
    create_battle_unit(&mut world, second, definition).unwrap();
    place_battle_unit(&mut world, second, map, 2, 37).unwrap();
    field(
        &mut world,
        second,
        "searchlight",
        serde_json::json!({"on":true,"destroyed":false,"remaining":0}),
    );
    refresh_battle_illumination(&mut world);
    field(
        &mut world,
        source,
        "searchlight",
        serde_json::json!({"on":false,"destroyed":false,"remaining":0}),
    );
    assert!(refresh_battle_illumination(&mut world).is_empty());
    field(
        &mut world,
        second,
        "searchlight",
        serde_json::json!({"on":false,"destroyed":true,"remaining":0}),
    );
    let checkpoint = world.clone();
    let notices = refresh_battle_illumination(&mut world);
    assert_eq!(
        notices,
        vec![BattleNotice {
            unit: lamp,
            text: "You are no longer being illuminated.".into()
        }]
    );
    let mut replay = checkpoint;
    assert_eq!(refresh_battle_illumination(&mut replay), notices);
    assert_eq!(world.btech, replay.btech);
    assert!(refresh_battle_illumination(&mut world).is_empty());
    field(
        &mut world,
        source,
        "searchlight",
        serde_json::json!({"on":true,"destroyed":false,"remaining":0}),
    );
    assert_eq!(
        refresh_battle_illumination(&mut world),
        vec![BattleNotice {
            unit: lamp,
            text: "You are being illuminated!".into()
        }]
    );
    persistence::save(&config.database(), &world).await.unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    assert!(world.btech.constructed_units()[&lamp].searchlight_warning());
    assert!(!battle_illumination_pending(&world));
    assert!(refresh_battle_illumination(&mut world).is_empty());
    // The carrier's own lamp is visible but never counts as an external warning source.
    field(
        &mut world,
        source,
        "searchlight",
        serde_json::json!({"on":false,"destroyed":false,"remaining":0}),
    );
    refresh_battle_illumination(&mut world);
    field(
        &mut world,
        lamp,
        "searchlight",
        serde_json::json!({"on":true,"destroyed":false,"remaining":0}),
    );
    assert!(battle_unit_illuminated(&world, lamp));
    assert!(refresh_battle_illumination(&mut world).is_empty());
}

#[tokio::test]
async fn terrain_beams_reach_beyond_unit_illumination_and_stop_at_obstructions() {
    let (_dir, config, mut world, lamp, _, map) = fixture().await;
    let distant = BattleHexCoordinate { x: 2, y: 0 };
    let behind = BattleHexCoordinate { x: 2, y: 36 };
    set_battle_map_visibility(&mut world, map, BattleLight::Night, 15).unwrap();
    assert!(!battle_hex_visible(&world, lamp, distant).unwrap());
    let _ = toggle_battle_searchlight(&mut world, lamp, ObjectId(1)).unwrap();
    for _ in 0..5 {
        let _ = advance_battle_searchlights(&mut world);
    }
    assert!(battle_hex_illuminated(&world, map, distant).unwrap());
    assert!(battle_hex_visible(&world, lamp, distant).unwrap());
    assert!(!battle_hex_illuminated(&world, map, behind).unwrap());
    let before = world.btech.clone();
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert!(battle_hex_visible(&restored, lamp, distant).unwrap());
    assert_eq!(world.btech, before);
    set_map_decoration(
        &mut world,
        map,
        BattleHexCoordinate { x: 2, y: 20 },
        Some(BattleDecoration::new(BattleDecorationKind::Smoke, 30, None)),
    )
    .unwrap();
    assert!(!battle_hex_illuminated(&world, map, distant).unwrap());
    set_map_decoration(&mut world, map, BattleHexCoordinate { x: 2, y: 20 }, None).unwrap();
    world
        .objects
        .get_mut(&lamp)
        .unwrap()
        .flags
        .insert(Flag::Going);
    assert!(!battle_hex_illuminated(&world, map, distant).unwrap());
}

/// Read the lamp's persisted switch state.
fn lamp_state(world: &World, id: ObjectId) -> BattleSearchlight {
    world.btech.constructed_units()[&id].searchlight()
}

/// Automatic lamps follow map darkness, cancel reversed switches and re-evaluate on map changes.
#[tokio::test]
async fn automatic_lamps_follow_map_light_changes_and_transfers() {
    let (_dir, config, mut world, lamp, _, map) = fixture().await;
    assert_eq!(lamp_state(&world, lamp).mode, BattleSearchlightMode::Auto);
    assert_eq!(lamp_state(&world, lamp).remaining, 0);
    set_battle_map_visibility(&mut world, map, BattleLight::Twilight, 30).unwrap();
    assert_eq!(lamp_state(&world, lamp).remaining, 0);
    set_battle_map_visibility(&mut world, map, BattleLight::Night, 30).unwrap();
    assert_eq!(lamp_state(&world, lamp).remaining, 5);
    for _ in 0..4 {
        advance_battle_searchlights(&mut world);
    }
    let notices = advance_battle_searchlights(&mut world);
    assert!(notices.iter().any(|n| n.text.contains("full power")));
    assert!(lamp_state(&world, lamp).on);
    // Visibility-only edits leave the lamp alone.
    set_battle_map_visibility(&mut world, map, BattleLight::Night, 10).unwrap();
    assert_eq!(lamp_state(&world, lamp).remaining, 0);
    // Daylight starts a cool-down; nightfall before it expires cancels it.
    set_battle_map_visibility(&mut world, map, BattleLight::Day, 30).unwrap();
    assert_eq!(lamp_state(&world, lamp).remaining, 5);
    advance_battle_searchlights(&mut world);
    set_battle_map_visibility(&mut world, map, BattleLight::Night, 30).unwrap();
    assert_eq!(lamp_state(&world, lamp).remaining, 0);
    assert!(lamp_state(&world, lamp).on);
    set_battle_map_visibility(&mut world, map, BattleLight::Day, 30).unwrap();
    for _ in 0..5 {
        advance_battle_searchlights(&mut world);
    }
    assert!(!lamp_state(&world, lamp).on);

    // Moving to a dark battlefield warms the lamp up.
    let dark = world.create(&config, "Dark field".into(), Kind::Room);
    let terrain = format!("5 40\n{}", ".0.0.0.0.0\n".repeat(40));
    create_battle_map(
        &mut world,
        dark,
        "dark.map",
        BattleMapAsset::from_cells(&terrain).unwrap(),
    )
    .unwrap();
    set_battle_map_visibility(&mut world, dark, BattleLight::Night, 30).unwrap();
    transfer_battle_unit(
        &mut world,
        lamp,
        BattlePosition {
            map: dark,
            x: 2,
            y: 20,
        },
    )
    .unwrap();
    assert_eq!(lamp_state(&world, lamp).remaining, 5);
    for _ in 0..5 {
        advance_battle_searchlights(&mut world);
    }
    assert!(lamp_state(&world, lamp).on);

    // Shutdown extinguishes the lamp; completing startup relights it.
    stop_battle_unit(
        &mut world,
        lamp,
        ObjectId(1),
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    assert!(!lamp_state(&world, lamp).on);
    assign_battle_pilot(&mut world, lamp, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, lamp, ObjectId(1), true).unwrap();
    for _ in 0..4 {
        advance_battle_units(&mut world, 0);
    }
    assert_eq!(lamp_state(&world, lamp).remaining, 0);
    advance_battle_units(&mut world, 0);
    assert_eq!(lamp_state(&world, lamp).remaining, 5);
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

/// Explicit modes hold their state through light changes; toggling leaves automatic mode.
#[tokio::test]
async fn manual_modes_override_automatic_switching() {
    let (_dir, config, world, lamp, _, map) = fixture().await;
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "slite bogus")
            .contains("slite [on|off|auto]")
    );
    assert!(support::run_text(&scripts, &config, ObjectId(1), 1, "slite off").contains("stay off"));
    assert_eq!(
        lamp_state(&scripts.world(), lamp).mode,
        BattleSearchlightMode::Off
    );
    set_battle_map_visibility(&mut scripts.world_mut(), map, BattleLight::Night, 30).unwrap();
    assert_eq!(lamp_state(&scripts.world(), lamp).remaining, 0);
    let text = support::run_text(&scripts, &config, ObjectId(1), 1, "slite auto");
    assert!(
        text.contains("at night") && text.contains("warm up"),
        "{text}"
    );
    assert_eq!(lamp_state(&scripts.world(), lamp).remaining, 5);
    // Choosing "on" while already warming up keeps the pending switch.
    let text = support::run_text(&scripts, &config, ObjectId(1), 1, "slite on");
    assert!(
        text.contains("stay on") && !text.contains("warm up"),
        "{text}"
    );
    assert_eq!(lamp_state(&scripts.world(), lamp).remaining, 5);
    // Turning it off mid warm-up cancels the switch.
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "slite off").contains("cancelled")
    );
    assert_eq!(lamp_state(&scripts.world(), lamp).remaining, 0);
    // A bare toggle is a manual choice.
    scripts
        .eval_callback::<()>(&format!(
            "btech.unit.slite({},1,btech.unit.searchlight_modes.AUTO)",
            lamp.0
        ))
        .unwrap();
    assert_eq!(
        lamp_state(&scripts.world(), lamp).mode,
        BattleSearchlightMode::Auto
    );
    assert_eq!(lamp_state(&scripts.world(), lamp).remaining, 5);
    support::run_text(&scripts, &config, ObjectId(1), 1, "slite");
    assert_eq!(
        lamp_state(&scripts.world(), lamp).mode,
        BattleSearchlightMode::On
    );
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.slite({},1,btech.map.light_levels.DAY)",
                lamp.0
            ))
            .is_err()
    );
}
