//! Map-only terrain views share rendering and clipping without cockpit or sensor side effects.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Source text preserves styled output for native/Lua publication comparisons.
fn output(scripts: &Scripts) -> String {
    scripts
        .drain_outbox()
        .into_iter()
        .map(|(_, text)| text.source().to_owned())
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn map_views_clip_both_parities_and_preserve_terrain_cells_and_preferences() {
    for (width, height) in [(1, 1), (3, 2), (12, 8)] {
        let (_dir, config, mut world) = support::isolated_world().await;
        let map = world.create(&config, "Map view".into(), Kind::Room);
        let row: String = (0..width)
            .map(|x| if x % 2 == 0 { "~2" } else { "/3" })
            .collect();
        create_battle_map(
            &mut world,
            map,
            "view",
            BattleMapAsset::from_cells(&format!(
                "{width} {height}\n{}",
                format!("{row}\n").repeat(height)
            ))
            .unwrap(),
        )
        .unwrap();
        support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
        world
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .insert(Flag::Ansi);
        set_battle_view_dimensions(
            &mut world,
            ObjectId(1),
            BattleViewDimensions {
                tactical_width: 5,
                tactical_height: 5,
                long_range_height: 11,
            },
        )
        .unwrap();
        let before = world.btech.clone();
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        for center in [
            BattleHexCoordinate {
                x: i32::MIN,
                y: i32::MIN,
            },
            BattleHexCoordinate { x: 3, y: 3 },
            BattleHexCoordinate { x: 4, y: 4 },
            BattleHexCoordinate {
                x: i32::MAX,
                y: i32::MAX,
            },
        ] {
            let report =
                view_battle_map_action(&native, &config, ObjectId(1), map, center).unwrap();
            assert_eq!(output(&native), report.text);
            assert_eq!(report.viewport.width, width.min(5) as u16);
            assert_eq!(report.viewport.height, height.min(5) as u16);
            assert_eq!(report.viewport.maximum_range, 0);
            assert_eq!(
                report.viewport.requested_center.x,
                center.x.clamp(0, width - 1)
            );
            assert_eq!(
                report.viewport.requested_center.y,
                center.y.clamp(0, height as i32 - 1)
            );
            let plain = text::plain(&report.text);
            let lines: Vec<_> = plain.lines().collect();
            assert_eq!(lines.len(), 4 + 2 * usize::from(report.viewport.height));
            for y in 0..usize::from(report.viewport.height) {
                for x in 0..usize::from(report.viewport.width) {
                    let global_x = report.viewport.origin.x + x as i32;
                    let row = 3 + 2 * y + usize::from(global_x % 2 == 0);
                    let column = 5 + 3 * x;
                    let (top, bottom) = if global_x % 2 == 0 {
                        ("~~", "~2")
                    } else {
                        ("##", "+3")
                    };
                    assert_eq!(&lines[row][column..column + 2], top);
                    assert_eq!(
                        &lines[row + 1][column..column + 2],
                        bottom,
                        "map {width}x{height}, center {center:?}, cell {x},{y}, viewport {:?}\n{plain}",
                        report.viewport
                    );
                }
            }
            assert!(report.text.contains("[fg="));
            let native_text = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("view {} {}", center.x, center.y),
            );
            assert_eq!(native_text, report.text);
            assert_eq!(text::plain(&native_text), plain);
            let lua_text: String = lua
                .eval_callback(&format!(
                    "return btech.map.view(1,{},{},{}).text",
                    map.0, center.x, center.y
                ))
                .unwrap();
            assert_eq!(lua_text, report.text);
            assert_eq!(output(&lua), report.text);
            native
                .world_mut()
                .objects
                .get_mut(&ObjectId(1))
                .unwrap()
                .flags
                .remove(Flag::Ansi);
            let unstyled =
                view_battle_map_action(&native, &config, ObjectId(1), map, center).unwrap();
            assert!(!unstyled.text.contains("[fg="));
            assert_eq!(text::plain(&unstyled.text), plain);
            assert_eq!(output(&native), unstyled.text);
            native
                .world_mut()
                .objects
                .get_mut(&ObjectId(1))
                .unwrap()
                .flags
                .insert(Flag::Ansi);
            assert_eq!(native.world().btech, before);
            assert_eq!(lua.world().btech, before);
        }
        let saved = native.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
    }
}

#[tokio::test]
async fn map_views_ignore_unit_markers_and_leave_all_supported_chassis_unchanged() {
    for template in firing::templates() {
        let (_dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(&template, None, include_str!("../game/mechs/AS7-D.toml"))
                .await;
        let map = world.btech.units()[&unit].map.unwrap();
        let actor = world.create(&config, "Map viewer".into(), Kind::Player);
        world
            .objects
            .get_mut(&actor)
            .unwrap()
            .flags
            .insert(Flag::Wizard);
        world.objects.get_mut(&actor).unwrap().location = Some(map);
        set_map_decoration(
            &mut world,
            map,
            BattleHexCoordinate { x: 0, y: 0 },
            Some(BattleDecoration::new(BattleDecorationKind::Fire, 0, None)),
        )
        .unwrap();
        let before = world.btech.clone();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let report = view_battle_map_action(
            &scripts,
            &config,
            actor,
            map,
            BattleHexCoordinate { x: 0, y: 0 },
        )
        .unwrap();
        let text = text::plain(&report.text);
        assert!(text.contains("&&"));
        assert!(!text.contains("**") && !text.contains("AA") && !text.contains("AB"));
        assert_eq!(scripts.world().btech, before);
    }
}

#[tokio::test]
async fn view_rejection_callback_and_partial_publication_leave_no_effects() {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Map view".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "view",
        BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let center = BattleHexCoordinate { x: 0, y: 0 };
    assert!(view_battle_map_action(&scripts, &config, map, map, center).is_err());
    assert!(view_battle_map_action(&scripts, &config, ObjectId(1), ObjectId(-1), center).is_err());
    assert!(
        scripts
            .eval_callback::<()>(&format!("btech.map.view(1,{},0,0);error('abort')", map.0))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(output(&scripts).is_empty());
    for input in [
        "view",
        "view nope 0",
        "view 0 0 extra",
        "view 2147483648 0",
        "view/nope 0 0",
    ] {
        let response = support::run_text(&scripts, &config, ObjectId(1), 1, input);
        assert!(!response.is_empty());
        assert_eq!(scripts.world().btech, before);
    }
    let path = dir.path().join("stompymux.toml");
    let mut table: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
    table
        .entry("lua")
        .or_insert(toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .unwrap()
        .insert("output_entry_limit".into(), 2.into());
    std::fs::write(path, toml::to_string(&table).unwrap()).unwrap();
    let config = Config::load(dir.path()).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(scripts.world().clone()))).unwrap();
    assert!(view_battle_map_action(&scripts, &config, ObjectId(1), map, center).is_err());
    assert_eq!(scripts.world().btech, before);
    assert!(output(&scripts).is_empty());
}
