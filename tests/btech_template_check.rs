//! Construction diagnostics, normalization previews and read-only native/Lua inspection.
use crate::support;
use stompymux_rs::*;

/// Template checks share construction rejection and retain the original source facts.
#[test]
fn template_check_reports_normalization_and_construction_failures() {
    let source =
        include_str!("fixtures/btech/mechs/JR7-D.toml").replace("Ammo_IS.SRM-4 25", "Ammo_IS.SRM-4 7");
    let template = BattleTemplate::parse("test",&source).unwrap();
    let original = template.clone();
    let report = check_battle_template(&template);
    assert!(report.constructible);
    assert!(report.rejection.is_none());
    assert_eq!((report.weapons, report.ammunition_bins), (5, 1));
    assert_eq!(
        report.ammunition_adjustments,
        [BattleAmmunitionAdjustment {
            location: CriticalLocation {
                section: BattleSection::RightTorso,
                slot: 0
            },
            supplied: 7,
            normalized: 12,
            inferred_half_ton: true,
        }]
    );
    assert_eq!(template, original);
    for invalid in [
        source.replace("IS.MediumLaser", "IS.Unknown"),
        source.replace("Gyro", "HeatSink"),
        source.replace("FlipArms", "UnknownTechnology"),
    ] {
        let template = BattleTemplate::parse("test",&invalid).unwrap();
        let expected = BattleUnit::from_template(template.clone()).unwrap_err();
        let report = check_battle_template(&template);
        assert!(!report.constructible);
        assert_eq!(report.rejection, Some(format!("{expected:#}")));
        assert!(report.ammunition_adjustments.is_empty());
    }
}

/// Operators and scripts see the same detached diagnostics without registration, notifications or writes.
#[tokio::test(flavor = "current_thread")]
async fn template_check_native_lua_read_only_and_bounded_asset_access() {
    let (dir, config, world) = support::isolated_world().await;
    std::fs::create_dir_all(dir.path().join("mechs")).unwrap();
    let source =
        include_str!("fixtures/btech/mechs/JR7-D.toml").replace("Ammo_IS.SRM-4 25", "Ammo_IS.SRM-4 7");
    std::fs::write(dir.path().join("mechs/partial.toml"), &source).unwrap();
    std::fs::write(
        dir.path().join("mechs/unsupported.toml"),
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
        "@btech template-check partial",
    );
    assert!(
        text.contains("constructible under current biped rules"),
        "{text}"
    );
    assert!(text.contains("7 -> 12 (inferred half-ton bin)"), "{text}");
    let report: mlua::Table = scripts
        .eval_callback("return btech.template.check('partial')")
        .unwrap();
    assert!(report.get::<bool>("constructible").unwrap());
    let adjustment: mlua::Table = report
        .get::<mlua::Table>("ammunition_adjustments")
        .unwrap()
        .get(1)
        .unwrap();
    assert_eq!(adjustment.get::<u16>("normalized").unwrap(), 12);
    report.set("constructible", false).unwrap();
    assert!(
        scripts
            .eval_callback::<bool>("return btech.template.check('partial').constructible")
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
    for invalid in ["../partial", "missing"] {
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
        "@btech template-check partial",
    );
    assert!(!denied.contains("constructible under current biped rules"));
    assert_eq!(scripts.world().btech, world.btech);
    assert_eq!(scripts.world().objects.len(), world.objects.len());
    assert!(scripts.drain_outbox().is_empty());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("mechs/partial.toml")).unwrap(),
        source
    );
}

/// Construction-time failures identify the exact bin even before normal loadout validation.
#[test]
fn ammunition_rejections_include_location_equipment_and_flags() {
    let original = BattleTemplate::parse("JR7-D",include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
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
            .get_mut(&BattleSection::RightTorso)
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
    let mut template = BattleTemplate::parse("JR7-D",include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
    template
        .sections
        .get_mut(&BattleSection::RightTorso)
        .unwrap()
        .criticals
        .get_mut(&0)
        .unwrap()
        .modes = vec!["Narc/Smoke".into()];
    assert!(check_battle_template(&template).constructible);
    let unit = stompymux_rs::BattleUnit::from_template(template).unwrap();
    assert_eq!(
        unit.loadout().unwrap().ammunition[0].mode,
        stompymux_rs::BattleAmmunitionMode::Narc
    );
}

/// Quad assets decode and construct with four load-bearing legs.
#[test]
fn quad_anatomy_decodes_and_constructs_assets() {
    let template = BattleTemplate::parse("SCP-1N",include_str!("../game/mechs/SCP-1N.toml")).unwrap();
    let chassis = template.chassis().unwrap();
    assert_eq!(chassis, BattleMechChassis::Quad);
    assert_eq!(chassis.legs().len(), 4);
    for (section, name) in [
        (BattleSection::LeftArm, "Front_Left_Leg"),
        (BattleSection::RightArm, "Front_Right_Leg"),
        (BattleSection::LeftLeg, "Rear_Left_Leg"),
        (BattleSection::RightLeg, "Rear_Right_Leg"),
    ] {
        assert!(chassis.is_leg(section));
        assert_eq!(chassis.section_name(section), name);
        assert_eq!(chassis.parse_section(name).unwrap(), section);
        assert_eq!(chassis.critical_slots(section), 6);
        assert_eq!(template.sections[&section].internal, 13);
    }
    assert_eq!(chassis.critical_slots(BattleSection::CenterTorso), 12);
    assert_eq!(chassis.critical_slots(BattleSection::Head), 6);
    let loadout = BattleLoadout::resolve(&template).unwrap();
    assert_eq!(loadout.weapons.len(), 2);
    let report = check_battle_template(&template);
    assert_eq!(report.chassis, Some(chassis));
    assert!(report.constructible, "{:?}", report.rejection);
    let saved: BattleTemplate =
        serde_json::from_value(serde_json::to_value(&template).unwrap()).unwrap();
    assert_eq!(saved, template);
    assert_eq!(saved.chassis().unwrap(), chassis);
}

/// Chassis fields can follow sections, but mixed, duplicate or missing limb headings are rejected.
#[test]
fn chassis_section_parsing_is_order_independent_and_rejects_mixed_anatomy() {
    let source = include_str!("../game/mechs/SCP-1N.toml");
    let movement = source
        .lines()
        .find(|line| line.starts_with("Move_Type"))
        .unwrap();
    let reordered = format!("{}\n{movement}\n", source.replace(movement, ""));
    assert_eq!(
        BattleTemplate::parse("test",&reordered).unwrap(),
        BattleTemplate::parse("test",source).unwrap()
    );
    for bad in [
        source.replace("Front_Left_Leg", "Left_Arm"),
        source.replace("Rear_Right_Leg", "Rear_Left_Leg"),
        source.replace("Quad", "Biped"),
    ] {
        assert!(BattleTemplate::parse("test",&bad).is_err());
    }
    let biped = BattleTemplate::parse("JR7-D",include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
    let chassis = biped.chassis().unwrap();
    assert_eq!(chassis, BattleMechChassis::Biped);
    assert!(!chassis.is_leg(BattleSection::LeftArm));
    assert_eq!(chassis.critical_slots(BattleSection::LeftArm), 12);
    assert!(chassis.parse_section("Front_Left_Leg").is_err());
    assert!(BattleMechChassis::Quad.parse_section("Left_Arm").is_err());
    assert!(BattleMechChassis::parse("Tracked").is_err());
}

/// Command selectors follow chassis labels; template parsing remains stricter.
#[test]
fn chassis_command_locations_round_trip_labels_and_short_names() {
    for (chassis, shorts) in [
        (
            BattleMechChassis::Biped,
            ["LA", "RA", "LT", "RT", "CT", "LL", "RL", "HD"],
        ),
        (
            BattleMechChassis::Quad,
            ["FLL", "FRL", "LT", "RT", "CT", "RLL", "RRL", "H"],
        ),
    ] {
        for (section, short) in BattleSection::ALL.into_iter().zip(shorts) {
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
        assert!(BattleMechChassis::Quad.parse_location(short).is_ok());
        assert!(BattleMechChassis::Biped.parse_location(short).is_err());
    }
    assert!(BattleMechChassis::Quad.parse_location("Left_Arm").is_err());
    assert!(BattleMechChassis::Quad.parse_location("LL").is_err());
    assert!(BattleMechChassis::Quad.parse_section("FLL").is_err());
}
