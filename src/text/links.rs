//! Validated OSC 8 actions and capability-filtered structured link metadata.
use super::{
    Palette, RenderOptions, Style,
    parser::{Directive, boolean},
};
use anyhow::{Result, ensure};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

/// C-supported interactive link style states.
pub const STATES: &[&str] = &[
    "active",
    "hover",
    "focus-visible",
    "focus",
    "visited",
    "selected",
    "disabled",
    "link",
    "any-link",
];

/// Link metadata retains explicit overrides separately from its referenced preset.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LinkConfig {
    pub preset: Option<String>,
    pub(crate) fields: BTreeMap<String, Value>,
}

/// Validated actions shared by bracket markup, Markdown and HTML.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkKind {
    External,
    Send,
    Prompt,
}

impl LinkKind {
    /// Canonical bracket-markup spelling for this action.
    pub fn name(self) -> &'static str {
        match self {
            Self::External => "link",
            Self::Send => "send",
            Self::Prompt => "prompt",
        }
    }
}

impl std::fmt::Display for LinkKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// Visible-label action with validated destination and presentation metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    pub kind: LinkKind,
    pub target: String,
    pub config: LinkConfig,
}

/// Percent encoding used by C command and configuration URI components.
pub fn percent(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// Shared external URL policy for terminal and HTML output.
pub fn external(s: &str) -> bool {
    let Some((scheme, body)) = s.split_once(':') else {
        return false;
    };
    if !matches!(
        scheme.to_ascii_lowercase().as_str(),
        "http" | "https" | "ftp"
    ) || body.is_empty()
        || s.len() > super::OSC8_URI_LIMIT
    {
        return false;
    }
    let mut bytes = s.bytes();
    while let Some(b) = bytes.next() {
        if b == b'%' {
            if !bytes.next().is_some_and(|b| b.is_ascii_hexdigit())
                || !bytes.next().is_some_and(|b| b.is_ascii_hexdigit())
            {
                return false;
            }
        } else if !b.is_ascii_alphanumeric() && !b"-._~:/?#[]@!$&'()*+,;=".contains(&b) {
            return false;
        }
    }
    true
}

impl LinkConfig {
    /// Decode C link properties, retaining explicit preset overrides.
    pub(super) fn parse(p: &Palette, ds: &[Directive]) -> Result<Self> {
        let mut config = Self::default();
        for d in ds {
            let name = d.name.as_str();
            let v = d.value.as_deref().unwrap_or("");
            if name == "preset" {
                ensure!(
                    d.quoted && !v.is_empty() && p.presets.contains_key(v),
                    "unknown OSC 8 preset"
                );
                ensure!(config.preset.is_none(), "duplicate preset");
                config.preset = Some(v.into());
                continue;
            }
            let val = if matches!(
                name,
                "tooltip" | "title" | "selection.group" | "selection.value"
            ) {
                ensure!(
                    d.quoted && !v.is_empty(),
                    "{name} must be quoted and nonempty"
                );
                json!(v)
            } else if matches!(
                name,
                "spoiler"
                    | "disabled"
                    | "selection.toggle"
                    | "selection.selected"
                    | "selection.exclusive"
                    | "selection.disabled"
                    | "visibility.wholeline"
                    | "visibility.expire.input"
                    | "visibility.expire.prompt"
                    | "visibility.expire.output"
            ) {
                json!(boolean(d)?)
            } else if matches!(name, "visibility.delay" | "visibility.expire.outputdelay") {
                ensure!(
                    !d.quoted && !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()),
                    "delay must be an unsigned decimal integer"
                );
                json!(v.parse::<u32>()?)
            } else if name == "visibility.action" {
                ensure!(
                    !d.quoted
                        && matches!(
                            v.to_ascii_lowercase().as_str(),
                            "conceal" | "reveal" | "reveal,conceal"
                        ),
                    "invalid visibility action"
                );
                json!(v.to_ascii_lowercase())
            } else if let Some(tail) = name.strip_prefix("menu.") {
                let (i, key) = tail
                    .split_once('.')
                    .ok_or_else(|| anyhow::anyhow!("invalid menu property"))?;
                ensure!(i.bytes().all(|b| b.is_ascii_digit()), "invalid menu index");
                let i: usize = i.parse()?;
                ensure!(
                    (1..=super::OSC8_URI_LIMIT).contains(&i),
                    "invalid menu index"
                );
                if key == "separator" {
                    {
                        ensure!(d.value.is_none(), "separator takes no value");
                        json!(true)
                    }
                } else {
                    ensure!(
                        matches!(key, "label" | "send" | "prompt" | "link") && d.quoted,
                        "invalid menu property"
                    );
                    if key == "link" {
                        ensure!(external(v), "invalid menu URL");
                    }
                    json!(v)
                }
            } else {
                let key = if let Some((state, key)) = name.split_once('.') {
                    ensure!(
                        state == "title" || STATES.contains(&state),
                        "unknown link style state"
                    );
                    key
                } else {
                    name
                };
                match key {
                    "fg" | "color" | "bg" | "text-decoration-color" => match p.color(v)? {
                        Some([r, g, b]) => json!(format!("#{r:02x}{g:02x}{b:02x}")),
                        None => json!("default"),
                    },
                    "bold" | "italic" => json!(boolean(d)?),
                    "underline" | "overline" | "strikethrough" => {
                        if matches!(
                            v.to_ascii_lowercase().as_str(),
                            "wavy" | "dotted" | "dashed"
                        ) {
                            json!(v.to_ascii_lowercase())
                        } else {
                            json!(boolean(d)?)
                        }
                    }
                    "blink" | "inverse" if !name.contains('.') => json!(boolean(d)?),
                    _ => anyhow::bail!("unknown OSC 8 property {name}"),
                }
            };
            ensure!(
                !config.fields.contains_key(name) || !name.starts_with("menu."),
                "duplicate menu property"
            );
            let name = if name == "fg" {
                "color".into()
            } else if let Some(prefix) = name.strip_suffix(".fg") {
                format!("{prefix}.color")
            } else {
                name.to_string()
            };
            config.fields.insert(name, val);
        }
        let mut menus: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for name in config.fields.keys() {
            if let Some(t) = name.strip_prefix("menu.") {
                let (i, k) = t.split_once('.').unwrap();
                menus.entry(i).or_default().push(k);
            }
        }
        for keys in menus.values() {
            if keys.contains(&"separator") {
                ensure!(keys.len() == 1, "separator cannot have actions");
            } else {
                ensure!(
                    keys.contains(&"label")
                        && keys
                            .iter()
                            .filter(|k| matches!(**k, "send" | "prompt" | "link"))
                            .count()
                            == 1,
                    "menu needs label and one action"
                );
            }
        }
        Ok(config)
    }

    /// Structural constraints are checked after preset overlays have been merged.
    pub fn validate_complete(&self) -> Result<()> {
        let f = &self.fields;
        let has = |prefix: &str| f.keys().any(|k| k.starts_with(prefix));
        ensure!(
            !f.contains_key("title") || has("menu."),
            "menu title requires a menu"
        );
        ensure!(
            !has("title.") || f.contains_key("title"),
            "title style requires title text"
        );
        let indices: std::collections::BTreeSet<usize> = f
            .keys()
            .filter_map(|k| {
                k.strip_prefix("menu.")
                    .and_then(|k| k.split('.').next())
                    .and_then(|k| k.parse().ok())
            })
            .collect();
        ensure!(
            indices.iter().copied().eq(1..=indices.len()),
            "menu indices must be contiguous"
        );
        ensure!(
            !has("visibility.") || f.contains_key("visibility.action"),
            "visibility requires an action"
        );
        ensure!(
            !has("visibility.expire.")
                || ["input", "prompt", "output"]
                    .iter()
                    .any(|k| f.get(&format!("visibility.expire.{k}")) == Some(&json!(true))),
            "expiry requires an enabled trigger"
        );
        ensure!(
            !f.contains_key("visibility.expire.outputdelay")
                || f.get("visibility.expire.output") == Some(&json!(true)),
            "outputDelay requires output expiry"
        );
        ensure!(
            !has("selection.")
                || (f.contains_key("selection.group") && f.contains_key("selection.value")),
            "selection requires group and value"
        );
        Ok(())
    }

    pub fn effective(&self, p: &Palette) -> Self {
        let mut base = self
            .preset
            .as_ref()
            .and_then(|n| p.presets.get(n))
            .cloned()
            .unwrap_or_default();
        if self.fields.keys().any(|k| k.starts_with("menu.")) {
            base.fields.retain(|k, _| !k.starts_with("menu."));
        }
        base.fields.extend(self.fields.clone());
        base.preset = None;
        base
    }

    /// Base ANSI styles remain useful when richer OSC capabilities are unavailable.
    pub fn fallback(&self, p: &Palette, style: &mut Style) {
        for (name, v) in &self.effective(p).fields {
            if name.contains('.') {
                continue;
            }
            let d = Directive {
                name: name.clone(),
                value: Some(match v {
                    Value::String(s) => s.clone(),
                    _ => v.to_string(),
                }),
                quoted: false,
            };
            let _ = super::parser::apply_style(p, style, &d);
        }
    }

    /// Serialize a definition using only capabilities advertised by the receiver.
    pub fn json(&self, o: &RenderOptions) -> Value {
        self.json_with_menu(o, self.has_menu(o))
    }

    /// Determine whether any menu action is usable by this receiver.
    fn has_menu(&self, o: &RenderOptions) -> bool {
        o.has("MENU")
            && self.fields.keys().any(|key| {
                key.starts_with("menu.")
                    && key.rsplit('.').next().is_some_and(|kind| {
                        matches!(kind, "send" | "prompt" | "link") && o.link_enabled(kind)
                    })
            })
    }

    /// Serialize filtered properties using the effective menu availability.
    fn json_with_menu(&self, o: &RenderOptions, menu_available: bool) -> Value {
        let mut root = Map::new();
        let mut style = Map::new();
        let mut title_style = Map::new();
        let mut menu: BTreeMap<usize, Map<String, Value>> = BTreeMap::new();
        for (name, v) in &self.fields {
            let (group, key) = name.split_once('.').unwrap_or(("", name));
            match group {
                "menu" if o.has("MENU") => {
                    let (i, k) = key.split_once('.').unwrap();
                    menu.entry(i.parse().unwrap())
                        .or_default()
                        .insert(k.into(), v.clone());
                }
                "visibility" if o.has("VISIBILITY") => {
                    insert_path(
                        &mut root,
                        &["visibility", key],
                        if key == "action" && v == "reveal,conceal" {
                            json!(["reveal", "conceal"])
                        } else {
                            v.clone()
                        },
                    );
                }
                "selection" if o.has("SELECTION") => {
                    insert_path(&mut root, &["selection", key], v.clone());
                }
                "title" if o.has("MENU") && o.has("STYLE_BASIC") => {
                    title_style.insert(canonical(key).into(), v.clone());
                }
                "" => match key {
                    "tooltip" if o.has("TOOLTIP") => {
                        root.insert(key.into(), v.clone());
                    }
                    "title" if o.has("MENU") => {
                        root.insert(key.into(), v.clone());
                    }
                    "spoiler" if o.has("SPOILER") => {
                        root.insert(key.into(), v.clone());
                    }
                    "disabled" if o.has("DISABLED") => {
                        root.insert(key.into(), v.clone());
                    }
                    "fg"
                    | "color"
                    | "bg"
                    | "bold"
                    | "italic"
                    | "underline"
                    | "overline"
                    | "strikethrough"
                    | "text-decoration-color"
                        if o.has("STYLE_BASIC") =>
                    {
                        style.insert(canonical(key).into(), v.clone());
                    }
                    _ => {}
                },
                state if STATES.contains(&state) && o.has("STYLE_STATES") => {
                    insert_path(&mut style, &[state, canonical(key)], v.clone());
                }
                _ => {}
            }
        }
        if !style.is_empty() {
            root.insert("style".into(), Value::Object(style));
        }
        if !title_style.is_empty()
            && let Some(title) = root.remove("title")
        {
            root.insert("title".into(), json!({"text":title,"style":title_style}));
        }
        let mut items = Vec::new();
        let mut separator = false;
        for item in menu.values() {
            if item.get("separator") == Some(&json!(true)) {
                separator = !items.is_empty();
                continue;
            }
            if let Some((kind, target)) = ["send", "prompt", "link"].into_iter().find_map(|k| {
                item.get(k)
                    .and_then(Value::as_str)
                    .filter(|_| o.link_enabled(k))
                    .map(|v| (k, v))
            }) {
                if separator {
                    items.push(json!("-"));
                    separator = false;
                }
                if let Some(label) = item.get("label").and_then(Value::as_str) {
                    let mut v = Map::new();
                    v.insert(
                        label.into(),
                        json!(if kind == "link" {
                            target.into()
                        } else {
                            format!("{kind}:{target}")
                        }),
                    );
                    items.push(Value::Object(v));
                }
            }
        }
        if !items.is_empty() {
            root.insert("menu".into(), json!(items));
        }
        if !menu_available {
            root.remove("title");
        }
        let mut v = Value::Object(root);
        if o.has("COMPACT") {
            compact(&mut v, "");
        }
        v
    }
}

/// Normalize property aliases before merging overrides.
fn canonical(k: &str) -> &str {
    match k {
        "fg" => "color",
        _ => k,
    }
}

/// Build nested JSON properties from validated dotted keys.
fn insert_path(map: &mut Map<String, Value>, path: &[&str], value: Value) {
    let key = path[0];
    if path.len() == 1 {
        if let Some((a, b)) = key.split_once('.') {
            insert_path(
                map,
                &[a, if b == "outputdelay" { "outputDelay" } else { b }],
                value,
            );
        } else {
            map.insert(key.into(), value);
        }
        return;
    }
    let child = map.entry(key).or_insert_with(|| json!({}));
    insert_path(child.as_object_mut().unwrap(), &path[1..], value);
}

/// Apply the C compact-key encoding recursively.
fn compact(v: &mut Value, parent: &str) {
    if let Value::Object(map) = v {
        let old = std::mem::take(map);
        for (k, mut value) in old {
            compact(&mut value, &k);
            let key = match (parent, k.as_str()) {
                ("", "style") | ("title", "style") => "s",
                ("", "tooltip") => "t",
                ("", "title") => "ti",
                ("", "menu") => "m",
                ("", "visibility") => "v",
                ("", "selection") => "sel",
                ("", "spoiler") => "sp",
                ("", "disabled") => "d",
                ("style", "active") => "a",
                ("style", "hover") => "h",
                ("style", "focus-visible") => "fv",
                ("style", "focus") => "f",
                ("style", "visited") => "vi",
                ("style", "selected") => "sl",
                ("style", "disabled") => "d",
                ("style", "link") => "l",
                ("style", "any-link") => "al",
                (_, "color") => "c",
                (_, "bold") => "b",
                (_, "italic") => "i",
                (_, "underline") => "u",
                (_, "overline") => "o",
                (_, "strikethrough") => "st",
                (_, "text-decoration-color") => "tdc",
                _ => &k,
            };
            map.insert(key.into(), value);
        }
    }
}

impl Link {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.target.is_empty() && !self.target.chars().any(char::is_control),
            "invalid link target"
        );
        if self.kind == LinkKind::External {
            ensure!(
                external(&self.target),
                "link URI must be percent-encoded http, https or ftp"
            );
        } else {
            ensure!(
                self.target.len() <= super::OSC8_URI_LIMIT
                    && self.kind.name().len() + 1 + percent(&self.target).len()
                        <= super::OSC8_URI_LIMIT,
                "encoded link is too long"
            );
        }
        Ok(())
    }

    pub fn uri(&self, p: &Palette, o: &RenderOptions) -> Option<String> {
        self.validate().ok()?;
        if !o.link_enabled(self.kind.name()) {
            return None;
        }
        let effective = self.config.effective(p);
        if effective.fields.get("disabled") == Some(&json!(true)) && !o.has("DISABLED") {
            return None;
        }
        let mut uri = if self.kind == LinkKind::External {
            self.target.clone()
        } else {
            format!("{}:{}", self.kind, percent(&self.target))
        };
        if self.kind == LinkKind::External {
            let (base, fragment) = uri
                .split_once('#')
                .map_or((uri.as_str(), None), |(a, b)| (a, Some(b)));
            if let Some((path, query)) = base.split_once('?') {
                let query = query
                    .split('&')
                    .map(|part| {
                        let (name, tail) = part
                            .split_once('=')
                            .map_or((part, None), |(a, b)| (a, Some(b)));
                        let name = if name == "config"
                            && o.capabilities
                                .iter()
                                .any(|c| !matches!(c.as_str(), "" | "SEND" | "PROMPT" | "PRESETS"))
                        {
                            "%63%6F%6E%66%69%67"
                        } else if name == "preset" && o.has("PRESETS") {
                            "%70%72%65%73%65%74"
                        } else {
                            name
                        };
                        format!("{name}{}", tail.map_or(String::new(), |v| format!("={v}")))
                    })
                    .collect::<Vec<_>>()
                    .join("&");
                uri = format!(
                    "{path}?{query}{}",
                    fragment.map_or(String::new(), |f| format!("#{f}"))
                );
            }
        }
        let mut config = if o.has("PRESETS") && self.config.preset.is_some() {
            self.config.clone()
        } else {
            effective.clone()
        };
        if o.has("PRESETS")
            && self
                .config
                .fields
                .keys()
                .any(|k| k == "title" || k.starts_with("title."))
        {
            config.fields.extend(
                effective
                    .fields
                    .iter()
                    .filter(|(k, _)| *k == "title" || k.starts_with("title."))
                    .map(|(k, v)| (k.clone(), v.clone())),
            );
        }
        let mut params = Vec::new();
        if o.has("PRESETS")
            && let Some(n) = &self.config.preset
        {
            params.push(format!("preset={}", percent(n)));
        }
        let v = config.json_with_menu(o, effective.has_menu(o));
        if v.as_object().is_some_and(|m| !m.is_empty()) {
            params.push(format!("config={}", percent(&v.to_string())));
        }
        if !params.is_empty() {
            let (base, fragment) = uri
                .split_once('#')
                .map_or((uri.as_str(), None), |(a, b)| (a, Some(b)));
            uri = format!(
                "{base}{}{}{}",
                if base.contains('?') { "&" } else { "?" },
                params.join("&"),
                fragment.map_or(String::new(), |f| format!("#{f}"))
            );
        }
        (uri.len() <= super::OSC8_URI_LIMIT).then_some(uri)
    }
}
