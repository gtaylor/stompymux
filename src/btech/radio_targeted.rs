//! Targeted line-of-sight radio with independent sender and recipient identity visibility.
use super::{BattleNotice, BattlePower, visible_contact};
use crate::{ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Captured cockpit notices for a targeted message; no channel, mine or experience phase is involved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish targeted radio notices through the enclosing host action"]
pub struct BattleTargetedRadioReport {
    /// Powered sending unit.
    pub sender: ObjectId,
    /// Visible acquired recipient.
    pub target: ObjectId,
    /// Sender echo followed by the target notice when its power is running.
    pub notices: Vec<BattleNotice>,
}

/// Resolve a powered pilot's targeted radio without spending dice or changing contact state.
pub fn resolve_targeted_radio(
    world: &World,
    sender: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    message: &str,
) -> Result<BattleTargetedRadioReport> {
    super::targeting::controlled(world, sender, pilot)?;
    let source = super::scanner::scanner_unit(world, sender).context("Sender is unavailable")?;
    ensure!(!source.observer, "You can't radio anyone.");
    ensure!(
        !message.is_empty() && message.chars().all(|c| !c.is_control()),
        "Radio message must be nonempty text without control characters"
    );
    let visible =
        visible_contact(world, sender, target)?.context("Target is not in line of sight!")?;
    let other = super::scanner::scanner_unit(world, target).context("Target is unavailable")?;
    let mut id = other.label.context("Target is not placed")?;
    if visible.friendly {
        id.make_ascii_lowercase();
    }
    let mut notices = vec![BattleNotice {
        unit: sender,
        text: format!("You radio {} [{id}] with, '{message}'", visible.name),
    }];
    if other.power == BattlePower::Running {
        let seen = visible_contact(world, target, sender)?;
        let mut source_id = source.label.context("Sender is not placed")?;
        if seen.as_ref().is_some_and(|v| v.friendly) {
            source_id.make_ascii_lowercase();
        }
        let name = seen.as_ref().map_or("something", |v| v.name.as_str());
        notices.push(BattleNotice {
            unit: target,
            text: format!("{name} [{source_id}] radios you with, '{message}'"),
        });
    }
    Ok(BattleTargetedRadioReport {
        sender,
        target,
        notices,
    })
}

/// Publish native/Lua targeted radio together; a rejected notification leaves no partial output.
pub fn send_targeted_radio_action(
    scripts: &Scripts,
    sender: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    message: &str,
) -> Result<BattleTargetedRadioReport> {
    let checkpoint = scripts.effects.checkpoint();
    let result = (|| {
        let report =
            resolve_targeted_radio(&scripts.world.borrow(), sender, pilot, target, message)?;
        for notice in &report.notices {
            super::notify_unit(scripts, notice.clone())?;
        }
        Ok(report)
    })();
    if result.is_err() {
        scripts.effects.restore(checkpoint);
    }
    result
}

/// Resolve an explicit dbref or case-insensitive saved battlefield label on the sender's map.
pub(super) fn target(world: &World, sender: ObjectId, text: &str) -> Result<ObjectId> {
    let map = super::scanner::scanner_unit(world, sender)
        .and_then(|unit| unit.position)
        .context("Sender is not placed")?
        .map;
    if let Some(number) = text.strip_prefix('#') {
        return Ok(ObjectId(number.parse().context("Invalid target")?));
    }
    world
        .btech
        .constructed_units()
        .keys()
        .chain(world.btech.vehicles().keys())
        .copied()
        .find(|id| {
            super::scanner::scanner_unit(world, *id).is_some_and(|unit| {
                unit.position.is_some_and(|position| position.map == map)
                    && unit
                        .label
                        .is_some_and(|label| label.eq_ignore_ascii_case(text))
            })
        })
        .context("Target is not in line of sight!")
}

/// Native targeted radio accepts map labels or dbrefs, with the payload following the first equals sign.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let (name, message) = input
            .args
            .split_once('=')
            .context("Invalid format! Usage: radio <id>=<message>")?;
        let (sender, recipient) = {
            let world = ctx.scripts.world.borrow();
            let sender = world
                .objects
                .get(&ctx.player)
                .and_then(|p| p.location)
                .context("Enter a unit first")?;
            (sender, target(&world, sender, name.trim())?)
        };
        send_targeted_radio_action(
            ctx.scripts,
            sender,
            ctx.player,
            recipient,
            message.trim_start(),
        )
    })();
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
