//! Audit regular files in a mech directory using bounded reads and the shared construction checker.
use anyhow::{Context, Result};
use std::{collections::BTreeMap, path::PathBuf};
use stompymux_rs::{check_battle_template, read_battle_template};

/// Print deterministic JSON diagnostics without creating a world or modifying any assets.
fn main() -> Result<()> {
    let directory = PathBuf::from(
        std::env::args()
            .nth(1)
            .context("Usage: btech_template_audit <mech-directory>")?,
    );
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&directory)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            files.push(
                entry
                    .file_name()
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("Non-UTF-8 asset name"))?,
            );
        }
    }
    files.sort();
    let mut results = Vec::new();
    let mut rejected = BTreeMap::<String, usize>::new();
    let mut constructible = 0;
    for name in files {
        match read_battle_template(&directory, &name) {
            Ok(template) => {
                let report = check_battle_template(&template);
                if report.constructible {
                    constructible += 1;
                }
                if let Some(reason) = &report.rejection {
                    *rejected.entry(reason.clone()).or_default() += 1;
                }
                results.push(serde_json::json!({"asset":name,"construction":report}));
            }
            Err(error) => {
                let reason = format!("{error:#}");
                *rejected.entry(reason.clone()).or_default() += 1;
                results.push(serde_json::json!({"asset":name,"asset_error":reason}));
            }
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"files":results.len(),"constructible":constructible,"rejections":rejected,"results":results})
        )?
    );
    Ok(())
}
