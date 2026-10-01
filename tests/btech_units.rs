//! Unit construction, durable definitions, callback rollback, and purge integration.
use crate::support;
use sqlx::{Connection, SqliteConnection};
use stompymux_rs::{
    BattleSection, BattleTemplate, BattleUnit, Flag, Kind, ObjectId, Scripts, create_battle_unit,
    dbck, persistence,
};
const JENNER: &str = include_str!("fixtures/btech/mechs/JR7-D.toml");
const ATLAS: &str = include_str!("fixtures/btech/mechs/AS7-D.toml");

#[test]
fn construction_sets_original_protection_and_independent_ammunition() {
    let unit = BattleUnit::from_template(BattleTemplate::parse("JR7-D", JENNER).unwrap()).unwrap();
    assert_eq!(unit.sections()[&BattleSection::CenterTorso].armor, 10);
    assert_eq!(unit.sections()[&BattleSection::CenterTorso].internal, 11);
    assert_eq!(unit.sections()[&BattleSection::CenterTorso].rear, 3);
    assert_eq!(unit.ammunition(), &[25]);
    let atlas = BattleUnit::from_template(BattleTemplate::parse("AS7-D", ATLAS).unwrap()).unwrap();
    assert_eq!(atlas.ammunition(), &[15, 6, 6, 5, 5]);
    let ams_source = JENNER.replace("IS.MediumLaser", "CL.Anti-MissileSystem");
    let ams =
        BattleUnit::from_template(BattleTemplate::parse("JR7-D", &ams_source).unwrap()).unwrap();
    assert_eq!(
        ams.loadout()
            .unwrap()
            .weapons
            .iter()
            .filter(|mount| mount.weapon.is_ams())
            .count(),
        4
    );
    assert!(ams.ams_enabled());
    for source in [
        JENNER.replace("\"FlipArms\"", "\"TripleStrengthMyomer\""),
        JENNER.replace("IS.MediumLaser", "IS.UnknownDefense"),
    ] {
        assert_ne!(source, JENNER);
        assert!(
            BattleUnit::from_template(BattleTemplate::parse("JR7-D", &source).unwrap()).is_err()
        );
    }
    let unknown_field = JENNER.replace("computer = 2", "unknown_field = 2");
    assert_ne!(unknown_field, JENNER);
    assert!(BattleTemplate::parse("JR7-D", &unknown_field).is_err());
}

#[tokio::test]
async fn constructed_units_survive_source_removal_and_purge_atomically() {
    let (dir, config, mut world) = support::isolated_world().await;
    std::fs::create_dir_all(dir.path().join("mechs")).unwrap();
    std::fs::write(dir.path().join("mechs/JR7-D.toml"), JENNER).unwrap();
    let first = world.create(&config, "First Jenner".into(), Kind::Thing);
    let second = world.create(&config, "Second Jenner".into(), Kind::Thing);
    for id in [first, second] {
        let object = world.objects.get_mut(&id).unwrap();
        object.location = Some(ObjectId(config.start()));
        object.home = Some(ObjectId(config.home()));
    }
    persistence::save(&config.database(), &world).await.unwrap();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let result = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        &format!("@btech unit-create #{}=JR7-D", first.0),
    );
    assert!(result.contains("constructed"), "{result}");
    let candidate = scripts.world().clone();
    persistence::save(&config.database(), &candidate)
        .await
        .unwrap();
    let error = scripts.eval_callback::<()>(&format!(
        "btech.unit.create({},'JR7-D'); error('abort')",
        second.0
    ));
    assert!(error.is_err());
    assert!(!scripts.world().btech.units().contains_key(&second));
    scripts
        .eval_callback::<()>(&format!("btech.unit.create({},'JR7-D')", second.0))
        .unwrap();
    let candidate = scripts.world().clone();
    let mut sql = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(config.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TRIGGER deny_registration BEFORE INSERT ON btech_special_registrations BEGIN SELECT RAISE(ABORT,'registration failure'); END").execute(&mut sql).await.unwrap();
    assert!(
        persistence::save(&config.database(), &candidate)
            .await
            .is_err()
    );
    let persisted = persistence::load(&config.database()).await.unwrap();
    assert_eq!(persisted.btech.constructed_units().len(), 1);
    assert!(!persisted.btech.units().contains_key(&second));
    sqlx::query("DROP TRIGGER deny_registration")
        .execute(&mut sql)
        .await
        .unwrap();
    sql.close().await.unwrap();
    persistence::save(&config.database(), &candidate)
        .await
        .unwrap();
    let armor: u16 = scripts.eval_callback(&format!("local u=btech.unit.state({}); u.sections.CenterTorso.armor=0; return btech.unit.state({}).sections.CenterTorso.armor", first.0, first.0)).unwrap();
    assert_eq!(armor, 10);
    std::fs::remove_file(dir.path().join("mechs/JR7-D.toml")).unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech.constructed_units().len(), 2);
    assert_eq!(loaded.btech, candidate.btech);
    let unchanged = loaded.btech.clone();
    assert!(
        create_battle_unit(
            &mut loaded,
            first,
            BattleTemplate::parse("JR7-D", JENNER).unwrap()
        )
        .is_err()
    );
    assert_eq!(loaded.btech, unchanged);
    loaded
        .objects
        .get_mut(&first)
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
    assert!(!loaded.btech.units().contains_key(&first));
    assert!(loaded.btech.constructed_units().contains_key(&second));
    let mut sql = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(config.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM btech_units")
            .fetch_one(&mut sql)
            .await
            .unwrap(),
        1
    );
    sqlx::query("UPDATE btech_units SET unit='{}'")
        .execute(&mut sql)
        .await
        .unwrap();
    assert!(persistence::load(&config.database()).await.is_err());
}
