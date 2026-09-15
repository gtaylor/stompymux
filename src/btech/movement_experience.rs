//! Persistent movement cadence and coordinate gating for piloting experience.
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Successful hex-changing updates and the coordinates of the last eligible XP attempt.
/// The mark deliberately excludes map identity, and starts at coordinate zero.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleMovementExperience {
    pub hexes_walked: u64,
    pub last_award_position: (u16, u16),
}

impl super::BattleUnit {
    /// Persisted movement progress, shared by ground and airborne movement.
    pub fn movement_experience(&self) -> BattleMovementExperience {
        self.movement_experience
    }
}

/// Count one accepted hex-changing update; callers own movement eligibility and rollback.
/// Disconnected crew still accumulate distance, but do not consume the coordinate mark.
pub(super) fn record_entry(
    world: &mut World,
    id: ObjectId,
    extended: bool,
) -> Result<Option<super::BattleChannelMessage>> {
    if !world.objects[&id].flags.contains(Flag::InCharacter) {
        return Ok(None);
    }
    let unit = Arc::make_mut(&mut world.btech.constructed)
        .get_mut(&id)
        .unwrap();
    let progress = &mut unit.movement_experience;
    progress.hexes_walked = progress
        .hexes_walked
        .checked_add(1)
        .context("Movement counter overflow")?;
    if !progress.hexes_walked.is_multiple_of(10) {
        return Ok(None);
    }
    let Some(pilot) = unit.pilot else {
        return Ok(None);
    };
    if world.objects.get(&pilot).is_none_or(|pilot| {
        pilot.location != Some(id)
            || !pilot.flags.contains(Flag::Connected)
            || pilot.flags.contains(Flag::Going)
    }) {
        return Ok(None);
    }
    let position = unit.position.context("Moving unit is not placed")?;
    let coordinates = (position.x, position.y);
    if progress.last_award_position == coordinates {
        return Ok(None);
    }
    progress.last_award_position = coordinates;
    let skill = unit.chassis().piloting_skill(extended);
    let award =
        super::award_skill_experience(world, pilot, skill, 1, crate::clock::wall_time(), false)?;
    Ok(award.accepted.then(|| {
        super::BattleChannelMessage::new(
            super::BattleChannel::PilotingExperience,
            format!("{} gained 1 {skill} XP", world.objects[&pilot].name),
        )
    }))
}
