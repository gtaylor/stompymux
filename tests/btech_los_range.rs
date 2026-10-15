//! The shared line-of-sight cutoff is daylight's sixty hexes of spatial distance for every chassis.
use crate::support;
use stompymux_rs::*;

/// Unobstructed north/south lane with a constructed observer and a Mech target.
async fn fixture(
    source: &str,
) -> (
    tempfile::TempDir,
    Config,
    World,
    ObjectId,
    ObjectId,
    ObjectId,
) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Long sight lane".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "lane",
        MapAsset::from_cells(&format!("1 201\n{}", ".0\n".repeat(201))).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let observer = world.create(&config, "Observer".into(), Kind::Thing);
    let target = world.create(&config, "Target".into(), Kind::Thing);
    UnitTemplate::parse("test", source)
        .unwrap()
        .create(&mut world, observer)
        .unwrap();
    support::seed_object_dice(&mut world, observer, support::FIXTURE_DICE_SEED);
    UnitTemplate::parse("JR7-D", include_str!("../game/units/JR7-D.toml"))
        .unwrap()
        .create(&mut world, target)
        .unwrap();
    support::seed_object_dice(&mut world, target, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, observer, map, 0, 0).unwrap();
    place_battle_unit(&mut world, target, map, 0, 60).unwrap();
    (dir, config, world, map, observer, target)
}

/// Edit only fixture facts, leaving the live geometry and LOS services under test.
fn edit(world: &mut World, id: ObjectId, update: impl FnOnce(&mut serde_json::Value)) {
    world.btech.rewrite_unit_record(id, update).unwrap();
}

/// Every chassis stops seeing at daylight's sixty hexes, measured spatially, in both directions
/// and without the query changing anything.
#[tokio::test]
async fn all_chassis_share_symmetric_sight_cutoffs_without_mutation() {
    let ground = include_str!("../game/units/Demolisher.toml");
    for source in [
        include_str!("../game/units/JR7-D.toml").to_owned(),
        include_str!("../game/units/GOL-1H.toml").to_owned(),
        ground.to_owned(),
        ground.replace("movement = \"track\"", "movement = \"wheel\""),
        crate::support::templates::hovercraft(ground),
        include_str!("../game/units/RadioTower.toml").to_owned(),
        include_str!("../game/units/Kestrel.toml").to_owned(),
    ] {
        let (_dir, config, mut world, map, observer, target) = fixture(&source).await;
        for distance in [59.999_f64, 60.0, 60.001, 180.0] {
            place_battle_unit(&mut world, target, map, 0, distance.round() as i64).unwrap();
            edit(&mut world, target, |unit| {
                unit["motion"]["point"]["y"] = (0.5 + distance).into()
            });
            let before = world.btech.clone();
            let expected = distance > f64::from(MAX_SIGHT);
            for (a, b) in [(observer, target), (target, observer)] {
                assert_eq!(
                    battle_unit_terrain_los(&world, a, b).unwrap().blocked,
                    expected,
                    "distance {distance}"
                );
            }
            assert_eq!(world.btech, before);
        }
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            battle_unit_terrain_los(&world, observer, target).unwrap(),
            battle_unit_terrain_los(&restored, observer, target).unwrap()
        );
    }
}

/// Height counts toward the spatial cutoff, and two endpoints high above the terrain still
/// obey it before the clear-sky shortcut skips the terrain walk.
#[tokio::test]
async fn spatial_height_and_sight_cutoff_apply_before_the_high_altitude_shortcut() {
    let (_dir, _config, mut world, map, observer, target) =
        fixture(include_str!("../game/units/JR7-D.toml")).await;
    place_battle_unit(&mut world, target, map, 0, 0).unwrap();
    // Five levels of height weigh as one hex of range: three hundred levels reach the cutoff.
    for height in [299.999, 300.0, 300.001] {
        edit(&mut world, target, |unit| {
            unit["ground_elevation"] = height.into()
        });
        assert_eq!(
            battle_unit_terrain_los(&world, observer, target)
                .unwrap()
                .blocked,
            height > 300.0
        );
    }
    // Both endpoints above terrain must still obey the cutoff before the clear-sky shortcut.
    edit(&mut world, observer, |unit| {
        unit["ground_elevation"] = 20.0.into()
    });
    for y in [60, 61] {
        place_battle_unit(&mut world, target, map, 0, y).unwrap();
        edit(&mut world, target, |unit| {
            unit["ground_elevation"] = 20.0.into()
        });
        let los = battle_unit_terrain_los(&world, observer, target).unwrap();
        assert_eq!(los.blocked, y > i64::from(MAX_SIGHT), "y {y}");
        assert!(!los.hills, "no terrain is traced above the shortcut height");
    }
}
