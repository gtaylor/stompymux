//! Cargo bays share construction accounting across chassis without absorbing loose-stock mass:
//! a bay weighs its capacity in tons whatever the unit's technology flags (TechManual p. 239).
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// All admitted movement classes use the same cargo bay definition. The vehicles give up
/// ammunition so that their cargo bays fit within their tonnage (TechManual p. 97).
fn templates() -> Vec<String> {
    let demolisher = include_str!("../game/units/Demolisher.toml");
    let hover = support::templates::hovercraft(demolisher).replace(
        "    { at = \"3-4\", item = \"Ammo_IS.AC/20\", rounds = 5 },\n",
        "",
    );
    let ground = demolisher.replace(
        "{ at = \"3-6\", item = \"Ammo_IS.AC/20\", rounds = 5 }",
        "{ at = \"3-4\", item = \"Ammo_IS.AC/20\", rounds = 5 }",
    );
    vec![
        include_str!("../game/units/JR7-D.toml").into(),
        include_str!("../game/units/GOL-1H.toml").into(),
        ground.clone(),
        ground.replace("movement = \"track\"", "movement = \"wheel\""),
        hover,
        ground
            .replace("movement = \"track\"", "movement = \"none\"")
            .replace("walk_mp = 3", "walk_mp = 0"),
        include_str!("../game/units/Kestrel.toml").into(),
    ]
}

/// Preserve other construction technologies while changing only the cargo bay and flags.
fn configured(source: &str, tons: &str, flags: &str) -> UnitTemplate {
    let mut template = UnitTemplate::parse("test", source).unwrap();
    let attributes = match &mut template {
        UnitTemplate::Mech(template) => &mut template.attributes,
        UnitTemplate::Vehicle(template) => &mut template.attributes,
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
    attributes.insert(CARGO_TONS.into(), tons.into());
    // A Mech's cargo bay comes out of its armor, so the design stays at its tonnage.
    if cargo_mass(Some(tons)).is_ok_and(|mass| mass < 1000 * 1024) {
        return crate::support::templates::legal_unit(template);
    }
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

/// A bay weighs its capacity whatever the technology flags; stock separation and restart agree
/// on every chassis.
#[tokio::test]
async fn cargo_bay_mass_is_shared_and_persists_with_ordinary_stock() {
    for source in templates() {
        for flags in ["", "CargoTech", "Carrier_Tech", "CargoTech Carrier_Tech"] {
            let expected = 3 * 512;
            let (_dir, config, mut world) = support::isolated_world().await;
            let empty = world.create(&config, "Empty".into(), Kind::Thing);
            configured(&source, "0", flags)
                .create(&mut world, empty)
                .unwrap();
            let unit = world.create(&config, "Cargo".into(), Kind::Thing);
            configured(&source, "1.5", flags)
                .create(&mut world, unit)
                .unwrap();
            // A Mech gives up armor for the installation; a vehicle adds it to its weight.
            let base = if world.btech.constructed_units().contains_key(&unit) {
                mass(&world, unit).1 - expected
            } else {
                mass(&world, empty).1
            };
            assert_eq!(mass(&world, unit), (expected, base + expected));
            let material = battle_unit_load(&world, unit, true).unwrap().material_mass;
            assert_eq!(material, base + expected);
            set_battle_inventory_named(&mut world, ObjectId(1), unit, "Gold", 2).unwrap();
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
            // The field states the capacity in hundredths of a ton.
            assert_eq!(fields.fields[0].value.as_deref(), Some("150"));
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

/// Invalid capacities and totals cannot publish a partially constructed unit: a bay is a
/// nonnegative whole number of half tons (TechManual p. 239).
#[tokio::test]
async fn cargo_bay_rejects_invalid_capacity_and_mass_overflow_before_creation() {
    for source in templates() {
        let (_dir, config, mut world) = support::isolated_world().await;
        let unit = world.create(&config, "Cargo".into(), Kind::Thing);
        for tons in ["-1", "1.2", "NaN", "inf", "4194304"] {
            let before = world.btech.clone();
            assert!(
                configured(&source, tons, "CargoTech")
                    .create(&mut world, unit)
                    .is_err(),
                "{tons}"
            );
            assert_eq!(world.btech, before);
        }
    }
}

/// Losing a limb changes surviving material but does not erase an unlocated cargo installation.
#[test]
fn mech_cargo_installation_survives_section_loss_and_does_not_enable_suits() {
    let UnitTemplate::Mech(template) = configured(&templates()[0], "5", "CargoTech") else {
        unreachable!()
    };
    let unit = Mech::from_template(crate::support::templates::legal(template.clone())).unwrap();
    let before = unit.mass().unwrap();
    assert_eq!(before.cargo, 5120);
    let mut saved = serde_json::to_value(&unit).unwrap();
    saved["sections"]["LeftArm"]["internal"] = 0.into();
    saved["sections"]["LeftArm"]["armor"] = 0.into();
    let damaged: Mech = serde_json::from_value(saved).unwrap();
    assert_eq!(damaged.mass().unwrap().cargo, before.cargo);
    assert!(damaged.mass().unwrap().total < before.total);
    let mut template = template;
    template.attributes.insert("max_suits".into(), "1".into());
    assert!(Mech::from_template(template).is_err());
}

/// A cargo bay heavier than the chassis can carry is a construction error for a combat
/// vehicle (TechManual p. 97); an empty installation is never a speed penalty.
#[tokio::test]
async fn overweight_cargo_installations_are_refused_rather_than_slowed() {
    for source in templates() {
        let (_dir, config, mut world) = support::isolated_world().await;
        let unit = world.create(&config, "Cargo".into(), Kind::Thing);
        let template = configured(&source, "500", "CargoTech");
        let stationary = matches!(
            &template,
            UnitTemplate::Vehicle(vehicle) if vehicle.movement == VehicleMovement::Stationary
        );
        let vehicle = matches!(template, UnitTemplate::Vehicle(_));
        match template.create(&mut world, unit) {
            Err(error) => {
                // A Mech's installation counts toward its tonnage (TechManual p. 46).
                let expected = if vehicle {
                    "TechManual p. 97"
                } else {
                    "-ton chassis"
                };
                assert!(!stationary, "{error:#}");
                assert!(format!("{error:#}").contains(expected), "{error:#}");
            }
            Ok(()) => {
                assert!(!vehicle || stationary);
                assert_eq!(
                    battle_unit_load(&world, unit, true).unwrap().carried_mass,
                    0
                );
                assert!(battle_throttle_maximum(&world, unit, true).unwrap() > 0.0 || stationary);
            }
        }
    }
}
