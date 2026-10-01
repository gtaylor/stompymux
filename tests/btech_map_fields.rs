//! Map field edits share domain controls, native/Lua transactions and persistent state.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

#[tokio::test]
async fn every_writable_field_matches_lua_and_restarts_across_chassis() {
    for template in firing::templates() {
        let (_dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(&template, None, include_str!("../game/mechs/AS7-D")).await;
        let map = world.btech.units()[&unit].map.unwrap();
        let actor = world.create(&config, "Field editor".into(), Kind::Player);
        let player = world.objects.get_mut(&actor).unwrap();
        player.flags.insert(Flag::Wizard);
        player.location = Some(map);
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        for (field, value) in [
            ("cfmax", "50000"),
            ("cf", "200"),
            ("regen_factor", "-2147483648"),
            ("gravity", "1000"),
            ("temperature", "-1000"),
            ("maplight", "1"),
            ("mapvis", "3"),
            ("winddir", "359"),
            ("windspeed", "50000"),
            ("cloudbase", "-50000"),
            ("flags", "dark special_rules"),
            ("sensorflags", "radar,probes"),
            ("MaPnAmE", "A map with spaces and unicode 海海海海"),
            ("mapname", "海海海海海海海海海海"),
        ] {
            let output = support::run_text(
                &native,
                &config,
                actor,
                1,
                &format!("@setmap {field} {value}"),
            );
            assert!(output.is_empty(), "{field}: {output}");
            lua.eval_callback::<()>(&format!(
                "btech.map.set_field({},{},{:?},{:?})",
                actor.0, map.0, field, value
            ))
            .unwrap();
            assert_eq!(native.world().btech, lua.world().btech, "{field}");
            assert!(lua.drain_outbox().is_empty());
        }
        let saved = native.world().clone();
        let record = &saved.btech.maps()[&map];
        assert_eq!(record.building.maximum_integrity, 32767);
        assert_eq!(record.building.integrity, 200);
        assert_eq!(record.building.regeneration, i64::from(i32::MIN));
        assert_eq!(record.building_repair, None);
        assert_eq!((record.gravity, record.temperature), (127, -128));
        assert_eq!(
            (record.light, record.visibility, record.maximum_visibility),
            (1, 3, 24)
        );
        assert_eq!((record.wind_direction, record.wind_speed), (359, 32767));
        assert_eq!(record.cloud_base, -32768);
        assert_eq!(record.flags, 2 | 32);
        assert_eq!(record.sensor_flags, 32 | 64);
        assert!(record.name.len() <= 29);
        assert_eq!(record.name, "海海海海海海海海海");
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
    }
}

#[tokio::test]
async fn invalid_fields_and_callback_failure_restore_state_without_output() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Map".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        BattleMapAsset::parse("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for (field, value) in [
        ("maplight", "3"),
        ("mapvis", "61"),
        ("winddir", "360"),
        ("windspeed", "-1"),
        ("cf", "1"),
        ("cfmax", "-1"),
        ("gravity", "2147483648"),
        ("flags", "ab"),
        ("sensorflags", "!"),
        ("mapname", "bad\nname"),
        ("map", "1"),
        ("maxvis", "30"),
        ("firstfree", "0"),
        ("buildonmap", "1"),
        ("mapheight", "1"),
        ("mapwidth", "1"),
    ] {
        assert!(
            set_battle_map_field_action(&scripts, &config, ObjectId(1), map, field, value).is_err(),
            "{field}"
        );
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
    }
    assert!(set_battle_map_field_action(&scripts, &config, map, map, "mapvis", "3").is_err());
    assert!(
        set_battle_map_field_action(&scripts, &config, ObjectId(1), ObjectId(-1), "mapvis", "3")
            .is_err()
    );
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.map.set_field(1,{},'mapvis','3');error('abort')",
                map.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    for (value, expected) in [
        ("-1000", 128),
        ("-1", 255),
        ("0", 0),
        ("100", 100),
        ("1000", 127),
    ] {
        set_battle_map_field_action(&scripts, &config, ObjectId(1), map, "gravity", value).unwrap();
        assert_eq!(scripts.world().btech.maps()[&map].gravity, expected);
        assert_eq!(
            scripts.world().btech.maps()[&map].flags,
            before.maps()[&map].flags
        );
    }
    set_battle_map_field_action(&scripts, &config, ObjectId(1), map, "gravity", "100").unwrap();
    for input in ["@setmap", "@setmap mapvis", "@setmap/nope mapvis 3"] {
        assert!(!support::run_text(&scripts, &config, ObjectId(1), 1, input).is_empty());
        assert_eq!(scripts.world().btech, before);
    }
}

#[tokio::test]
async fn lua_map_flags_use_typed_constants_and_preserve_other_flags() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Map".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        BattleMapAsset::parse("1 1\n&0\n").unwrap(),
    )
    .unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let flags = |scripts: &Scripts| scripts.world().btech.maps()[&map].flags;
    assert_eq!(flags(&scripts), BattleMapFlag::PermanentFire.bit());
    let enabled: (bool, bool, usize) = scripts
        .eval_callback(&format!(
            "btech.map.set_flag(1, {0}, btech.map.flags.DARK, true)
             return btech.map.has_flag({0}, btech.map.flags.DARK),
                 btech.map.has_flag({0}, btech.map.flags.VACUUM),
                 #btech.map.inspect({0}).flags",
            map.0
        ))
        .unwrap();
    assert_eq!(enabled, (true, false, 2));
    assert_eq!(
        flags(&scripts),
        BattleMapFlag::PermanentFire.bit() | BattleMapFlag::Dark.bit()
    );
    scripts
        .eval_callback::<()>(&format!(
            "btech.map.set_flag(1, {map}, btech.map.flags.PERMANENT_FIRE, false)",
            map = map.0
        ))
        .unwrap();
    assert_eq!(flags(&scripts), BattleMapFlag::Dark.bit());
    let before = scripts.world().btech.clone();
    for script in [
        format!("btech.map.set_flag(1, {}, 32, true)", map.0),
        format!(
            "btech.map.set_flag(1, {}, btech.map.light_levels.DAY, true)",
            map.0
        ),
        format!(
            "btech.map.set_flag({0}, {0}, btech.map.flags.DARK, false)",
            map.0
        ),
    ] {
        assert!(scripts.eval_callback::<()>(&script).is_err(), "{script}");
        assert_eq!(scripts.world().btech, before);
    }
}
