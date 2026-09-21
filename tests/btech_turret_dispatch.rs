//! Selected turret fields and reserved commands precede exit and global command lookup.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod firing;
use crate::support;

/// Carried stations receive field edits without relocating actors or modifying their location's station.
#[tokio::test]
async fn carried_station_fields_and_reserved_commands_select_one_object() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let station = world.create(&config, "Carried station".into(), Kind::Thing);
    let location = world.create(&config, "Location station".into(), Kind::Thing);
    world.objects.get_mut(&station).unwrap().location = Some(ObjectId(1));
    world.objects.get_mut(&location).unwrap().location = Some(ObjectId(0));
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for id in [station, location] {
        let reply = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("@btech/r #{}=TURRET", id.0),
        );
        assert!(reply.contains("Registered"), "{reply}");
    }
    scripts
        .world_mut()
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .location = Some(location);
    scripts
        .world_mut()
        .objects
        .get_mut(&location)
        .unwrap()
        .flags
        .insert(Flag::Zombie);
    let location_before = scripts.world().btech.gunner_stations()[&location].clone();
    for (field, value) in [("parent", 123), ("arcs", 5), ("gunner", -1)] {
        assert_eq!(
            support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("@SETTURRET {field} {value}")
            ),
            ""
        );
    }
    assert_eq!(
        scripts.world().btech.gunner_stations()[&station].parent,
        ObjectId(123)
    );
    assert_eq!(scripts.world().btech.gunner_stations()[&station].arcs, 5);
    assert_eq!(
        scripts.world().btech.gunner_stations()[&location],
        location_before
    );
    let before = scripts.world().btech.clone();
    let expected = view_gunner_fields(&scripts, ObjectId(1), station, "").unwrap();
    scripts.drain_outbox();
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "@VIEWTURRET"),
        expected
    );
    assert_eq!(scripts.world().btech, before);
    for name in ["ADDTIC", "DELTIC", "CLEARTIC", "LISTTIC", "FIRETIC"] {
        let exit = scripts.world_mut().create(&config, name.into(), Kind::Exit);
        scripts.world_mut().objects.get_mut(&exit).unwrap().location = Some(location);
        scripts
            .world_mut()
            .objects
            .get_mut(&exit)
            .unwrap()
            .destination = Some(ObjectId(0));
        for arguments in ["", "nonsense", "0 1-100"] {
            assert_eq!(
                support::run_text(
                    &scripts,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("{name} {arguments}")
                ),
                ""
            );
            assert_eq!(
                scripts.world().objects[&ObjectId(1)].location,
                Some(location)
            );
            assert_eq!(scripts.world().btech, before);
        }
    }
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    *scripts.world_mut() = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "@VIEWTURRET"),
        expected
    );
    let reader = scripts
        .world_mut()
        .create(&config, "Station reader".into(), Kind::Player);
    scripts
        .world_mut()
        .objects
        .get_mut(&reader)
        .unwrap()
        .location = Some(location);
    scripts
        .world_mut()
        .objects
        .get_mut(&station)
        .unwrap()
        .location = Some(reader);
    let before = scripts.world().btech.clone();
    for command in ["@SETTURRET parent 0", "@VIEWTURRET"] {
        assert_eq!(
            support::run_text(&scripts, &config, reader, 1, command),
            "Sorry, that command is restricted!"
        );
    }
    assert_eq!(
        support::run_text(&scripts, &config, reader, 1, "FIRETIC invalid"),
        ""
    );
    assert_eq!(scripts.world().objects[&reader].location, Some(location));
    assert_eq!(scripts.world().btech, before);
}

/// Carried station lifecycle uses candidate selection and the actor's location for takeover.
#[tokio::test]
async fn carried_station_lifecycle_precedes_exits_and_preserves_parent() {
    for template in firing::templates() {
        let (_dir, config, mut world, parent, _, _) =
            firing::fixture_with_target(&template, None, &template).await;
        let actor = world.create(&config, "Gunner".into(), Kind::Player);
        let previous = world.create(&config, "Previous gunner".into(), Kind::Player);
        let station = world.create(&config, "Carried station".into(), Kind::Thing);
        register_gunner_station(&mut world, ObjectId(1), station, parent, 5).unwrap();
        for id in [actor, previous] {
            world.objects.get_mut(&id).unwrap().location = Some(ObjectId(0));
        }
        world
            .objects
            .get_mut(&previous)
            .unwrap()
            .flags
            .insert(Flag::Connected);
        world.objects.get_mut(&station).unwrap().location = Some(previous);
        for name in ["initialize", "deinitialize"] {
            let exit = world.create(&config, name.into(), Kind::Exit);
            world.objects.get_mut(&exit).unwrap().location = Some(ObjectId(0));
            world.objects.get_mut(&exit).unwrap().destination = Some(parent);
        }
        let before = world.btech.clone();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        assert_eq!(
            support::run_text(&scripts, &config, previous, 1, "initialize ignored"),
            ""
        );
        assert_eq!(
            scripts.world().btech.gunner_stations()[&station].gunner,
            previous
        );
        assert_eq!(
            support::run_text(&scripts, &config, previous, 1, "initialize"),
            "You grap firmer hold on the joystick.."
        );
        scripts
            .world_mut()
            .objects
            .get_mut(&station)
            .unwrap()
            .location = Some(actor);
        let claimed = scripts.world().btech.clone();
        assert_eq!(
            support::run_text(&scripts, &config, actor, 1, "initialize"),
            "You need Previous gunner to leave or disconnect first."
        );
        assert_eq!(
            support::run_text(&scripts, &config, actor, 1, "deinitialize"),
            "You aren't gunner!"
        );
        assert_eq!(scripts.world().btech, claimed);
        // A connected previous gunner elsewhere does not prevent takeover.
        scripts
            .world_mut()
            .objects
            .get_mut(&previous)
            .unwrap()
            .location = Some(parent);
        assert_eq!(
            support::run_text(&scripts, &config, actor, 1, "initialize"),
            ""
        );
        assert_eq!(
            gunner_context(&scripts.world(), station, actor)
                .unwrap()
                .parent,
            parent
        );
        assert_eq!(scripts.world().objects[&actor].location, Some(ObjectId(0)));
        assert_eq!(scripts.world().objects[&station].location, Some(actor));
        assert_eq!(
            scripts.world().btech.constructed_units(),
            before.constructed_units()
        );
        assert_eq!(scripts.world().btech.vehicles(), before.vehicles());
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        *scripts.world_mut() = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            scripts.world().btech.gunner_stations()[&station].gunner,
            actor
        );
        assert_eq!(
            support::run_text(&scripts, &config, actor, 1, "deinitialize ignored"),
            ""
        );
        assert_eq!(
            scripts.world().btech.gunner_stations()[&station].gunner,
            ObjectId(-1)
        );
        assert_eq!(scripts.world().objects[&actor].location, Some(ObjectId(0)));
    }
}
