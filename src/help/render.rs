//! Recipient-specific help index layout using semantic text and action links.
use super::{HelpEntry, HelpResponse, IndexStyle};
use crate::text::{
    self, RenderOptions, Span, Style,
    links::{Link, LinkConfig, LinkKind},
};
use anyhow::{Result, ensure};
use unicode_width::UnicodeWidthStr;

/// Literal metadata cannot introduce styling or terminal controls.
fn literal(s: &str) -> Span {
    text::Document::Literal(s.into())
        .spans(&text::Palette::default(), &RenderOptions::default())
        .remove(0)
}

/// Index actions use primary keywords so displayed commands also work without OSC.
fn topic(entry: &HelpEntry) -> Span {
    Span {
        link: Some(Link {
            kind: LinkKind::Send,
            target: format!("help {}", entry.topic),
            config: LinkConfig::default(),
        }),
        ..literal(&entry.topic)
    }
}

impl HelpResponse {
    /// Render Markdown body and indexes independently, preserving explicit text formats.
    pub fn spans(&self, options: &RenderOptions) -> Vec<Span> {
        let Self::Article {
            body,
            path,
            entries,
            style,
        } = self
        else {
            let Self::Message(message) = self else {
                unreachable!()
            };
            return text::markdown::wrap(vec![literal(message)], options.width.max(1));
        };
        let mut out = text::markdown::spans_at(body, options, Some(path));
        if !entries.is_empty() {
            out.push(literal("\n"));
            let width = options.width.max(1);
            match style {
                IndexStyle::Columnar => {
                    let cell = entries
                        .iter()
                        .map(|e| UnicodeWidthStr::width(e.topic.as_str()) + 2)
                        .max()
                        .unwrap_or(20)
                        .max(20);
                    let columns = (width / cell).clamp(1, 3);
                    for row in entries.chunks(columns) {
                        for (i, entry) in row.iter().enumerate() {
                            if columns == 1 {
                                out.extend(text::markdown::wrap(vec![topic(entry)], width));
                            } else {
                                out.push(topic(entry));
                            }
                            if i + 1 < row.len() {
                                out.push(literal(&" ".repeat(cell.saturating_sub(
                                    UnicodeWidthStr::width(entry.topic.as_str()),
                                ))));
                            }
                        }
                        out.push(literal("\n"));
                    }
                }
                IndexStyle::ListWithDescription => {
                    if width >= 32 {
                        out.push(Span {
                            style: Style {
                                bold: true,
                                ..Style::default()
                            },
                            ..literal("TOPIC                DESCRIPTION\n")
                        });
                    }
                    for entry in entries {
                        let used = UnicodeWidthStr::width(entry.topic.as_str());
                        if width >= 32 && used <= 20 {
                            out.push(topic(entry));
                            out.push(literal(&" ".repeat(21 - used)));
                            let description =
                                text::markdown::wrap(vec![literal(&entry.description)], width - 21);
                            for mut span in description {
                                span.text =
                                    span.text.replace('\n', &format!("\n{}", " ".repeat(21)));
                                out.push(span);
                            }
                        } else {
                            out.extend(text::markdown::wrap(vec![topic(entry)], width));
                            out.push(literal("\n"));
                            out.extend(text::markdown::wrap(
                                vec![literal(&entry.description)],
                                width,
                            ));
                        }
                        out.push(literal("\n"));
                    }
                }
            }
            if !options.has("SEND") {
                out.push(literal("\n"));
                out.extend(text::markdown::wrap(
                    vec![literal("Type help <topic> to open a topic.")],
                    width,
                ));
                out.push(literal("\n"));
            }
        }
        out
    }

    /// Safe fragment rendering for future browser delivery, retaining structured actions.
    pub fn html(&self, limit: usize) -> Result<String> {
        let output = match self {
            Self::Message(message) => {
                text::Document::Literal(message.clone()).html(&text::Palette::default(), limit)?
            }
            Self::Article {
                body,
                path,
                entries,
                ..
            } => {
                let mut html = text::markdown::html_at(body, Some(path));
                if !entries.is_empty() {
                    html.push_str("<dl>");
                    for entry in entries {
                        html.push_str("<dt>");
                        html.push_str(&text::spans_html(
                            &[topic(entry)],
                            &text::Palette::default(),
                        ));
                        html.push_str("</dt><dd>");
                        html.push_str(&text::spans_html(
                            &[literal(&entry.description)],
                            &text::Palette::default(),
                        ));
                        html.push_str("</dd>");
                    }
                    html.push_str("</dl>");
                }
                html
            }
        };
        ensure!(output.len() <= limit, "HTML output limit exceeded");
        Ok(output)
    }
}
