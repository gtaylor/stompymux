//! Units inside building hexes on the battlefield (Total Warfare pp. 166–177): entering below
//! the roof, roofs from roof height, the building's share of incoming fire, collapse onto the
//! units inside, partial cover one level below the roof and the physical-attack prohibition.
use crate::support;
use stompymux_rs::*;

/// Two running Jenners at (1, 1) on a three-hex-wide field built from per-row notation.
async fn field(rows: &[&str]) -> (tempfile::TempDir, Config, World, ObjectId, [ObjectId; 2]) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Block".into(), Kind::Room);
    let source = format!("3 {}\n{}\n", rows.len(), rows.join("\n"));
    create_battle_map(
        &mut world,
        map,
        "block.map",
        MapAsset::from_cells(&source).unwrap(),
    )
    .unwrap();
    let mut units = Vec::new();
    for pilot in [ObjectId(1), ObjectId(2)] {
        let id = world.create(&config, format!("Jenner {}", pilot.0), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        create_battle_unit(
            &mut world,
            id,
            MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap(),
        )
        .unwrap();
        place_battle_unit(&mut world, id, map, 1, 1).unwrap();
        world.objects.get_mut(&pilot).unwrap().location = Some(id);
        assign_battle_pilot(&mut world, id, pilot).unwrap();
        start_battle_unit(&mut world, id, pilot, true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0, true);
        }
        units.push(id);
    }
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    (dir, config, world, map, [units[0], units[1]])
}

/// Ordinary tactical fall rules.
fn rules() -> FallRules {
    FallRules {
        vehicle_impact: VehicleImpactRules::STANDARD,
        extended_piloting: false,
        toughness: false,
    }
}

/// Walk `id` south at walking speed until it changes row, returning the notices of that tick.
fn walk_one_row(world: &mut World, id: ObjectId) -> Vec<Notice> {
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["motion"]["heading"] = serde_json::json!(180.0);
            record["motion"]["desired_heading"] = serde_json::json!(180.0);
        })
        .unwrap();
    set_battle_speed(world, id, ObjectId(1), 21.5).unwrap();
    let start = world.btech.constructed_units()[&id].position().unwrap().y;
    // Inside a medium building a walker crawls at a third of its pace.
    for _ in 0..200 {
        let notices = advance_battle_motion(world, MovementRules::STANDARD).unwrap();
        if world.btech.constructed_units()[&id].position().unwrap().y != start {
            return notices;
        }
    }
    panic!("unit never left row {start}");
}

/// Move a running unit administratively, which needs it shut down for the moment.
fn relocate(world: &mut World, id: ObjectId, map: ObjectId, x: i64, y: i64) {
    world.btech.set_unit_power(id, Power::Off).unwrap();
    place_battle_unit(world, id, map, x, y).unwrap();
    world.btech.set_unit_power(id, Power::Running).unwrap();
}

/// Put `id` inside the building at its hex, standing on `level`.
fn step_inside(world: &mut World, id: ObjectId, level: i32) {
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["ground_elevation"] = serde_json::json!(level);
        })
        .unwrap();
    assert_eq!(
        battle_unit_inside_building(world, id)
            .unwrap()
            .unwrap()
            .level,
        level
    );
}

/// The construction factor of the building hex at `x`, `y`.
fn cf(world: &World, map: ObjectId, x: i64, y: i64) -> u16 {
    world.btech.maps()[&map]
        .base_hex(x, y)
        .unwrap()
        .structure()
        .map_or(0, |structure| structure.cf)
}

/// A 'Mech at ground level walks into a two-level building at its ground floor, pushing
/// through the walls under a piloting roll and costing the hex a tenth of its tonnage; a
/// 'Mech arriving at roof height crosses the roof like a hill.
#[tokio::test]
async fn ground_entry_breaches_walls_and_roof_height_crosses_the_roof() {
    let (_dir, config, mut world, map, [walker, _]) =
        field(&[".0.0.0", ".0.0.0", "@2@2@2", "@2@2@2", ".0.0.0"]).await;
    assert_eq!(cf(&world, map, 1, 2), 40);
    let notices = walk_one_row(&mut world, walker);
    assert!(
        notices
            .iter()
            .any(|notice| notice.text == "You push through the walls of the medium building."),
        "{notices:?}"
    );
    let inside = battle_unit_inside_building(&world, walker)
        .unwrap()
        .expect("inside the building");
    assert_eq!(
        (inside.coordinate, inside.level),
        (HexCoordinate { x: 1, y: 2 }, 0)
    );
    assert_eq!(battle_unit_elevation(&world, walker).unwrap(), Some(0));
    // Jenner: 35 tons cost the hex four points of construction factor.
    assert_eq!(cf(&world, map, 1, 2), 36);
    assert!(inside_building(
        world.btech.maps()[&map].base_hex(1, 2).unwrap(),
        0
    ));
    // The next building hex is another breach; the open ground beyond is left at ground level.
    let notices = walk_one_row(&mut world, walker);
    assert!(
        notices
            .iter()
            .any(|notice| notice.text.contains("push through the walls"))
    );
    assert_eq!(cf(&world, map, 1, 3), 36);
    walk_one_row(&mut world, walker);
    assert_eq!(
        world.btech.constructed_units()[&walker]
            .position()
            .unwrap()
            .y,
        4
    );
    assert!(
        battle_unit_inside_building(&world, walker)
            .unwrap()
            .is_none()
    );
    assert_eq!(battle_unit_elevation(&world, walker).unwrap(), Some(0));
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );

    // From a two-level hill the same building is crossed on its roof, with no roll and no
    // damage to it.
    let (_dir, _config, mut world, map, [walker, _]) =
        field(&[".2.2.2", ".2.2.2", "@2@2@2", ".0.0.0"]).await;
    let notices = walk_one_row(&mut world, walker);
    assert!(notices.is_empty(), "{notices:?}");
    assert_eq!(battle_unit_elevation(&world, walker).unwrap(), Some(2));
    assert!(
        battle_unit_inside_building(&world, walker)
            .unwrap()
            .is_none()
    );
    assert_eq!(cf(&world, map, 1, 2), 40);
    assert_eq!(
        building_entry_level(world.btech.maps()[&map].base_hex(1, 2).unwrap(), 2),
        2
    );
    assert_eq!(
        building_entry_level(world.btech.maps()[&map].base_hex(1, 2).unwrap(), 1),
        1
    );
    assert_eq!(building_floor_cost(ConstructionClass::Medium), 2);
}

/// Fire at a unit inside a building loses a tenth of the construction factor from every hit
/// grouping, the building takes the same, and a building the salvo exhausts comes down on the
/// unit inside once the groupings have landed.
#[tokio::test]
async fn buildings_absorb_fire_and_collapse_onto_their_occupants() {
    let (_dir, _config, mut world, map, [target, _]) = field(&[".0.0.0", "@3@3@3", ".0.0.0"]).await;
    step_inside(&mut world, target, 0);
    let armor_before = world.btech.constructed_units()[&target].sections().clone();
    let report = resolve_battle_tactical_salvo(
        &mut world,
        target,
        Weapon::MediumLaser,
        HitArc::Front,
        rules(),
    )
    .unwrap();
    let cover = report.building.expect("the building covered the target");
    assert_eq!(
        cover,
        BuildingCover {
            coordinate: HexCoordinate { x: 1, y: 1 },
            absorbed: 4,
            groups: 1,
            damage: 4,
            remaining: 36,
        }
    );
    assert!(report.collapse.is_none());
    assert_eq!(report.groups.len(), 1);
    assert_eq!(report.groups[0].damage, 1);
    assert_eq!(cf(&world, map, 1, 1), 36);
    assert_ne!(
        world.btech.constructed_units()[&target].sections(),
        &armor_before
    );
    // A cluster weapon loses the share from every grouping; two-point missiles are absorbed
    // whole and never roll a location, though the hex still pays for each of them.
    let report =
        resolve_battle_tactical_salvo(&mut world, target, Weapon::Srm2, HitArc::Front, rules())
            .unwrap();
    let cover = report.building.expect("still inside");
    assert_eq!(cover.absorbed, 4);
    assert!(cover.groups >= 1);
    assert!(report.groups.is_empty(), "{:?}", report.groups);
    assert_eq!(cf(&world, map, 1, 1), 36 - 4 * cover.groups);

    // A nearly spent hex gives way: the unit inside takes the rubble and stands on it.
    let (_dir, config, mut world, map, [target, roof]) =
        field(&[".0.0.0", "@3@3@3", ".0.0.0"]).await;
    step_inside(&mut world, target, 0);
    world
        .btech
        .rewrite_map_record(map, |record| {
            support::edit_hex(&mut record["terrain"][4], |hex| {
                hex.with_structure_damage(39)
            });
        })
        .unwrap();
    assert_eq!(cf(&world, map, 1, 1), 1);
    let before = world.btech.constructed_units()[&target].sections().clone();
    let report = resolve_battle_tactical_salvo(
        &mut world,
        target,
        Weapon::MediumLaser,
        HitArc::Front,
        rules(),
    )
    .unwrap();
    let cover = report.building.unwrap();
    assert_eq!((cover.absorbed, cover.remaining), (1, 0));
    assert_eq!(report.groups[0].damage, 4);
    let collapse = report.collapse.expect("the hex collapsed");
    // The unit inside takes the rubble; the one on the roof takes its share and drops with it
    // (Total Warfare pp. 176–177).
    assert_eq!(collapse.buried, vec![target, roof]);
    assert!(
        collapse
            .notices
            .iter()
            .any(|notice| notice.text == "The building comes down on top of you for 1 points!")
    );
    let tile = world.btech.maps()[&map].base_hex(1, 1).unwrap();
    assert!(tile.structure().is_none());
    assert_eq!(tile.ground(), Ground::Rubble);
    assert!(
        battle_unit_inside_building(&world, target)
            .unwrap()
            .is_none()
    );
    assert_eq!(battle_unit_elevation(&world, target).unwrap(), Some(0));
    assert_ne!(world.btech.constructed_units()[&target].sections(), &before);
    assert_eq!(
        building_collapse_damage(90, 2),
        27,
        "ninety points over three levels bury the ground floor for twenty-seven"
    );
    world.validate(&config).unwrap();
}

/// A standing 'Mech inside a building one level below its roof has partial cover from any
/// height, and nothing may be punched or kicked from outside its hex.
#[tokio::test]
async fn interiors_give_partial_cover_and_block_outside_physical_attacks() {
    let (_dir, _config, mut world, _map, [attacker, target]) =
        field(&[".3.3.3", ".0.0.0", "@1@1@1", ".0.0.0"]).await;
    // The attacker looks down from a three-level hill at the target on the building's roof...
    relocate(&mut world, attacker, _map, 1, 0);
    relocate(&mut world, target, _map, 1, 2);
    assert!(
        !battle_unit_terrain_los(&world, attacker, target)
            .unwrap()
            .partial_cover
    );
    // ...and once the target is inside, its upper body alone shows above the roof.
    step_inside(&mut world, target, 0);
    let terrain = battle_unit_terrain_los(&world, attacker, target).unwrap();
    assert!(terrain.partial_cover && !terrain.blocked);
    let physical = PhysicalRules { fall: rules() };
    relocate(&mut world, attacker, _map, 1, 1);
    step_inside(&mut world, target, 0);
    let error = battle_punch_profile(&world, attacker, ObjectId(1), target, Arm::Left, physical)
        .unwrap_err()
        .to_string();
    assert!(error.contains("inside a building"), "{error}");
    // Sharing the hex, the attacker reaches it like any other target.
    relocate(&mut world, attacker, _map, 1, 2);
    step_inside(&mut world, attacker, 0);
    step_inside(&mut world, target, 0);
    let result = battle_punch_profile(&world, attacker, ObjectId(1), target, Arm::Left, physical);
    assert!(
        result
            .as_ref()
            .err()
            .is_none_or(|error| !error.to_string().contains("inside a building")),
        "{result:?}"
    );
}
