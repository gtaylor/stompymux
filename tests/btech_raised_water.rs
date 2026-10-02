//! Water, ice and bridges above level 0 behave exactly like the same terrain at level 0,
//! only higher: lifting a whole map leaves movement, falls, flooding and sight unchanged.
use crate::support;
use std::sync::Arc;
use stompymux_rs::*;

/// How far the raised map is lifted.
const LIFT: u8 = 3;

/// A lane south from open ground across ice, through shallow and deep water, then woods
/// and over a bridge.
const LANE: &str =
    "3 10\n.0.0.0\n.0.0.0\n-2-2-2\n.0.0.0\n~1~1~1\n~2~2~2\n.0.0.0\n`0`0`0\n/1/1/1\n.0.0.0\n";

/// The lane with every hex's ground, and so every water surface and deck, lifted.
fn lifted(asset: &BattleMapAsset) -> BattleMapAsset {
    BattleMapAsset {
        hexes: Arc::new(
            asset
                .hexes
                .iter()
                .map(|hex| hex.with_level(hex.level() + LIFT))
                .collect(),
        ),
        ..asset.clone()
    }
}

/// Two running bipeds at 1,1: the first walks south through the lane, the second watches.
async fn field(asset: BattleMapAsset) -> (tempfile::TempDir, World, [ObjectId; 2]) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Raised field".into(), Kind::Room);
    create_battle_map(&mut world, map, "raised.map", asset).unwrap();
    let mut units = Vec::new();
    for pilot in [ObjectId(1), ObjectId(2)] {
        let id = world.create(&config, format!("Lane unit {}", pilot.0), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        create_battle_unit(
            &mut world,
            id,
            BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml"))
                .unwrap(),
        )
        .unwrap();
        place_battle_unit(&mut world, id, map, 1, 1).unwrap();
        world.objects.get_mut(&pilot).unwrap().location = Some(id);
        assign_battle_pilot(&mut world, id, pilot).unwrap();
        start_battle_unit(&mut world, id, pilot, true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
        let mut state = serde_json::to_value(&world.btech).unwrap();
        let unit = &mut state["constructed"][id.0.to_string()];
        unit["dice"] = serde_json::to_value(BattleDice::seeded([23; 32])).unwrap();
        unit["crew_recovery"]["dice"] = serde_json::to_value(BattleDice::seeded([13; 32])).unwrap();
        unit["motion"]["heading"] = serde_json::json!(180.0);
        unit["motion"]["desired_heading"] = serde_json::json!(180.0);
        state["recoveries"][pilot.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([11; 32])).unwrap();
        world.btech = serde_json::from_value(state).unwrap();
        units.push(id);
    }
    set_battle_speed(&mut world, units[0], ObjectId(1), 32.25).unwrap();
    (dir, world, [units[0], units[1]])
}

/// Each top-level field that differs between two serialized units, with both values.
fn differing(low: &serde_json::Value, high: &serde_json::Value) -> Vec<String> {
    low.as_object()
        .unwrap()
        .iter()
        .filter(|(key, value)| high.get(key.as_str()) != Some(value))
        .map(|(key, value)| format!("{key}: {value} vs {}", high[key.as_str()]))
        .collect()
}

/// The parts of a unit that must not depend on how high the map sits.
fn outcome(world: &World, id: ObjectId) -> serde_json::Value {
    let mut unit = serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap();
    for height in ["ground_elevation", "flight", "free_fall"] {
        unit.as_object_mut().unwrap().remove(height);
    }
    unit
}

#[tokio::test]
async fn lifting_a_map_lifts_units_without_changing_what_happens() {
    let low = BattleMapAsset::from_cells(LANE).unwrap();
    let high = lifted(&low);
    assert_eq!(high.hex(1, 5).unwrap().water_line(), i16::from(LIFT));
    assert_eq!(
        high.hex(1, 5).unwrap().surface_height(),
        i16::from(LIFT) - 2
    );
    let (_low_dir, mut low_world, units) = field(low).await;
    let (_high_dir, mut high_world, high_units) = field(high).await;
    assert_eq!(units, high_units);
    let mut deepest = 0;
    for tick in 0..240 {
        let low_report = advance_battle_motion(&mut low_world, BattleMovementRules::STANDARD);
        let high_report = advance_battle_motion(&mut high_world, BattleMovementRules::STANDARD);
        assert_eq!(
            format!("{low_report:?}"),
            format!("{high_report:?}"),
            "tick {tick}"
        );
        advance_battle_units(&mut low_world, 0);
        advance_battle_units(&mut high_world, 0);
        for id in units {
            let differing = differing(&outcome(&low_world, id), &outcome(&high_world, id));
            assert!(differing.is_empty(), "tick {tick}: {differing:#?}");
            let low_height = battle_unit_elevation(&low_world, id).unwrap().unwrap();
            let high_height = battle_unit_elevation(&high_world, id).unwrap().unwrap();
            assert_eq!(high_height, low_height + i32::from(LIFT), "tick {tick}");
            deepest = deepest.min(low_height);
        }
        assert_eq!(
            battle_unit_terrain_los(&low_world, units[1], units[0]).unwrap(),
            battle_unit_terrain_los(&high_world, units[1], units[0]).unwrap(),
            "tick {tick}"
        );
    }
    // The walker really went into the water and out the other side.
    assert!(deepest < 0, "the walker never entered the water");
    assert!(
        low_world.btech.constructed_units()[&units[0]]
            .position()
            .unwrap()
            .y
            >= 9
    );
}

/// Line of sight between every pair of lane hexes is the same at either height.
#[tokio::test]
async fn lifting_a_map_keeps_ground_sight() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let low = world.create(&config, "Low".into(), Kind::Room);
    let high = world.create(&config, "High".into(), Kind::Room);
    let asset = BattleMapAsset::from_cells(LANE).unwrap();
    create_battle_map(&mut world, low, "low.map", asset.clone()).unwrap();
    create_battle_map(&mut world, high, "high.map", lifted(&asset)).unwrap();
    let maps = world.btech.maps();
    for from in 0..9 {
        for to in 0..9 {
            let observer = BattleHexCoordinate { x: 1, y: from };
            let target = BattleHexCoordinate { x: 1, y: to };
            assert_eq!(
                ground_terrain_los(&maps[&low], observer, target).unwrap(),
                ground_terrain_los(&maps[&high], observer, target).unwrap(),
                "{from} -> {to}"
            );
        }
    }
}

/// A running vehicle heading east along `row`, repeated over three rows.
async fn vehicle_field(
    template: &str,
    row: &str,
    lift: bool,
) -> (tempfile::TempDir, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Raised crossing".into(), Kind::Room);
    let asset = BattleMapAsset::from_cells(&format!("12 3\n{row}\n{row}\n{row}\n")).unwrap();
    let asset = if lift { lifted(&asset) } else { asset };
    create_battle_map(&mut world, map, "crossing", asset).unwrap();
    let id = world.create(&config, "Vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        BattleVehicleTemplate::parse("test", template).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 1).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    let mut state = serde_json::to_value(&world.btech).unwrap();
    let unit = &mut state["vehicles"][id.0.to_string()];
    unit["dice"] = serde_json::to_value(BattleDice::seeded([29; 32])).unwrap();
    unit["crew_recovery"]["dice"] = serde_json::to_value(BattleDice::seeded([13; 32])).unwrap();
    state["recoveries"]["1"]["dice"] = serde_json::to_value(BattleDice::seeded([11; 32])).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    set_battle_heading(&mut world, id, ObjectId(1), 90.0).unwrap();
    for _ in 0..10 {
        advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
    }
    set_battle_speed(&mut world, id, ObjectId(1), 53.75).unwrap();
    (dir, world, id)
}

/// Hovercraft skim raised water, ice and under-span passages at the lifted water line, and
/// a truck crossing a raised bridge deck or stopping at raised water does what it would at
/// level 0.
#[tokio::test]
async fn lifting_a_map_lifts_vehicles_without_changing_what_happens() {
    let hover = include_str!("../game/mechs/Fulcrum.toml");
    let truck = include_str!("../game/mechs/Flatbed_Truck.toml");
    // The truck halts at the water's edge rather than drive in.
    for (template, row, reach) in [
        (hover, ".0.0~5/3/4/3/2/3~7.0.0.0", 4),
        (hover, ".0.0-2~5/3-1/2~2.0.0.0.0", 4),
        (truck, ".2.2.2/3/3/3/3/3.2.2.2.2", 4),
        (truck, ".0.0~1~2.0.0.0.0.0.0.0.0", 1),
    ] {
        let (_low_dir, mut low, id) = vehicle_field(template, row, false).await;
        let (_high_dir, mut high, high_id) = vehicle_field(template, row, true).await;
        assert_eq!(id, high_id);
        let mut furthest = 0;
        for tick in 0..90 {
            let low_report = advance_battle_motion(&mut low, BattleMovementRules::STANDARD);
            let high_report = advance_battle_motion(&mut high, BattleMovementRules::STANDARD);
            assert_eq!(
                format!("{low_report:?}"),
                format!("{high_report:?}"),
                "{row} tick {tick}"
            );
            advance_battle_units(&mut low, 0);
            advance_battle_units(&mut high, 0);
            let state = |world: &World| {
                let mut unit = serde_json::to_value(&world.btech.vehicles()[&id]).unwrap();
                unit.as_object_mut().unwrap().remove("ground_elevation");
                unit
            };
            let differing = differing(&state(&low), &state(&high));
            assert!(differing.is_empty(), "{row} tick {tick}: {differing:#?}");
            assert_eq!(
                battle_unit_elevation(&high, id)
                    .unwrap()
                    .map(|height| height - i32::from(LIFT)),
                battle_unit_elevation(&low, id).unwrap(),
                "{row} tick {tick}"
            );
            if let Some(position) = low.btech.vehicles()[&id].position() {
                furthest = furthest.max(position.x);
            }
        }
        assert!(
            furthest >= reach,
            "{row}: the vehicle stopped at {furthest}"
        );
    }
}
