//! Mine coverage is derived from minefield records for every map caller.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// A five-by-three field with a workshop map available as a building interior.
async fn fixture() -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Mine field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "mines",
        BattleMapAsset::from_cells("5 3\n.0.0.0.0.0\n.0.0.0.0.0\n.0.0.0.0.0\n").unwrap(),
    )
    .unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
    (dir, config, world, map)
}

/// Common authored mine in the field's far corner.
fn mine() -> BattleMinefield {
    BattleMinefield {
        coordinate: BattleHexCoordinate { x: 4, y: 2 },
        kind: BattleMineKind::Standard,
        strength: 10,
        extra: 0,
        owner: ObjectId(1),
    }
}

/// Fire one mine-laying artillery shell at `center` and return the deposited field slots.
fn shell(world: &mut World, map: ObjectId, center: BattleHexCoordinate) -> Vec<u32> {
    let mut flight = BattleArtilleryFlight::new(
        center,
        center,
        BattleWeapon::LongTom,
        BattleArtilleryMode::Mine,
        true,
    )
    .unwrap();
    let mut result = None;
    for _ in 0..10 {
        result =
            advance_artillery_flight(world, map, &mut flight, BattleMovementRules::STANDARD.fall)
                .unwrap();
    }
    result.unwrap().mines
}

#[tokio::test]
async fn coverage_follows_records_through_edits_persistence_and_deletion() {
    let (_dir, config, mut world, map) = fixture().await;
    let field = mine();
    let elsewhere = BattleHexCoordinate { x: 0, y: 0 };
    assert!(
        !world.btech.maps()[&map]
            .mine_coverage(field.coordinate)
            .unwrap()
    );
    set_minefield(&mut world, map, 0, Some(field)).unwrap();
    assert!(
        world.btech.maps()[&map]
            .mine_coverage(field.coordinate)
            .unwrap()
    );
    assert!(!world.btech.maps()[&map].mine_coverage(elsewhere).unwrap());
    assert!(
        world.btech.maps()[&map]
            .mine_coverage(BattleHexCoordinate { x: 5, y: 0 })
            .is_err()
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert!(
        restored.btech.maps()[&map]
            .mine_coverage(field.coordinate)
            .unwrap()
    );
    assert_eq!(restored.btech, world.btech);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    // The retired lookup-object type no longer parses.
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "delobj TBITS"),
        "Invalid type!"
    );
    assert!(
        scripts.world().btech.maps()[&map]
            .mine_coverage(field.coordinate)
            .unwrap()
    );
    delete_battle_map_objects_action(
        &scripts,
        &config,
        ObjectId(1),
        map,
        Some(BattleMapObjectKind::Mine),
        None,
    )
    .unwrap();
    assert!(
        !scripts.world().btech.maps()[&map]
            .mine_coverage(field.coordinate)
            .unwrap()
    );
}

#[tokio::test]
async fn artillery_mines_are_active_at_once_and_do_not_stack() {
    let (_dir, config, mut world, map) = fixture().await;
    let center = BattleHexCoordinate { x: 2, y: 1 };
    assert_eq!(shell(&mut world, map, center), vec![0]);
    assert!(world.btech.maps()[&map].mine_coverage(center).unwrap());
    assert_eq!(world.btech.maps()[&map].minefields()[&0].owner, ObjectId(0));
    persistence::save(&config.database(), &world).await.unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    assert!(shell(&mut world, map, center).is_empty());
    assert_eq!(world.btech.maps()[&map].minefields().len(), 1);
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn resizing_a_mined_map_clears_its_mines() {
    let (_dir, config, mut world, map) = fixture().await;
    set_minefield(&mut world, map, 0, Some(mine())).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    resize_battle_map_action(&scripts, &config, ObjectId(1), map, 2, 2).unwrap();
    let world = scripts.world();
    assert!(world.btech.maps()[&map].minefields().is_empty());
    assert!(
        !world.btech.maps()[&map]
            .mine_coverage(BattleHexCoordinate { x: 1, y: 1 })
            .unwrap()
    );
}
