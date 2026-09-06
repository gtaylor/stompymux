//! Explicit styled-text and Markdown documents, independent of output transport.
mod colors;
pub mod links;
pub mod markdown;
mod parser;
mod render;

use anyhow::{Result, ensure};
pub use render::{ColorDepth, RenderOptions, preset};
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
    Markdown(String),
    Literal(String),
}

impl Document {
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
            Self::Styled(s) | Self::Markdown(s) | Self::Literal(s) => s,
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
            Self::Styled(s) => parser::parse(palette, s, false).unwrap_or_default(),
            Self::Markdown(s) => markdown::spans(s, options),
            Self::Literal(s) => vec![Span {
                text: parser::strip_controls(s),
                ..Span::default()
            }],
        }
    }

    /// Render within the encoded byte budget, reserving terminal closures.
    pub fn telnet(&self, palette: &Palette, options: &RenderOptions, limit: usize) -> Vec<u8> {
        render::telnet(&self.spans(palette, options), palette, options, limit)
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
