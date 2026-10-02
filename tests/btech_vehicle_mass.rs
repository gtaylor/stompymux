//! Intact vehicle mass diagnostics cover hull technologies, equipment, cooling and listed ammunition.
use stompymux_rs::*;

/// The tracked Demolisher supplies a fixed 80-ton chassis and two whole turret weapons.
fn vehicle(flags: &str) -> BattleVehicleTemplate {
    let mut template =
        BattleVehicleTemplate::parse("Demolisher", include_str!("../game/mechs/Demolisher.toml"))
            .unwrap();
    template.attributes.insert("specials".into(), flags.into());
    template
}

#[test]
fn intact_vehicle_mass_separates_design_bins_from_loaded_rounds() {
    let mut template = vehicle("ICEEngine_Tech");
    let original = template.clone();
    let mass = template.mass().unwrap();
    assert_eq!(
        (mass.engine, mass.cockpit, mass.turret),
        (23 * 1024, 4 * 1024, 3 * 1024)
    );
    assert_eq!(
        (mass.structure, mass.armor, mass.equipment),
        (8 * 1024, 10 * 1024, 28 * 1024)
    );
    assert_eq!((mass.components, mass.cooling, mass.cargo), (0, 0, 0));
    assert_eq!(
        (mass.ammunition, mass.ammunition_capacity),
        (4 * 1024, 4 * 1024)
    );
    assert_eq!(mass.total, 80 * 1024);
    assert_eq!(mass.total, mass.design_total);
    assert_eq!(template, original);
    let restored: BattleVehicleTemplate =
        serde_json::from_value(serde_json::to_value(&template).unwrap()).unwrap();
    assert_eq!(restored.mass().unwrap(), mass);
    template
        .sections
        .get_mut(&BattleVehicleSection::Turret)
        .unwrap()
        .criticals
        .get_mut(&2)
        .unwrap()
        .data = "0".into();
    let empty_bin = template.mass().unwrap();
    assert_eq!(empty_bin.design_total, mass.design_total);
    assert_eq!(empty_bin.total, mass.total - 1024);
    let bin = template
        .sections
        .get_mut(&BattleVehicleSection::Turret)
        .unwrap()
        .criticals
        .get_mut(&2)
        .unwrap();
    bin.modes.push("Halfton".into());
    let half_bin = template.mass().unwrap();
    assert_eq!(half_bin.design_total, mass.design_total - 512);
    assert_eq!(half_bin.total, empty_bin.total);
    let truck = BattleVehicleTemplate::parse(
        "Flatbed_Truck",
        include_str!("../game/mechs/Flatbed_Truck.toml"),
    )
    .unwrap()
    .mass()
    .unwrap();
    assert_eq!((truck.total, truck.turret), (4 * 1024, 0));
    let hover = BattleVehicleTemplate::parse("Fulcrum", include_str!("../game/mechs/Fulcrum.toml"))
        .unwrap()
        .mass()
        .unwrap();
    assert_eq!(
        (hover.engine, hover.components, hover.cockpit),
        (21 * 512, 5 * 1024, 5 * 512)
    );
}

#[test]
fn vehicle_mass_applies_chassis_material_cooling_and_cargo_rules() {
    for (flag, structure, armor) in [
        ("-", 8 * 1024, 10 * 1024),
        ("EndoSteel_Tech", 4 * 1024, 10 * 1024),
        ("CompositeInternal_Tech", 4 * 1024, 10 * 1024),
        (
            "ReinforcedInternal_Tech EndoSteel_Tech",
            16 * 1024,
            10 * 1024,
        ),
        ("FerroFibrous_Tech", 8 * 1024, 9 * 1024),
        ("Clan FerroFibrous_Tech", 8 * 1024, 17 * 512),
        ("HvyFerroFibrous_Tech", 8 * 1024, 17 * 512),
        ("LtFerroFibrous_Tech", 8 * 1024, 19 * 512),
        ("HardenedArmor_Tech", 8 * 1024, 20 * 1024),
    ] {
        let mass = vehicle(flag).mass().unwrap();
        assert_eq!((mass.structure, mass.armor), (structure, armor), "{flag}");
    }
    for (flags, cooling, cargo) in [
        ("-", 15 * 1024, 2048),
        ("DoubleHS", 2 * 1024, 2048),
        ("Clan", 2 * 1024, 2048),
        ("ICEEngine_Tech", 25 * 1024, 2048),
        ("ICEEngine_Tech DoubleHS CargoTech", 12 * 1024, 10 * 1024),
        ("Carrier_Tech CargoTech", 15 * 1024, 1024),
    ] {
        let mut template = vehicle(flags);
        template.heat_sinks = Some(25);
        template
            .attributes
            .insert("cargo_space".into(), "1000".into());
        let mass = template.mass().unwrap();
        assert_eq!((mass.cooling, mass.cargo), (cooling, cargo), "{flags}");
    }
    let mut template = vehicle("-");
    for value in ["-1", "NaN", "4294967295"] {
        template
            .attributes
            .insert("cargo_space".into(), value.into());
        assert!(template.mass().is_err(), "{value}");
    }
    template.attributes.remove("cargo_space");
    template.tons = 11;
    template.max_speed = 16.125;
    assert!(template.engine().unwrap().standard_mass.is_none());
    assert_eq!(template.mass().unwrap().engine, 0);
}

#[test]
fn vehicle_systems_use_whole_installation_mass_without_duplicate_sink_weight() {
    for (equipment, expected) in [
        ("C3Master", 5120),
        ("C3i", 2560),
        ("AngelEcm", 2048),
        ("BloodhoundProbe", 2048),
        ("BeagleProbe", 1536),
        ("Ecm", 1536),
        ("Light_BAP", 512),
        ("CASE", 512),
        ("TAG", 1024),
        ("HeatSink", 0),
        ("FerroFibrous", 0),
        ("JumpJet", 1024),
    ] {
        let mut template = vehicle("ICEEngine_Tech");
        for section in template.sections.values_mut() {
            section.criticals.clear();
        }
        template
            .sections
            .get_mut(&BattleVehicleSection::Front)
            .unwrap()
            .criticals
            .insert(
                0,
                CriticalDefinition {
                    equipment: equipment.into(),
                    data: "-".into(),
                    modes: vec![],
                    brand: None,
                },
            );
        let report = template.mass().unwrap();
        assert_eq!(report.equipment, expected, "{equipment}");
        assert_eq!((report.turret, report.cooling), (0, 0));
    }
    let mut template = vehicle("ICEEngine_Tech");
    template.sections.remove(&BattleVehicleSection::Left);
    assert!(template.mass().is_err());
    let mut template = vehicle("ICEEngine_Tech");
    template
        .sections
        .get_mut(&BattleVehicleSection::Turret)
        .unwrap()
        .internal = 0;
    let report = template.mass().unwrap();
    assert_eq!(
        (report.turret, report.equipment, report.ammunition),
        (0, 0, 0)
    );
    assert_eq!(report.armor, 15 * 512);
}

/// Stationary construction needs no propulsion; hovercraft retain their minimum engine mass.
#[test]
fn stationary_construction_has_no_propulsion_mass() {
    let mut template = vehicle("-");
    template.movement = BattleVehicleMovement::Stationary;
    template.max_speed = 0.0;
    let before = template.clone();
    let engine = template.engine().unwrap();
    assert_eq!(engine.nominal_rating, 0);
    assert_eq!(engine.standard_mass, None);
    assert_eq!(engine.installed_mass, 0);
    let mass = template.mass().unwrap();
    assert_eq!(mass.engine, 0);
    assert!(mass.equipment > 0 && mass.armor > 0 && mass.structure > 0);
    assert_eq!(template, before);
    for movement in [
        BattleVehicleMovement::Tracked,
        BattleVehicleMovement::Wheeled,
    ] {
        template.movement = movement;
        assert_eq!(template.mass().unwrap().engine, 0);
    }
    template.movement = BattleVehicleMovement::Hover;
    assert_eq!(
        template.mass().unwrap().engine,
        u32::from(template.tons) * 1024 / 5
    );
    template.movement = BattleVehicleMovement::Stationary;
    template.tons = 11;
    template.max_speed = 16.125;
    assert_eq!(template.mass().unwrap().engine, 0);
}

/// Non-catalogue hover ratings retain diagnostics but use the reference propulsion minimum.
#[test]
fn hovercraft_missing_catalogue_rating_uses_mass_floor() {
    let template =
        BattleVehicleTemplate::parse("Shamash", include_str!("../game/mechs/Shamash.toml"))
            .unwrap();
    let engine = template.engine().unwrap();
    assert_eq!(engine.weight_rating, 58);
    assert_eq!(engine.standard_mass, None);
    assert_eq!(engine.engine_mass, 0);
    assert_eq!(engine.installed_mass, 11 * 1024 / 5);
    let vehicle = BattleVehicle::new(template.clone()).unwrap();
    assert_eq!(
        vehicle.definition().mass().unwrap().engine,
        engine.installed_mass
    );
    let restored: BattleVehicle =
        serde_json::from_value(serde_json::to_value(&vehicle).unwrap()).unwrap();
    assert_eq!(restored, vehicle);
    for flags in [
        "ICEEngine_Tech",
        "XLEngine_Tech",
        "XXL_Tech",
        "CompactEngine_Tech",
    ] {
        let mut modified = template.clone();
        modified.attributes.insert("specials".into(), flags.into());
        assert_eq!(modified.mass().unwrap().engine, engine.installed_mass);
    }
}

/// Physical mass follows material and live inventory while broken mounts retain their metal.
#[test]
fn live_vehicle_mass_tracks_ammunition_protection_and_section_loss() {
    let mut unit = BattleVehicle::new(vehicle("ICEEngine_Tech")).unwrap();
    let intact = unit.mass().unwrap();
    assert_eq!(intact, unit.definition().mass().unwrap());
    unit.expend_ammunition(0, 1).unwrap();
    assert_eq!(unit.mass().unwrap().ammunition, 3 * 1024 + 819);
    assert_eq!(unit.mass().unwrap().design_total, intact.design_total);
    unit.destroy_critical(VehicleCriticalLocation {
        section: BattleVehicleSection::Turret,
        slot: 0,
    })
    .unwrap();
    assert_eq!(unit.mass().unwrap().equipment, intact.equipment);
    unit.damage_phase(
        BattleVehicleSection::Front,
        16,
        BattleDamagePhase::Armor { rear: false },
    )
    .unwrap();
    assert_eq!(unit.mass().unwrap().armor, 9 * 1024);
    unit.damage_phase(BattleVehicleSection::Front, 4, BattleDamagePhase::Internal)
        .unwrap();
    assert_eq!(unit.mass().unwrap().structure, 15 * 512);
    unit.damage_phase(BattleVehicleSection::Turret, 8, BattleDamagePhase::Internal)
        .unwrap();
    let mass = unit.mass().unwrap();
    assert_eq!(
        (
            mass.turret,
            mass.equipment,
            mass.ammunition,
            mass.ammunition_capacity
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(mass.structure, 6 * 1024);
    assert_eq!(mass.armor, 13 * 512);
    assert!(!unit.is_destroyed());
    assert_eq!(unit.definition().mass().unwrap(), intact);
    let restored: BattleVehicle =
        serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
    assert_eq!(restored.mass().unwrap(), mass);
    for section in [
        BattleVehicleSection::Left,
        BattleVehicleSection::Right,
        BattleVehicleSection::Front,
        BattleVehicleSection::Rear,
    ] {
        unit.damage_phase(section, u16::MAX, BattleDamagePhase::Internal)
            .unwrap();
    }
    let wreck = unit.mass().unwrap();
    assert_eq!(
        (
            wreck.engine,
            wreck.cockpit,
            wreck.components,
            wreck.structure,
            wreck.armor
        ),
        (0, 0, 0, 0, 0)
    );
    assert_eq!(wreck.total, 1);
}

/// Crew death does not remove machinery, and large protection values do not overflow intermediate mass.
#[test]
fn live_vehicle_mass_separates_material_loss_from_crew_loss() {
    let mut template = vehicle("ICEEngine_Tech");
    for layout in template.sections.values_mut() {
        layout.internal = u16::MAX;
    }
    let unit = BattleVehicle::new(template).unwrap();
    assert_eq!(unit.mass().unwrap().structure, 8 * 1024);
    let mut saved = serde_json::to_value(&unit).unwrap();
    saved["pilot_injuries"] = 6.into();
    saved["pilot_killed"] = true.into();
    let dead: BattleVehicle = serde_json::from_value(saved).unwrap();
    assert!(dead.is_destroyed());
    assert_eq!(dead.mass().unwrap(), unit.mass().unwrap());
}
