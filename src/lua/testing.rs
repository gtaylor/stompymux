//! C-style suite execution in a separate VM, with live durable mutations per invocation.
use super::Scripts;
use crate::{config::Config, persistence};
use anyhow::{Result, ensure};
use mlua::{Function, Table, Value};
use std::time::Instant;

/// Directory selection and literal, case-sensitive module:test filter.
#[derive(Clone, Debug)]
pub struct Request {
    pub unit: bool,
    pub integration: bool,
    pub verbose: bool,
    pub filter: String,
}
impl Request {
    /// Parse the command's complete switch set; unrelated switches never fall through.
    pub fn parse(switch: &str, filter: &str) -> Result<Self> {
        let parts: Vec<_> = switch.split('/').collect();
        ensure!(
            parts.contains(&"test")
                && parts
                    .iter()
                    .all(|p| ["test", "unit", "integration", "verbose"].contains(p)),
            "Invalid @lua switch combination."
        );
        let both = !parts.contains(&"unit") && !parts.contains(&"integration");
        Ok(Self {
            unit: both || parts.contains(&"unit"),
            integration: both || parts.contains(&"integration"),
            verbose: parts.contains(&"verbose"),
            filter: filter.into(),
        })
    }
    fn selects(&self, path: &str) -> bool {
        if path.starts_with("unit/") {
            self.unit
        } else if path.starts_with("integration/") {
            self.integration
        } else {
            self.unit && self.integration
        }
    }
    fn matches(&self, path: &str, name: &str) -> bool {
        format!("{path}:{name}").contains(&self.filter)
    }
}

/// Restrict diagnostic fields by bytes without splitting UTF-8 sequences.
fn bounded(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.into();
    }
    let mut end = limit.saturating_sub(3);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &text[..end])
}

/// Bounded details with independent lifetime result counters.
#[derive(Default, Debug)]
pub struct Report {
    pub passed: usize,
    pub failed: usize,
    pub errored: usize,
    pub skipped: usize,
    pub failures: Vec<String>,
    pub passes: Vec<String>,
    pub failures_truncated: bool,
    pub passes_truncated: bool,
    pub elapsed_seconds: f64,
}
impl Report {
    fn failure(&mut self, path: &str, name: &str, kind: &str, message: &str) {
        if self.failures.len() == 64 {
            self.failures_truncated = true;
            return;
        }
        let text = format!("{path}:{name}: {kind}: {message}");
        self.failures.push(bounded(&text, 4096));
    }
    /// C summary, with optional passing names and explicit detail omissions.
    pub fn render(&self, verbose: bool) -> String {
        let mut lines = self.failures.clone();
        if self.failures_truncated {
            lines.push("Additional Lua test failures were omitted.".into());
        }
        if verbose {
            lines.extend(self.passes.clone());
            if self.passes_truncated {
                lines.push("Additional passing Lua tests were omitted.".into());
            }
        }
        lines.push(format!(
            "{} passed, {} failed, {} errored, {} skipped in {:.2}s.",
            self.passed, self.failed, self.errored, self.skipped, self.elapsed_seconds
        ));
        lines.join("\n")
    }
}

/// Capture the traceback function before removing debug from editable modules.
pub(super) fn install(lua: &mlua::Lua) -> mlua::Result<()> {
    let trace = lua.create_function(|lua, ()| {
        let mut lines = vec!["stack traceback:".to_string()];
        for level in 1..32 {
            let Some(line) = lua.inspect_stack(level, |frame| {
                format!(
                    "  {}:{}",
                    frame.source().short_src.as_deref().unwrap_or("?"),
                    frame.current_line().unwrap_or(0)
                )
            }) else {
                break;
            };
            lines.push(line);
        }
        Ok(lines.join("\n"))
    })?;
    let wrapper: Function = lua.load(r#"
        local traceback = ...
        local unpack, tostring = unpack, tostring
        return function(fn, args)
            local ok, result = xpcall(function() return fn(unpack(args, 1, args.n or #args)) end,
                function(e)
                    local code, detail, message
                    if type(e) == 'table' then code=e.code; detail=e.detail; message=e.message end
                    local text = tostring(message or e)
                    if type(detail)=='table' then text=text..'\n  expected: '..tostring(detail.expected)..'\n  actual: '..tostring(detail.actual) end
                    return {assertion=code=='testing.assertion', message=text..'\n'..traceback()}
                end)
            return {ok=ok, value=result}
        end
    "#).call(trace)?;
    lua.set_named_registry_value("test_invoke", wrapper)
}

/// Test locks load object modules lazily in the test VM, as in the C runner.
/// Initialization remains inside the invoking test's durable boundary.
pub(super) fn install_parents(s: &Scripts) -> mlua::Result<()> {
    let parents = s.lua.create_table()?;
    let metatable = s.lua.create_table()?;
    let sources = s.sources.clone();
    metatable.set(
        "__index",
        s.lua
            .create_function(move |lua, (cache, path): (Table, String)| {
                let key = format!("object_logic/{path}");
                let source = sources
                    .files
                    .get(&key)
                    .ok_or_else(|| mlua::Error::runtime(format!("Missing lock parent {path}")))?;
                let module: Table = lua.load(source).set_name(&key).eval()?;
                cache.raw_set(path, module.clone())?;
                Ok(module)
            })?,
    )?;
    parents.set_metatable(Some(metatable))?;
    s.lua.globals().set("_parents", parents)?;
    s.lua
        .globals()
        .set("_object_parents", s.lua.create_table()?)
}

fn valid_suite(suite: &Table) -> mlua::Result<Table> {
    suite.get("tests")
}
fn case(value: Value) -> mlua::Result<(String, Function)> {
    let Value::Table(t) = value else {
        return Err(mlua::Error::runtime(
            "test entries need a name and run function",
        ));
    };
    Ok((t.get("name")?, t.get("run")?))
}

/// Check declarations without running hooks or test bodies in the checking VM.
pub fn check(s: &Scripts) -> Result<()> {
    for (path, source) in s
        .sources
        .files
        .iter()
        .filter(|(p, _)| p.starts_with("tests/"))
    {
        s.budget.reset();
        let result = (|| -> mlua::Result<()> {
            let suite: Table = s.lua.load(source).set_name(path).eval()?;
            let tests = valid_suite(&suite)?;
            for entry in tests.sequence_values::<Value>() {
                case(entry?)?;
            }
            for name in ["before_all", "before_each", "after_each", "after_all"] {
                let value: Value = suite.get(name)?;
                if !matches!(value, Value::Nil | Value::Function(_)) {
                    return Err(mlua::Error::runtime("hook must be a function"));
                }
            }
            Ok(())
        })();
        result.map_err(|e| anyhow::anyhow!("checking {path}: {e}"))?;
    }
    Ok(())
}

struct Outcome {
    ok: bool,
    assertion: bool,
    message: String,
    value: Value,
}
impl Outcome {
    fn error(message: impl ToString) -> Self {
        Self {
            ok: false,
            assertion: false,
            message: message.to_string(),
            value: Value::Nil,
        }
    }
}

/// One live invocation, including persistence after Lua errors. Only invalid/unsaved changes roll back.
async fn invoke(
    s: &Scripts,
    c: &Config,
    function: Function,
    args: Table,
    deliver: &mut impl FnMut(),
) -> Outcome {
    let before = s.world.borrow().clone();
    let pending = s.outbox.borrow().clone();
    s.budget.reset();
    let result = super::transactions::live_test(&s.lua, || {
        let wrapper: Function = s.lua.named_registry_value("test_invoke")?;
        // The wrapper catches script errors inside the transaction so they do not undo mutations.
        wrapper.call::<Table>((function, args))
    });
    let mut outcome = match result {
        Ok(t) => {
            let ok = t.get::<bool>("ok").unwrap_or(false);
            let value = t.get::<Value>("value").unwrap_or(Value::Nil);
            let mut outcome = Outcome {
                ok,
                value: value.clone(),
                assertion: false,
                message: String::new(),
            };
            if !ok {
                if let Value::Table(e) = value {
                    outcome.assertion = e.get("assertion").unwrap_or(false);
                    outcome.message = e.get("message").unwrap_or_else(|_| "Lua error".into());
                } else {
                    outcome.message = "Lua error while collecting failure details".into();
                }
            }
            outcome
        }
        Err(e) => Outcome::error(e),
    };
    let after = s.world.borrow().clone();
    let saved = async {
        after.validate(c)?;
        if serde_json::to_vec(&before)? != serde_json::to_vec(&after)? {
            persistence::persist(c.database(), after, c.database.busy_timeout_ms).await?;
        }
        Ok::<_, anyhow::Error>(())
    }
    .await;
    if let Err(e) = saved {
        *s.world.borrow_mut() = before;
        *s.outbox.borrow_mut() = pending;
        outcome = Outcome::error(format!("validation/persistence: {e:#}"));
    }
    deliver();
    outcome
}

async fn hook(
    s: &Scripts,
    c: &Config,
    suite: &Table,
    ctx: &Table,
    name: &str,
    deliver: &mut impl FnMut(),
) -> Outcome {
    match suite.get::<Value>(name) {
        Ok(Value::Nil) => Outcome {
            ok: true,
            assertion: false,
            message: String::new(),
            value: Value::Nil,
        },
        Ok(Value::Function(f)) => match s.lua.create_sequence_from([ctx.clone()]) {
            Ok(args) => invoke(s, c, f, args, deliver).await,
            Err(e) => Outcome::error(e),
        },
        _ => Outcome::error("hook must be a function"),
    }
}

/// Run selected suites; the caller delivers each committed invocation's staged messages.
pub async fn run(
    s: &Scripts,
    c: &Config,
    request: &Request,
    mut deliver: impl FnMut(),
) -> Result<Report> {
    let started = Instant::now();
    let mut report = Report::default();
    for (full, source) in s
        .sources
        .files
        .iter()
        .filter(|(p, _)| p.starts_with("tests/"))
    {
        let path = full.strip_prefix("tests/").unwrap();
        if !request.selects(path) {
            continue;
        }
        let function = match s.lua.load(source).set_name(full).into_function() {
            Ok(f) => f,
            Err(e) => {
                report.errored += 1;
                report.failure(path, "<load>", "load", &e.to_string());
                continue;
            }
        };
        let loaded = invoke(
            s,
            c,
            function,
            s.lua
                .create_table()
                .map_err(|e| anyhow::anyhow!(e.to_string()))?,
            &mut deliver,
        )
        .await;
        if !loaded.ok {
            report.errored += 1;
            report.failure(path, "<load>", "load", &loaded.message);
            continue;
        }
        let Value::Table(suite) = loaded.value else {
            report.errored += 1;
            report.failure(
                path,
                "<suite>",
                "definition",
                "test module must return a testing.suite table",
            );
            continue;
        };
        let tests = match valid_suite(&suite) {
            Ok(t) => t,
            Err(e) => {
                report.errored += 1;
                report.failure(path, "<suite>", "definition", &e.to_string());
                continue;
            }
        };
        let cases: Vec<_> = (1..=tests.raw_len())
            .map(|i| tests.raw_get::<Value>(i).and_then(case))
            .collect();
        if !cases
            .iter()
            .any(|v| v.as_ref().is_ok_and(|(n, _)| request.matches(path, n)))
        {
            report.skipped += cases.iter().filter(|v| v.is_ok()).count();
            continue;
        }
        let ctx = s
            .lua
            .create_table()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let before = hook(s, c, &suite, &ctx, "before_all", &mut deliver).await;
        if !before.ok {
            report.failure(path, "<before_all>", "before_all", &before.message);
        }
        for entry in cases {
            let (name, function) = match entry {
                Ok(v) => v,
                Err(e) => {
                    report.errored += 1;
                    report.failure(path, "<invalid test>", "definition", &e.to_string());
                    continue;
                }
            };
            if !request.matches(path, &name) {
                report.skipped += 1;
                continue;
            }
            if !before.ok {
                report.errored += 1;
                continue;
            }
            let pre = hook(s, c, &suite, &ctx, "before_each", &mut deliver).await;
            if !pre.ok {
                report.failure(path, &name, "before_each", &pre.message);
            }
            let outcome = if pre.ok {
                match suite.get::<Table>("expect") {
                    Ok(expect) => {
                        invoke(
                            s,
                            c,
                            function,
                            s.lua
                                .create_sequence_from([ctx.clone(), expect])
                                .map_err(|e| anyhow::anyhow!(e.to_string()))?,
                            &mut deliver,
                        )
                        .await
                    }
                    Err(e) => Outcome::error(e),
                }
            } else {
                Outcome::error("before_each failed")
            };
            if pre.ok && !outcome.ok {
                report.failure(
                    path,
                    &name,
                    if outcome.assertion {
                        "assertion"
                    } else {
                        "runtime"
                    },
                    &outcome.message,
                );
            }
            let post = hook(s, c, &suite, &ctx, "after_each", &mut deliver).await;
            if !post.ok {
                report.failure(path, &name, "after_each", &post.message);
            }
            if !pre.ok || !post.ok || (!outcome.ok && !outcome.assertion) {
                report.errored += 1;
            } else if outcome.ok {
                report.passed += 1;
                if report.passes.len() < 64 {
                    report.passes.push(bounded(&format!("{path}:{name}"), 768));
                } else {
                    report.passes_truncated = true;
                }
            } else {
                report.failed += 1;
            }
            tokio::task::yield_now().await;
        }
        let post = hook(s, c, &suite, &ctx, "after_all", &mut deliver).await;
        if !post.ok {
            report.errored += 1;
            report.failure(path, "<after_all>", "after_all", &post.message);
        }
    }
    report.elapsed_seconds = started.elapsed().as_secs_f64();
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selectors_and_literal_filters_match_c() {
        for switches in [
            "test",
            "test/unit/integration",
            "verbose/integration/test/unit",
        ] {
            let r = Request::parse(switches, "Probe*").unwrap();
            assert!(
                r.selects("unit/a.lua")
                    && r.selects("integration/b.lua")
                    && r.selects("other/c.lua")
            );
            assert!(r.matches("unit/a.lua", "Probe*"));
            assert!(!r.matches("unit/a.lua", "ProbeAnything"));
            assert!(!r.matches("unit/a.lua", "probe*"));
        }
        let r = Request::parse("test/unit/verbose", "").unwrap();
        assert!(r.unit && !r.integration && r.verbose);
        assert!(!r.selects("other/a.lua"));
        for bad in [
            "unit",
            "test/",
            "test/check",
            "test/unknown",
            "test//verbose",
        ] {
            assert!(Request::parse(bad, "").is_err());
        }
    }

    #[test]
    fn diagnostic_fields_are_utf8_bounded() {
        let text = "界".repeat(2000);
        let mut report = Report::default();
        report.failure("path", "test", "runtime", &text);
        assert!(report.failures[0].len() <= 4096);
        assert!(report.failures[0].ends_with("..."));
    }
}
