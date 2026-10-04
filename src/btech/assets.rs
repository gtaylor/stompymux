//! Size-bounded reads of named map and template assets confined to their configured directory.
use super::{BattleMapAsset, BattleTemplate};
use anyhow::{Context, Result, ensure};
use std::{
    fs::File,
    io::Read,
    path::{Component, Path},
};

/// Why a named map file could not be loaded, without parsing error strings.
#[derive(Debug, Clone, Copy)]
pub(super) enum MapFileFailure {
    /// No map file has that name.
    Unavailable,
    /// The file exists but is not a valid map; the error chain carries the reason.
    Invalid,
}

impl std::fmt::Display for MapFileFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unavailable => "#-1 Map not found.",
            Self::Invalid => "#-1 Map invalid.",
        })
    }
}

impl std::error::Error for MapFileFailure {}

/// Read a relative asset without allowing parent traversal, symlink escape or unbounded input.
fn read_bytes(root: &Path, name: &str, limit: usize) -> Result<Vec<u8>> {
    let path = Path::new(name);
    ensure!(
        !name.is_empty()
            && path
                .components()
                .all(|part| matches!(part, Component::Normal(_))),
        "invalid asset name"
    );
    let root = root.canonicalize().context("asset directory unavailable")?;
    let path = root
        .join(path)
        .canonicalize()
        .with_context(|| format!("asset {name} not found"))?;
    ensure!(
        path.starts_with(&root),
        "asset is outside its configured directory"
    );
    let file = File::open(path)?;
    ensure!(file.metadata()?.is_file(), "asset is not a regular file");
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= limit, "asset exceeds size limit");
    Ok(bytes)
}

/// Decode the map file `NAME.toml` from a configured map directory.
pub fn read_map(root: &Path, name: &str) -> Result<BattleMapAsset> {
    read_map_with_flags(root, name, 0)
}

/// Decode a map file, keeping `inherited_flags` when it names no flags of its own.
pub(super) fn read_map_with_flags(
    root: &Path,
    name: &str,
    inherited_flags: i64,
) -> Result<BattleMapAsset> {
    let bytes = read_bytes(root, &format!("{name}.toml"), 2_100_000)
        .context(MapFileFailure::Unavailable)?;
    String::from_utf8(bytes)
        .context("map file is not UTF-8")
        .and_then(|source| BattleMapAsset::parse_with_flags(&source, inherited_flags))
        .context(MapFileFailure::Invalid)
        .with_context(|| format!("map {name}"))
}

/// Decode a biped template from a configured mech directory.
pub fn read_template(root: &Path, name: &str) -> Result<BattleTemplate> {
    let (reference, source) = super::template_contract_assets::read_template_document(root, name)?;
    BattleTemplate::parse(&reference, &source).with_context(|| format!("template {name}"))
}

/// Decode a ground-vehicle definition from the configured game asset directory.
pub fn read_vehicle_template(root: &Path, name: &str) -> Result<super::BattleVehicleTemplate> {
    let (reference, source) = super::template_contract_assets::read_template_document(root, name)?;
    super::BattleVehicleTemplate::parse(&reference, &source)
        .with_context(|| format!("vehicle template {name}"))
}

/// Read an explicitly typed construction asset from the configured unit directory.
pub fn read_unit_template(root: &Path, name: &str) -> Result<super::BattleUnitTemplate> {
    let (reference, source) = super::template_contract_assets::read_template_document(root, name)?;
    super::BattleUnitTemplate::parse(&reference, &source)
        .with_context(|| format!("unit template {name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confines_assets_and_bounds_reads() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("asset"), "four").unwrap();
        assert!(read_bytes(dir.path(), "asset", 3).is_err());
        assert_eq!(read_bytes(dir.path(), "asset", 4).unwrap(), b"four");
        assert!(read_bytes(dir.path(), "../asset", 10).is_err());
        assert!(read_bytes(dir.path(), "/etc/passwd", 10).is_err());
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("asset"), "outside").unwrap();
        std::os::unix::fs::symlink(outside.path().join("asset"), dir.path().join("escape"))
            .unwrap();
        assert!(read_bytes(dir.path(), "escape", 10).is_err());
    }
}
