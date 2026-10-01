//! Reference SCAN/REPORT information columns share motion, condition and geometry authorities.
use crate::support;
use crate::support::btech_firing as firing;
use stompymux_rs::*;

/// Every supported chassis uses the same name field, indentation, tabs and read-only native/Lua report.
#[tokio::test]
async fn report_layout_and_names_share_all_chassis() {
    use std::{cell::RefCell, rc::Rc};
    for (template, kind) in firing::templates().into_iter().zip([
        "MECH",
        "MECH",
        "VEHICLE",
        "VEHICLE",
        "VEHICLE",
        "INSTALLATION",
        "VTOL",
    ]) {
        let (_dir, config, mut world, scanner, target, _) =
            firing::fixture_with_target(include_str!("../game/mechs/JR7-D.toml"), None, &template)
                .await;
        firing::edit(&mut world, target, |state| {
            state["display_name"] = "[bold]123456789012345678901234567890123456789".into()
        });
        let before = serde_json::to_value(&world.btech).unwrap();
        let report = report_battle_unit(&world, scanner, ObjectId(1), target).unwrap();
        let plain = text::plain(&report);
        let lines: Vec<_> = plain.lines().collect();
        assert!(
            lines[0].starts_with("[ab]  [bold]1234567890123456789 Tonnage: "),
            "{}",
            lines[0]
        );
        assert_eq!(lines[1], "      Range: 1.0 hex\t\tBearing: 0 degrees");
        assert_eq!(lines[2], "      Speed: 0.0 KPH\t\tHeading: 0 degrees");
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with(&format!("      Type: {kind}")))
        );
        assert!(lines.contains(&"      In Forward Weapons Arc"));
        assert!(
            lines[1..lines.len() - 1]
                .iter()
                .all(|line| line.starts_with("      "))
        );
        assert_eq!(lines.last().copied(), Some(" "));
        let scan = scan_battle_unit(&world, scanner, ObjectId(1), target, "A").unwrap();
        assert!(scan.starts_with(&report));
        assert_eq!(serde_json::to_value(&world.btech).unwrap(), before);
        persistence::save(&config.database(), &world).await.unwrap();
        let scripts = Scripts::new(
            &config,
            Rc::new(RefCell::new(
                persistence::load(&config.database()).await.unwrap(),
            )),
        )
        .unwrap();
        let lua: String = scripts
            .eval_callback(&format!(
                "return btech.unit.report({},1,{})",
                scanner.0, target.0
            ))
            .unwrap();
        assert_eq!(lua, report);
        let native = support::run_text_for_player(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("report #{}", target.0),
        );
        assert_eq!(native, report);
        assert!(scripts.drain_outbox().is_empty());
        assert_eq!(
            serde_json::to_value(&scripts.world().btech).unwrap(),
            before
        );
    }
}

/// Lateral heading belongs to travel; torso controls remain an INFO supplement, not condition leakage.
#[tokio::test]
async fn lateral_and_torso_rows_use_committed_state() {
    let (_dir, _, mut world, scanner, target, _) = firing::fixture_with_target(
        include_str!("../game/mechs/JR7-D.toml"),
        None,
        include_str!("../game/mechs/GOL-1H.toml"),
    )
    .await;
    firing::edit(&mut world, target, |state| {
        state["motion"]["heading"] = 100.into();
        state["motion"]["desired_heading"] = 100.into();
        state["lateral"]["active"] = "front_left".into();
        state["facing"]["torso"] = "both".into();
    });
    let before = serde_json::to_value(&world.btech).unwrap();
    let report = report_battle_unit(&world, scanner, ObjectId(1), target).unwrap();
    assert!(report.contains("      Speed: 0.0 KPH\t\tHeading: 40 degrees"));
    assert!(report.contains("      Mech is moving laterally Front/Left"));
    assert!(!report.contains("Torso is"));
    let info = scan_battle_unit(&world, scanner, ObjectId(1), target, "I").unwrap();
    assert!(info.ends_with("Torso is 60 degrees right\nTorso is 60 degrees left\n "));
    assert_eq!(serde_json::to_value(&world.btech).unwrap(), before);
}

/// Turret offsets retain positive 180, wrap above it, and disappear from fixed-installation reports.
#[tokio::test]
async fn turret_summary_uses_absolute_bearing_and_signed_offset() {
    for template in firing::templates().into_iter().skip(2).take(4) {
        let (_dir, _, world, scanner, target, _) =
            firing::fixture_with_target(include_str!("../game/mechs/JR7-D.toml"), None, &template)
                .await;
        let fixed = world.btech.vehicles()[&target].definition().movement
            == BattleVehicleMovement::Stationary;
        for (offset, absolute, signed) in [
            (0, 100, 0),
            (180, 280, 180),
            (200, 300, -160),
            (300, 40, -60),
        ] {
            let mut world = world.clone();
            firing::edit(&mut world, target, |state| {
                state["motion"]["heading"] = 100.into();
                state["motion"]["desired_heading"] = 100.into();
                state["turret_offset"] = offset.into();
            });
            let report = report_battle_unit(&world, scanner, ObjectId(1), target).unwrap();
            let expected = if fixed || offset == 0 {
                format!("      Turret Facing: {absolute} degrees")
            } else {
                format!("      Turret Facing: {absolute} degrees ({signed} offset from heading)")
            };
            assert!(report.lines().any(|line| line == expected), "{report}");
            let cockpit = battle_unit_status(&world, target, "I").unwrap();
            assert!(cockpit.lines().any(|line| line == expected));
            let info = scan_battle_unit(&world, scanner, ObjectId(1), target, "I").unwrap();
            assert_eq!(info.matches("Turret Facing:").count(), 1);
        }
    }
}

/// Public condition banners retain styling and order without exposing the cockpit's ECM state.
#[tokio::test]
async fn scan_conditions_share_state_without_cockpit_only_flags() {
    for template in [
        include_str!("../game/mechs/JR7-D.toml"),
        include_str!("../game/mechs/Demolisher.toml"),
    ] {
        let (_dir, _, mut world, scanner, target, _) =
            firing::fixture_with_target(include_str!("../game/mechs/JR7-D.toml"), None, template)
                .await;
        firing::edit(&mut world, target, |state| {
            state["fortified"] = true.into();
            state["weapons_hold"] = true.into();
            state["electronics"]["field"]["protected"] = true.into();
            state["power"] = serde_json::to_value(BattlePower::Off).unwrap();
        });
        let before = serde_json::to_value(&world.btech).unwrap();
        let report = report_battle_unit(&world, scanner, ObjectId(1), target).unwrap();
        assert!(report.contains(
            "      [fg=green bold]FORTIFIED[reset]\r\n      [fg=red bold]WEAPONS HOLD[reset]"
        ));
        assert!(report.ends_with("      SHUTDOWN\r\n "));
        assert!(!report.contains("PROTECTED BY ECM"));
        assert!(
            battle_unit_status(&world, target, "A")
                .unwrap()
                .contains("PROTECTED BY ECM")
        );
        assert_eq!(serde_json::to_value(&world.btech).unwrap(), before);
    }
}

/// VTOL vertical motion and an observer's turret arc do not replace its ordinary hull arc.
#[tokio::test]
async fn vtol_report_includes_vertical_speed_and_both_observer_arcs() {
    let (_dir, _, mut world, scanner, target, _) = firing::fixture_with_target(
        include_str!("../game/mechs/Demolisher.toml"),
        None,
        include_str!("../game/mechs/Kestrel.toml"),
    )
    .await;
    firing::edit(&mut world, target, |state| {
        state["vtol_flight"]["phase"] = serde_json::json!({"kind":"airborne"});
        state["vtol_flight"]["altitude"] = 12.into();
        state["vtol_flight"]["vertical_speed"] = (-2).into();
    });
    let before = serde_json::to_value(&world.btech).unwrap();
    let report = report_battle_unit(&world, scanner, ObjectId(1), target).unwrap();
    assert!(report.contains("      Vertical speed: -2.0 KPH"));
    assert!(report.contains("      Type: VTOL                Movement: VTOL"));
    assert!(report.contains("      In Turret Arc\r\n      In Forward Weapons Arc"));
    assert_eq!(serde_json::to_value(&world.btech).unwrap(), before);
}

/// Jump heading comes from the committed route without advancing its cursor.
#[tokio::test]
async fn report_shows_live_jump_heading_without_advancing_flight() {
    let (_dir, _, mut world, scanner, target, _) = firing::fixture_with_target(
        include_str!("../game/mechs/JR7-D.toml"),
        None,
        include_str!("../game/mechs/JR7-D.toml"),
    )
    .await;
    let path = BattleJumpPath::new(
        BattleHexCoordinate { x: 0, y: 10 }.center(),
        BattleHexCoordinate { x: 0, y: 8 }.center(),
        0,
        0,
        5,
    )
    .unwrap();
    firing::edit(&mut world, target, |state| {
        state["flight"] = serde_json::to_value(BattleJumpFlight::new(path)).unwrap()
    });
    let before = serde_json::to_value(&world.btech).unwrap();
    let report = report_battle_unit(&world, scanner, ObjectId(1), target).unwrap();
    assert!(report.ends_with("      Mech is Jumping!\tJump Heading: 0\r\n "));
    assert_eq!(serde_json::to_value(&world.btech).unwrap(), before);
}

/// INFO reports the existing tow relationship from the carrier's view without adding it to plain REPORT.
#[tokio::test]
async fn scan_info_towing_uses_shared_relationship_and_literal_names() {
    for (carrier, towed_template, name) in [
        (
            include_str!("../game/mechs/JR7-D.toml"),
            include_str!("../game/mechs/Demolisher.toml"),
            "Demolisher",
        ),
        (
            include_str!("../game/mechs/Demolisher.toml"),
            include_str!("../game/mechs/JR7-D.toml"),
            "Jenner",
        ),
    ] {
        let (_dir, config, mut world, scanner, target, _) =
            firing::fixture_with_target(include_str!("../game/mechs/JR7-D.toml"), None, carrier)
                .await;
        let tow = world.create(&config, "Tow".into(), Kind::Thing);
        world.objects.get_mut(&tow).unwrap().home = Some(ObjectId(config.home()));
        BattleUnitTemplate::parse("test", towed_template)
            .unwrap()
            .create(&mut world, tow)
            .unwrap();
        let map = world.btech.units()[&target].map.unwrap();
        place_battle_unit(&mut world, tow, map, 0, 10).unwrap();
        set_battle_tow(&mut world, target, Some(tow)).unwrap();
        firing::edit(&mut world, tow, |state| {
            state["display_name"] = format!("[bold]{name}").into()
        });
        firing::edit(&mut world, target, |state| {
            state["contacts"][tow.0.to_string()] = serde_json::json!({"identified":true});
        });
        let before = serde_json::to_value(&world.btech).unwrap();
        let report = report_battle_unit(&world, scanner, ObjectId(1), target).unwrap();
        assert!(!report.contains("Towing"));
        let info = scan_battle_unit(&world, scanner, ObjectId(1), target, "I").unwrap();
        assert!(
            text::plain(&info).ends_with(&format!("Towing [bold]{name} [ac].\n ")),
            "{info}"
        );
        assert_eq!(serde_json::to_value(&world.btech).unwrap(), before);
        world.validate(&config).unwrap();
    }
}
