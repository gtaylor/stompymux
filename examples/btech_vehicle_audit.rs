//! Inspect named ground-vehicle assets with equipment diagnostics, without claiming live simulation support.
use anyhow::{Context, Result};
use std::{collections::BTreeMap, path::PathBuf};
use stompymux_rs::{Vehicle, VehicleLoadout, read_battle_vehicle_template};

/// Print typed definitions or diagnostics for explicitly selected assets in the configured directory.
fn main() -> Result<()> {
    let mut arguments = std::env::args().skip(1);
    let root = PathBuf::from(
        arguments
            .next()
            .context("Usage: btech_vehicle_audit <mech-directory> <asset>...")?,
    );
    let mut results = BTreeMap::new();
    for name in arguments {
        let result = match read_battle_vehicle_template(&root, &name) {
            Ok(definition) => {
                let construction = match Vehicle::new(definition.clone()) {
                    Ok(_) => serde_json::json!({"constructed": true}),
                    Err(error) => {
                        serde_json::json!({"constructed": false, "error": format!("{error:#}")})
                    }
                };
                let engine = match definition.engine() {
                    Ok(report) => serde_json::json!({"report": report}),
                    Err(error) => serde_json::json!({"error": format!("{error:#}")}),
                };
                let mass = match definition.mass() {
                    Ok(report) => serde_json::json!({"report": report}),
                    Err(error) => serde_json::json!({"error": format!("{error:#}")}),
                };
                let equipment = match VehicleLoadout::resolve(&definition) {
                    Ok(loadout) => serde_json::json!({"resolved": true, "loadout": loadout}),
                    Err(error) => {
                        serde_json::json!({"resolved": false, "error": format!("{error:#}")})
                    }
                };
                serde_json::json!({ "definition": definition, "construction": construction, "equipment": equipment, "engine": engine, "mass": mass, "decoded": true, "simulation_supported": false })
            }
            Err(error) => {
                serde_json::json!({ "error": format!("{error:#}"), "decoded": false, "simulation_supported": false })
            }
        };
        results.insert(name, result);
    }
    anyhow::ensure!(!results.is_empty(), "Select at least one vehicle asset");
    println!("{}", serde_json::to_string_pretty(&results)?);
    Ok(())
}
