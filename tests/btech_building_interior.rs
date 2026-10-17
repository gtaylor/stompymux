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
    // A leg hit strikes the building the target stands in (Total Warfare p. 171).
    assert_eq!(terrain.cover_building, Some(HexCoordinate { x: 1, y: 2 }));
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

/// A leg hit on a 'Mech that the building it stands in covers strikes that building instead,
/// damaging it as normal (Total Warfare p. 171): the building pays its tenth of the
/// construction factor and the rest of the shot too, and the legs are untouched.
#[tokio::test]
async fn covered_leg_hits_strike_the_building_the_target_stands_in() {
    let (_dir, _config, base, map, [attacker, target]) =
        field(&[".3.3.3", ".0.0.0", "@1@1@1", ".0.0.0"]).await;
    let mut base = base;
    relocate(&mut base, attacker, map, 1, 0);
    relocate(&mut base, target, map, 1, 2);
    step_inside(&mut base, target, 0);
    refresh_battle_contacts(&mut base, &[attacker]).unwrap();
    let rules = ShotRules {
        tsm_tow_bonus: true,
        vehicle_impact: VehicleImpactRules::STANDARD,
        aim: AimRules {
            override_weapon_arcs: true,
        },
        extended_gunnery: false,
        extended_piloting: false,
        target_toughness: false,
    };
    let laser = base.btech.constructed_units()[&attacker]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == Weapon::MediumLaser)
        .unwrap();
    let hit = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 12)
        .unwrap();
    base.btech
        .set_unit_dice(attacker, Dice::seeded([hit; 32]))
        .unwrap();
    let before = cf(&base, map, 1, 2);
    let legs = |world: &World| {
        let unit = &world.btech.constructed_units()[&target];
        [MechSection::LeftLeg, MechSection::RightLeg].map(|leg| unit.sections()[&leg].clone())
    };
    let covered = (0..=255).find_map(|seed| {
        let mut world = base.clone();
        world
            .btech
            .set_unit_dice(target, Dice::seeded([seed; 32]))
            .unwrap();
        let shot =
            resolve_battle_shot(&mut world, attacker, ObjectId(1), target, laser, rules).unwrap();
        let Some(TargetSalvo::Mech(salvo)) = &shot.salvo else {
            return None;
        };
        let group = salvo.groups.first()?;
        let leg = matches!(
            group.hit.section,
            MechSection::LeftLeg | MechSection::RightLeg
        );
        leg.then_some((world, group.damage))
    });
    let (world, damage) = covered.expect("some seed hits a leg");
    assert_eq!(
        damage, 1,
        "the building absorbs four of the laser's five points"
    );
    assert_eq!(cf(&world, map, 1, 2), before - 5);
    assert_eq!(legs(&world), legs(&base));
}

/// A shot from another level of the same building hex hits a 'Mech on the Special 'Mech Hit
/// Location Table (Total Warfare p. 175): from above it never finds a leg, and from below
/// never an arm or the head. Fire inside the same hex is not shielded by the building.
#[tokio::test]
async fn shots_between_building_levels_use_the_special_hit_location_table() {
    let (_dir, _config, base, _map, [shooter, target]) =
        field(&[".0.0.0", "@4@4@4", ".0.0.0"]).await;
    let rules = ShotRules {
        tsm_tow_bonus: true,
        vehicle_impact: VehicleImpactRules::STANDARD,
        aim: AimRules {
            override_weapon_arcs: true,
        },
        extended_gunnery: false,
        extended_piloting: false,
        target_toughness: false,
    };
    use MechSection::*;
    for (shooter_level, target_level, allowed) in [
        (
            2,
            0,
            &[LeftArm, LeftTorso, CenterTorso, RightTorso, RightArm, Head][..],
        ),
        (0, 2, &[LeftLeg, LeftTorso, RightTorso, RightLeg][..]),
    ] {
        let mut world = base.clone();
        step_inside(&mut world, shooter, shooter_level);
        step_inside(&mut world, target, target_level);
        refresh_battle_contacts(&mut world, &[shooter]).unwrap();
        assert!(
            visible_battle_contact(&world, shooter, target)
                .unwrap()
                .is_some()
        );
        let mut hits = 0;
        for seed in 0..=63 {
            let mut trial = world.clone();
            trial
                .btech
                .set_unit_dice(shooter, Dice::seeded([seed; 32]))
                .unwrap();
            trial
                .btech
                .set_unit_dice(target, Dice::seeded([seed; 32]))
                .unwrap();
            let report =
                resolve_battle_shot(&mut trial, shooter, ObjectId(1), target, 0, rules).unwrap();
            let Some(TargetSalvo::Mech(salvo)) = report.salvo else {
                continue;
            };
            assert!(salvo.building.is_none());
            for group in &salvo.groups {
                assert!(
                    allowed.contains(&group.hit.section),
                    "{shooter_level} over {target_level}: {:?}",
                    group.hit.section
                );
                assert!(!group.hit.through_armor_critical);
                hits += 1;
            }
        }
        assert!(hits > 4, "{shooter_level} over {target_level}: {hits} hits");
    }
}

/// Make the building hex at `x`, `y` one of `class` with construction factor `cf`.
fn rebuild(
    world: &mut World,
    map: ObjectId,
    (x, y): (i64, i64),
    class: ConstructionClass,
    cf: u16,
) {
    let tile = world.btech.maps()[&map].base_hex(x, y).unwrap();
    let structure = tile.structure().unwrap();
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    let width = world.btech.maps()[&map].width;
    encoded["maps"][map.0.to_string()]["terrain"][(y * width + x) as usize] =
        serde_json::to_value(tile.with_structure(Some(Structure {
            class,
            cf,
            ..structure
        })))
        .unwrap();
    world.btech = serde_json::from_value(encoded).unwrap();
}

/// Land a Thumper round from the north on (1, 1) and return its report.
fn shell(world: &mut World, map: ObjectId) -> ArtilleryImpactReport {
    let mut flight = ArtilleryFlight::new(
        HexCoordinate { x: 1, y: 0 },
        HexCoordinate { x: 1, y: 1 },
        Weapon::Thumper,
        ArtilleryMode::Standard,
        ArtilleryOutcome::Hit,
    )
    .unwrap();
    loop {
        if let Some(report) = advance_artillery_flight(world, map, &mut flight, rules()).unwrap() {
            return report;
        }
    }
}

/// The blast damage the report dealt `id`, or zero when it was spared.
fn blast(report: &ArtilleryImpactReport, id: ObjectId) -> u16 {
    report
        .hits
        .iter()
        .filter(|hit| hit.unit == id)
        .map(|hit| hit.damage)
        .sum()
}

/// Artillery striking a building hex damages every unit inside a light, medium or heavy
/// building in full, whatever its floor, and then the building itself as an area-effect
/// weapon striking a hex and its neighbours: three times the Damage Value, twice for the
/// level below the roof and once for the next (Tactical Operations: Advanced Rules p. 152;
/// Total Warfare p. 174).
#[tokio::test]
async fn artillery_reaches_every_floor_and_damages_the_building() {
    let (_dir, _config, mut world, map, [inside, roof]) =
        field(&[".0.0.0", "@3@3@3", ".0.0.0"]).await;
    rebuild(&mut world, map, (1, 1), ConstructionClass::Heavy, 90);
    step_inside(&mut world, inside, 0);
    let report = shell(&mut world, map);
    // A Thumper round is fifteen points in the struck hex (p. 152), on the ground floor of a
    // three-level building as on its roof.
    assert_eq!(blast(&report, inside), 15);
    assert_eq!(blast(&report, roof), 15);
    assert_eq!(
        report.structures[0].coordinate,
        HexCoordinate { x: 1, y: 1 }
    );
    // Three times fifteen, twice for the level below the roof and once for the next: ninety,
    // which brings the heavy building down.
    assert_eq!(report.structures[0].damage, 90);
    assert_eq!(report.structures[0].cf, 0);
    assert!(report.structures[0].collapse.is_some());
    // The medium building hexes beside it take twice the five-point ring, and once more for
    // their level below the roof.
    let beside = report
        .structures
        .iter()
        .find(|hit| hit.coordinate == HexCoordinate { x: 0, y: 1 })
        .unwrap();
    assert_eq!(beside.damage, 15);
    assert_eq!(cf(&world, map, 0, 1), 40 - 15);
}

/// Inside a hardened building the round bursts on the roof: the roof and the level below take
/// the full Damage Value, the next level half, and the floors below nothing (Total Warfare
/// p. 173).
#[tokio::test]
async fn hardened_buildings_shelter_their_lower_floors() {
    for (level, expected) in [(0, 0), (1, 8), (2, 15)] {
        let (_dir, _config, mut world, map, [inside, _]) =
            field(&[".0.0.0", "@3@3@3", ".0.0.0"]).await;
        rebuild(&mut world, map, (1, 1), ConstructionClass::Hardened, 150);
        step_inside(&mut world, inside, level);
        let report = shell(&mut world, map);
        assert_eq!(blast(&report, inside), expected, "{level}");
        assert_eq!(cf(&world, map, 1, 1), 150 - 90);
    }
}
