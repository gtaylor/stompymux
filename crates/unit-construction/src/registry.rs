//! Confined, bounded template lookup: `<reference>.toml` documents anywhere under a unit root.

use super::{RawTemplate, UnitTemplate};
use anyhow::{Context, Result, ensure};
use std::{
    collections::{BTreeMap, HashMap},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

/// Extension carried by every unit template document.
const TEMPLATE_EXTENSION: &str = "toml";

/// Linux `O_NOFOLLOW`, so a symlink swapped in after the check is never written through.
const O_NOFOLLOW: i32 = 0x20000;

/// One BattleTech context's lazily populated template registry.
#[derive(Clone, Debug, Default)]
pub struct TemplateRegistryCache {
    /// Lowercase reference to document path, per canonical root.
    roots: HashMap<PathBuf, BTreeMap<String, PathBuf>>,
}

impl PartialEq for TemplateRegistryCache {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for TemplateRegistryCache {}

impl TemplateRegistryCache {
    /// Invalidate every cached root before an administrative template write attempt.
    pub fn clear(&mut self) {
        self.roots.clear();
    }
}

/// Index every visible `.toml` document under `root` by its lowercase file stem.
/// When two documents share a reference, the lexically first path wins.
fn scan(root: &Path) -> Result<BTreeMap<String, PathBuf>> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for item in std::fs::read_dir(&directory)? {
            let item = item?;
            let name = item.file_name();
            if name.as_encoded_bytes().starts_with(b".") {
                continue;
            }
            let kind = item.file_type()?;
            let path = item.path();
            if kind.is_dir() {
                pending.push(path);
            } else if kind.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext == TEMPLATE_EXTENSION)
                && let Some(stem) = path.file_stem().and_then(|stem| stem.to_str())
            {
                found.push((stem.to_ascii_lowercase(), path));
            }
        }
    }
    found.sort_by(|left, right| left.1.cmp(&right.1));
    let mut index = BTreeMap::new();
    for (key, path) in found {
        index.entry(key).or_insert(path);
    }
    Ok(index)
}

/// Find the cached case-insensitive reference anywhere under the root.
pub fn resolve_template_path_cached(
    cache: &mut TemplateRegistryCache,
    root: &Path,
    reference: &str,
) -> Result<Option<PathBuf>> {
    resolve_template_path_bytes_cached(cache, root, reference.as_bytes())
}

/// Resolve an opaque Lua reference; references that are not UTF-8 never match a document.
pub fn resolve_template_path_bytes_cached(
    cache: &mut TemplateRegistryCache,
    root: &Path,
    reference: &[u8],
) -> Result<Option<PathBuf>> {
    let Ok(root) = root.canonicalize() else {
        return Ok(None);
    };
    let Ok(reference) = std::str::from_utf8(reference) else {
        return Ok(None);
    };
    let wanted = reference.to_ascii_lowercase();
    for attempt in 0..2 {
        if !cache.roots.contains_key(&root) {
            let Ok(index) = scan(&root) else {
                return Ok(None);
            };
            cache.roots.insert(root.clone(), index);
        }
        match cache.roots[&root].get(&wanted) {
            Some(path) if path.is_file() => return Ok(Some(path.clone())),
            Some(_) if attempt == 0 => {
                cache.roots.remove(&root);
            }
            _ => return Ok(None),
        }
    }
    Ok(None)
}

/// One-shot lookup for callers that deliberately do not retain a BattleTech context.
pub fn resolve_template_path(root: &Path, reference: &str) -> Result<Option<PathBuf>> {
    resolve_template_path_cached(&mut TemplateRegistryCache::default(), root, reference)
}

/// Read one resolved document confined to its root; its file stem is the unit reference.
fn read_confined(root: &Path, path: &Path) -> Result<(String, String)> {
    let root = root.canonicalize()?;
    let path = path.canonicalize()?;
    ensure!(path.starts_with(&root), "template path escapes its root");
    let metadata = std::fs::metadata(&path)?;
    ensure!(
        metadata.len() <= super::document::TEMPLATE_SIZE_LIMIT as u64,
        "template exceeds size limit"
    );
    let reference = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .context("template name is not UTF-8")?
        .to_owned();
    Ok((reference, std::fs::read_to_string(path)?))
}

/// Resolve a reference without a cached registry and read its document, returning the
/// reference as spelled by the document's file stem.
pub fn read_template_document(root: &Path, reference: &str) -> Result<(String, String)> {
    let path = resolve_template_path(root, reference)?
        .with_context(|| format!("template {reference} not found"))?;
    read_confined(root, &path)
}

/// Read one already-resolved template with confinement and the shared size bound.
pub fn read_resolved_template(root: &Path, path: &Path) -> Result<UnitTemplate> {
    let (reference, source) = read_confined(root, path)?;
    UnitTemplate::parse(&reference, &source)
}

/// Read one resolved template into class-neutral contract state, applying the
/// native load finalize so every raw read reflects loaded-mech semantics.
pub fn read_resolved_raw_template(root: &Path, path: &Path) -> Result<RawTemplate> {
    let (reference, source) = read_confined(root, path)?;
    let mut template = RawTemplate::parse(&reference, &source)?;
    finalize_raw_load_specials(&mut template);
    Ok(template)
}

/// Snapshot the native load finalize's geometry-derived technology bits into a
/// freshly read raw template: flippable biped arms follow the arm actuator
/// layout, and a compact engine is any center-torso installation below four
/// engine criticals. The bits stay stored until a mutating operation clears
/// them; the inspection listing only re-derives the compact-engine bit.
pub fn finalize_raw_load_specials(template: &mut RawTemplate) {
    use super::RawSectionCode::{CenterTorso, LeftArm, RightArm};
    if template.class != super::RawUnitClass::Mech {
        return;
    }
    let occupies = |code: super::RawSectionCode, slot: u8, equipment: &str| {
        template
            .sections
            .get(&code)
            .and_then(|section| section.criticals.get(&slot))
            .is_some_and(|critical| critical.equipment.eq_ignore_ascii_case(equipment))
    };
    let flippable = !occupies(LeftArm, 2, "LowerActuator")
        && !occupies(RightArm, 2, "LowerActuator")
        && !occupies(LeftArm, 3, "HandOrFootActuator")
        && !occupies(RightArm, 3, "HandOrFootActuator");
    let compact = template.sections.get(&CenterTorso).is_none_or(|section| {
        section
            .criticals
            .values()
            .filter(|critical| critical.equipment.eq_ignore_ascii_case("Engine"))
            .count()
            < 4
    });
    super::edit_special(&mut template.attributes, "specials", "FlipArms", flippable);
    if compact {
        super::edit_special(
            &mut template.attributes,
            "specials",
            "CompactEngine_Tech",
            true,
        );
    }
}

/// Write one template document, replacing the existing document for the
/// reference wherever it lives under the root, else creating `<reference>.toml`
/// directly in the root. The registry is invalidated before the attempt.
pub fn write_template(
    cache: &mut TemplateRegistryCache,
    root: &Path,
    reference: &str,
    source: &str,
) -> Result<()> {
    cache.clear();
    ensure!(
        !reference.is_empty()
            && !reference.starts_with('.')
            && !reference.as_bytes().contains(&0)
            && !reference.contains('/')
            && !reference.contains('\\'),
        "invalid template reference"
    );
    let root = root.canonicalize()?;
    ensure!(root.is_dir(), "template root is not a directory");
    let existing = scan(&root)?.remove(&reference.to_ascii_lowercase());
    let path = existing.unwrap_or_else(|| root.join(format!("{reference}.{TEMPLATE_EXTENSION}")));
    if let Ok(metadata) = std::fs::symlink_metadata(&path) {
        ensure!(
            !metadata.file_type().is_symlink(),
            "template target is a symbolic link"
        );
        ensure!(metadata.is_file(), "template target is not a regular file");
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .custom_flags(O_NOFOLLOW)
        .open(&path)?;
    file.write_all(source.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_matches_whole_references_case_insensitively_at_any_depth() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        std::fs::create_dir_all(root.join("clan/second")).unwrap();
        std::fs::create_dir(root.join(".hidden")).unwrap();
        std::fs::write(root.join("JR7-D.toml"), "x").unwrap();
        std::fs::write(root.join("clan/second/Mad-Cat-Prime.toml"), "x").unwrap();
        std::fs::write(root.join(".hidden/Hidden.toml"), "x").unwrap();
        std::fs::write(root.join("NoExtension"), "x").unwrap();
        let mut cache = TemplateRegistryCache::default();
        assert_eq!(
            resolve_template_path_cached(&mut cache, root, "jr7-d").unwrap(),
            Some(root.canonicalize().unwrap().join("JR7-D.toml"))
        );
        assert!(
            resolve_template_path_cached(&mut cache, root, "MAD-CAT-PRIME")
                .unwrap()
                .is_some()
        );
        for omitted in ["JR7", "Hidden", "NoExtension", "JR7-D.toml"] {
            assert!(
                resolve_template_path_cached(&mut cache, root, omitted)
                    .unwrap()
                    .is_none(),
                "{omitted}"
            );
        }
    }

    #[test]
    fn registry_refreshes_once_after_a_document_disappears() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        std::fs::create_dir(root.join("stock")).unwrap();
        std::fs::write(root.join("stock/Moved.toml"), "x").unwrap();
        let mut cache = TemplateRegistryCache::default();
        assert!(
            resolve_template_path_cached(&mut cache, root, "moved")
                .unwrap()
                .is_some()
        );
        std::fs::rename(root.join("stock/Moved.toml"), root.join("Moved.toml")).unwrap();
        assert_eq!(
            resolve_template_path_cached(&mut cache, root, "moved").unwrap(),
            Some(root.canonicalize().unwrap().join("Moved.toml"))
        );
    }

    #[test]
    fn duplicate_references_resolve_to_the_lexically_first_path() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        std::fs::create_dir_all(root.join("a")).unwrap();
        std::fs::create_dir_all(root.join("b")).unwrap();
        std::fs::write(root.join("b/Twin.toml"), "x").unwrap();
        std::fs::write(root.join("a/twin.toml"), "x").unwrap();
        assert_eq!(
            resolve_template_path(root, "TWIN").unwrap(),
            Some(root.canonicalize().unwrap().join("a/twin.toml"))
        );
    }

    #[test]
    fn writer_replaces_existing_documents_and_rejects_symlinks() {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("templates");
        std::fs::create_dir_all(root.join("stock")).unwrap();
        std::fs::write(root.join("stock/Old.toml"), "old").unwrap();
        let mut cache = TemplateRegistryCache::default();
        assert!(
            resolve_template_path_cached(&mut cache, &root, "new")
                .unwrap()
                .is_none()
        );
        write_template(&mut cache, &root, "new", "new").unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("new.toml")).unwrap(),
            "new"
        );
        assert!(
            resolve_template_path_cached(&mut cache, &root, "new")
                .unwrap()
                .is_some()
        );
        write_template(&mut cache, &root, "OLD", "replaced").unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("stock/Old.toml")).unwrap(),
            "replaced"
        );
        assert!(!root.join("OLD.toml").exists());

        let outside = directory.path().join("outside");
        std::fs::write(&outside, "safe").unwrap();
        symlink(&outside, root.join("linked.toml")).unwrap();
        assert!(write_template(&mut cache, &root, "linked", "bad").is_err());
        assert_eq!(std::fs::read_to_string(outside).unwrap(), "safe");
        for bad in ["../escape", "a/b", ".hidden", ""] {
            assert!(
                write_template(&mut cache, &root, bad, "bad").is_err(),
                "{bad}"
            );
        }
    }
}
