//! Map command routing retains the selected registration without relocating the invoking actor.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
mod support;

/// Distinct map names and terrain reveal accidental fallback to the actor's location.
async fn fixture() -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let room = world.create(&config, "Location map".into(), Kind::Room);
    let carried = world.create(&config, "Carried map".into(), Kind::Thing);
    for (id, name, tile) in [(room, "location", ".0"), (carried, "selected", "~1")] {
        create_battle_map(
            &mut world,
            id,
            name,
            BattleMapAsset::parse(&format!(
                "5 5\n{}",
                format!("{}\n", tile.repeat(5)).repeat(5)
            ))
            .unwrap(),
        )
        .unwrap();
    }
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(room);
    world.objects.get_mut(&carried).unwrap().location = Some(ObjectId(1));
    world
        .objects
        .get_mut(&room)
        .unwrap()
        .flags
        .insert(Flag::Zombie);
    let exit = world.create(&config, "FIXMAP".into(), Kind::Exit);
    world.objects.get_mut(&exit).unwrap().location = Some(room);
    world.objects.get_mut(&exit).unwrap().destination = Some(ObjectId(0));
    let root = config.path(&config.database.map_database);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("route.map"),
        format!("5 5\n{}", ".2.2.2.2.2\n".repeat(5)),
    )
    .unwrap();
    (dir, config, world, room, carried)
}

/// Every terrain/map adapter agrees with invoking that same handler from inside the selected map.
#[tokio::test]
async fn inventory_map_commands_share_handlers_and_preserve_location() {
    let (_dir, config, base, room, selected) = fixture().await;
    for command in [
        "@VIEWMAP",
        "@SETMAP gravity 150",
        "ADDICE 100",
        "DELICE 100",
        "SETCOND 50 -40",
        "VIEW 1 1",
        "ADDBLOCK 1 1 2",
        "ADDMINE 1 1 1 5",
        "ADDHEX 1 1 . 2",
        "SETLINKED",
        "@MAPEMIT Routed message",
        "FIXMAP",
        "LOADMAP route.map",
        "SAVEMAP saved.map",
        "SETMAPSIZE 3 3",
        "LIST OBJS",
        "CLEARMECHS",
        "ADDFIRE 1 1 30",
        "ADDSMOKE 1 1 30",
        "DELOBJ",
        "UPDATELINKS",
    ] {
        let actual = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
        let mut inside = base.clone();
        // The comparison fixture puts the map in a room before moving its observer inside.
        inside.objects.get_mut(&selected).unwrap().location = Some(ObjectId(0));
        inside.objects.get_mut(&ObjectId(1)).unwrap().location = Some(selected);
        let expected = Scripts::new(&config, Rc::new(RefCell::new(inside))).unwrap();
        let output = support::run_text(&actual, &config, ObjectId(1), 1, command);
        assert_eq!(
            output,
            support::run_text(&expected, &config, ObjectId(1), 1, command),
            "{command}"
        );
        assert_eq!(actual.world().btech, expected.world().btech, "{command}");
        assert_eq!(actual.world().objects[&ObjectId(1)].location, Some(room));
        assert_eq!(
            actual.world().btech.maps()[&room],
            base.btech.maps()[&room],
            "{command}"
        );
        if command == "SETCOND 50 -40" {
            assert_eq!(
                actual.world().btech.maps()[&selected].environment().gravity,
                50
            );
        }
        actual.world().validate(&config).unwrap();
    }
    let actual = Scripts::new(&config, Rc::new(RefCell::new(base))).unwrap();
    assert!(
        support::run_text(&actual, &config, ObjectId(1), 1, "SETCOND 50 -40")
            .contains("Conditions set!")
    );
    let saved = actual.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, saved.btech);
    let actual = Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
    assert!(
        support::run_text(&actual, &config, ObjectId(1), 1, "SETCOND 150 60")
            .contains("Conditions set!")
    );
    assert_eq!(
        actual.world().btech.maps()[&selected].environment().gravity,
        150
    );
    assert_eq!(actual.world().objects[&ObjectId(1)].location, Some(room));
}

/// The reference's map STORES and stock corrections operate on the actor's location.
#[tokio::test]
async fn map_stores_uses_manifest_and_location_inventory() {
    let (_dir, config, mut world, room, selected) = fixture().await;
    set_battle_inventory_quantity(&mut world, ObjectId(1), room, 422, 0, 7).unwrap();
    set_battle_inventory_quantity(&mut world, ObjectId(1), selected, 422, 0, 3).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    let stores = support::run_text(&scripts, &config, ObjectId(1), 1, "STORES");
    assert_eq!(
        stores,
        support::run_text(&scripts, &config, ObjectId(1), 1, "MANIFEST")
    );
    assert!(stores.starts_with("7 "), "{stores}");
    assert_eq!(scripts.world().btech, before);
    let selected_stock = battle_inventory(&scripts.world(), selected)
        .unwrap()
        .to_vec();
    support::run_text(&scripts, &config, ObjectId(1), 1, "CLEARSTUFF");
    assert!(battle_inventory(&scripts.world(), room).unwrap().is_empty());
    assert_eq!(
        battle_inventory(&scripts.world(), selected).unwrap(),
        selected_stock
    );
}

/// A queued map actor operates on itself, retaining its cause and physical containment.
#[tokio::test]
async fn map_actor_is_selected_before_its_location() {
    let (_dir, config, mut world, room, selected) = fixture().await;
    world
        .objects
        .get_mut(&room)
        .unwrap()
        .flags
        .remove(Flag::Zombie);
    world.objects.get_mut(&selected).unwrap().location = Some(room);
    world
        .objects
        .get_mut(&selected)
        .unwrap()
        .flags
        .insert(Flag::Wizard);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let execution = commands::ExecutionContext {
        executor: selected,
        cause: ObjectId(1),
        session: None,
        origin: commands::InputOrigin::Queued,
    };
    commands::execute(&scripts, &config, execution, "SETCOND 150 60").unwrap();
    assert_eq!(
        scripts.world().btech.maps()[&selected]
            .environment()
            .gravity,
        150
    );
    assert_eq!(
        scripts.world().btech.maps()[&room].environment().gravity,
        100
    );
    assert_eq!(scripts.world().objects[&selected].location, Some(room));
}
