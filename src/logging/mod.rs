//! Server diagnostics through `tracing`: audit targets, filter parsing, the stderr subscriber
//! with its live filter reload, commit-staged wizard audits, and the `@log` file-append command.
//!
//! Library code only emits events; the server binary installs the subscriber with [`init`].
//! Routine diagnostics use their module path as the target. Audit trails that operators switch
//! on and off use the fixed targets in [`targets`], so a filter such as
//! `info,audit::commands=info` enables command auditing without touching anything else.
pub mod audit;
mod worker;
use crate::{
    commands::{Action, CommandContext, CommandInput},
    config::{Config, LogFormat},
};
use anyhow::{Context, Result};
use std::{
    io::IsTerminal,
    sync::{Arc, Mutex, OnceLock},
};
use tracing_subscriber::{
    EnvFilter, Registry, fmt::MakeWriter, layer::SubscriberExt, reload, util::SubscriberInitExt,
};
pub use worker::{FileRequest, Logger};

/// Fixed targets for audit trails, independent of which module emits them.
pub mod targets {
    /// Every command a player enters (redacted where it may carry secrets).
    pub const COMMANDS: &str = "audit::commands";
    /// Commands entered by SUSPECT players; also enabled by an `audit::commands` directive.
    pub const SUSPECT_COMMANDS: &str = "audit::commands::suspect";
    /// Commands the server did not recognize.
    pub const BAD_COMMANDS: &str = "audit::bad_commands";
    /// Per-session traffic and duration totals written at disconnect.
    pub const ACCOUNTING: &str = "audit::accounting";
    /// Connects, disconnects, failed and throttled logins.
    pub const LOGINS: &str = "audit::logins";
    /// Character registration and wizard account administration.
    pub const ACCOUNTS: &str = "audit::accounts";
    /// Runtime configuration edits.
    pub const CONFIG: &str = "audit::config";
    /// Wizard shouts.
    pub const SHOUTS: &str = "audit::shouts";
    /// Operator changes to BattleTech settings.
    pub const WIZARD: &str = "audit::wizard";
    /// Pilot experience awards for skills other than gunnery and piloting, and the
    /// battle-value formula's noisy gain details.
    pub const BTECH_EXPERIENCE: &str = "btech::experience";
    /// Accepted gunnery experience awards.
    pub const BTECH_GUNNERY_EXPERIENCE: &str = "btech::experience::gunnery";
    /// Accepted piloting experience awards.
    pub const BTECH_PILOTING_EXPERIENCE: &str = "btech::experience::piloting";
    /// Piloting skill roll inputs: base skill, modifiers, damage and target number.
    pub const BTECH_PILOTING_ROLLS: &str = "btech::piloting::rolls";
    /// Self-destruct detonations.
    pub const BTECH_SELF_DESTRUCT: &str = "btech::self_destruct";
    /// Parts stock additions and removals.
    pub const BTECH_ECONOMY: &str = "btech::economy";
    /// Radio frequency settings that match an opposing team's channel.
    pub const BTECH_RADIO_FREQUENCIES: &str = "btech::radio::frequencies";
    /// Radio transmissions on frequency zero over an in-character battlefield, with their text.
    pub const BTECH_RADIO_ZERO_FREQUENCY: &str = "btech::radio::zero_frequency";
    /// Map files rejected when loading, with the reason.
    pub const BTECH_MAP_LOAD: &str = "btech::map::load";
}

/// Filter used when neither `logging.filter` nor `RUST_LOG` says otherwise: everything at
/// `info` and above except the high-volume command and accounting audits.
pub const DEFAULT_FILTER: &str =
    "info,audit::commands=off,audit::bad_commands=off,audit::accounting=off";

/// Parse filter directives in the `RUST_LOG` syntax, rejecting any malformed directive.
pub fn parse_filter(directives: &str) -> Result<EnvFilter> {
    EnvFilter::builder()
        .parse(directives)
        .with_context(|| format!("invalid log filter {directives:?}"))
}

/// Handle used to swap the active filter after `@admin log_filter` edits.
static RELOAD: OnceLock<reload::Handle<EnvFilter, Registry>> = OnceLock::new();

/// Install the process-wide stderr subscriber. `RUST_LOG`, when set, overrides
/// `logging.filter` until the next `@admin log_filter` edit. Keep the returned guard alive for
/// the life of the process; dropping it flushes buffered output.
pub fn init(c: &Config) -> Result<tracing_appender::non_blocking::WorkerGuard> {
    let filter = match std::env::var("RUST_LOG") {
        Ok(directives) if !directives.is_empty() => parse_filter(&directives)?,
        _ => parse_filter(&c.logging.filter)?,
    };
    let (filter, handle) = reload::Layer::new(filter);
    let ansi = std::io::stderr().is_terminal();
    let (writer, guard) = tracing_appender::non_blocking::NonBlockingBuilder::default()
        .lossy(true)
        .finish(std::io::stderr());
    let fmt = tracing_subscriber::fmt::layer()
        .with_writer(writer)
        .with_ansi(ansi);
    let registry = tracing_subscriber::registry().with(filter);
    match c.logging.format {
        LogFormat::Full => registry.with(fmt).try_init(),
        LogFormat::Compact => registry.with(fmt.compact()).try_init(),
        LogFormat::Json => registry.with(fmt.json()).try_init(),
    }
    .context("installing the log subscriber")?;
    let _ = RELOAD.set(handle);
    Ok(guard)
}

/// Make `logging.filter` the active filter; a no-op when [`init`] has not run.
pub fn apply_filter(c: &Config) -> Result<()> {
    let Some(handle) = RELOAD.get() else {
        return Ok(());
    };
    handle
        .reload(parse_filter(&c.logging.filter)?)
        .context("reloading the log filter")
}

/// The filter currently deciding which events are written.
pub fn active_filter(c: &Config) -> String {
    RELOAD
        .get()
        .and_then(|handle| handle.with_current(|filter| filter.to_string()).ok())
        .unwrap_or_else(|| c.logging.filter.clone())
}

/// A wizard audit held until its transaction commits, so rolled-back changes leave no trace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditRecord {
    pub message: String,
}

impl AuditRecord {
    /// Emit the committed audit to [`targets::WIZARD`].
    pub fn emit(&self) {
        tracing::info!(target: targets::WIZARD, "{}", self.message);
    }
}

/// BattleTech diagnostic topics, each written to its own fixed target in [`targets`] at its
/// own level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceTopic {
    Experience,
    GunneryExperience,
    PilotingExperience,
    PilotingRolls,
    SelfDestruct,
    Economy,
    RadioFrequencies,
    RadioZeroFrequency,
    MapLoad,
}

/// A diagnostic held until its transaction commits, so rolled-back actions leave no trace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceRecord {
    pub topic: TraceTopic,
    pub message: String,
}

impl TraceRecord {
    /// Emit the committed record to its topic's target, as one inert line.
    pub fn emit(&self) {
        let message = clean(&self.message);
        match self.topic {
            TraceTopic::Experience => {
                tracing::debug!(target: targets::BTECH_EXPERIENCE, "{message}")
            }
            TraceTopic::GunneryExperience => {
                tracing::debug!(target: targets::BTECH_GUNNERY_EXPERIENCE, "{message}")
            }
            TraceTopic::PilotingExperience => {
                tracing::debug!(target: targets::BTECH_PILOTING_EXPERIENCE, "{message}")
            }
            TraceTopic::PilotingRolls => {
                tracing::debug!(target: targets::BTECH_PILOTING_ROLLS, "{message}")
            }
            TraceTopic::SelfDestruct => {
                tracing::debug!(target: targets::BTECH_SELF_DESTRUCT, "{message}")
            }
            TraceTopic::Economy => tracing::debug!(target: targets::BTECH_ECONOMY, "{message}"),
            TraceTopic::RadioFrequencies => {
                tracing::debug!(target: targets::BTECH_RADIO_FREQUENCIES, "{message}")
            }
            TraceTopic::RadioZeroFrequency => {
                tracing::info!(target: targets::BTECH_RADIO_ZERO_FREQUENCY, "{message}")
            }
            TraceTopic::MapLoad => tracing::error!(target: targets::BTECH_MAP_LOAD, "{message}"),
        }
    }
}

/// Strip styles and escape control bytes so player text stays one inert line.
pub(crate) fn clean(text: &str) -> String {
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

/// Active filter and format, for `@list logging`.
pub fn report(c: &Config) -> String {
    format!(
        "Log filter: {}\nLog format: {:?}",
        active_filter(c),
        c.logging.format
    )
}

/// In-memory event sink for asserting on emitted diagnostics in tests.
#[derive(Clone, Debug, Default)]
pub struct Capture(Arc<Mutex<Vec<u8>>>);

impl Capture {
    /// Capture this thread's events that pass `filter` until the returned guard drops.
    pub fn install(filter: &str) -> (Self, tracing::subscriber::DefaultGuard) {
        let capture = Self::default();
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(parse_filter(filter).expect("valid capture filter"))
            .with_writer(capture.clone())
            .with_ansi(false)
            .without_time()
            .finish();
        (capture, tracing::subscriber::set_default(subscriber))
    }

    /// Everything captured so far, one formatted event per line.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }

    /// Captured lines containing `needle`.
    pub fn lines_containing(&self, needle: &str) -> Vec<String> {
        self.text()
            .lines()
            .filter(|line| line.contains(needle))
            .map(str::to_owned)
            .collect()
    }
}

/// Appends formatted events to the shared capture buffer.
pub struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for CaptureWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Capture {
    type Writer = CaptureWriter;

    fn make_writer(&'a self) -> Self::Writer {
        CaptureWriter(self.0.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each trace topic keeps its level: the default filter writes zero-frequency radio and
    /// map-load errors but hides debug topics, and player text stays one inert line.
    #[test]
    fn trace_topics_use_their_levels_and_clean_player_text() {
        let (capture, _guard) = Capture::install(DEFAULT_FILTER);
        for (topic, message) in [
            (TraceTopic::Economy, "stock"),
            (
                TraceTopic::RadioZeroFrequency,
                "said \"hi\u{1b}[31m\"\rthere",
            ),
            (TraceTopic::MapLoad, "bad map"),
        ] {
            TraceRecord {
                topic,
                message: message.into(),
            }
            .emit();
        }
        assert!(capture.lines_containing(targets::BTECH_ECONOMY).is_empty());
        let radio = capture.lines_containing(targets::BTECH_RADIO_ZERO_FREQUENCY);
        assert_eq!(radio.len(), 1, "{}", capture.text());
        assert!(radio[0].contains("INFO"), "{}", radio[0]);
        assert!(
            !radio[0].contains('\u{1b}') && !radio[0].contains('\r'),
            "{}",
            radio[0]
        );
        let map = capture.lines_containing(targets::BTECH_MAP_LOAD);
        assert_eq!(map.len(), 1, "{}", capture.text());
        assert!(
            map[0].contains("ERROR") && map[0].contains("bad map"),
            "{}",
            map[0]
        );
    }

    /// Filters parse with `RUST_LOG` syntax and reject malformed directives.
    #[test]
    fn filters_parse_and_reject_garbage() {
        parse_filter(DEFAULT_FILTER).unwrap();
        parse_filter("warn,stompymux_rs::server=debug").unwrap();
        assert!(parse_filter("info,audit::commands=loud").is_err());
    }

    /// The default filter keeps routine events but silences the high-volume audits.
    #[test]
    fn default_filter_silences_command_audits() {
        let (capture, _guard) = Capture::install(DEFAULT_FILTER);
        tracing::info!("routine");
        tracing::info!(target: targets::COMMANDS, "entered");
        tracing::info!(target: targets::SUSPECT_COMMANDS, "suspect");
        tracing::info!(target: targets::LOGINS, "connected");
        let text = capture.text();
        assert!(text.contains("routine") && text.contains("connected"));
        assert!(!text.contains("entered") && !text.contains("suspect"));
    }

    /// Enabling command audits also enables the suspect-command trail beneath it.
    #[test]
    fn command_audit_directive_covers_suspect_commands() {
        let (capture, _guard) = Capture::install("warn,audit::commands=info");
        tracing::info!("routine");
        tracing::info!(target: targets::SUSPECT_COMMANDS, "suspect");
        let text = capture.text();
        assert!(!text.contains("routine") && text.contains("suspect"));
    }
}
