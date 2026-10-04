//! Shared launch-failure feedback for unit and coordinate shots.
use super::{HexShotReport, MechShotReport, Notice, Weapon};
use crate::ObjectId;

/// Mechanical launch facts plus the consequence notices already captured by the resolver.
pub(super) struct LaunchFeedback {
    critical_failure: Option<super::WeaponDamageKind>,
    shooter: ObjectId,
    weapon: Weapon,
    fire_mode: super::FireMode,
    propellant_roll: Option<u8>,
    loader_destroyed: bool,
    jammed: bool,
    launched: bool,
    misload: bool,
    pilot_notices: Vec<super::PilotNotice>,
    notices: Vec<Notice>,
}

impl From<&MechShotReport> for LaunchFeedback {
    /// Capture common mechanical facts without inventing a damage target.
    fn from(report: &MechShotReport) -> Self {
        Self {
            critical_failure: report.expenditure.critical_failure,
            shooter: report.shooter,
            weapon: report.expenditure.weapon,
            fire_mode: report.expenditure.fire_mode,
            propellant_roll: report.propellant_roll,
            loader_destroyed: report.loader_destroyed,
            jammed: report.jammed,
            launched: report.launched,
            misload: report.misload.is_some(),
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
            critical_failure: report.expenditure.critical_failure,
            shooter: report.shooter,
            weapon: report.expenditure.weapon,
            fire_mode: report.expenditure.fire_mode,
            propellant_roll: report.propellant_roll,
            loader_destroyed: report.loader_destroyed,
            jammed: report.jammed,
            launched: report.launched,
            misload: report.misload.is_some(),
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
            critical_failure: report.expenditure.critical_failure,
            shooter: report.shooter,
            weapon: report.expenditure.weapon,
            fire_mode: report.expenditure.fire_mode,
            propellant_roll: report.propellant_roll,
            loader_destroyed: report.loader_destroyed,
            jammed: report.jammed,
            launched: report.launched,
            misload: report.misload.is_some(),
            pilot_notices: report
                .misload
                .as_ref()
                .map_or_else(Vec::new, |hit| hit.pilot_notices().to_vec()),
            notices: report.notices(),
        }
    }
}

/// Return terminal failure feedback, or None when ordinary fire messages should follow.
pub(super) fn failure_messages(
    report: LaunchFeedback,
    observers: Vec<(ObjectId, String)>,
    private: &mut Vec<super::PilotNotice>,
) -> Option<Vec<(ObjectId, String)>> {
    let shooter = report.shooter;
    if let Some(failure) = report.critical_failure {
        let name = report.weapon.name().split_once('.').unwrap().1;
        let text = match failure {
            super::WeaponDamageKind::Crystal => format!(
                "[fg=red bold]The damaged charging crystal on your {name} overloads![reset]"
            ),
            super::WeaponDamageKind::Feed => format!(
                "[fg=red bold]The damaged ammo feed on your {name} triggers an internal explosion![reset]"
            ),
            _ => format!("[fg=red bold]The ammo loader mechanism jams on your {name}![reset]"),
        };
        let mut messages = vec![(shooter, text)];
        if report.misload {
            messages.extend(observers);
        }
        super::piloting::append_feedback(private, report.pilot_notices, messages.len());
        messages.extend(
            report
                .notices
                .into_iter()
                .map(|notice| (notice.unit, notice.text)),
        );
        return Some(messages);
    }

    if report.propellant_roll.is_some() {
        let name = report.weapon.name().split_once('.').unwrap().1;
        let mut messages = vec![(
            shooter,
            format!("[fg=red bold]The ammo loading mechanism jams on your {name}![reset]"),
        )];
        if report.loader_destroyed {
            messages.extend(observers);
            messages.push((
                shooter,
                format!("[fg=red bold]Propellant from your {name} ignites and destroys it![reset]"),
            ));
        }
        super::piloting::append_feedback(private, report.pilot_notices, messages.len());
        messages.extend(
            report
                .notices
                .clone()
                .into_iter()
                .map(|notice| (notice.unit, notice.text.to_owned())),
        );
        return Some(messages);
    }
    if report.misload {
        let mut messages = vec![(
            shooter,
            format!(
                "[fg=red bold]A catastrophic misload on your {} destroys it and causes an internal explosion![reset]",
                report.weapon.name().split_once('.').unwrap().1
            ),
        )];
        messages.extend(observers);
        super::piloting::append_feedback(private, report.pilot_notices, messages.len());
        messages.extend(
            report
                .notices
                .clone()
                .into_iter()
                .map(|notice| (notice.unit, notice.text.to_owned())),
        );
        return Some(messages);
    }
    if report.loader_destroyed {
        let messages = vec![(
            shooter,
            format!(
                "The loader jams on your {}, destroying it!",
                report.weapon.name().split_once('.').unwrap().1
            ),
        )];
        return Some(messages);
    }
    if report.jammed {
        let messages = vec![(
            shooter,
            format!(
                "[fg=red bold]The ammo {} mechanism jams on your {}![reset]",
                if report.fire_mode == super::FireMode::Rapid || report.weapon.is_rotary() {
                    "loader"
                } else {
                    "loading"
                },
                report.weapon.name().split_once('.').unwrap().1
            ),
        )];
        return Some(messages);
    }
    if !report.launched {
        let messages = report
            .notices
            .clone()
            .into_iter()
            .map(|notice| (notice.unit, notice.text.to_owned()))
            .collect();
        return Some(messages);
    }
    None
}

impl From<&super::VehicleShotReport> for LaunchFeedback {
    /// Vehicle storage supplies the same launch facts to the common feedback policy.
    fn from(report: &super::VehicleShotReport) -> Self {
        Self {
            critical_failure: None,
            shooter: report.shooter,
            weapon: report.expenditure.weapon,
            fire_mode: report.expenditure.fire_mode,
            propellant_roll: report.propellant_roll,
            loader_destroyed: report.loader_destroyed,
            jammed: report.jammed,
            launched: report.expenditure.launched,
            misload: report.misload.is_some(),
            pilot_notices: Vec::new(),
            notices: report.notices(),
        }
    }
}

/// Both unit classes use the same failed-lock feedback without expenditure or target damage.
pub(super) fn streak_failure(shooter: ObjectId) -> Vec<Notice> {
    vec![Notice {
        unit: shooter,
        text: "Your streak fails to lock on.".into(),
    }]
}
