//! Cargo installations share construction accounting across chassis without absorbing loose-stock mass.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// All admitted movement classes use the same cargo-space definition.
fn templates() -> Vec<String> {
    let ground = include_str!("../game/mechs/Demolisher.toml");
    vec![
        include_str!("../game/mechs/JR7-D.toml").into(),
        include_str!("../game/mechs/GOL-1H.toml").into(),
        ground.into(),
        ground.replace("Tracked", "Wheeled"),
        ground.replace("Tracked", "Hover"),
        ground.replace("Tracked", "None"),
        include_str!("../game/mechs/Kestrel.toml").into(),
    ]
}

/// Preserve other construction technologies while changing only cargo installation facts.
fn configured(source: &str, space: &str, flags: &str) -> BattleUnitTemplate {
    let mut template = BattleUnitTemplate::parse("test",source).unwrap();
    let attributes = match &mut template {
        BattleUnitTemplate::Mech(template) => &mut template.attributes,
        BattleUnitTemplate::Vehicle(template) => &mut template.attributes,
    };
    let previous = attributes.get("specials").cloned().unwrap_or_default();
    let mut technologies = previous
        .split_whitespace()
        .filter(|flag| {
            !["-", "CargoTech", "Carrier_Tech"]
                .iter()
                .any(|omit| flag.eq_ignore_ascii_case(omit))
        })
        .map(str::to_owned)
        .collect::<Vec<_>>();
    technologies.extend(flags.split_whitespace().map(str::to_owned));
    attributes.insert(
        "specials".into(),
        if technologies.is_empty() {
            "-".into()
        } else {
            technologies.join(" ")
        },
    );
    attributes.insert("cargo_space".into(), space.into());
    template
}

/// Material inspection is a projection of the owning chassis, not a second cargo calculation.
fn mass(world: &World, id: ObjectId) -> (u32, u32) {
    if let Some(unit) = world.btech.constructed_units().get(&id) {
        let mass = unit.mass().unwrap();
        return (mass.cargo, mass.total);
    }
    let mass = world.btech.vehicles()[&id].mass().unwrap();
    (mass.cargo, mass.total)
}

/// Fractional installation mass, technology precedence, stock separation and restart agree on every chassis.
#[tokio::test]
async fn cargo_space_mass_is_shared_and_persists_with_ordinary_stock() {
    for source in templates() {
        for (flags, expected) in [
            ("", 251),
            ("CargoTech", 1259),
            ("Carrier_Tech", 125),
            ("CargoTech Carrier_Tech", 125),
        ] {
            let (_dir, config, mut world) = support::isolated_world().await;
            let empty = world.create(&config, "Empty".into(), Kind::Thing);
            configured(&source, "0", flags)
                .create(&mut world, empty)
                .unwrap();
            let unit = world.create(&config, "Cargo".into(), Kind::Thing);
            configured(&source, "123", flags)
                .create(&mut world, unit)
                .unwrap();
            let base = mass(&world, empty).1;
            assert_eq!(mass(&world, unit), (expected, base + expected));
            let material = battle_unit_load(&world, unit, true).unwrap().material_mass;
            assert_eq!(material, base + expected);
            set_battle_inventory_named(&mut world, ObjectId(1), unit, "Gold", 0, 2).unwrap();
            assert_eq!(mass(&world, unit), (expected, base + expected));
            assert!(battle_unit_load(&world, unit, true).unwrap().carried_mass > 0);
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let reported: u32 = scripts
                .eval_callback(&format!("return btech.unit.state({}).mass.cargo", unit.0))
                .unwrap();
            assert_eq!(reported, expected);
            let fields =
                view_battle_unit_fields_action(&scripts, &config, ObjectId(1), unit, "cargo")
                    .unwrap();
            assert_eq!(fields.fields[0].name, "cargospace");
            assert_eq!(fields.fields[0].value.as_deref(), Some("123"));
            let lua: mlua::Table = scripts
                .eval_callback(&format!("return btech.unit.fields(1,{},'cargo')", unit.0))
                .unwrap();
            assert_eq!(
                serde_json::to_value(lua).unwrap(),
                serde_json::to_value(fields).unwrap()
            );
            let saved = scripts.world().clone();
            saved.validate(&config).unwrap();
            persistence::save(&config.database(), &saved).await.unwrap();
            let loaded = persistence::load(&config.database()).await.unwrap();
            assert_eq!(loaded.btech, saved.btech);
            assert_eq!(mass(&loaded, unit), (expected, base + expected));
        }
    }
}

/// Invalid capacities and totals cannot publish a partially constructed unit.
#[tokio::test]
async fn cargo_space_rejects_invalid_capacity_and_mass_overflow_before_creation() {
    for source in templates() {
        let (_dir, config, mut world) = support::isolated_world().await;
        let unit = world.create(&config, "Cargo".into(), Kind::Thing);
        for (space, flags) in [
            ("-1", ""),
            ("1.5", ""),
            ("4294967296", ""),
            ("4294967295", "CargoTech"),
            ("4194303750", "Carrier_Tech"),
        ] {
            let before = world.btech.clone();
            assert!(
                configured(&source, space, flags)
                    .create(&mut world, unit)
                    .is_err(),
                "{space} {flags}"
            );
            assert_eq!(world.btech, before);
        }
    }
}

/// Losing a limb changes surviving material but does not erase an unlocated cargo installation.
#[test]
fn mech_cargo_installation_survives_section_loss_and_does_not_enable_suits() {
    let BattleUnitTemplate::Mech(template) = configured(&templates()[0], "500", "CargoTech") else {
        unreachable!()
    };
    let unit = BattleUnit::from_template(template.clone()).unwrap();
    let before = unit.mass().unwrap();
    assert_eq!(before.cargo, 5120);
    let mut saved = serde_json::to_value(&unit).unwrap();
    saved["sections"]["LeftArm"]["internal"] = 0.into();
    saved["sections"]["LeftArm"]["armor"] = 0.into();
    let damaged: BattleUnit = serde_json::from_value(saved).unwrap();
    assert_eq!(damaged.mass().unwrap().cargo, before.cargo);
    assert!(damaged.mass().unwrap().total < before.total);
    let mut template = template;
    template.attributes.insert("max_suits".into(), "1".into());
    assert!(BattleUnit::from_template(template).is_err());
}

/// An empty but overweight cargo installation participates in the common propulsion limit.
#[tokio::test]
async fn cargo_installation_alone_applies_load_speed_limits() {
    for source in templates() {
        let (_dir, config, mut world) = support::isolated_world().await;
        let unit = world.create(&config, "Cargo".into(), Kind::Thing);
        configured(&source, "50000", "CargoTech")
            .create(&mut world, unit)
            .unwrap();
        assert_eq!(battle_inventory_mass(&world, unit).unwrap(), 0);
        assert_eq!(
            battle_unit_load(&world, unit, true).unwrap().carried_mass,
            0
        );
        assert_eq!(battle_throttle_maximum(&world, unit, true).unwrap(), 0.0);
    }
}
