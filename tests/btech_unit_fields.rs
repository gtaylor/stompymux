//! Unit field inspection shares native/Lua authority, live projections and atomic publication.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Find a full field name independently of the selected display width.
fn field<'a>(report: &'a UnitFieldReport, name: &str) -> Option<&'a str> {
    report
        .fields
        .iter()
        .find(|field| field.name == name)
        .unwrap()
        .value
        .as_deref()
}

/// One shared fixture, VM pair and database snapshot per harness; scenarios
/// install prepared worlds, per the sandbox-reuse convention. Panic messages
/// carry the chassis template so failures stay attributable.
struct UnitFields {
    _dir: tempfile::TempDir,
    config: Config,
    base: World,
    native: Scripts,
    lua: Scripts,
    pristine_db: std::path::PathBuf,
}

impl UnitFields {
    async fn new() -> Self {
        let (_dir, config, base) = support::isolated_world().await;
        let native = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
        let pristine_db = support::snapshot_database(&config);
        Self {
            _dir,
            config,
            base,
            native,
            lua,
            pristine_db,
        }
    }

    /// Fresh scenario world for a chassis pair over the shared fixture.
    fn world(&self, source: &str) -> (World, ObjectId, ObjectId, usize) {
        self.pair(source, source)
    }

    /// Fresh scenario world with an explicit target template over the shared fixture.
    fn pair(&self, source: &str, target: &str) -> (World, ObjectId, ObjectId, usize) {
        support::restore_database(&self.config, &self.pristine_db);
        firing::supply_fixture_on(
            self.base.clone(),
            &self.config,
            source,
            None,
            target,
            false,
            None,
        )
    }

    /// Fresh scenario world with an injected weapon, optional computer and ammo flag.
    fn supply(
        &self,
        source: &str,
        weapon: Option<Weapon>,
        target_source: &str,
        ammunition_flag: Option<&str>,
    ) -> (World, ObjectId, ObjectId, usize) {
        support::restore_database(&self.config, &self.pristine_db);
        firing::supply_fixture_on(
            self.base.clone(),
            &self.config,
            source,
            weapon,
            target_source,
            false,
            ammunition_flag,
        )
    }

    fn install(&self, world: &World) {
        support::install(&self.native, world.clone());
        support::install(&self.lua, world.clone());
    }
}

async fn all_chassis_field_reports_scenario(f: &UnitFields) {
    let config = &f.config;
    for (index, source) in firing::templates().into_iter().enumerate() {
        let (mut world, id, target, _) = f.world(&source);
        set_battle_unit_experience(
            &mut world,
            id,
            UnitExperience {
                multiplier: 2.5,
                suppress_gunnery: false,
            },
        )
        .unwrap();
        let before = world.btech.clone();
        let native = &f.native;
        let lua = &f.lua;
        f.install(&world);
        let report = view_battle_unit_fields_action(native, config, ObjectId(1), id, "").unwrap();
        let lua_report: mlua::Table = lua
            .eval_callback(&format!("return btech.unit.fields(1,{})", id.0))
            .unwrap();
        assert_eq!(
            serde_json::to_value(&report).unwrap(),
            serde_json::to_value(lua_report).unwrap()
        );
        assert_eq!(field(&report, "xpmod"), Some("2.50"));
        for name in ["turret0", "turret1", "turret2"] {
            assert_eq!(field(&report, name), Some("-1"));
        }
        assert_eq!(field(&report, "unit_era"), Some("Undefined"));
        assert_eq!(field(&report, "unit_tro"), Some("Undefined"));
        assert_eq!(field(&report, "jumpheading"), Some("0"));
        assert_eq!(field(&report, "jumplength"), Some("0"));
        assert_eq!(field(&report, "pilotnum"), Some("1"));
        assert_eq!(
            field(&report, "target"),
            Some(target.0.to_string().as_str())
        );
        assert_eq!(field(&report, "x"), Some("0"));
        assert_eq!(field(&report, "y"), Some("11"));
        assert_eq!(field(&report, "z"), Some("0"));
        assert_eq!(field(&report, "fy"), Some("3708.75"));
        assert_eq!(
            field(&report, "mechtype"),
            Some(if index < 2 {
                "Mech"
            } else if index == 6 {
                "VTOL"
            } else {
                "Vehicle"
            })
        );
        assert_eq!(native.world().btech, before);
        assert_eq!(lua.world().btech, before);
        let expected = native
            .drain_outbox()
            .into_iter()
            .map(|(_, doc)| doc.source().to_owned())
            .collect::<Vec<_>>();
        let actual = lua
            .drain_outbox()
            .into_iter()
            .map(|(_, doc)| doc.source().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(expected, actual);
        let native_text = support::run_text(native, config, ObjectId(1), 1, "@viewmech");
        assert!(native_text.contains("xpmod"));
        assert_eq!(native.world().btech, before);
        let saved = native.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, before);
    }
}

async fn field_layout_filters_scenario(f: &UnitFields) {
    let sources = firing::templates();
    let config = &f.config;
    let (mut world, id, _, _) = f.world(&sources[0]);
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    world.objects.get_mut(&id).unwrap().name = "[fg=red]Hostile name[reset]".into();
    let scripts = &f.native;
    support::install(scripts, world.clone());
    let before = serde_json::to_value(&*scripts.world()).unwrap();
    for (query, columns, names) in [
        ("1XP", 1, vec!["xpmod"]),
        (
            "4mech",
            4,
            vec![
                "mechname",
                "MechPrefs",
                "mechtype",
                "mechmovetype",
                "mechdamage",
                "mechref",
            ],
        ),
        ("doesnotexist", 2, vec![]),
        ("stall", 2, vec![]),
    ] {
        let report =
            view_battle_unit_fields_action(scripts, config, ObjectId(1), id, query).unwrap();
        assert_eq!(report.columns, columns);
        assert_eq!(
            report
                .fields
                .iter()
                .map(|field| field.name)
                .collect::<Vec<_>>(),
            names
        );
        let output = scripts
            .drain_outbox()
            .into_iter()
            .map(|(_, doc)| text::plain(doc.source()))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(output, report.text);
    }
    assert!(view_battle_unit_fields_action(scripts, config, ObjectId(2), id, "").is_err());
    assert!(
        view_battle_unit_fields_action(scripts, config, ObjectId(1), ObjectId(999999), "").is_err()
    );
    scripts
        .eval_callback::<()>("mux.world.pemit(1,'PRIOR')")
        .unwrap();
    assert!(
        scripts
            .eval_callback::<()>(&format!("btech.unit.fields(1,{}); error('abort')", id.0))
            .is_err()
    );
    let output = scripts.drain_outbox();
    assert_eq!(output.len(), 1);
    assert_eq!(output[0].1.source(), "PRIOR");
    assert_eq!(serde_json::to_value(&*scripts.world()).unwrap(), before);
}

async fn named_edits_share_validation_scenario(f: &UnitFields) {
    let config = &f.config;
    for source in firing::templates() {
        let (world, id, _, _) = f.world(&source);
        let native = &f.native;
        let lua = &f.lua;
        f.install(&world);
        for (name, value) in [("TEAM", "-7"), ("xpmod", "2.5")] {
            let command = format!("@setmech {name} {value}");
            assert!(support::run_text(native, config, ObjectId(1), 1, &command).is_empty());
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'{name}','{value}')",
                id.0
            ))
            .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
        }
        let report = view_battle_unit_fields_action(lua, config, ObjectId(1), id, "1").unwrap();
        assert_eq!(field(&report, "team"), Some("-7"));
        assert_eq!(field(&report, "xpmod"), Some("2.50"));
        lua.drain_outbox();
        let before = serde_json::to_value(&*lua.world()).unwrap();
        for (name, value) in [
            ("xpmod", "NaN"),
            ("xpmod", "-1"),
            ("xpmod", "1e100"),
            ("team", "2147483648"),
            ("mapindex", "1"),
            ("unknown", "0"),
        ] {
            assert!(
                set_battle_unit_field_action(lua, config, ObjectId(1), id, name, value).is_err()
            );
            assert_eq!(serde_json::to_value(&*lua.world()).unwrap(), before);
        }
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'team','33'); error('abort')",
                id.0
            ))
            .is_err()
        );
        assert_eq!(serde_json::to_value(&*lua.world()).unwrap(), before);
        assert!(lua.drain_outbox().is_empty());
        let saved = lua.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
    }
}

async fn detached_field_geometry_scenario(f: &UnitFields) {
    let config = &f.config;
    for source in firing::templates() {
        let (world, id, _, _) = f.world(&source);
        let scripts = &f.native;
        support::install(scripts, world.clone());
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.setxy(1,{},0,9,7); btech.unit.setmapindex(1,{},-1)",
                id.0, id.0
            ))
            .unwrap();
        set_battle_unit_field_action(scripts, config, ObjectId(1), id, "team", "4").unwrap();
        let report = view_battle_unit_fields_action(scripts, config, ObjectId(1), id, "").unwrap();
        for (name, value) in [
            ("mapindex", "-1"),
            ("x", "0"),
            ("y", "9"),
            ("z", "7"),
            ("fz", "451.50"),
            ("team", "4"),
        ] {
            assert_eq!(field(&report, name), Some(value), "{name}");
        }
        scripts.world().validate(config).unwrap();
    }
}

async fn special_field_commands_scenario(f: &UnitFields) {
    let sources = firing::templates();
    let config = &f.config;
    let (mut world, unit, _, _) = f.world(&sources[0]);
    let map = world.btech.constructed_units()[&unit]
        .position()
        .unwrap()
        .map;
    stop_battle_unit(&mut world, unit, ObjectId(1), MovementRules::STANDARD.fall).unwrap();
    for (id, set, view, assignment) in [
        (map, "@setmap", "@viewmap", "mapname A field with spaces"),
        (unit, "@setmech", "@viewmech", "team 7"),
    ] {
        let mut base = world.clone();
        base.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
        let specific = &f.native;
        let generic = &f.lua;
        support::install(specific, base.clone());
        support::install(generic, base.clone());
        let expected = support::run_text(
            specific,
            config,
            ObjectId(1),
            1,
            &format!("{set} {assignment}"),
        );
        let actual = support::run_text(
            generic,
            config,
            ObjectId(1),
            1,
            &format!("@setspecial {assignment}"),
        );
        assert!(expected.is_empty(), "{expected}");
        assert_eq!(expected, actual);
        assert_eq!(specific.world().btech, generic.world().btech);
        let expected = support::run_text(specific, config, ObjectId(1), 1, &format!("{view} 1"));
        let actual = support::run_text(generic, config, ObjectId(1), 1, "@viewspecial 1");
        assert_eq!(expected, actual);
        let before = generic.world().btech.clone();
        for command in [
            "@setspecial",
            "@setspecial foo",
            "@setspecial/invalid x y",
            "@viewspecial/invalid",
        ] {
            assert!(!support::run_text(generic, config, ObjectId(1), 1, command).is_empty());
            assert_eq!(generic.world().btech, before);
        }
    }
}

async fn hardware_fields_share_gameplay_ranges_and_vtol_fuel_edits_are_atomic_scenario(
    f: &UnitFields,
) {
    let config = &f.config;
    for source in firing::templates() {
        let (world, id, _, _) = f.pair(&source, &source);
        let ranges = world.btech.constructed_units().get(&id).map_or_else(
            || world.btech.vehicles()[&id].sensor_ranges(),
            Mech::sensor_ranges,
        );
        let radio = world.btech.constructed_units().get(&id).map_or_else(
            || world.btech.vehicles()[&id].radio_capabilities(),
            Mech::radio_capabilities,
        );
        let fuel = world.btech.vehicles().get(&id).and_then(Vehicle::vtol_fuel);
        let scripts = &f.native;
        support::install(scripts, world.clone());
        let report = view_battle_unit_fields_action(scripts, config, ObjectId(1), id, "").unwrap();
        for (name, expected) in [
            ("tacrange", ranges.tactical),
            ("lrsrange", ranges.long_range),
            ("scanrange", ranges.scan),
        ] {
            assert_eq!(field(&report, name), Some(expected.to_string().as_str()));
        }
        assert_eq!(
            field(&report, "radiorange"),
            Some(radio.range.to_string().as_str())
        );
        let configuration: u8 = field(&report, "radiotype").unwrap().parse().unwrap();
        assert_eq!(configuration % 16, radio.channels);
        assert_eq!(configuration & 16 != 0, radio.relay);
        assert_eq!(configuration & 32 != 0, radio.info);
        assert_eq!(configuration & 64 != 0, radio.scan);
        assert_eq!(configuration & 128 == 0, radio.digital);
        if let Some(fuel) = fuel {
            assert_eq!(
                field(&report, "fuel_orig"),
                Some(fuel.capacity().to_string().as_str())
            );
            scripts
                .eval_callback::<()>(&format!("btech.unit.set_field(1,{},'fuel','123')", id.0))
                .unwrap();
            let report =
                view_battle_unit_fields_action(scripts, config, ObjectId(1), id, "fuel").unwrap();
            assert_eq!(field(&report, "fuel"), Some("123"));
            assert_eq!(
                field(&report, "fuel_orig"),
                Some(fuel.capacity().to_string().as_str())
            );
        } else {
            assert_eq!(field(&report, "fuel"), None);
            assert!(
                set_battle_unit_field_action(scripts, config, ObjectId(1), id, "fuel", "123")
                    .is_err()
            );
        }
        let before = scripts.world().btech.clone();
        for value in ["-1", "4294967295", "nan"] {
            assert!(
                set_battle_unit_field_action(scripts, config, ObjectId(1), id, "fuel", value)
                    .is_err()
            );
            assert_eq!(scripts.world().btech, before);
        }
    }
}

async fn identity_edits_preserve_combat_state_and_survive_restart_for_every_chassis_scenario(
    f: &UnitFields,
) {
    let config = &f.config;
    for source in firing::templates() {
        let (world, id, _, _) = f.pair(&source, &source);
        let native = &f.native;
        let lua = &f.lua;
        f.install(&world);
        let before = serde_json::to_value(&native.world().btech).unwrap();
        let name = "Custom %cr name with spaces";
        let reference = "CUSTOM-1";
        for (field_name, value) in [("mechname", name), ("mechref", reference)] {
            set_battle_unit_field_action(native, config, ObjectId(1), id, field_name, value)
                .unwrap();
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'{}','{}')",
                id.0, field_name, value
            ))
            .unwrap();
        }
        assert_eq!(native.world().btech, lua.world().btech);
        let report =
            view_battle_unit_fields_action(native, config, ObjectId(1), id, "1mech").unwrap();
        assert_eq!(field(&report, "mechname"), Some(name));
        assert_eq!(field(&report, "mechref"), Some(reference));
        let after = native.world().btech.clone();
        let mut expected = before;
        // Only identity changes; ammunition, damage, motion and random state must remain exact.
        let store = if expected["constructed"].get(id.0.to_string()).is_some() {
            "constructed"
        } else {
            "vehicles"
        };
        let definition = &mut expected[store][id.0.to_string()]["definition"];
        definition["name"] = name.into();
        definition["reference"] = reference.into();
        definition["attributes"]["name"] = name.into();
        definition["attributes"]["reference"] = reference.into();
        expected["units"][id.0.to_string()]["name"] = name.into();
        expected["units"][id.0.to_string()]["template"] = reference.into();
        assert_eq!(serde_json::to_value(&after).unwrap(), expected);
        for invalid in [String::new(), "x".repeat(129)] {
            assert!(
                set_battle_unit_field_action(native, config, ObjectId(1), id, "mechname", &invalid)
                    .is_err()
            );
            assert_eq!(native.world().btech, after);
        }
        assert!(
            native
                .eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'mechref','temporary'); error('abort')",
                    id.0
                ))
                .is_err()
        );
        assert_eq!(native.world().btech, after);
        let saved = native.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, after);
    }
}

async fn runtime_hardware_zeroes_change_gameplay_and_survive_restart_scenario(f: &UnitFields) {
    let config = &f.config;
    for source in firing::templates() {
        let (world, id, _, _) = f.pair(&source, &source);
        let native = &f.native;
        let lua = &f.lua;
        f.install(&world);
        for (name, value) in [
            ("tacrange", "0"),
            ("lrsrange", "17"),
            ("scanrange", "0"),
            ("radiorange", "0"),
            ("radiotype", "0"),
        ] {
            set_battle_unit_field_action(native, config, ObjectId(1), id, name, value).unwrap();
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'{}','{}')",
                id.0, name, value
            ))
            .unwrap();
        }
        assert_eq!(native.world().btech, lua.world().btech);
        let report = view_battle_unit_fields_action(native, config, ObjectId(1), id, "").unwrap();
        for (name, expected) in [
            ("tacrange", "0"),
            ("lrsrange", "17"),
            ("scanrange", "0"),
            ("radiorange", "0"),
            ("radiotype", "0"),
        ] {
            assert_eq!(field(&report, name), Some(expected));
        }
        assert_eq!(field(&report, "targcomp"), Some("0"));
        {
            let world = native.world();
            let (ranges, radio, channels) =
                if let Some(unit) = world.btech.constructed_units().get(&id) {
                    (
                        unit.sensor_ranges(),
                        unit.radio_capabilities(),
                        unit.radio_channels().len(),
                    )
                } else {
                    let unit = &world.btech.vehicles()[&id];
                    (
                        unit.sensor_ranges(),
                        unit.radio_capabilities(),
                        unit.radio_channels().len(),
                    )
                };
            assert_eq!(ranges.scan, 0);
            assert_eq!(ranges.tactical, 0);
            assert_eq!(ranges.long_range, 17);
            assert_eq!(radio.range, 0);
            assert_eq!(channels, 0);
        }
        let before = native.world().btech.clone();
        for (name, value) in [
            ("tacrange", "128"),
            ("scanrange", "-1"),
            ("radiorange", "32768"),
            ("radiotype", "256"),
        ] {
            assert!(
                set_battle_unit_field_action(native, config, ObjectId(1), id, name, value).is_err()
            );
            assert_eq!(native.world().btech, before);
        }
        assert!(
            native
                .eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'scanrange','25'); error('abort')",
                    id.0
                ))
                .is_err()
        );
        assert_eq!(native.world().btech, before);
        let saved = native.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, before);
    }
}

async fn thermal_fields_retain_committed_samples_and_share_cooling_with_status_scenario(
    f: &UnitFields,
) {
    let config = &f.config;
    for (index, source) in firing::templates().into_iter().enumerate() {
        let (mut world, id, _, _) = f.pair(&source, &source);
        if index < 2 {
            firing::edit(&mut world, id, |unit| unit["heat"]["stored"] = 40.0.into());
        }
        let scripts = &f.native;
        support::install(scripts, world.clone());
        let initial = view_battle_unit_fields_action(scripts, config, ObjectId(1), id, "").unwrap();
        assert_eq!(field(&initial, "heat"), Some("0.00"));
        assert_eq!(field(&initial, "dissheat"), Some("0.00"));
        advance_battle_heat(&mut scripts.world_mut());
        let report = view_battle_unit_fields_action(scripts, config, ObjectId(1), id, "").unwrap();
        let expected_sinks = if index < 2 {
            scripts.world().btech.constructed_units()[&id].cooling_capacity()
        } else {
            scripts.world().btech.vehicles()[&id]
                .cooling_capacity()
                .unwrap()
        };
        assert_eq!(
            field(&report, "heatsinks"),
            Some(expected_sinks.to_string().as_str())
        );
        assert_eq!(field(&report, "disabled_hs"), Some("0"));
        if index < 2 {
            let sample = scripts.world().btech.constructed_units()[&id].sampled_heat_rates();
            assert_eq!(sample.production, 40.0);
            assert_eq!(field(&report, "heat"), Some("40.00"));
            assert_eq!(
                field(&report, "dissheat"),
                Some(format!("{:.2}", sample.dissipation).as_str())
            );
            assert_eq!(
                field(&report, "overheat"),
                Some(format!("{:.2}", (40.0 - sample.dissipation).max(0.0)).as_str())
            );
            firing::edit(&mut scripts.world_mut(), id, |unit| {
                unit["heat"]["stored"] = 80.0.into()
            });
            let unchanged =
                view_battle_unit_fields_action(scripts, config, ObjectId(1), id, "heat").unwrap();
            assert_eq!(field(&unchanged, "heat"), Some("40.00"));
        } else {
            for name in ["heat", "dissheat", "overheat"] {
                assert_eq!(field(&report, name), Some("0.00"));
            }
        }
        let before = scripts.world().btech.clone();
        let lua: mlua::Table = scripts
            .eval_callback(&format!("return btech.unit.fields(1,{})", id.0))
            .unwrap();
        let native = view_battle_unit_fields_action(scripts, config, ObjectId(1), id, "").unwrap();
        assert_eq!(
            serde_json::to_value(lua).unwrap(),
            serde_json::to_value(native).unwrap()
        );
        assert_eq!(scripts.world().btech, before);
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, before);
        if index < 2 {
            let mut restored = restored;
            advance_battle_heat(&mut restored);
            assert_eq!(
                restored.btech.constructed_units()[&id]
                    .sampled_heat_rates()
                    .production,
                80.0
            );
        }
    }
}

async fn thermal_edits_share_native_lua_validation_and_preserve_stored_heat_scenario(
    f: &UnitFields,
) {
    let config = &f.config;
    for (index, source) in firing::templates().into_iter().enumerate() {
        let (mut world, id, _, _) = f.pair(&source, &source);
        if index < 2 {
            firing::edit(&mut world, id, |unit| unit["heat"]["stored"] = 40.0.into());
        }
        let native = &f.native;
        let lua = &f.lua;
        f.install(&world);
        if index >= 2 {
            let before = native.world().btech.clone();
            for field in ["heat", "dissheat", "overheat", "disabled_hs"] {
                assert!(
                    set_battle_unit_field_action(native, config, ObjectId(1), id, field, "1")
                        .is_err()
                );
                assert_eq!(native.world().btech, before);
            }
            continue;
        }
        for (name, value) in [("heat", "12.5"), ("dissheat", "-2.5"), ("overheat", "17")] {
            let output = support::run_text(
                native,
                config,
                ObjectId(1),
                1,
                &format!("@setmech {name} {value}"),
            );
            assert!(output.is_empty(), "{output}");
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'{}','{}')",
                id.0, name, value
            ))
            .unwrap();
        }
        assert_eq!(native.world().btech, lua.world().btech);
        {
            let world = native.world();
            let unit = &world.btech.constructed_units()[&id];
            assert_eq!(unit.heat().stored, 40.0);
            assert_eq!(unit.heat().excess, 17.0);
            assert_eq!(unit.heat().to_hit_modifier(), 3);
            assert_eq!(unit.sampled_heat_rates().production, 12.5);
            assert_eq!(unit.sampled_heat_rates().dissipation, -2.5);
            assert_eq!(
                unit.heat_rates(&world).dissipation,
                f64::from(unit.cooling_capacity())
            );
        }
        let before = native.world().btech.clone();
        for (name, value) in [
            ("heat", "NaN"),
            ("dissheat", "inf"),
            ("heat", "1e100"),
            ("overheat", "-1"),
            ("disabled_hs", "65535"),
        ] {
            assert!(
                set_battle_unit_field_action(native, config, ObjectId(1), id, name, value).is_err()
            );
            assert_eq!(native.world().btech, before);
        }
        assert!(
            native
                .eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'overheat','24'); error('abort')",
                    id.0
                ))
                .is_err()
        );
        assert_eq!(native.world().btech, before);
        let saved = native.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, before);
        advance_battle_heat(&mut restored);
        let unit = &restored.btech.constructed_units()[&id];
        assert_eq!(unit.sampled_heat_rates().production, 40.0);
        assert_ne!(unit.sampled_heat_rates().dissipation, -2.5);
        assert!(unit.heat().stored < 40.0);
    }
}

/// Hex-center navigation fields share live services with Lua and survive restart unchanged.
async fn navigation_fields_share_live_services_without_advancing_them_scenario(f: &UnitFields) {
    let config = &f.config;
    for source in firing::templates() {
        let (mut world, id, _, _) = f.pair(&source, &source);
        let center = HexCoordinate { x: 0, y: 11 }.center();
        firing::edit(&mut world, id, |unit| {
            unit["motion"]["point"] =
                serde_json::to_value(center.project(270.0, 0.2).unwrap()).unwrap();
        });
        let navigation = find_battle_hex_center(&world, id, ObjectId(1)).unwrap();
        let scripts = &f.native;
        support::install(scripts, world.clone());
        let before = scripts.world().btech.clone();
        let report = view_battle_unit_fields_action(scripts, config, ObjectId(1), id, "").unwrap();
        assert_eq!(
            field(&report, "centdist"),
            Some(format!("{:.2}", navigation.range).as_str())
        );
        assert_eq!(
            field(&report, "centbearing"),
            Some(navigation.bearing.to_string().as_str())
        );
        assert_eq!(field(&report, "centdist"), Some("0.20"));
        assert_eq!(field(&report, "centbearing"), Some("90"));
        let lua: mlua::Table = scripts
            .eval_callback(&format!("return btech.unit.fields(1,{})", id.0))
            .unwrap();
        assert_eq!(
            serde_json::to_value(lua).unwrap(),
            serde_json::to_value(report).unwrap()
        );
        assert_eq!(scripts.world().btech, before);
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, before);
        let scripts = Scripts::new(config, Rc::new(RefCell::new(restored))).unwrap();
        let report =
            view_battle_unit_fields_action(&scripts, config, ObjectId(1), id, "centdist").unwrap();
        assert_eq!(field(&report, "centdist"), Some("0.20"));
    }
}

async fn authored_and_edited_metadata_share_validation_across_chassis_and_restart_scenario(
    f: &UnitFields,
) {
    let config = &f.config;
    for source in firing::templates() {
        let source = format!("unit_era = \"Clan Invasion\"\nunit_tro = \"TRO 3050\"\n{source}");
        let (world, id, _, _) = f.pair(&source, &source);
        let native = &f.native;
        let lua = &f.lua;
        f.install(&world);
        let original =
            view_battle_unit_fields_action(native, config, ObjectId(1), id, "unit_").unwrap();
        assert_eq!(field(&original, "unit_era"), Some("Clan Invasion"));
        assert_eq!(field(&original, "unit_tro"), Some("TRO 3050"));
        native.drain_outbox();
        for (name, value) in [("unit_era", "Civil War"), ("unit_tro", "Custom %cr TRO")] {
            let output = support::run_text(
                native,
                config,
                ObjectId(1),
                1,
                &format!("@setmech {name} {value}"),
            );
            assert!(output.is_empty(), "{output}");
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'{}','{}')",
                id.0, name, value
            ))
            .unwrap();
        }
        assert_eq!(native.world().btech, lua.world().btech);
        let report =
            view_battle_unit_fields_action(native, config, ObjectId(1), id, "unit_").unwrap();
        assert_eq!(field(&report, "unit_era"), Some("Civil War"));
        assert_eq!(field(&report, "unit_tro"), Some("Custom %cr TRO"));
        let before = native.world().btech.clone();
        for value in ["x".repeat(25), "é".repeat(13)] {
            assert!(
                set_battle_unit_field_action(native, config, ObjectId(1), id, "unit_era", &value)
                    .is_err()
            );
            assert_eq!(native.world().btech, before);
        }
        assert!(
            native
                .eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'unit_tro','temporary'); error('abort')",
                    id.0
                ))
                .is_err()
        );
        assert_eq!(native.world().btech, before);
        set_battle_unit_field_action(native, config, ObjectId(1), id, "unit_tro", "").unwrap();
        let saved = native.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, saved.btech);
        let scripts = Scripts::new(config, Rc::new(RefCell::new(restored))).unwrap();
        let report =
            view_battle_unit_fields_action(&scripts, config, ObjectId(1), id, "unit_").unwrap();
        assert_eq!(field(&report, "unit_tro"), Some(""));
    }
}

async fn enemy_contact_count_follows_acquisition_and_teams_without_rescanning_scenario(
    f: &UnitFields,
) {
    let config = &f.config;
    let sources = firing::templates();
    for (index, source) in sources.iter().enumerate() {
        let (world, id, target, _) = f.pair(source, &sources[(index + 1) % sources.len()]);
        let scripts = &f.native;
        support::install(scripts, world.clone());
        for (edited, team, expected) in [
            (target, "0", "0"),
            (target, "1", "1"),
            (id, "1", "0"),
            (target, "2", "1"),
        ] {
            set_battle_unit_field_action(scripts, config, ObjectId(1), edited, "team", team)
                .unwrap();
            let before = scripts.world().btech.clone();
            let report =
                view_battle_unit_fields_action(scripts, config, ObjectId(1), id, "numseen")
                    .unwrap();
            assert_eq!(field(&report, "numseen"), Some(expected));
            let lua: mlua::Table = scripts
                .eval_callback(&format!("return btech.unit.fields(1,{},'numseen')", id.0))
                .unwrap();
            assert_eq!(
                serde_json::to_value(lua).unwrap(),
                serde_json::to_value(report).unwrap()
            );
            assert_eq!(scripts.world().btech, before);
        }
        firing::edit(&mut scripts.world_mut(), id, |unit| {
            unit["contacts"][target.0.to_string()]["identified"] = false.into()
        });
        let report =
            view_battle_unit_fields_action(scripts, config, ObjectId(1), id, "numseen").unwrap();
        assert_eq!(field(&report, "numseen"), Some("1"));
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, saved.btech);
        let scripts = Scripts::new(config, Rc::new(RefCell::new(loaded))).unwrap();
        firing::edit(&mut scripts.world_mut(), id, |unit| {
            unit["contacts"] = serde_json::json!({});
            unit["target_lock"] = serde_json::Value::Null;
        });
        let report =
            view_battle_unit_fields_action(&scripts, config, ObjectId(1), id, "numseen").unwrap();
        assert_eq!(field(&report, "numseen"), Some("0"));
    }
}

async fn explicit_cockpit_links_share_named_edits_and_preserve_deferred_references_scenario(
    f: &UnitFields,
) {
    let config = &f.config;
    for source in firing::templates() {
        let (world, id, target, _) = f.pair(&source, &source);
        let native = &f.native;
        let lua = &f.lua;
        f.install(&world);
        for (name, value) in [
            ("turret0", target.0.to_string()),
            ("turret1", id.0.to_string()),
            ("turret2", "-99".into()),
        ] {
            let output = support::run_text(
                native,
                config,
                ObjectId(1),
                1,
                &format!("@setmech {name} {value}"),
            );
            assert!(output.is_empty(), "{output}");
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'{}','{}')",
                id.0, name, value
            ))
            .unwrap();
        }
        assert_eq!(native.world().btech, lua.world().btech);
        let report =
            view_battle_unit_fields_action(native, config, ObjectId(1), id, "turret").unwrap();
        assert_eq!(
            field(&report, "turret0"),
            Some(target.0.to_string().as_str())
        );
        assert_eq!(field(&report, "turret1"), Some(id.0.to_string().as_str()));
        assert_eq!(field(&report, "turret2"), Some("-99"));
        let before = native.world().btech.clone();
        assert!(
            set_battle_unit_field_action(
                native,
                config,
                ObjectId(1),
                id,
                "turret0",
                "not-a-reference"
            )
            .is_err()
        );
        assert_eq!(native.world().btech, before);
        assert!(
            native
                .eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'turret0','-1'); error('abort')",
                    id.0
                ))
                .is_err()
        );
        assert_eq!(native.world().btech, before);
        let saved = native.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, before);
    }
}

/// Presentation overrides preserve template identity, share contact naming, and survive restart.
async fn display_name_edits_share_chassis_reports_and_restore_template_fallback_scenario(
    f: &UnitFields,
) {
    let config = &f.config;
    for source in firing::templates() {
        let (world, id, target, _) = f.pair(&source, &source);
        let original_status = battle_unit_status(&world, id, "").unwrap();
        let before = serde_json::to_value(&world.btech).unwrap();
        let native = &f.native;
        let lua = &f.lua;
        f.install(&world);
        assert_eq!(battle_display_name(&native.world(), id).unwrap(), "");
        assert!(
            support::run_text(
                native,
                config,
                ObjectId(1),
                1,
                "@setmech displayname Silver Fox"
            )
            .is_empty()
        );
        let success: bool = lua
            .eval_callback(&format!(
                "return btech.unit.set_display_name_as(1, {}, 'Silver Fox')",
                id.0
            ))
            .unwrap();
        assert!(success);
        assert_eq!(native.world().btech, lua.world().btech);
        assert!(
            battle_unit_status(&native.world(), id, "")
                .unwrap()
                .contains("Silver Fox")
        );
        let reported: String = lua
            .eval_callback(&format!("return btech.unit.display_name({})", id.0))
            .unwrap();
        assert_eq!(reported, "Silver Fox");
        let report = view_battle_unit_fields_action(native, config, ObjectId(1), id, "").unwrap();
        assert_eq!(field(&report, "displayname"), Some("Silver Fox"));
        let key = if native.world().btech.vehicles().contains_key(&id) {
            "vehicles"
        } else {
            "constructed"
        };
        let mut after = serde_json::to_value(&native.world().btech).unwrap();
        after[key][id.0.to_string()]["display_name"] = serde_json::json!("");
        after["unit_configuration"]
            .as_object_mut()
            .unwrap()
            .remove(&id.0.to_string());
        assert_eq!(after, before);
        set_battle_unit_field_action(
            native,
            config,
            ObjectId(1),
            target,
            "displayname",
            "Red Fox",
        )
        .unwrap();
        let contact = visible_battle_contacts(&native.world(), id)
            .unwrap()
            .into_iter()
            .find(|view| view.target == target)
            .unwrap();
        assert_eq!(contact.name, "Red Fox");
        let saved = native.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, saved.btech);
        assert!(
            battle_unit_status(&restored, id, "")
                .unwrap()
                .contains("Silver Fox")
        );
        for name in ["x".repeat(121), "é".repeat(61)] {
            assert!(
                set_battle_unit_field_action(native, config, ObjectId(1), id, "displayname", &name)
                    .is_err()
            );
            let mut corrupt = serde_json::to_value(&saved.btech).unwrap();
            corrupt[key][id.0.to_string()]["display_name"] = serde_json::json!(name);
            assert!(serde_json::from_value::<BtechState>(corrupt).is_err());
        }
        assert_eq!(native.world().btech, saved.btech);
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_display_name_as(1, {}, 'Discard'); error('abort')",
                id.0
            ))
            .is_err()
        );
        assert_eq!(battle_display_name(&lua.world(), id).unwrap(), "Silver Fox");
        lua.eval_callback::<()>(&format!("btech.unit.set_display_name_as(1, {}, '')", id.0))
            .unwrap();
        assert_eq!(
            battle_unit_status(&lua.world(), id, "").unwrap(),
            original_status
        );
    }
}

/// Startup history changes only at completion, across chassis, interruption and database reload.
async fn startup_history_uses_supplied_completion_time_and_preserves_aborted_history_scenario(
    f: &UnitFields,
) {
    let config = &f.config;
    for source in firing::templates() {
        let (world, id, _, _) = f.pair(&source, &source);
        let native = &f.native;
        let lua = &f.lua;
        f.install(&world);
        assert!(
            support::run_text(native, config, ObjectId(1), 1, "@setmech last_startup -50")
                .is_empty()
        );
        lua.eval_callback::<()>(&format!(
            "btech.unit.set_field(1, {}, 'last_startup', '-50')",
            id.0
        ))
        .unwrap();
        assert_eq!(native.world().btech, lua.world().btech);
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1, {}, 'last_startup', '12'); error('abort')",
                id.0
            ))
            .is_err()
        );
        assert_eq!(native.world().btech, lua.world().btech);
        assert!(
            set_battle_unit_field_action(
                native,
                config,
                ObjectId(1),
                id,
                "last_startup",
                "9223372036854775808"
            )
            .is_err()
        );
        assert_eq!(native.world().btech, lua.world().btech);
        let mut world = native.world().clone();
        let key = if world.btech.vehicles().contains_key(&id) {
            "vehicles"
        } else {
            "constructed"
        };
        let history = |world: &World| {
            serde_json::to_value(&world.btech).unwrap()[key][id.0.to_string()]["last_startup"]
                .as_i64()
                .unwrap()
        };
        stop_battle_unit(&mut world, id, ObjectId(1), MovementRules::STANDARD.fall).unwrap();
        assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
        for now in [100, 101] {
            advance_battle_units(&mut world, now);
        }
        assert_eq!(history(&world), -50);
        stop_battle_unit(&mut world, id, ObjectId(1), MovementRules::STANDARD.fall).unwrap();
        advance_battle_units(&mut world, 102);
        assert_eq!(history(&world), -50);
        for (fast, duration, start) in [(true, 5, 1_800_000_000_i64), (false, 30, 1_900_000_000)] {
            assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
            support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
            start_battle_unit(&mut world, id, ObjectId(1), fast).unwrap();
            let previous = history(&world);
            for second in 1..=duration {
                advance_battle_units(&mut world, start + second);
                assert_eq!(
                    history(&world),
                    if second == duration {
                        start + duration
                    } else {
                        previous
                    }
                );
                if second == 2 || second == duration {
                    persistence::save(&config.database(), &world).await.unwrap();
                    let loaded = persistence::load(&config.database()).await.unwrap();
                    assert_eq!(loaded.btech, world.btech);
                    world = loaded;
                }
            }
            advance_battle_units(&mut world, start + 100);
            assert_eq!(history(&world), start + duration);
            stop_battle_unit(&mut world, id, ObjectId(1), MovementRules::STANDARD.fall).unwrap();
        }
        let scripts = &f.native;
        support::install(scripts, world.clone());
        let report =
            view_battle_unit_fields_action(scripts, config, ObjectId(1), id, "last_startup")
                .unwrap();
        assert_eq!(field(&report, "last_startup"), Some("1900000030"));
    }
}

/// Administrative bitvectors and cockpit commands mutate one set of gameplay preferences.
async fn preference_fields_share_cockpit_state_validation_and_restart_scenario(f: &UnitFields) {
    let config = &f.config;
    for source in firing::templates() {
        let (world, id, _, _) = f.pair(&source, &source);
        let named = Scripts::new(config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let cockpit = Scripts::new(config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(config, Rc::new(RefCell::new(world))).unwrap();
        let report =
            view_battle_unit_fields_action(&named, config, ObjectId(1), id, "MechPrefs").unwrap();
        assert_eq!(field(&report, "MechPrefs"), Some("-"));
        named.drain_outbox();
        assert!(
            support::run_text(&named, config, ObjectId(1), 1, "@setmech MechPrefs bcdegh")
                .is_empty()
        );
        lua.eval_callback::<()>(&format!(
            "btech.unit.set_field(1,{},'MechPrefs','222')",
            id.0
        ))
        .unwrap();
        for command in [
            "SLWarn ON",
            "AutoFall ON",
            "ArmorWarn OFF",
            "AmmoWarn OFF",
            "AutoconShutdown ON",
            "FFSafety ON",
        ] {
            let text = support::run_text(
                &cockpit,
                config,
                ObjectId(1),
                1,
                &format!("mechprefs {command}"),
            );
            assert!(
                text.trim_end_matches('.')
                    .ends_with(command.split_whitespace().last().unwrap()),
                "{text}"
            );
        }
        assert_eq!(named.world().btech, cockpit.world().btech);
        assert_eq!(named.world().btech, lua.world().btech);
        let report =
            view_battle_unit_fields_action(&named, config, ObjectId(1), id, "mechprefs").unwrap();
        assert_eq!(field(&report, "MechPrefs"), Some("bcdegh"));
        let before = named.world().btech.clone();
        for value in [
            "i",
            "256",
            "bcdeghi",
            "k",
            "-1",
            "G",
            "2147483648",
            "bcdegh!",
        ] {
            assert!(
                set_battle_unit_field_action(&named, config, ObjectId(1), id, "MechPrefs", value)
                    .is_err(),
                "{value}"
            );
            assert_eq!(named.world().btech, before);
        }
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'MechPrefs','0'); error('abort')",
                id.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, before);
        set_battle_unit_field_action(&named, config, ObjectId(1), id, "MechPrefs", "bcdegh!b!g")
            .unwrap();
        let report =
            view_battle_unit_fields_action(&named, config, ObjectId(1), id, "MechPrefs").unwrap();
        assert_eq!(field(&report, "MechPrefs"), Some("cdeh"));
        let saved = named.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, saved.btech);
        set_battle_unit_field_action(&named, config, ObjectId(1), id, "MechPrefs", "0").unwrap();
        let report =
            view_battle_unit_fields_action(&named, config, ObjectId(1), id, "MechPrefs").unwrap();
        assert_eq!(field(&report, "MechPrefs"), Some("-"));
    }
}

/// Named BV follows saved damage and current runtime weapon values, which reset on reload.
async fn battle_value_field_tracks_live_damage_and_weapon_configuration_scenario(f: &UnitFields) {
    let config = &f.config;
    for source in firing::templates() {
        let (mut world, id, _, _) = f.supply(&source, Some(Weapon::MediumLaser), &source, None);
        let baseline = battle_unit_value(&world, id, config.battletech.tsm_tow_bonus != 0)
            .unwrap()
            .total;
        firing::edit(&mut world, id, |unit| {
            let section = if unit["sections"].get("CenterTorso").is_some() {
                "CenterTorso"
            } else {
                "front"
            };
            unit["sections"][section]["armor"] = 0.into();
        });
        let damaged = battle_unit_value(&world, id, config.battletech.tsm_tow_bonus != 0)
            .unwrap()
            .total;
        assert!(damaged < baseline);
        set_battle_weapon_battle_value(&mut world, ObjectId(1), "IS.MediumLaser", 1000).unwrap();
        let increased = battle_unit_value(&world, id, config.battletech.tsm_tow_bonus != 0)
            .unwrap()
            .total;
        assert!(increased > damaged);
        let expected = format!("{increased:.2}");
        let before = world.btech.clone();
        let scripts = &f.native;
        support::install(scripts, world.clone());
        let report =
            view_battle_unit_fields_action(scripts, config, ObjectId(1), id, "BV").unwrap();
        assert_eq!(field(&report, "bv"), Some(expected.as_str()));
        let lua: mlua::Table = scripts
            .eval_callback(&format!("return btech.unit.fields(1,{},'bv')", id.0))
            .unwrap();
        assert_eq!(
            serde_json::to_value(lua).unwrap(),
            serde_json::to_value(&report).unwrap()
        );
        assert!(
            support::run_text(scripts, config, ObjectId(1), 1, "@viewmech bv").contains(&expected)
        );
        assert!(
            set_battle_unit_field_action(scripts, config, ObjectId(1), id, "bv", "123").is_err()
        );
        assert_eq!(scripts.world().btech, before);
        scripts.drain_outbox();
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.fields(1,{},'bv'); error('abort')",
                    id.0
                ))
                .is_err()
        );
        assert!(scripts.drain_outbox().is_empty());
        let snapshot = scripts.world().clone();
        persistence::save(&config.database(), &snapshot)
            .await
            .unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        let scripts = Scripts::new(config, Rc::new(RefCell::new(restored))).unwrap();
        let report =
            view_battle_unit_fields_action(&scripts, config, ObjectId(1), id, "bv").unwrap();
        assert_eq!(field(&report, "bv"), Some(format!("{damaged:.2}").as_str()));
        set_battle_weapon_battle_value(
            &mut scripts.world_mut(),
            ObjectId(1),
            "IS.MediumLaser",
            1000,
        )
        .unwrap();
        assert_eq!(scripts.world().btech, before);
    }
}

/// Administrative movement and engine allocation fields round-trip without changing live physics.
async fn construction_fields_share_storage_authority_and_restart_without_changing_physics_scenario(
    f: &UnitFields,
) {
    let config = &f.config;
    for source in firing::templates() {
        let (world, id, _, _) = f.pair(&source, &source);
        let native = &f.native;
        let lua = &f.lua;
        f.install(&world);
        let original = view_battle_unit_fields_action(native, config, ObjectId(1), id, "").unwrap();
        for name in ["basewalkspeed", "baserunspeed", "hsengoverride"] {
            assert_eq!(field(&original, name), Some("0"));
        }
        for (name, value) in [
            ("basewalkspeed", "-2147483648"),
            ("baserunspeed", "2147483647"),
            ("hsengoverride", "12"),
        ] {
            commands::run(
                native,
                config,
                ObjectId(1),
                1,
                &format!("@setmech {name} {value}"),
            )
            .unwrap();
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'{name}','{value}')",
                id.0
            ))
            .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            let report = view_battle_unit_fields_action(lua, config, ObjectId(1), id, "").unwrap();
            assert_eq!(field(&report, name), Some(value));
            for speed in ["speed", "maxspeed", "templatesp", "heatsinks"] {
                assert_eq!(field(&report, speed), field(&original, speed));
            }
            let before = lua.world().btech.clone();
            lua.drain_outbox();
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'{name}','7'); error('abort')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            for invalid in ["2147483648", "-2147483649", "3.5", "", "walk"] {
                assert!(
                    set_battle_unit_field_action(lua, config, ObjectId(1), id, name, invalid)
                        .is_err()
                );
                assert_eq!(lua.world().btech, before);
            }
            assert!(
                set_battle_unit_field_action(lua, config, ObjectId(99999), id, name, "7").is_err()
            );
            assert_eq!(lua.world().btech, before);
        }
        let snapshot = lua.world().clone();
        persistence::save(&config.database(), &snapshot)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            snapshot.btech
        );
    }
}

/// Authored overrides survive construction, while invalid template values fail before unit insertion.
async fn engine_sink_override_validates_authored_values_across_chassis_scenario(f: &UnitFields) {
    let config = &f.config;
    for source in firing::templates() {
        let authored = format!("hs_engine_override = 14\n{source}");
        let (mut world, id, _, _) = f.pair(&authored, &source);
        let candidate = world.create(config, "Invalid construction".into(), Kind::Thing);
        let before = world.btech.clone();
        // Typed TOML values reject non-integers and values outside a signed 32-bit range.
        for value in ["\"no\"", "2147483648", "-2147483649", "1.5"] {
            let template =
                UnitTemplate::parse("test", &format!("hs_engine_override = {value}\n{source}"));
            let result = template.and_then(|template| template.create(&mut world, candidate));
            let error = format!("{:#}", result.unwrap_err());
            assert!(error.contains("hs_engine_override"), "{error}");
            assert_eq!(world.btech, before);
        }
        let scripts = &f.native;
        support::install(scripts, world.clone());
        let report =
            view_battle_unit_fields_action(scripts, config, ObjectId(1), id, "hsengoverride")
                .unwrap();
        assert_eq!(field(&report, "hsengoverride"), Some("14"));
    }
}

/// Inactive read-only counters do not alias active damage history or physical weapon arcs.
async fn inactive_readonly_fields_stay_zero_with_damage_and_across_restart_scenario(
    f: &UnitFields,
) {
    let config = &f.config;
    for source in firing::templates() {
        let (mut world, id, _, _) = f.pair(&source, &source);
        if world.btech.constructed_units().contains_key(&id) {
            firing::edit(&mut world, id, |unit| {
                unit["stagger"]["turn_damage"] = 35.into();
                unit["stagger"]["hits"] = serde_json::json!([
                    {"damage":25,"remaining":30,"counted":true},
                    {"damage":15,"remaining":60,"counted":false}
                ]);
            });
        }
        let before = world.btech.clone();
        let scripts = &f.native;
        support::install(scripts, world.clone());
        for name in ["StaggerDamage", "unusablearcs"] {
            let report =
                view_battle_unit_fields_action(scripts, config, ObjectId(1), id, name).unwrap();
            assert_eq!(field(&report, name), Some("0"));
            let lua: mlua::Table = scripts
                .eval_callback(&format!("return btech.unit.fields(1,{},'{name}')", id.0))
                .unwrap();
            assert_eq!(
                serde_json::to_value(lua).unwrap(),
                serde_json::to_value(report).unwrap()
            );
            for value in ["0", "20", "-10"] {
                assert!(
                    set_battle_unit_field_action(scripts, config, ObjectId(1), id, name, value)
                        .is_err()
                );
                assert!(
                    scripts
                        .eval_callback::<()>(&format!(
                            "btech.unit.set_field(1,{},'{name}','{value}')",
                            id.0
                        ))
                        .is_err()
                );
            }
            scripts.drain_outbox();
            assert!(
                scripts
                    .eval_callback::<()>(&format!(
                        "btech.unit.fields(1,{},'{name}'); error('abort')",
                        id.0
                    ))
                    .is_err()
            );
            assert!(scripts.drain_outbox().is_empty());
            assert_eq!(scripts.world().btech, before);
        }
        let snapshot = scripts.world().clone();
        persistence::save(&config.database(), &snapshot)
            .await
            .unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, before);
        let scripts = Scripts::new(config, Rc::new(RefCell::new(restored))).unwrap();
        for name in ["StaggerDamage", "unusablearcs"] {
            let report =
                view_battle_unit_fields_action(&scripts, config, ObjectId(1), id, name).unwrap();
            assert_eq!(field(&report, name), Some("0"));
        }
    }
}

/// Administrative crew and target edits share transactions without requiring acquired contacts.
async fn crew_and_target_fields_share_native_lua_validation_and_restart_scenario(f: &UnitFields) {
    let config = &f.config;
    for source in firing::templates() {
        let (mut world, id, target, _) = f.pair(&source, &source);
        let replacement = world.create(config, "Relief pilot".into(), Kind::Player);
        world.objects.get_mut(&replacement).unwrap().location = Some(id);
        world.objects.get_mut(&replacement).unwrap().home = Some(ObjectId(config.home()));
        // Both interface runs start with the same private recovery random stream.
        prepare_battle_recovery(&mut world, replacement).unwrap();
        support::seed_object_dice(&mut world, replacement, support::FIXTURE_DICE_SEED);
        let unplaced = world.create(config, "Unplaced unit".into(), Kind::Thing);
        world.objects.get_mut(&unplaced).unwrap().home = Some(ObjectId(config.home()));
        UnitTemplate::parse("test", &source)
            .unwrap()
            .create(&mut world, unplaced)
            .unwrap();
        support::seed_object_dice(&mut world, unplaced, support::FIXTURE_DICE_SEED);
        firing::edit(&mut world, id, |unit| {
            unit["contacts"] = serde_json::json!({})
        });
        let native = &f.native;
        let lua = &f.lua;
        f.install(&world);
        for (name, value) in [
            ("pilotnum", replacement.0),
            ("pilotnum", -1),
            ("pilotnum", 1),
            ("target", -1),
            ("target", target.0),
        ] {
            let before = lua.world().btech.clone();
            lua.drain_outbox();
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'{name}','{value}'); error('abort')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            commands::run(
                native,
                config,
                ObjectId(1),
                1,
                &format!("@setmech {name} {value}"),
            )
            .unwrap();
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'{name}','{value}')",
                id.0
            ))
            .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            let report =
                view_battle_unit_fields_action(lua, config, ObjectId(1), id, name).unwrap();
            assert_eq!(field(&report, name), Some(value.to_string().as_str()));
        }
        let selected = lua
            .world()
            .btech
            .constructed_units()
            .get(&id)
            .and_then(Mech::target_selection)
            .or_else(|| {
                lua.world()
                    .btech
                    .vehicles()
                    .get(&id)
                    .and_then(Vehicle::target_selection)
            })
            .unwrap();
        assert_eq!(selected.remaining(), 8);
        let before = lua.world().btech.clone();
        for (name, values) in [
            (
                "pilotnum",
                vec![
                    "-2".into(),
                    "999999".into(),
                    target.0.to_string(),
                    "bad".into(),
                ],
            ),
            (
                "target",
                vec![
                    "-2".into(),
                    "999999".into(),
                    id.0.to_string(),
                    replacement.0.to_string(),
                    unplaced.0.to_string(),
                ],
            ),
        ] {
            for value in values {
                assert!(
                    set_battle_unit_field_action(lua, config, ObjectId(1), id, name, &value)
                        .is_err()
                );
                assert_eq!(lua.world().btech, before);
            }
            assert!(
                set_battle_unit_field_action(lua, config, replacement, id, name, "-1").is_err()
            );
            assert_eq!(lua.world().btech, before);
        }
        let snapshot = lua.world().clone();
        persistence::save(&config.database(), &snapshot)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            before
        );
    }
}

/// Catalog read-only fields reject even otherwise-valid values through every administrative interface.
async fn readonly_unit_fields_reject_native_and_lua_writes_on_every_chassis_scenario(
    f: &UnitFields,
) {
    let config = &f.config;
    for source in firing::templates() {
        let (mut world, id, _, _) = f.pair(&source, &source);
        if world.btech.constructed_units().contains_key(&id) {
            firing::edit(&mut world, id, |unit| {
                unit["heat_cutoff"]["disabled"] = 3.into()
            });
        }
        let before = world.btech.clone();
        let scripts = &f.native;
        support::install(scripts, world.clone());
        for name in [
            "mapindex",
            "towing",
            "disabled_hs",
            "heatsinks",
            "C3iNetworkSize",
            "StaggerDamage",
            "cocoon",
            "unusablearcs",
            "id",
            "centdist",
            "centbearing",
            "bv",
            "numseen",
        ] {
            let original =
                view_battle_unit_fields_action(scripts, config, ObjectId(1), id, name).unwrap();
            scripts.drain_outbox();
            for value in ["0", "-1", "3"] {
                let error =
                    set_battle_unit_field_action(scripts, config, ObjectId(1), id, name, value)
                        .unwrap_err();
                assert!(error.to_string().contains("read-only"));
                let error = scripts
                    .eval_callback::<()>(&format!(
                        "btech.unit.set_field(1,{},'{}','{value}')",
                        id.0,
                        name.to_ascii_uppercase()
                    ))
                    .unwrap_err();
                assert!(error.to_string().contains("read-only"));
                assert!(scripts.drain_outbox().is_empty());
                let output = support::run_text(
                    scripts,
                    config,
                    ObjectId(1),
                    1,
                    &format!("@setmech {} {value}", name.to_ascii_uppercase()),
                );
                assert!(output.contains("read-only"), "{output}");
                assert_eq!(scripts.world().btech, before);
            }
            let report =
                view_battle_unit_fields_action(scripts, config, ObjectId(1), id, name).unwrap();
            assert_eq!(report, original);
            scripts.drain_outbox();
        }
    }
}

/// Deferred administrative transitions remain inspectable and reject edits without side effects.
async fn deferred_field_writes_reject_native_and_lua_edits_across_chassis_scenario(f: &UnitFields) {
    let config = &f.config;
    for source in firing::templates() {
        let (world, id, _, _) = f.pair(&source, &source);
        let scripts = &f.native;
        support::install(scripts, world.clone());
        let before = scripts.world().btech.clone();
        for (name, value) in [("status", "0"), ("critstatus", "a"), ("mechtype", "Mech")] {
            let report =
                view_battle_unit_fields_action(scripts, config, ObjectId(1), id, name).unwrap();
            assert!(field(&report, name).is_some());
            scripts.drain_outbox();
            let error = set_battle_unit_field_action(scripts, config, ObjectId(1), id, name, value)
                .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("direct writes are not supported")
            );
            let error = scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'{}','{value}')",
                    id.0,
                    name.to_ascii_uppercase()
                ))
                .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("direct writes are not supported")
            );
            assert!(scripts.drain_outbox().is_empty());
            let output = support::run_text(
                scripts,
                config,
                ObjectId(1),
                1,
                &format!("@setmech {} {value}", name.to_ascii_uppercase()),
            );
            assert!(
                output.contains("direct writes are not supported"),
                "{output}"
            );
            assert_eq!(scripts.world().btech, before);
            let after =
                view_battle_unit_fields_action(scripts, config, ObjectId(1), id, name).unwrap();
            assert_eq!(after, report);
            scripts.drain_outbox();
        }
    }
}

/// Cargo edits retain stock while updating shared mass, movement limits and saved construction.
async fn cargo_field_edits_share_load_rules_and_atomic_publication_scenario(f: &UnitFields) {
    let config = &f.config;
    for source in firing::templates() {
        let (mut world, id, _, _) = f.pair(&source, &source);
        set_battle_inventory_named(&mut world, ObjectId(1), id, "Gold", 2).unwrap();
        let moving = world.btech.constructed_units().contains_key(&id)
            || world
                .btech
                .vehicles()
                .get(&id)
                .is_some_and(|unit| !unit.definition().is_vtol() && unit.maximum_speed() > 0.0);
        if moving {
            firing::edit(&mut world, id, |unit| {
                unit["motion"]["speed"] = 1.0.into();
                unit["motion"]["desired_speed"] = 1.0.into();
            });
        }
        let original_load = battle_unit_load(&world, id, true).unwrap();
        let (original_cargo, carrier, cargo) =
            if let Some(unit) = world.btech.constructed_units().get(&id) {
                (
                    unit.mass().unwrap().cargo,
                    unit.definition()
                        .attributes
                        .get("specials")
                        .is_some_and(|flags| {
                            flags
                                .split_whitespace()
                                .any(|flag| flag.eq_ignore_ascii_case("Carrier_Tech"))
                        }),
                    unit.definition()
                        .attributes
                        .get("specials")
                        .is_some_and(|flags| {
                            flags
                                .split_whitespace()
                                .any(|flag| flag.eq_ignore_ascii_case("CargoTech"))
                        }),
                )
            } else {
                let unit = &world.btech.vehicles()[&id];
                (
                    unit.mass().unwrap().cargo,
                    unit.definition()
                        .attributes
                        .get("specials")
                        .is_some_and(|flags| {
                            flags
                                .split_whitespace()
                                .any(|flag| flag.eq_ignore_ascii_case("Carrier_Tech"))
                        }),
                    unit.definition()
                        .attributes
                        .get("specials")
                        .is_some_and(|flags| {
                            flags
                                .split_whitespace()
                                .any(|flag| flag.eq_ignore_ascii_case("CargoTech"))
                        }),
                )
            };
        let divisor = if carrier {
            1000.0
        } else if cargo {
            100.0
        } else {
            500.0
        };
        let stock = battle_inventory(&world, id).unwrap().to_vec();
        let native = &f.native;
        let lua = &f.lua;
        f.install(&world);
        for value in [123, 100_000, 0] {
            let before = lua.world().btech.clone();
            lua.drain_outbox();
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'CARGOSPACE','{value}'); error('abort')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            commands::run(
                native,
                config,
                ObjectId(1),
                1,
                &format!("@setmech CARGOSPACE {value}"),
            )
            .unwrap();
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'cargospace','{value}')",
                id.0
            ))
            .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            let report =
                view_battle_unit_fields_action(lua, config, ObjectId(1), id, "cargospace").unwrap();
            assert_eq!(
                field(&report, "cargospace"),
                Some(value.to_string().as_str())
            );
            assert_eq!(battle_inventory(&lua.world(), id).unwrap(), stock);
            let load = battle_unit_load(&lua.world(), id, true).unwrap();
            assert_eq!(load.carried_mass, original_load.carried_mass);
            assert_eq!(
                load.material_mass,
                original_load.material_mass - original_cargo
                    + (value as f32 / divisor * 1024.0).trunc() as u32
            );
            if value == 100_000 {
                assert_eq!(load.maximum_speed(100.0).unwrap(), 0.0);
                let world = lua.world();
                let motion = world
                    .btech
                    .constructed_units()
                    .get(&id)
                    .and_then(Mech::motion)
                    .or_else(|| world.btech.vehicles().get(&id).and_then(Vehicle::motion))
                    .unwrap();
                assert_eq!(motion.speed, 0.0);
                assert_eq!(motion.desired_speed, 0.0);
            }
            let saved = lua.world().clone();
            persistence::save(&config.database(), &saved).await.unwrap();
            let loaded = persistence::load(&config.database()).await.unwrap();
            assert_eq!(loaded.btech, saved.btech);
        }
        let before = lua.world().btech.clone();
        for value in ["-1", "2147483648", "2147483647", "NaN", "1.5"] {
            assert!(
                set_battle_unit_field_action(lua, config, ObjectId(1), id, "cargospace", value)
                    .is_err()
            );
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'cargospace','{value}')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
        }
    }
}

/// Tank baseline edits preserve actual fuel and share load accounting, rollback and restart.
async fn original_fuel_field_preserves_inventory_and_updates_surplus_load_scenario(f: &UnitFields) {
    let config = &f.config;
    for source in firing::templates() {
        let (world, id, _, _) = f.pair(&source, &source);
        let is_vtol = world
            .btech
            .vehicles()
            .get(&id)
            .is_some_and(|unit| unit.definition().is_vtol());
        let native = &f.native;
        let lua = &f.lua;
        f.install(&world);
        if !is_vtol {
            let before = lua.world().btech.clone();
            assert!(
                set_battle_unit_field_action(lua, config, ObjectId(1), id, "fuel_orig", "123")
                    .is_err()
            );
            assert_eq!(lua.world().btech, before);
            continue;
        }
        let initial = battle_vtol_fuel_status(&lua.world(), id).unwrap();
        for capacity in [0, 123, 8000, i32::MAX] {
            let before = lua.world().btech.clone();
            lua.drain_outbox();
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'fuel_orig','{capacity}'); error('abort')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            commands::run(
                native,
                config,
                ObjectId(1),
                1,
                &format!("@setmech FUEL_ORIG {capacity}"),
            )
            .unwrap();
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'fuel_orig','{capacity}')",
                id.0
            ))
            .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            let fuel = battle_vtol_fuel_status(&lua.world(), id).unwrap();
            assert_eq!(fuel.remaining, initial.remaining);
            assert_eq!(fuel.original_capacity, capacity as u32);
            assert_eq!(
                fuel.capacity,
                u64::from(capacity as u32)
                    + (initial.auxiliary_tanks + initial.installed_tanks) * 2000
            );
            assert_eq!(
                fuel.excess_mass,
                (initial.remaining - i64::from(capacity)).max(0) as u64
            );
            let report =
                view_battle_unit_fields_action(lua, config, ObjectId(1), id, "fuel_orig").unwrap();
            assert_eq!(
                field(&report, "fuel_orig"),
                Some(capacity.to_string().as_str())
            );
            let saved = lua.world().clone();
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                saved.btech
            );
        }
        let before = lua.world().btech.clone();
        for value in ["-1", "2147483648", "NaN", "1.5"] {
            assert!(
                set_battle_unit_field_action(lua, config, ObjectId(1), id, "fuel_orig", value)
                    .is_err()
            );
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'fuel_orig','{value}')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
        }
        // The exhaustion sentinel must not be reset by installing a larger baseline tank.
        firing::edit(&mut lua.world_mut(), id, |unit| {
            unit["vtol_fuel"]["remaining"] = (-1).into()
        });
        set_battle_unit_field_action(lua, config, ObjectId(1), id, "fuel_orig", "4000").unwrap();
        let fuel = battle_vtol_fuel_status(&lua.world(), id).unwrap();
        assert_eq!((fuel.remaining, fuel.excess_mass), (-1, 0));
    }
}

/// Actual motion fields retain controls and position, with one transaction on every chassis.
async fn motion_fields_edit_actual_state_without_advancing_controls_scenario(f: &UnitFields) {
    let config = &f.config;
    for source in firing::templates() {
        let (world, id, _, _) = f.pair(&source, &source);
        let movable = world
            .btech
            .vehicles()
            .get(&id)
            .is_none_or(|unit| unit.maximum_speed() > 0.0);
        let initial_motion = world
            .btech
            .constructed_units()
            .get(&id)
            .and_then(Mech::motion)
            .or_else(|| world.btech.vehicles().get(&id).and_then(Vehicle::motion))
            .unwrap();
        let native = &f.native;
        let lua = &f.lua;
        f.install(&world);
        for (name, value) in [
            ("heading", "359"),
            ("speed", "1.25"),
            ("speed", "-1.25"),
            ("speed", "0"),
            ("heading", "0"),
        ] {
            let before = lua.world().btech.clone();
            lua.drain_outbox();
            if !movable && value != "0" {
                assert!(
                    set_battle_unit_field_action(lua, config, ObjectId(1), id, name, value)
                        .is_err()
                );
                assert_eq!(lua.world().btech, before);
                continue;
            }
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'{name}','{value}'); error('abort')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            commands::run(
                native,
                config,
                ObjectId(1),
                1,
                &format!("@setmech {} {value}", name.to_ascii_uppercase()),
            )
            .unwrap();
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'{name}','{value}')",
                id.0
            ))
            .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            let saved = {
                let world = lua.world();
                let motion = world
                    .btech
                    .constructed_units()
                    .get(&id)
                    .and_then(Mech::motion)
                    .or_else(|| world.btech.vehicles().get(&id).and_then(Vehicle::motion))
                    .unwrap();
                assert_eq!(motion.point, initial_motion.point);
                assert_eq!(motion.desired_speed, initial_motion.desired_speed);
                assert_eq!(motion.desired_heading, initial_motion.desired_heading);
                assert_eq!(
                    if name == "heading" {
                        motion.heading
                    } else {
                        motion.speed
                    },
                    value.parse::<f64>().unwrap()
                );
                world.clone()
            };
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                saved.btech
            );
        }
        let before = lua.world().btech.clone();
        for (name, value) in [
            ("heading", "-1"),
            ("heading", "360"),
            ("heading", "1.5"),
            ("heading", "32768"),
            ("speed", "NaN"),
            ("speed", "inf"),
            ("speed", "1e100"),
            ("speed", "100000"),
            ("speed", "-100000"),
        ] {
            assert!(
                set_battle_unit_field_action(lua, config, ObjectId(1), id, name, value).is_err()
            );
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'{name}','{value}')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
        }
    }
}

/// Integer coordinate edits compose the existing scenario service instead of bypassing placement.
async fn coordinate_fields_share_scenario_placement_and_rollback_scenario(f: &UnitFields) {
    let config = &f.config;
    for source in firing::templates() {
        let (world, id, _, _) = f.pair(&source, &source);
        let native = &f.native;
        let lua = &f.lua;
        f.install(&world);
        let scenario = Scripts::new(config, Rc::new(RefCell::new(world))).unwrap();
        for (name, value, x, y, z) in [
            ("y", 9, 0, 9, 0),
            ("z", 2, 0, 9, 2),
            ("x", 0, 0, 9, 2),
            ("y", 11, 0, 11, 2),
            ("z", 0, 0, 11, 0),
        ] {
            let before = lua.world().btech.clone();
            lua.drain_outbox();
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'{name}','{value}'); error('abort')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            set_battle_coordinates_action(
                &scenario,
                config,
                ObjectId(1),
                id,
                ScenarioPosition {
                    coordinate: HexCoordinate { x, y },
                    elevation: Some(z),
                },
            )
            .unwrap();
            commands::run(
                native,
                config,
                ObjectId(1),
                1,
                &format!("@setmech {} {value}", name.to_ascii_uppercase()),
            )
            .unwrap();
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'{name}','{value}')",
                id.0
            ))
            .unwrap();
            assert_eq!(native.world().btech, scenario.world().btech);
            assert_eq!(lua.world().btech, scenario.world().btech);
            let report =
                view_battle_unit_fields_action(lua, config, ObjectId(1), id, name).unwrap();
            assert_eq!(field(&report, name), Some(value.to_string().as_str()));
            let saved = lua.world().clone();
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                saved.btech
            );
        }
        let before = lua.world().btech.clone();
        for (name, value) in [
            ("x", "-1"),
            ("x", "1"),
            ("y", "12"),
            ("z", "32768"),
            ("z", "-32769"),
            ("x", "1.5"),
            ("z", "NaN"),
        ] {
            lua.drain_outbox();
            assert!(
                set_battle_unit_field_action(lua, config, ObjectId(1), id, name, value).is_err()
            );
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'{name}','{value}')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
        }
    }
}

/// Continuous field edits retain fractional geometry and the other axes across all chassis.
async fn precise_coordinate_fields_preserve_fractional_position_and_restart_scenario(
    f: &UnitFields,
) {
    let config = &f.config;
    for source in firing::templates() {
        let (world, id, _, _) = f.pair(&source, &source);
        let initial = world
            .btech
            .constructed_units()
            .get(&id)
            .and_then(Mech::motion)
            .or_else(|| world.btech.vehicles().get(&id).and_then(Vehicle::motion))
            .unwrap();
        let native = &f.native;
        let lua = &f.lua;
        f.install(&world);
        let mut expected = initial.point;
        let mut height = 0.0;
        for (name, scaled) in [
            ("fx", (initial.point.x + 0.05) * 322.5),
            ("fy", 9.25 * 322.5),
            ("fz", 2.25 * 64.5),
        ] {
            let value = scaled as f32;
            match name {
                "fx" => expected.x = f64::from(value) / 322.5,
                "fy" => expected.y = f64::from(value) / 322.5,
                _ => height = f64::from(value) / 64.5,
            }
            let before = lua.world().btech.clone();
            lua.drain_outbox();
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'{name}','{value}'); error('abort')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            commands::run(
                native,
                config,
                ObjectId(1),
                1,
                &format!("@setmech {} {value}", name.to_ascii_uppercase()),
            )
            .unwrap();
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'{name}','{value}')",
                id.0
            ))
            .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            let saved = lua.world().clone();
            let motion = saved
                .btech
                .constructed_units()
                .get(&id)
                .and_then(Mech::motion)
                .or_else(|| saved.btech.vehicles().get(&id).and_then(Vehicle::motion))
                .unwrap();
            assert_eq!(
                motion,
                Motion {
                    point: expected,
                    ..initial
                }
            );
            let report =
                view_battle_unit_fields_action(lua, config, ObjectId(1), id, "fz").unwrap();
            assert!(
                (field(&report, "fz").unwrap().parse::<f64>().unwrap() - height * 64.5).abs()
                    < 0.01
            );
            let position = saved
                .btech
                .constructed_units()
                .get(&id)
                .and_then(Mech::position)
                .or_else(|| saved.btech.vehicles().get(&id).and_then(Vehicle::position))
                .unwrap();
            let coordinate = expected.containing_hex().unwrap();
            assert_eq!(
                (i32::from(position.x), i32::from(position.y)),
                (coordinate.x, coordinate.y)
            );
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                saved.btech
            );
        }
        let before = lua.world().btech.clone();
        for name in ["fx", "fy", "fz"] {
            for value in ["NaN", "inf", "1e100", "1e20"] {
                assert!(
                    set_battle_unit_field_action(lua, config, ObjectId(1), id, name, value)
                        .is_err()
                );
                assert_eq!(lua.world().btech, before);
            }
        }
    }
}

/// Template speed changes the firing threshold while preserving actual motion and live mobility.
async fn template_speed_field_changes_shared_attack_penalty_without_propulsion_edits_scenario(
    f: &UnitFields,
) {
    let config = &f.config;
    for source in firing::templates() {
        let (mut world, id, _, _) = f.pair(&source, &source);
        let movable = world
            .btech
            .vehicles()
            .get(&id)
            .is_none_or(|unit| unit.maximum_speed() > 0.0);
        if movable {
            firing::edit(&mut world, id, |unit| unit["motion"]["speed"] = 1.0.into());
        }
        let original = serde_json::to_value(&world.btech).unwrap();
        let native = &f.native;
        let lua = &f.lua;
        f.install(&world);
        for baseline in [0, 100] {
            let before = lua.world().btech.clone();
            lua.drain_outbox();
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'templatesp','{baseline}'); error('abort')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            commands::run(
                native,
                config,
                ObjectId(1),
                1,
                &format!("@setmech TEMPLATESP {baseline}"),
            )
            .unwrap();
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'templatesp','{baseline}')",
                id.0
            ))
            .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            let report =
                view_battle_unit_fields_action(lua, config, ObjectId(1), id, "templatesp").unwrap();
            assert_eq!(
                field(&report, "templatesp")
                    .unwrap()
                    .parse::<f64>()
                    .unwrap(),
                f64::from(baseline)
            );
            let saved = lua.world().clone();
            let penalty = if let Some(unit) = saved.btech.constructed_units().get(&id) {
                unit.attacker_movement_modifier(false)
            } else {
                saved.btech.vehicles()[&id].attacker_movement_modifier(false)
            };
            assert_eq!(
                penalty,
                if !movable {
                    0
                } else if baseline == 0 {
                    2
                } else {
                    1
                }
            );
            let mut actual = serde_json::to_value(&saved.btech).unwrap();
            let collection = if saved.btech.vehicles().contains_key(&id) {
                "vehicles"
            } else {
                "constructed"
            };
            actual[collection][id.0.to_string()]["definition"]["attributes"]
                .as_object_mut()
                .unwrap()
                .remove("template_speed");
            assert_eq!(actual, original);
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                saved.btech
            );
        }
        let before = lua.world().btech.clone();
        for value in ["-1", "NaN", "inf", "1e100", "bad"] {
            assert!(
                set_battle_unit_field_action(lua, config, ObjectId(1), id, "templatesp", value)
                    .is_err()
            );
            assert_eq!(lua.world().btech, before);
        }
    }
}

/// Live propulsion survives restart independently of construction and follows chassis damage rules.
async fn maximum_speed_fields_preserve_mass_and_recalculate_from_the_correct_baseline_scenario(
    f: &UnitFields,
) {
    let config = &f.config;
    for source in firing::templates() {
        let (world, id, _, _) = f.pair(&source, &source);
        let movable = world
            .btech
            .vehicles()
            .get(&id)
            .is_none_or(|unit| unit.maximum_speed() > 0.0);
        let original_load = battle_unit_load(&world, id, true).unwrap();
        let native = &f.native;
        let lua = &f.lua;
        f.install(&world);
        let before = lua.world().btech.clone();
        if !movable {
            assert!(
                set_battle_unit_field_action(lua, config, ObjectId(1), id, "maxspeed", "40")
                    .is_err()
            );
            assert_eq!(lua.world().btech, before);
            continue;
        }
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'maxspeed','40'); error('abort')",
                id.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, before);
        assert!(lua.drain_outbox().is_empty());
        commands::run(native, config, ObjectId(1), 1, "@setmech MAXSPEED 40").unwrap();
        lua.eval_callback::<()>(&format!("btech.unit.set_field(1,{},'maxspeed','40')", id.0))
            .unwrap();
        assert_eq!(lua.world().btech, native.world().btech);
        assert_eq!(
            battle_unit_load(&lua.world(), id, true).unwrap(),
            original_load
        );
        set_battle_unit_field_action(lua, config, ObjectId(1), id, "templatesp", "80").unwrap();
        let report =
            view_battle_unit_fields_action(lua, config, ObjectId(1), id, "maxspeed").unwrap();
        assert_eq!(
            field(&report, "maxspeed").unwrap().parse::<f64>().unwrap(),
            40.0
        );
        let saved = lua.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, saved.btech);
        let expected = if loaded.btech.constructed_units().contains_key(&id) {
            // Life-support damage must not reset the live speed correction.
            destroy_battle_critical(
                &mut loaded,
                id,
                CriticalLocation {
                    section: MechSection::Head,
                    slot: 0,
                },
            )
            .unwrap();
            assert_eq!(
                loaded.btech.constructed_units()[&id]
                    .mobility()
                    .maximum_speed,
                40.0
            );
            destroy_battle_critical(
                &mut loaded,
                id,
                CriticalLocation {
                    section: MechSection::LeftLeg,
                    slot: 1,
                },
            )
            .unwrap();
            80.0 - 10.75
        } else {
            let mut unit = loaded.btech.vehicles()[&id].clone();
            unit.apply_motive_hit(VehicleMotiveHit::SpeedLoss { movement_points: 1 });
            let value = serde_json::to_value(unit).unwrap();
            firing::edit(&mut loaded, id, |unit| *unit = value);
            40.0 - 10.75
        };
        let after = Scripts::new(config, Rc::new(RefCell::new(loaded))).unwrap();
        let report =
            view_battle_unit_fields_action(&after, config, ObjectId(1), id, "maxspeed").unwrap();
        assert_eq!(
            field(&report, "maxspeed").unwrap().parse::<f64>().unwrap(),
            expected
        );
        let saved = after.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
        let before = after.world().btech.clone();
        for value in ["-1", "NaN", "inf", "1e100"] {
            assert!(
                set_battle_unit_field_action(&after, config, ObjectId(1), id, "maxspeed", value)
                    .is_err()
            );
            assert_eq!(after.world().btech, before);
        }
    }
}

/// Jump edits preserve equipment, survive restart and apply subsequent jet losses once.
async fn jump_speed_fields_share_thrust_without_rebuilding_equipment_scenario(f: &UnitFields) {
    let config = &f.config;
    for source in firing::templates() {
        let (mut world, id, _, _) = f.pair(&source, &source);
        let jets: Vec<_> = world
            .btech
            .constructed_units()
            .get(&id)
            .map(|unit| {
                unit.loadout()
                    .unwrap()
                    .systems
                    .into_iter()
                    .filter(|part| part.system == System::JumpJet)
                    .map(|part| part.location)
                    .collect()
            })
            .unwrap_or_default();
        if let Some(&location) = jets.first() {
            destroy_battle_critical(&mut world, id, location).unwrap();
        }
        let load = battle_unit_load(&world, id, true).unwrap();
        let native = &f.native;
        let lua = &f.lua;
        f.install(&world);
        for speed in [21.5, 0.0, 21.5] {
            let before = lua.world().btech.clone();
            lua.drain_outbox();
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'maxjumpspeed','{speed}'); error('abort')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            commands::run(
                native,
                config,
                ObjectId(1),
                1,
                &format!("@setmech MAXJUMPSPEED {speed}"),
            )
            .unwrap();
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'maxjumpspeed','{speed}')",
                id.0
            ))
            .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            assert_eq!(battle_unit_load(&lua.world(), id, true).unwrap(), load);
            let report =
                view_battle_unit_fields_action(lua, config, ObjectId(1), id, "maxjumpspeed")
                    .unwrap();
            assert_eq!(
                field(&report, "maxjumpspeed")
                    .unwrap()
                    .parse::<f64>()
                    .unwrap(),
                speed
            );
            let saved = lua.world().clone();
            if let Some(unit) = saved.btech.constructed_units().get(&id) {
                assert_eq!(unit.jump_capacity(100).unwrap().speed, speed);
                assert_eq!(unit.jump_capacity(200).unwrap().speed, speed / 2.0);
                if let Some(&location) = jets.first() {
                    assert!(unit.critical_destroyed(location));
                }
            }
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                saved.btech
            );
        }
        if let Some(&location) = jets.get(1) {
            let mut loaded = persistence::load(&config.database()).await.unwrap();
            destroy_battle_critical(&mut loaded, id, location).unwrap();
            assert_eq!(
                loaded.btech.constructed_units()[&id]
                    .jump_capacity(100)
                    .unwrap()
                    .speed,
                10.75
            );
            // Repeated loss does not subtract another movement point.
            destroy_battle_critical(&mut loaded, id, location).unwrap();
            assert_eq!(
                loaded.btech.constructed_units()[&id]
                    .jump_capacity(100)
                    .unwrap()
                    .speed,
                10.75
            );
        }
        let before = lua.world().btech.clone();
        for value in ["-1", "NaN", "inf", "1e100", "352256"] {
            assert!(
                set_battle_unit_field_action(lua, config, ObjectId(1), id, "maxjumpspeed", value)
                    .is_err()
            );
            assert_eq!(lua.world().btech, before);
        }
    }
}

/// Injury edits preserve recovery clocks and dice for player-owned and empty cockpit crews.
async fn pilot_damage_fields_share_recovery_and_fatal_cleanup_scenario(f: &UnitFields) {
    let config = &f.config;
    for source in firing::templates() {
        for assigned in [true, false] {
            let (mut world, id, _, _) = f.pair(&source, &source);
            firing::edit(&mut world, id, |unit| {
                unit["pilot_injuries"] = 3.into();
                if !assigned {
                    unit["pilot"] = serde_json::Value::Null;
                    unit["crew_recovery"]["mode"] =
                        serde_json::json!({"kind":"tactical","injuries":3});
                    unit["crew_recovery"]["remaining"] = 17.into();
                }
            });
            if assigned {
                let mut state = serde_json::to_value(&world.btech).unwrap();
                state["recoveries"]["1"]["mode"] =
                    serde_json::json!({"kind":"tactical","injuries":3});
                state["recoveries"]["1"]["remaining"] = 17.into();
                world.btech = serde_json::from_value(state).unwrap();
            }
            let native = &f.native;
            let lua = &f.lua;
            f.install(&world);
            for value in [5, 1, 0] {
                let before = lua.world().btech.clone();
                lua.drain_outbox();
                assert!(
                    lua.eval_callback::<()>(&format!(
                        "btech.unit.set_field(1,{},'pilotdam','{value}'); error('abort')",
                        id.0
                    ))
                    .is_err()
                );
                assert_eq!(lua.world().btech, before);
                assert!(lua.drain_outbox().is_empty());
                commands::run(
                    native,
                    config,
                    ObjectId(1),
                    1,
                    &format!("@setmech PILOTDAM {value}"),
                )
                .unwrap();
                lua.eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'pilotdam','{value}')",
                    id.0
                ))
                .unwrap();
                assert_eq!(native.world().btech, lua.world().btech);
                let mut expected = serde_json::to_value(before).unwrap();
                let collection = if lua.world().btech.vehicles().contains_key(&id) {
                    "vehicles"
                } else {
                    "constructed"
                };
                expected[collection][id.0.to_string()]["pilot_injuries"] = value.into();
                if assigned {
                    expected["recoveries"]["1"]["mode"]["injuries"] = value.into();
                } else {
                    expected[collection][id.0.to_string()]["crew_recovery"]["mode"]["injuries"] =
                        value.into();
                }
                assert_eq!(serde_json::to_value(&lua.world().btech).unwrap(), expected);
                let saved = lua.world().clone();
                persistence::save(&config.database(), &saved).await.unwrap();
                assert_eq!(
                    persistence::load(&config.database()).await.unwrap().btech,
                    saved.btech
                );
            }
            let before = lua.world().btech.clone();
            for value in ["-1", "7", "256", "1.5", "bad"] {
                assert!(
                    set_battle_unit_field_action(lua, config, ObjectId(1), id, "pilotdam", value)
                        .is_err()
                );
                assert_eq!(lua.world().btech, before);
            }
            lua.drain_outbox();
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'pilotdam','6'); error('abort')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            commands::run(native, config, ObjectId(1), 1, "@setmech PILOTDAM 6").unwrap();
            lua.eval_callback::<()>(&format!("btech.unit.set_field(1,{},'pilotdam','6')", id.0))
                .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            let saved = lua.world().clone();
            let state = if let Some(unit) = saved.btech.constructed_units().get(&id) {
                (unit.is_destroyed(), unit.pilot(), unit.power())
            } else {
                let unit = &saved.btech.vehicles()[&id];
                (unit.is_destroyed(), unit.pilot(), unit.power())
            };
            assert_eq!(state, (true, None, Power::Off));
            assert!(
                set_battle_unit_field_action(lua, config, ObjectId(1), id, "pilotdam", "0")
                    .is_err()
            );
            assert_eq!(lua.world().btech, saved.btech);
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                saved.btech
            );
        }
    }
}

/// Saved propulsion must satisfy the same low-gravity capacity bounds as administrative edits.
async fn saved_jump_override_rejects_capacity_overflow_before_runtime_scenario(f: &UnitFields) {
    let config = &f.config;
    let source = include_str!("../game/mechs/JR7-D.toml");
    let (mut world, id, _, _) = f.pair(source, source);
    let location = world.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .systems
        .into_iter()
        .find(|part| part.system == System::JumpJet)
        .unwrap()
        .location;
    destroy_battle_critical(&mut world, id, location).unwrap();
    let scripts = &f.native;
    support::install(scripts, world.clone());
    // The stored baseline includes the destroyed jet, so it can exceed the live-speed bound.
    set_battle_unit_field_action(scripts, config, ObjectId(1), id, "maxjumpspeed", "352255")
        .unwrap();
    let saved = scripts.world().clone();
    saved.validate(config).unwrap();
    persistence::save(&config.database(), &saved).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, saved.btech);
    firing::edit(&mut loaded, id, |unit| {
        unit["propulsion"]["jump"] = (352256.0 + 10.75).into()
    });
    let error = loaded.validate(config).unwrap_err();
    assert!(format!("{error:#}").contains("Invalid jump capacity"));
}

/// Secondary mask edits preserve observed fields and reject partial changes atomically.
async fn secondary_status_edits_preserve_observations_and_validate_controls_scenario(
    f: &UnitFields,
) {
    let config = &f.config;
    for source in firing::templates() {
        let (mut world, id, _, _) = f.pair(&source, &source);
        firing::edit(&mut world, id, |unit| {
            unit["electronics"]["field"]["disturbed"] = true.into()
        });
        let scripts = &f.native;
        support::install(scripts, world.clone());
        for value in ["cwxy", "c", "-", "cdekl"] {
            set_battle_unit_field_action(scripts, config, ObjectId(1), id, "status2", value)
                .unwrap();
            let report =
                view_battle_unit_fields_action(scripts, config, ObjectId(1), id, "status2")
                    .unwrap();
            assert_eq!(field(&report, "status2"), Some(value));
            assert_eq!(
                weapons_hold(&scripts.world(), id).unwrap(),
                value.contains('x')
            );
        }
        let before = scripts.world().btech.clone();
        for value in ["q", "65536", "cq", "abc", "cij", "cF", "cghw"] {
            scripts.drain_outbox();
            assert!(
                set_battle_unit_field_action(scripts, config, ObjectId(1), id, "status2", value)
                    .is_err()
            );
            assert!(
                scripts
                    .eval_callback::<()>(&format!(
                        "btech.unit.set_field(1,{},'status2','{value}')",
                        id.0
                    ))
                    .is_err()
            );
            assert_eq!(scripts.world().btech, before);
            assert!(scripts.drain_outbox().is_empty());
        }
    }
}

/// Live mass edits share load, persistence and rollback without changing construction.
async fn live_mass_fields_share_load_and_expire_on_material_changes_scenario(f: &UnitFields) {
    let config = &f.config;
    for source in firing::templates() {
        let (world, id, target, index) = f.supply(
            &source,
            Some(Weapon::Mml3),
            include_str!("../game/mechs/AS7-D.toml"),
            Some(""),
        );
        let original = battle_unit_load(&world, id, false).unwrap();
        let native = &f.native;
        let lua = &f.lua;
        f.install(&world);
        for mass in [0, 1, 12345, i32::MAX] {
            let value = mass.to_string();
            let before = lua.world().btech.clone();
            lua.drain_outbox();
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'realweight','{value}'); error('abort')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            commands::run(
                native,
                config,
                ObjectId(1),
                1,
                &format!("@setmech REALWEIGHT {value}"),
            )
            .unwrap();
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'realweight','{value}')",
                id.0
            ))
            .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            let report =
                view_battle_unit_fields_action(lua, config, ObjectId(1), id, "realweight").unwrap();
            assert_eq!(field(&report, "realweight"), Some(value.as_str()));
            let saved = lua.world().clone();
            let load = battle_unit_load(&saved, id, false).unwrap();
            assert_eq!(load.material_mass, mass as u32);
            assert_eq!(load.nominal_tons, original.nominal_tons);
            assert_eq!(load.carried_mass, original.carried_mass);
            if mass == i32::MAX {
                assert_eq!(load.maximum_speed(100.0).unwrap(), 0.0);
            }
            if let Some(unit) = saved.btech.constructed_units().get(&id) {
                assert_eq!(unit.mass().unwrap().total, original.material_mass);
                let mut damaged = unit.clone();
                damaged.damage_phase(
                    MechSection::CenterTorso,
                    0,
                    DamagePhase::Armor { rear: false },
                );
                assert_eq!(damaged.effective_mass().unwrap(), mass as u32);
                damaged.damage_phase(
                    MechSection::CenterTorso,
                    1,
                    DamagePhase::Armor { rear: false },
                );
                assert_eq!(
                    damaged.effective_mass().unwrap(),
                    damaged.mass().unwrap().total
                );
            } else {
                let unit = &saved.btech.vehicles()[&id];
                assert_eq!(unit.mass().unwrap().total, original.material_mass);
                let mut damaged = unit.clone();
                damaged
                    .damage_phase(VehicleSection::Front, 0, DamagePhase::Armor { rear: false })
                    .unwrap();
                assert_eq!(damaged.effective_mass().unwrap(), mass as u32);
                damaged
                    .damage_phase(VehicleSection::Front, 1, DamagePhase::Armor { rear: false })
                    .unwrap();
                assert_eq!(
                    damaged.effective_mass().unwrap(),
                    damaged.mass().unwrap().total
                );
            }
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                saved.btech
            );
        }
        let before = lua.world().btech.clone();
        for value in ["-1", "2147483648", "1.5", "NaN", "bad"] {
            assert!(
                set_battle_unit_field_action(lua, config, ObjectId(1), id, "realweight", value)
                    .is_err()
            );
            assert_eq!(lua.world().btech, before);
        }
        lua.eval_callback::<mlua::Table>(&format!(
            "return btech.unit.fire({},1,{index},{})",
            id.0, target.0
        ))
        .unwrap();
        let saved = lua.world().clone();
        let mass = if let Some(unit) = saved.btech.constructed_units().get(&id) {
            unit.mass().unwrap().total
        } else {
            saved.btech.vehicles()[&id].mass().unwrap().total
        };
        assert_eq!(
            battle_unit_load(&saved, id, false).unwrap().material_mass,
            mass
        );
        assert!(mass < original.material_mass);
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
    }
}

/// Nominal tonnage edits change construction-derived mass without rebuilding live unit state.
async fn tonnage_fields_preserve_equipment_damage_and_live_corrections_scenario(f: &UnitFields) {
    let config = &f.config;
    for source in firing::templates() {
        let (mut world, id, _, _) = f.pair(&source, &source);
        let mech = world.btech.constructed_units().contains_key(&id);
        firing::edit(&mut world, id, |state| {
            let section = if mech { "CenterTorso" } else { "front" };
            let armor = state["sections"][section]["armor"].as_u64().unwrap();
            state["sections"][section]["armor"] = (armor - 1).into();
        });
        let original = battle_unit_load(&world, id, false).unwrap();
        let native = &f.native;
        let lua = &f.lua;
        f.install(&world);
        for tons in [original.nominal_tons + 5, original.nominal_tons] {
            let value = tons.to_string();
            let before = lua.world().btech.clone();
            lua.drain_outbox();
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'tons','{value}'); error('abort')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            commands::run(
                native,
                config,
                ObjectId(1),
                1,
                &format!("@setmech TONS {value}"),
            )
            .unwrap();
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'tons','{value}')",
                id.0
            ))
            .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            let report =
                view_battle_unit_fields_action(lua, config, ObjectId(1), id, "tons").unwrap();
            assert_eq!(field(&report, "tons"), Some(value.as_str()));
            let saved = lua.world().clone();
            let mut expected = serde_json::to_value(&before).unwrap();
            let collection = if mech { "constructed" } else { "vehicles" };
            expected[collection][id.0.to_string()]["definition"]["tons"] = tons.into();
            expected["units"][id.0.to_string()]["tons"] = tons.into();
            expected[collection][id.0.to_string()]["definition"]["attributes"]["tons"] =
                value.into();
            assert_eq!(serde_json::to_value(&saved.btech).unwrap(), expected);
            let load = battle_unit_load(&saved, id, false).unwrap();
            assert_eq!(load.nominal_tons, tons);
            if tons != original.nominal_tons {
                assert_ne!(load.material_mass, original.material_mass);
            } else {
                assert_eq!(load, original);
            }
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                saved.btech
            );
        }
        set_battle_unit_field_action(lua, config, ObjectId(1), id, "realweight", "12345").unwrap();
        set_battle_unit_field_action(
            lua,
            config,
            ObjectId(1),
            id,
            "tons",
            &(original.nominal_tons + 5).to_string(),
        )
        .unwrap();
        assert_eq!(
            battle_unit_load(&lua.world(), id, false)
                .unwrap()
                .material_mass,
            12345
        );
        let before = lua.world().btech.clone();
        for value in ["0", "-1", "65536", "1.5", "bad"] {
            assert!(
                set_battle_unit_field_action(lua, config, ObjectId(1), id, "tons", value).is_err()
            );
            assert_eq!(lua.world().btech, before);
        }
        if mech {
            for value in ["19", "37", "105"] {
                assert!(
                    set_battle_unit_field_action(lua, config, ObjectId(1), id, "tons", value)
                        .is_err()
                );
                assert_eq!(lua.world().btech, before);
            }
        } else {
            let mut corrupt = serde_json::to_value(&lua.world().btech.vehicles()[&id]).unwrap();
            corrupt["definition"]["tons"] = 0.into();
            assert!(serde_json::from_value::<Vehicle>(corrupt).is_err());
        }
    }
}

/// Locomotion edits retain material and use the same transaction for every chassis.
async fn movement_fields_preserve_material_and_validate_anatomy_scenario(f: &UnitFields) {
    let config = &f.config;
    for source in firing::templates() {
        let (mut world, id, _, _) = f.pair(&source, &source);
        let mech = world.btech.constructed_units().contains_key(&id);
        let vtol = !mech && world.btech.vehicles()[&id].definition().is_vtol();
        firing::edit(&mut world, id, |state| {
            let section = if mech { "CenterTorso" } else { "front" };
            let armor = state["sections"][section]["armor"].as_u64().unwrap();
            state["sections"][section]["armor"] = (armor - 1).into();
        });
        let native = &f.native;
        let lua = &f.lua;
        f.install(&world);
        let values: &[&str] = if mech {
            &["Biped", "Quad", "Biped"]
        } else if vtol {
            &["VTOL", "None", "VTOL"]
        } else {
            &["Wheel", "Hover", "Track", "None", "Track"]
        };
        for value in values {
            let before = lua.world().btech.clone();
            lua.drain_outbox();
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'mechmovetype','{value}'); error('abort')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            let result = lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'mechmovetype','{value}')",
                id.0
            ));
            commands::run(
                native,
                config,
                ObjectId(1),
                1,
                &format!("@setmech MECHMOVETYPE {value}"),
            )
            .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            result.unwrap_or_else(|error| panic!("{value}: {error:#}"));
            let report =
                view_battle_unit_fields_action(lua, config, ObjectId(1), id, "mechmovetype")
                    .unwrap();
            assert_eq!(field(&report, "mechmovetype"), Some(*value));
            let collection = if mech { "constructed" } else { "vehicles" };
            let old = serde_json::to_value(&before).unwrap();
            let saved = lua.world().clone();
            let new = serde_json::to_value(&saved.btech).unwrap();
            let mut expected = old;
            expected[collection][id.0.to_string()]["definition"]["attributes"]["move_type"] =
                (*value).into();
            let (code, movement) = match *value {
                "Biped" => (0, ""),
                "Quad" => (8, ""),
                "Track" => (1, "tracked"),
                "Wheel" => (2, "wheeled"),
                "Hover" => (3, "hover"),
                "VTOL" => (4, "vtol"),
                "None" => (10, "stationary"),
                _ => unreachable!(),
            };
            expected["units"][id.0.to_string()]["movement_code"] = code.into();
            if !mech {
                expected[collection][id.0.to_string()]["definition"]["movement"] = movement.into();
            }
            assert_eq!(new, expected);
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                saved.btech
            );
        }
        let before = lua.world().btech.clone();
        let invalid = if mech {
            "Track"
        } else if vtol {
            "Hover"
        } else {
            "VTOL"
        };
        for value in [invalid, "Naval", "", "garbage"] {
            assert!(
                set_battle_unit_field_action(lua, config, ObjectId(1), id, "mechmovetype", value)
                    .is_err()
            );
            assert_eq!(lua.world().btech, before);
        }
    }
}

/// Reducing limb capacity must never discard installed critical slots.
async fn movement_field_rejects_overfilled_quad_limbs_atomically_scenario(f: &UnitFields) {
    let config = &f.config;
    let source = &firing::templates()[0];
    let (mut world, id, _, _) = f.pair(source, source);
    firing::edit(&mut world, id, |state| {
        let slots = &mut state["definition"]["sections"]["LeftArm"]["criticals"];
        slots["11"] = slots["0"].clone();
    });
    world.validate(config).unwrap();
    let scripts = &f.native;
    support::install(scripts, world.clone());
    let before = scripts.world().btech.clone();
    let error =
        set_battle_unit_field_action(scripts, config, ObjectId(1), id, "mechmovetype", "Quad")
            .unwrap_err();
    assert!(format!("{error:#}").contains("Critical outside section capacity"));
    assert_eq!(scripts.world().btech, before);
}

/// A construction edit cannot strand chassis-specific posture or a pending event.
async fn movement_field_rejects_incompatible_live_conditions_scenario(f: &UnitFields) {
    let config = &f.config;
    let templates = firing::templates();
    for (source, movement, expected) in [
        (&templates[1], "Biped", "Invalid hull-down posture"),
        (&templates[2], "Hover", "Digging requires"),
    ] {
        let (mut world, id, _, _) = f.pair(source, source);
        let mech = world.btech.constructed_units().contains_key(&id);
        firing::edit(&mut world, id, |state| {
            if mech {
                state["hull_down"] = serde_json::json!({
                    "active": false, "pending": true, "remaining": 7
                });
            } else {
                state["dig"] = serde_json::to_value(DigState::preparing(7)).unwrap();
            }
        });
        world.validate(config).unwrap();
        let scripts = &f.native;
        support::install(scripts, world.clone());
        let before = scripts.world().btech.clone();
        let error = set_battle_unit_field_action(
            scripts,
            config,
            ObjectId(1),
            id,
            "mechmovetype",
            movement,
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains(expected), "{error:#}");
        assert_eq!(scripts.world().btech, before);
    }
}

/// Vehicles expose no conventional jump course and must not acquire a fabricated flight.
async fn vehicle_jump_course_edits_reject_without_mutation_scenario(f: &UnitFields) {
    let config = &f.config;
    for source in firing::templates().into_iter().skip(2) {
        let (world, id, _, _) = f.pair(&source, &source);
        let scripts = &f.native;
        support::install(scripts, world.clone());
        let before = scripts.world().btech.clone();
        for field in ["jumpheading", "jumplength"] {
            for value in ["0", "1"] {
                let error =
                    set_battle_unit_field_action(scripts, config, ObjectId(1), id, field, value)
                        .unwrap_err();
                assert!(format!("{error:#}").contains("require a Mech"));
                assert!(
                    scripts
                        .eval_callback::<()>(&format!(
                            "btech.unit.set_field(1,{},'{field}','{value}')",
                            id.0
                        ))
                        .is_err()
                );
                assert_eq!(scripts.world().btech, before);
            }
        }
    }
}

/// Every supported anatomy emits the same ordered damage-record grammar without side effects.
async fn compact_damage_reports_round_trip_through_the_shared_codec_scenario(f: &UnitFields) {
    let config = &f.config;
    for source in firing::templates() {
        let (mut world, id, _, _) = f.pair(&source, &source);
        let mech = world.btech.constructed_units().contains_key(&id);
        firing::edit(&mut world, id, |state| {
            let section = if mech { "CenterTorso" } else { "front" };
            for layer in ["armor", "internal"] {
                let remaining = state["sections"][section][layer].as_u64().unwrap();
                state["sections"][section][layer] = (remaining - 1).into();
            }
            if let Some(rounds) = state["ammunition"]
                .as_array_mut()
                .and_then(|bins| bins.first_mut())
            {
                let remaining = rounds.as_u64().unwrap();
                if remaining > 0 {
                    *rounds = (remaining - 1).into();
                }
            }
        });
        world.validate(config).unwrap();
        let before = world.btech.clone();
        let text = battle_unit_damage_field(&world, id).unwrap();
        let records = parse_battle_damage_field(&text).unwrap();
        assert!(
            records
                .iter()
                .any(|record| matches!(record, DamageRecord::Armor { loss: 1, .. }))
        );
        assert!(
            records
                .iter()
                .any(|record| matches!(record, DamageRecord::Internal { loss: 1, .. }))
        );
        assert_eq!(
            records
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
            text
        );
        assert_eq!(world.btech, before);
    }
}

/// One shared sandbox per theme; each scenario installs its worlds over it.

#[tokio::test]
async fn unit_fields_layout_filters_and_administrative_edits_harness() {
    let f = UnitFields::new().await;
    all_chassis_field_reports_scenario(&f).await;
    field_layout_filters_scenario(&f).await;
    named_edits_share_validation_scenario(&f).await;
    detached_field_geometry_scenario(&f).await;
    special_field_commands_scenario(&f).await;
}

#[tokio::test]
async fn unit_fields_hardware_identity_and_thermal_harness() {
    let f = UnitFields::new().await;
    hardware_fields_share_gameplay_ranges_and_vtol_fuel_edits_are_atomic_scenario(&f).await;
    identity_edits_preserve_combat_state_and_survive_restart_for_every_chassis_scenario(&f).await;
    runtime_hardware_zeroes_change_gameplay_and_survive_restart_scenario(&f).await;
    thermal_fields_retain_committed_samples_and_share_cooling_with_status_scenario(&f).await;
    thermal_edits_share_native_lua_validation_and_preserve_stored_heat_scenario(&f).await;
}

/// Navigation, metadata, contact counts, cockpit links, names and startup history scenarios.
#[tokio::test]
async fn unit_fields_navigation_metadata_and_history_harness() {
    let f = UnitFields::new().await;
    navigation_fields_share_live_services_without_advancing_them_scenario(&f).await;
    authored_and_edited_metadata_share_validation_across_chassis_and_restart_scenario(&f).await;
    enemy_contact_count_follows_acquisition_and_teams_without_rescanning_scenario(&f).await;
    explicit_cockpit_links_share_named_edits_and_preserve_deferred_references_scenario(&f).await;
    display_name_edits_share_chassis_reports_and_restore_template_fallback_scenario(&f).await;
    startup_history_uses_supplied_completion_time_and_preserves_aborted_history_scenario(&f).await;
}

#[tokio::test]
async fn unit_fields_preferences_values_and_construction_harness() {
    let f = UnitFields::new().await;
    preference_fields_share_cockpit_state_validation_and_restart_scenario(&f).await;
    battle_value_field_tracks_live_damage_and_weapon_configuration_scenario(&f).await;
    construction_fields_share_storage_authority_and_restart_without_changing_physics_scenario(&f)
        .await;
    engine_sink_override_validates_authored_values_across_chassis_scenario(&f).await;
    inactive_readonly_fields_stay_zero_with_damage_and_across_restart_scenario(&f).await;
    crew_and_target_fields_share_native_lua_validation_and_restart_scenario(&f).await;
}

#[tokio::test]
async fn unit_fields_rejections_cargo_and_motion_harness() {
    let f = UnitFields::new().await;
    readonly_unit_fields_reject_native_and_lua_writes_on_every_chassis_scenario(&f).await;
    deferred_field_writes_reject_native_and_lua_edits_across_chassis_scenario(&f).await;
    cargo_field_edits_share_load_rules_and_atomic_publication_scenario(&f).await;
    original_fuel_field_preserves_inventory_and_updates_surplus_load_scenario(&f).await;
    motion_fields_edit_actual_state_without_advancing_controls_scenario(&f).await;
}

#[tokio::test]
async fn unit_fields_coordinates_speed_and_pilot_damage_harness() {
    let f = UnitFields::new().await;
    coordinate_fields_share_scenario_placement_and_rollback_scenario(&f).await;
    precise_coordinate_fields_preserve_fractional_position_and_restart_scenario(&f).await;
    template_speed_field_changes_shared_attack_penalty_without_propulsion_edits_scenario(&f).await;
    maximum_speed_fields_preserve_mass_and_recalculate_from_the_correct_baseline_scenario(&f).await;
    jump_speed_fields_share_thrust_without_rebuilding_equipment_scenario(&f).await;
    pilot_damage_fields_share_recovery_and_fatal_cleanup_scenario(&f).await;
}

#[tokio::test]
async fn unit_fields_overrides_mass_and_anatomy_harness() {
    let f = UnitFields::new().await;
    saved_jump_override_rejects_capacity_overflow_before_runtime_scenario(&f).await;
    secondary_status_edits_preserve_observations_and_validate_controls_scenario(&f).await;
    live_mass_fields_share_load_and_expire_on_material_changes_scenario(&f).await;
    tonnage_fields_preserve_equipment_damage_and_live_corrections_scenario(&f).await;
    movement_fields_preserve_material_and_validate_anatomy_scenario(&f).await;
}

#[tokio::test]
async fn unit_fields_live_rejections_and_codec_harness() {
    let f = UnitFields::new().await;
    movement_field_rejects_overfilled_quad_limbs_atomically_scenario(&f).await;
    movement_field_rejects_incompatible_live_conditions_scenario(&f).await;
    vehicle_jump_course_edits_reject_without_mutation_scenario(&f).await;
    compact_damage_reports_round_trip_through_the_shared_codec_scenario(&f).await;
}
