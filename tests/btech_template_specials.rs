//! Technology flag lists merge repeats without relaxing duplicate scalar or section validation.
use stompymux_rs::*;
const SOURCE: &str = include_str!("fixtures/btech/mechs/JR7-D.toml");

/// Technology flags form a case-insensitive union, preserving first occurrence spelling and order.
#[test]
fn repeated_template_specials_accumulate_and_keep_unknown_flags_visible() {
    let source = SOURCE.replace(
        "specials = [\"FlipArms\"]",
        "specials = [\"FlipArms\", \"HDGYRO\", \"fliparms\", \"hdgyro\"]",
    );
    assert_ne!(source, SOURCE);
    let parsed = BattleTemplate::parse("JR7-D", &source).unwrap();
    assert_eq!(parsed.attributes["specials"], "FlipArms HDGYRO");
    let unit = BattleUnit::from_template(parsed).unwrap();
    assert_eq!(unit.gyro(), BattleGyro::Hardened);
    let unsupported = BattleTemplate::parse(
        "JR7-D",
        &source.replace("\"hdgyro\"]", "\"hdgyro\", \"UnknownTechnology\"]"),
    )
    .unwrap();
    assert!(unsupported.attributes["specials"].contains("UnknownTechnology"));
    assert!(!check_battle_template(&unsupported).constructible);
    assert!(BattleTemplate::parse("JR7-D", &format!("tons = 35\n{SOURCE}")).is_err());
    assert!(BattleTemplate::parse("JR7-D", &format!("{SOURCE}\n[sections.left_arm]\n")).is_err());
}

/// An empty technology list keeps the ordinary no-specials representation.
#[test]
fn only_empty_specials_construct_normally() {
    let source = SOURCE.replace("specials = [\"FlipArms\"]", "specials = []");
    assert_ne!(source, SOURCE);
    let parsed = BattleTemplate::parse("JR7-D", &source).unwrap();
    let plain =
        BattleTemplate::parse("JR7-D", &SOURCE.replace("specials = [\"FlipArms\"]\n", "")).unwrap();
    assert_eq!(
        parsed.attributes.get("specials"),
        plain.attributes.get("specials")
    );
    assert!(check_battle_template(&parsed).constructible);
}
