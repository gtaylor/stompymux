//! Typed ground-vehicle assets preserve equipment and metadata without entering Mech construction.
use stompymux_rs::*;

const DEMOLISHER: &str = include_str!("../game/mechs/Demolisher.toml");

/// Existing tracked, wheeled, hover and stationary assets retain distinct anatomy and metadata.
#[test]
fn vehicle_assets_decode_without_mech_anatomy() {
    for (source, movement) in [
        (DEMOLISHER, VehicleMovement::Tracked),
        (
            include_str!("../game/mechs/Flatbed_Truck.toml"),
            VehicleMovement::Wheeled,
        ),
        (
            include_str!("../game/mechs/Fulcrum.toml"),
            VehicleMovement::Hover,
        ),
        (
            include_str!("../game/mechs/RadioTower.toml"),
            VehicleMovement::Stationary,
        ),
    ] {
        let vehicle = VehicleTemplate::parse("test", source).unwrap();
        assert_eq!(vehicle.movement, movement);
        assert!(MechTemplate::parse("test", source).is_err());
        assert!(vehicle.sections.contains_key(&VehicleSection::Front));
        let restored: VehicleTemplate =
            serde_json::from_value(serde_json::to_value(&vehicle).unwrap()).unwrap();
        assert_eq!(restored, vehicle);
    }
    let vehicle = VehicleTemplate::parse("Demolisher", DEMOLISHER).unwrap();
    assert_eq!(vehicle.tons, 80);
    assert_eq!(vehicle.max_speed, 53.75);
    assert_eq!(vehicle.heat_sinks, None);
    assert_eq!(vehicle.attributes["specials"], "ICEEngine_Tech");
    let turret = &vehicle.sections[&VehicleSection::Turret];
    assert_eq!(turret.criticals.len(), 6);
    assert_eq!(turret.criticals[&0].equipment, "IS.AC/20");
    assert_eq!(turret.criticals[&1].equipment, "IS.AC/20");
    assert_eq!(turret.criticals[&2].data, "5");
    let truck = VehicleTemplate::parse(
        "Flatbed_Truck",
        include_str!("../game/mechs/Flatbed_Truck.toml"),
    )
    .unwrap();
    assert!(!truck.sections.contains_key(&VehicleSection::Turret));
}

/// The shared syntax retains modes and merged specials while rejecting malformed or mixed anatomy.
#[test]
fn vehicle_template_validation_and_shared_syntax() {
    for source in [
        DEMOLISHER.replace("movement = \"track\"", "movement = \"vtol\""),
        DEMOLISHER.replace("class = \"vehicle\"", "class = \"Vehicle\""),
        DEMOLISHER.replace("[sections.left_side]", "[sections.left_arm]"),
        DEMOLISHER.replace("[sections.turret]", "[sections.rotor]"),
        DEMOLISHER.replace("tons = 80", "tons = 0"),
        DEMOLISHER.replace("walk_mp = 5", "max_speed = nan"),
        DEMOLISHER.replace("walk_mp = 5", "walk_mp = -1"),
        DEMOLISHER.replace("at = \"1-2\"", "at = \"1-13\""),
        DEMOLISHER.replace("at = \"3-6\"", "at = \"2-6\""),
        format!("slots = [{{ at = 1, item = \"IS.MediumLaser\" }}]\n{DEMOLISHER}"),
        format!("name = \"Duplicate\"\n{DEMOLISHER}"),
        format!("future_field = \"preserved\"\n{DEMOLISHER}"),
        format!("{DEMOLISHER}\n[sections.left_side]\narmor = 1\n"),
        format!("specials = [\"unclosed\"\n{DEMOLISHER}"),
        "x".repeat(1_048_577),
    ] {
        assert_ne!(source, DEMOLISHER);
        assert!(VehicleTemplate::parse("Demolisher", &source).is_err());
    }
    assert!(
        VehicleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).is_err()
    );
    let custom = DEMOLISHER
        .replace(
            "walk_mp = 5\n",
            "walk_mp = 5\nspecials = [\"CargoTech\", \"cargotech\"]\n",
        )
        .replace(
            "item = \"IS.AC/20\" }",
            "item = \"IS.AC/20\", modes = [\"Hotload\", \"RearMount\"] }",
        );
    let parsed = VehicleTemplate::parse("Demolisher", &custom).unwrap();
    assert_eq!(parsed.attributes["specials"], "ICEEngine_Tech CargoTech");
    assert_eq!(
        parsed.sections[&VehicleSection::Turret].criticals[&0].modes,
        ["Hotload", "RearMount"]
    );
}

/// Vehicle reads use the same bounded configured-directory access as maps and Mech assets.
#[test]
fn vehicle_asset_reader_is_confined() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("vehicle.toml"), DEMOLISHER).unwrap();
    assert_eq!(
        read_battle_vehicle_template(directory.path(), "vehicle").unwrap(),
        VehicleTemplate::parse("vehicle", DEMOLISHER).unwrap()
    );
    assert!(read_battle_vehicle_template(directory.path(), "../vehicle").is_err());
    assert!(read_battle_vehicle_template(directory.path(), "/etc/passwd").is_err());
}

/// Bundled vehicle ammunition rows retain whitespace-separated loading and half-ton flags.
#[test]
fn vehicle_assets_accept_multiple_ammunition_mode_words() {
    for source in [
        include_str!("../game/mechs/Jeep.toml"),
        include_str!("../game/mechs/HTracked_APC.toml"),
    ] {
        let definition = VehicleTemplate::parse("test", source).unwrap();
        let unit = Vehicle::new(definition).unwrap();
        let loadout = unit.loadout().unwrap();
        let bin = loadout
            .ammunition
            .iter()
            .find(|bin| bin.weapon == Weapon::MachineGun)
            .unwrap();
        assert_eq!(bin.capacity, 100);
        assert!(bin.half_ton);
        assert!(bin.hotload);
        let restored: Vehicle =
            serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
        assert_eq!(unit, restored);
    }
}

/// Bare system entries in existing artillery carriers receive the ordinary empty metadata.
#[test]
fn artillery_vehicle_assets_decode_abbreviated_ecm() {
    for source in [
        include_str!("../game/mechs/Huey.toml"),
        include_str!("../game/mechs/Huitzilopochtli.toml"),
    ] {
        let definition = VehicleTemplate::parse("test", source).unwrap();
        let vehicle = Vehicle::new(definition).unwrap();
        assert!(
            vehicle
                .loadout()
                .unwrap()
                .systems
                .iter()
                .any(|part| part.system == System::Ecm)
        );
    }
}

/// The copied Streak carrier asset records the reference loader's effective full-bin quantity.
#[test]
fn svantovit_streak_records_valid_full_bin_capacity() {
    let definition = VehicleTemplate::parse(
        "Svantovit-Streak",
        include_str!("../game/mechs/Svantovit-Streak.toml"),
    )
    .unwrap();
    let vehicle = Vehicle::new(definition).unwrap();
    let loadout = vehicle.loadout().unwrap();
    let bin = loadout
        .ammunition
        .iter()
        .find(|bin| bin.weapon == Weapon::ClanStreakSrm4)
        .unwrap();
    assert_eq!((bin.rounds, bin.capacity), (25, 25));
    assert!(!bin.half_ton);
}

/// The copied tracked transport has ground anatomy, without the stray aircraft section.
#[test]
fn tracked_transport_asset_has_only_ground_sections() {
    let definition = VehicleTemplate::parse(
        "J-27_Transport",
        include_str!("../game/mechs/J-27_Transport.toml"),
    )
    .unwrap();
    assert_eq!(definition.movement, VehicleMovement::Tracked);
    assert_eq!(definition.sections.len(), 5);
    let vehicle = Vehicle::new(definition).unwrap();
    assert!(!vehicle.is_destroyed());
    assert!(vehicle.loadout().unwrap().weapons.is_empty());
}

/// Rotorcraft retain the same hull slots, weapon definitions and ammunition parser as ground units.
#[test]
fn vtol_assets_share_vehicle_anatomy_and_equipment_without_ground_admission() {
    let mut count = 0;
    for entry in std::fs::read_dir(crate::support::repository_root().join("game/mechs")).unwrap() {
        let path = entry.unwrap().path();
        if !path.is_file() {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        if !source.lines().any(|line| line == "class = \"vtol\"") {
            continue;
        }
        let reference = path.file_stem().unwrap().to_str().unwrap();
        let template = VehicleTemplate::parse(reference, &source)
            .unwrap_or_else(|error| panic!("{}: {error:#}", path.display()));
        assert!(template.is_vtol());
        assert!(matches!(
            template.movement,
            VehicleMovement::Vtol | VehicleMovement::Stationary
        ));
        assert!(template.sections[&VehicleSection::Rotor].internal > 0);
        let loadout = VehicleLoadout::resolve(&template)
            .unwrap_or_else(|error| panic!("{}: {error:#}", path.display()));
        assert!(
            loadout
                .weapons
                .iter()
                .all(|mount| !mount.criticals.is_empty())
        );
        let restored =
            serde_json::from_value::<VehicleTemplate>(serde_json::to_value(&template).unwrap())
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
    let source = include_str!("../game/mechs/Kestrel.toml");
    for invalid in [
        source.replace("class = \"vtol\"", "class = \"vehicle\""),
        source.replace("movement = \"vtol\"", "movement = \"track\""),
        source.replace("[sections.rotor]", "[sections.turret]"),
    ] {
        assert_ne!(invalid, source);
        assert!(
            VehicleTemplate::parse("Kestrel", &invalid).is_err(),
            "{invalid}"
        );
    }
    for extended in [false, true] {
        assert_eq!(
            VehicleMovement::Vtol.piloting_skill(extended),
            Some("Piloting-Aerospace")
        );
    }
    for (tons, suspension) in [(10, 50), (11, 95), (20, 95), (21, 140), (30, 140)] {
        assert_eq!(VehicleMovement::Vtol.suspension(tons), suspension);
    }
    let mut forged = VehicleTemplate::parse("Demolisher", DEMOLISHER).unwrap();
    forged.movement = VehicleMovement::Vtol;
    assert!(Vehicle::new(forged).is_err());
    let template = VehicleTemplate::parse("Kestrel", source).unwrap();
    let engine = template.engine().unwrap();
    assert_eq!(engine.nominal_rating, 300);
    assert_eq!(engine.weight_rating, 160);
    let mass = template.mass().unwrap();
    assert_eq!(mass.components, 2560);
    assert_eq!(mass.cockpit, 1280);
    assert_eq!(mass.engine, engine.installed_mass);
}
