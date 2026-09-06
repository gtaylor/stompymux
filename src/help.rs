//! Startup help index: TOML metadata, permission-filtered lookup and Markdown bodies.
use crate::{config::Config, text::Document};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{collections::BTreeMap, path::Path};

/// Required article front matter and optional index presentation.
#[derive(Clone, Debug, Deserialize)]
struct Metadata {
    title: String,
    description: String,
    keywords: Vec<String>,
    #[serde(default)]
    article_tags: Vec<String>,
    #[serde(default)]
    show_index_for_article_tags: Vec<String>,
    #[serde(default = "default_style")]
    index_style: String,
    #[serde(default)]
    weight: Option<i64>,
    #[serde(default)]
    wizard_only: bool,
}

/// Legacy default for generated topic indexes.
fn default_style() -> String {
    "list_with_description".into()
}

/// Loaded source and metadata with a stable relative identity.
#[derive(Clone, Debug)]
struct Article {
    meta: Metadata,
    body: String,
    path: String,
}

/// Immutable index; authorization is evaluated for each request, not cached.
#[derive(Default)]
pub struct HelpIndex {
    articles: Vec<Article>,
    keywords: BTreeMap<String, usize>,
}

impl HelpIndex {
    /// Read articles in lexical order and diagnose malformed files.
    pub fn load(config: &Config) -> Result<Self> {
        let root = config.root.join(&config.mux.help_directory);
        let mut index = Self::default();
        if !root.exists() {
            return Ok(index);
        }

        /// Collect regular help files without following symlinks.
        fn walk(root: &Path, dir: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<()> {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let p = entry.path();
                if entry.file_type()?.is_symlink() {
                    continue;
                }
                if p.is_dir() {
                    walk(root, &p, out)?;
                } else if p.extension().is_some_and(|s| s == "md") {
                    ensure!(p.starts_with(root), "help file outside root");
                    out.push(p);
                }
            }
            Ok(())
        }
        let mut paths = Vec::new();
        walk(&root, &root, &mut paths)?;
        paths.sort();
        for path in paths {
            let load = || -> Result<Article> {
                ensure!(
                    std::fs::metadata(&path)?.len() <= config.lua.output_byte_limit as u64,
                    "help article exceeds text budget"
                );
                let source = std::fs::read_to_string(&path)?;
                ensure!(
                    source.len() <= config.lua.output_byte_limit,
                    "help article exceeds text budget"
                );
                let source = source.replace("\r\n", "\n");
                let source = source
                    .strip_prefix("+++\n")
                    .context("missing TOML front matter")?;
                let (meta, body) = source
                    .split_once("\n+++")
                    .context("unclosed TOML front matter")?;
                let meta: Metadata = toml::from_str(meta)?;
                ensure!(
                    !meta.title.is_empty()
                        && !meta.description.is_empty()
                        && !meta.keywords.is_empty()
                        && meta.keywords.iter().all(|s| !s.trim().is_empty()),
                    "title, description and keywords are required"
                );
                ensure!(
                    matches!(
                        meta.index_style.as_str(),
                        "list_with_description" | "columnar"
                    ),
                    "invalid index_style"
                );
                Document::markdown(body.into(), config.lua.output_byte_limit)?;
                Ok(Article {
                    meta,
                    body: body.trim_start_matches('\n').into(),
                    path: path
                        .strip_prefix(&root)?
                        .to_string_lossy()
                        .replace('\\', "/"),
                })
            };
            match load().with_context(|| path.display().to_string()) {
                Ok(a) => index.articles.push(a),
                Err(e) => eprintln!("Help: {e:#}"),
            }
        }
        for (i, a) in index.articles.iter().enumerate() {
            for k in &a.meta.keywords {
                let k = k.to_ascii_lowercase();
                if let std::collections::btree_map::Entry::Vacant(e) =
                    index.keywords.entry(k.clone())
                {
                    e.insert(i);
                } else {
                    eprintln!("Help: {}: duplicate keyword {k}", a.path);
                }
            }
        }
        Ok(index)
    }

    pub fn lookup(&self, topic: &str, wizard: bool) -> Document {
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
        if let Some(a) = article {
            if a.meta.show_index_for_article_tags.is_empty() {
                return Document::Markdown(a.body.clone());
            }
            let mut entries: Vec<_> = self
                .articles
                .iter()
                .filter(|entry| {
                    (wizard || !entry.meta.wizard_only)
                        && entry
                            .meta
                            .article_tags
                            .iter()
                            .any(|tag| a.meta.show_index_for_article_tags.contains(tag))
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
            let mut body = a.body.clone();
            body.push_str("\n\n");
            if a.meta.index_style == "columnar" {
                body.push_str("| Topic | Topic | Topic |\n| --- | --- | --- |\n");
                for chunk in entries.chunks(3) {
                    body.push('|');
                    for i in 0..3 {
                        if let Some(entry) = chunk.get(i) {
                            body.push_str(&format!(
                                " [{}]({}) |",
                                md_escape(&entry.meta.keywords[0]),
                                entry.path
                            ));
                        } else {
                            body.push_str(" | ");
                        }
                    }
                    body.push('\n');
                }
            } else {
                for e in entries {
                    body.push_str(&format!(
                        "- [{}]({}): {}\n",
                        md_escape(&e.meta.keywords[0]),
                        e.path,
                        md_escape(&e.meta.description)
                    ));
                }
            }
            return Document::Markdown(body);
        }
        let suggestions = self
            .keywords
            .iter()
            .filter(|(k, i)| k.contains(&topic) && (wizard || !self.articles[**i].meta.wizard_only))
            .map(|(k, _)| k.as_str())
            .collect::<Vec<_>>();
        Document::Literal(if topic.is_empty() {
            "Unable to render default help article".into()
        } else if suggestions.is_empty() {
            format!("No help found for '{topic}'.")
        } else {
            format!(
                "No exact match for '{topic}'. Did you mean:\n{}",
                suggestions.join("  ")
            )
        })
    }
}

/// Quote generated index labels as literal Markdown text.
fn md_escape(s: &str) -> String {
    s.chars()
        .map(|c| {
            if "\\`*_{}[]<>()#+-.!|".contains(c) {
                format!("\\{c}")
            } else {
                c.to_string()
            }
        })
        .collect()
}
