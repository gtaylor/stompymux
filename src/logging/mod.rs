//! Categorized diagnostics, bounded asynchronous output and explicit game log writes.
pub mod audit;
mod worker;
use crate::{
    commands::{Action, CommandContext, CommandInput},
    config::{Config, LogOption},
};
use anyhow::Result;
pub use worker::{FileRequest, Logger};

/// C event categories that can be switched on or off under `logging.topics`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Accounting,
    AllCommands,
    SuspectCommands,
    BadCommands,
    Bugs,
    Checkpoints,
    ConfigChanges,
    Create,
    Logins,
    Network,
    Problems,
    Security,
    Shouts,
    Startup,
    Wizard,
}
/// Declaration order and minimum C abbreviations.
pub const CATEGORIES: &[(Category, &str, usize)] = &[
    (Category::Accounting, "accounting", 2),
    (Category::AllCommands, "all_commands", 2),
    (Category::SuspectCommands, "suspect_commands", 2),
    (Category::BadCommands, "bad_commands", 2),
    (Category::Bugs, "bugs", 3),
    (Category::Checkpoints, "checkpoints", 2),
    (Category::ConfigChanges, "config_changes", 2),
    (Category::Create, "create", 2),
    (Category::Logins, "logins", 1),
    (Category::Network, "network", 1),
    (Category::Problems, "problems", 1),
    (Category::Security, "security", 2),
    (Category::Shouts, "shouts", 2),
    (Category::Startup, "startup", 2),
    (Category::Wizard, "wizard", 1),
];
impl Category {
    /// Sample the effective topic switches before enqueueing a record.
    pub fn enabled(self, c: &Config) -> bool {
        let t = &c.logging.topics;
        match self {
            Self::Accounting => t.accounting,
            Self::AllCommands => t.all_commands,
            Self::SuspectCommands => t.suspect_commands,
            Self::BadCommands => t.bad_commands,
            Self::Bugs => t.bugs,
            Self::Checkpoints => t.checkpoints,
            Self::ConfigChanges => t.config_changes,
            Self::Create => t.create,
            Self::Logins => t.logins,
            Self::Network => t.network,
            Self::Problems => t.problems,
            Self::Security => t.security,
            Self::Shouts => t.shouts,
            Self::Startup => t.startup,
            Self::Wizard => t.wizard,
        }
    }
}
/// A complete immutable record; the worker never reads world or live configuration.
#[derive(Clone, Debug)]
pub struct Record {
    pub text: String,
}
impl Record {
    /// C stderr header and decorations, bounded and stripped of executable terminal text.
    pub fn new(c: &Config, primary: &str, secondary: &str, message: &str) -> Self {
        let time = if c.logging.log_options.contains(&LogOption::Timestamp) {
            chrono::Local::now().format("%Y%m%d.%H%M%S ").to_string()
        } else {
            String::new()
        };
        let name = clean(&c.server.mud_name);
        let name = &name[..name.floor_char_boundary(name.len().min(128))];
        let primary = clean(primary);
        let secondary = clean(secondary);
        let header = if secondary.is_empty() {
            format!("{time}{name} {primary:<9}: ")
        } else {
            format!("{time}{name} {primary:>3}/{secondary:<5}: ")
        };
        let text = clean(message);
        const RECORD_LIMIT: usize = 8192;
        let available = RECORD_LIMIT.saturating_sub(header.len() + 1);
        let text = if text.len() > available {
            let n = available.saturating_sub(14);
            format!("{}...[truncated]", &text[..text.floor_char_boundary(n)])
        } else {
            text
        };
        Self {
            text: format!("{header}{text}\n"),
        }
    }
}
/// Strip styles and escape control bytes so one diagnostic remains one physical line.
pub fn clean(text: &str) -> String {
    let plain: String = crate::text::Document::Styled(text.into())
        .spans(&Default::default(), &Default::default())
        .iter()
        .map(|s| s.text.as_str())
        .collect();
    plain
        .chars()
        .flat_map(|c| {
            if c.is_control() {
                c.escape_default().collect::<Vec<_>>()
            } else {
                vec![c]
            }
        })
        .collect()
}
impl Config {
    /// Record if any category is enabled; producer threads never wait on diagnostic output.
    pub fn log(
        &self,
        categories: &[Category],
        primary: &str,
        secondary: &str,
        message: impl AsRef<str>,
    ) {
        if categories.iter().any(|v| v.enabled(self)) {
            self.logger.record(
                self,
                Record::new(self, primary, secondary, message.as_ref()),
            );
        }
    }
}
/// Wizard entry point delegates file I/O to the world owner, outside database transactions.
pub fn command(_: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    let (filename, message) = input.args.split_once('=').unwrap_or((&input.args, ""));
    if message.is_empty() {
        return Ok(Action::Report(crate::commands::Report::Reply(
            "Nothing to log!".into(),
        )));
    }
    if filename.is_empty() {
        return Ok(Action::Report(crate::commands::Report::Reply(
            "Invalid logfile.".into(),
        )));
    }
    Ok(match FileRequest::new(filename, message) {
        Ok(request) => Action::Server(crate::commands::ServerRequest::Log(request)),
        Err(_) => Action::Report(crate::commands::Report::Reply("Request failed.".into())),
    })
}
/// Effective C topic/decorator report, with no claims about unavailable producers.
pub fn report(c: &Config) -> String {
    let mut lines = vec!["Events Logged:".into()];
    lines.extend(CATEGORIES.iter().map(|(v, name, _)| {
        format!(
            "{name}: {}",
            if v.enabled(c) { "enabled" } else { "disabled" }
        )
    }));
    lines.push("Information Logged:".into());
    for (name, option) in [
        ("flags", LogOption::Flags),
        ("location", LogOption::Location),
        ("timestamp", LogOption::Timestamp),
    ] {
        lines.push(format!(
            "{name}: {}",
            if c.logging.log_options.contains(&option) {
                "yes"
            } else {
                "no"
            }
        ));
    }
    lines.join("\n")
}

/// Emergency process diagnostics remain visible even before configuration or after worker failure.
pub fn fatal(message: &str) {
    let _ = std::io::Write::write_all(
        &mut std::io::stderr(),
        format!("FATAL: {}\n", clean(message)).as_bytes(),
    );
}
