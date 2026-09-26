//! Command execution and staged text collection shared by direct-runtime tests.

use stompymux_rs::{CommandAction, CommandReport, Config, ObjectId, Scripts, commands};

/// Run one command and collect all direct and transaction-staged text.
pub fn run_text(
    scripts: &Scripts,
    config: &Config,
    player: ObjectId,
    descriptor: u64,
    line: &str,
) -> String {
    run_filtered(scripts, config, player, descriptor, line, None)
}

/// Collect only the actor's output when a command also publishes to other occupants.
pub fn run_text_for_player(
    scripts: &Scripts,
    config: &Config,
    player: ObjectId,
    descriptor: u64,
    line: &str,
) -> String {
    run_filtered(scripts, config, player, descriptor, line, Some(player))
}

/// Preserve direct command reports while optionally selecting one staged recipient.
fn run_filtered(
    scripts: &Scripts,
    config: &Config,
    player: ObjectId,
    descriptor: u64,
    line: &str,
    recipient: Option<ObjectId>,
) -> String {
    let action = commands::run(scripts, config, player, descriptor, line).unwrap();
    let mut messages = scripts
        .drain_outbox()
        .into_iter()
        .filter(|(id, _)| recipient.is_none_or(|recipient| *id == recipient))
        .map(|(_, message)| message.source().to_owned())
        .collect::<Vec<_>>();
    match action {
        CommandAction::CommitReply(text)
        | CommandAction::Report(
            CommandReport::Reply(text)
            | CommandReport::Inspection(text)
            | CommandReport::Literal(text)
            | CommandReport::Styled(text),
        ) => messages.push(text),
        _ => {}
    }
    messages.join("\n")
}
