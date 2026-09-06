//! Typed, binary-safe object state shared by native commands, Lua and persistence.
pub mod commands;
use crate::{
    config::Config,
    world::{Kind, ObjectId, World},
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Legacy name limits are byte counts, excluding the terminating NUL.
pub const NAMESPACE_LIMIT: usize = 127;
pub const KEY_LIMIT: usize = 255;

/// Tagged snapshots retain integers, floating-point values and arbitrary string bytes exactly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Value {
    Boolean(bool),
    Integer(i64),
    Number(f64),
    String(Vec<u8>),
}

/// Ordered namespaces and keys match the C bytewise enumeration order.
pub type State = BTreeMap<String, BTreeMap<String, Value>>;

/// Runtime-only object incarnation; rollback cannot make provisional handles valid again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Generation(u64);
impl Default for Generation {
    fn default() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

/// Match C names, including slash-separated policy keys, without locale-dependent classes.
pub fn valid_name(name: &str, limit: usize) -> bool {
    !name.is_empty()
        && name.len() <= limit
        && name.as_bytes()[0].is_ascii_alphabetic()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-./".contains(&b))
}

/// Validate addresses consistently for reads, writes, commands and Lua handles.
pub fn address(namespace: &str, key: Option<&str>) -> Result<()> {
    ensure!(
        valid_name(namespace, NAMESPACE_LIMIT),
        "invalid state namespace"
    );
    if let Some(key) = key {
        ensure!(valid_name(key, KEY_LIMIT), "invalid state key");
    }
    Ok(())
}

impl Value {
    /// C state quotas count value payload bytes, not serialized JSON overhead.
    pub fn bytes(&self) -> usize {
        match self {
            Self::Boolean(_) => 1,
            Self::Integer(_) | Self::Number(_) => 8,
            Self::String(s) => s.len(),
        }
    }

    /// Stable type labels used by native inspection.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Boolean(_) => "boolean",
            Self::Integer(_) => "integer",
            Self::Number(_) => "number",
            Self::String(_) => "string",
        }
    }

    /// Human inspection never interprets binary or bracket bytes as game markup.
    pub fn display(&self) -> String {
        match self {
            Self::Boolean(v) => v.to_string(),
            Self::Integer(v) => v.to_string(),
            Self::Number(v) => number_display(*v),
            Self::String(v) => quoted(v),
        }
    }
}

/// Render the C inspection precision (17 significant digits, general notation).
fn number_display(value: f64) -> String {
    let scientific = format!("{value:.16e}");
    let (mantissa, exponent) = scientific.split_once('e').expect("scientific exponent");
    let exponent: i32 = exponent.parse().expect("numeric exponent");
    let mantissa = mantissa.trim_end_matches('0').trim_end_matches('.');
    if !(-4..17).contains(&exponent) {
        return format!("{mantissa}e{exponent:+03}");
    }
    let sign = if value.is_sign_negative() { "-" } else { "" };
    let digits = mantissa.trim_start_matches('-').replace('.', "");
    let position = exponent + 1;
    if position <= 0 {
        format!("{sign}0.{}{digits}", "0".repeat((-position) as usize))
    } else if position as usize >= digits.len() {
        format!(
            "{sign}{digits}{}",
            "0".repeat(position as usize - digits.len())
        )
    } else {
        let (a, b) = digits.split_at(position as usize);
        format!("{sign}{a}.{b}")
    }
}

/// Escape each byte as the C inspection command does, preserving even malformed UTF-8.
pub fn quoted(bytes: &[u8]) -> String {
    let mut s = String::from("\"");
    for b in bytes {
        match b {
            b'"' => s.push_str("\\\""),
            b'\\' => s.push_str("\\\\"),
            b'\n' => s.push_str("\\n"),
            b'\r' => s.push_str("\\r"),
            b'\t' => s.push_str("\\t"),
            32..=126 => s.push(*b as char),
            _ => s.push_str(&format!("\\x{b:02X}")),
        }
    }
    s.push('"');
    s
}

/// Validate the final collection with checked byte accounting, independent of serialization.
pub fn validate(state: &State, c: &Config) -> Result<()> {
    let mut entries = 0usize;
    let mut total = 0usize;
    for (ns, values) in state {
        address(ns, None)?;
        for (key, value) in values {
            address(ns, Some(key))?;
            ensure!(
                !matches!(value,Value::Number(v) if !v.is_finite()),
                "state number must be finite"
            );
            ensure!(
                value.bytes() <= c.lua.state_value_limit,
                "state value exceeds {} bytes",
                c.lua.state_value_limit
            );
            entries = entries
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("state count overflow"))?;
            total = total
                .checked_add(ns.len() + 1 + key.len() + 1)
                .and_then(|n| n.checked_add(value.bytes()))
                .ok_or_else(|| anyhow::anyhow!("state byte count overflow"))?;
        }
    }
    ensure!(
        entries <= c.lua.state_entry_limit,
        "object state exceeds {} entries",
        c.lua.state_entry_limit
    );
    ensure!(
        total <= c.lua.state_object_limit,
        "object state exceeds {} bytes",
        c.lua.state_object_limit
    );
    Ok(())
}

/// Apply one atomic state mutation and publish only its validated final collection.
pub fn change<T>(
    w: &mut World,
    c: &Config,
    id: ObjectId,
    work: impl FnOnce(&mut State) -> Result<T>,
) -> Result<T> {
    let object = w
        .objects
        .get_mut(&id)
        .filter(|o| o.kind != Kind::Garbage)
        .ok_or_else(|| anyhow::anyhow!("invalid state object #{}", id.0))?;
    let mut candidate = object.state.clone();
    let result = work(&mut candidate)?;
    candidate.retain(|_, values| !values.is_empty());
    validate(&candidate, c)?;
    object.state = candidate;
    Ok(result)
}

/// Set or remove one key after validating the address, even for a missing deletion.
pub fn set(state: &mut State, ns: &str, key: &str, value: Option<Value>) -> Result<()> {
    address(ns, Some(key))?;
    if let Some(value) = value {
        state
            .entry(ns.into())
            .or_default()
            .insert(key.into(), value);
    } else if let Some(values) = state.get_mut(ns) {
        values.remove(key);
    }
    Ok(())
}
