//! Material damage, transfer accounting, section loss and durable core destruction.
use crate::support;
use stompymux_rs::{
    DamagePhase as Phase, Kind, Mech, MechSection as Section, MechTemplate, ObjectId,
    apply_damage_phase, create_battle_unit, persistence,
};

/// A fresh Jenner with reference armor and structure quantities.
fn jenner() -> Mech {
    Mech::from_template(
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap()
}

#[test]
fn front_rear_internal_phases_and_empty_hits_account_for_every_damage_point() {
    let mut unit = jenner();
    let before = unit.clone();
    let zero = unit.damage_phase(Section::CenterTorso, 0, Phase::Armor { rear: true });
    assert_eq!(zero.absorbed, 0);
    assert_eq!(unit, before);
    let armor = unit.damage_phase(Section::CenterTorso, 5, Phase::Armor { rear: true });
    assert_eq!((armor.absorbed, armor.remaining), (3, 2));
    assert_eq!(unit.sections()[&Section::CenterTorso].armor, 10);
    assert_eq!(unit.sections()[&Section::CenterTorso].internal, 11);
    let internal = unit.damage_phase(Section::CenterTorso, armor.remaining, Phase::Internal);
    assert_eq!((internal.absorbed, internal.remaining), (2, 0));
    assert_eq!(unit.sections()[&Section::CenterTorso].internal, 9);
    let limb = unit.damage_phase(Section::LeftArm, 5, Phase::Armor { rear: true });
    assert_eq!((limb.absorbed, limb.remaining), (4, 1));
    assert_eq!(unit.sections()[&Section::LeftArm].internal, 6);
}

#[test]
fn destroyed_locations_transfer_without_absorbing_and_side_torsos_remove_arms_and_ammo() {
    let mut unit = jenner();
    // A 20-point hit crosses right arm armor/structure and then right torso armor/structure.
    let armor = unit.damage_phase(Section::RightArm, 20, Phase::Armor { rear: false });
    let structure = unit.damage_phase(Section::RightArm, armor.remaining, Phase::Internal);
    assert_eq!(structure.destroyed_sections, vec![Section::RightArm]);
    assert_eq!(structure.remaining, 10);
    let torso = Section::RightArm.damage_transfer().unwrap();
    let armor = unit.damage_phase(torso, structure.remaining, Phase::Armor { rear: false });
    assert_eq!((armor.absorbed, armor.remaining), (8, 2));
    unit.damage_phase(torso, armor.remaining, Phase::Internal);
    assert_eq!(unit.sections()[&torso].internal, 6);
    let gone = unit.damage_phase(Section::RightArm, 7, Phase::Armor { rear: false });
    assert_eq!((gone.absorbed, gone.remaining), (0, 7));
    assert!(gone.destroyed_sections.is_empty());
    let report = unit.damage_phase(torso, 6, Phase::Internal);
    assert_eq!(report.destroyed_sections, vec![torso]);
    assert_eq!(unit.ammunition(), &[0]);
    assert_eq!(unit.sections()[&torso].rear, 0);
    assert!(!unit.is_destroyed());
    let mut unit = jenner();
    let report = unit.damage_phase(Section::LeftTorso, 8, Phase::Internal);
    assert_eq!(
        report.destroyed_sections,
        vec![Section::LeftTorso, Section::LeftArm]
    );
    assert_eq!(unit.sections()[&Section::LeftArm].armor, 0);
}

#[tokio::test]
async fn core_damage_survives_restart_and_rejects_invalid_phase_targets_atomically() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Damage Jenner".into(), Kind::Thing);
    let object = world.objects.get_mut(&id).unwrap();
    object.location = Some(ObjectId(config.start()));
    object.home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        id,
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
    let map = world.create(&config, "Damage field".into(), Kind::Room);
    stompymux_rs::create_battle_map(
        &mut world,
        map,
        "damage.map",
        stompymux_rs::MapAsset::from_cells("2 2\n.0.0\n.0.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    stompymux_rs::place_battle_unit(&mut world, id, map, 0, 1).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    stompymux_rs::assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    stompymux_rs::start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        stompymux_rs::advance_battle_units(&mut world, 0);
    }
    stompymux_rs::set_battle_speed(&mut world, id, ObjectId(1), 10.0).unwrap();
    stompymux_rs::advance_battle_motion(
        &mut world,
        stompymux_rs::MovementRules {
            fasa_turning: false,
            slowdown: 2,
            ..stompymux_rs::MovementRules::STANDARD
        },
    )
    .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let before = world.clone();
    assert!(
        apply_damage_phase(&mut world, ObjectId(-1), Section::Head, 1, Phase::Internal).is_err()
    );
    assert_eq!(world.btech, before.btech);
    let armor = apply_damage_phase(
        &mut world,
        id,
        Section::Head,
        20,
        Phase::Armor { rear: false },
    )
    .unwrap();
    let report = apply_damage_phase(
        &mut world,
        id,
        Section::Head,
        armor.remaining,
        Phase::Internal,
    )
    .unwrap();
    assert!(report.unit_destroyed);
    assert_eq!(world.btech.constructed_units()[&id].pilot(), None);
    assert_eq!(world.objects[&ObjectId(1)].location, Some(id));
    assert_eq!(
        world.btech.constructed_units()[&id].power(),
        stompymux_rs::Power::Off
    );
    assert_eq!(
        world.btech.constructed_units()[&id].motion().unwrap().speed,
        0.0
    );
    assert!(stompymux_rs::start_battle_unit(&mut world, id, ObjectId(1), true).is_err());

    assert_eq!(report.remaining, 10);
    assert_eq!(Section::Head.damage_transfer(), None);
    use sqlx::Connection;
    let mut sql = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TRIGGER reject_damage BEFORE UPDATE ON btech_units BEGIN SELECT RAISE(ABORT,'damage failure'); END;").execute(&mut sql).await.unwrap();
    assert!(persistence::save(&config.database(), &world).await.is_err());
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        before.btech
    );
    sqlx::query("DROP TRIGGER reject_damage")
        .execute(&mut sql)
        .await
        .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    assert!(loaded.btech.constructed_units()[&id].is_destroyed());
    let scripts =
        stompymux_rs::Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(loaded)))
            .unwrap();
    assert!(
        scripts
            .eval_callback::<bool>(&format!("return btech.unit.state({}).destroyed", id.0))
            .unwrap()
    );
    let text = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        &format!("@btech inspect #{}", id.0),
    );
    assert!(text.contains("Unit destroyed"), "{text}");
}
