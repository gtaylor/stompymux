//! BattleTech asset, command, scripting, persistence-preservation and lifecycle scenarios.
use crate::support;
use sqlx::{Connection, Row, SqliteConnection};
use std::path::Path;
use stompymux_rs::{
    BattleMapAsset, BattleSection, BattleTemplate, BtechState, Flag, Kind, ObjectId, Scripts,
    Terrain, dbck, persistence, read_battle_map, read_battle_template,
};

/// Install only isolated BattleTech fixture assets into a temporary game directory.
fn assets(root: &Path) {
    support::copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/btech"),
        root,
    );
}

#[test]
fn supplied_templates_and_maps_decode_without_asset_conversion() {
    let dir = tempfile::tempdir().unwrap();
    assets(dir.path());
    let jenner = read_battle_template(&dir.path().join("mechs"), "JR7-D").unwrap();
    assert_eq!(
        (
            jenner.name.as_str(),
            jenner.tons,
            jenner.max_speed,
            jenner.jump_speed,
            jenner.heat_sinks
        ),
        ("Jenner", 35, 118.25, 53.75, 10)
    );
    let torso = &jenner.sections[&BattleSection::CenterTorso];
    assert_eq!((torso.armor, torso.internal, torso.rear), (10, 11, 3));
    assert_eq!(torso.criticals[&10].equipment, "IS.SRM-4");
    assert_eq!(torso.criticals[&0].equipment, "Engine");
    assert_eq!(torso.criticals[&2].equipment, "Engine");
    let atlas = read_battle_template(&dir.path().join("mechs"), "AS7-D").unwrap();
    assert_eq!(
        atlas.sections[&BattleSection::CenterTorso].criticals[&10].modes,
        ["RearMount"]
    );
    let map = read_battle_map(&dir.path().join("maps"), "test.map").unwrap();
    assert_eq!((map.width, map.height, map.hexes.len()), (50, 50, 2500));
    assert_eq!(map.hex(0, 0).unwrap().terrain(), Terrain::Grassland);
    let environment = read_battle_map(&dir.path().join("maps"), "environment.map").unwrap();
    assert_eq!(
        (
            environment.flags,
            environment.gravity,
            environment.temperature
        ),
        (34, 88, 19)
    );
}

#[test]
fn malformed_templates_do_not_become_partially_supported_units() {
    let source = include_str!("fixtures/btech/mechs/JR7-D.toml");
    for malformed in [
        source.replace("walk_mp = 11", "max_speed = nan"),
        source.replace("movement = \"biped\"", "movement = \"quad\""),
        source.replace("class = \"mech\"", "class = \"Mech\""),
        source.replace(
            "at = \"1-2\", item = \"JumpJet\"",
            "at = \"0-2\", item = \"JumpJet\"",
        ),
        source.replace(
            "at = \"3-4\", item = \"IS.MediumLaser\"",
            "at = \"2-3\", item = \"IS.MediumLaser\"",
        ),
        source.replace("tons = 35", "tons = -1"),
        source.replace("name = \"Jenner\"", "name = \"Jenner"),
        format!("reference = \"JR7-D\"\n{source}"),
        format!("{source}\n[sections.head]\narmor = 1\n"),
    ] {
        assert_ne!(malformed, source);
        assert!(
            BattleTemplate::parse("JR7-D", &malformed).is_err(),
            "unexpectedly parsed {malformed}"
        );
    }
    let multiline = source.replace("name = \"Jenner\"", "name = \"\"\"\nJenner\"\"\"");
    assert_eq!(
        BattleTemplate::parse("JR7-D", &multiline).unwrap().name,
        "Jenner"
    );
    // A malformed settings line rejects the map instead of being ignored.
    assert!(BattleMapAsset::from_cells("1 1\n.0\n42: 88 19 extra\n").is_err());
}

#[tokio::test]
async fn commands_and_lua_inspection_have_permissions_and_callback_guards() {
    let (dir, config, scripts) = support::isolated_scripts().await;
    assets(dir.path());
    let before = serde_json::to_vec(&*scripts.world()).unwrap();
    let report = support::run_text(&scripts, &config, ObjectId(1), 1, "@btech template JR7-D");
    assert!(report.contains("Jenner (JR7-D)"), "{report}");
    assert!(report.contains("unit construction validates supported equipment and chassis rules"));
    assert!(
        support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            "@btech mapfile environment.map"
        )
        .contains("Gravity 88%")
    );
    scripts
        .world_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    assert!(
        support::run_text(&scripts, &config, ObjectId(2), 2, "@btech status")
            .contains("Permission denied")
    );
    let (name, gravity, same): (String, i64, bool) = scripts.eval_callback("local b=require('btech'); local t=b.template.inspect('JR7-D'); return t.name,b.map.inspect_file('environment.map').gravity,b==btech").unwrap();
    assert_eq!((name.as_str(), gravity, same), ("Jenner", 88, true));
    let outside: bool = scripts
        .inspect_lua()
        .load("return pcall(btech.template.inspect,'JR7-D')")
        .eval()
        .unwrap();
    assert!(!outside);
    let code: String = scripts
        .eval_callback(
            "local ok,e=pcall(btech.template.inspect,'../outside'); assert(not ok); return e.code",
        )
        .unwrap();
    assert_eq!(code, "btech.template.invalid");
    let checking = scripts
        .from_sources_for_inspection(
            &config,
            stompymux_rs::help::HelpIndex::load(&config).unwrap(),
            std::sync::Arc::new(stompymux_rs::LuaSources::read(&config).unwrap()),
            stompymux_rs::RuntimeMode::Checking,
        )
        .unwrap();
    let code: String = checking
        .inspect_lua()
        .load("local ok,e=pcall(btech.template.inspect,'JR7-D'); assert(not ok); return e.code")
        .eval()
        .unwrap();
    assert_eq!(code, "mux.unavailable.checking");
    let detached: String = scripts.eval_callback("local t=btech.template.inspect('JR7-D'); t.name='changed'; return btech.template.inspect('JR7-D').name").unwrap();
    assert_eq!(detached, "Jenner");
    // Read-only btech callbacks do not alter their loaded projection or the durable file.
    assert_eq!(scripts.world().btech, BtechState::default());
    let loaded = persistence::load(&config.database()).await.unwrap();
    let original: stompymux_rs::World = serde_json::from_slice(&before).unwrap();
    assert_eq!(loaded.btech, original.btech);
}

/// Seed all required columns of a test-only row, overriding the scenario's interesting values.
async fn seed_row(sql: &mut SqliteConnection, table: &str, id: ObjectId) {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!("PRAGMA table_info({table})")))
        .fetch_all(&mut *sql)
        .await
        .unwrap();
    let names: Vec<String> = rows.iter().map(|row| row.get("name")).collect();
    let values: Vec<_> = rows
        .iter()
        .map(|row| {
            let name: String = row.get("name");
            let kind: String = row.get("type");
            if name == "dbref" {
                return id.0.to_string();
            }
            if kind == "TEXT" {
                return "'fixture'".into();
            }
            "0".into()
        })
        .collect();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO {table} ({}) VALUES ({})",
        names.join(","),
        values.join(",")
    )))
    .execute(sql)
    .await
    .unwrap();
}

#[tokio::test]
async fn saved_identities_are_inspectable_preserved_and_cleaned_atomically() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Battlefield".into(), Kind::Room);
    let unit = world.create(&config, "Jenner".into(), Kind::Thing);
    world.objects.get_mut(&unit).unwrap().location = Some(map);
    world.objects.get_mut(&unit).unwrap().home = Some(ObjectId(config.home()));
    persistence::save(&config.database(), &world).await.unwrap();
    let mut sql = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(config.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    seed_row(&mut sql, "btech_maps", map).await;
    seed_row(&mut sql, "btech_mechs", unit).await;
    sqlx::query("UPDATE btech_mechs SET mech_name='Jenner',mech_type='JR7-D',tons=35,map_dbref=? WHERE dbref=?").bind(map.0).bind(unit.0).execute(&mut sql).await.unwrap();
    sqlx::query("UPDATE btech_maps SET width=1,height=1,gravity=75,temperature=-12 WHERE dbref=?")
        .bind(map.0)
        .execute(&mut sql)
        .await
        .unwrap();
    for (id, kind) in [(map, "MAP"), (unit, "MECH")] {
        sqlx::query("INSERT INTO btech_special_registrations VALUES(?,?)")
            .bind(id.0)
            .bind(kind)
            .execute(&mut sql)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO btech_map_hexes VALUES(?,0,0,231)")
        .bind(map.0)
        .execute(&mut sql)
        .await
        .unwrap();
    sqlx::raw_sql("ALTER TABLE btech_maps ADD COLUMN future_data BLOB; UPDATE btech_maps SET future_data=x'00ff42'").execute(&mut sql).await.unwrap();
    let mut world = persistence::load(&config.database()).await.unwrap();
    assert_eq!(world.btech.units()[&unit].template, "JR7-D");
    assert_eq!(world.btech.maps()[&map].temperature, -12);
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert!(
        support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("@btech inspect #{}", unit.0)
        )
        .contains("JR7-D")
    );
    let inspected: String = scripts
        .eval_callback(&format!("return btech.unit.inspect({}).template", unit.0))
        .unwrap();
    assert_eq!(inspected, "JR7-D");
    world.objects.get_mut(&unit).unwrap().description = Some("durable MUX edit".into());
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, Vec<u8>>("SELECT future_data FROM btech_maps WHERE dbref=?")
            .bind(map.0)
            .fetch_one(&mut sql)
            .await
            .unwrap(),
        [0, 255, 66]
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT value FROM btech_map_hexes WHERE map_dbref=?")
            .bind(map.0)
            .fetch_one(&mut sql)
            .await
            .unwrap(),
        231
    );
    let mut bad = world.clone();
    bad.btech = BtechState::default();
    bad.objects.get_mut(&unit).unwrap().description = Some("must not commit".into());
    assert!(persistence::save(&config.database(), &bad).await.is_err());
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().objects[&unit]
            .description
            .as_deref(),
        Some("durable MUX edit")
    );
    world
        .objects
        .get_mut(&map)
        .unwrap()
        .flags
        .insert(Flag::Going);
    persistence::repair(&config.database(), 5000, |raw| {
        dbck::plan(&world, raw, &config)
    })
    .await
    .unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert!(!loaded.btech.maps().contains_key(&map));
    assert!(!loaded.btech.registrations().contains_key(&map));
    assert_eq!(loaded.btech.units()[&unit].map, None);
    sql.close().await.unwrap();
}
