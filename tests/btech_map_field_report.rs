//! Map inspection preserves field order, literal output and native/Lua transaction behavior.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

fn output(scripts: &Scripts) -> String {
    scripts
        .drain_outbox()
        .into_iter()
        .map(|(_, line)| text::plain(line.source()))
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn report_values_order_filters_and_layouts_are_explicit() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "[fg=red]Map[reset]".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "[reset]海",
        MapAsset::from_cells("2 1\n.0.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let report = view_battle_map_fields_action(&scripts, &config, ObjectId(1), map, "").unwrap();
    let expected = [
        ("buildonmap", Some("0")),
        ("cf", Some("0")),
        ("cfmax", Some("0")),
        ("gravity", Some("100")),
        ("firstfree", None),
        ("mapheight", Some("1")),
        ("maplight", Some("2")),
        ("mapname", Some("[reset]海")),
        ("mapvis", Some("30")),
        ("mapwidth", Some("2")),
        ("maxvis", Some("60")),
        ("temperature", Some("20")),
        ("winddir", Some("0")),
        ("windspeed", Some("0")),
        ("cloudbase", Some("200")),
        ("flags", Some("-")),
        ("sensorflags", Some("-")),
        ("regen_factor", Some("1")),
    ];
    assert_eq!(
        report
            .fields
            .iter()
            .map(|field| (field.name, field.value.as_deref()))
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(report.columns, 2);
    assert_eq!(report.text.lines().count(), 13);
    assert!(report.text.contains("Data for [fg=red]Map[reset] (MAP)"));
    assert_eq!(output(&scripts), report.text);
    for (arguments, columns, names) in [
        (
            "1MAP",
            1,
            vec!["mapheight", "maplight", "mapname", "mapvis", "mapwidth"],
        ),
        ("4 cf", 4, vec!["cf", "cfmax"]),
        ("wind", 2, vec!["winddir", "windspeed"]),
        ("4first", 4, vec!["firstfree"]),
        ("unknown", 2, vec![]),
        ("2", 2, vec![]),
    ] {
        let report =
            view_battle_map_fields_action(&scripts, &config, ObjectId(1), map, arguments).unwrap();
        assert_eq!(report.columns, columns);
        assert_eq!(
            report.fields.iter().map(|f| f.name).collect::<Vec<_>>(),
            names
        );
        assert_eq!(output(&scripts), report.text);
        assert_eq!(
            report.text.lines().count(),
            4 + names.len().div_ceil(columns)
        );
        assert_eq!(
            text::plain(&support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("@viewmap {arguments}")
            )),
            report.text
        );
        let lua: String = scripts
            .eval_callback(&format!(
                "return btech.map.fields(1,{}, {:?}).text",
                map.0, arguments
            ))
            .unwrap();
        assert_eq!(lua, report.text);
        assert_eq!(output(&scripts), report.text);
    }
    set_battle_map_field_action(&scripts, &config, ObjectId(1), map, "gravity", "-1").unwrap();
    set_battle_map_field_action(
        &scripts,
        &config,
        ObjectId(1),
        map,
        "flags",
        "dark,underground",
    )
    .unwrap();
    assert_eq!(
        view_battle_map_fields_action(&scripts, &config, ObjectId(1), map, "gravity")
            .unwrap()
            .fields[0]
            .value
            .as_deref(),
        Some("-1")
    );
    assert_eq!(
        view_battle_map_fields_action(&scripts, &config, ObjectId(1), map, "flags")
            .unwrap()
            .fields[0]
            .value
            .as_deref(),
        Some("underground dark")
    );
}

#[tokio::test]
async fn field_reports_leave_every_chassis_unchanged_and_survive_restart() {
    for template in firing::templates() {
        let (_dir, config, mut world, source, _, _) =
            firing::fixture_with_target(&template, None, include_str!("../game/mechs/AS7-D.toml"))
                .await;
        let map = world.btech.units()[&source].map.unwrap();
        let actor = world.create(&config, "Inspector".into(), Kind::Player);
        world
            .objects
            .get_mut(&actor)
            .unwrap()
            .flags
            .insert(Flag::Wizard);
        world.objects.get_mut(&actor).unwrap().location = Some(map);
        let before = world.btech.clone();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let report = view_battle_map_fields_action(&scripts, &config, actor, map, "1").unwrap();
        assert_eq!(report.fields.len(), 18);
        assert_eq!(output(&scripts), report.text);
        assert_eq!(scripts.world().btech, before);
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let world = persistence::load(&config.database()).await.unwrap();
        assert_eq!(world.btech, before);
        let restart = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        assert_eq!(
            view_battle_map_fields_action(&restart, &config, actor, map, "1").unwrap(),
            report
        );
    }
}

#[tokio::test]
async fn inspection_authority_and_publication_failures_publish_no_partial_report() {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Map".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    assert!(view_battle_map_fields_action(&scripts, &config, map, map, "").is_err());
    assert!(
        view_battle_map_fields_action(&scripts, &config, ObjectId(1), ObjectId(-1), "").is_err()
    );
    assert!(
        scripts
            .eval_callback::<()>(&format!("btech.map.fields(1,{});error('abort')", map.0))
            .is_err()
    );
    assert!(scripts.drain_outbox().is_empty());
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
    assert!(view_battle_map_fields_action(&scripts, &config, ObjectId(1), map, "").is_err());
    assert!(scripts.drain_outbox().is_empty());
    assert_eq!(scripts.world().btech, before);
}
