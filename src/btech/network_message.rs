//! Command-network communication, independent of radio channels and contact visibility.
use super::Notice;
use super::network_unit::unit as network_unit;
use crate::CommandNetwork;
use crate::{ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};

/// Prepare network notices without changing state, consuming dice, or publishing output.
/// Offline, jammed, and unconscious peers retain membership but do not receive a message.
fn message_for(
    world: &World,
    id: ObjectId,
    pilot: ObjectId,
    text: &str,
    kind: CommandNetwork,
) -> Result<Vec<Notice>> {
    super::command_network::ready_for(world, id, pilot, kind)?;
    let name = kind.name();
    let members = super::command_network::members_for(world, id, kind)?;
    ensure!(
        !members.is_empty(),
        "There are no other units in your {name} network!"
    );
    let text = text.trim_start_matches(|c: char| c.is_ascii_whitespace());
    ensure!(
        !text.is_empty(),
        "What do you want to send on the {name} Network?"
    );
    let sender = network_unit(world, id)?;
    let identity = format!(
        "{} [{}]",
        sender.name(),
        sender
            .battlefield_id()
            .context("Unit has no battlefield ID")?
    );
    let line = |speaker: &str| {
        format!(
            "[bold]{}[reset]",
            crate::text::escape(&format!("{name}/{speaker}: {text}"))
        )
    };
    let mut notices = Vec::new();
    for peer in super::command_network::active_members(world, id, kind, true)? {
        if peer == id {
            continue;
        }
        notices.push(Notice {
            unit: peer,
            text: line(&identity),
        });
    }
    notices.push(Notice {
        unit: id,
        text: line("You"),
    });
    Ok(notices)
}

/// Publish every recipient under one host checkpoint, including Lua callback rollback.
fn send_for(
    scripts: &Scripts,
    id: ObjectId,
    pilot: ObjectId,
    text: &str,
    kind: CommandNetwork,
) -> Result<Vec<Notice>> {
    scripts.atomic(|_| {
        let notices = message_for(&scripts.world(), id, pilot, text, kind)?;
        for notice in &notices {
            super::notify_unit(scripts, notice.clone())?;
        }
        Ok(notices)
    })
}

/// Native network message sender uses the invoking player's cockpit.
fn command_for(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
    kind: CommandNetwork,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let id = ctx
            .scripts
            .world()
            .objects
            .get(&ctx.player)
            .and_then(|p| p.location)
            .context("Enter a unit first")?;
        send_for(ctx.scripts, id, ctx.player, &input.args, kind)
    })();
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

/// Prepare C3i recipient notices without publishing output.
pub fn message(world: &World, id: ObjectId, pilot: ObjectId, text: &str) -> Result<Vec<Notice>> {
    message_for(world, id, pilot, text, CommandNetwork::C3i)
}
/// Prepare classic C3 notices using available master capacity.
pub fn c3_message(world: &World, id: ObjectId, pilot: ObjectId, text: &str) -> Result<Vec<Notice>> {
    message_for(world, id, pilot, text, CommandNetwork::C3)
}
/// Publish C3i messages under a host checkpoint.
pub fn send(scripts: &Scripts, id: ObjectId, pilot: ObjectId, text: &str) -> Result<Vec<Notice>> {
    send_for(scripts, id, pilot, text, CommandNetwork::C3i)
}
/// Publish classic C3 messages under a host checkpoint.
pub fn send_c3(
    scripts: &Scripts,
    id: ObjectId,
    pilot: ObjectId,
    text: &str,
) -> Result<Vec<Notice>> {
    send_for(scripts, id, pilot, text, CommandNetwork::C3)
}
/// Native C3i message dispatch.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, input, CommandNetwork::C3i)
}
/// Native classic C3 message dispatch.
pub(crate) fn c3_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, input, CommandNetwork::C3)
}
