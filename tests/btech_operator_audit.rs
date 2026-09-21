//! Operator diagnostics share native/Lua actions and obey transaction output limits.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Enable wizard diagnostics explicitly; the fixture defaults to disabled.
fn enable(config: &Config, scripts: &mut Scripts) -> Config {
    let candidate = config
        .administer(
            &config::administration::Request {
                directive: "wizard".into(),
                value: "yes".into(),
            },
            &scripts.world(),
            ObjectId(1),
            scripts.commands(),
        )
        .unwrap();
    scripts.configure(&candidate.config).unwrap();
    candidate.config
}

#[tokio::test]
async fn native_and_lua_edits_audit_canonical_names_and_rollback() {
    let (_dir, config, world) = support::isolated_world().await;
    let mut scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let config = enable(&config, &mut scripts);
    assert!(logging::Category::Wizard.enabled(&config));
    for (command, lua, expected) in [
        (
            "setvrt Magna.IS.SmallLaser 127",
            "btech.weapon.set_recycle(1,'IS.SmallLaser',127)",
            "VRT for IS.SmallLaser set to 127 by #1",
        ),
        (
            "setxplevel PilBip 17",
            "btech.character.set_threshold(1,'PilBip',17)",
            "Exp threshold for Piloting-Biped changed to 17 by #1",
        ),
        (
            "@btech skill-threshold PilBip=19",
            "btech.character.set_threshold(1,'PilBip',19)",
            "Exp threshold for Piloting-Biped changed to 19 by #1",
        ),
        (
            "@btech setvrt IS.SmallLaser=12",
            "btech.weapon.set_recycle(1,'IS.SmallLaser',12)",
            "VRT for IS.SmallLaser set to 12 by #1",
        ),
    ] {
        support::run_text(&scripts, &config, ObjectId(1), 1, command);
        let native = scripts.drain_records_for_inspection();
        assert_eq!(native.len(), 1, "{command}");
        assert!(native[0].text.contains("WIZ/CHANGE"));
        assert!(native[0].text.ends_with(&format!("{expected}\n")));
        scripts.eval_callback::<()>(lua).unwrap();
        let records = scripts.drain_records_for_inspection();
        assert_eq!(records.len(), 1);
        assert!(records[0].text.ends_with(&format!("{expected}\n")));
    }
    let before = scripts.world().btech.clone();
    for lua in [
        "btech.weapon.set_recycle(1,'IS.SmallLaser',1);error('abort')",
        "btech.character.set_threshold(1,'PilBip',0);error('abort')",
        "btech.weapon.set_recycle(4,'IS.SmallLaser',1)",
        "btech.character.set_threshold(4,'PilBip',1)",
        "btech.weapon.set_recycle(1,'IS.SmallLaser',128)",
        "btech.character.set_threshold(1,'Build',1)",
    ] {
        assert!(scripts.eval_callback::<()>(lua).is_err());
        assert!(scripts.drain_records_for_inspection().is_empty());
        assert_eq!(scripts.world().btech, before);
    }
    scripts
        .eval_callback::<()>("btech.weapon.set_battle_value(1,'IS.SmallLaser',17)")
        .unwrap();
    assert!(scripts.drain_records_for_inspection().is_empty());
}

#[tokio::test]
async fn audit_limits_restore_candidate_and_respect_topic_controls() {
    let (dir, _, world) = support::isolated_world().await;
    let path = dir.path().join("stompymux.toml");
    let mut table: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
    table
        .entry("lua")
        .or_insert(toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .unwrap()
        .insert("output_entry_limit".into(), 1.into());
    std::fs::write(&path, toml::to_string(&table).unwrap()).unwrap();
    let config = Config::load(dir.path()).unwrap();
    let mut scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let config = enable(&config, &mut scripts);
    edit_battle_weapon_settings(&scripts, &config, ObjectId(1), "IS.SmallLaser", 17, true).unwrap();
    let before = scripts.world().btech.clone();
    assert!(edit_battle_skill_threshold(&scripts, &config, ObjectId(1), "PilBip", 1).is_err());
    assert_eq!(scripts.world().btech, before);
    assert_eq!(scripts.drain_records_for_inspection().len(), 1);
    assert!(scripts.eval_callback::<()>("btech.weapon.set_recycle(1,'IS.SmallLaser',1); btech.character.set_threshold(1,'PilBip',1)").is_err());
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_records_for_inspection().is_empty());
    let candidate = config
        .administer(
            &config::administration::Request {
                directive: "wizard".into(),
                value: "no".into(),
            },
            &scripts.world(),
            ObjectId(1),
            scripts.commands(),
        )
        .unwrap();
    scripts.configure(&candidate.config).unwrap();
    scripts.eval_callback::<()>("btech.weapon.set_recycle(1,'IS.SmallLaser',1); btech.character.set_threshold(1,'PilBip',1)").unwrap();
    assert!(scripts.drain_records_for_inspection().is_empty());
    assert_eq!(
        battle_skill_threshold(&scripts.world(), "PilBip").unwrap(),
        1
    );
}

#[tokio::test]
async fn diagnostic_effects_follow_savepoints_inheritance_and_commit() {
    let (_dir, config, _) = support::isolated_world().await;
    let outbox = Outbox::default();
    let effects = Effects::new(&config, &outbox);
    let first = logging::Record::new(&config, "WIZ", "CHANGE", "first");
    effects.stage_record(first.clone()).unwrap();
    let checkpoint = effects.checkpoint();
    effects
        .stage_record(logging::Record::new(&config, "WIZ", "CHANGE", "discard"))
        .unwrap();
    effects.restore(checkpoint);
    let loading = Effects::new(&config, &outbox);
    loading.stage_record(first.clone()).unwrap();
    loading.inherit(&effects);
    assert_eq!(loading.drain_records()[0].text, first.text);
    effects.commit();
    assert_eq!(effects.drain_records()[0].text, first.text);
    effects.stage_record(first).unwrap();
    effects.rollback();
    assert!(effects.drain_records().is_empty());
}
