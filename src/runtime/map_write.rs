//! Deferred map assets use bounded paths and atomic replacement after world commit.
use crate::{Config, ObjectId};
use anyhow::{Context, Result, ensure};
use std::{
    io::Write,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

/// One prepared map replacement; construction and publication both validate the destination.
#[derive(Clone)]
pub struct MapAssetWrite {
    /// Operator receiving the final publication result.
    pub actor: ObjectId,
    /// Relative asset destination, checked again when publishing.
    pub name: String,
    /// Immutable complete file contents shared by effect checkpoints.
    source: Arc<str>,
}

impl MapAssetWrite {
    /// Prepare a bounded write without changing any file.
    pub fn new(config: &Config, actor: ObjectId, name: &str, source: String) -> Result<Self> {
        ensure!(source.len() <= 2_100_000, "Map asset exceeds size limit");
        destination(config, name)?;
        Ok(Self {
            actor,
            name: name.into(),
            source: source.into(),
        })
    }

    /// Storage accounted to the enclosing transaction's effect budget.
    pub fn bytes(&self) -> usize {
        self.name.len() + self.source.len()
    }

    /// Publish complete bytes using a same-directory temporary file and atomic rename.
    /// Any failure before replacement leaves an existing destination untouched.
    pub fn publish(&self, config: &Config) -> Result<()> {
        let path = destination(config, &self.name)?;
        let parent = path.parent().context("Map directory is unavailable")?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        if let Ok(metadata) = std::fs::metadata(&path) {
            file.as_file().set_permissions(metadata.permissions())?;
        }
        file.write_all(self.source.as_bytes())?;
        file.as_file().sync_all()?;
        // Recheck the named destination immediately before replacement.
        ensure!(
            destination(config, &self.name)? == path,
            "Map destination changed"
        );
        file.persist(&path).map_err(|error| error.error)?;
        Ok(())
    }
}

/// Require an existing directory under the configured asset root; never follow a destination symlink.
fn destination(config: &Config, name: &str) -> Result<PathBuf> {
    let relative = Path::new(name);
    ensure!(
        !name.is_empty()
            && !name.contains('\0')
            && relative
                .components()
                .all(|part| matches!(part, Component::Normal(_))),
        "Invalid map asset name"
    );
    let root = config
        .path(&config.database.map_database)
        .canonicalize()
        .context("Map directory is unavailable")?;
    let candidate = root.join(relative);
    let parent = candidate
        .parent()
        .context("Invalid map asset name")?
        .canonicalize()
        .context("Map directory is unavailable")?;
    ensure!(
        parent.starts_with(&root),
        "Map asset is outside its configured directory"
    );
    let path = parent.join(candidate.file_name().context("Invalid map asset name")?);
    match std::fs::symlink_metadata(&path) {
        Ok(metadata) => ensure!(
            metadata.file_type().is_file(),
            "Map destination must be a regular file"
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(path)
}
