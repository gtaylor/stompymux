//! Ground-vehicle engine catalogue lookup, technology ordering and hover minimums.
use stompymux_rs::*;

/// A tracked 80-ton chassis with three walking movement points has a 240-rating powerplant.
fn tracked(flags: &str) -> BattleVehicleTemplate {
    let mut template =
        BattleVehicleTemplate::parse("Demolisher",include_str!("../game/mechs/Demolisher.toml")).unwrap();
    template.attributes.insert("specials".into(), flags.into());
    template
}

#[test]
fn vehicle_engine_shielding_precedes_family_rounding() {
    for (flags, powerplant, half_tons) in [
        ("ICEEngine_Tech", BattleVehiclePowerplant::Combustion, 46),
        (
            "-",
            BattleVehiclePowerplant::Fusion(BattleEngine::Standard),
            35,
        ),
        (
            "XLEngine_Tech",
            BattleVehiclePowerplant::Fusion(BattleEngine::Xl),
            18,
        ),
        (
            "XXL_Tech",
            BattleVehiclePowerplant::Fusion(BattleEngine::Xxl),
            12,
        ),
        (
            "LightEngine_Tech",
            BattleVehiclePowerplant::Fusion(BattleEngine::Light),
            27,
        ),
        (
            "CompactEngine_Tech",
            BattleVehiclePowerplant::Fusion(BattleEngine::Compact),
            53,
        ),
        (
            "ICEEngine_Tech XLEngine_Tech",
            BattleVehiclePowerplant::Combustion,
            46,
        ),
        (
            "XLEngine_Tech XXL_Tech LightEngine_Tech",
            BattleVehiclePowerplant::Fusion(BattleEngine::Xl),
            18,
        ),
    ] {
        let template = tracked(flags);
        let before = template.clone();
        let report = template.engine().unwrap();
        assert_eq!(report.powerplant, powerplant);
        assert_eq!(report.nominal_rating, 240);
        assert_eq!(report.weight_rating, 240);
        assert_eq!(report.standard_mass, Some(23 * 512));
        assert_eq!(report.installed_mass, half_tons * 512, "{flags}");
        assert_eq!(report.hover_minimum, 0);
        assert_eq!(template, before);
        let restored: BattleVehicleTemplate =
            serde_json::from_value(serde_json::to_value(template).unwrap()).unwrap();
        assert_eq!(restored.engine().unwrap(), report);
    }
}

#[test]
fn vehicle_suspension_and_missing_engine_diagnostics() {
    for (tons, expected) in [
        (1, 40),
        (10, 40),
        (11, 85),
        (20, 85),
        (21, 130),
        (30, 130),
        (31, 175),
        (40, 175),
        (41, 235),
        (100, 235),
    ] {
        assert_eq!(BattleVehicleMovement::Hover.suspension(tons), expected);
        assert_eq!(BattleVehicleMovement::Wheeled.suspension(tons), 20);
        assert_eq!(BattleVehicleMovement::Tracked.suspension(tons), 0);
    }
    let truck = BattleVehicleTemplate::parse("Flatbed_Truck",include_str!("../game/mechs/Flatbed_Truck.toml"))
        .unwrap()
        .engine()
        .unwrap();
    assert_eq!((truck.nominal_rating, truck.weight_rating), (50, 30));
    assert_eq!(truck.installed_mass, 2 * 1024);
    let hover = BattleVehicleTemplate::parse("Fulcrum",include_str!("../game/mechs/Fulcrum.toml"))
        .unwrap()
        .engine()
        .unwrap();
    assert_eq!((hover.nominal_rating, hover.weight_rating), (500, 265));
    assert_eq!(hover.engine_mass, 21 * 512);
    assert_eq!(hover.hover_minimum, 10 * 1024);
    let mut template = tracked("-");
    template.movement = BattleVehicleMovement::Hover;
    template.tons = 5;
    template.max_speed = 16.125;
    let engine = template.engine().unwrap();
    assert_eq!(engine.weight_rating, -35);
    assert_eq!(engine.standard_mass, None);
    assert_eq!(engine.engine_mass, 0);
    assert_eq!(engine.installed_mass, 1024);
    template.movement = BattleVehicleMovement::Tracked;
    template.tons = 11;
    assert_eq!(template.engine().unwrap().standard_mass, None);
    assert_eq!(template.engine().unwrap().installed_mass, 0);
    for speed in [f64::NAN, f64::INFINITY, -1.0, f64::MAX] {
        template.max_speed = speed;
        assert!(template.engine().is_err());
    }
}
