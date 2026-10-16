//! Construction diagnostics and read-only native/Lua template inspection.
use crate::support;
use stompymux_rs::*;

/// Template checks share construction rejection and leave the template unchanged. A machine
/// gun bin must hold its full lot: a short one is refused rather than refilled or taken for a
/// half-ton lot (TechManual p. 57).
#[test]
fn template_check_reports_construction_failures() {
    let jenner = include_str!("fixtures/btech/units/JR7-D.toml");
    let original = MechTemplate::parse("JR7-D", jenner).unwrap();
    let report = check_battle_template(&original);
    assert!(report.constructible);
    assert!(report.rejection.is_none());
    assert_eq!((report.weapons, report.ammunition_bins), (5, 1));
    let short = MechTemplate::parse(
        "JR7-D",
        &jenner.replace(
            "item = \"Ammo_IS.SRM-4\", rounds = 25",
            "item = \"Ammo_IS.MachineGun\", rounds = 100",
        ),
    )
    .unwrap();
    let unknown_weapon = jenner.replace("IS.MediumLaser", "IS.Unknown");
    assert_ne!(unknown_weapon, jenner);
    let mut missing_gyro = original.clone();
    for critical in missing_gyro
        .sections
        .get_mut(&MechSection::CenterTorso)
        .unwrap()
        .criticals
        .values_mut()
        .filter(|critical| critical.equipment == "Gyro")
    {
        critical.equipment = "HeatSink".into();
    }
    let mut unknown_technology = original.clone();
    unknown_technology
        .attributes
        .insert("specials".into(), "UnknownTechnology".into());
    for (template, expected) in [
        (
            short,
            "lists 100 rounds; a machine gun bin holds 200 rounds a ton, or 100 in a half-ton lot marked Halfton",
        ),
        (
            MechTemplate::parse("JR7-D", &unknown_weapon).unwrap(),
            "Unsupported weapon IS.Unknown",
        ),
        (missing_gyro, "Incomplete conventional Gyro system"),
        (unknown_technology, "Unsupported chassis specials"),
    ] {
        let before = template.clone();
        let refused = Mech::from_template(template.clone()).unwrap_err();
        let report = check_battle_template(&template);
        assert!(!report.constructible);
        assert_eq!(report.rejection, Some(format!("{refused:#}")));
        assert!(format!("{refused:#}").contains(expected), "{refused:#}");
        assert_eq!(report.ammunition_bins, 0);
        assert_eq!(template, before);
    }
}

/// Operators and scripts see the same detached diagnostics without registration, notifications or writes.
#[tokio::test(flavor = "current_thread")]
async fn template_check_native_lua_read_only_and_bounded_asset_access() {
    let (dir, config, world) = support::isolated_world().await;
    std::fs::create_dir_all(dir.path().join("units")).unwrap();
    let source = include_str!("fixtures/btech/units/JR7-D.toml");
    std::fs::write(dir.path().join("units/jenner.toml"), source).unwrap();
    std::fs::write(
        dir.path().join("units/unsupported.toml"),
        source.replace("IS.MediumLaser", "IS.Unknown"),
    )
    .unwrap();
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let text = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        "@btech template-check jenner",
    );
    assert!(
        text.contains("constructible under current biped rules"),
        "{text}"
    );
    let report: mlua::Table = scripts
        .eval_callback("return btech.template.check('jenner')")
        .unwrap();
    assert!(report.get::<bool>("constructible").unwrap());
    assert_eq!(report.get::<u32>("ammunition_bins").unwrap(), 1);
    report.set("constructible", false).unwrap();
    assert!(
        scripts
            .eval_callback::<bool>("return btech.template.check('jenner').constructible")
            .unwrap()
    );
    let rejected: mlua::Table = scripts
        .eval_callback("return btech.template.check('unsupported')")
        .unwrap();
    assert!(!rejected.get::<bool>("constructible").unwrap());
    let reason: String = rejected.get("rejection").unwrap();
    let text = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        "@btech template-check unsupported",
    );
    assert!(text.contains(&reason), "{text}");
    for invalid in ["../jenner", "missing"] {
        assert!(
            scripts
                .eval_callback::<mlua::Table>(&format!("return btech.template.check('{invalid}')"))
                .is_err()
        );
    }
    let denied = support::run_text(
        &scripts,
        &config,
        ObjectId(4),
        1,
        "@btech template-check jenner",
    );
    assert!(!denied.contains("constructible under current biped rules"));
    assert_eq!(scripts.world().btech, world.btech);
    assert_eq!(scripts.world().objects.len(), world.objects.len());
    assert!(scripts.drain_outbox().is_empty());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("units/jenner.toml")).unwrap(),
        source
    );
}

/// Construction-time failures identify the exact bin even before normal loadout validation.
#[test]
fn ammunition_rejections_include_location_equipment_and_flags() {
    let original =
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap();
    for (equipment, data, flags, expected) in [
        (
            "Ammo_IS.SRM-4",
            "25",
            vec!["AP"],
            "Unsupported ammunition mode [\"AP\"] for IS.SRM-4",
        ),
        (
            "Ammo_IS.Unknown",
            "25",
            vec![],
            "Unsupported weapon IS.Unknown",
        ),
        (
            "Ammo_IS.SRM-4",
            "invalid",
            vec![],
            "Invalid ammunition count",
        ),
    ] {
        let mut template = original.clone();
        let bin = template
            .sections
            .get_mut(&MechSection::RightTorso)
            .unwrap()
            .criticals
            .get_mut(&0)
            .unwrap();
        bin.equipment = equipment.into();
        bin.data = data.into();
        bin.modes = flags.into_iter().map(str::to_owned).collect();
        let before = template.clone();
        let report = check_battle_template(&template);
        assert!(!report.constructible);
        let rejection = report.rejection.unwrap();
        assert!(
            rejection.contains(&format!("Right_Torso critical 1 ({equipment})")),
            "{rejection}"
        );
        assert!(rejection.contains(expected), "{rejection}");
        assert_eq!(template, before);
    }
}

/// Narc-compatible SRM bins are constructible and retain their selected ammunition type.
#[test]
fn narc_ammunition_is_constructible() {
    let mut template =
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap();
    template
        .sections
        .get_mut(&MechSection::RightTorso)
        .unwrap()
        .criticals
        .get_mut(&0)
        .unwrap()
        .modes = vec!["Narc/Smoke".into()];
    assert!(check_battle_template(&template).constructible);
    let unit = stompymux_rs::Mech::from_template(template).unwrap();
    assert_eq!(
        unit.loadout().unwrap().ammunition[0].mode,
        stompymux_rs::AmmunitionMode::Narc
    );
}

/// Quad assets decode and construct with four load-bearing legs.
#[test]
fn quad_anatomy_decodes_and_constructs_assets() {
    let template =
        MechTemplate::parse("SCP-1N", include_str!("../game/units/SCP-1N.toml")).unwrap();
    let chassis = template.chassis().unwrap();
    assert_eq!(chassis, MechChassis::Quad);
    assert_eq!(chassis.legs().len(), 4);
    for (section, name) in [
        (MechSection::LeftArm, "Front_Left_Leg"),
        (MechSection::RightArm, "Front_Right_Leg"),
        (MechSection::LeftLeg, "Rear_Left_Leg"),
        (MechSection::RightLeg, "Rear_Right_Leg"),
    ] {
        assert!(chassis.is_leg(section));
        assert_eq!(chassis.section_name(section), name);
        assert_eq!(chassis.parse_section(name).unwrap(), section);
        assert_eq!(chassis.critical_slots(section), 6);
        assert_eq!(template.sections[&section].internal, 13);
    }
    assert_eq!(chassis.critical_slots(MechSection::CenterTorso), 12);
    assert_eq!(chassis.critical_slots(MechSection::Head), 6);
    let loadout = MechLoadout::resolve(&template).unwrap();
    assert_eq!(loadout.weapons.len(), 2);
    let report = check_battle_template(&template);
    assert_eq!(report.chassis, Some(chassis));
    assert!(report.constructible, "{:?}", report.rejection);
    let saved: MechTemplate =
        serde_json::from_value(serde_json::to_value(&template).unwrap()).unwrap();
    assert_eq!(saved, template);
    assert_eq!(saved.chassis().unwrap(), chassis);
}

/// Chassis fields and sections can appear in any order, but mixed, duplicate or missing limb tables are rejected.
#[test]
fn chassis_section_parsing_is_order_independent_and_rejects_mixed_anatomy() {
    let source = include_str!("../game/units/SCP-1N.toml");
    let movement = "movement = \"quad\"\n";
    let (fields, sections) = source.split_at(source.find("[sections.").unwrap());
    let mut sections: Vec<_> = sections.trim_end().split("\n\n").collect();
    sections.reverse();
    let reordered = format!(
        "{movement}{}{}\n",
        fields.replacen(movement, "", 1),
        sections.join("\n\n")
    );
    assert_ne!(reordered, source);
    assert_eq!(
        MechTemplate::parse("SCP-1N", &reordered).unwrap(),
        MechTemplate::parse("SCP-1N", source).unwrap()
    );
    for bad in [
        source.replace("[sections.front_left_leg]", "[sections.left_arm]"),
        source.replace("[sections.rear_right_leg]", "[sections.rear_left_leg]"),
        source.replace("[sections.rear_right_leg]", "[sections.right_leg]"),
        source.replace("movement = \"quad\"", "movement = \"biped\""),
    ] {
        assert_ne!(bad, source);
        assert!(MechTemplate::parse("SCP-1N", &bad).is_err());
    }
    let biped =
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap();
    let chassis = biped.chassis().unwrap();
    assert_eq!(chassis, MechChassis::Biped);
    assert!(!chassis.is_leg(MechSection::LeftArm));
    assert_eq!(chassis.critical_slots(MechSection::LeftArm), 12);
    assert!(chassis.parse_section("Front_Left_Leg").is_err());
    assert!(MechChassis::Quad.parse_section("Left_Arm").is_err());
    assert!(MechChassis::parse("Tracked").is_err());
}

/// Command selectors follow chassis labels; template parsing remains stricter.
#[test]
fn chassis_command_locations_round_trip_labels_and_short_names() {
    for (chassis, shorts) in [
        (
            MechChassis::Biped,
            ["LA", "RA", "LT", "RT", "CT", "LL", "RL", "HD"],
        ),
        (
            MechChassis::Quad,
            ["FLL", "FRL", "LT", "RT", "CT", "RLL", "RRL", "H"],
        ),
    ] {
        for (section, short) in MechSection::ALL.into_iter().zip(shorts) {
            assert_eq!(chassis.parse_location(short).unwrap(), section);
            let name = chassis.section_name(section);
            assert_eq!(chassis.parse_location(name).unwrap(), section);
            assert_eq!(
                chassis.parse_location(&name.replace('_', " ")).unwrap(),
                section
            );
        }
    }
    for short in ["FLLEG", "FRLEG", "RLLEG", "RRLEG"] {
        assert!(MechChassis::Quad.parse_location(short).is_ok());
        assert!(MechChassis::Biped.parse_location(short).is_err());
    }
    assert!(MechChassis::Quad.parse_location("Left_Arm").is_err());
    assert!(MechChassis::Quad.parse_location("LL").is_err());
    assert!(MechChassis::Quad.parse_section("FLL").is_err());
}

/// Vehicle templates go through the TechManual combat vehicle check, which reports what the
/// design spends against its limits or the rule it breaks.
#[tokio::test(flavor = "current_thread")]
async fn template_check_reports_vehicle_construction() {
    let (dir, config, world) = support::isolated_world().await;
    std::fs::create_dir_all(dir.path().join("units")).unwrap();
    let tank = include_str!("../game/units/Demolisher.toml");
    std::fs::write(dir.path().join("units/tank.toml"), tank).unwrap();
    std::fs::write(
        dir.path().join("units/heavy.toml"),
        tank.replace(
            "[sections.aft_side]\narmor = 20",
            "[sections.aft_side]\narmor = 28",
        ),
    )
    .unwrap();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let text = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        "@btech template-check tank",
    );
    assert!(
        text.contains(
            "Demolisher (tank): constructible combat vehicle; 2 weapons, 4 ammunition bins."
        ),
        "{text}"
    );
    assert!(
        text.contains(
            "Mass 80 tons; item slots 3 of 21; armor 160 of 320 points; energy weapon heat 0."
        ),
        "{text}"
    );
    let text = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        "@btech template-check heavy",
    );
    assert!(
        text.contains("not constructible: Components weigh 80.5 tons, more than the vehicle's 80 tons (TechManual p. 97)"),
        "{text}"
    );
    let report = check_battle_vehicle_template(&VehicleTemplate::parse("tank", tank).unwrap());
    assert!(report.constructible);
    assert_eq!(
        report.construction.unwrap().maximum_armor,
        vehicle_maximum_armor(80)
    );
}
