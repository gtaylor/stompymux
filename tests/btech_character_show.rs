//! Wizard catalog reports preserve native admission and never initialize character records.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
mod support;

/// Categories, ignored second arguments and denied access survive native dispatch and restart.
#[tokio::test]
async fn wizard_catalogs_are_read_only_and_available_after_restart() {
    let (_dir, config, mut world) = support::isolated_world().await;
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let cases = [
        ("allvalues", "charvalues", 119),
        ("values", "Char_value", 14),
        ("skills", "Char_skill", 78),
        ("advantages", "Char_advantage", 22),
        ("attributes", "Char_attribute", 5),
    ];
    for (category, heading, count) in cases {
        let report = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("+show {category}"),
        );
        assert!(
            report.contains(&format!("List of {heading} available:")),
            "{report}"
        );
        assert!(
            report.contains(&format!("Total of {count} things found.")),
            "{report}"
        );
        assert_eq!(
            report,
            support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("+show {}=ignored", category.to_ascii_uppercase())
            )
        );
    }
    let fields = support::run_text(&scripts, &config, ObjectId(1), 1, "+show btechvalues");
    assert!(fields.contains("BTech fields available to view and set:"));
    assert!(fields.contains("\t0\tmapindex"));
    assert!(fields.contains("\t4\tlockmode"));
    assert!(fields.contains("\t0\thexes_walked"));
    assert_eq!(fields.lines().count(), 116);
    for command in ["+show char_skills", "+show s", "+show btechvalues mech"] {
        assert!(
            support::run_text(&scripts, &config, ObjectId(1), 1, command)
                .contains("Invalid arguments to +show command!")
        );
    }
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "+show").contains("Valid arguments:")
    );
    let denied = support::run_text(&scripts, &config, ObjectId(2), 2, "+show skills");
    assert!(!denied.contains("Acrobatics"));
    assert!(denied.contains("Permission denied"), "{denied}");
    assert!(scripts.world().btech == world.btech);
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    let restarted = Scripts::new(&config, Rc::new(RefCell::new(restored))).unwrap();
    assert_eq!(
        fields,
        support::run_text(&restarted, &config, ObjectId(1), 1, "+show btechvalues")
    );
}
