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

/// Translate safe URLs and help destinations into shared actions.
fn link(url: &str) -> Option<Link> {
    let (kind, target) = if links::external(url) {
        ("url", url.to_string())
    } else if let Some(topic) = url
        .strip_prefix("help:")
        .filter(|s| !s.is_empty() && !s.chars().any(char::is_control))
    {
        ("send", format!("help {topic}"))
    } else if !url.contains(':') && !url.starts_with("//") && !url.chars().any(char::is_control) {
        let topic = url.trim_end_matches(".md").trim_start_matches("./");
        ("send", format!("help {topic}"))
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
fn wrap(spans: Vec<Span>, width: usize) -> Vec<Span> {
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

/// Render Markdown blocks into styled runs without reparsing bracket examples.
pub fn spans(source: &str, options: &super::RenderOptions) -> Vec<Span> {
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
    let flush = |block: &mut Vec<Span>, out: &mut Vec<Span>, code: bool| {
        if !block.is_empty() {
            let spans = std::mem::take(block);
            out.extend(if code { spans } else { wrap(spans, width) });
        }
    };
    for event in parse(source) {
        let flush_after = matches!(
            event,
            Event::End(TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::Item)
        );
        let mut text = None;
        match event {
            Event::Start(Tag::Table(alignment)) => {
                table_alignment = alignment;
                flush(&mut block, &mut out, code);
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
                    out.extend(table_spans(t, width, &table_alignment));
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
                flush(&mut block, &mut out, code);
                style.bold = true;
                text = Some(format!("{} ", "#".repeat(level as usize)));
            }
            Event::End(TagEnd::Heading(_)) => {
                style = Style::default();
                text = Some("\n\n".into());
            }
            Event::Start(Tag::CodeBlock(_)) => {
                flush(&mut block, &mut out, code);
                code = true;
            }
            Event::End(TagEnd::CodeBlock) => {
                flush(&mut block, &mut out, true);
                code = false;
                text = Some("\n".into());
            }
            Event::Start(Tag::BlockQuote(_)) => {
                flush(&mut block, &mut out, code);
                text = Some("> ".into());
            }
            Event::Start(Tag::List(start)) => {
                flush(&mut block, &mut out, code);
                lists.push(start);
            }
            Event::End(TagEnd::List(_)) => {
                lists.pop();
                text = Some("\n".into());
            }
            Event::Start(Tag::Item) => {
                let depth = lists.len().saturating_sub(1);
                let marker = if let Some(Some(n)) = lists.last_mut() {
                    let s = format!("{n}. ");
                    *n += 1;
                    s
                } else {
                    "- ".into()
                };
                text = Some(format!("{}{marker}", "  ".repeat(depth)));
            }
            Event::End(TagEnd::Item) => text = Some("\n".into()),
            Event::End(TagEnd::Paragraph) => {
                text = Some(if lists.is_empty() { "\n\n" } else { "\n" }.into());
            }
            Event::Start(Tag::Link { dest_url, .. }) => active_link = link(&dest_url),
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
            flush(&mut block, &mut out, code);
        }
    }
    flush(&mut block, &mut out, code);
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
    let mut link_stack = Vec::new();
    let events = parse(source).filter_map(|e| match e {
        Event::Html(_) | Event::InlineHtml(_) => None,
        Event::Start(Tag::Link { dest_url, .. }) => {
            let l = link(&dest_url);
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
