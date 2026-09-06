//! Bounded bracket lexer and semantic parser, with strict and display modes.
use super::{
    Palette, Span, Style,
    links::{Link, LinkConfig},
};
use anyhow::{Result, ensure};

/// One quoted or bare directive, retaining quoting for C validation.
#[derive(Clone, Debug)]
pub struct Directive {
    pub name: String,
    pub value: Option<String>,
    pub quoted: bool,
}

/// Lex property names, quoted values and legacy escapes.
pub fn directives(s: &str) -> Result<Vec<Directive>> {
    let mut rest = s.trim();
    let mut result = Vec::new();
    while !rest.is_empty() {
        let end = rest
            .find(|c: char| c.is_ascii_whitespace() || c == '=')
            .unwrap_or(rest.len());
        ensure!(end > 0, "invalid directive");
        let name = rest[..end].to_ascii_lowercase();
        rest = &rest[end..];
        let mut value = None;
        let mut quoted = false;
        if let Some(tail) = rest.strip_prefix('=') {
            rest = tail;
            let mut v = String::new();
            if let Some(tail) = rest.strip_prefix('"') {
                quoted = true;
                rest = tail;
                let mut closed = false;
                while let Some(c) = rest.chars().next() {
                    rest = &rest[c.len_utf8()..];
                    if c == '"' {
                        closed = true;
                        break;
                    }
                    if c == '\\' {
                        let c = rest
                            .chars()
                            .next()
                            .ok_or_else(|| anyhow::anyhow!("unterminated escape"))?;
                        ensure!(matches!(c, '"' | '\\'), "invalid escape in directive");
                        v.push(c);
                        rest = &rest[c.len_utf8()..];
                    } else {
                        ensure!(!c.is_control(), "control byte in directive");
                        v.push(c);
                    }
                }
                ensure!(
                    closed && (rest.is_empty() || rest.starts_with(char::is_whitespace)),
                    "unterminated or malformed quoted directive"
                );
            } else {
                let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
                v.push_str(&rest[..end]);
                rest = &rest[end..];
            }
            ensure!(
                !v.is_empty() && v.len() <= super::OSC8_URI_LIMIT,
                "empty or oversized directive value"
            );
            value = Some(v);
        }
        result.push(Directive {
            name,
            value,
            quoted,
        });
        rest = rest.trim_start();
    }
    Ok(result)
}

/// Parse the C boolean spellings without numeric coercion.
pub fn boolean(d: &Directive) -> Result<bool> {
    ensure!(!d.quoted, "boolean cannot be quoted");
    match d.value.as_deref().map(str::to_ascii_lowercase).as_deref() {
        None | Some("true") => Ok(true),
        Some("false") => Ok(false),
        _ => anyhow::bail!("invalid boolean"),
    }
}

/// Apply one validated style property to the current style.
pub fn apply_style(p: &Palette, style: &mut Style, d: &Directive) -> Result<()> {
    match d.name.as_str() {
        "fg" | "color" => {
            style.foreground = p.color(d.value.as_deref().unwrap_or(""))?;
            style.foreground_ansi = None;
        }
        "bg" => {
            style.background = p.color(d.value.as_deref().unwrap_or(""))?;
            style.background_ansi = None;
        }
        "bold" => style.bold = boolean(d)?,
        "italic" => style.italic = boolean(d)?,
        "blink" => {
            ensure!(d.value.is_none(), "blink takes no value");
            style.blink = true;
        }
        "inverse" => {
            ensure!(d.value.is_none(), "inverse takes no value");
            style.inverse = true;
        }
        "underline" | "overline" | "strikethrough" => {
            let b = if matches!(
                d.value.as_deref().map(str::to_ascii_lowercase).as_deref(),
                Some("wavy" | "dotted" | "dashed")
            ) {
                true
            } else {
                boolean(d)?
            };
            match d.name.as_str() {
                "underline" => style.underline = b,
                "overline" => style.overline = b,
                _ => style.strikethrough = b,
            }
        }
        "text-decoration-color" => {
            p.color(d.value.as_deref().unwrap_or(""))?;
        }
        _ => anyhow::bail!("unknown style tag {}", d.name),
    }
    Ok(())
}

/// Strip controls other than layout whitespace; raw escapes never become markup.
pub fn strip_controls(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(c) = rest.chars().next() {
        if c == '\x1b' {
            rest = &rest[escape_len(rest)..];
            continue;
        }
        if !c.is_control() || matches!(c, '\n' | '\r' | '\t') {
            out.push(c);
        }
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// Consume one terminal escape without exposing its control bytes.
fn escape_len(s: &str) -> usize {
    let b = s.as_bytes();
    if b.get(1) == Some(&b'[') {
        return b
            .iter()
            .enumerate()
            .skip(2)
            .find(|(_, b)| (0x40..=0x7e).contains(*b))
            .map_or(b.len(), |(i, _)| i + 1);
    }
    if matches!(b.get(1), Some(b']' | b'P' | b'_' | b'^')) {
        for i in 2..b.len() {
            if b[i] == 7 {
                return i + 1;
            }
            if b[i] == 27 && b.get(i + 1) == Some(&b'\\') {
                return i + 2;
            }
        }
        return b.len();
    }
    s.char_indices().nth(2).map_or(s.len(), |(i, _)| i)
}

/// Interpret supported SGR attributes into semantic styles.
fn sgr(style: &mut Style, s: &str) {
    let values: Vec<u16> = s
        .trim_start_matches("\x1b[")
        .trim_end_matches('m')
        .split(';')
        .map(|n| n.parse().unwrap_or(0))
        .collect();
    if values.len() > 32 {
        return;
    }
    let mut i = 0;
    while i < values.len() {
        let n = values[i];
        match n {
            0 => *style = Style::default(),
            1 => style.bold = true,
            3 => style.italic = true,
            4 => style.underline = true,
            5 => style.blink = true,
            7 => style.inverse = true,
            9 => style.strikethrough = true,
            22 => style.bold = false,
            23 => style.italic = false,
            24 => style.underline = false,
            25 => style.blink = false,
            27 => style.inverse = false,
            29 => style.strikethrough = false,
            53 => style.overline = true,
            55 => style.overline = false,
            30..=37 | 90..=97 => {
                style.foreground_ansi = Some(if n >= 90 {
                    (n - 90 + 8) as u8
                } else {
                    (n - 30) as u8
                });
                style.foreground = Some(super::render::ansi_rgb(if n >= 90 {
                    (n - 90 + 8) as usize
                } else {
                    (n - 30) as usize
                }))
            }
            40..=47 | 100..=107 => {
                style.background_ansi = Some(if n >= 100 {
                    (n - 100 + 8) as u8
                } else {
                    (n - 40) as u8
                });
                style.background = Some(super::render::ansi_rgb(if n >= 100 {
                    (n - 100 + 8) as usize
                } else {
                    (n - 40) as usize
                }))
            }
            39 => {
                style.foreground = None;
                style.foreground_ansi = None;
            }
            49 => {
                style.background = None;
                style.background_ansi = None;
            }
            38 | 48 => {
                if n == 38 {
                    style.foreground_ansi = None;
                } else {
                    style.background_ansi = None;
                }
                let color = if values.get(i + 1) == Some(&5) && values.len() > i + 2 {
                    i += 2;
                    Some(super::render::ansi_rgb(values[i].min(255) as usize))
                } else if values.get(i + 1) == Some(&2) && values.len() > i + 4 {
                    i += 4;
                    Some([
                        values[i - 2].min(255) as u8,
                        values[i - 1].min(255) as u8,
                        values[i].min(255) as u8,
                    ])
                } else {
                    None
                };
                if n == 38 {
                    style.foreground = color
                } else {
                    style.background = color
                }
            }
            _ => {}
        }
        i += 1;
    }
}

/// Parse without changing state on malformed tags in permissive mode.
pub fn parse(p: &Palette, source: &str, strict: bool) -> Result<Vec<Span>> {
    ensure!(
        !strict || !source.contains(['\x1b', '\0']),
        "literal escapes or NUL are not allowed"
    );
    let mut spans = Vec::new();
    let mut current = Span::default();
    let mut stack = Vec::new();
    let mut rest = source;
    while !rest.is_empty() {
        if rest.starts_with('\x1b') {
            let len = escape_len(rest);
            let seq = &rest[..len];
            if !current.text.is_empty() {
                spans.push(current.clone());
                current.text.clear();
            }
            if seq.starts_with("\x1b[") && seq.ends_with('m') {
                sgr(&mut current.style, seq);
            }
            rest = &rest[len..];
            continue;
        }
        if rest.starts_with("[[") {
            current.text.push('[');
            rest = &rest[2..];
            continue;
        }
        if rest.starts_with('[') {
            // Bound scanning and stop at another unquoted opener, avoiding quadratic rescans.
            let mut quoted = false;
            let mut escaped = false;
            let mut end = None;
            for (i, c) in rest
                .char_indices()
                .skip(1)
                .take_while(|(i, _)| *i < super::OSC8_URI_LIMIT + 32)
            {
                if escaped {
                    escaped = false;
                    continue;
                }
                if quoted && c == '\\' {
                    escaped = true;
                    continue;
                }
                if c == '"' {
                    quoted = !quoted;
                }
                if !quoted && c == '[' {
                    break;
                }
                if !quoted && c == ']' {
                    end = Some(i);
                    break;
                }
            }
            let tag_result = (|| -> Result<(Style, Option<Link>, bool, bool)> {
                let end = end.ok_or_else(|| anyhow::anyhow!("unclosed style tag"))?;
                ensure!(end < super::OSC8_URI_LIMIT + 32, "oversized style tag");
                let tag = rest[1..end].trim();
                if tag == "/" {
                    let (s, l) = stack
                        .last()
                        .cloned()
                        .ok_or_else(|| anyhow::anyhow!("unmatched close tag"))?;
                    return Ok((s, l, true, false));
                }
                if tag.eq_ignore_ascii_case("reset") {
                    return Ok((Style::default(), None, false, true));
                }
                ensure!(
                    stack.len() < super::MAX_NESTING,
                    "style nesting is too deep"
                );
                let ds = directives(tag)?;
                ensure!(!ds.is_empty(), "empty style tag");
                let mut style = current.style.clone();
                let mut link = current.link.clone();
                if matches!(ds[0].name.as_str(), "url" | "link" | "send" | "prompt") {
                    ensure!(link.is_none(), "nested links are not allowed");
                    let target = ds[0]
                        .value
                        .clone()
                        .ok_or_else(|| anyhow::anyhow!("missing link destination"))?;
                    ensure!(ds[0].quoted, "link destination must be quoted");
                    let kind = match ds[0].name.as_str() {
                        "send" => "send",
                        "prompt" => "prompt",
                        _ => "url",
                    };
                    for d in ds[1..]
                        .iter()
                        .filter(|d| matches!(d.name.as_str(), "blink" | "inverse"))
                    {
                        apply_style(p, &mut style, d)?;
                    }
                    link = Some(Link {
                        kind: match kind {
                            "send" => super::links::LinkKind::Send,
                            "prompt" => super::links::LinkKind::Prompt,
                            _ => super::links::LinkKind::External,
                        },
                        target,
                        config: LinkConfig::parse(
                            p,
                            &ds[1..]
                                .iter()
                                .filter(|d| !matches!(d.name.as_str(), "blink" | "inverse"))
                                .cloned()
                                .collect::<Vec<_>>(),
                        )?,
                    });
                    let l = link.as_ref().unwrap();
                    l.validate()?;
                    l.config.effective(p).validate_complete()?;
                    let options = super::RenderOptions {
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
                    ensure!(l.uri(p, &options).is_some(), "encoded link URI is too long");
                } else {
                    for d in ds {
                        apply_style(p, &mut style, &d)?;
                    }
                }
                Ok((style, link, false, false))
            })();
            match tag_result {
                Ok((style, link, pop, reset)) => {
                    if !current.text.is_empty() {
                        spans.push(current.clone());
                        current.text.clear();
                    }
                    if pop {
                        stack.pop();
                    } else if reset {
                        stack.clear();
                    } else {
                        stack.push((current.style.clone(), current.link.clone()));
                    }
                    current.style = style;
                    current.link = link;
                    rest = &rest[end.unwrap() + 1..];
                    continue;
                }
                Err(e) if strict => return Err(e),
                Err(_) => {}
            }
        }
        let c = rest.chars().next().unwrap();
        if !c.is_control() || matches!(c, '\n' | '\r' | '\t') {
            current.text.push(c);
        }
        rest = &rest[c.len_utf8()..];
    }
    ensure!(!strict || stack.is_empty(), "unclosed style tag");
    if !current.text.is_empty() {
        spans.push(current);
    }
    Ok(spans)
}
