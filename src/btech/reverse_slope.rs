//! Shared reverse-slope control checks and feedback; each chassis owns its fall and placement.
use super::*;
use crate::{ObjectId, World};
use anyhow::Result;

/// Result of encountering an incline or drop while moving backward.
pub(super) struct ReverseSlopeCheck {
    pub success: bool,
    pub pilot_notices: Vec<PilotNotice>,
    pub notices: Vec<Notice>,
    pub experience_messages: Vec<DiagnosticMessage>,
}

/// Use the same height modifier, pilot exemption and XP rules across ground unit types.
pub(super) fn check(
    world: &mut World,
    id: ObjectId,
    change: i16,
    extended: bool,
    character: bool,
) -> Result<ReverseSlopeCheck> {
    let control = super::terrain_control::check(world, id, change.abs() - 1, extended, character)?;
    let success = control.success;
    let mut notices = vec![Notice {
        unit: id,
        text: if change > 0 {
            "You notice a small incline behind you!"
        } else {
            "You notice a small drop behind you!"
        }
        .into(),
    }];
    let mut pilot_notices = Vec::new();
    control.capture_feedback(id, &mut notices, &mut pilot_notices);
    notices.push(Notice {
        unit: id,
        text: if success {
            "You manage to overcome the obstacle."
        } else if change > 0 {
            "You stumble on your rear and fall down."
        } else {
            "You fall on your rear off the small incline."
        }
        .into(),
    });
    if !success {
        notices.extend(super::broadcast::observer_notices(
            world,
            id,
            if change > 0 {
                "falls on its back walking up an incline."
            } else {
                "falls off the back of a small incline."
            },
        ));
    }
    Ok(ReverseSlopeCheck {
        success,
        notices,
        experience_messages: control.experience_messages,
        pilot_notices,
    })
}
