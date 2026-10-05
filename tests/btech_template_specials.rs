//! Technology flag lists merge repeats without relaxing duplicate scalar or section validation.
use stompymux_rs::*;
const SOURCE: &str = include_str!("fixtures/btech/units/JR7-D.toml");

/// Feature flags form a case-insensitive union in their canonical spelling, after the flags
/// construction choices set.
#[test]
fn template_specials_merge_canonically_after_construction_flags() {
    let source = format!(
        "specials = [\"searchlight\", \"CargoTech\", \"SEARCHLIGHT\"]\n{}",
        SOURCE.replace(
            "\n[sections.left_arm]",
            "\n[construction]\ngyro = \"heavy_duty\"\n\n[sections.left_arm]",
        )
    );
    let parsed = MechTemplate::parse("JR7-D", &source).unwrap();
    assert_eq!(
        parsed.attributes["specials"],
        "HDGyro_Tech SearchLight CargoTech FlipArms"
    );
    let unit = Mech::from_template(parsed).unwrap();
    assert_eq!(unit.gyro(), Gyro::Hardened);
    for invalid in [
        format!("specials = [\"UnknownTechnology\"]\n{SOURCE}"),
        format!("specials = [\"HDGYRO\"]\n{SOURCE}"),
        format!("specials = [\"FlipArms\"]\n{SOURCE}"),
        format!("tons = 35\n{SOURCE}"),
        format!("{SOURCE}\n[sections.left_arm]\n"),
    ] {
        assert!(MechTemplate::parse("JR7-D", &invalid).is_err(), "{invalid}");
    }
}

/// An empty technology list keeps the ordinary no-specials representation.
#[test]
fn only_empty_specials_construct_normally() {
    let parsed = MechTemplate::parse("JR7-D", &format!("specials = []\n{SOURCE}")).unwrap();
    let plain = MechTemplate::parse("JR7-D", SOURCE).unwrap();
    assert_eq!(
        parsed.attributes.get("specials"),
        plain.attributes.get("specials")
    );
    assert!(check_battle_template(&parsed).constructible);
}
