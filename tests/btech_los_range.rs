//! Shared LOS ceilings use spatial distance and radar installation on either endpoint.
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
        BattleMapAsset::from_cells(&format!("1 201\n{}", ".0\n".repeat(201))).unwrap(),
    )
    .unwrap();
    let observer = world.create(&config, "Observer".into(), Kind::Thing);
    let target = world.create(&config, "Target".into(), Kind::Thing);
    BattleUnitTemplate::parse("test", source)
        .unwrap()
        .create(&mut world, observer)
        .unwrap();
    BattleUnitTemplate::parse("JR7-D", include_str!("../game/mechs/JR7-D.toml"))
        .unwrap()
        .create(&mut world, target)
        .unwrap();
    place_battle_unit(&mut world, observer, map, 0, 0).unwrap();
    place_battle_unit(&mut world, target, map, 0, 60).unwrap();
    (dir, config, world, map, observer, target)
}

/// Edit only fixture facts, leaving the live geometry and LOS services under test.
fn edit(world: &mut World, id: ObjectId, update: impl FnOnce(&mut serde_json::Value)) {
    let class = if world.btech.vehicles().contains_key(&id) {
        "vehicles"
    } else {
        "constructed"
    };
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    update(&mut encoded[class][id.0.to_string()]);
    world.btech = serde_json::from_value(encoded).unwrap();
}

#[tokio::test]
async fn all_chassis_share_symmetric_map_and_radar_cutoffs_without_mutation() {
    let ground = include_str!("../game/mechs/Demolisher.toml");
    for source in [
        include_str!("../game/mechs/JR7-D.toml").to_owned(),
        include_str!("../game/mechs/GOL-1H.toml").to_owned(),
        ground.to_owned(),
        ground.replace("movement = \"track\"", "movement = \"wheel\""),
        ground.replace("movement = \"track\"", "movement = \"hover\""),
        include_str!("../game/mechs/RadioTower.toml").to_owned(),
        include_str!("../game/mechs/Kestrel.toml").to_owned(),
    ] {
        let (_dir, config, base, map, observer, target) = fixture(&source).await;
        for radar_endpoint in [None, Some(observer), Some(target)] {
            let mut world = base.clone();
            // RadioTower is normally radar-equipped; make each installation case explicit.
            for id in [observer, target] {
                edit(&mut world, id, |unit| {
                    let flags = unit["definition"]["attributes"]["specials"]
                        .as_str()
                        .unwrap_or("")
                        .split_whitespace()
                        .filter(|flag| *flag != "-" && !flag.eq_ignore_ascii_case("AntiAircraft"))
                        .collect::<Vec<_>>()
                        .join(" ");
                    unit["definition"]["attributes"]["specials"] = if Some(id) == radar_endpoint {
                        format!("{flags} AntiAircraft")
                    } else if flags.is_empty() {
                        "-".into()
                    } else {
                        flags
                    }
                    .into();
                });
            }
            for distance in [59.999_f64, 60.0, 60.001, 179.999, 180.0, 180.001] {
                place_battle_unit(&mut world, target, map, 0, distance.round() as i64).unwrap();
                edit(&mut world, target, |unit| {
                    unit["motion"]["point"]["y"] = (0.5 + distance).into()
                });
                let before = world.btech.clone();
                let expected = distance
                    > if radar_endpoint.is_some() {
                        180.0
                    } else {
                        60.0
                    };
                for (a, b) in [(observer, target), (target, observer)] {
                    assert_eq!(
                        battle_unit_terrain_los(&world, a, b).unwrap().blocked,
                        expected,
                        "distance {distance}, radar {radar_endpoint:?}"
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
}

#[tokio::test]
async fn spatial_height_and_live_map_ceiling_apply_before_the_high_altitude_shortcut() {
    let (_dir, _config, mut world, map, observer, target) =
        fixture(include_str!("../game/mechs/JR7-D.toml")).await;
    place_battle_unit(&mut world, target, map, 0, 0).unwrap();
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
    // Both endpoints above terrain must still obey the map cutoff before the clear-sky shortcut.
    edit(&mut world, observer, |unit| {
        unit["ground_elevation"] = 20.0.into()
    });
    edit(&mut world, target, |unit| {
        unit["ground_elevation"] = 20.0.into()
    });
    place_battle_unit(&mut world, target, map, 0, 11).unwrap();
    edit(&mut world, target, |unit| {
        unit["ground_elevation"] = 20.0.into()
    });
    for maximum in [11, 10] {
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        encoded["maps"][map.0.to_string()]["maximum_visibility"] = maximum.into();
        world.btech = serde_json::from_value(encoded).unwrap();
        assert_eq!(
            battle_unit_terrain_los(&world, observer, target)
                .unwrap()
                .blocked,
            maximum == 10
        );
    }
}
