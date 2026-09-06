//! Metadata indexing, live article reads and typed help responses shared by command/rendering code.
mod render;
use crate::{config::Config, text::Document};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::io::Read;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

/// Supported legacy generated-index layouts.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexStyle {
    #[default]
    ListWithDescription,
    Columnar,
}

/// Required front matter and cached lookup/visibility attributes.
#[derive(Clone, Debug, Deserialize)]
struct Metadata {
    title: String,
    description: String,
    keywords: Vec<String>,
    #[serde(default)]
    article_tags: Vec<String>,
    #[serde(default)]
    show_index_for_article_tags: Vec<String>,
    #[serde(default)]
    index_style: IndexStyle,
    #[serde(default)]
    weight: Option<i64>,
    #[serde(default)]
    wizard_only: bool,
}

/// Stable root-relative identity and metadata; bodies are read when requested.
#[derive(Clone, Debug)]
struct Article {
    meta: Metadata,
    path: String,
}

/// Diagnostics from a completed candidate build, including individually skipped files.
#[derive(Clone, Debug, Default)]
pub struct HelpLoadReport {
    /// Number of valid indexed articles, including articles with duplicate keywords.
    pub articles: usize,
    /// Number of distinct keyword mappings retained after duplicate resolution.
    pub keywords: usize,
    /// Contextual diagnostics for skipped malformed/unreadable files.
    pub errors: Vec<String>,
    /// Nonfatal duplicate-keyword diagnostics with the winning source.
    pub warnings: Vec<String>,
}

impl HelpLoadReport {
    /// Legacy administrative completion summary.
    pub fn summary(&self) -> String {
        format!(
            "Help reindexed: {} article(s), {} keyword(s), {} error(s), {} warning(s).",
            self.articles,
            self.keywords,
            self.errors.len(),
            self.warnings.len()
        )
    }

    /// Record details without exposing filesystem diagnostics to ordinary players.
    pub fn log(&self) {
        for detail in self.errors.iter().chain(&self.warnings) {
            eprintln!("Help: {detail}");
        }
        eprintln!("{}", self.summary());
    }
}

/// A permission-filtered index entry, independent of Markdown syntax.
#[derive(Clone, Debug)]
pub struct HelpEntry {
    /// Primary keyword displayed and sent by index action links.
    pub topic: String,
    /// Literal front-matter summary displayed alongside the topic.
    pub description: String,
}

/// Resolved help content with semantic index entries for recipient-specific layout.
#[derive(Clone, Debug)]
pub enum HelpResponse {
    Message(String),
    Article {
        body: String,
        path: String,
        entries: Vec<HelpEntry>,
        style: IndexStyle,
    },
}

/// Immutable metadata snapshot replaced only after a successful reload build.
#[derive(Clone, Debug, Default)]
pub struct HelpIndex {
    root: PathBuf,
    text_limit: usize,
    articles: Vec<Article>,
    keywords: BTreeMap<String, usize>,
    /// Build results associated with this installed metadata snapshot.
    pub report: HelpLoadReport,
}

/// Read bounded UTF-8 source and require front-matter delimiters on complete lines.
fn read_article(path: &Path, limit: usize) -> Result<(String, String)> {
    ensure!(
        std::fs::metadata(path)?.len() <= limit as u64,
        "help article exceeds text budget"
    );
    let mut source = String::new();
    std::fs::File::open(path)?
        .take(limit.saturating_add(1) as u64)
        .read_to_string(&mut source)?;
    ensure!(source.len() <= limit, "help article exceeds text budget");
    let source = source.replace("\r\n", "\n");
    let source = source
        .strip_prefix("+++\n")
        .context("missing TOML front matter")?;
    let end = source
        .lines()
        .scan(0usize, |offset, line| {
            let start = *offset;
            *offset += line.len() + 1;
            Some((start, line))
        })
        .find(|(_, line)| *line == "+++")
        .map(|(offset, _)| offset)
        .context("unclosed TOML front matter")?;
    let body = source[end + 3..].trim_start_matches('\n').to_string();
    Document::markdown(body.clone(), limit)?;
    Ok((source[..end].to_string(), body))
}

/// Traverse once in lexical order; traversal errors abort the candidate build.
fn files(dir: &Path, paths: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| dir.display().to_string())? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            files(&entry.path(), paths)?;
        } else if kind.is_file() && entry.path().extension().is_some_and(|e| e == "md") {
            paths.push(entry.path());
        }
    }
    Ok(())
}

impl HelpIndex {
    /// Startup tolerates absent help content, retaining fixture/fresh-world behavior.
    pub fn load(config: &Config) -> Result<Self> {
        let root = config.root.join(&config.mux.help_directory);
        if !root.exists() {
            let index = Self {
                root,
                text_limit: config.lua.output_byte_limit,
                ..Self::default()
            };
            index.report.log();
            return Ok(index);
        }
        let index = Self::reload(config)?;
        index.report.log();
        Ok(index)
    }

    /// Build a new index; callers install it only after this operation succeeds.
    pub fn reload(config: &Config) -> Result<Self> {
        let root = config
            .root
            .join(&config.mux.help_directory)
            .canonicalize()
            .context("opening help directory")?;
        let mut index = Self {
            root,
            text_limit: config.lua.output_byte_limit,
            ..Self::default()
        };
        let mut paths = Vec::new();
        files(&index.root, &mut paths)?;
        paths.sort();
        for path in paths {
            let load = || -> Result<Article> {
                let (front, _) = read_article(&path, index.text_limit)?;
                let meta: Metadata = toml::from_str(&front)?;
                ensure!(
                    !meta.title.trim().is_empty()
                        && !meta.description.trim().is_empty()
                        && !meta.keywords.is_empty()
                        && meta.keywords.iter().all(|s| !s.trim().is_empty()),
                    "title, description and keywords are required"
                );
                Ok(Article {
                    meta,
                    path: path
                        .strip_prefix(&index.root)?
                        .to_string_lossy()
                        .replace('\\', "/"),
                })
            };
            match load().with_context(|| path.display().to_string()) {
                Ok(article) => index.articles.push(article),
                Err(error) => index.report.errors.push(format!("{error:#}")),
            }
        }
        for (i, article) in index.articles.iter().enumerate() {
            for keyword in &article.meta.keywords {
                let keyword = keyword.to_ascii_lowercase();
                match index.keywords.entry(keyword.clone()) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(i);
                    }
                    std::collections::btree_map::Entry::Occupied(entry) => {
                        index.report.warnings.push(format!(
                            "keyword '{keyword}' declared by both '{}' and '{}'; '{}' wins",
                            index.articles[*entry.get()].path,
                            article.path,
                            index.articles[*entry.get()].path
                        ));
                    }
                }
            }
        }
        index.report.articles = index.articles.len();
        index.report.keywords = index.keywords.len();
        Ok(index)
    }

    /// Resolve metadata first, then read only the authorized article's current body.
    /// This filesystem operation must run on a blocking worker during serving.
    pub fn lookup(&self, topic: &str, wizard: bool) -> Result<HelpResponse> {
        let topic = topic.trim().to_ascii_lowercase();
        let article = if topic.is_empty() {
            self.articles.iter().find(|a| a.path == "index.md")
        } else {
            self.keywords
                .get(&topic)
                .map(|&i| &self.articles[i])
                .or_else(|| {
                    self.articles
                        .iter()
                        .find(|a| a.path.trim_end_matches(".md").eq_ignore_ascii_case(&topic))
                })
        }
        .filter(|a| wizard || !a.meta.wizard_only);
        if let Some(article) = article {
            let path = self
                .root
                .join(&article.path)
                .canonicalize()
                .with_context(|| article.path.clone())?;
            ensure!(
                path.starts_with(&self.root),
                "help article outside help root"
            );
            let (_, body) =
                read_article(&path, self.text_limit).with_context(|| article.path.clone())?;
            let mut entries: Vec<_> = self
                .articles
                .iter()
                .filter(|entry| {
                    entry.path != article.path
                        && (wizard || !entry.meta.wizard_only)
                        && entry
                            .meta
                            .article_tags
                            .iter()
                            .any(|tag| article.meta.show_index_for_article_tags.contains(tag))
                })
                .collect();
            entries.sort_by(|a, b| {
                a.meta
                    .weight
                    .is_none()
                    .cmp(&b.meta.weight.is_none())
                    .then(a.meta.weight.cmp(&b.meta.weight))
                    .then_with(|| {
                        if a.meta.weight.is_some() {
                            a.meta.keywords[0]
                                .to_ascii_lowercase()
                                .cmp(&b.meta.keywords[0].to_ascii_lowercase())
                        } else {
                            a.meta
                                .article_tags
                                .first()
                                .map(|s| s.to_ascii_lowercase())
                                .cmp(&b.meta.article_tags.first().map(|s| s.to_ascii_lowercase()))
                        }
                    })
                    .then(a.path.cmp(&b.path))
            });
            return Ok(HelpResponse::Article {
                body,
                path: article.path.clone(),
                style: article.meta.index_style,
                entries: entries
                    .into_iter()
                    .map(|entry| HelpEntry {
                        topic: entry.meta.keywords[0].clone(),
                        description: entry.meta.description.clone(),
                    })
                    .collect(),
            });
        }
        let suggestions: Vec<_> = self
            .keywords
            .iter()
            .filter(|(k, i)| k.contains(&topic) && (wizard || !self.articles[**i].meta.wizard_only))
            .map(|(k, _)| k.as_str())
            .collect();
        Ok(HelpResponse::Message(if topic.is_empty() {
            "Unable to render default help article".into()
        } else if suggestions.is_empty() {
            format!("No help found for '{topic}'.")
        } else {
            format!(
                "No exact match for '{topic}'. Did you mean:\n{}",
                suggestions.join("  ")
            )
        }))
    }
}
