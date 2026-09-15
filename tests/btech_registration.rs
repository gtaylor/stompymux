//! DEBUG registration is usable without SQL imports and persists through creation and teardown.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
mod support;

/// Register, use, reload and unregister a carried tool without changing its game containment.
#[tokio::test]
async fn debug_registration_round_trip_and_idempotence() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let tool = world.create(&config, "Debug Console".into(), Kind::Thing);
    world.objects.get_mut(&tool).unwrap().location = Some(ObjectId(1));
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for _ in 0..2 {
        assert_eq!(
            support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                "@btech/register Debug Console=debug"
            ),
            format!("Registered #{} as BTech type DEBUG.", tool.0)
        );
    }
    assert_eq!(
        support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("@btech #{}", tool.0)
        ),
        format!("#{} BTech type: DEBUG", tool.0)
    );
    for command in [
        "@btech Debug Console",
        "@btech/i Debug Console",
        "@btech/info Debug Console",
    ] {
        assert_eq!(
            support::run_text(&scripts, &config, ObjectId(1), 1, command),
            format!("#{} BTech type: DEBUG", tool.0)
        );
    }
    assert_eq!(
        support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            "@btech/r Debug Console=DEBUG"
        ),
        format!("Registered #{} as BTech type DEBUG.", tool.0)
    );
    assert_eq!(scripts.world().objects[&tool].location, Some(ObjectId(1)));
    for (command, expected) in [
        (
            format!("@btech/register #{}", tool.0),
            "Specify MECH, DEBUG, MAP, AUTOPILOT, or TURRET.".to_owned(),
        ),
        (
            format!("@btech/register #{}=MAP", tool.0),
            "object is already registered as DEBUG; unregister it first.".to_owned(),
        ),
        (
            format!("@btech/register #{}=unknown", tool.0),
            "invalid BTech type unknown.".to_owned(),
        ),
        (
            "@btech/register me=DEBUG".into(),
            "target must be a live thing.".into(),
        ),
    ] {
        let before = scripts.world().btech.clone();
        assert_eq!(
            support::run_text(&scripts, &config, ObjectId(1), 1, &command),
            expected
        );
        assert_eq!(scripts.world().btech, before);
    }
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    *scripts.world_mut() = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        scripts
            .world()
            .btech
            .registrations()
            .get(&tool)
            .map(String::as_str),
        Some("DEBUG")
    );
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "SHUTDOWN"),
        "Invalid map number!"
    );
    for _ in 0..2 {
        assert_eq!(
            support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("@btech/unregister #{}", tool.0)
            ),
            format!("Unregistered #{} from BTech.", tool.0)
        );
    }
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert!(!restored.btech.registrations().contains_key(&tool));
    assert_eq!(restored.objects[&tool].location, Some(ObjectId(1)));
}

/// Wizard command access cannot override the separate control rule for another Wizard's object.
#[tokio::test]
async fn registration_requires_live_target_and_control() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let actor = world.create(&config, "Wizard".into(), Kind::Player);
    world
        .objects
        .get_mut(&actor)
        .unwrap()
        .flags
        .insert(Flag::Wizard);
    let tool = world.create(&config, "Protected tool".into(), Kind::Thing);
    world
        .objects
        .get_mut(&tool)
        .unwrap()
        .flags
        .insert(Flag::Wizard);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    for operation in ["register", "unregister"] {
        assert_eq!(
            support::run_text(
                &scripts,
                &config,
                actor,
                1,
                &format!("@btech/{operation} #{}=DEBUG", tool.0)
            ),
            "permission denied."
        );
    }
    assert_eq!(scripts.world().btech, before);
    scripts
        .world_mut()
        .objects
        .get_mut(&tool)
        .unwrap()
        .flags
        .insert(Flag::Going);
    assert_eq!(
        support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("@btech/register #{}=DEBUG", tool.0)
        ),
        "target must be a live thing."
    );
    assert_eq!(scripts.world().btech, before);
}

/// MAP registration supplies the reference default grid and immediately supports ordinary map tools.
#[tokio::test]
async fn map_registration_defaults_view_load_and_restart() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Training field".into(), Kind::Thing);
    world.objects.get_mut(&map).unwrap().location = Some(ObjectId(1));
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    assert_eq!(
        support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            "@btech/r Training field=map"
        ),
        format!("Registered #{} as BTech type MAP.", map.0)
    );
    {
        let world = scripts.world();
        let field = &world.btech.maps()[&map];
        assert_eq!(field.name, "Default Map");
        assert_eq!((field.width, field.height), (21, 11));
        assert_eq!((field.gravity, field.temperature, field.flags), (0, 0, 0));
        assert_eq!(
            (
                field.light,
                field.visibility,
                field.maximum_visibility,
                field.cloud_base
            ),
            (2, 30, 60, 200)
        );
        assert_eq!(field.building.regeneration, 1);
        for y in 0..11 {
            for x in 0..21 {
                assert_eq!(
                    field.hex(x, y).unwrap(),
                    BattleHex {
                        terrain: Terrain::Grassland,
                        elevation: 0
                    }
                );
            }
        }
    }
    let before = scripts.world().btech.clone();
    let viewed = support::run_text(&scripts, &config, ObjectId(1), 1, "VIEW 10 5");
    assert!(viewed.lines().count() > 10, "{viewed}");
    assert_eq!(scripts.world().btech, before);
    // Repeated registration never rebuilds terrain or resets existing simulation state.
    assert_eq!(
        support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            "@btech/register Training field=MAP"
        ),
        format!("Registered #{} as BTech type MAP.", map.0)
    );
    assert_eq!(scripts.world().btech, before);
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    *scripts.world_mut() = persistence::load(&config.database()).await.unwrap();
    assert_eq!(scripts.world().btech, before);
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "VIEW 10 5"),
        viewed
    );
    let directory = config.path(&config.database.map_database);
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("registration.map"), "3 2\n~2~2~2\n~2~2~2\n").unwrap();
    let output = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        "LOADMAP registration.map",
    );
    assert_eq!(
        (
            scripts.world().btech.maps()[&map].width,
            scripts.world().btech.maps()[&map].height
        ),
        (3, 2),
        "{output}"
    );
    assert_eq!(
        scripts.world().btech.maps()[&map]
            .hex(2, 1)
            .unwrap()
            .terrain,
        Terrain::Water
    );
    assert_eq!(scripts.world().objects[&map].location, Some(ObjectId(1)));
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, saved.btech);
}
