//! Vehicle cliff feedback and control checks; movement owns position and fall publication.
use super::*;
use crate::{ObjectId, World};
use anyhow::Result;

/// Attempt to stop before an uphill crash or downhill fall, without awarding control XP.
pub(super) fn check(
    world: &mut World,
    id: ObjectId,
    change: i32,
    speed: f64,
    rules: MovementRules,
) -> Result<(bool, super::movement_report::MovementReport)> {
    let control = super::cliff::avoids(world, id, i16::try_from(change)?, speed, rules)?;
    let success = control.success;
    let (text, broadcast) = if success {
        if change > 0 {
            (
                "You manage to stop before crashing.",
                "stops suddenly to avoid a cliff!",
            )
        } else {
            (
                "You manage to stop before falling off.",
                "stops suddenly to avoid falling off a cliff!",
            )
        }
    } else if change < 0 {
        (
            "You drive off the cliff and fall to the ground below.",
            "drives off a cliff and falls to the ground below.",
        )
    } else if rules.skid_cliff {
        ("You skid to a violent halt!", "skids to a halt!")
    } else {
        ("You smash into a cliff!", "smashes into a cliff!")
    };
    let mut notices = vec![Notice {
        unit: id,
        text: if change > 0 {
            "You attempt to climb a hill too steep for you."
        } else {
            "You notice a large drop in front of you"
        }
        .into(),
    }];
    let mut pilot_notices = Vec::new();
    control.capture_feedback(id, &mut notices, &mut pilot_notices);
    notices.push(Notice {
        unit: id,
        text: text.into(),
    });
    notices.extend(super::broadcast::observer_notices(world, id, broadcast));
    Ok((
        success,
        super::movement_report::MovementReport {
            notices,
            pilot_notices,
            ..Default::default()
        },
    ))
}
