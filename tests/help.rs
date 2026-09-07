//! C help-index compatibility, supplied corpus, live content and bounded rich delivery.
use std::path::Path;
use stompymux_rs::{
    config::Config,
    help::{HelpIndex, HelpResponse},
    text::{self, Document, Palette, RenderOptions, Span},
};
use unicode_width::UnicodeWidthStr;

/// Isolated configurable help tree without a live database.
fn fixture() -> (tempfile::TempDir, Config) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("help")).unwrap();
    std::fs::write(dir.path().join("stompymux.toml"), "").unwrap();
    let config = Config::load(dir.path()).unwrap();
    (dir, config)
}

/// Author a fixture with required metadata plus optional index/visibility fields.
fn article(root: &Path, path: &str, topic: &str, extra: &str, body: &str) {
    let path = root.join("help").join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        path,
        format!(
            "+++\ntitle={topic:?}\ndescription={:?}\nkeywords=[{topic:?}]\n{extra}\n+++\n{body}",
            format!("{topic} description")
        ),
    )
    .unwrap();
}

/// Inspect semantic visible content without treating bracket examples as markup.
fn visible(spans: &[Span]) -> String {
    spans.iter().map(|s| s.text.as_str()).collect()
}

/// Every supplied keyword resolves, including privileged references for implemented/deferred commands.
#[test]
fn supplied_corpus_is_reachable_and_renderable() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("game");
    let config = Config::load(&root).unwrap();
    let index = HelpIndex::load(&config).unwrap();
    assert_eq!(index.report.articles, 97);
    assert!(index.report.errors.is_empty(), "{:?}", index.report);
    assert!(index.report.warnings.is_empty(), "{:?}", index.report);
    fn walk(path: &Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(path).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|e| e == "md") {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    walk(&root.join("help"), &mut files);
    for file in files {
        let source = std::fs::read_to_string(&file).unwrap();
        let metadata: toml::Value = toml::from_str(source.split("+++").nth(1).unwrap()).unwrap();
        for keyword in metadata["keywords"].as_array().unwrap() {
            let keyword = keyword.as_str().unwrap();
            let response = index.lookup(keyword, true).unwrap();
            assert!(
                matches!(response, HelpResponse::Article { .. }),
                "{keyword}"
            );
            for width in [24, 80] {
                let options = RenderOptions {
                    width,
                    ..RenderOptions::all()
                };
                let spans = response.spans(&options);
                let chunks = text::telnet_chunks(
                    &spans,
                    &Palette::default(),
                    &options,
                    4096,
                    config.lua.output_byte_limit,
                )
                .unwrap();
                assert!(!chunks.is_empty(), "{keyword}");
                assert!(chunks.iter().all(|c| c.len() <= 4096));
            }
            assert!(
                !response
                    .html(config.lua.output_byte_limit)
                    .unwrap()
                    .is_empty()
            );
            if metadata.get("wizard_only").and_then(toml::Value::as_bool) == Some(true) {
                assert!(
                    matches!(
                        index.lookup(keyword, false).unwrap(),
                        HelpResponse::Message(_)
                    ),
                    "{keyword}"
                );
            }
        }
    }
}

/// C help_render.c uses twenty-column topic fields and excludes the index itself.
#[test]
fn index_layout_and_visibility_match_legacy() {
    let (d, c) = fixture();
    article(
        d.path(),
        "index.md",
        "help",
        "article_tags=['root']\nshow_index_for_article_tags=['root']",
        "# Help\n",
    );
    article(
        d.path(),
        "a.md",
        "Alpha",
        "article_tags=['root']\nweight=1",
        "Alpha",
    );
    article(
        d.path(),
        "b.md",
        "Beta",
        "article_tags=['root']\nweight=2",
        "Beta",
    );
    article(
        d.path(),
        "s.md",
        "Secret",
        "article_tags=['root']\nweight=0\nwizard_only=true",
        "secret",
    );
    let index = HelpIndex::load(&c).unwrap();
    let response = index.lookup("", false).unwrap();
    let spans = response.spans(&RenderOptions::default());
    let output = visible(&spans);
    assert!(output.contains("TOPIC                DESCRIPTION\nAlpha                Alpha description\nBeta                 Beta description\n"), "{output}");
    assert!(!output.contains("help description"));
    assert!(!output.contains("Secret"));
    assert!(output.contains("Type help <topic>"));
    assert!(
        spans
            .iter()
            .any(|s| s.link.as_ref().is_some_and(|l| l.target == "help Alpha"))
    );
    let wizard = visible(&index.lookup("", true).unwrap().spans(&RenderOptions::all()));
    assert!(wizard.find("Secret").unwrap() < wizard.find("Alpha").unwrap());
    assert!(!wizard.contains("Type help <topic>"));
    assert!(matches!(
        index.lookup("secret", false).unwrap(),
        HelpResponse::Message(_)
    ));
    for width in [8, 24, 32] {
        let output = visible(&response.spans(&RenderOptions {
            width,
            ..Default::default()
        }));
        assert!(
            output
                .lines()
                .all(|line| UnicodeWidthStr::width(line) <= width),
            "{width}: {output}"
        );
    }
    article(
        d.path(),
        "index.md",
        "help",
        "show_index_for_article_tags=['root']\nindex_style='columnar'",
        "# Help\n",
    );
    let index = HelpIndex::reload(&c).unwrap();
    let wide = visible(
        &index
            .lookup("", false)
            .unwrap()
            .spans(&RenderOptions::all()),
    );
    assert!(wide.contains("Alpha               Beta\n"), "{wide}");
    let narrow = visible(&index.lookup("", false).unwrap().spans(&RenderOptions {
        width: 20,
        ..RenderOptions::all()
    }));
    assert!(narrow.contains("Alpha\nBeta\n"), "{narrow}");
}

/// Reload changes metadata; ordinary reads see body edits but cannot bypass cached visibility.
#[test]
fn live_bodies_reload_reports_and_failed_candidates() {
    let (d, c) = fixture();
    article(d.path(), "index.md", "help", "", "initial");
    article(
        d.path(),
        "secret.md",
        "secret",
        "wizard_only=true",
        "old secret",
    );
    let index = HelpIndex::load(&c).unwrap();
    article(d.path(), "index.md", "new-name", "", "edited live");
    assert!(
        visible(
            &index
                .lookup("help", false)
                .unwrap()
                .spans(&RenderOptions::default())
        )
        .contains("edited live")
    );
    assert!(matches!(
        index.lookup("new-name", false).unwrap(),
        HelpResponse::Message(_)
    ));
    article(
        d.path(),
        "secret.md",
        "secret",
        "",
        "now public after reload",
    );
    assert!(matches!(
        index.lookup("secret", false).unwrap(),
        HelpResponse::Message(_)
    ));
    article(d.path(), "z-duplicate.md", "new-name", "", "duplicate");
    std::fs::write(d.path().join("help/bad.md"), "+++\nbogus=true\n+++\nbody").unwrap();
    let next = HelpIndex::reload(&c).unwrap();
    assert_eq!(next.report.articles, 3);
    assert_eq!(next.report.errors.len(), 1);
    assert_eq!(next.report.warnings.len(), 1);
    assert!(next.report.errors[0].contains("bad.md"));
    assert!(next.report.warnings[0].contains("index.md"));
    assert!(matches!(
        next.lookup("secret", false).unwrap(),
        HelpResponse::Article { .. }
    ));
    assert!(
        visible(
            &next
                .lookup("new-name", false)
                .unwrap()
                .spans(&RenderOptions::default())
        )
        .contains("edited live")
    );
    std::fs::remove_file(d.path().join("help/secret.md")).unwrap();
    assert!(next.lookup("secret", true).is_err());
    assert_eq!(HelpIndex::reload(&c).unwrap().report.articles, 2);
    std::fs::rename(d.path().join("help"), d.path().join("away")).unwrap();
    assert!(HelpIndex::reload(&c).is_err());
    assert_eq!(next.report.articles, 3);
}

/// Shared Markdown blocks retain literal examples and nested continuation prefixes.
#[test]
fn markdown_block_layout_and_article_relative_links() {
    let source = "- First item with words that wrap\n  - Nested item with more words\n\n> A quote that wraps over several lines\n>\n> Next paragraph\n\n```text\n  [fg=red]literal[/]\n```\n";
    let options = RenderOptions {
        width: 20,
        ..Default::default()
    };
    let output = visible(&text::markdown::spans(source, &options));
    assert!(
        output.contains("- First item with\n  words that wrap\n"),
        "{output}"
    );
    assert!(
        output.contains("  - Nested item with\n    more words"),
        "{output}"
    );
    assert!(
        output.contains("> A quote that wraps\n> over several lines"),
        "{output}"
    );
    assert!(output.contains("> Next paragraph"), "{output}");
    assert!(output.contains("  [fg=red]literal[/]"));
    let body = "[Other](../other.md) [Unsafe](../../../outside.md) [Bad](javascript:alert) [Code](help:flags) [Space](../space%20topic.md) [Control](../bad%0Atopic.md)";
    let spans = text::markdown::spans_at(body, &options, Some("nested/page.md"));
    let targets: Vec<_> = spans
        .iter()
        .filter_map(|s| s.link.as_ref().map(|l| l.target.as_str()))
        .collect();
    assert!(targets.contains(&"help other"));
    assert!(targets.contains(&"help flags"));
    assert!(targets.contains(&"help space topic"));
    assert!(!targets.iter().any(|target| target.contains('\n')));
    assert!(
        !targets
            .iter()
            .any(|t| t.contains("outside") || t.contains("javascript"))
    );
    let html = text::markdown::html_at(body, Some("nested/page.md"));
    assert!(html.contains("data-mux-command=\"help other\""));
    assert!(!html.contains("javascript:"));
}

/// Independent ANSI/OSC chunks preserve every grapheme and reserve all closing bytes.
#[test]
fn complete_chunks_preserve_text_and_report_impossible_limits() {
    let p = Palette::default();
    let mut options = RenderOptions::all();
    options.capabilities.remove("STYLE_BASIC");
    let doc = Document::Styled(format!(
        "[send=\"help flags\" bold]{}[/]",
        "e\u{301}界👩‍💻 ".repeat(200)
    ));
    let spans = doc.spans(&p, &options);
    let chunks = text::telnet_chunks(&spans, &p, &options, 128, 65536).unwrap();
    assert!(chunks.len() > 10);
    let decoded: String = chunks
        .iter()
        .map(|chunk| {
            assert!(chunk.len() <= 128);
            let s = std::str::from_utf8(chunk).unwrap();
            assert!(s.ends_with("\x1b]8;;\x1b\\\x1b[0m"));
            visible(&Document::Literal(s.into()).spans(&p, &options))
        })
        .collect();
    assert_eq!(decoded, visible(&spans));
    assert!(text::telnet_chunks(&spans, &p, &options, 8, 65536).is_err());
    assert!(text::telnet_chunks(&spans, &p, &options, 128, 100).is_err());
}

/// A stalled client cannot hold the world owner indefinitely while help fills its queue.
#[tokio::test(flavor = "current_thread")]
async fn slow_help_queue_obeys_deadline_and_counters() {
    use stompymux_rs::sessions::{LoginFlow, Session};
    let (dir, _) = fixture();
    std::fs::write(
        dir.path().join("stompymux.toml"),
        "[runtime]\nwrite_timeout_ms=10\noutput_message_limit=64\n",
    )
    .unwrap();
    let config = Config::load(dir.path()).unwrap();
    let (output, _receiver) = tokio::sync::mpsc::channel(1);
    let now = std::time::Instant::now();
    let session = Session {
        site: Default::default(),
        output,
        stats: Default::default(),
        palette: Default::default(),
        color_override: Default::default(),
        presets_emitted: Default::default(),
        peer: "127.0.0.1".parse().unwrap(),
        player: None,
        flow: LoginFlow::Name,
        connected: now,
        active: now,
        decoder: Default::default(),
        quota: 1,
        quota_at: now,
        failed: Default::default(),
        output_message_limit: 64,
    };
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        session.help(&HelpResponse::Message("word ".repeat(500)), false, &config),
    )
    .await
    .unwrap();
    assert!(result.is_err());
    assert!(session.failed.get());
    let [pending, lost, total] = session.stats.snapshot().output;
    assert!(pending > 0 && pending <= 64);
    assert!(lost > 0 && lost <= 64);
    assert_eq!(total, pending + lost);
}
