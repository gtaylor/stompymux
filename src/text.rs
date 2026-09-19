//! Explicit styled-text and Markdown documents, independent of output transport.
mod colors;
pub mod links;
pub mod markdown;
mod parser;
mod render;

use anyhow::{Result, ensure};
pub(crate) use render::html as spans_html;
pub use render::{ColorDepth, RenderOptions, preset, telnet_chunks};
use std::collections::BTreeMap;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// C compatibility limits; these are grammar/protocol constraints, not runtime settings.
pub const MAX_NESTING: usize = 32;
pub const OSC8_URI_LIMIT: usize = 4096;
pub const OSC8_CLOSE: &str = "\x1b]8;;\x1b\\";
/// The empty suffix denotes ordinary external hyperlinks.
pub const OSC_CAPABILITIES: &[&str] = &[
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
    "COMPACT",
    "PRESETS",
];

/// Palette and validated OSC preset definitions, shared by all text consumers.
#[derive(Clone, Debug, Default)]
pub struct Palette {
    /// Additional named RGB colors; built-in names remain protected.
    pub colors: BTreeMap<String, [u8; 3]>,
    /// Validated named OSC definitions shared by all sessions.
    pub presets: BTreeMap<String, links::LinkConfig>,
}

impl Palette {
    /// Build configuration-dependent catalogs before starting a server.
    pub fn from_config(config: &crate::config::Config) -> Result<Self> {
        let mut palette = Self::default();
        for (name, rgb) in &config.colors {
            ensure!(
                !colors::COLORS
                    .iter()
                    .any(|(n, _)| n.eq_ignore_ascii_case(name)),
                "colors.{name}: built-in CSS/X11 color cannot be replaced"
            );
            palette.colors.insert(name.to_ascii_lowercase(), *rgb);
        }
        for (name, value) in &config.osc8.presets {
            ensure!(
                name.len() <= 60
                    && name
                        .as_bytes()
                        .first()
                        .is_some_and(u8::is_ascii_alphanumeric)
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._~-".contains(&b)),
                "osc8.presets.{name}: invalid preset name"
            );
            let config = links::LinkConfig::parse(&palette, &parser::directives(value)?)
                .map_err(|e| anyhow::anyhow!("osc8.presets.{name}: {e}"))?;
            ensure!(
                config.preset.is_none(),
                "osc8.presets.{name}: presets cannot reference presets"
            );
            ensure!(
                !config.fields.contains_key("blink") && !config.fields.contains_key("inverse"),
                "osc8.presets.{name}: ANSI-only properties are not valid in presets"
            );
            let mut options = RenderOptions::all();
            options.capabilities.remove("COMPACT");
            ensure!(
                preset(name, &config, &options).len() <= OSC8_URI_LIMIT + 2 * OSC8_CLOSE.len(),
                "osc8.presets.{name}: encoded preset is too long"
            );
            palette.presets.insert(name.clone(), config);
        }
        Ok(palette)
    }

    /// Resolve C color spellings, custom names and RGB literals.
    pub fn color(&self, name: &str) -> Result<Option<[u8; 3]>> {
        let name = name.to_ascii_lowercase();
        if let Some(body) = name.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
            ensure!(
                body.bytes().all(|b| b.is_ascii_digit() || b == b','),
                "RGB channels must be unsigned decimal integers"
            );
            let rgb: Vec<u8> = body
                .split(',')
                .map(str::parse)
                .collect::<std::result::Result<_, _>>()?;
            ensure!(rgb.len() == 3, "RGB requires three channels");
            return Ok(Some([rgb[0], rgb[1], rgb[2]]));
        }
        if let Some(hex) = name.strip_prefix('#') {
            ensure!(hex.len() == 6 && hex.is_ascii(), "invalid RGB color");
            return Ok(Some([
                u8::from_str_radix(&hex[0..2], 16)?,
                u8::from_str_radix(&hex[2..4], 16)?,
                u8::from_str_radix(&hex[4..6], 16)?,
            ]));
        }
        if let Some(rgb) = self.colors.get(&name) {
            return Ok(Some(*rgb));
        }
        if let Some((_, rgb)) = colors::COLORS.iter().find(|(n, _)| *n == name) {
            return Ok(Some(*rgb));
        }
        let aliases = [
            "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
        ];
        if let Some(n) = name
            .strip_prefix("bright-")
            .and_then(|n| aliases.iter().position(|a| *a == n))
        {
            return Ok(Some(render::ansi_rgb(n + 8)));
        }
        anyhow::bail!("unknown color {name:?}")
    }
}

/// Semantic inline style; no terminal control sequences are stored here.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Style {
    pub foreground: Option<[u8; 3]>,
    /// Raw ANSI 16 indices retain the client's palette across rendering depths.
    pub foreground_ansi: Option<u8>,
    pub background_ansi: Option<u8>,
    pub background: Option<[u8; 3]>,
    pub bold: bool,
    pub italic: bool,
    pub blink: bool,
    pub underline: bool,
    pub overline: bool,
    pub strikethrough: bool,
    pub inverse: bool,
}

/// A run of visible text with semantic styling and an optional action.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Span {
    /// Visible content; terminal control bytes are never stored in a run.
    pub text: String,
    /// Presentation attributes shared across rendering backends.
    pub style: Style,
    /// Optional action associated with this visible run.
    pub link: Option<links::Link>,
}

/// Input format is explicit; Markdown is never inferred from an ordinary message.
#[derive(Clone, Debug, PartialEq)]
pub enum Document {
    Styled(String),
    /// Trusted compatibility report whose explicit style transitions are observable.
    NativeStyled(String),
    Markdown(String),
    Literal(String),
    /// Styled forwarding prefix around an explicitly formatted document.
    Prefixed {
        source: String,
        prefix: String,
        document: Box<Document>,
    },
}

impl Document {
    /// Construct a bounded native compatibility report.
    pub fn native_styled(source: String, limit: usize) -> Result<Self> {
        ensure!(source.len() <= limit, "native styled input limit exceeded");
        Ok(Self::NativeStyled(source))
    }

    /// Prefix without interpreting a Markdown body as bracket markup or creating nested wrappers.
    pub fn prefixed(&self, prefix: &str) -> Self {
        let (prefix, document) = match self {
            Self::Prefixed {
                prefix: old,
                document,
                ..
            } => (format!("{prefix}{old}"), document.clone()),
            _ => (prefix.to_string(), Box::new(self.clone())),
        };
        Self::Prefixed {
            source: format!("{prefix}{}", document.source()),
            prefix,
            document,
        }
    }

    /// Validate bounded Markdown at its API boundary.
    pub fn markdown(source: String, limit: usize) -> Result<Self> {
        ensure!(source.len() <= limit, "Markdown input limit exceeded");
        ensure!(
            !source.contains('\0'),
            "Markdown contains an embedded NUL byte"
        );
        let mut depth = 0usize;
        for event in markdown::parse(&source) {
            match event {
                pulldown_cmark::Event::Start(_) => {
                    depth += 1;
                    ensure!(depth <= MAX_NESTING, "Markdown nesting limit exceeded");
                }
                pulldown_cmark::Event::End(_) => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        Ok(Self::Markdown(source))
    }

    /// Original source retained independently of the selected renderer.
    pub fn source(&self) -> &str {
        match self {
            Self::Styled(s) | Self::NativeStyled(s) | Self::Markdown(s) | Self::Literal(s) => s,
            Self::Prefixed { source, .. } => source,
        }
    }

    /// Source bytes charged against existing message budgets.
    pub fn len(&self) -> usize {
        self.source().len()
    }

    /// Whether the document has any source bytes.
    pub fn is_empty(&self) -> bool {
        self.source().is_empty()
    }

    /// Parse explicit input into semantic inline runs for this recipient.
    pub fn spans(&self, palette: &Palette, options: &RenderOptions) -> Vec<Span> {
        match self {
            Self::Styled(s) | Self::NativeStyled(s) => {
                parser::parse(palette, s, false).unwrap_or_default()
            }
            Self::Markdown(s) => markdown::spans(s, options),
            Self::Prefixed {
                prefix, document, ..
            } => {
                let mut spans = parser::parse(palette, prefix, false).unwrap_or_default();
                spans.extend(document.spans(palette, options));
                spans
            }
            Self::Literal(s) => vec![Span {
                text: parser::strip_controls(s),
                ..Span::default()
            }],
        }
    }

    /// Render within the encoded byte budget, reserving terminal closures.
    pub fn telnet(&self, palette: &Palette, options: &RenderOptions, limit: usize) -> Vec<u8> {
        match self {
            Self::NativeStyled(source) => {
                render::native_telnet(&parser::native_events(palette, source), options, limit)
            }
            Self::Prefixed {
                prefix, document, ..
            } if matches!(document.as_ref(), Self::NativeStyled(_)) => {
                let mut events = parser::native_events(palette, prefix);
                let prefix_style = events.iter().rev().find_map(|event| match event {
                    parser::NativeEvent::Style(style) => Some(style),
                    parser::NativeEvent::Text(_) => None,
                });
                if prefix_style.is_some_and(|style| *style != Style::default()) {
                    events.push(parser::NativeEvent::Style(Style::default()));
                }
                events.extend(parser::native_events(palette, document.source()));
                render::native_telnet(&events, options, limit)
            }
            _ => render::telnet(&self.spans(palette, options), palette, options, limit),
        }
    }

    /// Render a safe HTML fragment or fail if its output exceeds the budget.
    pub fn html(&self, palette: &Palette, limit: usize) -> Result<String> {
        let output = match self {
            Self::Markdown(s) => markdown::html(s),
            _ => render::html(&self.spans(palette, &RenderOptions::default()), palette),
        };
        ensure!(output.len() <= limit, "HTML output limit exceeded");
        Ok(output)
    }
}

impl From<String> for Document {
    fn from(s: String) -> Self {
        Self::Styled(s)
    }
}

impl From<&str> for Document {
    fn from(s: &str) -> Self {
        Self::Styled(s.into())
    }
}

impl std::ops::Deref for Document {
    type Target = str;
    fn deref(&self) -> &str {
        self.source()
    }
}

impl std::fmt::Display for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.source())
    }
}

/// Strict markup validation leaves the author's original source unchanged.
pub fn validate(palette: &Palette, s: &str) -> Result<String> {
    parser::parse(palette, s, true)?;
    Ok(s.into())
}

/// Byte length of one UTF-8 character from its leading byte.
fn utf8_step(byte: u8) -> usize {
    match byte {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    }
}

/// C styled_append_utf8_codepoint (src/mux/support/styled_text/output.c):
/// strict UTF-8 is decoded at each byte and every byte that fails to begin a
/// valid sequence becomes exactly one U+FFFD. Rust's from_utf8_lossy instead
/// coalesces maximal invalid subparts, so truncated multi-byte prefixes would
/// produce fewer replacement characters than the C renderer.
pub fn c_utf8_lossy(bytes: &[u8]) -> String {
    let mut out = Vec::with_capacity(bytes.len());
    let mut cursor = 0;
    while cursor < bytes.len() {
        match utf8_decode_step(&bytes[cursor..]) {
            Some(length) => {
                out.extend_from_slice(&bytes[cursor..cursor + length]);
                cursor += length;
            }
            None => {
                out.extend_from_slice(b"\xef\xbf\xbd");
                cursor += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Length of the strict UTF-8 sequence starting at the first byte, if any
/// (utf8.c utf8_decode: lead range C2..F4, all continuations present, and no
/// overlong, surrogate, or out-of-range codepoint).
fn utf8_decode_step(bytes: &[u8]) -> Option<usize> {
    let first = *bytes.first()?;
    let needed = match first {
        0x00..=0x7f => return Some(1),
        0xc2..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf4 => 4,
        _ => return None,
    };
    let prefix = bytes.get(..needed)?;
    std::str::from_utf8(prefix).is_ok().then_some(needed)
}

/// Longest strict UTF-8 prefix of the raw bytes. C styled_text_truncate stops
/// at the first byte that does not begin a valid sequence instead of
/// substituting replacement characters, so truncation runs on this prefix.
pub fn utf8_valid_prefix(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(error) => {
            let end = error.valid_up_to();
            String::from_utf8_lossy(&bytes[..end]).into_owned()
        }
    }
}

/// Length and SGR classification of one escape sequence starting at ESC.
fn ansi_step(s: &str) -> (usize, bool) {
    let bytes = s.as_bytes();
    if bytes.len() < 2 {
        return (bytes.len(), false);
    }
    match bytes[1] {
        b'[' => {
            for (offset, byte) in bytes.iter().enumerate().skip(2) {
                if (0x40..=0x7e).contains(byte) {
                    return (offset + 1, *byte == b'm');
                }
            }
            (bytes.len(), false)
        }
        b']' => {
            for (offset, byte) in bytes.iter().enumerate().skip(2) {
                if *byte == 0x07 {
                    return (offset + 1, false);
                }
                if *byte == 0x1b && bytes.get(offset + 1) == Some(&b'\\') {
                    return (offset + 2, false);
                }
            }
            (bytes.len(), false)
        }
        _ => (2, false),
    }
}

/// Apply one raw markup tag exactly as C's apply_tag candidate check does.
fn mux_tag_applies(palette: &Palette, inner: &str, stack: &mut Vec<bool>) -> bool {
    if inner == "/" {
        return stack.pop().is_some();
    }
    let Ok(list) = parser::directives(inner) else {
        return false;
    };
    let mut style = Style::default();
    let mut applied = false;
    for directive in list {
        if parser::apply_style(palette, &mut style, &directive).is_err() {
            return false;
        }
        applied = true;
    }
    if applied {
        stack.push(true);
    }
    applied
}

/// C styled_text_strip: plain rendering keeps every non-markup byte, control
/// characters included, and drops valid tags plus escape sequences.
pub fn mux_plain(palette: &Palette, s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut stack = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        match bytes[cursor] {
            0x1b => cursor += ansi_step(&s[cursor..]).0,
            b'[' if bytes.get(cursor + 1) == Some(&b'[') => {
                out.push(b'[');
                cursor += 2;
            }
            b'[' => {
                let matched = s[cursor..]
                    .find(']')
                    .filter(|close| close > &1)
                    .is_some_and(|close| {
                        mux_tag_applies(palette, &s[cursor + 1..cursor + close], &mut stack)
                    });
                if let Some(close) = matched.then(|| s[cursor..].find(']').unwrap()) {
                    cursor += close + 1;
                } else {
                    out.push(b'[');
                    cursor += 1;
                }
            }
            _ => {
                let step = utf8_step(bytes[cursor]).min(bytes.len() - cursor);
                out.extend_from_slice(&bytes[cursor..cursor + step]);
                cursor += step;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// C styled_text_truncate: original markup is copied verbatim while a visible
/// byte budget lasts; unclosed tags are re-closed and SGR sequences reset.
pub fn mux_truncate(palette: &Palette, s: &str, width: usize) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut stack = Vec::new();
    let mut visible = 0usize;
    let mut saw_sgr = false;
    let mut cursor = 0;
    while cursor < bytes.len() && visible < width {
        match bytes[cursor] {
            0x1b => {
                let (step, is_sgr) = ansi_step(&s[cursor..]);
                if is_sgr {
                    out.extend_from_slice(&bytes[cursor..cursor + step]);
                    saw_sgr = true;
                }
                cursor += step;
            }
            b'[' if bytes.get(cursor + 1) == Some(&b'[') => {
                out.push(b'[');
                cursor += 2;
                visible += 1;
            }
            b'[' => {
                let tag = s[cursor..]
                    .find(']')
                    .filter(|close| close > &1 && close < &1024)
                    .and_then(|close| {
                        mux_tag_applies(palette, &s[cursor + 1..cursor + close], &mut stack)
                            .then_some(close)
                    });
                if let Some(close) = tag {
                    out.extend_from_slice(&bytes[cursor..cursor + close + 1]);
                    cursor += close + 1;
                } else {
                    out.push(b'[');
                    cursor += 1;
                    visible += 1;
                }
            }
            _ => {
                let step = utf8_step(bytes[cursor]).min(bytes.len() - cursor);
                if visible + step > width {
                    break;
                }
                out.extend_from_slice(&bytes[cursor..cursor + step]);
                cursor += step;
                visible += step;
            }
        }
    }
    for _ in 0..stack.len() {
        out.extend_from_slice(b"[/]");
    }
    if saw_sgr {
        out.extend_from_slice(b"\x1b[0m");
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Visible text ignores markup and terminal escapes.
pub fn plain(s: &str) -> String {
    plain_with(&Palette::default(), s)
}

/// Visible names use the same configured palette as rendering.
pub fn plain_with(palette: &Palette, s: &str) -> String {
    parser::parse(palette, s, false)
        .unwrap_or_default()
        .iter()
        .map(|s| s.text.as_str())
        .collect()
}

/// Count display columns after removing legacy styling.
pub fn width(s: &str) -> usize {
    UnicodeWidthStr::width(plain(s).as_str())
}

/// Truncate on grapheme boundaries and preserve styles through canonical markup.
pub fn truncate(s: &str, max: usize) -> String {
    truncate_with(&Palette::default(), s, max)
}

/// Use the configured palette while retaining whole visible graphemes.
pub fn truncate_with(palette: &Palette, s: &str, max: usize) -> String {
    let spans = parser::parse(palette, s, false).unwrap_or_default();
    let plain: String = spans.iter().map(|s| s.text.as_str()).collect();
    let mut used = 0;
    let mut bytes = 0;
    for g in plain.graphemes(true) {
        let n = UnicodeWidthStr::width(g);
        if used + n > max {
            break;
        }
        bytes += g.len();
        used += n;
    }
    let mut out = String::new();
    for mut span in spans {
        let n = bytes.min(span.text.len());
        span.text.truncate(n);
        out.push_str(&render::to_markup(&span));
        bytes -= n;
        if bytes == 0 {
            break;
        }
    }
    out
}

pub fn style(s: &str, color: &str) -> String {
    format!("[fg={color}]{s}[/]")
}

pub fn escape(s: &str) -> String {
    s.replace('[', "[[")
}

/// Grapheme-safe literal prefix measured in terminal columns, without parsing markup.
pub fn literal_prefix(s: &str, columns: usize) -> &str {
    let mut width = 0;
    let mut end = 0;
    for g in s.graphemes(true) {
        let n = UnicodeWidthStr::width(g);
        if width + n > columns {
            break;
        }
        width += n;
        end += g.len();
    }
    &s[..end]
}
