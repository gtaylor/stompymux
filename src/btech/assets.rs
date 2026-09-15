//! Size-bounded reads of named assets confined to their configured asset directory.
use super::{BattleMapAsset, BattleTemplate};
use anyhow::{Context, Result, ensure};
use std::{
    fs::File,
    io::Read,
    path::{Component, Path},
};

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

/// Text templates retain UTF-8 validation over the same bounded, confined reader.
fn read(root: &Path, name: &str, limit: usize) -> Result<String> {
    String::from_utf8(read_bytes(root, name, limit)?).context("asset is not UTF-8")
}

/// Decode a map from a configured map directory.
pub fn read_map(root: &Path, name: &str) -> Result<BattleMapAsset> {
    read_map_diagnostics(root, name, 0).map(|(map, _)| map)
}

/// Keep decoding and file confinement identical for inspection and activated maps.
pub(super) fn read_map_diagnostics(
    root: &Path,
    name: &str,
    initial_flags: i32,
) -> Result<(BattleMapAsset, Vec<super::map::MapTerrainWarning>)> {
    BattleMapAsset::parse_diagnostics(
        &read_bytes(root, name, 2_100_000).context(super::map::MapFileFailure::Unavailable)?,
        initial_flags,
    )
    .with_context(|| format!("map {name}"))
}

/// Publish substitutions through the configured map-error channel inside the caller's transaction.
pub(super) fn publish_map_warnings(
    scripts: &crate::Scripts,
    config: &crate::Config,
    id: crate::ObjectId,
    warnings: &[super::map::MapTerrainWarning],
) -> Result<()> {
    let messages = warnings
        .iter()
        .map(|&super::map::MapTerrainWarning { x, y, symbol }| {
            super::BattleChannelMessage::new(
                super::BattleChannel::MapErrors,
                format!("Map #{}: Invalid terrain at {x},{y}: '{symbol}'", id.0),
            )
        })
        .collect::<Vec<_>>();
    super::channels::publish(scripts, config, &messages)
}

/// Decode a biped template from a configured mech directory.
pub fn read_template(root: &Path, name: &str) -> Result<BattleTemplate> {
    BattleTemplate::parse(&read(root, name, 1_048_576)?).with_context(|| format!("template {name}"))
}

/// Decode a ground-vehicle definition from the configured game asset directory.
pub fn read_vehicle_template(root: &Path, name: &str) -> Result<super::BattleVehicleTemplate> {
    super::BattleVehicleTemplate::parse(&read(root, name, 1_048_576)?)
        .with_context(|| format!("vehicle template {name}"))
}

/// Read an explicitly typed construction asset from the configured unit directory.
pub fn read_unit_template(root: &Path, name: &str) -> Result<super::BattleUnitTemplate> {
    super::BattleUnitTemplate::parse(&read(root, name, 1_048_576)?)
        .with_context(|| format!("unit template {name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confines_assets_and_bounds_reads() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("asset"), "four").unwrap();
        assert!(read(dir.path(), "asset", 3).is_err());
        assert_eq!(read(dir.path(), "asset", 4).unwrap(), "four");
        assert!(read(dir.path(), "../asset", 10).is_err());
        assert!(read(dir.path(), "/etc/passwd", 10).is_err());
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("asset"), "outside").unwrap();
        std::os::unix::fs::symlink(outside.path().join("asset"), dir.path().join("escape"))
            .unwrap();
        assert!(read(dir.path(), "escape", 10).is_err());
    }
}
