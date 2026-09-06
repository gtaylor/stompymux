//! CommonMark parsing delegated to pulldown-cmark; only layout is application-owned.
use super::{
    Span, Style,
    links::{self, Link, LinkConfig},
};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Enable only CommonMark, tables, task lists and strikethrough.
pub fn parse(s: &str) -> Parser<'_> {
    Parser::new_ext(
        s,
        Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH,
    )
}

/// Decode local navigation URLs without admitting control bytes or invalid UTF-8.
fn local_url(url: &str) -> Option<String> {
    let mut bytes = url.bytes();
    let mut decoded = Vec::new();
    while let Some(byte) = bytes.next() {
        decoded.push(if byte == b'%' {
            let high = (bytes.next()? as char).to_digit(16)?;
            let low = (bytes.next()? as char).to_digit(16)?;
            (high * 16 + low) as u8
        } else {
            byte
        });
    }
    let decoded = String::from_utf8(decoded).ok()?;
    if decoded.chars().any(char::is_control) {
        None
    } else {
        Some(decoded)
    }
}

/// Translate safe URLs and help destinations into shared actions.
fn link(url: &str, article: Option<&str>) -> Option<Link> {
    let (kind, target) = if links::external(url) {
        ("url", url.to_string())
    } else if let Some(topic) = url
        .strip_prefix("help:")
        .filter(|s| !s.is_empty() && !s.chars().any(char::is_control))
    {
        ("send", format!("help {}", local_url(topic)?))
    } else if !url.contains(':') && !url.starts_with("//") && !url.chars().any(char::is_control) {
        let url = local_url(url)?;
        let mut parts: Vec<&str> = if url.starts_with('/') {
            Vec::new()
        } else {
            article
                .and_then(|p| p.rsplit_once('/').map(|(dir, _)| dir))
                .map(|dir| dir.split('/').collect())
                .unwrap_or_default()
        };
        for component in url
            .trim_start_matches('/')
            .trim_end_matches(".md")
            .split('/')
        {
            match component {
                "" | "." => {}
                ".." => {
                    parts.pop()?;
                }
                other => parts.push(other),
            }
        }
        if parts.is_empty() {
            return None;
        }
        ("send", format!("help {}", parts.join("/")))
    } else {
        return None;
    };
    Some(Link {
        kind: if kind == "send" {
            links::LinkKind::Send
        } else {
            links::LinkKind::External
        },
        target,
        config: LinkConfig::default(),
    })
}

/// Keep Unicode word boundaries when possible, then split oversized words by grapheme.
pub(crate) fn wrap(spans: Vec<Span>, width: usize) -> Vec<Span> {
    let plain: String = spans.iter().map(|s| s.text.as_str()).collect();
    let mut breaks = std::collections::BTreeSet::new();
    let mut omitted = std::collections::BTreeSet::new();
    let mut pending = Vec::new();
    let mut pending_width = 0;
    let mut col = 0;
    for (offset, word) in plain.split_word_bound_indices() {
        if word.chars().all(char::is_whitespace) && !word.contains('\n') {
            pending.extend(word.char_indices().map(|(i, _)| offset + i));
            pending_width += UnicodeWidthStr::width(word);
            continue;
        }
        let w = UnicodeWidthStr::width(word);
        if word.contains('\n') {
            omitted.extend(std::mem::take(&mut pending));
            pending_width = 0;
            col = 0;
            continue;
        }
        if col > 0 && col + pending_width + w > width {
            omitted.extend(std::mem::take(&mut pending));
            breaks.insert(offset);
            col = 0;
        } else {
            col += pending_width;
            pending.clear();
        }
        pending_width = 0;
        for (i, g) in word.grapheme_indices(true) {
            let w = UnicodeWidthStr::width(g);
            if col + w > width && col > 0 {
                breaks.insert(offset + i);
                col = 0;
            }
            col += w;
        }
    }
    omitted.extend(pending);
    let mut offset = 0;
    spans
        .into_iter()
        .map(|mut span| {
            let mut text = String::new();
            for (i, c) in span.text.char_indices() {
                if breaks.contains(&(offset + i)) {
                    text.push('\n');
                }
                if !omitted.contains(&(offset + i)) {
                    text.push(c);
                }
            }
            offset += span.text.len();
            span.text = text;
            span
        })
        .collect()
}

/// Prefix complete display lines without letting prefixes inherit executable links.
fn prefixed(spans: Vec<Span>, first: &str, continuation: &str) -> Vec<Span> {
    let mut out = Vec::new();
    let mut start = true;
    let mut initial = true;
    for span in spans {
        for piece in span.text.split_inclusive('\n') {
            if start && piece != "\n" {
                out.push(Span {
                    text: if initial { first } else { continuation }.into(),
                    ..Span::default()
                });
                initial = false;
            }
            out.push(Span {
                text: piece.into(),
                ..span.clone()
            });
            start = piece.ends_with('\n');
        }
    }
    out
}

/// List markers and quote prefixes belong to block layout rather than inline text.
#[derive(Default)]
struct BlockLayout {
    quotes: usize,
    indents: Vec<usize>,
    marker: Option<String>,
}

impl BlockLayout {
    /// Flush a paragraph or literal code block while retaining its enclosing block prefixes.
    fn flush(&mut self, block: &mut Vec<Span>, out: &mut Vec<Span>, code: bool, width: usize) {
        if block.is_empty() {
            return;
        }
        let indent = self.indents.iter().sum::<usize>();
        let base = "> ".repeat(self.quotes);
        let continuation = format!("{base}{}", " ".repeat(indent));
        let first = if let Some(marker) = self.marker.take() {
            format!(
                "{base}{}{marker}",
                " ".repeat(indent.saturating_sub(marker.len()))
            )
        } else {
            continuation.clone()
        };
        let spans = std::mem::take(block);
        let available = width
            .saturating_sub(UnicodeWidthStr::width(continuation.as_str()))
            .max(1);
        // At extremely narrow widths, omit structural prefixes rather than losing content.
        let omit = UnicodeWidthStr::width(continuation.as_str()) >= width;
        out.extend(prefixed(
            if code { spans } else { wrap(spans, available) },
            if omit { "" } else { &first },
            if omit { "" } else { &continuation },
        ));
    }
}

/// Render Markdown blocks into styled runs without reparsing bracket examples.
pub fn spans(source: &str, options: &super::RenderOptions) -> Vec<Span> {
    spans_at(source, options, None)
}

/// Resolve relative navigation against an article's root-relative path.
pub fn spans_at(source: &str, options: &super::RenderOptions, article: Option<&str>) -> Vec<Span> {
    let width = options.width.max(1);
    let mut out = Vec::new();
    let mut block = Vec::new();
    let mut style = Style::default();
    let mut stack = Vec::new();
    let mut active_link = None;
    let mut code = false;
    let mut lists = Vec::new();
    let mut table: Option<Vec<Vec<Vec<Span>>>> = None;
    let mut table_alignment = Vec::new();
    let mut row = Vec::new();
    let mut cell = Vec::new();
    let mut in_cell = false;
    let mut layout = BlockLayout::default();
    for event in parse(source) {
        let flush_after = matches!(event, Event::End(TagEnd::Paragraph | TagEnd::Heading(_)));
        let mut text = None;
        match event {
            Event::Start(Tag::Table(alignment)) => {
                table_alignment = alignment;
                layout.flush(&mut block, &mut out, code, width);
                table = Some(Vec::new());
            }
            Event::Start(Tag::TableHead | Tag::TableRow) => row = Vec::new(),
            Event::Start(Tag::TableCell) => {
                cell = Vec::new();
                in_cell = true;
            }
            Event::End(TagEnd::TableCell) => {
                row.push(std::mem::take(&mut cell));
                in_cell = false;
            }
            Event::End(TagEnd::TableHead | TagEnd::TableRow) => {
                if let Some(t) = &mut table {
                    t.push(std::mem::take(&mut row));
                }
            }
            Event::End(TagEnd::Table) => {
                if let Some(t) = table.take() {
                    let inset = layout.quotes * 2 + layout.indents.iter().sum::<usize>();
                    let available = if inset >= width { width } else { width - inset };
                    let mut rendered = table_spans(t, available, &table_alignment);
                    layout.flush(&mut rendered, &mut out, true, width);
                }
            }
            Event::Start(Tag::Emphasis | Tag::Strong | Tag::Strikethrough) => {
                stack.push(style.clone());
                match event {
                    Event::Start(Tag::Emphasis) => style.italic = true,
                    Event::Start(Tag::Strong) => style.bold = true,
                    _ => style.strikethrough = true,
                }
            }
            Event::End(TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough) => {
                style = stack.pop().unwrap_or_default()
            }
            Event::Start(Tag::Heading { level, .. }) => {
                layout.flush(&mut block, &mut out, code, width);
                style.bold = true;
                text = Some(format!("{} ", "#".repeat(level as usize)));
            }
            Event::End(TagEnd::Heading(_)) => {
                style = Style::default();
                text = Some("\n\n".into());
            }
            Event::Start(Tag::CodeBlock(_)) => {
                layout.flush(&mut block, &mut out, code, width);
                code = true;
            }
            Event::End(TagEnd::CodeBlock) => {
                layout.flush(&mut block, &mut out, true, width);
                code = false;
                text = Some("\n".into());
            }
            Event::Start(Tag::BlockQuote(_)) => {
                layout.flush(&mut block, &mut out, code, width);
                layout.quotes += 1;
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                layout.flush(&mut block, &mut out, code, width);
                layout.quotes = layout.quotes.saturating_sub(1);
            }
            Event::Start(Tag::List(start)) => {
                if block
                    .last()
                    .is_some_and(|span: &Span| !span.text.ends_with('\n'))
                {
                    block.push(Span {
                        text: "\n".into(),
                        ..Span::default()
                    });
                }
                layout.flush(&mut block, &mut out, code, width);
                lists.push(start);
            }
            Event::End(TagEnd::List(_)) => {
                layout.flush(&mut block, &mut out, code, width);
                lists.pop();
                if lists.is_empty() {
                    out.push(Span {
                        text: "\n".into(),
                        ..Span::default()
                    });
                }
            }
            Event::Start(Tag::Item) => {
                layout.flush(&mut block, &mut out, code, width);
                let marker = if let Some(Some(n)) = lists.last_mut() {
                    let s = format!("{n}. ");
                    *n += 1;
                    s
                } else {
                    "- ".into()
                };
                layout.indents.push(marker.len());
                layout.marker = Some(marker);
            }
            Event::End(TagEnd::Item) => {
                if block.last().is_some_and(|s| !s.text.ends_with('\n')) {
                    block.push(Span {
                        text: "\n".into(),
                        ..Span::default()
                    });
                }
                layout.flush(&mut block, &mut out, code, width);
                layout.indents.pop();
                layout.marker = None;
            }
            Event::End(TagEnd::Paragraph) => {
                text = Some(if lists.is_empty() { "\n\n" } else { "\n" }.into());
            }
            Event::Start(Tag::Link { dest_url, .. }) => active_link = link(&dest_url, article),
            Event::End(TagEnd::Link) => {
                if let Some(link) = active_link.take()
                    && link.kind == links::LinkKind::External
                    && !options.link_enabled("url")
                {
                    text = Some(format!(" ({})", link.target));
                }
            }
            Event::Text(s) => text = Some(super::parser::strip_controls(&s)),
            Event::Code(s) => {
                let span = Span {
                    text: super::parser::strip_controls(&s),
                    style: Style::default(),
                    link: None,
                };
                if in_cell {
                    cell.push(span)
                } else {
                    block.push(span)
                }
            }
            Event::SoftBreak => text = Some(" ".into()),
            Event::HardBreak => text = Some("\n".into()),
            Event::Rule => text = Some(format!("{}\n", "─".repeat(width.min(40)))),
            Event::TaskListMarker(checked) => {
                text = Some(if checked { "[x] " } else { "[ ] " }.into())
            }
            _ => {}
        }
        if let Some(text) = text {
            let span = Span {
                text,
                style: style.clone(),
                link: active_link.clone(),
            };
            if in_cell {
                cell.push(span)
            } else {
                block.push(span);
            }
        }
        if flush_after {
            layout.flush(&mut block, &mut out, code, width);
        }
    }
    layout.flush(&mut block, &mut out, code, width);
    out
}

fn table_spans(
    rows: Vec<Vec<Vec<Span>>>,
    width: usize,
    alignment: &[pulldown_cmark::Alignment],
) -> Vec<Span> {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0; columns];
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(UnicodeWidthStr::width(
                cell.iter()
                    .map(|s| s.text.as_str())
                    .collect::<String>()
                    .as_str(),
            ));
        }
    }
    let fits = widths.iter().sum::<usize>() + columns.saturating_sub(1) * 3 <= width;
    let mut out = Vec::new();
    for (r, row) in rows.iter().enumerate() {
        for (i, cell) in row.iter().enumerate() {
            if !fits && r > 0 {
                let label = rows[0]
                    .get(i)
                    .map(|c| c.iter().map(|s| s.text.as_str()).collect::<String>())
                    .unwrap_or_else(|| format!("Column {}", i + 1));
                out.push(Span {
                    text: format!("{label}: "),
                    style: Style {
                        bold: true,
                        ..Style::default()
                    },
                    link: None,
                });
            }
            let used = UnicodeWidthStr::width(
                cell.iter()
                    .map(|s| s.text.as_str())
                    .collect::<String>()
                    .as_str(),
            );
            let padding = widths[i] - used;
            let leading = if fits {
                match alignment.get(i) {
                    Some(pulldown_cmark::Alignment::Right) => padding,
                    Some(pulldown_cmark::Alignment::Center) => padding / 2,
                    _ => 0,
                }
            } else {
                0
            };
            if leading > 0 {
                out.push(Span {
                    text: " ".repeat(leading),
                    ..Span::default()
                });
            }
            out.extend(cell.clone());
            out.push(Span {
                text: if fits && i + 1 < columns {
                    format!("{} | ", " ".repeat(padding - leading))
                } else {
                    "\n".into()
                },
                ..Span::default()
            });
        }
    }
    if fits { out } else { wrap(out, width) }
}

/// Library HTML formatting with raw HTML removed and explicit link policy.
pub fn html(source: &str) -> String {
    html_at(source, None)
}

/// HTML and Telnet use the same article-relative link policy.
pub fn html_at(source: &str, article: Option<&str>) -> String {
    let mut link_stack = Vec::new();
    let events = parse(source).filter_map(|e| match e {
        Event::Html(_) | Event::InlineHtml(_) => None,
        Event::Start(Tag::Link { dest_url, .. }) => {
            let l = link(&dest_url, article);
            let tag = l
                .as_ref()
                .map(|l| {
                    if l.kind == links::LinkKind::External {
                        format!("<a href=\"{}\">", super::render::html_escape(&l.target))
                    } else {
                        format!(
                            "<a data-mux-action=\"send\" data-mux-command=\"{}\">",
                            super::render::html_escape(&l.target)
                        )
                    }
                })
                .unwrap_or_default();
            link_stack.push(l.is_some());
            Some(Event::InlineHtml(tag.into()))
        }
        Event::End(TagEnd::Link) => Some(Event::InlineHtml(
            if link_stack.pop().unwrap_or(false) {
                "</a>"
            } else {
                ""
            }
            .into(),
        )),
        Event::Start(Tag::Image { .. }) | Event::End(TagEnd::Image) => None,
        Event::Text(s) => Some(Event::Text(super::parser::strip_controls(&s).into())),
        Event::Code(s) => Some(Event::Code(super::parser::strip_controls(&s).into())),
        _ => Some(e),
    });
    let mut out = String::new();
    pulldown_cmark::html::push_html(&mut out, events);
    out
}
