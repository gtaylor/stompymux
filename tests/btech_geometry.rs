//! Reference coordinate anchors and range adapters on signed-elevation battlefields.
use crate::support;
use stompymux_rs::{
    Flag, HexCoordinate, Kind, MapAsset, MechTemplate, ObjectId, Scripts, battle_unit_range,
    create_battle_map, create_battle_unit, place_battle_unit,
};

#[test]
fn normalized_centers_match_reference_coordinate_anchors() {
    // Reference real-coordinate units are 322.5 per hex; even columns have a 161.25 offset.
    for (x, y, real_x, real_y) in [
        (0, 0, 186.19546, 161.25),
        (1, 0, 465.48865, 0.0),
        (2, 0, 744.78184, 161.25),
        (0, 1, 186.19546, 483.75),
    ] {
        let point = HexCoordinate { x, y }.center();
        assert!((point.x * 322.5 - real_x).abs() < 0.0001);
        assert!((point.y * 322.5 - real_y).abs() < 0.0001);
    }
}

#[tokio::test]
async fn ranges_include_depth_and_elevation_and_reject_different_maps() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Terrain range".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "range.map",
        MapAsset::from_cells("3 1\n~2.0^3\n").unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for (name, x) in [("Water Jenner", 0), ("Hill Jenner", 2)] {
        let id = world.create(&config, name.into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        create_battle_unit(
            &mut world,
            id,
            MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap(),
        )
        .unwrap();
        place_battle_unit(&mut world, id, map, x, 0).unwrap();
        ids.push(id);
    }
    let range = battle_unit_range(&world, ids[0], ids[1]).unwrap();
    assert!((range.horizontal - 3.0_f64.sqrt()).abs() < 1e-12);
    assert!((range.spatial - 2.0).abs() < 1e-12);
    assert_eq!(range.hex_distance, 2);
    assert_eq!(range.bearing, Some(90.0));
    let reverse = battle_unit_range(&world, ids[1], ids[0]).unwrap();
    assert_eq!(reverse.spatial, range.spatial);
    assert_eq!(reverse.bearing, Some(270.0));
    let same = battle_unit_range(&world, ids[0], ids[0]).unwrap();
    assert_eq!(same.spatial, 0.0);
    assert_eq!(same.bearing, None);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let value: f64 = scripts
        .eval_callback(&format!(
            "return btech.unit.range({},{}).spatial",
            ids[0].0, ids[1].0
        ))
        .unwrap();
    assert_eq!(value, range.spatial);
    let text = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        &format!("@btech range #{},#{}", ids[0].0, ids[1].0),
    );
    assert!(text.contains("spatial range 2.000"), "{text}");
    assert!(text.contains("line of sight is not evaluated"), "{text}");
    scripts
        .world_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    assert!(
        support::run_text(
            &scripts,
            &config,
            ObjectId(2),
            2,
            &format!("@btech range #{},#{}", ids[0].0, ids[1].0)
        )
        .contains("Permission denied")
    );
    let outside: bool = scripts
        .inspect_lua()
        .load(format!(
            "return pcall(btech.unit.range,{},{})",
            ids[0].0, ids[1].0
        ))
        .eval()
        .unwrap();
    assert!(!outside);
    let other = scripts
        .world_mut()
        .create(&config, "Other battlefield".into(), Kind::Room);
    create_battle_map(
        &mut scripts.world_mut(),
        other,
        "other.map",
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut scripts.world_mut(), ids[1], other, 0, 0).unwrap();
    assert!(battle_unit_range(&scripts.world(), ids[0], ids[1]).is_err());
    assert!(battle_unit_range(&scripts.world(), ids[0], ObjectId(999999)).is_err());
}

#[test]
fn segment_trace_covers_intermediate_cells_and_reverses_consistently() {
    use stompymux_rs::Point;
    let start = HexCoordinate { x: 5, y: 5 }.center();
    let end = HexCoordinate { x: 5, y: 0 }.center();
    assert_eq!(
        start.trace(end).unwrap(),
        (0..=5)
            .rev()
            .map(|y| HexCoordinate { x: 5, y })
            .collect::<Vec<_>>()
    );
    // A fan of oblique paths includes both column parities and the clipped left border.
    for angle in (0..360).step_by(7) {
        let end = start.project(f64::from(angle) + 0.13, 7.3).unwrap();
        let trace = start.trace(end).unwrap();
        let mut reverse = end.trace(start).unwrap();
        reverse.reverse();
        assert_eq!(trace, reverse);
        for step in 0..=400 {
            let t = f64::from(step) / 400.0;
            let point = Point {
                x: start.x + (end.x - start.x) * t,
                y: start.y + (end.y - start.y) * t,
            };
            assert!(trace.contains(&point.containing_hex().unwrap()));
        }
    }
    assert_eq!(
        start.trace(start).unwrap(),
        vec![start.containing_hex().unwrap()]
    );
    assert!(start.trace(start.project(0.0, 4097.0).unwrap()).is_err());
}

#[test]
fn segment_trace_detects_a_thin_corner_crossing_between_dry_endpoints() {
    use stompymux_rs::Point;
    let obstacle = HexCoordinate { x: 4, y: 4 };
    let center = obstacle.center();
    let x = center.x + 1.0 / 3.0_f64.sqrt() - 1e-7;
    let start = Point {
        x,
        y: center.y - 0.213,
    };
    let end = Point {
        x,
        y: center.y + 0.197,
    };
    assert_ne!(start.containing_hex().unwrap(), obstacle);
    assert_ne!(end.containing_hex().unwrap(), obstacle);
    assert!(start.trace(end).unwrap().contains(&obstacle));
    // Moving just outside the corner must not create a false collision.
    let start = Point {
        x: x + 2e-7,
        ..start
    };
    let end = Point { x: x + 2e-7, ..end };
    assert!(!start.trace(end).unwrap().contains(&obstacle));
}
