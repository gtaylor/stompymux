//! Durable dice streams, transactional retries, invalid requests and script privacy.
use crate::support;
use sqlx::{Connection, SqliteConnection};
use stompymux_rs::{
    Dice, Kind, MechTemplate, ObjectId, Scripts, VehicleTemplate, create_battle_unit,
    create_battle_vehicle, persistence, roll_unit_dice,
};

#[test]
fn seeded_dice_resume_across_serialization_and_cover_all_two_dice_results() {
    let mut dice = Dice::seeded([7; 32]);
    for _ in 0..7 {
        dice.d6();
    }
    let encoded = serde_json::to_string(&dice).unwrap();
    let mut resumed: Dice = serde_json::from_str(&encoded).unwrap();
    let mut counts = [0; 13];
    for _ in 0..4096 {
        let roll = dice.two_d6();
        assert_eq!(resumed.two_d6(), roll);
        assert!((2..=12).contains(&roll));
        counts[usize::from(roll)] += 1;
    }
    assert!(counts[2..=12].iter().all(|count| *count > 0));
    assert!(counts[7] > counts[2] && counts[7] > counts[12]);
    assert!(serde_json::from_str::<Dice>(&encoded.replace("chacha8-v1", "unknown")).is_err());
    assert_eq!(format!("{dice:?}"), "Dice(chacha8-v1)");
}

#[tokio::test]
async fn restart_and_failed_save_retry_keep_the_same_dice_and_hide_state_from_lua() {
    verify_saved_dice(false).await;
}

#[tokio::test]
async fn vehicle_restart_and_failed_save_retry_preserve_private_dice() {
    verify_saved_dice(true).await;
}

/// Both owned unit classes persist the stream atomically and keep it out of script state.
async fn verify_saved_dice(vehicle: bool) {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Dice Jenner".into(), Kind::Thing);
    let object = world.objects.get_mut(&id).unwrap();
    object.location = Some(ObjectId(config.start()));
    object.home = Some(ObjectId(config.home()));
    if vehicle {
        create_battle_vehicle(
            &mut world,
            id,
            VehicleTemplate::parse("Demolisher", include_str!("../game/units/Demolisher.toml"))
                .unwrap(),
        )
        .unwrap();
    } else {
        create_battle_unit(
            &mut world,
            id,
            MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap(),
        )
        .unwrap();
    }
    persistence::save(&config.database(), &world).await.unwrap();
    let checkpoint = world.clone();
    for count in [0, 21, 255] {
        assert!(roll_unit_dice(&mut world, id, count).is_err());
    }
    assert!(roll_unit_dice(&mut world, ObjectId(-1), 2).is_err());
    assert_eq!(world.btech, checkpoint.btech);
    let expected = roll_unit_dice(&mut world, id, 20).unwrap();
    let mut sql = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    let trigger = if vehicle {
        "CREATE TRIGGER reject_dice BEFORE UPDATE ON btech_vehicles BEGIN SELECT RAISE(ABORT,'dice save failure'); END;"
    } else {
        "CREATE TRIGGER reject_dice BEFORE UPDATE ON btech_units BEGIN SELECT RAISE(ABORT,'dice save failure'); END;"
    };
    sqlx::raw_sql(trigger).execute(&mut sql).await.unwrap();
    assert!(persistence::save(&config.database(), &world).await.is_err());
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, checkpoint.btech);
    // The transaction owner restores its checkpoint before retrying the complete action.
    world = checkpoint;
    assert_eq!(roll_unit_dice(&mut world, id, 20).unwrap(), expected);
    sqlx::query("DROP TRIGGER reject_dice")
        .execute(&mut sql)
        .await
        .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    assert_eq!(
        roll_unit_dice(&mut loaded, id, 20).unwrap(),
        roll_unit_dice(&mut world, id, 20).unwrap()
    );
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let hidden: bool = scripts.eval_callback(&format!("local u=btech.unit.state({}); return u.dice == nil and u.definition ~= nil and u.sections ~= nil", id.0)).unwrap();
    assert!(hidden);
}

#[tokio::test]
async fn vehicle_streams_are_independent_and_reject_unknown_algorithms() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let mut ids = Vec::new();
    for name in ["First vehicle", "Second vehicle"] {
        let id = world.create(&config, name.into(), Kind::Thing);
        let object = world.objects.get_mut(&id).unwrap();
        object.home = Some(ObjectId(config.home()));
        object.location = Some(ObjectId(config.start()));
        create_battle_vehicle(
            &mut world,
            id,
            VehicleTemplate::parse("Demolisher", include_str!("../game/units/Demolisher.toml"))
                .unwrap(),
        )
        .unwrap();
        ids.push(id);
    }
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    for &id in &ids {
        encoded["vehicles"][id.0.to_string()]["dice"] =
            serde_json::to_value(Dice::seeded([19; 32])).unwrap();
    }
    world.btech = serde_json::from_value(encoded).unwrap();
    let checkpoint = world.clone();
    let first = roll_unit_dice(&mut world, ids[0], 20).unwrap();
    assert_eq!(
        world.btech.vehicles()[&ids[1]],
        checkpoint.btech.vehicles()[&ids[1]]
    );
    assert_ne!(
        world.btech.vehicles()[&ids[0]],
        checkpoint.btech.vehicles()[&ids[0]]
    );
    assert_eq!(first, roll_unit_dice(&mut world, ids[1], 20).unwrap());
    let mut invalid = serde_json::to_value(&world.btech.vehicles()[&ids[0]]).unwrap();
    invalid["dice"]["algorithm"] = "unknown".into();
    assert!(serde_json::from_value::<stompymux_rs::Vehicle>(invalid).is_err());
}
