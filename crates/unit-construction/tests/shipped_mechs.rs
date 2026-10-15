//! Every shipped BattleMech template passes TechManual construction validation. The game
//! directory is only read, never written.
use std::path::Path;
use stompymux_unit_construction::{MechTemplate, check_template};

/// Run the whole-set template check over `game/units` and list every Mech construction
/// refuses or that leaves its engine rating or technology base undeclared, so a rule change
/// and the templates it rejects land together.
#[test]
fn shipped_mech_templates_pass_construction() {
    let units = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../game/units");
    let mut entries: Vec<_> = std::fs::read_dir(&units)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "toml")
        })
        .collect();
    entries.sort();
    let mut checked = 0;
    let mut rejected = Vec::new();
    for path in entries {
        let source = std::fs::read_to_string(&path).unwrap();
        // DropShips and other non-Mech classes the parser refuses share the `mech` class
        // spelling; only biped and quad chassis are BattleMechs.
        if !source.lines().any(|line| line == "class = \"mech\"")
            || !source
                .lines()
                .any(|line| line == "movement = \"biped\"" || line == "movement = \"quad\"")
        {
            continue;
        }
        let reference = path.file_stem().unwrap().to_str().unwrap();
        let template = match MechTemplate::parse(reference, &source) {
            Ok(template) => template,
            Err(error) => {
                rejected.push(format!("{reference}: {error:#}"));
                continue;
            }
        };
        checked += 1;
        let report = check_template(&template);
        if let Some(rejection) = report.rejection {
            rejected.push(format!("{reference}: {rejection}"));
        }
        // Every shipped Mech states its engine rating rather than leaving it to the MP, and
        // its technology base rather than leaving it to the Inner Sphere default.
        if report.declared_engine_rating.is_none() {
            rejected.push(format!("{reference}: no declared engine_rating"));
        }
        if !source.lines().any(|line| line.starts_with("tech_base = ")) {
            rejected.push(format!("{reference}: no declared tech_base"));
        }
    }
    assert!(checked > 1000, "only {checked} Mech templates found");
    assert!(
        rejected.is_empty(),
        "{} of {checked} shipped Mech templates are rejected:\n{}",
        rejected.len(),
        rejected.join("\n")
    );
}
