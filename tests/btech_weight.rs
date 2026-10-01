//! Wizard allocation reports retain design mass across live losses and share native/Lua publication.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Report totals count intact parts and installed bins rather than depleted live ammunition.
#[tokio::test]
async fn weight_reports_share_design_accounting_and_survive_damage_and_restart() {
    for (index, source) in firing::templates().iter().enumerate() {
        let (_dir, config, world, unit, _, _) =
            firing::fixture_with_supply(source, Some(BattleWeapon::Srm2), source, false, Some(""))
                .await;
        let expected = if index < 2 {
            let design = BattleUnit::from_template(
                world.btech.constructed_units()[&unit].definition().clone(),
            )
            .unwrap();
            let mass = design.mass().unwrap();
            let bins: u32 = design
                .loadout()
                .unwrap()
                .ammunition
                .iter()
                .map(|bin| if bin.half_ton { 512 } else { 1024 })
                .sum();
            i64::from(mass.total - mass.ammunition + bins)
        } else {
            i64::from(
                world.btech.vehicles()[&unit]
                    .definition()
                    .mass()
                    .unwrap()
                    .design_total,
            )
        };
        let report = battle_weight_report(&world, ObjectId(1), unit).unwrap();
        assert_eq!(report.total, expected);
        assert_eq!(
            report.total,
            report.entries.iter().map(|entry| entry.mass).sum::<i64>()
        );
        assert!(
            report
                .entries
                .iter()
                .any(|entry| entry.name == BattleWeapon::Srm2.name() && entry.mass > 0)
        );
        assert!(
            report
                .entries
                .iter()
                .any(|entry| entry.name.starts_with("Ammo_") && entry.mass > 0)
        );
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let text: String = scripts
            .eval_callback(&format!("return btech.unit.weight(1, {})", unit.0))
            .unwrap();
        assert_eq!(text, report.render());
        let output = scripts.drain_outbox();
        assert_eq!(output.len(), text.split("\r\n").count());
        assert!(
            output
                .iter()
                .all(|(recipient, _)| *recipient == ObjectId(1))
        );
        for args in ["", "ignored trailing arguments"] {
            let native = support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("@weight {args}"),
            );
            assert!(native.contains("Weight totals for"), "{native}");
            assert!(native.contains("Total:"), "{native}");
            assert_eq!(scripts.world().btech, world.btech);
        }
        firing::edit(&mut scripts.world_mut(), unit, |state| {
            for rounds in state["ammunition"].as_array_mut().unwrap() {
                *rounds = 0.into();
            }
        });
        battle_damage_action(
            &scripts,
            &config,
            ObjectId(1),
            unit,
            BattleScenarioSalvo {
                damage: 1000,
                clusters: 4,
                rear: false,
                critical: false,
            },
        )
        .unwrap();
        assert_eq!(
            battle_weight_report(&scripts.world(), ObjectId(1), unit).unwrap(),
            report
        );
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            battle_weight_report(&restored, ObjectId(1), unit).unwrap(),
            report
        );
    }
}

/// Reporting is wizard-only, accepts unplaced designs and rolls back partial output and callbacks.
#[tokio::test]
async fn weight_report_permissions_literal_names_and_output_rollback() {
    for source in firing::templates() {
        let (dir, config, mut world) = support::isolated_world().await;
        let unit = world.create(&config, "Unit".into(), Kind::Thing);
        world.objects.get_mut(&unit).unwrap().home = Some(ObjectId(config.home()));
        let mut template = BattleUnitTemplate::parse("test",&source).unwrap();
        let criticals: Vec<_> = match &mut template {
            BattleUnitTemplate::Mech(definition) => definition
                .sections
                .values_mut()
                .flat_map(|section| section.criticals.values_mut())
                .collect(),
            BattleUnitTemplate::Vehicle(definition) => definition
                .sections
                .values_mut()
                .flat_map(|section| section.criticals.values_mut())
                .collect(),
        };
        for critical in criticals {
            if critical.equipment.starts_with("Ammo_") {
                critical.data = "0".into();
                critical.modes.push("Halfton".into());
            }
        }
        template.create(&mut world, unit).unwrap();
        let allocation = battle_weight_report(&world, ObjectId(1), unit).unwrap();
        let bins: Vec<_> = allocation
            .entries
            .iter()
            .filter(|entry| entry.name.starts_with("Ammo_"))
            .collect();
        assert!(!bins.is_empty());
        for bin in bins {
            assert_eq!(bin.mass, i64::from(bin.count.unwrap()) * 512);
        }

        let visitor = world.create(&config, "Visitor".into(), Kind::Player);
        assert!(battle_weight_report(&world, visitor, unit).is_err());
        let mut gone = world.clone();
        gone.objects
            .get_mut(&unit)
            .unwrap()
            .flags
            .insert(Flag::Going);
        assert!(battle_weight_report(&gone, ObjectId(1), unit).is_err());
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        assert!(
            scripts
                .eval_callback::<()>(&format!("btech.unit.weight(1, {}); error('abort')", unit.0))
                .is_err()
        );
        assert!(scripts.drain_outbox().is_empty());
        assert_eq!(scripts.world().btech, world.btech);
        let report = BattleWeightReport {
            name: "[fg=red]Literal".into(),
            nominal_tons: 1,
            entries: vec![BattleWeightEntry {
                name: "[bold]Part".into(),
                count: Some(2),
                mass: 2048,
            }],
            total: 2048,
        };
        assert!(
            report
                .render()
                .contains(&stompymux_rs::text::escape("[fg=red]Literal"))
        );
        assert!(
            report
                .render()
                .contains(&stompymux_rs::text::escape("[bold]Part"))
        );
        assert!(report.render().contains("offset: -1.0"));
        let path = dir.path().join("stompymux.toml");
        let mut table: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
        table
            .entry("lua")
            .or_insert(toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .unwrap()
            .insert("output_entry_limit".into(), 2.into());
        std::fs::write(path, toml::to_string(&table).unwrap()).unwrap();
        let config = Config::load(dir.path()).unwrap();
        let limited = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let error = battle_weight_action(&limited, ObjectId(1), unit).unwrap_err();
        assert!(error.to_string().contains("output limit"), "{error:#}");
        assert!(limited.drain_outbox().is_empty());
        assert_eq!(limited.world().btech, world.btech);
    }
}
