//! Convert MegaMek BattleMech (`.mtf`) and vehicle (`.blk`) files into stompymux unit templates.
//!
//! ```text
//! megamek-convert "Atlas AS7-D.mtf"                      # print the template
//! megamek-convert -o game/mechs mechs/*.mtf tanks/*.blk  # write <reference>.toml files
//! ```
//!
//! Each unit is read into a draft template, then checked by the game itself: the draft must
//! parse as a template and construct as a live unit, so anything stompymux cannot field is
//! refused. Unsupported unit types, weapons, ammunition and construction technology are errors,
//! never silent omissions. The written template is the game's own canonical rendering.
mod blk;
mod draft;
mod equipment;
mod layout;
mod mtf;

use anyhow::{Context, Result, bail, ensure};
use clap::Parser;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use stompymux_rs::{UnitTemplate, Vehicle, check_battle_template};

/// Command-line options.
#[derive(Parser)]
#[command(about = "Convert MegaMek .mtf and .blk unit files into stompymux unit templates")]
struct Args {
    /// MegaMek unit files to convert.
    #[arg(required = true)]
    files: Vec<PathBuf>,

    /// Write `<reference>.toml` templates into this directory instead of printing them.
    #[arg(short, long)]
    output_dir: Option<PathBuf>,

    /// Template reference (file stem) to use instead of one derived from the unit's name.
    #[arg(short, long)]
    reference: Option<String>,

    /// Replace templates that already exist in the output directory.
    #[arg(long)]
    force: bool,
}

/// A unit read from a MegaMek file, before validation.
pub struct Converted {
    pub draft: draft::Draft,
    /// Suggested template reference, before sanitizing.
    pub reference: String,
    /// Chassis-qualified reference to use when the suggested one is already taken.
    pub full_reference: String,
    /// Data the converter dropped without failing.
    pub warnings: Vec<String>,
}

fn main() -> Result<ExitCode> {
    let args = Args::parse();
    ensure!(
        args.output_dir.is_some() || args.files.len() == 1,
        "pass --output-dir to convert more than one file"
    );
    ensure!(
        args.reference.is_none() || args.files.len() == 1,
        "--reference names a single unit"
    );
    let mut failures = 0;
    let mut written = std::collections::BTreeSet::new();
    for path in &args.files {
        if let Err(error) = run(&args, path, &mut written) {
            failures += 1;
            eprintln!("{}: error: {error:#}", path.display());
        }
    }
    if args.files.len() > 1 {
        eprintln!(
            "{} file(s) converted, {failures} failed",
            args.files.len() - failures
        );
    }
    Ok(if failures == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// Convert one file and print or write its template. `written` holds the references already
/// written in this run, so two units sharing a model name never overwrite each other.
fn run(args: &Args, path: &Path, written: &mut std::collections::BTreeSet<String>) -> Result<()> {
    let source = fs::read_to_string(path).context("reading")?;
    let converted = match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("mtf") => mtf::convert(&source)?,
        Some("blk") => blk::convert(&source)?,
        _ => bail!("expected a MegaMek .mtf or .blk file"),
    };
    for warning in &converted.warnings {
        eprintln!("{}: warning: {warning}", path.display());
    }
    let reference = match &args.reference {
        Some(reference) => sanitize(reference)?,
        None => {
            let suggested = sanitize(&converted.reference)?;
            if written.contains(&suggested) {
                sanitize(&converted.full_reference)?
            } else {
                suggested
            }
        }
    };
    ensure!(
        !written.contains(&reference),
        "another unit in this run already wrote {reference}; pass --reference"
    );
    let template = finish(&reference, &converted.draft)?;
    let Some(directory) = &args.output_dir else {
        print!("{template}");
        return Ok(());
    };
    let target = directory.join(format!("{reference}.toml"));
    ensure!(
        args.force || !target.exists(),
        "{} already exists; pass --force to replace it",
        target.display()
    );
    fs::write(&target, template).with_context(|| format!("writing {}", target.display()))?;
    written.insert(reference);
    eprintln!("{} -> {}", path.display(), target.display());
    Ok(())
}

/// Validate a draft as the game would load it and return the canonical template document.
fn finish(reference: &str, draft: &draft::Draft) -> Result<String> {
    let source = draft.render();
    let template = UnitTemplate::parse(reference, &source)
        .context("stompymux rejected the converted template")?;
    let document = match template {
        UnitTemplate::Mech(mech) => {
            let check = check_battle_template(&mech);
            if let Some(rejection) = check.rejection {
                bail!("stompymux cannot construct this mech: {rejection}");
            }
            ensure!(
                check.ammunition_adjustments.is_empty(),
                "ammunition bins do not match stompymux capacities: {:?}",
                check.ammunition_adjustments
            );
            mech.to_document()?
        }
        UnitTemplate::Vehicle(vehicle) => {
            Vehicle::new(vehicle.clone()).context("stompymux cannot construct this vehicle")?;
            vehicle.to_document()?
        }
    };
    UnitTemplate::parse(reference, &document).context("the rendered template does not load")?;
    Ok(document)
}

/// Turn a unit name into a template reference: spaces become underscores, slashes become
/// hyphens and other punctuation is dropped.
fn sanitize(reference: &str) -> Result<String> {
    let sanitized: String = reference
        .trim()
        .chars()
        .filter_map(|c| match c {
            c if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') => Some(c),
            ' ' => Some('_'),
            '/' => Some('-'),
            _ => None,
        })
        .collect();
    ensure!(
        !sanitized.is_empty() && !sanitized.starts_with('.'),
        "cannot derive a template reference from {reference:?}; pass --reference"
    );
    Ok(sanitized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_are_file_safe() {
        assert_eq!(sanitize("AS7-D").unwrap(), "AS7-D");
        assert_eq!(
            sanitize("Bulldog Medium Tank (LRM)").unwrap(),
            "Bulldog_Medium_Tank_LRM"
        );
        assert_eq!(sanitize("AC/2 Carrier").unwrap(), "AC-2_Carrier");
        assert!(sanitize("..").is_err());
        assert!(sanitize("()").is_err());
    }
}
