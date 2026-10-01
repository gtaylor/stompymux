//! Convert brace-delimited unit templates into `<reference>.toml` documents in place.
//!
//! Each converted document is re-read and compared against the original before
//! the original is removed. Usage: `convert-templates <directory>...`
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let mut failures = 0;
    let mut converted = 0;
    for directory in std::env::args().skip(1) {
        let mut paths: Vec<_> = std::fs::read_dir(&directory)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<_, _>>()?;
        paths.sort();
        for path in paths {
            if !path.is_file() || path.extension().is_some_and(|extension| extension == "toml") {
                continue;
            }
            match convert(&path) {
                Ok(()) => converted += 1,
                Err(error) => {
                    failures += 1;
                    eprintln!("{}: {error:#}", path.display());
                }
            }
        }
    }
    println!("converted {converted}, failed {failures}");
    anyhow::ensure!(failures == 0, "{failures} templates failed to convert");
    Ok(())
}

/// Convert, verify, write the document and remove the original.
fn convert(path: &Path) -> anyhow::Result<()> {
    let reference = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("template name is not UTF-8"))?;
    let legacy = std::fs::read_to_string(path)?;
    let document = stompymux_rs::convert_legacy_template(&legacy)?;
    stompymux_rs::verify_legacy_conversion(reference, &legacy, &document)?;
    std::fs::write(path.with_file_name(format!("{reference}.toml")), document)?;
    std::fs::remove_file(path)?;
    Ok(())
}
