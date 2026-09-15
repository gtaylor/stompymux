//! Repeated technology fields accumulate without relaxing duplicate scalar or section validation.
use stompymux_rs::*;
const SOURCE: &str = include_str!("fixtures/btech/mechs/JR7-D");

/// Technology flags form a case-insensitive union, preserving first occurrence spelling and order.
#[test]
fn repeated_template_specials_accumulate_and_keep_unknown_flags_visible() {
    let source = format!(
        "{SOURCE}\nSpecials {{ HDGYRO }}\nSPECIALS {{ fliparms hdgyro - }}\nSpecials {{ - }}\n"
    );
    let parsed = BattleTemplate::parse(&source).unwrap();
    assert_eq!(parsed.attributes["specials"], "FlipArms HDGYRO");
    let unit = BattleUnit::from_template(parsed).unwrap();
    assert_eq!(unit.gyro(), BattleGyro::Hardened);
    let unsupported =
        BattleTemplate::parse(&format!("{source}\nSpecials {{ UnknownTechnology }}")).unwrap();
    assert!(unsupported.attributes["specials"].contains("UnknownTechnology"));
    assert!(!check_battle_template(&unsupported).constructible);
    assert!(BattleTemplate::parse(&format!("{SOURCE}\nTons {{ 35 }}")).is_err());
    assert!(BattleTemplate::parse(&format!("{SOURCE}\nLeft_Arm\n")).is_err());
}

/// Empty technology records do not erase flags before or after an empty record.
#[test]
fn empty_specials_records_are_neutral() {
    let source = SOURCE.replace(
        "Specials\t { FlipArms }",
        "Specials { - }\nSpecials { }\nSpecials { FlipArms }",
    );
    assert_ne!(source, SOURCE);
    let parsed = BattleTemplate::parse(&source).unwrap();
    assert_eq!(parsed.attributes["specials"], "FlipArms");
    assert!(check_battle_template(&parsed).constructible);
}

/// A template with only empty technology records keeps the ordinary no-specials representation.
#[test]
fn only_empty_specials_construct_normally() {
    let source = SOURCE.replace("FlipArms", "-");
    let parsed = BattleTemplate::parse(&format!("{source}\nSpecials {{ }}")).unwrap();
    assert_eq!(parsed.attributes["specials"], "-");
    assert!(check_battle_template(&parsed).constructible);
}
