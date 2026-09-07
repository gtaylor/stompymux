//! Operational command requests, connection report layout and platform process sampling.
mod process;
use crate::commands::{Action, CommandContext, CommandInput};
use anyhow::Result;
pub use process::{Limit, ProcessSnapshot, collect, process_report};
use unicode_width::UnicodeWidthStr;

/// Cargo's package identity is also the CLI's version source.
pub const VERSION: &str = concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"));

/// Native administrative WHO requires the actual invoking player's connection.
pub fn who_command(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    Ok(if ctx.session.is_none() {
        Action::Reply("@who is only available from an active connection.".into())
    } else {
        Action::Who(input.args.clone())
    })
}

/// Public build identity, usable by queued commands too.
pub fn version_command(_: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    Ok(Action::Reply(if input.args.is_empty() {
        VERSION.into()
    } else {
        "Usage: version".into()
    }))
}

/// One immutable connection row, already ordered by the owning session map.
pub struct WhoRow {
    pub name: String,
    pub connected: u64,
    pub idle: u64,
    pub markers: String,
    pub location: i64,
    pub commands: u64,
    pub host: String,
}

/// Preserve literal Unicode while preventing client-controlled terminal characters.
fn visible(text: &str) -> String {
    text.chars()
        .flat_map(|ch| {
            if ch.is_control() {
                ch.escape_default().collect::<Vec<_>>()
            } else {
                vec![ch]
            }
        })
        .collect()
}

/// C dump_users fields and count semantics, with Unicode-aware aligned columns.
pub fn who_report(rows: &[WhoRow], record: usize, maximum: i64) -> String {
    let mut table = vec![vec![
        "Player Name".into(),
        "On For".into(),
        "Idle".into(),
        "Flags".into(),
        "Room".into(),
        "Cmds".into(),
        "Host".into(),
    ]];
    for row in rows {
        table.push(vec![
            visible(crate::text::literal_prefix(&row.name, 16)),
            crate::telnet::diagnostics::connected_time(row.connected),
            crate::telnet::diagnostics::idle_time(row.idle),
            row.markers.clone(),
            format!("#{}", row.location),
            row.commands.to_string(),
            visible(&row.host),
        ]);
    }
    let widths: Vec<_> = (0..7)
        .map(|i| table.iter().map(|r| r[i].width()).max().unwrap())
        .collect();
    let mut lines: Vec<String> = table
        .into_iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(i, s)| {
                    let pad = " ".repeat(widths[i] - s.width());
                    if i == 0 || i == 3 || i == 6 {
                        format!("{s}{pad}")
                    } else {
                        format!("{pad}{s}")
                    }
                })
                .collect::<Vec<_>>()
                .join("  ")
                .trim_end()
                .to_owned()
        })
        .collect();
    lines.push(format!(
        "{} Player{} logged in, {record} record, {} maximum.",
        rows.len(),
        if rows.len() == 1 { "" } else { "s" },
        if maximum < 0 {
            "no".into()
        } else {
            maximum.to_string()
        }
    ));
    lines.join("\n")
}
