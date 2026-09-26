//! Scenario IDs stay independent of membership and survive collision resolution and restart.
use crate::support;
use crate::support::btech_firing as firing;
use stompymux_rs::*;

/// Resolve the shared identity without depending on chassis storage.
fn identity(world: &World, id: ObjectId) -> String {
    world
        .btech
        .vehicles()
        .get(&id)
        .map_or_else(
            || world.btech.constructed_units()[&id].battlefield_id(),
            |unit| unit.battlefield_id(),
        )
        .unwrap()
}

#[tokio::test]
async fn authored_ids_preserve_membership_and_replay_collisions_for_every_chassis() {
    for source in firing::templates() {
        let (_dir, config, mut world, unit, other, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let before = serde_json::to_value(&world.btech).unwrap();
        assert_eq!(
            assign_battlefield_id(&mut world, unit, Some("z9ignored")).unwrap(),
            "ZA"
        );
        assert_eq!(identity(&world, unit), "ZA");
        let key = if world.btech.vehicles().contains_key(&unit) {
            "vehicles"
        } else {
            "constructed"
        };
        let after = serde_json::to_value(&world.btech).unwrap();
        for field in [
            "map_slot",
            "position",
            "motion",
            "pilot",
            "dice",
            "target_lock",
        ] {
            assert_eq!(
                before[key][unit.0.to_string()][field],
                after[key][unit.0.to_string()][field],
                "{field}"
            );
        }
        for preference in [Some("ZA"), None, Some("x")] {
            let mut replay = world.clone();
            let assigned = assign_battlefield_id(&mut world, other, preference).unwrap();
            assert_ne!(assigned, "ZA");
            assert_eq!(
                assign_battlefield_id(&mut replay, other, preference).unwrap(),
                assigned
            );
            assert_eq!(world.btech, replay.btech);
        }
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
        for label in ["ZA", "aA", "A", "AB!", "ABCDEFGH"] {
            let mut invalid = world.clone();
            firing::edit(&mut invalid, other, |state| {
                state["battlefield_label"] = label.into()
            });
            assert!(invalid.validate(&config).is_err(), "{label}");
        }
        let before = world.btech.clone();
        assert!(assign_battlefield_id(&mut world, ObjectId(1), None).is_err());
        assert_eq!(before, world.btech);
    }
}

#[tokio::test]
async fn normal_placement_avoids_authored_ids_without_reordering_membership() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Battlefield".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "ids",
        BattleMapAsset::parse("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let mut units = Vec::new();
    for source in firing::templates() {
        let unit = world.create(&config, "Unit".into(), Kind::Thing);
        world.objects.get_mut(&unit).unwrap().home = Some(ObjectId(config.home()));
        BattleUnitTemplate::parse(&source)
            .unwrap()
            .create(&mut world, unit)
            .unwrap();
        place_battle_unit(&mut world, unit, map, 0, 0).unwrap();
        units.push(unit);
        if units.len() == 1 {
            assign_battlefield_id(&mut world, unit, Some("AB")).unwrap();
        }
    }
    assert_eq!(identity(&world, units[0]), "AB");
    assert_eq!(identity(&world, units[1]), "AA");
    for (slot, &unit) in units.iter().enumerate() {
        let actual = world.btech.vehicles().get(&unit).map_or_else(
            || world.btech.constructed_units()[&unit].map_slot(),
            |unit| unit.map_slot(),
        );
        assert_eq!(actual, Some(slot as u32));
        let label = identity(&world, unit);
        place_battle_unit(&mut world, unit, map, 0, 0).unwrap();
        assert_eq!(identity(&world, unit), label);
    }
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}
