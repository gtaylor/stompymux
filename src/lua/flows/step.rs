//! C flow outcomes, bounded scratch encoding and immediate callback transitions.
use super::*;

impl Engine {
    /// Run a submitted line and bounded immediate transitions under the caller's budget.
    pub(super) fn drive(
        &self,
        lua: &Lua,
        session: u64,
        mut input: Option<&str>,
    ) -> mlua::Result<()> {
        {
            let mut control = self.control.borrow_mut();
            if control.depth >= TRANSITIONS {
                return Err(error(lua, "runtime", "Flow nested-start limit exceeded"));
            }
            control.depth += 1;
        }
        let result = (|| {
            for transition in 0..=TRANSITIONS {
                let mut active = self
                    .effects
                    .flow(session)
                    .ok_or_else(|| error(lua, "connection.invalid", "flow is no longer active"))?;
                let handler = self.handler(lua, &active.module, &active.step)?;
                let ctx = lua.create_table()?;
                ctx.set("scope", "flow")?;
                ctx.set("enactor", active.identity.player.0)?;
                ctx.set("cause", active.identity.player.0)?;
                ctx.set("descriptor", session)?;
                ctx.set("input", input)?;
                let scratch = lua.create_table()?;
                for (k, v) in &active.scratch {
                    scratch.raw_set(lua.create_string(k)?, lua.create_string(v)?)?;
                }
                ctx.set("flow", scratch)?;
                let result: Value = with_root(lua, &active.module, || {
                    transactions::with_cause(lua, active.identity.player, || {
                        transactions::with_descriptor(lua, Some(session), || {
                            handler.call(ctx.clone())
                        })
                    })
                })?;
                active.scratch = encode(lua, ctx.raw_get("flow")?)?;
                let Value::Table(result) = result else {
                    return Err(error(
                        lua,
                        "runtime",
                        "Flow step must return an outcome table",
                    ));
                };
                let action = match result.raw_get::<Value>("action")? {
                    Value::Nil => "repeat".to_owned(),
                    Value::String(s) => s.to_str()?.to_owned(),
                    _ => return Err(error(lua, "runtime", "Invalid flow action")),
                };
                let prompt = result.raw_get::<Value>("prompt")?;
                let message = if prompt.is_nil() {
                    result.raw_get::<Value>("message")?
                } else {
                    prompt
                };
                let message = match message {
                    Value::Nil => None,
                    Value::String(s) => {
                        let text = s.to_str()?.to_owned();
                        validate_string(&text, VALUE_BYTES, false)?;
                        Some(text)
                    }
                    _ => {
                        return Err(error(
                            lua,
                            "runtime",
                            "Flow prompt/message must be a string",
                        ));
                    }
                };
                match action.as_str() {
                    "repeat" => {
                        if let Some(message) = message {
                            active.prompt = message;
                        }
                        self.output(session, &active.prompt, true)?;
                        self.effects.insert_flow(session, active);
                        return Ok(());
                    }
                    "done" | "cancel" => {
                        if let Some(message) = message {
                            self.output(session, &message, false)?;
                        }
                        self.effects.remove_flow(session);
                        return Ok(());
                    }
                    "goto" => {
                        if transition == TRANSITIONS {
                            return Err(error(
                                lua,
                                "runtime",
                                "Flow exceeded 32 immediate GOTO transitions",
                            ));
                        }
                        let next: mlua::LuaString = result.raw_get("step")?;
                        let next = next.to_str()?.to_owned();
                        validate_string(&next, STEP_BYTES, true)?;
                        self.handler(lua, &active.module, &next)?;
                        if let Some(message) = message {
                            self.output(session, &message, false)?;
                        }
                        active.step = next;
                        self.effects.insert_flow(session, active);
                        input = None;
                    }
                    _ => {
                        return Err(error(
                            lua,
                            "runtime",
                            format!("Unknown flow action {action:?}"),
                        ));
                    }
                }
            }
            unreachable!()
        })();
        self.control.borrow_mut().depth -= 1;
        result
    }
}

/// Check C byte bounds without silently truncating or dropping invalid data.
pub(super) fn validate_string(text: &str, limit: usize, nonempty: bool) -> mlua::Result<()> {
    if text.len() > limit || text.contains('\0') || (nonempty && text.is_empty()) {
        return Err(mlua::Error::runtime(format!(
            "Flow string must be {}at most {limit} bytes without NUL",
            if nonempty { "nonempty and " } else { "" }
        )));
    }
    Ok(())
}

/// Store binary scratch independently of Lua values; stringify finite numbers.
fn encode(lua: &Lua, scratch: Table) -> mlua::Result<BTreeMap<Vec<u8>, Vec<u8>>> {
    let mut fields = BTreeMap::new();
    for pair in scratch.pairs::<Value, Value>() {
        let (key, value) = pair?;
        let Value::String(key) = key else {
            return Err(error(lua, "runtime", "Flow scratch keys must be strings"));
        };
        let value = match value {
            Value::String(s) => s,
            Value::Integer(n) => lua.coerce_string(Value::Integer(n))?.unwrap(),
            Value::Number(n) if n.is_finite() => lua.coerce_string(Value::Number(n))?.unwrap(),
            _ => {
                return Err(error(
                    lua,
                    "runtime",
                    "Flow scratch values must be strings or finite numbers",
                ));
            }
        };
        let key = key.as_bytes().to_vec();
        let value = value.as_bytes().to_vec();
        if fields.len() >= FIELDS
            || key.len() > KEY_BYTES
            || value.len() > VALUE_BYTES
            || key.contains(&0)
            || value.contains(&0)
        {
            return Err(error(
                lua,
                "runtime",
                "Flow scratch limit exceeded or embedded NUL",
            ));
        }
        fields.insert(key, value);
    }
    Ok(fields)
}
