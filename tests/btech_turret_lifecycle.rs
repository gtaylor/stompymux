//! TURRET role lifecycle preserves world objects and independently owned station state.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
mod support;

/// Unattached defaults, edited stations, independent timers and restart share one native lifecycle.
#[tokio::test]
async fn turret_registration_and_teardown_preserve_other_stations() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let first = world.create(&config, "First station".into(), Kind::Thing);
    let second = world.create(&config, "Second station".into(), Kind::Thing);
    let occupant = world.create(&config, "Station occupant".into(), Kind::Player);
    world.objects.get_mut(&first).unwrap().location = Some(ObjectId(1));
    world.objects.get_mut(&second).unwrap().location = Some(ObjectId(1));
    world.objects.get_mut(&occupant).unwrap().location = Some(first);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for station in [first, second] {
        assert_eq!(
            support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("@btech/r #{}=turret", station.0)
            ),
            format!("Registered #{} as BTech type TURRET.", station.0)
        );
        let world = scripts.world();
        let record = &world.btech.gunner_stations()[&station];
        assert_eq!(
            (record.parent, record.gunner, record.target),
            (ObjectId(0), ObjectId(0), ObjectId(-1))
        );
        assert_eq!(record.target_coordinates, [-1, -1, 0]);
        assert_eq!(record.tics, [0; 4]);
        assert_eq!(
            (
                record.arcs,
                record.lock_modes,
                record.lock_remaining,
                record.artillery_adjustment
            ),
            (0, 0, 0, 0)
        );
    }
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    *scripts.world_mut() = persistence::load(&config.database()).await.unwrap();
    assert_eq!(scripts.world().btech, saved.btech);
    // Raw station fields permit deferred targets; retiring one role must not erase another's events.
    let mut encoded = serde_json::to_value(&scripts.world().btech).unwrap();
    for (station, target) in [(first, second), (second, first)] {
        let record = &mut encoded["gunner_stations"][station.0.to_string()];
        record["tics"] = serde_json::json!([1, 2, 4, 8]);
        record["target"] = target.0.into();
        record["lock_remaining"] = 3.into();
        record["artillery_adjustment"] = 4.into();
    }
    scripts.world_mut().btech = serde_json::from_value(encoded).unwrap();
    let before = scripts.world().clone();
    persistence::save(&config.database(), &before)
        .await
        .unwrap();
    assert_eq!(
        support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("@btech/register #{}=TURRET", first.0)
        ),
        format!("Registered #{} as BTech type TURRET.", first.0)
    );
    assert_eq!(scripts.world().btech, before.btech);
    scripts
        .world_mut()
        .objects
        .get_mut(&first)
        .unwrap()
        .flags
        .insert(Flag::Going);
    assert_eq!(
        support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("@btech/u #{}", first.0)
        ),
        format!("Unregistered #{} from BTech.", first.0)
    );
    let after = scripts.world().clone();
    assert!(!after.btech.gunner_stations().contains_key(&first));
    assert!(!after.btech.registrations().contains_key(&first));
    assert_eq!(
        after.btech.gunner_stations()[&second],
        before.btech.gunner_stations()[&second]
    );
    assert_eq!(after.objects[&occupant].location, Some(first));
    persistence::save(&config.database(), &after).await.unwrap();
    *scripts.world_mut() = persistence::load(&config.database()).await.unwrap();
    assert_eq!(scripts.world().btech, after.btech);
    assert_eq!(scripts.world().objects[&occupant].location, Some(first));
    scripts
        .world_mut()
        .objects
        .get_mut(&first)
        .unwrap()
        .flags
        .remove(Flag::Going);
    assert_eq!(
        support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("@btech/r #{}=TURRET", first.0)
        ),
        format!("Registered #{} as BTech type TURRET.", first.0)
    );
    assert_eq!(
        scripts.world().btech.gunner_stations()[&first],
        BattleGunnerStation::default()
    );
}

/// Durable station deletion rolls back its timers and registration if the database refuses the write.
#[tokio::test]
async fn failed_turret_save_restores_previous_records() {
    use sqlx::{Connection, SqliteConnection};
    let (_dir, config, mut world) = support::isolated_world().await;
    let station = world.create(&config, "Station".into(), Kind::Thing);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        &format!("@btech/r #{}=TURRET", station.0),
    );
    let mut encoded = serde_json::to_value(&scripts.world().btech).unwrap();
    let record = &mut encoded["gunner_stations"][station.0.to_string()];
    record["target"] = station.0.into();
    record["lock_remaining"] = 3.into();
    record["artillery_adjustment"] = 4.into();
    scripts.world_mut().btech = serde_json::from_value(encoded).unwrap();
    let before = scripts.world().clone();
    persistence::save(&config.database(), &before)
        .await
        .unwrap();
    let mut sql = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TRIGGER refuse_turret_delete BEFORE DELETE ON btech_turrets BEGIN SELECT RAISE(ABORT, 'test failure'); END").execute(&mut sql).await.unwrap();
    sql.close().await.unwrap();
    support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        &format!("@btech/u #{}", station.0),
    );
    let after = scripts.world().clone();
    assert!(persistence::save(&config.database(), &after).await.is_err());
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        before.btech
    );
}

/// Explicit unregister/register transitions do not require an intermediate database save.
#[tokio::test]
async fn supported_role_changes_commit_in_one_save() {
    for previous in ["DEBUG", "MAP", "TURRET"] {
        for current in ["DEBUG", "MAP", "TURRET"] {
            let (_dir, config, mut world) = support::isolated_world().await;
            let object = world.create(&config, "Changeable role".into(), Kind::Thing);
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let output = support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("@btech/r #{}={previous}", object.0),
            );
            assert!(output.contains("Registered"), "{output}");
            if previous == "TURRET" {
                let mut encoded = serde_json::to_value(&scripts.world().btech).unwrap();
                encoded["gunner_stations"][object.0.to_string()]["tics"] =
                    serde_json::json!([7, 8, 9, 10]);
                scripts.world_mut().btech = serde_json::from_value(encoded).unwrap();
            }
            let before = scripts.world().clone();
            persistence::save(&config.database(), &before)
                .await
                .unwrap();
            support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("@btech/u #{}", object.0),
            );
            let output = support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("@btech/r #{}={current}", object.0),
            );
            assert!(output.contains("Registered"), "{output}");
            let after = scripts.world().clone();
            persistence::save(&config.database(), &after)
                .await
                .unwrap_or_else(|error| panic!("{previous} -> {current}: {error:#}"));
            let loaded = persistence::load(&config.database()).await.unwrap();
            assert_eq!(loaded.btech, after.btech, "{previous} -> {current}");
        }
    }
}
