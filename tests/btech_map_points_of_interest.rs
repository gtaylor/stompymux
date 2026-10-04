//! Map-file points of interest persist with their map and are queryable from Lua.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// A 3x2 map with objectives whose types differ only in case.
const SOURCE: &str = r#"
terrain = '''
...
...
'''
level = '''
012
000
'''

[[points_of_interest]]
type = "Objective"
name = "Comms Tower"
x = 2
y = 0
elevation = 3

[[points_of_interest]]
type = "objective"
name = "Ford"
x = 0
y = 1

[[points_of_interest]]
type = "Objective"
name = "Depot"
x = 1
y = 1
elevation = -1
"#;

/// The same map with a single point of interest.
const RELOADED: &str = r#"
terrain = '''
...
...
'''
level = '''
000
000
'''

[[points_of_interest]]
type = "landing zone"
name = "LZ Alpha"
x = 1
y = 0
"#;

/// A wizard standing on a map built from [`SOURCE`], saved to the database.
async fn fixture() -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "POI field".into(), Kind::Room);
    create_battle_map(&mut world, map, "poi", MapAsset::parse(SOURCE).unwrap()).unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let actor = world.create(&config, "POI operator".into(), Kind::Player);
    let operator = world.objects.get_mut(&actor).unwrap();
    operator.flags.insert(Flag::Wizard);
    operator.location = Some(map);
    let root = config.path(&config.database.map_database);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("poi.map.toml"), SOURCE).unwrap();
    std::fs::write(root.join("reloaded.map.toml"), RELOADED).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    (dir, config, world, map, actor)
}

/// Points of interest survive a restart, and scripts list them in file order with an exact,
/// case-sensitive type filter, both from a live map and straight from a map file.
#[tokio::test]
async fn points_of_interest_persist_and_are_queryable_from_lua() {
    let (_dir, config, world, map, _) = fixture().await;
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, world.btech);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(restored))).unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("map_id", map.0)
        .unwrap();
    scripts
        .eval_callback::<()>(
            r#"
            local all=btech.map.points_of_interest(map_id)
            assert(#all==3)
            assert(all[1].type=='Objective' and all[1].name=='Comms Tower' and all[1].x==2 and all[1].y==0 and all[1].elevation==3)
            assert(all[2].type=='objective' and all[2].name=='Ford' and all[2].x==0 and all[2].y==1 and all[2].elevation==nil)
            assert(all[3].name=='Depot' and all[3].elevation==-1)
            local upper=btech.map.points_of_interest(map_id,'Objective')
            assert(#upper==2 and upper[1].name=='Comms Tower' and upper[2].name=='Depot')
            local lower=btech.map.points_of_interest(map_id,'objective')
            assert(#lower==1 and lower[1].name=='Ford')
            assert(#btech.map.points_of_interest(map_id,'OBJECTIVE')==0)
            local ok,err=mux.error.pcall(btech.map.points_of_interest,map_id,7)
            assert(not ok and err.code=='mux.arg.invalid' and err.detail.argument==2)
            local summary=btech.map.inspect_file('reloaded.map')
            assert(#summary.points_of_interest==1)
            assert(summary.points_of_interest[1].type=='landing zone' and summary.points_of_interest[1].name=='LZ Alpha')
        "#,
        )
        .unwrap();
}

/// Reloading a map replaces its points of interest durably, and saving the map writes them
/// back into the map file.
#[tokio::test]
async fn reload_replaces_and_save_writes_points_of_interest() {
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
    let reloaded = MapAsset::parse(RELOADED).unwrap().points_of_interest;
    let saved = scripts.world().clone();
    assert_eq!(
        saved.btech.maps()[&map].export_asset().unwrap(),
        MapAsset::parse(RELOADED).unwrap().to_file().unwrap()
    );
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, saved.btech);
    let exported = MapAsset::parse(&restored.btech.maps()[&map].export_asset().unwrap())
        .unwrap()
        .points_of_interest;
    assert_eq!(exported, reloaded);
}

/// Cropping a map drops the points of interest that fall off it and keeps the rest.
#[tokio::test]
async fn resizing_crops_points_of_interest() {
    let (_dir, config, world, map, actor) = fixture().await;
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    resize_battle_map_action(&scripts, &config, actor, map, 2, 2).unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("map_id", map.0)
        .unwrap();
    scripts
        .eval_callback::<()>(
            r#"
            local kept=btech.map.points_of_interest(map_id)
            assert(#kept==2 and kept[1].name=='Ford' and kept[2].name=='Depot')
        "#,
        )
        .unwrap();
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, saved.btech);
}
