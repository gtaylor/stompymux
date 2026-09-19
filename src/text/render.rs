//! Capability-aware ANSI/OSC and escaped HTML rendering with bounded output.
use super::{Palette, Span, Style, links};
use std::collections::BTreeSet;
use unicode_segmentation::UnicodeSegmentation;

/// Terminal color modes supported by the output renderer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorDepth {
    None,
    #[default]
    Ansi16,
    Ansi256,
    Truecolor,
}

impl ColorDepth {
    /// User-facing spelling used by color commands and diagnostics.
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "off",
            Self::Ansi16 => "16",
            Self::Ansi256 => "256",
            Self::Truecolor => "truecolor",
        }
    }

    /// Translate negotiated terminal color depth to a renderer mode.
    pub fn advertised(n: u16) -> Self {
        match n {
            0 => Self::None,
            24 => Self::Truecolor,
            256 => Self::Ansi256,
            _ => Self::Ansi16,
        }
    }
}

/// Effective recipient capabilities and display width for one delivery.
#[derive(Clone, Debug)]
pub struct RenderOptions {
    pub color: ColorDepth,
    pub capabilities: BTreeSet<String>,
    pub width: usize,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            color: ColorDepth::None,
            capabilities: BTreeSet::new(),
            width: 80,
        }
    }
}

impl RenderOptions {
    /// Full compatibility capabilities for validating serialized link metadata.
    pub fn all() -> Self {
        Self {
            capabilities: super::OSC_CAPABILITIES
                .iter()
                .map(|s| s.to_string())
                .collect(),
            color: ColorDepth::Truecolor,
            width: 80,
        }
    }

    /// Test an exact capability suffix from the negotiated environment.
    pub fn has(&self, s: &str) -> bool {
        self.capabilities.contains(s)
    }

    /// Check support for the requested link action.
    pub fn link_enabled(&self, kind: &str) -> bool {
        self.has(match kind {
            "send" => "SEND",
            "prompt" => "PROMPT",
            _ => "",
        })
    }
}

/// Resolve the C-compatible ANSI 16/256 palette.
pub fn ansi_rgb(n: usize) -> [u8; 3] {
    const BASE: [[u8; 3]; 16] = [
        [0, 0, 0],
        [128, 0, 0],
        [0, 128, 0],
        [128, 128, 0],
        [0, 0, 128],
        [128, 0, 128],
        [0, 128, 128],
        [192, 192, 192],
        [128, 128, 128],
        [255, 0, 0],
        [0, 255, 0],
        [255, 255, 0],
        [0, 0, 255],
        [255, 0, 255],
        [0, 255, 255],
        [255, 255, 255],
    ];
    if n < 16 {
        return BASE[n];
    }
    if n >= 232 {
        return [8 + 10 * (n.min(255) - 232) as u8; 3];
    }
    let n = n - 16;
    let channel = |n| if n == 0 { 0 } else { 55 + 40 * n as u8 };
    [channel(n / 36), channel(n / 6 % 6), channel(n % 6)]
}

/// Reduce RGB to the closest supported palette entry.
fn color(rgb: [u8; 3], bg: bool, depth: ColorDepth) -> String {
    if depth == ColorDepth::Truecolor {
        return format!(
            "\x1b[{};2;{};{};{}m",
            if bg { 48 } else { 38 },
            rgb[0],
            rgb[1],
            rgb[2]
        );
    }
    let max = if depth == ColorDepth::Ansi256 {
        256
    } else {
        16
    };
    let n = (0..max)
        .min_by_key(|&i| {
            ansi_rgb(i)
                .iter()
                .zip(rgb)
                .map(|(&a, b)| (a as i32 - b as i32).pow(2))
                .sum::<i32>()
        })
        .unwrap();
    if max == 256 {
        format!("\x1b[{};5;{n}m", if bg { 48 } else { 38 })
    } else {
        format!(
            "\x1b[{}m",
            if bg { 40 } else { 30 } + n % 8 + if n >= 8 { 60 } else { 0 }
        )
    }
}

/// Encode a semantic style for the effective terminal depth.
fn ansi(s: &Style, depth: ColorDepth) -> String {
    if depth == ColorDepth::None {
        return String::new();
    }
    let mut out = String::from("\x1b[0m");
    for (b, n) in [
        (s.bold, 1),
        (s.italic, 3),
        (s.underline, 4),
        (s.blink, 5),
        (s.inverse, 7),
        (s.strikethrough, 9),
        (s.overline, 53),
    ] {
        if b {
            out.push_str(&format!("\x1b[{n}m"));
        }
    }
    for (rgb, index, bg) in [
        (s.foreground, s.foreground_ansi, false),
        (s.background, s.background_ansi, true),
    ] {
        if let Some(index) = index {
            out.push_str(&format!(
                "\x1b[{}m",
                (if bg { 40 } else { 30 }) + index % 8 + if index >= 8 { 60 } else { 0 }
            ));
        } else if let Some(rgb) = rgb {
            out.push_str(&color(rgb, bg, depth));
        }
    }
    out
}

/// Keep each grapheme and its style boundaries atomic, even across semantic spans.
pub fn telnet(spans: &[Span], p: &Palette, o: &RenderOptions, limit: usize) -> Vec<u8> {
    render_chunks(spans, p, o, limit, usize::MAX, false)
        .expect("single output is bounded")
        .pop()
        .unwrap_or_default()
}

/// Render trusted native report markup without coalescing explicit style
/// transitions. This preserves repeated reset/color controls used by the C UI.
pub(crate) fn native_telnet(
    events: &[super::parser::NativeEvent],
    o: &RenderOptions,
    limit: usize,
) -> Vec<u8> {
    let mut out = Vec::new();
    let mut styled = false;
    for event in events {
        match event {
            super::parser::NativeEvent::Style(style) => {
                let encoded = crate::telnet::encode(&ansi(style, o.color));
                let next_styled = styled || o.color != ColorDepth::None;
                let reserve = usize::from(next_styled) * 4;
                if out.len() + encoded.len() + reserve > limit {
                    break;
                }
                out.extend(encoded);
                styled = next_styled;
            }
            super::parser::NativeEvent::Text(text) => {
                for grapheme in text.graphemes(true) {
                    let encoded = crate::telnet::encode(grapheme);
                    let reserve = usize::from(styled) * 4;
                    if out.len() + encoded.len() + reserve > limit {
                        close(&mut out, false, styled);
                        return out;
                    }
                    out.extend(encoded);
                }
            }
        }
    }
    close(&mut out, false, styled);
    out
}

/// Render complete text in bounded, independently closed transport messages.
pub fn telnet_chunks(
    spans: &[Span],
    p: &Palette,
    o: &RenderOptions,
    limit: usize,
    total_limit: usize,
) -> anyhow::Result<Vec<Vec<u8>>> {
    render_chunks(spans, p, o, limit, total_limit, true)
}

/// Close controls before yielding a chunk; the next chunk restores presentation explicitly.
fn close(out: &mut Vec<u8>, link: bool, styled: bool) {
    if link {
        out.extend_from_slice(super::OSC8_CLOSE.as_bytes());
    }
    if styled {
        out.extend_from_slice(b"\x1b[0m");
    }
}

/// Share the exact same encoder for ordinary truncated messages and complete help responses.
fn render_chunks(
    spans: &[Span],
    p: &Palette,
    o: &RenderOptions,
    limit: usize,
    total_limit: usize,
    complete: bool,
) -> anyhow::Result<Vec<Vec<u8>>> {
    let visible: String = spans.iter().map(|s| s.text.as_str()).collect();
    let prepared: Vec<_> = spans
        .iter()
        .map(|span| {
            let mut style = span.style.clone();
            let uri = span.link.as_ref().and_then(|link| link.uri(p, o));
            if let Some(link) = &span.link
                && (!o.has("STYLE_BASIC") || uri.is_none())
            {
                link.config.fallback(p, &mut style);
            }
            (style, uri)
        })
        .collect();
    let mut out = Vec::new();
    let mut span_index = 0;
    let mut offset = 0;
    let mut style = Style::default();
    let mut link = None::<String>;
    let mut styled = false;
    let mut graphemes = visible.graphemes(true).peekable();
    let mut chunks = Vec::new();
    let mut total = 0usize;
    while let Some(g) = graphemes.peek().copied() {
        let saved_position = (span_index, offset);
        let mut remaining = g.len();
        let mut candidate = String::new();
        let mut next_style = style.clone();
        let mut next_link = link.clone();
        let mut next_styled = styled;
        while remaining > 0 {
            while span_index < spans.len() && offset == spans[span_index].text.len() {
                span_index += 1;
                offset = 0;
            }
            if span_index == spans.len() {
                break;
            }
            let span = &spans[span_index];
            let n = remaining.min(span.text.len() - offset);
            let (s, l) = prepared[span_index].clone();
            if l != next_link {
                if next_link.is_some() {
                    candidate.push_str("\x1b]8;;\x1b\\");
                }
                if let Some(l) = &l {
                    candidate.push_str(&format!("\x1b]8;;{l}\x1b\\"));
                }
                next_link = l;
            }
            if s != next_style {
                candidate.push_str(&ansi(&s, o.color));
                next_style = s;
                next_styled = o.color != ColorDepth::None;
            }
            candidate.push_str(&span.text[offset..offset + n]);
            offset += n;
            remaining -= n;
        }
        let encoded = crate::telnet::encode(&candidate);
        let reserve = usize::from(next_link.is_some()) * super::OSC8_CLOSE.len()
            + usize::from(next_styled) * 4;
        if out.len() + encoded.len() + reserve > limit {
            if !complete {
                break;
            }
            anyhow::ensure!(!out.is_empty(), "output limit cannot fit a styled grapheme");
            close(&mut out, link.is_some(), styled);
            total += out.len();
            anyhow::ensure!(total <= total_limit, "help output exceeds text budget");
            chunks.push(std::mem::take(&mut out));
            (span_index, offset) = saved_position;
            style = Style::default();
            link = None;
            styled = false;
            continue;
        }
        graphemes.next();
        out.extend(encoded);
        style = next_style;
        link = next_link;
        styled = next_styled;
    }
    close(&mut out, link.is_some(), styled);
    total += out.len();
    anyhow::ensure!(total <= total_limit, "help output exceeds text budget");
    if !out.is_empty() {
        chunks.push(out);
    }
    Ok(chunks)
}

/// Escape text and attribute values for HTML fragments.
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Build controlled CSS declarations without accepting arbitrary CSS.
pub fn css(s: &Style) -> String {
    let mut rules = Vec::new();
    for (name, rgb) in [("color", s.foreground), ("background-color", s.background)] {
        if let Some([r, g, b]) = rgb {
            rules.push(format!("{name}:#{r:02x}{g:02x}{b:02x}"));
        }
    }
    if s.bold {
        rules.push("font-weight:bold".into());
    }
    if s.italic {
        rules.push("font-style:italic".into());
    }
    let mut decor = Vec::new();
    if s.underline {
        decor.push("underline");
    }
    if s.overline {
        decor.push("overline");
    }
    if s.strikethrough {
        decor.push("line-through");
    }
    if !decor.is_empty() {
        rules.push(format!("text-decoration-line:{}", decor.join(" ")));
    }
    rules.join(";")
}

pub fn html(spans: &[Span], palette: &Palette) -> String {
    let mut out = String::new();
    for span in spans {
        let mut s = html_escape(&span.text);
        let mut style = span.style.clone();
        if let Some(link) = &span.link {
            link.config.fallback(palette, &mut style);
        }
        let css = css(&style);
        if !css.is_empty() {
            s = format!("<span style=\"{css}\">{s}</span>");
        }
        if style.inverse || style.blink {
            s = format!(
                "<span class=\"{}{}\">{s}</span>",
                if style.inverse { "mux-inverse" } else { "" },
                if style.blink { " mux-blink" } else { "" }
            );
        }
        if let Some(l) = &span.link
            && l.validate().is_ok()
        {
            if l.kind == links::LinkKind::External {
                s = format!("<a href=\"{}\">{s}</a>", html_escape(&l.target));
            } else {
                s = format!(
                    "<a data-mux-action=\"{}\" data-mux-command=\"{}\">{s}</a>",
                    html_escape(l.kind.name()),
                    html_escape(&l.target)
                );
            }
        }
        if let Some(l) = &span.link {
            let config = l.config.effective(palette);
            let options = RenderOptions {
                capabilities: [
                    "",
                    "SEND",
                    "PROMPT",
                    "STYLE_BASIC",
                    "STYLE_STATES",
                    "TOOLTIP",
                    "MENU",
                    "VISIBILITY",
                    "SELECTION",
                    "SPOILER",
                    "DISABLED",
                ]
                .into_iter()
                .map(str::to_string)
                .collect(),
                ..Default::default()
            };
            let value = config.json(&options);
            if value.as_object().is_some_and(|m| !m.is_empty()) {
                s = s.replacen(
                    "<a ",
                    &format!(
                        "<a data-mux-config=\"{}\" ",
                        html_escape(&value.to_string())
                    ),
                    1,
                );
            }
        }
        out.push_str(&s);
    }
    out
}

/// Canonical source serialization for style-preserving truncation.
pub fn to_markup(span: &Span) -> String {
    let s = &span.style;
    let mut tags = Vec::new();
    for (name, rgb) in [("fg", s.foreground), ("bg", s.background)] {
        if let Some([r, g, b]) = rgb {
            tags.push(format!("{name}=#{r:02x}{g:02x}{b:02x}"));
        }
    }
    for (name, b) in [
        ("bold", s.bold),
        ("italic", s.italic),
        ("blink", s.blink),
        ("underline", s.underline),
        ("overline", s.overline),
        ("strikethrough", s.strikethrough),
        ("inverse", s.inverse),
    ] {
        if b {
            tags.push(name.into());
        }
    }
    let mut text = super::escape(&span.text);
    let mut raw = String::new();
    for (index, bg) in [(s.foreground_ansi, false), (s.background_ansi, true)] {
        if let Some(index) = index {
            raw.push_str(&format!(
                "\x1b[{}m",
                (if bg { 40 } else { 30 }) + index % 8 + if index >= 8 { 60 } else { 0 }
            ));
        }
    }
    if !raw.is_empty() {
        text = format!("{raw}{text}\x1b[0m");
    }

    let mut result = if tags.is_empty() {
        text
    } else {
        format!("[{}]{text}[/]", tags.join(" "))
    };
    if let Some(l) = &span.link {
        let mut ds = Vec::new();
        if let Some(p) = &l.config.preset {
            ds.push(format!("preset={}", serde_json::to_string(p).unwrap()));
        }
        for (k, v) in &l.config.fields {
            ds.push(format!("{k}={v}"));
        }
        result = format!(
            "[{}={}{}]{result}[/]",
            l.kind,
            serde_json::to_string(&l.target).unwrap(),
            if ds.is_empty() {
                String::new()
            } else {
                format!(" {}", ds.join(" "))
            }
        );
    }
    result
}

pub fn preset(name: &str, config: &links::LinkConfig, o: &RenderOptions) -> String {
    format!(
        "\x1b]8;;preset:{}?config={}\x1b\\\x1b]8;;\x1b\\",
        links::percent(name),
        links::percent(&config.json(o).to_string())
    )
}
