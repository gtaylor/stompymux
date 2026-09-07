//! Include expansion with provenance, followed by validation of recognized leaves.
use super::{
    catalog::{KEYS, KeySpec},
    types::*,
};
use anyhow::{Context, Result, bail, ensure};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
use toml::Value;
#[derive(Default)]
/// Merged values, source provenance and nonfatal diagnostics.
pub struct Document {
    pub values: toml::Table,
    pub origins: BTreeMap<String, PathBuf>,
    pub warnings: Vec<String>,
}
/// Merge ordered values and provenance using the C TOML loader's rules.
fn merge(a: &mut Document, b: Document) {
    fn value(
        a: &mut Value,
        b: Value,
        path: &str,
        origins: &mut BTreeMap<String, PathBuf>,
        source: &BTreeMap<String, PathBuf>,
    ) {
        if let (Value::Table(old), Value::Table(new)) = (&mut *a, &b) {
            for (key, v) in new {
                let child = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                value(
                    old.entry(key.clone())
                        .or_insert(Value::String(String::new())),
                    v.clone(),
                    &child,
                    origins,
                    source,
                );
            }
            if let Some(file) = source.get(path) {
                origins.insert(path.into(), file.clone());
            }
            return;
        }
        if let (Value::Array(old), Value::Array(new)) = (&mut *a, &b)
            && old.iter().all(Value::is_table)
            && new.iter().all(Value::is_table)
        {
            let offset = old.len();
            for (index, item) in new.iter().enumerate() {
                let from = format!("{path}[{index}]");
                let to = format!("{path}[{}]", offset + index);
                for (key, file) in source {
                    if key == &from || key.starts_with(&format!("{from}.")) {
                        origins.insert(format!("{to}{}", &key[from.len()..]), file.clone());
                    }
                }
                old.push(item.clone());
            }
            return;
        }
        origins.retain(|key, _| {
            key != path
                && !key.starts_with(&format!("{path}."))
                && !key.starts_with(&format!("{path}["))
        });
        for (key, file) in source {
            if key == path
                || key.starts_with(&format!("{path}."))
                || key.starts_with(&format!("{path}["))
            {
                origins.insert(key.clone(), file.clone());
            }
        }
        *a = b;
    }
    let mut root = Value::Table(std::mem::take(&mut a.values));
    value(
        &mut root,
        Value::Table(b.values),
        "",
        &mut a.origins,
        &b.origins,
    );
    a.values = root.as_table().unwrap().clone();
    a.warnings.extend(b.warnings);
}
/// Read and merge a configuration file and its recursive includes.
pub fn read(path: &Path) -> Result<Document> {
    let mut doc = read_inner(path, &mut BTreeSet::new())?;
    let mut values = toml::Table::new();
    let origins = doc.origins.clone();
    filter(
        std::mem::take(&mut doc.values),
        &mut values,
        "",
        path,
        &origins,
        &mut doc.warnings,
    )?;
    doc.values = values;
    Ok(doc)
}
/// Expand one include while tracking recursion and source ownership.
fn read_inner(path: &Path, stack: &mut BTreeSet<PathBuf>) -> Result<Document> {
    let path = path
        .canonicalize()
        .with_context(|| format!("reading {}", path.display()))?;
    ensure!(
        stack.len() <= 8,
        "{}: include nesting exceeds 8",
        path.display()
    );
    ensure!(
        stack.insert(path.clone()),
        "{}: configuration include cycle",
        path.display()
    );
    let mut table: toml::Table = toml::from_str(&std::fs::read_to_string(&path)?)
        .with_context(|| format!("{}: invalid TOML", path.display()))?;
    let mut result = Document::default();
    if let Some(include) = table.remove("include") {
        for entry in include
            .as_array()
            .with_context(|| format!("{}: include must be an array", path.display()))?
        {
            let relative = entry
                .as_str()
                .with_context(|| format!("{}: include entries must be strings", path.display()))?;
            let next = read_inner(&path.parent().unwrap().join(relative), stack)?;
            merge(&mut result, next);
        }
    }
    let mut own = Document {
        values: table,
        ..Default::default()
    };
    for (key, value) in &own.values {
        record_origins(key, value, &path, &mut own.origins);
    }
    merge(&mut result, own);
    stack.remove(&path);
    Ok(result)
}
/// Validate known leaves and omit unknown settings with diagnostics.
fn filter(
    table: toml::Table,
    out: &mut toml::Table,
    prefix: &str,
    source: &Path,
    origins: &BTreeMap<String, PathBuf>,
    warnings: &mut Vec<String>,
) -> Result<()> {
    for (key, mut value) in table {
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        let source = origins.get(&path).map(PathBuf::as_path).unwrap_or(source);
        if let Some(spec) = KEYS.iter().find(|s| s.path == path) {
            normalize(&path, &mut value);
            clean_entry_keys(&path, &mut value, source, warnings);
            if spec.kind == "Vec<SiteRule>"
                && let Some(entries) = value.as_array()
            {
                for (index, entry) in entries.iter().enumerate() {
                    let item = format!("{path}[{index}]");
                    let file = origins.get(&item).map(PathBuf::as_path).unwrap_or(source);
                    validate(spec, &Value::Array(vec![entry.clone()]))
                        .with_context(|| format!("{}: {item}", file.display()))?;
                }
            }
            validate(spec, &value).with_context(|| format!("{}: {path}", source.display()))?;
            out.insert(key, value);
        } else if KEYS.iter().any(|s| s.path.starts_with(&format!("{path}."))) {
            let table = value
                .as_table()
                .with_context(|| format!("{}: {path} must be a table", source.display()))?;
            let mut child = toml::Table::new();
            filter(table.clone(), &mut child, &path, source, origins, warnings)?;
            out.insert(key, Value::Table(child));
        } else {
            warnings.push(format!(
                "{}: unknown configuration key {path}; skipped",
                source.display()
            ));
        }
    }
    Ok(())
}
/// Associate a value and its nested map entries with their defining source.
fn record_origins(
    path: &str,
    value: &Value,
    source: &Path,
    origins: &mut BTreeMap<String, PathBuf>,
) {
    origins.insert(path.into(), source.into());
    if let Value::Array(items) = value {
        for (index, item) in items.iter().enumerate() {
            record_origins(&format!("{path}[{index}]"), item, source, origins);
        }
    }
    if let Value::Table(t) = value {
        for (k, v) in t {
            record_origins(&format!("{path}.{k}"), v, source, origins);
        }
    }
}
/// Canonicalize case-insensitive legacy token arrays before deserialization.
fn normalize(path: &str, value: &mut Value) {
    let flags = (path.starts_with("mux.default_") && path.ends_with("_flags"))
        || path == "logging.log_options";
    if flags && let Some(values) = value.as_array_mut() {
        for value in values {
            if let Some(name) = value.as_str() {
                *value = Value::String(name.to_ascii_lowercase());
            }
        }
    }
}
/// Validate a structured leaf through its typed deserializer.
fn typed<T: serde::de::DeserializeOwned>(value: &Value) -> Result<()> {
    let _: T = value.clone().try_into()?;
    Ok(())
}
/// Require a TOML table for a map-shaped configuration value.
fn table(value: &Value) -> Result<&toml::Table> {
    value.as_table().context("expected a table")
}
/// Validate catalog leaf types, numeric bounds and structural constraints.
pub fn validate(spec: &KeySpec, value: &Value) -> Result<()> {
    match spec.kind {
        "i64" | "usize" | "u64" | "u16" => {
            ensure!(value.is_integer(), "expected an integer");
        }
        "f64" => {
            ensure!(value.is_float() || value.is_integer(), "expected a number");
        }
        "bool" => typed::<bool>(value)?,
        "String" | "PathBuf" => typed::<String>(value)?,
        "IpAddr" => typed::<std::net::IpAddr>(value)?,
        "ErrorReporting" => typed::<ErrorReporting>(value)?,
        "Vec<Flag>" => typed::<Vec<String>>(value)?,
        "Vec<LogOption>" => typed::<Vec<LogOption>>(value)?,
        "Vec<String>" => typed::<Vec<String>>(value)?,
        "BTreeMap<String, String>" => {
            for (k, v) in table(value)? {
                typed::<String>(v).with_context(|| k.clone())?;
            }
        }
        "BTreeMap<String, Permissions>" => {
            for (k, v) in table(value)? {
                typed::<Permissions>(v).with_context(|| k.clone())?;
            }
        }
        "BTreeMap<String, Rgb>" => {
            for (k, v) in table(value)? {
                ensure!(
                    !k.is_empty()
                        && k.len() <= 60
                        && k.bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b)),
                    "invalid color name {k}"
                );
                typed::<Rgb>(v).with_context(|| k.clone())?;
            }
        }
        "Vec<SiteRule>" => {
            let rules: Vec<SiteRule> = value.clone().try_into()?;
            for rule in rules {
                ensure!(
                    rule.address.is_ipv4() == rule.mask.is_ipv4(),
                    "site address/mask families differ"
                );
            }
        }
        "BTreeMap<BootstrapId, BootstrapObject>" => {
            typed::<BTreeMap<BootstrapId, BootstrapObject>>(value)?;
            let mut ids = BTreeSet::new();
            for key in table(value)?.keys() {
                ensure!(
                    ids.insert(BootstrapId::try_from(key.clone()).map_err(anyhow::Error::msg)?),
                    "duplicate numeric bootstrap dbref {key}"
                );
            }
        }
        other => bail!("unhandled catalog type {other}"),
    }
    if let Some((min, max)) = spec.bounds {
        let number = value
            .as_integer()
            .map(|v| v as f64)
            .or_else(|| value.as_float())
            .context("expected a number")?;
        ensure!(
            number.is_finite() && number >= min && number <= max,
            "expected a finite value in {min}..={max}"
        );
    }
    if spec.path == "osc8.presets" {
        for name in table(value)?.keys() {
            ensure!(
                !name.is_empty()
                    && name.len() <= 60
                    && name.as_bytes()[0].is_ascii_alphanumeric()
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._~-".contains(&b)),
                "invalid preset name {name}"
            );
        }
    }
    Ok(())
}

/// Warn and discard unknown properties inside structured array/map entries.
fn clean_entry_keys(path: &str, value: &mut Value, source: &Path, warnings: &mut Vec<String>) {
    let mut clean = |table: &mut toml::Table, allowed: &[&str], prefix: &str| {
        table.retain(|key, _| {
            let known = allowed.contains(&key);
            if !known {
                warnings.push(format!(
                    "{}: unknown configuration key {prefix}.{key}; skipped",
                    source.display()
                ));
            }
            known
        });
    };
    if path == "database.bootstrap.objects" {
        if let Some(table) = value.as_table_mut() {
            for (id, entry) in table {
                if let Some(entry) = entry.as_table_mut() {
                    clean(entry, &["type", "name", "wizard"], &format!("{path}.{id}"));
                }
            }
        }
    } else if path.starts_with("sites.")
        && let Some(entries) = value.as_array_mut()
    {
        for (index, entry) in entries.iter_mut().enumerate() {
            if let Some(table) = entry.as_table_mut() {
                clean(table, &["address", "mask"], &format!("{path}[{index}]"));
            }
        }
    }
}
/// Resolve default flags after includes have supplied the complete alias table.
pub fn resolve_flags(doc: &mut Document) -> Result<()> {
    let mut aliases = BTreeMap::<String, String>::new();
    if let Some(table) = doc
        .values
        .get("aliases")
        .and_then(|v| v.get("flags"))
        .and_then(Value::as_table)
    {
        for (name, value) in table {
            let target = value
                .as_str()
                .context("flag alias target must be a string")?
                .to_ascii_lowercase();
            let origin = doc.origins.get(&format!("aliases.flags.{name}"));
            let _: Flag = Value::String(target.clone()).try_into().with_context(|| {
                format!(
                    "{}: aliases.flags.{name}: unknown target {target}",
                    origin.map(|p| p.display().to_string()).unwrap_or_default()
                )
            })?;
            let key = name.to_ascii_lowercase();
            ensure!(
                aliases
                    .insert(key.clone(), target.clone())
                    .is_none_or(|old| old == target),
                "aliases.flags.{name}: conflicting case-insensitive mappings"
            );
            if let Ok(flag) = Value::String(key).try_into::<Flag>() {
                ensure!(
                    flag.world_name().eq_ignore_ascii_case(&target),
                    "aliases.flags.{name}: cannot rebind a built-in flag"
                );
            }
        }
    }
    if let Some(mux) = doc.values.get_mut("mux").and_then(Value::as_table_mut) {
        for kind in ["player", "room", "thing", "exit"] {
            let key = format!("default_{kind}_flags");
            if let Some(flags) = mux.get_mut(&key).and_then(Value::as_array_mut) {
                for flag in flags {
                    let name = flag
                        .as_str()
                        .context("expected flag name")?
                        .to_ascii_lowercase();
                    let canonical = aliases.get(&name).unwrap_or(&name).clone();
                    let _: Flag =
                        Value::String(canonical.clone())
                            .try_into()
                            .with_context(|| {
                                format!(
                                    "{}: mux.{key}: unknown flag {name}",
                                    doc.origins
                                        .get(&format!("mux.{key}"))
                                        .map(|p| p.display().to_string())
                                        .unwrap_or_default()
                                )
                            })?;
                    *flag = Value::String(canonical);
                }
            }
        }
    }
    Ok(())
}
