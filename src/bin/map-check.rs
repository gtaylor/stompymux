//! Check map assets for problems the game's map loader rejects or silently tolerates.
//!
//! `map-check [--fix] [DIR...]` checks every file in each directory (default `game/maps`) and
//! exits non-zero when any problem remains. `--fix` first removes mechanical junk (CRLF endings,
//! padding, blank lines, trailing lines the loader ignores); it refuses any rewrite that would
//! change how a loadable map decodes, so terrain problems are always left for a person.
use std::{env, fs, path::PathBuf, process::ExitCode};

use anyhow::{Context, Result};
use stompymux_rs::{BattleMapAsset, check_map_source, tidy_map_source};

fn main() -> Result<ExitCode> {
    let mut fix = false;
    let mut dirs = Vec::new();
    for arg in env::args().skip(1) {
        match arg.as_str() {
            "--fix" => fix = true,
            _ => dirs.push(PathBuf::from(arg)),
        }
    }
    if dirs.is_empty() {
        dirs.push(PathBuf::from("game/maps"));
    }
    let mut files = Vec::new();
    for dir in &dirs {
        for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
            let path = entry?.path();
            if path.is_file() {
                files.push(path);
            }
        }
    }
    files.sort();
    let (mut failing, mut fixed) = (0, 0);
    for path in &files {
        let mut source = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        if fix {
            let tidy = tidy_map_source(&source);
            let before = String::from_utf8(source.clone())
                .ok()
                .and_then(|text| BattleMapAsset::from_cells(&text).ok());
            let after = String::from_utf8(tidy.clone())
                .ok()
                .and_then(|text| BattleMapAsset::from_cells(&text).ok());
            if tidy != source && (before.is_none() || before == after) {
                fs::write(path, &tidy).with_context(|| format!("writing {}", path.display()))?;
                source = tidy;
                fixed += 1;
            }
        }
        let issues = check_map_source(&source);
        if !issues.is_empty() {
            failing += 1;
        }
        for issue in issues {
            println!("{}: {issue}", path.display());
        }
    }
    if fix {
        println!("tidied {fixed} file(s)");
    }
    println!("{} map(s) checked, {failing} with problems", files.len());
    Ok(if failing == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
