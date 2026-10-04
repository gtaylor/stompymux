//! Check that every map file in a directory loads.
//!
//! `map-check [DIR...]` parses each `*.toml` file in each directory (default `game/maps`) as a
//! map, reports anything else in the directory, and exits non-zero when any problem remains.
use std::{env, fs, path::PathBuf, process::ExitCode};

use anyhow::{Context, Result};
use stompymux_map::MapAsset;

fn main() -> Result<ExitCode> {
    let mut dirs: Vec<PathBuf> = env::args().skip(1).map(PathBuf::from).collect();
    if dirs.is_empty() {
        dirs.push(PathBuf::from("game/maps"));
    }
    let mut files = Vec::new();
    for dir in &dirs {
        for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
            files.push(entry?.path());
        }
    }
    files.sort();
    let mut failing = 0;
    for path in &files {
        let problem = if path.extension().is_none_or(|extension| extension != "toml") {
            Some("not a .toml map file".to_owned())
        } else {
            fs::read_to_string(path)
                .context("reading")
                .and_then(|source| MapAsset::parse(&source))
                .err()
                .map(|error| format!("{error:#}"))
        };
        if let Some(problem) = problem {
            failing += 1;
            println!("{}: {problem}", path.display());
        }
    }
    println!("{} map(s) checked, {failing} with problems", files.len());
    Ok(if failing == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
