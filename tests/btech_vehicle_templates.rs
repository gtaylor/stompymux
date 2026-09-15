//! Typed ground-vehicle assets preserve equipment and metadata without entering Mech construction.
use stompymux_rs::*;

const DEMOLISHER: &str = include_str!("../game/mechs/Demolisher");

/// Existing tracked, wheeled, hover and stationary assets retain distinct anatomy and metadata.
#[test]
fn vehicle_assets_decode_without_mech_anatomy() {
    for (source, movement) in [
        (DEMOLISHER, BattleVehicleMovement::Tracked),
        (
            include_str!("../game/mechs/Flatbed_Truck"),
            BattleVehicleMovement::Wheeled,
        ),
        (
            include_str!("../game/mechs/Fulcrum"),
            BattleVehicleMovement::Hover,
        ),
        (
            include_str!("../game/mechs/RadioTower"),
            BattleVehicleMovement::Stationary,
        ),
    ] {
        let vehicle = BattleVehicleTemplate::parse(source).unwrap();
        assert_eq!(vehicle.movement, movement);
        assert!(BattleTemplate::parse(source).is_err());
        assert!(vehicle.sections.contains_key(&BattleVehicleSection::Front));
        let restored: BattleVehicleTemplate =
            serde_json::from_value(serde_json::to_value(&vehicle).unwrap()).unwrap();
        assert_eq!(restored, vehicle);
    }
    let vehicle = BattleVehicleTemplate::parse(DEMOLISHER).unwrap();
    assert_eq!(vehicle.tons, 80);
    assert_eq!(vehicle.max_speed, 53.75);
    assert_eq!(vehicle.heat_sinks, None);
    assert_eq!(vehicle.attributes["specials"], "ICEEngine_Tech");
    let turret = &vehicle.sections[&BattleVehicleSection::Turret];
    assert_eq!(turret.criticals.len(), 6);
    assert_eq!(turret.criticals[&0].equipment, "IS.AC/20");
    assert_eq!(turret.criticals[&1].equipment, "IS.AC/20");
    assert_eq!(turret.criticals[&2].data, "5");
    let truck = BattleVehicleTemplate::parse(include_str!("../game/mechs/Flatbed_Truck")).unwrap();
    assert!(!truck.sections.contains_key(&BattleVehicleSection::Turret));
    assert_eq!(truck.attributes["computer"], "3");
}

/// The shared syntax retains modes and merged specials while rejecting malformed or mixed anatomy.
#[test]
fn vehicle_template_validation_and_shared_syntax() {
    for source in [
        DEMOLISHER.replace("Track", "VTOL"),
        DEMOLISHER.replace("Left_Side", "Left_Arm"),
        DEMOLISHER.replace("Turret", "Rotor"),
        DEMOLISHER.replace("{ 80 }", "{ 0 }"),
        DEMOLISHER.replace("53.75", "NaN"),
        DEMOLISHER.replace("53.75", "-1"),
        DEMOLISHER.replace("CRIT_1-2", "CRIT_1-13"),
        format!("{DEMOLISHER}\nCRIT_1 {{ IS.MediumLaser - - }}"),
        format!("{DEMOLISHER}\nName {{ Duplicate }}"),
        format!("{DEMOLISHER}\nLeft_Side\nArmor {{ 1 }}"),
        format!("{DEMOLISHER}\nSpecials {{ unclosed"),
        "x".repeat(1_048_577),
    ] {
        assert!(BattleVehicleTemplate::parse(&source).is_err());
    }
    assert!(BattleVehicleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).is_err());
    let custom = format!(
        "{}\nSpecials {{\n ICEEngine_Tech CargoTech\n}}\nFuture_Field {{ preserved }}",
        DEMOLISHER.replace("IS.AC/20 - -", "IS.AC/20 - Hotload|RearMount")
    );
    let parsed = BattleVehicleTemplate::parse(&custom).unwrap();
    assert_eq!(parsed.attributes["specials"], "ICEEngine_Tech CargoTech");
    assert_eq!(parsed.attributes["future_field"], "preserved");
    assert_eq!(
        parsed.sections[&BattleVehicleSection::Turret].criticals[&0].modes,
        ["Hotload", "RearMount"]
    );
}

/// Vehicle reads use the same bounded configured-directory access as maps and Mech assets.
#[test]
fn vehicle_asset_reader_is_confined() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("vehicle"), DEMOLISHER).unwrap();
    assert_eq!(
        read_battle_vehicle_template(directory.path(), "vehicle").unwrap(),
        BattleVehicleTemplate::parse(DEMOLISHER).unwrap()
    );
    assert!(read_battle_vehicle_template(directory.path(), "../vehicle").is_err());
    assert!(read_battle_vehicle_template(directory.path(), "/etc/passwd").is_err());
}

/// Bundled vehicle ammunition rows retain whitespace-separated loading and half-ton flags.
#[test]
fn vehicle_assets_accept_multiple_ammunition_mode_words() {
    for source in [
        include_str!("../game/mechs/Jeep"),
        include_str!("../game/mechs/HTracked_APC"),
    ] {
        let definition = BattleVehicleTemplate::parse(source).unwrap();
        let unit = BattleVehicle::new(definition).unwrap();
        let loadout = unit.loadout().unwrap();
        let bin = loadout
            .ammunition
            .iter()
            .find(|bin| bin.weapon == BattleWeapon::MachineGun)
            .unwrap();
        assert_eq!(bin.capacity, 100);
        assert!(bin.half_ton);
        assert!(bin.hotload);
        let restored: BattleVehicle =
            serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
        assert_eq!(unit, restored);
    }
}

/// Bare system entries in existing artillery carriers receive the ordinary empty metadata.
#[test]
fn artillery_vehicle_assets_decode_abbreviated_ecm() {
    for source in [
        include_str!("../game/mechs/Huey"),
        include_str!("../game/mechs/Huitzilopochtli"),
    ] {
        let definition = BattleVehicleTemplate::parse(source).unwrap();
        let vehicle = BattleVehicle::new(definition).unwrap();
        assert!(
            vehicle
                .loadout()
                .unwrap()
                .systems
                .iter()
                .any(|part| part.system == BattleSystem::Ecm)
        );
    }
}

/// The copied Streak carrier asset records the reference loader's effective full-bin quantity.
#[test]
fn svantovit_streak_records_valid_full_bin_capacity() {
    let definition =
        BattleVehicleTemplate::parse(include_str!("../game/mechs/Svantovit-Streak")).unwrap();
    let vehicle = BattleVehicle::new(definition).unwrap();
    let loadout = vehicle.loadout().unwrap();
    let bin = loadout
        .ammunition
        .iter()
        .find(|bin| bin.weapon == BattleWeapon::ClanStreakSrm4)
        .unwrap();
    assert_eq!((bin.rounds, bin.capacity), (25, 25));
    assert!(!bin.half_ton);
}

/// The copied tracked transport has ground anatomy, without the stray aircraft section.
#[test]
fn tracked_transport_asset_has_only_ground_sections() {
    let definition =
        BattleVehicleTemplate::parse(include_str!("../game/mechs/J-27_Transport")).unwrap();
    assert_eq!(definition.movement, BattleVehicleMovement::Tracked);
    assert_eq!(definition.sections.len(), 5);
    let vehicle = BattleVehicle::new(definition).unwrap();
    assert!(!vehicle.is_destroyed());
    assert!(vehicle.loadout().unwrap().weapons.is_empty());
}

/// Rotorcraft retain the same hull slots, weapon definitions and ammunition parser as ground units.
#[test]
fn vtol_assets_share_vehicle_anatomy_and_equipment_without_ground_admission() {
    let mut count = 0;
    for entry in std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/game/mechs")).unwrap() {
        let path = entry.unwrap().path();
        if !path.is_file() {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        if !source
            .lines()
            .any(|line| line.starts_with("Type") && line.contains("VTOL"))
        {
            continue;
        }
        let template = BattleVehicleTemplate::parse(&source)
            .unwrap_or_else(|error| panic!("{}: {error:#}", path.display()));
        assert!(template.is_vtol());
        assert!(matches!(
            template.movement,
            BattleVehicleMovement::Vtol | BattleVehicleMovement::Stationary
        ));
        assert!(template.sections[&BattleVehicleSection::Rotor].internal > 0);
        let loadout = BattleVehicleLoadout::resolve(&template)
            .unwrap_or_else(|error| panic!("{}: {error:#}", path.display()));
        assert!(
            loadout
                .weapons
                .iter()
                .all(|mount| !mount.criticals.is_empty())
        );
        let restored = serde_json::from_value::<BattleVehicleTemplate>(
            serde_json::to_value(&template).unwrap(),
        )
        .unwrap();
        assert_eq!(restored, template);
        // Parsed aircraft share material construction; world admission is checked separately.
        count += 1;
    }
    assert_eq!(count, 34);
}

/// Type/locomotion mismatches and missing rotors must not create ambiguous chassis definitions.
#[test]
fn vtol_anatomy_requires_matching_type_movement_and_rotor() {
    let source = include_str!("../game/mechs/Kestrel");
    for invalid in [
        source.replacen("{ VTOL }", "{ Vehicle }", 1),
        source.replace("Move_Type        { VTOL }", "Move_Type        { Track }"),
        source.replace("Rotor", "Turret"),
    ] {
        assert!(BattleVehicleTemplate::parse(&invalid).is_err(), "{invalid}");
    }
    for extended in [false, true] {
        assert_eq!(
            BattleVehicleMovement::Vtol.piloting_skill(extended),
            Some("Piloting-Aerospace")
        );
    }
    for (tons, suspension) in [(10, 50), (11, 95), (20, 95), (21, 140), (30, 140)] {
        assert_eq!(BattleVehicleMovement::Vtol.suspension(tons), suspension);
    }
    let mut forged = BattleVehicleTemplate::parse(DEMOLISHER).unwrap();
    forged.movement = BattleVehicleMovement::Vtol;
    assert!(BattleVehicle::new(forged).is_err());
    let template = BattleVehicleTemplate::parse(source).unwrap();
    let engine = template.engine().unwrap();
    assert_eq!(engine.nominal_rating, 300);
    assert_eq!(engine.weight_rating, 160);
    let mass = template.mass().unwrap();
    assert_eq!(mass.components, 2560);
    assert_eq!(mass.cockpit, 1280);
    assert_eq!(mass.engine, engine.installed_mass);
}
