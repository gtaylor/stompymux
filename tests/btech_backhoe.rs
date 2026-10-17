//! A vehicle's backhoe digs into an adjacent building hex and never attacks a unit: the blow
//! always lands, for six points plus 1D6, and a building that runs out of construction factor
//! collapses (Total Warfare pp. 130, 146, 171). A hovercraft digs only at a standstill
//! (TechManual p. 241).
use crate::support;
use stompymux_rs::*;

/// The shipped Engineering vehicle, a tracked 40-tonner with a turret backhoe.
const ENGINEERING: &str = include_str!("../game/units/Engineering.toml");

/// A running `source` vehicle at (1,1), facing north, below a one-level medium building at
/// (1,0) with 40 construction factor.
async fn fixture(source: &str) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    fixture_at(source, 1).await
}

/// The [`fixture`] with the vehicle at (1,`y`).
async fn fixture_at(
    source: &str,
    y: i64,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Building site".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "site",
        MapAsset::from_cells("3 3\n.0@1.0\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Digger".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    let template =
        support::templates::fitted(VehicleTemplate::parse("Engineering", source).unwrap());
    create_battle_vehicle(&mut world, id, template).unwrap();
    place_battle_unit(&mut world, id, map, 1, y).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0, true);
    }
    (dir, config, world, map, id)
}

/// The configured fall policy a server tick would use.
fn rules() -> FallRules {
    FallRules {
        vehicle_impact: VehicleImpactRules::STANDARD,
        extended_piloting: true,
        toughness: false,
    }
}

/// The construction factor left in the building hex at (1,0), or `None` once it is gone.
fn building_cf(world: &World, map: ObjectId) -> Option<u16> {
    world.btech.maps()[&map]
        .hex(1, 0)
        .unwrap()
        .structure()
        .map(|structure| structure.cf)
}

/// The blow needs no roll and takes six plus 1D6 off the building; the vehicle may dig once a
/// turn, and its weapons are not tied up by it.
#[tokio::test]
async fn backhoes_dig_into_adjacent_buildings_once_a_turn() {
    let (_dir, config, mut world, map, id) = fixture(ENGINEERING).await;
    assert_eq!(building_cf(&world, map), Some(40));
    let report = resolve_battle_backhoe(
        &mut world,
        id,
        ObjectId(1),
        HexCoordinate { x: 1, y: 0 },
        rules(),
    )
    .unwrap();
    assert!((1..=6).contains(&report.roll));
    assert_eq!(report.damage, BACKHOE_DAMAGE + report.roll);
    assert_eq!(report.cf, 40 - report.damage);
    assert_eq!(building_cf(&world, map), Some(report.cf));
    assert!(report.collapse.is_none());
    assert_eq!(
        world.btech.vehicles()[&id].last_physical_attack(),
        Some(world.btech.simulation_time())
    );
    let again = resolve_battle_backhoe(
        &mut world.clone(),
        id,
        ObjectId(1),
        HexCoordinate { x: 1, y: 0 },
        rules(),
    )
    .unwrap_err()
    .to_string();
    assert!(again.contains("already made a physical attack"), "{again}");
    world.validate(&config).unwrap();

    // The native command digs into the locked hex, or the one it names.
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(fixture(ENGINEERING).await.2)),
    )
    .unwrap();
    let text = support::run_text(&scripts, &config, ObjectId(1), 1, "backhoe 1 0");
    assert!(
        text.contains("You dig your backhoe into the building at 1,0 for"),
        "{text}"
    );
    let text = support::run_text(&scripts, &config, ObjectId(1), 1, "backhoe 1");
    assert!(text.contains("Usage: backhoe [<x> <y>]"), "{text}");
}

/// A building the blow takes to no construction factor collapses into rubble.
#[tokio::test]
async fn a_backhoe_brings_down_a_worn_building() {
    let (_dir, config, mut world, map, id) = fixture(ENGINEERING).await;
    world
        .btech
        .rewrite_map_record(map, |record| {
            record["terrain"][1]["structure"]["cf"] = 5.into();
        })
        .unwrap();
    assert_eq!(building_cf(&world, map), Some(5));
    let report = resolve_battle_backhoe(
        &mut world,
        id,
        ObjectId(1),
        HexCoordinate { x: 1, y: 0 },
        rules(),
    )
    .unwrap();
    assert_eq!(report.cf, 0);
    assert!(report.collapse.is_some());
    assert_eq!(building_cf(&world, map), None);
    assert_eq!(
        world.btech.maps()[&map].hex(1, 0).unwrap().ground(),
        Ground::Rubble
    );
    world.validate(&config).unwrap();
}

/// The building must stand in an adjacent hex the backhoe faces, and a hovercraft must be at a
/// standstill.
#[tokio::test]
async fn backhoes_need_a_faced_adjacent_building() {
    let (_dir, _config, base, _, id) = fixture(ENGINEERING).await;
    let dig = |world: &World, x, y| {
        resolve_battle_backhoe(
            &mut world.clone(),
            id,
            ObjectId(1),
            HexCoordinate { x, y },
            rules(),
        )
        .unwrap_err()
        .to_string()
    };
    assert!(dig(&base, 0, 1).contains("no building at 0,1"));
    let (_far_dir, _config, far, _, far_id) = fixture_at(ENGINEERING, 2).await;
    let refused = resolve_battle_backhoe(
        &mut far.clone(),
        far_id,
        ObjectId(1),
        HexCoordinate { x: 1, y: 0 },
        rules(),
    )
    .unwrap_err()
    .to_string();
    assert!(refused.contains("adjacent hex"), "{refused}");
    let mut turned = base.clone();
    set_battle_turret(&mut turned, id, ObjectId(1), 180.0).unwrap();
    assert!(dig(&turned, 1, 0).contains("does not face"));

    let hover = ENGINEERING.replace("movement = \"track\"", "movement = \"hover\"");
    let (_dir, _config, mut world, _, id) = fixture(&hover).await;
    set_battle_speed(&mut world, id, ObjectId(1), 21.5).unwrap();
    let moving = resolve_battle_backhoe(
        &mut world.clone(),
        id,
        ObjectId(1),
        HexCoordinate { x: 1, y: 0 },
        rules(),
    )
    .unwrap_err()
    .to_string();
    assert!(moving.contains("must stop"), "{moving}");
    set_battle_speed(&mut world, id, ObjectId(1), 0.0).unwrap();
    resolve_battle_backhoe(
        &mut world,
        id,
        ObjectId(1),
        HexCoordinate { x: 1, y: 0 },
        rules(),
    )
    .unwrap();
}
