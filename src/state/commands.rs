//! Wizard state inspection and atomic typed mutations, compatible with C command syntax.
use super::{State, Value};
use crate::commands::{Action, CommandContext, CommandInput, target::admin_target};
use anyhow::{Context, Result, bail, ensure};

/// Parse C's explicit string escapes before trying scalar literals.
pub fn parse_value(source: &str) -> Result<Option<Value>> {
    let source = source.trim();
    if source.is_empty() {
        return Ok(None);
    }
    if source.starts_with('"') {
        ensure!(
            source.len() >= 2 && source.ends_with('"'),
            "Unterminated quoted state string"
        );
        let mut out = Vec::new();
        let mut input = source.as_bytes()[1..source.len() - 1].iter().copied();
        while let Some(b) = input.next() {
            if b != b'\\' {
                ensure!(b != b'"', "Unexpected quote in state string");
                out.push(b);
                continue;
            }
            out.push(match input.next().context("Incomplete state escape")? {
                b'"' => b'"',
                b'\\' => b'\\',
                b'n' => b'\n',
                b'r' => b'\r',
                b't' => b'\t',
                b'x' => {
                    let high = input
                        .next()
                        .and_then(|v| (v as char).to_digit(16))
                        .context("Invalid hexadecimal state escape")?;
                    let low = input
                        .next()
                        .and_then(|v| (v as char).to_digit(16))
                        .context("Invalid hexadecimal state escape")?;
                    (high * 16 + low) as u8
                }
                _ => bail!("Invalid state escape"),
            });
        }
        return Ok(Some(Value::String(out)));
    }
    Ok(Some(match source {
        "true" => Value::Boolean(true),
        "false" => Value::Boolean(false),
        _ => {
            if let Ok(v) = source.parse::<i64>() {
                Value::Integer(v)
            } else if let Some(v) = hexadecimal_number(source).or_else(|| decimal_number(source))
                && v.is_finite()
            {
                Value::Number(v)
            } else {
                Value::String(source.as_bytes().to_vec())
            }
        }
    }))
}

/// Stable namespace summary shared with the limited object examination command.
pub fn summary(state: &State) -> String {
    let mut out = String::new();
    if !state.is_empty() {
        out.push_str("State namespaces:\r\n");
    }
    for (ns, values) in state {
        out.push_str(&format!(
            "  {ns}: {} value{}\r\n",
            values.len(),
            if values.len() == 1 { "" } else { "s" }
        ));
    }
    out
}

/// Split a namespace and key while retaining spaces in the object name.
fn key_address(input: &str) -> Result<(&str, &str)> {
    let input = input.trim();
    let pos = input
        .rfind(char::is_whitespace)
        .context("Expected a namespace and state key")?;
    let (prefix, key) = input.split_at(pos);
    let key = key.trim();
    ensure!(
        !prefix.trim().is_empty() && !key.is_empty(),
        "Expected a namespace and state key"
    );
    Ok((prefix.trim(), key))
}

/// Parse the object separately from slash-containing namespace names.
fn object_address<'a>(
    ctx: &CommandContext<'_>,
    input: &'a str,
) -> Result<(crate::world::ObjectId, Option<&'a str>)> {
    let (object, ns) = input
        .split_once('/')
        .map_or((input, None), |(o, n)| (o, Some(n)));
    ensure!(!object.trim().is_empty(), "Expected an object");
    if let Some(ns) = ns {
        super::address(ns, None)?;
    }
    Ok((
        admin_target(&ctx.scripts.world.borrow(), ctx.player, object)?,
        ns,
    ))
}

/// Bound native output before it reaches queue and rendering budgets.
fn bounded(mut text: String, limit: usize) -> String {
    if text.len() > limit {
        let suffix = "\r\n[truncated]";
        let mut end = limit.saturating_sub(suffix.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        if suffix.len() <= limit {
            text.push_str(suffix);
        }
    }
    text
}

/// Registered after the central Wizard check; control permissions do not further restrict state.
pub fn command(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    let result = execute(ctx, input);
    match result {
        Ok((text, mutated)) => {
            let text = bounded(text, ctx.config.runtime.output_message_limit);
            if mutated {
                ctx.scripts
                    .outbox
                    .borrow_mut()
                    .push((ctx.player, crate::text::Document::Literal(text)));
                Ok(Action::Continue)
            } else {
                Ok(Action::Reply(text))
            }
        }
        Err(e) => Ok(Action::Reply(format!("Unable to access state: {e}."))),
    }
}

/// Build read-only output or publish one fully checked mutation for the server transaction.
fn execute(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<(String, bool)> {
    let args = input.args.trim();
    let Some(switch) = input.switch.as_deref() else {
        return Ok(("@state command switches:\r\n  /examine  Inspect persistent object state.\r\n  /set      Set or clear a state value.\r\n  /wipe     Clear object state or one namespace.\r\n  /copy     Copy a state value on an object.\r\n  /move     Move a state value on an object.".into(),false));
    };
    if switch == "examine" {
        let (id, ns) = object_address(ctx, if args.is_empty() { "here" } else { args })?;
        let w = ctx.scripts.world.borrow();
        let state = &w.objects[&id].state;
        let text = if let Some(ns) = ns {
            if let Some(values) = state.get(ns) {
                let mut out = format!("State namespace {ns}:\r\n");
                for (key, v) in values {
                    out.push_str(&format!("  {key} ({}): {}\r\n", v.kind(), v.display()));
                }
                out
            } else {
                format!("No state namespace named {ns}.")
            }
        } else {
            format!(
                "{}Type @state/examine <object>/<namespace> to list the values in a namespace.",
                summary(state)
            )
        };
        return Ok((text, false));
    }
    if switch == "wipe" {
        let (id, ns) = object_address(ctx, args)?;
        let count = super::change(
            &mut ctx.scripts.world.borrow_mut(),
            ctx.config,
            id,
            |state| {
                Ok(if let Some(ns) = ns {
                    state.remove(ns).map_or(0, |v| v.len())
                } else {
                    let n = state.values().map(|v| v.len()).sum();
                    state.clear();
                    n
                })
            },
        )?;
        return Ok((
            format!(
                "{count} state value{} wiped.",
                if count == 1 { "" } else { "s" }
            ),
            true,
        ));
    }
    ensure!(
        ["set", "copy", "move"].contains(&switch),
        "Invalid @state switch combination"
    );
    let (left, right) = args.split_once('=').unwrap_or((args, ""));
    let (target, key) = key_address(left)?;
    let (id, ns) = object_address(ctx, target)?;
    let ns = ns.context("Expected object/namespace")?;
    super::address(ns, Some(key))?;
    let message = if switch == "set" {
        let value = parse_value(right)?;
        let cleared = value.is_none();
        super::change(&mut ctx.scripts.world.borrow_mut(), ctx.config, id, |s| {
            super::set(s, ns, key, value)
        })?;
        if cleared {
            "State value cleared."
        } else {
            "State value set."
        }
    } else {
        let (to_ns, to_key) = key_address(right)?;
        super::address(to_ns, Some(to_key))?;
        super::change(&mut ctx.scripts.world.borrow_mut(), ctx.config, id, |s| {
            let value = s
                .get(ns)
                .and_then(|v| v.get(key))
                .cloned()
                .context("Source state value not found")?;
            if switch == "move" && (ns != to_ns || key != to_key) {
                super::set(s, ns, key, None)?;
            }
            super::set(s, to_ns, to_key, Some(value))
        })?;
        if switch == "move" {
            "State value moved."
        } else {
            "State value copied."
        }
    };
    Ok((message.into(), true))
}

/// C strtod also accepts hexadecimal mantissas; keep that syntax without locale or FFI.
fn hexadecimal_number(source: &str) -> Option<f64> {
    let (sign, text) = if let Some(s) = source.strip_prefix('-') {
        (-1.0, s)
    } else {
        (1.0, source.strip_prefix('+').unwrap_or(source))
    };
    let text = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))?;
    let (mantissa, exponent) = if let Some((m, e)) = text.split_once(['p', 'P']) {
        (m, e.parse::<i32>().ok()?)
    } else {
        (text, 0)
    };
    let mut value = 0.0;
    let mut fractional = None;
    let mut digits = 0;
    for b in mantissa.bytes() {
        if b == b'.' {
            if fractional.is_some() {
                return None;
            }
            fractional = Some(0);
            continue;
        }
        let digit = (b as char).to_digit(16)?;
        digits += 1;
        value = value * 16.0 + f64::from(digit);
        if let Some(count) = &mut fractional {
            *count += 4;
        }
    }
    if digits == 0 {
        return None;
    }
    let value = sign * value * 2f64.powi(exponent.checked_sub(fractional.unwrap_or(0))?);
    value.is_finite().then_some(value)
}

/// Match strtod range failures rather than silently accepting decimal underflow as zero.
fn decimal_number(source: &str) -> Option<f64> {
    let value = source.parse::<f64>().ok()?;
    if !value.is_finite() || (value != 0.0 && value.abs() < f64::MIN_POSITIVE) {
        return None;
    }
    let mantissa = source.split(['e', 'E']).next()?;
    if value == 0.0 && mantissa.bytes().any(|b| (b'1'..=b'9').contains(&b)) {
        return None;
    }
    Some(value)
}
