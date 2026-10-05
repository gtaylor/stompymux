//! TOML `#` comments in template documents are ignored without changing equipment or field validation.
use crate::support;
use stompymux_rs::*;

/// Surround a template document with `#` comments, including one inside the given section.
fn commented(source: &str, section: &str) -> String {
    let heading = format!("[sections.{section}]\n");
    assert!(source.contains(&heading), "{section}");
    format!(
        "# first note\n{}\n# final note\n",
        source.replace(&heading, &format!("{heading}# a note within the section\n"))
    )
}

#[test]
fn comments_are_ignored_and_do_not_relax_unit_fields() {
    for (reference, source, section, mech) in [
        (
            "JR7-D",
            include_str!("../game/units/JR7-D.toml"),
            "left_arm",
            true,
        ),
        (
            "Demolisher",
            include_str!("../game/units/Demolisher.toml"),
            "turret",
            false,
        ),
        (
            "Kestrel",
            include_str!("../game/units/Kestrel.toml"),
            "rotor",
            false,
        ),
    ] {
        let commented = commented(source, section);
        let duplicate = format!("name = \"duplicate\"\n{commented}");
        let unclosed = format!("unit_era = \"unclosed\n{commented}");
        if mech {
            let expected = MechTemplate::parse(reference, source).unwrap();
            let actual = MechTemplate::parse(reference, &commented).unwrap();
            assert_eq!(actual, expected);
            assert!(!actual.attributes.contains_key("comment"));
            assert_eq!(
                MechLoadout::resolve(&actual).unwrap(),
                MechLoadout::resolve(&expected).unwrap()
            );
            assert!(MechTemplate::parse(reference, &duplicate).is_err());
            assert!(MechTemplate::parse(reference, &unclosed).is_err());
        } else {
            let expected = VehicleTemplate::parse(reference, source).unwrap();
            let actual = VehicleTemplate::parse(reference, &commented).unwrap();
            assert_eq!(actual, expected);
            assert!(!actual.attributes.contains_key("comment"));
            assert_eq!(
                VehicleLoadout::resolve(&actual).unwrap(),
                VehicleLoadout::resolve(&expected).unwrap()
            );
            assert!(VehicleTemplate::parse(reference, &duplicate).is_err());
            assert!(VehicleTemplate::parse(reference, &unclosed).is_err());
        }
    }
    let oversized = format!("# {}", "x".repeat(1_048_576));
    assert!(MechTemplate::parse("oversized", &oversized).is_err());
    assert!(VehicleTemplate::parse("oversized", &oversized).is_err());
}

#[tokio::test]
async fn asset_comments_are_not_exposed_through_native_or_lua_inspection() {
    let (dir, config, scripts) = support::isolated_scripts().await;
    std::fs::create_dir_all(dir.path().join("units")).unwrap();
    let source = commented(
        include_str!("../game/units/Grendel-Prime.toml"),
        "left_torso",
    );
    std::fs::write(dir.path().join("units/Grendel-Prime.toml"), &source).unwrap();
    let expected = MechTemplate::parse("Grendel-Prime", &source).unwrap();
    assert!(!expected.attributes.contains_key("comment"));
    assert_eq!(
        expected,
        MechTemplate::parse(
            "Grendel-Prime",
            include_str!("../game/units/Grendel-Prime.toml")
        )
        .unwrap()
    );
    let before = scripts.world().btech.clone();
    let native = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        "@btech template Grendel-Prime",
    );
    assert!(native.contains("Asset parsed"), "{native}");
    assert!(!native.contains("note"), "{native}");
    let absent: bool = scripts
        .eval_callback("return btech.template.inspect('Grendel-Prime').attributes.comment == nil")
        .unwrap();
    assert!(absent);
    assert_eq!(scripts.world().btech, before);
}
