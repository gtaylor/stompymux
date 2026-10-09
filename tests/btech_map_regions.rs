//! Map-file regions persist with their map, are replaced by a reload, are written back by a
//! save, and lose the corners a resize crops off.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// A 6x4 map with a filled deployment zone and a one-hex region.
const SOURCE: &str = r#"
terrain = '''
......
......
......
......
'''
level = '''
000000
000000
000000
000000
'''

[[regions]]
type = "deployment"
name = "North LZ"
corners = [[0, 0], [5, 0], [5, 3], [0, 3]]

[[regions]]
type = "Objective"
name = "Bunker"
corners = [[4, 1]]
"#;

/// The same map with a different region.
const RELOADED: &str = r#"
terrain = '''
......
......
......
......
'''
level = '''
000000
000000
000000
000000
'''

[[regions]]
type = "deployment"
name = "South LZ"
corners = [[1, 2], [4, 3]]
"#;

/// A wizard standing on a map built from [`SOURCE`], saved to the database.
async fn fixture() -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Region field".into(), Kind::Room);
    create_battle_map(&mut world, map, "regions", MapAsset::parse(SOURCE).unwrap()).unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let actor = world.create(&config, "Region operator".into(), Kind::Player);
    let operator = world.objects.get_mut(&actor).unwrap();
    operator.flags.insert(Flag::Wizard);
    operator.location = Some(map);
    let root = config.path(&config.database.map_database);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("regions.map.toml"), SOURCE).unwrap();
    std::fs::write(root.join("reloaded.map.toml"), RELOADED).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    (dir, config, world, map, actor)
}

/// The regions a live map exports, read back from the exported map file.
fn exported_regions(world: &World, map: ObjectId) -> Vec<MapRegion> {
    MapAsset::parse(&world.btech.maps()[&map].export_asset().unwrap())
        .unwrap()
        .regions
}

/// Regions survive a restart in file order with their corners in order, and a save writes
/// them back into the map file.
#[tokio::test]
async fn regions_persist_and_export() {
    let (_dir, config, world, map, _) = fixture().await;
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, world.btech);
    let regions = exported_regions(&restored, map);
    assert_eq!(regions, MapAsset::parse(SOURCE).unwrap().regions);
    assert_eq!(regions[0].corners, [[0, 0], [5, 0], [5, 3], [0, 3]]);
    assert!(regions[0].contains(HexCoordinate { x: 2, y: 2 }).unwrap());
    assert_eq!(regions[1].hexes().unwrap(), [HexCoordinate { x: 4, y: 1 }]);
}

/// Reloading a map replaces its regions durably.
#[tokio::test]
async fn reload_replaces_regions() {
    let (_dir, config, world, map, actor) = fixture().await;
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    assert!(
        scripts
            .eval_callback::<bool>(&format!(
                "return btech.map.load_as({}, {}, 'reloaded.map')",
                actor.0, map.0
            ))
            .unwrap()
    );
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, saved.btech);
    assert_eq!(
        exported_regions(&restored, map),
        MapAsset::parse(RELOADED).unwrap().regions
    );
}

/// Cropping a map drops the corners that fall off it, and the regions left without corners.
#[tokio::test]
async fn resizing_crops_region_corners() {
    let (_dir, config, world, map, actor) = fixture().await;
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    resize_battle_map_action(&scripts, &config, actor, map, 4, 3).unwrap();
    let saved = scripts.world().clone();
    let regions = exported_regions(&saved, map);
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].name, "North LZ");
    assert_eq!(regions[0].corners, [[0, 0]]);
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, saved.btech);
}
