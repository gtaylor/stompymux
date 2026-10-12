//! Shared terrain control checks, including pilotless exemption and optional character XP.
use super::*;
use crate::{ObjectId, World};
use anyhow::Result;

/// A terrain control decision and its deferred experience feedback.
pub(super) struct TerrainControl {
    pub success: bool,
    pub experience_messages: Vec<DiagnosticMessage>,
    pilot: Option<ObjectId>,
    roll: Option<PilotingCheck>,
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

    /// How far the roll fell short of its target, or zero when it succeeded or was not rolled.
    pub fn margin_of_failure(&self) -> u8 {
        match &self.roll {
            Some(check) if !check.success => check.roll.map_or(0, |roll| {
                (check.target - i32::from(roll)).clamp(0, 255) as u8
            }),
            _ => 0,
        }
    }

    /// Insert deferred roll feedback after the caller's terrain warning.
    pub fn capture_feedback(
        &self,
        id: ObjectId,
        notices: &mut Vec<Notice>,
        private: &mut Vec<PilotNotice>,
    ) {
        if let Some(check) = &self.roll {
            super::piloting::capture_feedback(id, self.pilot, check, notices, private);
        }
    }
}

/// A roll made to step onto a hex, which takes the hex's entry modifier instead of the modifier
/// the hex adds to rolls forced on units already in it: ice is +0 to step onto and +4 to keep
/// your feet on (Tactical Operations p. 48).
pub(super) fn check_entry(
    world: &mut World,
    id: ObjectId,
    modifier: i16,
    extended: bool,
    character: bool,
) -> Result<TerrainControl> {
    let standing = super::planetary_conditions::terrain_piloting_modifier(world, id);
    check(world, id, modifier - standing, extended, character)
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
