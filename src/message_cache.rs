//! Bounded UTF-8 connection-file snapshots, refreshed off the world executor.
use crate::config::Config;
use anyhow::{Context, Result, ensure};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
};

/// The five C file-cache roles used for connection admission and closure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum File {
    Connect,
    BadSite,
    Down,
    Full,
    Quit,
}
const FILES: [File; 5] = [
    File::Connect,
    File::BadSite,
    File::Down,
    File::Full,
    File::Quit,
];
impl File {
    fn name(self) -> &'static str {
        match self {
            Self::Connect => "Conn",
            Self::BadSite => "Conn/Badsite",
            Self::Down => "Conn/Down",
            Self::Full => "Conn/Full",
            Self::Quit => "Quit",
        }
    }
    fn path(self, c: &Config) -> PathBuf {
        c.path(match self {
            Self::Connect => &c.mux.connect_file,
            Self::BadSite => &c.mux.badsite_file,
            Self::Down => &c.mux.down_file,
            Self::Full => &c.mux.full_file,
            Self::Quit => &c.mux.quit_file,
        })
    }
}
/// Immutable after publication; failed replacements retain last-good entries.
#[derive(Clone, Default)]
pub struct MessageCache {
    entries: BTreeMap<File, String>,
    banners: BTreeMap<PathBuf, String>,
}
impl MessageCache {
    pub fn text(&self, file: File) -> &str {
        self.entries.get(&file).map(String::as_str).unwrap_or("")
    }
    /// Selection is injectable for deterministic tests; callers provide a uniform index.
    pub fn welcome(&self, index: usize) -> &str {
        self.banners
            .values()
            .nth(index)
            .map(String::as_str)
            .unwrap_or_else(|| self.text(File::Connect))
    }
    pub fn banner_count(&self) -> usize {
        self.banners.len()
    }
    /// Build one bounded replacement; directory traversal failure retains the collection.
    pub fn reload(&self, c: &Config) -> (Self, Vec<String>) {
        let mut candidate = self.clone();
        let mut report = Vec::new();
        let limit = c.lua.output_byte_limit;
        for file in FILES {
            let path = file.path(c);
            match read(&path, limit) {
                Ok(text) => {
                    report.push(format!(
                        "{}...{}",
                        file.name(),
                        crate::telnet::encode(&text).len()
                    ));
                    candidate.entries.insert(file, text);
                }
                Err(e) => report.push(format!("{}...retained ({e:#})", file.name())),
            }
        }
        if c.mux.connect_dir.as_os_str().is_empty() {
            candidate.banners.clear();
        } else {
            let dir = c.path(&c.mux.connect_dir);
            let paths = (|| -> Result<Vec<PathBuf>> {
                let mut paths = Vec::new();
                for entry in
                    std::fs::read_dir(&dir).with_context(|| format!("{}", dir.display()))?
                {
                    let entry = entry?;
                    let name = entry.file_name();
                    let name = name.to_string_lossy();
                    if !name.starts_with('.')
                        && name.contains(".txt")
                        && entry.file_type()?.is_file()
                    {
                        paths.push(entry.path());
                        paths.sort();
                        paths.truncate(100);
                    }
                }
                paths.sort();
                paths.truncate(100);
                Ok(paths)
            })();
            match paths {
                Err(e) => report.push(format!("Banners retained: {e:#}")),
                Ok(paths) => {
                    candidate.banners.retain(|path, _| paths.contains(path));
                    for path in paths {
                        match read(&path, limit) {
                            Ok(text) => {
                                candidate.banners.insert(path, text);
                            }
                            Err(e) => report.push(format!("Banner retained: {e:#}")),
                        }
                    }
                }
            }
        }
        let size: usize = candidate
            .entries
            .values()
            .chain(candidate.banners.values())
            .map(String::len)
            .sum();
        if size > limit {
            report.push("Cache unchanged: aggregate output-byte budget exceeded".into());
            return (self.clone(), report);
        }
        report.push(format!("Banners...{}", candidate.banner_count()));
        (candidate, report)
    }
}
/// Read at most the existing aggregate budget and normalize C file-cache newline semantics.
fn read(path: &Path, limit: usize) -> Result<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .with_context(|| format!("{}", path.display()))?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= limit,
        "{} exceeds output-byte budget",
        path.display()
    );
    let text =
        String::from_utf8(bytes).with_context(|| format!("{}: invalid UTF-8", path.display()))?;
    Ok(text.chars().filter(|c| !matches!(c, '\r' | '\0')).collect())
}
