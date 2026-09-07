//! Conservative command classification shared by stderr and suspect-channel auditing.
use crate::{
    commands::{CommandInput, ExecutionContext},
    config::{Config, LogOption},
    world::{ObjectId, World},
};
/// Classify expanded command forms without retaining raw secret-bearing input.
pub fn safe_command(c: &Config, w: &World, actor: ObjectId, line: &str) -> String {
    // Direct macro administration wins before expansion, even if a set shadows its token.
    let direct = CommandInput::parse(c, line);
    if direct.name == ".def" {
        return ".def [arguments redacted]".into();
    }
    let expanded = match w
        .macros
        .expand(actor, line.trim(), c.runtime.input_line_limit)
    {
        Ok(v) => v,
        Err(_) => return "[unclassified command: arguments redacted]".into(),
    };
    let input = CommandInput::parse(c, expanded.as_deref().unwrap_or(line));
    // Macro definitions and queued wrappers may embed arbitrarily nested commands; redact the body.
    if matches!(
        input.name.as_str(),
        "@pcreate" | "@newpassword" | "@password" | "@force" | "@wait" | ".def"
    ) {
        return format!("{} [arguments redacted]", input.name);
    }
    // If a configured alias still resolves to another alias, do not guess its eventual meaning.
    if c.aliases.commands.contains_key(&input.name) || input.name.starts_with('.') {
        return format!("{} [arguments redacted]", input.name);
    }
    super::clean(&input.line)
}
/// Capture identity and configured decorations before callbacks can change them.
pub fn message(c: &Config, w: &World, execution: ExecutionContext, line: &str) -> String {
    let actor = execution.executor;
    let identity = w.objects.get(&actor).map_or_else(
        || format!("#{}", actor.0),
        |o| {
            let flags = if c.logging.log_options.contains(&LogOption::Flags) {
                crate::find::suffix(o)
            } else {
                format!("(#{})", actor.0)
            };
            let location = if c.logging.log_options.contains(&LogOption::Location) {
                format!(" (in #{})", o.location.map_or(-1, |id| id.0))
            } else {
                String::new()
            };
            format!("{}{flags}{location}", super::clean(&o.name))
        },
    );
    format!(
        "{identity} [cause #{}; session {}] entered: '{}'",
        execution.cause.0,
        execution.session.map_or("none".into(), |s| s.to_string()),
        safe_command(c, w, actor, line)
    )
}
