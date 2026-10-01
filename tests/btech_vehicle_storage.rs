//! Ground-vehicle ownership, transactional storage, replay, and owning-object cleanup.
use crate::support;
use sqlx::{Connection, SqliteConnection};
use stompymux_rs::*;

#[tokio::test]
async fn vehicles_save_damage_replay_and_purge_with_their_objects() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Demolisher".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().location = Some(ObjectId(config.start()));
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    persistence::save(&config.database(), &world).await.unwrap();
    let original = world.clone();
    create_battle_vehicle(
        &mut world,
        id,
        BattleVehicleTemplate::parse("Demolisher",include_str!("../game/mechs/Demolisher.toml")).unwrap(),
    )
    .unwrap();
    assert_eq!(world.btech.units()[&id].class_code, 1);
    assert_eq!(world.btech.units()[&id].movement_code, 1);
    assert!(!world.btech.constructed_units().contains_key(&id));
    assert!(!original.btech.vehicles().contains_key(&id));
    let before = world.btech.clone();
    assert!(
        create_battle_vehicle(
            &mut world,
            id,
            BattleVehicleTemplate::parse("Demolisher",include_str!("../game/mechs/Demolisher.toml")).unwrap()
        )
        .is_err()
    );
    assert_eq!(world.btech, before);
    let mut sql = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(config.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TRIGGER deny_vehicle_registration BEFORE INSERT ON btech_special_registrations BEGIN SELECT RAISE(ABORT,'registration failure'); END").execute(&mut sql).await.unwrap();
    assert!(persistence::save(&config.database(), &world).await.is_err());
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        original.btech
    );
    sqlx::query("DROP TRIGGER deny_vehicle_registration")
        .execute(&mut sql)
        .await
        .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    let undamaged = loaded.clone();
    damage_battle_vehicle_phase(
        &mut loaded,
        id,
        BattleVehicleSection::Turret,
        8,
        BattleDamagePhase::Internal,
    )
    .unwrap();
    assert_eq!(undamaged.btech.vehicles()[&id].ammunition(), &[5, 5, 5, 5]);
    persistence::save(&config.database(), &loaded)
        .await
        .unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, loaded.btech);
    assert_eq!(restored.btech.vehicles()[&id].ammunition(), &[0, 0, 0, 0]);
    assert!(!restored.btech.vehicles()[&id].is_destroyed());
    restored
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::Going);
    assert!(
        damage_battle_vehicle_phase(
            &mut restored,
            id,
            BattleVehicleSection::Front,
            8,
            BattleDamagePhase::Internal
        )
        .is_err()
    );
    persistence::save(&config.database(), &restored)
        .await
        .unwrap();
    persistence::repair(&config.database(), config.database.busy_timeout_ms, |raw| {
        dbck::plan(&restored, raw, &config)
    })
    .await
    .unwrap();
    let purged = persistence::load(&config.database()).await.unwrap();
    assert!(!purged.btech.vehicles().contains_key(&id));
    assert!(!purged.btech.units().contains_key(&id));
    assert!(!purged.btech.registrations().contains_key(&id));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM btech_vehicles")
            .fetch_one(&mut sql)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn vehicle_storage_rejects_corrupt_and_oversized_records() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Truck".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().location = Some(ObjectId(config.start()));
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    let definition =
        BattleVehicleTemplate::parse("Flatbed_Truck",include_str!("../game/mechs/Flatbed_Truck.toml")).unwrap();
    assert!(create_battle_vehicle(&mut world, ObjectId(1), definition.clone()).is_err());
    create_battle_vehicle(&mut world, id, definition).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut sql = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(config.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    for encoded in ["{}".to_owned(), " ".repeat(1_048_577)] {
        sqlx::query("UPDATE btech_vehicles SET unit=?")
            .bind(encoded)
            .execute(&mut sql)
            .await
            .unwrap();
        assert!(persistence::load(&config.database()).await.is_err());
    }
}
