//! Free-text unit metadata (era and technical readout) kept in template attributes.
use anyhow::{Result, ensure};
use std::collections::BTreeMap;

/// Metadata stays in the owned template and shares bounds across construction and named edits.
pub fn validate_unit_metadata(attributes: &BTreeMap<String, String>) -> Result<()> {
    ensure!(
        ["unit_era", "unit_tro"]
            .into_iter()
            .all(|key| attributes.get(key).is_none_or(|value| value.len() <= 24)),
        "Unit metadata exceeds 24 bytes"
    );
    Ok(())
}

/// Absent source metadata has the same visible default on every chassis.
pub fn unit_metadata<'a>(attributes: &'a BTreeMap<String, String>, field: &str) -> &'a str {
    attributes.get(field).map_or("Undefined", String::as_str)
}
