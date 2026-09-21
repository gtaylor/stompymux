//! Wizard station field editing, shared reports, independent coordinates and transactional persistence.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod firing;
use crate::support;

/// Field edits never mutate physical parent state and survive reload with exact integer semantics.
#[tokio::test]
async fn station_fields_share_native_lua_and_persistence() {
    for template in firing::templates() {
        let (dir, config, mut world, parent, target, _) =
            firing::fixture_with_target(&template, Some(BattleWeapon::MediumLaser), &template)
                .await;
        let visitor = world.create(&config, "Visitor".into(), Kind::Player);
        let station = world.create(&config, "Station".into(), Kind::Thing);
        register_gunner_station(&mut world, ObjectId(1), station, parent, 0).unwrap();
        stop_battle_unit(
            &mut world,
            parent,
            ObjectId(1),
            BattleMovementRules::STANDARD.fall,
        )
        .unwrap();
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(station);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let physical = serde_json::to_value(&scripts.world().btech).unwrap();
        for (field, value) in [
            ("arcs", "31".into()),
            ("parent", parent.0.to_string()),
            ("gunner", "1".into()),
            ("target", target.0.to_string()),
            ("targx", "40000".into()),
            ("targy", "-40000".into()),
            ("targz", "22".into()),
            ("lockmode", "0".into()),
        ] {
            let native = support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("@setturret {field} {value}"),
            );
            assert!(native.is_empty(), "{field}: {native}");
            let expected = scripts.world().btech.clone();
            scripts
                .eval_callback::<()>(&format!(
                    "btech.gunner.set_field(1, {}, '{}', '{}')",
                    station.0,
                    field.to_ascii_uppercase(),
                    value
                ))
                .unwrap();
            assert_eq!(scripts.world().btech, expected);
        }
        let record = scripts.world().btech.gunner_stations()[&station].clone();
        assert_eq!(record.target, target);
        assert_eq!(record.target_coordinates, [i16::MAX, i16::MIN, 22]);
        let after = serde_json::to_value(&scripts.world().btech).unwrap();
        assert_eq!(after["constructed"], physical["constructed"]);
        assert_eq!(after["vehicles"], physical["vehicles"]);
        for options in ["", "1", "4", "1targ", "4parent", "missing"] {
            let native = support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("@viewturret {options}"),
            );
            let lua: String = scripts
                .eval_callback(&format!(
                    "return btech.gunner.view_fields(1, {}, '{options}')",
                    station.0
                ))
                .unwrap();
            scripts.drain_outbox();
            assert_eq!(
                native.lines().collect::<Vec<_>>(),
                lua.lines().collect::<Vec<_>>()
            );
        }
        let before = scripts.world().btech.clone();
        for (field, value) in [
            ("unknown", "1"),
            ("target", "#4"),
            ("arcs", "2147483648"),
            ("targx", "2147483648"),
            ("arcs", "1 2"),
        ] {
            assert!(
                set_gunner_field(&scripts, &config, ObjectId(1), station, field, value).is_err()
            );
            assert_eq!(scripts.world().btech, before);
        }
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.gunner.set_field(1, {}, 'arcs', '0'); error('abort')",
                    station.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        assert!(set_gunner_field(&scripts, &config, visitor, station, "arcs", "0").is_err());
        assert!(view_gunner_fields(&scripts, visitor, station, "").is_err());
        // Deferred parent references remain inspectable but cannot admit combat operators.
        set_gunner_field(&scripts, &config, ObjectId(1), station, "parent", "-1").unwrap();
        assert!(gunner_context(&scripts.world(), station, ObjectId(1)).is_err());
        assert!(
            view_gunner_fields(&scripts, ObjectId(1), station, "parent")
                .unwrap()
                .contains("-1")
        );
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            restored.btech.gunner_stations(),
            saved.btech.gunner_stations()
        );
        // A report that exceeds publication capacity must leave no partial lines behind.
        let path = dir.path().join("stompymux.toml");
        let mut table: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
        table
            .entry("lua")
            .or_insert(toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .unwrap()
            .insert("output_entry_limit".into(), 2.into());
        std::fs::write(path, toml::to_string(&table).unwrap()).unwrap();
        let limited_config = Config::load(dir.path()).unwrap();
        let limited = Scripts::new(&limited_config, Rc::new(RefCell::new(restored))).unwrap();
        assert!(view_gunner_fields(&limited, ObjectId(1), station, "").is_err());
        assert!(limited.drain_outbox().is_empty());
        assert_eq!(limited.world().btech, saved.btech);
    }
}
