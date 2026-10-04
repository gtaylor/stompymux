//! Shared cockpit maps and navigation retain mixed-chassis contacts, authority and replay.
use crate::support;
use stompymux_rs::*;

/// Read the staggered LRS cell for a known coordinate, independently of contact marker choice.
fn cell(report: &BattleLongRangeMap, x: i32, y: i32) -> char {
    let x_offset = (x - report.viewport.origin.x) as usize;
    let y_offset = (y - report.viewport.origin.y) as usize;
    let label_rows = (report.viewport.origin.x + i32::from(report.viewport.width) - 1)
        .to_string()
        .len()
        .max(3);
    text::plain(&report.text)
        .lines()
        .nth(label_rows + y_offset * 2 + usize::from(x % 2 == 0))
        .unwrap()
        .chars()
        .nth(4 + x_offset)
        .unwrap()
}

#[tokio::test]
async fn mixed_maps_navigation_and_measurements_share_every_supported_movement_type() {
    let (_dir, config, mut base) = support::isolated_world().await;
    let map = base.create(&config, "Map formation".into(), Kind::Room);
    create_battle_map(
        &mut base,
        map,
        "formation",
        MapAsset::from_cells(
            "7 4\n.0.0.0.0.0.0.0\n.0.0.0.0.0.0.0\n.0.0.0.0.0.0.0\n.0.0.0.0.0.0.0\n",
        )
        .unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut base, map, support::FIXTURE_DICE_SEED);
    let stationary = include_str!("../game/mechs/Demolisher.toml")
        .replace("movement = \"track\"", "movement = \"none\"")
        .replace("walk_mp = 5", "walk_mp = 0");
    let sources = [
        (include_str!("fixtures/btech/mechs/JR7-D.toml"), 'b'),
        (include_str!("../game/mechs/SCP-1N.toml"), 'q'),
        (include_str!("../game/mechs/Demolisher.toml"), 't'),
        (include_str!("../game/mechs/Jeep.toml"), 'w'),
        (include_str!("../game/mechs/Savannah_Master.toml"), 'h'),
        (include_str!("../game/mechs/Kestrel.toml"), 'v'),
        (stationary.as_str(), 'u'),
    ];
    let mut ids = Vec::new();
    for (x, (source, _)) in sources.iter().enumerate() {
        let id = base.create(&config, "Formation unit".into(), Kind::Thing);
        BattleUnitTemplate::parse("test", source)
            .unwrap()
            .create(&mut base, id)
            .unwrap();
        support::seed_object_dice(&mut base, id, support::FIXTURE_DICE_SEED);
        place_battle_unit(&mut base, id, map, x as i64, 2).unwrap();
        ids.push(id);
    }
    let mut state = serde_json::to_value(&base.btech).unwrap();
    for &id in &ids {
        let key = if base.btech.vehicles().contains_key(&id) {
            "vehicles"
        } else {
            "constructed"
        };
        let unit = &mut state[key][id.0.to_string()];
        unit["power"] = serde_json::to_value(BattlePower::Running).unwrap();
        for &target in &ids {
            if target != id {
                unit["contacts"][target.0.to_string()] = serde_json::json!({"identified": false});
            }
        }
    }
    state["vehicles"][ids[5].0.to_string()]["vtol_flight"]["phase"] =
        serde_json::to_value(BattleVtolFlightPhase::Airborne).unwrap();
    state["vehicles"][ids[5].0.to_string()]["vtol_flight"]["altitude"] = 5.25.into();
    state["vehicles"][ids[5].0.to_string()]["vtol_flight"]["vertical_speed"] = 1.25.into();
    base.btech = serde_json::from_value(state).unwrap();
    for (index, &observer) in ids.iter().enumerate() {
        let mut world = base.clone();
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(observer);
        assign_battle_pilot(&mut world, observer, ObjectId(1)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        let target = ids[(index + 1) % ids.len()];
        let _ = select_battle_target(&mut world, observer, ObjectId(1), Some(target)).unwrap();
        world.validate(&config).unwrap();
        let before = world.btech.clone();
        let dimensions = BattleViewDimensions::default();
        let lrs = battle_long_range_map(
            &world,
            observer,
            ObjectId(1),
            BattleLongRangeMode::Units,
            "",
            dimensions,
        )
        .unwrap();
        for (x, (_, glyph)) in sources.iter().enumerate() {
            assert_eq!(
                cell(&lrs, x as i32, 2),
                if x == index { '*' } else { *glyph }
            );
        }
        let tac = battle_tactical_map(&world, observer, ObjectId(1), "", dimensions).unwrap();
        assert!(tac.text.contains("**"));
        for (x, _) in sources.iter().enumerate().filter(|(x, _)| *x != index) {
            assert!(text::plain(&tac.text).contains(&format!("a{}", char::from(b'a' + x as u8))));
        }
        for mode in [
            BattleLongRangeMode::Terrain,
            BattleLongRangeMode::Elevation,
            BattleLongRangeMode::ColoredElevation,
            BattleLongRangeMode::VisibleTerrain,
            BattleLongRangeMode::VisibleElevation,
            BattleLongRangeMode::VisibleUnits,
            BattleLongRangeMode::UnderlyingTerrain,
        ] {
            battle_long_range_map(&world, observer, ObjectId(1), mode, "", dimensions).unwrap();
        }
        for flag in ["L", "C", "T", "B", "M"] {
            battle_tactical_map(&world, observer, ObjectId(1), flag, dimensions).unwrap();
        }
        let nav = battle_navigate(&world, observer, ObjectId(1), "").unwrap();
        assert!(nav.text.contains(&format!(
            "Vertical Speed:  {:6.1}",
            if index == 5 { 1.25 } else { 0.0 }
        )));
        let range = battle_range_report(&world, observer, ObjectId(1), "").unwrap();
        assert!(
            (range.spatial - battle_unit_range(&world, observer, target).unwrap().spatial).abs()
                < 1e-10
        );
        let bearing = battle_bearing(&world, observer, ObjectId(1), "").unwrap();
        let centered = parse_battle_view_center(
            &world,
            observer,
            ObjectId(1),
            BattleViewKind::Tactical,
            &format!("#{}", target.0),
        )
        .unwrap();
        assert_eq!(centered.center.x, ((index + 1) % ids.len()) as i32);
        assert!(
            parse_battle_view_center(
                &world,
                observer,
                ObjectId(1),
                BattleViewKind::Tactical,
                "90 1000"
            )
            .is_err()
        );
        assert!(battle_navigate(&world, observer, ObjectId(-1), "").is_err());
        let scripts = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        for (command, expression, expected) in [
            (
                "tactical",
                format!("btech.unit.tactical({},1).text", observer.0),
                tac.text,
            ),
            (
                "lrsmap M",
                format!("btech.unit.lrsmap({},1,'M').text", observer.0),
                lrs.text.clone(),
            ),
            (
                "navigate",
                format!("btech.unit.navigate({},1).text", observer.0),
                nav.text,
            ),
            (
                "range",
                format!("btech.unit.range_report({},1).text", observer.0),
                range.text,
            ),
            (
                "bearing",
                format!("btech.unit.bearing({},1).text", observer.0),
                bearing.text,
            ),
        ] {
            assert_eq!(
                support::run_text(&scripts, &config, ObjectId(1), 1, command),
                expected
            );
            assert_eq!(
                scripts
                    .eval_callback::<String>(&format!("return {expression}"))
                    .unwrap(),
                expected
            );
        }
        for (mode, selector) in [
            (BattleLongRangeMode::Terrain, "Terrain-map"),
            (BattleLongRangeMode::Elevation, "Elevation-map"),
            (BattleLongRangeMode::ColoredElevation, "Combined"),
            (BattleLongRangeMode::Units, "Mechs"),
            (BattleLongRangeMode::VisibleTerrain, "LOS"),
            (BattleLongRangeMode::VisibleElevation, "Height"),
            (BattleLongRangeMode::VisibleUnits, "Sensors"),
        ] {
            let label = format!("a{}", char::from(b'a' + ((index + 1) % ids.len()) as u8));
            for arguments in [
                String::new(),
                "180 1".into(),
                format!("#{}", target.0),
                label,
            ] {
                let expected = battle_long_range_map(
                    &world,
                    observer,
                    ObjectId(1),
                    mode,
                    &arguments,
                    dimensions,
                )
                .unwrap();
                for name in ["LRS", "lrsmap"] {
                    assert_eq!(
                        support::run_text(
                            &scripts,
                            &config,
                            ObjectId(1),
                            1,
                            &format!("{name} {selector} {arguments}")
                        ),
                        expected.text
                    );
                }
                assert_eq!(
                    scripts
                        .eval_callback::<String>(&format!(
                            "return btech.unit.lrsmap({},1,{selector:?},{arguments:?}).text",
                            observer.0
                        ))
                        .unwrap(),
                    expected.text
                );
            }
        }
        for gate in ["pilot", "power", "hardware", "center", "mode"] {
            let mut candidate = world.clone();
            let (arguments, expected) = match gate {
                "pilot" => {
                    release_battle_pilot(&mut candidate, observer, ObjectId(1)).unwrap();
                    ("bogus", "pilot")
                }
                "power" => {
                    let _ = stop_battle_unit(
                        &mut candidate,
                        observer,
                        ObjectId(1),
                        BattleFallRules::configured(&config),
                    )
                    .unwrap();
                    assign_battle_pilot(&mut candidate, observer, ObjectId(1)).unwrap();
                    support::seed_object_dice(
                        &mut candidate,
                        ObjectId(1),
                        support::FIXTURE_DICE_SEED,
                    );
                    ("bogus", "Start the unit first")
                }
                "hardware" => {
                    let Some(mut unit) =
                        candidate.btech.constructed_units().get(&observer).cloned()
                    else {
                        continue;
                    };
                    for slot in [1, 4] {
                        unit.destroy_critical(CriticalLocation {
                            section: BattleSection::Head,
                            slot,
                        })
                        .unwrap();
                    }
                    let mut state = serde_json::to_value(&candidate.btech).unwrap();
                    state["constructed"][observer.0.to_string()] =
                        serde_json::to_value(unit).unwrap();
                    candidate.btech = serde_json::from_value(state).unwrap();
                    ("bogus", "inoperational")
                }
                "center" => ("90 nope", "Invalid bearing or range"),
                _ => ("", "Supported LRS sensor types"),
            };
            let expected_state = candidate.btech.clone();
            let rejected = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(candidate)),
            )
            .unwrap();
            for name in ["LRS", "lrsmap"] {
                let response = support::run_text(
                    &rejected,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("{name} ? {arguments}"),
                );
                assert!(response.contains(expected), "{index} {gate}: {response}");
            }
            let error = rejected
                .eval_callback::<mlua::Table>(&format!(
                    "return btech.unit.lrsmap({},1,'?',{arguments:?})",
                    observer.0
                ))
                .unwrap_err();
            assert!(
                error.to_string().contains(expected),
                "{index} {gate}: {error}"
            );
            assert_eq!(rejected.world().btech, expected_state);
        }
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            battle_long_range_map(
                &restored,
                observer,
                ObjectId(1),
                BattleLongRangeMode::Units,
                "",
                dimensions
            )
            .unwrap(),
            lrs
        );
        set_battle_unit_signature(
            &mut world,
            target,
            BattleUnitSignature {
                team: 1,
                ..Default::default()
            },
        )
        .unwrap();
        let hostile = battle_long_range_map(
            &world,
            observer,
            ObjectId(1),
            BattleLongRangeMode::Units,
            "",
            dimensions,
        )
        .unwrap();
        let target_index = (index + 1) % ids.len();
        assert_eq!(
            cell(&hostile, target_index as i32, 2),
            sources[target_index].1.to_ascii_uppercase()
        );
    }
}
