//! Vehicle observations retain acquisition and reconcile cross-class placement and object cleanup.
use crate::support;
use stompymux_rs::*;

/// Two Mechs and two vehicles at opposite ends of a north/south lane.
async fn fixture(
    tiles: &str,
    vehicle: &str,
) -> (tempfile::TempDir, Config, World, ObjectId, [ObjectId; 4]) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Sight lane".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "sight",
        MapAsset::from_cells(&format!("1 5\n{tiles}")).unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for index in 0..4 {
        let id = world.create(&config, format!("Unit {index}"), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        if index < 2 {
            let mut definition =
                MechTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml"))
                    .unwrap();
            definition
                .attributes
                .insert("specials".into(), "FlipArms Searchlight".into());
            create_battle_unit(&mut world, id, definition).unwrap();
        } else {
            create_battle_vehicle(
                &mut world,
                id,
                VehicleTemplate::parse("test", vehicle).unwrap(),
            )
            .unwrap();
        }
        place_battle_unit(&mut world, id, map, 0, if index % 2 == 0 { 4 } else { 0 }).unwrap();
        ids.push(id);
    }
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    (dir, config, world, map, ids.try_into().unwrap())
}

/// Assign scenario power without introducing crew actions into sensor tests.
fn power(world: &mut World, ids: &[ObjectId], value: Power) {
    for id in ids {
        world.btech.set_unit_power(*id, value).unwrap();
    }
}

/// Explicit team and perception facts for the contact domain action.
fn rules(acquire: bool) -> ContactRules {
    ContactRules {
        hostile: false,
        hidden: false,
        perception: 7,
        acquire,
    }
}

/// Vehicles acquire visible units without dice, retain them without change, and lose them
/// once no channel reaches; saved contacts survive a database round trip.
#[tokio::test]
async fn vehicle_contacts_acquire_retain_lose_and_replay_without_extra_rolls() {
    let (_dir, config, mut world, map, [_, mech, vehicle, other]) = fixture(
        ".0\n.0\n.0\n.0\n.0\n",
        include_str!("../game/mechs/Demolisher.toml"),
    )
    .await;
    power(&mut world, &[vehicle], Power::Running);
    set_battle_map_visibility(&mut world, map, Light::Day, 30).unwrap();
    for target in [mech, other] {
        let before = world.btech.clone();
        let update = update_battle_contact(&mut world, vehicle, target, rules(false)).unwrap();
        assert_eq!(update.transition, ContactTransition::Unseen);
        assert_eq!(world.btech, before);
        let update = update_battle_contact(&mut world, vehicle, target, rules(true)).unwrap();
        assert_eq!(update.transition, ContactTransition::Acquired);
        assert_eq!(
            update.detection,
            Some(Detection {
                detected: true,
                threshold: 0,
                roll: None,
            })
        );
        assert!(world.btech.vehicles()[&vehicle].contacts()[&target].identified);
        let before = world.btech.clone();
        let update = update_battle_contact(&mut world, vehicle, target, rules(false)).unwrap();
        assert_eq!(update.transition, ContactTransition::Retained);
        assert_eq!(update.detection, None);
        assert_eq!(world.btech, before);
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, world.btech);
        set_battle_map_perception(&mut restored, map, MapPerceptionFlag::Sensors, false).unwrap();
        set_battle_map_visibility(&mut restored, map, Light::Day, 2).unwrap();
        let update = update_battle_contact(&mut restored, vehicle, target, rules(true)).unwrap();
        assert_eq!(update.transition, ContactTransition::Lost);
        assert_eq!(update.detection, None);
        assert!(
            !restored.btech.vehicles()[&vehicle]
                .contacts()
                .contains_key(&target)
        );
        assert_eq!(
            roll_unit_dice(&mut restored, vehicle, 3).unwrap(),
            roll_unit_dice(&mut world.clone(), vehicle, 3).unwrap()
        );
    }
    let before = world.btech.clone();
    assert!(update_battle_contact(&mut world, vehicle, vehicle, rules(true)).is_err());
    assert_eq!(world.btech, before);
    power(&mut world, &[vehicle], Power::Off);
    let before = world.btech.clone();
    assert!(update_battle_contact(&mut world, vehicle, mech, rules(true)).is_err());
    assert_eq!(world.btech, before);
}

/// Mech and vehicle contacts are forgotten on placement, removal and database purge.
#[tokio::test]
async fn mixed_contacts_follow_placement_removal_and_database_purge() {
    let (_dir, config, mut world, _map, ids) = fixture(
        ".0\n.0\n.0\n.0\n.0\n",
        include_str!("../game/mechs/Demolisher.toml"),
    )
    .await;
    let [a, b, c, d] = ids;
    // Place everyone together for automatic detection, independent of random seed or direction.
    let map = world.btech.vehicles()[&c].position().unwrap().map;
    for id in ids {
        place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    }
    power(&mut world, &ids, Power::Running);
    for (observer, target) in [(a, c), (c, a), (c, d), (d, c), (d, b)] {
        assert_eq!(
            update_battle_contact(&mut world, observer, target, rules(true))
                .unwrap()
                .transition,
            ContactTransition::Acquired
        );
    }
    power(&mut world, &ids, Power::Off);
    place_battle_unit(&mut world, c, map, 0, 1).unwrap();
    assert!(world.btech.constructed_units()[&a].contacts().is_empty());
    assert!(world.btech.vehicles()[&c].contacts().is_empty());
    assert!(!world.btech.vehicles()[&d].contacts().contains_key(&c));
    assert!(world.btech.vehicles()[&d].contacts().contains_key(&b));
    remove_battle_unit(&mut world, b, ObjectId(config.home())).unwrap();
    assert!(world.btech.vehicles()[&d].contacts().is_empty());
    power(&mut world, &[a, d], Power::Running);
    update_battle_contact(&mut world, a, d, rules(true)).unwrap();
    update_battle_contact(&mut world, d, a, rules(true)).unwrap();
    power(&mut world, &[a, d], Power::Off);
    world.objects.get_mut(&a).unwrap().flags.insert(Flag::Going);
    persistence::save(&config.database(), &world).await.unwrap();
    persistence::repair(&config.database(), config.database.busy_timeout_ms, |raw| {
        dbck::plan(&world, raw, &config)
    })
    .await
    .unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert!(!restored.btech.constructed_units().contains_key(&a));
    assert!(restored.btech.vehicles()[&d].contacts().is_empty());
}

/// Saved vehicle contacts must reference another placed unit on the same battlefield.
#[tokio::test]
async fn vehicle_contact_snapshots_reject_invalid_references() {
    let (_dir, config, mut world, map, [a, b, c, d]) = fixture(
        ".0\n.0\n.0\n.0\n.0\n",
        include_str!("../game/mechs/Demolisher.toml"),
    )
    .await;
    let good = Contact { identified: true };
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][c.0.to_string()]["contacts"][a.0.to_string()] =
        serde_json::to_value(good).unwrap();
    let mut candidate = world.clone();
    candidate.btech = serde_json::from_value(saved).unwrap();
    candidate.validate(&config).unwrap();
    for target in [c, ObjectId(99999)] {
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        saved["vehicles"][c.0.to_string()]["contacts"][target.0.to_string()] =
            serde_json::to_value(good).unwrap();
        let mut candidate = world.clone();
        candidate.btech = serde_json::from_value(saved).unwrap();
        assert!(candidate.validate(&config).is_err());
    }
    remove_battle_unit(&mut world, b, ObjectId(config.home())).unwrap();
    let elsewhere = world.create(&config, "Another battlefield".into(), Kind::Room);
    create_battle_map(
        &mut world,
        elsewhere,
        "elsewhere",
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, d, elsewhere, 0, 0).unwrap();
    for target in [b, d] {
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        saved["vehicles"][c.0.to_string()]["contacts"][target.0.to_string()] =
            serde_json::to_value(good).unwrap();
        let mut candidate = world.clone();
        candidate.btech = serde_json::from_value(saved).unwrap();
        assert!(candidate.validate(&config).is_err());
    }
    assert_eq!(world.btech.vehicles()[&c].position().unwrap().map, map);
    world.validate(&config).unwrap();
}
