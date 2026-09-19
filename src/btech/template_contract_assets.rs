//! Confined, bounded template lookup matching the legacy root and immediate-subdirectory registry.

use super::{BattleUnitTemplate, RawTemplate};
use anyhow::{Result, ensure};
use std::{
    collections::HashMap,
    io::Write,
    os::unix::ffi::OsStringExt,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    name: Vec<u8>,
    directory: Option<Vec<u8>>,
}
/// One BattleTech context's lazily populated legacy template registry.
#[derive(Clone, Debug, Default)]
pub struct TemplateRegistryCache {
    roots: HashMap<PathBuf, Vec<Entry>>,
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
fn key(name: &[u8]) -> Vec<u8> {
    name.iter().take(24).map(u8::to_ascii_lowercase).collect()
}
fn scan(root: &Path) -> Result<Vec<Entry>> {
    let mut templates = Vec::new();
    let mut directories = Vec::new();
    for item in std::fs::read_dir(root)? {
        let item = item?;
        let ty = item.metadata()?;
        let name = item.file_name();
        let bytes = name.as_encoded_bytes();
        if ty.is_dir() && !bytes.starts_with(b".") && bytes.len() <= 34 {
            directories.push(item.path())
        } else if ty.is_file() {
            templates.push(Entry {
                name: bytes[..bytes.len().min(34)].to_vec(),
                directory: None,
            });
        }
    }
    directories.reverse();
    for directory in directories {
        for item in std::fs::read_dir(&directory)? {
            let item = item?;
            if item.metadata()?.is_file() {
                let name = item.file_name();
                let bytes = name.as_encoded_bytes();
                templates.push(Entry {
                    name: bytes[..bytes.len().min(34)].to_vec(),
                    directory: directory
                        .file_name()
                        .map(|name| name.as_encoded_bytes().to_vec()),
                });
            }
        }
    }
    heap_sort(&mut templates);
    Ok(templates)
}

fn heap_sort(entries: &mut [Entry]) {
    fn sift(entries: &mut [Entry], mut root: usize, end: usize) {
        while root < end && root <= (end - 1) / 2 {
            let mut child = root * 2 + 1;
            if child < end && key(&entries[child].name) < key(&entries[child + 1].name) {
                child += 1;
            }
            if key(&entries[root].name) >= key(&entries[child].name) {
                return;
            }
            entries.swap(root, child);
            root = child;
        }
    }
    if entries.len() < 2 {
        return;
    }
    for start in (0..entries.len() / 2).rev() {
        sift(entries, start, entries.len() - 1);
    }
    for end in (1..entries.len()).rev() {
        entries.swap(0, end);
        sift(entries, 0, end - 1);
    }
}

fn search(entries: &[Entry], wanted: &[u8]) -> Option<usize> {
    let (mut first, mut remaining) = (0, entries.len());
    while remaining > 0 {
        let offset = remaining / 2;
        let index = first + offset;
        match wanted.cmp(&key(&entries[index].name)) {
            std::cmp::Ordering::Equal => return Some(index),
            std::cmp::Ordering::Less => remaining = offset,
            std::cmp::Ordering::Greater => {
                first = index + 1;
                remaining -= offset + 1;
            }
        }
    }
    None
}

fn old_style(root: &Path, reference: &[u8]) -> Option<PathBuf> {
    const SUBDIRECTORIES: &[&str] = &[
        "3025",
        "3050",
        "3055",
        "3058",
        "3060",
        "2750",
        "Aero",
        "MISC",
        "Clan",
        "ClanVehicles",
        "Clan2nd",
        "ClanAero",
        "Custom",
        "Solaris",
        "Vehicles",
        "MFNA",
        "Infantry",
    ];
    let reference = std::ffi::OsString::from_vec(reference.to_vec());
    std::iter::once(root.join(&reference))
        .chain(
            SUBDIRECTORIES
                .iter()
                .map(|directory| root.join(directory).join(&reference)),
        )
        .find(|path| std::fs::File::open(path).is_ok())
}

/// Find the cached case-insensitive, first-24-byte reference in the root or immediate subdirectories.
pub fn resolve_template_path_cached(
    cache: &mut TemplateRegistryCache,
    root: &Path,
    reference: &str,
) -> Result<Option<PathBuf>> {
    resolve_template_path_bytes_cached(cache, root, reference.as_bytes())
}

/// Resolve an opaque Lua/C reference without imposing UTF-8 validation.
pub fn resolve_template_path_bytes_cached(
    cache: &mut TemplateRegistryCache,
    root: &Path,
    reference: &[u8],
) -> Result<Option<PathBuf>> {
    let Ok(root) = root.canonicalize() else {
        return Ok(None);
    };
    let wanted = key(reference);
    for attempt in 0..2 {
        if !cache.roots.contains_key(&root) {
            let Ok(entries) = scan(&root) else {
                return Ok(old_style(&root, reference));
            };
            cache.roots.insert(root.clone(), entries);
        }
        let found = search(&cache.roots[&root], &wanted).map(|index| {
            let entry = &cache.roots[&root][index];
            let mut path = root.clone();
            if let Some(directory) = &entry.directory {
                path.push(std::ffi::OsString::from_vec(directory.clone()));
            }
            path.push(std::ffi::OsString::from_vec(entry.name.clone()));
            path
        });
        match found {
            Some(path) if std::fs::File::open(&path).is_ok() => return Ok(Some(path)),
            Some(_) if attempt == 0 => {
                cache.roots.remove(&root);
            }
            Some(_) => return Ok(old_style(&root, reference)),
            None => return Ok(None),
        }
    }
    Ok(None)
}

/// One-shot lookup for callers that deliberately do not retain a BattleTech context.
pub fn resolve_template_path(root: &Path, reference: &str) -> Result<Option<PathBuf>> {
    resolve_template_path_cached(&mut TemplateRegistryCache::default(), root, reference)
}

/// Read one already-resolved template with confinement and the shared one-megabyte bound.
pub fn read_resolved_template(root: &Path, path: &Path) -> Result<BattleUnitTemplate> {
    let root = root.canonicalize()?;
    let path = path.canonicalize()?;
    ensure!(path.starts_with(&root), "template path escapes its root");
    let metadata = std::fs::metadata(&path)?;
    ensure!(metadata.len() <= 1_048_576, "template exceeds size limit");
    BattleUnitTemplate::parse(&std::fs::read_to_string(path)?)
}

/// Read one resolved template into class-neutral contract state, applying the
/// native load finalize so every raw read reflects loaded-mech semantics.
pub fn read_resolved_raw_template(root: &Path, path: &Path) -> Result<RawTemplate> {
    let root = root.canonicalize()?;
    let path = path.canonicalize()?;
    ensure!(path.starts_with(&root), "template path escapes its root");
    let metadata = std::fs::metadata(&path)?;
    ensure!(metadata.len() <= 1_048_576, "template exceeds size limit");
    let mut template = RawTemplate::parse(&std::fs::read_to_string(path)?)?;
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
    let compact = template.sections.get(&CenterTorso).map_or(true, |section| {
        section
            .criticals
            .values()
            .filter(|critical| critical.equipment.eq_ignore_ascii_case("Engine"))
            .count()
            < 4
    });
    super::unit::edit_special(&mut template.attributes, "specials", "FlipArms", flippable);
    if compact {
        super::unit::edit_special(
            &mut template.attributes,
            "specials",
            "CompactEngine_Tech",
            true,
        );
    }
}

/// Write one direct-child template after invalidating the owning context's registry.
pub fn write_template(
    cache: &mut TemplateRegistryCache,
    root: &Path,
    reference: &str,
    source: &str,
) -> Result<()> {
    cache.clear();
    ensure!(
        !reference.is_empty()
            && reference != "."
            && reference != ".."
            && !reference.as_bytes().contains(&0)
            && !reference.contains('/')
            && !reference.contains('\\'),
        "invalid template reference"
    );
    let root = root.canonicalize()?;
    ensure!(root.is_dir(), "template root is not a directory");
    let path = root.join(reference);
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
        .custom_flags(0x20000)
        .open(&path)?;
    file.write_all(source.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_matches_first_24_bytes_and_refreshes_only_an_unreadable_hit() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        std::fs::create_dir(root.join("stock")).unwrap();
        let stored = "abcdefghijklmnopqrstuvwx-FIRST-LON";
        assert_eq!(stored.len(), 34);
        std::fs::write(root.join("stock").join(stored), "one").unwrap();
        let mut cache = TemplateRegistryCache::default();
        let path = resolve_template_path_cached(&mut cache, root, "ABCDEFGHIJKLMNOPQRSTUVWX-other")
            .unwrap()
            .unwrap();
        assert_eq!(
            path.file_name().unwrap().as_encoded_bytes(),
            stored.as_bytes()
        );
        assert_eq!(
            resolve_template_path_cached(&mut cache, root, "abcdefghijklmnopqrstuvwx")
                .unwrap()
                .unwrap(),
            path
        );
        std::fs::remove_file(&path).unwrap();
        assert!(
            resolve_template_path_cached(&mut cache, root, stored)
                .unwrap()
                .is_none()
        );

        let long = "abcdefghijklmnopqrstuvwx-SECOND-LONG-NAME";
        std::fs::write(root.join(long), "long").unwrap();
        let mut cache = TemplateRegistryCache::default();
        assert_eq!(
            resolve_template_path_cached(&mut cache, root, long).unwrap(),
            Some(root.join(long))
        );
    }

    #[test]
    fn registry_scans_only_admitted_immediate_directories() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        std::fs::create_dir(root.join("stock")).unwrap();
        std::fs::create_dir(root.join(".hidden")).unwrap();
        std::fs::create_dir(root.join("x".repeat(35))).unwrap();
        std::fs::create_dir_all(root.join("stock/deeper")).unwrap();
        std::fs::write(root.join("stock/Visible"), "x").unwrap();
        std::fs::write(root.join(".hidden/Hidden"), "x").unwrap();
        std::fs::write(root.join("x".repeat(35)).join("TooDeep"), "x").unwrap();
        std::fs::write(root.join("stock/deeper/Nested"), "x").unwrap();
        let mut cache = TemplateRegistryCache::default();
        assert!(
            resolve_template_path_cached(&mut cache, root, "visible")
                .unwrap()
                .is_some()
        );
        for omitted in ["Hidden", "TooDeep", "Nested"] {
            assert!(
                resolve_template_path_cached(&mut cache, root, omitted)
                    .unwrap()
                    .is_none()
            );
        }
    }

    #[test]
    fn heap_sort_and_midpoint_search_pin_duplicate_selection() {
        let mut entries = vec![
            Entry {
                name: b"abcdefghijklmnopqrstuvwx-a".to_vec(),
                directory: None,
            },
            Entry {
                name: b"other".to_vec(),
                directory: None,
            },
            Entry {
                name: b"abcdefghijklmnopqrstuvwx-b".to_vec(),
                directory: None,
            },
        ];
        heap_sort(&mut entries);
        let wanted = b"abcdefghijklmnopqrstuvwx";
        assert_eq!(search(&entries, wanted), Some(1));
        assert_eq!(key(&entries[1].name), wanted);
    }

    #[test]
    fn writer_invalidates_before_success_or_failure_and_rejects_symlinks() {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("templates");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("old"), "old").unwrap();
        let mut cache = TemplateRegistryCache::default();
        assert!(
            resolve_template_path_cached(&mut cache, &root, "old")
                .unwrap()
                .is_some()
        );
        write_template(&mut cache, &root, "new", "new").unwrap();
        assert!(
            resolve_template_path_cached(&mut cache, &root, "new")
                .unwrap()
                .is_some()
        );

        let outside = directory.path().join("outside");
        std::fs::write(&outside, "safe").unwrap();
        symlink(&outside, root.join("linked")).unwrap();
        assert!(write_template(&mut cache, &root, "linked", "bad").is_err());
        assert_eq!(std::fs::read_to_string(outside).unwrap(), "safe");
        assert!(write_template(&mut cache, &root, "../escape", "bad").is_err());
    }
}
