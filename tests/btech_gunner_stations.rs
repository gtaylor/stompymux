//! Independent station ownership, supported-parent admission and selective persistence.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod firing;
use crate::support;

#[tokio::test]
async fn station_lifecycle_is_shared_across_all_supported_parents() {
    for template in firing::templates() {
        let (_dir, config, mut world, parent, _, _) =
            firing::fixture_with_target(&template, None, &template).await;
        let gunner = world.create(&config, "Gunner".into(), Kind::Player);
        let replacement = world.create(&config, "Replacement".into(), Kind::Player);
        let gunner_id = gunner.0;
        let replacement_id = replacement.0;
        let station = world.create(&config, "Gunner station".into(), Kind::Thing);
        register_gunner_station(&mut world, ObjectId(1), station, parent, 5).unwrap();
        world.objects.get_mut(&gunner).unwrap().location = Some(station);
        world.objects.get_mut(&replacement).unwrap().location = Some(station);
        world
            .objects
            .get_mut(&gunner)
            .unwrap()
            .flags
            .insert(Flag::Connected);
        let parent_before = world.btech.clone();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        assert!(gunner_context(&scripts.world(), station, gunner).is_err());
        support::run_text(&scripts, &config, gunner, 4, "initialize ignored");
        let context = gunner_context(&scripts.world(), station, gunner).unwrap();
        assert_eq!(
            (context.parent, context.station, context.arcs),
            (parent, station, 5)
        );
        assert_eq!(
            scripts.world().btech.constructed_units(),
            parent_before.constructed_units()
        );
        assert_eq!(scripts.world().btech.vehicles(), parent_before.vehicles());
        assert!(gunner_station_action(&scripts, station, replacement, true).is_err());
        assert!(gunner_station_action(&scripts, station, replacement, false).is_err());
        assert!(gunner_context(&scripts.world(), station, replacement).is_err());
        let grip = support::run_text(&scripts, &config, gunner, 4, "initialize");
        assert!(grip.contains("joystick"));
        scripts
            .world_mut()
            .objects
            .get_mut(&gunner)
            .unwrap()
            .flags
            .remove(Flag::Connected);
        scripts
            .eval_callback::<()>(&format!(
                "btech.gunner.initialize({replacement_id},{})",
                station.0
            ))
            .unwrap();
        assert_eq!(
            scripts.world().btech.gunner_stations()[&station].gunner,
            replacement
        );
        let before = scripts.world().btech.clone();
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.gunner.deinitialize({replacement_id},{});error('abort')",
                    station.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        support::run_text(&scripts, &config, replacement, 2, "deinitialize ignored");
        assert_eq!(
            scripts.world().btech.gunner_stations()[&station].gunner,
            ObjectId(-1)
        );
        scripts
            .eval_callback::<()>(&format!(
                "btech.gunner.initialize({gunner_id},{})",
                station.0
            ))
            .unwrap();
        scripts
            .world_mut()
            .objects
            .get_mut(&gunner)
            .unwrap()
            .flags
            .insert(Flag::Connected);
        scripts
            .world_mut()
            .objects
            .get_mut(&gunner)
            .unwrap()
            .location = Some(ObjectId(0));
        gunner_station_action(&scripts, station, replacement, true).unwrap();
        assert!(gunner_context(&scripts.world(), station, gunner).is_err());
        let before = scripts.world().btech.clone();
        scripts
            .eval_callback::<()>(&format!(
                "local s=btech.gunner.state({});s.gunner=999",
                station.0
            ))
            .unwrap();
        assert_eq!(scripts.world().btech, before);
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            loaded.btech.gunner_stations(),
            saved.btech.gunner_stations()
        );
        assert_eq!(
            gunner_context(&loaded, station, replacement)
                .unwrap()
                .parent,
            parent
        );
    }
}

#[tokio::test]
async fn registration_authority_and_saved_aim_fields_are_preserved() {
    use sqlx::Connection;
    let template = &firing::templates()[0];
    let (_dir, config, mut world, parent, target, _) =
        firing::fixture_with_target(template, None, template).await;
    let gunner = world.create(&config, "Gunner".into(), Kind::Player);
    let station = world.create(&config, "Station".into(), Kind::Thing);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for (actor, object, host) in [
        (gunner.0, station, parent),
        (1, parent, parent),
        (1, station, ObjectId(0)),
    ] {
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.gunner.register({actor},{},{},5)",
                    object.0, host.0
                ))
                .is_err()
        );
    }
    scripts
        .eval_callback::<()>(&format!(
            "btech.gunner.register(1,{},{},5)",
            station.0, parent.0
        ))
        .unwrap();
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let mut db = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE btech_turrets SET target=?,target_x=-32768,target_y=32767,target_z=123,lock_mode=-2147483648,arcs=-1 WHERE dbref=?").bind(target.0).bind(station.0).execute(&mut db).await.unwrap();
    sqlx::query("UPDATE btech_turret_tics SET value=987 WHERE turret_dbref=? AND tic_index=3")
        .bind(station.0)
        .execute(&mut db)
        .await
        .unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    let aim = loaded.btech.gunner_stations()[&station].clone();
    loaded.objects.get_mut(&gunner).unwrap().location = Some(station);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
    gunner_station_action(&scripts, station, gunner, true).unwrap();
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let reloaded = persistence::load(&config.database()).await.unwrap();
    let mut expected = aim;
    expected.gunner = gunner;
    assert_eq!(reloaded.btech.gunner_stations()[&station], expected);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT value FROM btech_turret_tics WHERE turret_dbref=? AND tic_index=3"
        )
        .bind(station.0)
        .fetch_one(&mut db)
        .await
        .unwrap(),
        987
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn unavailable_parents_publication_failure_and_purge_are_safe() {
    let template = &firing::templates()[0];
    let (dir, config, mut world, parent, _, _) =
        firing::fixture_with_target(template, None, template).await;
    let gunner = world.create(&config, "Gunner".into(), Kind::Player);
    let replacement = world.create(&config, "Replacement".into(), Kind::Player);
    let station = world.create(&config, "Station".into(), Kind::Thing);
    register_gunner_station(&mut world, ObjectId(1), station, parent, 1).unwrap();
    for actor in [replacement, gunner] {
        world.objects.get_mut(&actor).unwrap().location = Some(station);
    }
    let path = dir.path().join("stompymux.toml");
    let mut table: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
    table
        .entry("lua")
        .or_insert(toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .unwrap()
        .insert("output_entry_limit".into(), 1.into());
    std::fs::write(&path, toml::to_string(&table).unwrap()).unwrap();
    let limited = Config::load(dir.path()).unwrap();
    let scripts = Scripts::new(&limited, Rc::new(RefCell::new(world.clone()))).unwrap();
    assert!(gunner_station_action(&scripts, station, gunner, true).is_err());
    assert_eq!(scripts.world().btech, world.btech);
    assert!(scripts.drain_outbox().is_empty());
    world
        .objects
        .get_mut(&parent)
        .unwrap()
        .flags
        .insert(Flag::Going);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    assert!(gunner_station_action(&scripts, station, gunner, true).is_err());
    assert_eq!(scripts.world().btech, world.btech);
    persistence::save(&config.database(), &world).await.unwrap();
    persistence::repair(&config.database(), config.database.busy_timeout_ms, |raw| {
        dbck::plan(&world, raw, &config)
    })
    .await
    .unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        loaded.btech.gunner_stations()[&station].parent,
        ObjectId(-1)
    );
    assert!(gunner_context(&loaded, station, gunner).is_err());
    loaded
        .objects
        .get_mut(&station)
        .unwrap()
        .flags
        .insert(Flag::Going);
    persistence::save(&config.database(), &loaded)
        .await
        .unwrap();
    persistence::repair(&config.database(), config.database.busy_timeout_ms, |raw| {
        dbck::plan(&loaded, raw, &config)
    })
    .await
    .unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert!(!loaded.btech.gunner_stations().contains_key(&station));
}
