//! Shared terrain control checks, including pilotless exemption and optional character XP.
use super::*;
use crate::{ObjectId, World};
use anyhow::Result;

/// A terrain control decision and its deferred experience feedback.
pub(super) struct TerrainControl {
    pub success: bool,
    pub experience_messages: Vec<BattleChannelMessage>,
    pilot: Option<ObjectId>,
    roll: Option<BattlePilotingCheck>,
}

impl TerrainControl {
    /// Exemption or explicit auto-fall decisions carry neither dice nor feedback.
    pub fn automatic(success: bool) -> Self {
        Self {
            success,
            experience_messages: Vec::new(),
            pilot: None,
            roll: None,
        }
    }

    /// Insert deferred roll feedback after the caller's terrain warning.
    pub fn capture_feedback(
        &self,
        id: ObjectId,
        notices: &mut Vec<BattleNotice>,
        private: &mut Vec<BattlePilotNotice>,
    ) {
        if let Some(check) = &self.roll {
            super::piloting::capture_feedback(id, self.pilot, check, notices, private);
        }
    }
}

/// Terrain avoidance succeeds without dice when no pilot is assigned.
pub(super) fn check(
    world: &mut World,
    id: ObjectId,
    modifier: i16,
    extended: bool,
    character: bool,
) -> Result<TerrainControl> {
    let pilot = crate::btech::with_unit!(world.btech.unit(id).unwrap(), |unit| { unit.pilot() });
    if pilot.is_none() {
        return Ok(TerrainControl::automatic(true));
    }
    let mut check = super::roll_piloting(world, id, modifier, extended)?;
    let mut experience_messages = Vec::new();
    if character {
        experience_messages.extend(super::piloting::award_control_check(
            world, id, &mut check, extended,
        )?);
    }
    Ok(TerrainControl {
        success: check.success,
        experience_messages,
        pilot,
        roll: Some(check),
    })
}
