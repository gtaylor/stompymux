//! Repeated template comments retain authored notes without changing equipment or field validation.
use crate::support;
use stompymux_rs::*;

#[test]
fn comments_preserve_section_context_and_do_not_relax_unit_fields() {
    for (source, section, mech) in [
        (include_str!("../game/mechs/JR7-D.toml"), "Left_Arm", true),
        (include_str!("../game/mechs/Demolisher.toml"), "Turret", false),
        (include_str!("../game/mechs/Kestrel.toml"), "Rotor", false),
    ] {
        let commented = format!(
            "Comment {{ first note }}\n{}\ncOmMeNt {{ final note }}\n",
            source.replace(
                &format!("{section}\n"),
                &format!("{section}\nCOMMENT {{ a note within the section }}\n")
            )
        );
        if mech {
            let mut expected = BattleTemplate::parse("test",source).unwrap();
            expected.attributes.insert(
                "comment".into(),
                "first note\na note within the section\nfinal note".into(),
            );
            let actual = BattleTemplate::parse("test",&commented).unwrap();
            assert_eq!(actual, expected);
            assert_eq!(
                BattleLoadout::resolve(&actual).unwrap(),
                BattleLoadout::resolve(&BattleTemplate::parse("test",source).unwrap()).unwrap()
            );
            assert!(BattleTemplate::parse("test",&format!("{commented}\nName {{ duplicate }}")).is_err());
            assert!(BattleTemplate::parse("test",&format!("{commented}\nComment {{ unclosed")).is_err());
        } else {
            let mut expected = BattleVehicleTemplate::parse("test",source).unwrap();
            expected.attributes.insert(
                "comment".into(),
                "first note\na note within the section\nfinal note".into(),
            );
            let actual = BattleVehicleTemplate::parse("test",&commented).unwrap();
            assert_eq!(actual, expected);
            assert_eq!(
                BattleVehicleLoadout::resolve(&actual).unwrap(),
                BattleVehicleLoadout::resolve(&BattleVehicleTemplate::parse("test",source).unwrap())
                    .unwrap()
            );
            assert!(
                BattleVehicleTemplate::parse("test",&format!("{commented}\nName {{ duplicate }}"))
                    .is_err()
            );
            assert!(
                BattleVehicleTemplate::parse("test",&format!("{commented}\nComment {{ unclosed")).is_err()
            );
        }
    }
    let oversized = format!("Comment {{ {} }}", "x".repeat(1_048_576));
    assert!(BattleTemplate::parse("test",&oversized).is_err());
    assert!(BattleVehicleTemplate::parse("test",&oversized).is_err());
}

#[tokio::test]
async fn repeated_asset_comments_are_available_through_native_and_lua_inspection() {
    let (dir, config, scripts) = support::isolated_scripts().await;
    std::fs::create_dir_all(dir.path().join("mechs")).unwrap();
    let source = include_str!("../game/mechs/Grendel-Prime.toml");
    std::fs::write(dir.path().join("mechs/Grendel-Prime.toml"), source).unwrap();
    let expected = BattleTemplate::parse("test",source).unwrap();
    assert_eq!(expected.attributes["comment"], "HeatSink - -\nHeatSink - -");
    let before = scripts.world().btech.clone();
    let native = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        "@btech template Grendel-Prime",
    );
    assert!(native.contains("Asset parsed"), "{native}");
    let comments: String = scripts
        .eval_callback("return btech.template.inspect('Grendel-Prime').attributes.comment")
        .unwrap();
    assert_eq!(comments, expected.attributes["comment"]);
    assert_eq!(scripts.world().btech, before);
}
