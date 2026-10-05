//! Shared in-character perception attempts and accepted experience diagnostics for scan actions.
use super::{DiagnosticMessage, DiagnosticTopic};
use crate::{Flag, Kind, ObjectId, World};
use anyhow::{Context, Result};

/// Roll only for an active in-character pilot; success is independent of the XP interval.
/// The enclosing action owns rollback of dice and any accepted experience award.
pub(super) fn attempt(
    world: &mut World,
    unit: ObjectId,
    pilot: ObjectId,
    modifier: i64,
    now: i64,
) -> Result<(bool, Option<DiagnosticMessage>)> {
    if !world.objects[&unit].flags.contains(Flag::InCharacter)
        || world.objects.get(&pilot).is_none_or(|p| {
            p.kind != Kind::Player
                || !p.flags.contains(Flag::Connected)
                || p.flags.contains(Flag::Going)
        })
    {
        return Ok((false, None));
    }
    let target = i64::from(
        super::scanner::scanner_unit(world, unit)
            .context("Scanner is unavailable")?
            .perception,
    ) + modifier;
    let roll = super::dice::unit_dice_mut(world, unit)?.generic_roll();
    if i64::from(roll) < target {
        return Ok((false, None));
    }
    let award = super::award_skill_experience(world, pilot, "Perception", 1, now, false)?;
    let message = award.accepted.then(|| {
        DiagnosticMessage::new(
            DiagnosticTopic::Experience,
            format!("{} gained 1 perception XP", world.objects[&pilot].name),
        )
    });
    Ok((true, message))
}
