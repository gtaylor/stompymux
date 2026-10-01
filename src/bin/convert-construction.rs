//! Rewrite literal TOML unit templates as construction-based documents, in place.
//!
//! Each document is verified to decode to the same sections and flags before it is
//! replaced. Usage: `convert-construction <directory>...`
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let (mut converted, mut failed) = (0, 0);
    for directory in std::env::args().skip(1) {
        let mut paths: Vec<_> = std::fs::read_dir(&directory)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<_, _>>()?;
        paths.sort();
        for path in paths.iter().filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "toml")
        }) {
            match convert(path) {
                Ok(()) => converted += 1,
                Err(error) => {
                    failed += 1;
                    eprintln!("{}: {error:#}", path.display());
                }
            }
        }
    }
    println!("converted {converted}, failed {failed}");
    anyhow::ensure!(failed == 0, "{failed} templates failed to convert");
    Ok(())
}

/// Convert and verify one document, then replace it.
fn convert(path: &Path) -> anyhow::Result<()> {
    let reference = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| anyhow::anyhow!("template name is not UTF-8"))?;
    let source = std::fs::read_to_string(path)?;
    let document = stompymux_rs::convert_constructed_template(reference, &source)?;
    std::fs::write(path, document)?;
    Ok(())
}
