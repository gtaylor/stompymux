//! Shared launch-failure feedback for unit and coordinate shots.
//!
//! A failed Streak lock replaces the ordinary fire messages. Every feed failure fires the shot
//! first (Total Warfare p. 114), so its consequences follow the ordinary messages instead.
use super::{HexShotReport, MechShotReport, Notice, Weapon};
use crate::ObjectId;

/// Mechanical launch facts plus the consequence notices already captured by the resolver.
pub(super) struct LaunchFeedback {
    shooter: ObjectId,
    weapon: Weapon,
    fire_mode: super::FireMode,
    feed_critical_roll: Option<u8>,
    feed_broken: bool,
    loader_destroyed: bool,
    jammed: bool,
    launched: bool,
    /// Internal-damage notices from rapid-fire rounds exploding in the barrel.
    misload_notices: Option<Vec<Notice>>,
    pilot_notices: Vec<super::PilotNotice>,
    notices: Vec<Notice>,
}

impl From<&MechShotReport> for LaunchFeedback {
    /// Capture common mechanical facts without inventing a damage target.
    fn from(report: &MechShotReport) -> Self {
        Self {
            shooter: report.shooter,
            weapon: report.expenditure.weapon,
            fire_mode: report.expenditure.fire_mode,
            feed_critical_roll: report.feed_critical_roll,
            feed_broken: report.feed_broken,
            loader_destroyed: report.loader_destroyed,
            jammed: report.jammed,
            launched: report.launched,
            misload_notices: report.misload.as_ref().map(|hit| hit.notices.clone()),
            pilot_notices: report
                .misload
                .as_ref()
                .map_or_else(Vec::new, |hit| hit.pilot_notices.clone()),
            notices: report.notices(),
        }
    }
}

impl From<&HexShotReport> for LaunchFeedback {
    /// Capture common mechanical facts without inventing a damage target.
    fn from(report: &HexShotReport) -> Self {
        Self {
            shooter: report.shooter,
            weapon: report.expenditure.weapon,
            fire_mode: report.expenditure.fire_mode,
            feed_critical_roll: report.feed_critical_roll,
            feed_broken: report.feed_broken,
            loader_destroyed: report.loader_destroyed,
            jammed: report.jammed,
            launched: report.launched,
            misload_notices: report.misload.as_ref().map(|hit| hit.notices()),
            pilot_notices: report
                .misload
                .as_ref()
                .map_or_else(Vec::new, |hit| hit.pilot_notices().to_vec()),
            notices: report.notices(),
        }
    }
}

impl From<&super::ArtilleryLaunchReport> for LaunchFeedback {
    /// Artillery shares loader failures and immediate shooter consequences.
    fn from(report: &super::ArtilleryLaunchReport) -> Self {
        Self {
            shooter: report.shooter,
            weapon: report.expenditure.weapon,
            fire_mode: report.expenditure.fire_mode,
            feed_critical_roll: report.feed_critical_roll,
            feed_broken: report.feed_broken,
            loader_destroyed: report.loader_destroyed,
            jammed: report.jammed,
            launched: report.launched,
            misload_notices: report.misload.as_ref().map(|hit| hit.notices()),
            pilot_notices: report
                .misload
                .as_ref()
                .map_or_else(Vec::new, |hit| hit.pilot_notices().to_vec()),
            notices: report.notices(),
        }
    }
}

impl From<&super::VehicleShotReport> for LaunchFeedback {
    /// Vehicle storage supplies the same launch facts to the common feedback policy.
    fn from(report: &super::VehicleShotReport) -> Self {
        Self {
            shooter: report.shooter,
            weapon: report.expenditure.weapon,
            fire_mode: report.expenditure.fire_mode,
            feed_critical_roll: report.feed_critical_roll,
            feed_broken: report.feed_broken,
            loader_destroyed: report.loader_destroyed,
            jammed: report.jammed,
            launched: report.expenditure.launched,
            misload_notices: report.misload.as_ref().map(|hit| hit.notices.clone()),
            pilot_notices: Vec::new(),
            notices: report.notices(),
        }
    }
}

/// Return failed-lock feedback, or None when ordinary fire messages should follow.
pub(super) fn failure_messages(report: &LaunchFeedback) -> Option<Vec<(ObjectId, String)>> {
    if report.launched {
        return None;
    }
    Some(
        report
            .notices
            .iter()
            .map(|notice| (notice.unit, notice.text.to_owned()))
            .collect(),
    )
}

/// Feed consequences published after the shot's ordinary feedback. `observers` is the
/// internal-explosion broadcast for rapid-fire rounds exploding in the barrel; `base` is the
/// position the first returned message will take in the caller's message list.
pub(super) fn consequence_messages(
    report: LaunchFeedback,
    observers: Vec<(ObjectId, String)>,
    private: &mut Vec<super::PilotNotice>,
    base: usize,
) -> Vec<(ObjectId, String)> {
    let shooter = report.shooter;
    let name = report.weapon.name().split_once('.').unwrap().1;
    let mut messages = Vec::new();
    if let Some(critical) = report.feed_critical_roll {
        messages.push((
            shooter,
            format!(
                "[fg=red bold]The caseless feed on your {name} breaks down and the shot goes wild![reset]"
            ),
        ));
        messages.push((
            shooter,
            if report.loader_destroyed && super::impact::critical_count(critical) > 0 {
                format!("Critical check: {critical}. The misfeed wrecks your {name}!")
            } else if super::impact::critical_count(critical) > 0 {
                format!("Critical check: {critical}. The misfeed damages your {name}!")
            } else {
                format!("Critical check: {critical}. Your {name} is jammed beyond field repair.")
            },
        ));
    }
    if let Some(misload_notices) = report.misload_notices {
        messages.push((
            shooter,
            format!(
                "[fg=red bold]The rounds in your {name} explode in the barrel, destroying it![reset]"
            ),
        ));
        messages.extend(observers);
        super::piloting::append_feedback(private, report.pilot_notices, base + messages.len());
        messages.extend(
            misload_notices
                .into_iter()
                .map(|notice| (notice.unit, notice.text.to_owned())),
        );
        return messages;
    }
    if report.feed_critical_roll.is_some() {
        return messages;
    }
    if report.loader_destroyed {
        messages.push((
            shooter,
            format!("[fg=red bold]The feed on your {name} fails, destroying it![reset]"),
        ));
        return messages;
    }
    if report.feed_broken {
        messages.push((
            shooter,
            format!(
                "[fg=red bold]The ammo loader jams on your {name}! It is useless until repaired.[reset]"
            ),
        ));
        return messages;
    }
    if report.jammed {
        messages.push((
            shooter,
            format!(
                "[fg=red bold]The ammo {} mechanism jams on your {name}![reset]",
                if report.fire_mode == super::FireMode::Rapid || report.weapon.is_rotary() {
                    "loader"
                } else {
                    "loading"
                },
            ),
        ));
    }
    messages
}

/// Both unit classes use the same failed-lock feedback without expenditure or target damage.
pub(super) fn streak_failure(shooter: ObjectId) -> Vec<Notice> {
    vec![Notice {
        unit: shooter,
        text: "Your streak fails to lock on.".into(),
    }]
}
