//! Shared visible firing messages for Mech and vehicle actions.
use super::{BattleHexCoordinate, BattleNotice, BattleWeapon};
use crate::ObjectId;

/// Target-independent report facts used by the common formatter.
pub(super) struct ShotFeedback {
    pub shooter: ObjectId,
    pub target: ObjectId,
    pub weapon: BattleWeapon,
    pub aimed_section: String,
    pub roll: u8,
    pub target_number: Option<i32>,
    pub glancing: bool,
    pub hit: bool,
    pub observer_hit: bool,
    pub coordinate: Option<BattleHexCoordinate>,
    pub pilot_notices: Vec<super::BattlePilotNotice>,
    pub notices: Vec<BattleNotice>,
}

/// Identities and geometry captured before damage can change visibility.
pub(super) struct ShotAudience {
    pub attacker_visible: bool,
    pub bearing: f64,
    pub observers: Vec<super::broadcast::InteractionObserver>,
    pub coordinate_messages: Vec<(ObjectId, String)>,
}

/// Format one already-launched shot with identical cockpit and third-party wording for all classes.
pub(super) fn messages(
    report: ShotFeedback,
    audience: ShotAudience,
    private: &mut Vec<super::BattlePilotNotice>,
) -> Vec<(ObjectId, String)> {
    let ShotAudience {
        attacker_visible,
        bearing,
        observers: fire_observers,
        coordinate_messages: hex_messages,
    } = audience;
    let mut messages = Vec::new();
    let coordinate_fire = report.coordinate.is_some();
    let target_hex = report
        .coordinate
        .unwrap_or(super::BattleHexCoordinate { x: 0, y: 0 });
    let catalog_name = report.weapon.name();
    let weapon_name = if coordinate_fire {
        catalog_name.split_once('.').expect("catalog namespace").1
    } else {
        catalog_name
    };
    let outcome = if report.glancing {
        "Glancing hit"
    } else if report.hit {
        "Hit"
    } else {
        "Miss"
    };
    let target_number = report
        .target_number
        .map_or("out of range".into(), |number| number.to_string());
    if coordinate_fire {
        messages.push((
            report.shooter,
            format!(
                "You fire {} at ({},{}) - BTH: {target_number} Roll: {}.",
                weapon_name, target_hex.x, target_hex.y, report.roll
            ),
        ));
    } else {
        messages.push((
            report.shooter,
            format!(
                "You fire {} at #{}{} - BTH: {target_number} Roll: {}. {outcome}.",
                weapon_name, report.target.0, report.aimed_section, report.roll
            ),
        ));
    }
    let source = if attacker_visible {
        format!("#{}", report.shooter.0)
    } else {
        format!("Something from bearing {bearing:.0}")
    };
    if report.shooter != report.target && coordinate_fire && !attacker_visible {
        messages.push((
            report.target,
            format!(
                "Something has fired a {} at you from bearing {bearing:.0}!",
                weapon_name
            ),
        ));
    } else if report.shooter != report.target {
        messages.push((
            report.target,
            format!("{source} has fired a {} at you!", weapon_name),
        ));
    }
    let hit = report.observer_hit;
    if coordinate_fire {
        messages.extend(hex_messages);
    } else {
        messages.extend(
            fire_observers
                .into_iter()
                .map(|observer| observer.fire_message(report.weapon, hit)),
        );
    }
    super::piloting::append_feedback(private, report.pilot_notices, messages.len());
    for notice in report.notices {
        messages.push((notice.unit, notice.text.to_owned()));
    }
    messages
}

/// Ordinary glancing hits notify the receiving cockpit for every unit class.
pub(super) fn glancing_notices(
    target: ObjectId,
    weapon: BattleWeapon,
    glancing: bool,
) -> Vec<BattleNotice> {
    if !glancing || weapon.is_streak() {
        return Vec::new();
    }
    vec![BattleNotice {
        unit: target,
        text: "You are nicked by a glancing blow!".into(),
    }]
}

/// Electronic interference has the same launcher feedback regardless of its carrier.
pub(super) fn streak_notices(shooter: ObjectId, confused: bool) -> Vec<BattleNotice> {
    if !confused {
        return Vec::new();
    }
    vec![BattleNotice {
        unit: shooter,
        text: "The ECM confuses your streak homing system!".into(),
    }]
}

/// Announce the completed lethal outcome once after all damage groups.
pub(super) fn destruction_notices(
    shooter: ObjectId,
    target: ObjectId,
    destroyed: bool,
) -> Vec<BattleNotice> {
    if !destroyed {
        return Vec::new();
    }
    vec![
        BattleNotice {
            unit: shooter,
            text: "You destroyed the target!".into(),
        },
        BattleNotice {
            unit: target,
            text: "You have been destroyed!".into(),
        },
    ]
}

/// Collect Mech damage packets once, regardless of which class fired them.
pub(super) fn mech_salvo_notices(
    salvo: &super::BattleSalvoReport,
    private: &mut Vec<super::BattlePilotNotice>,
) -> Vec<BattleNotice> {
    let mut notices = salvo
        .initial_woods
        .iter()
        .chain(salvo.woods.iter())
        .flat_map(|woods| woods.notices.iter().cloned())
        .collect::<Vec<_>>();
    if let Some(inferno) = &salvo.inferno {
        notices.extend(inferno.notices.iter().cloned());
    }
    for group in &salvo.groups {
        super::piloting::append_feedback(private, group.pilot_notices.clone(), notices.len());
        notices.extend(group.notices.iter().cloned());
    }
    notices
}

/// Join vehicle damage packets for either shooter class while retaining pilot-only checks.
pub(super) fn vehicle_salvo_notices(
    salvo: &super::BattleVehicleSalvoReport,
    private: &mut Vec<super::BattlePilotNotice>,
) -> Vec<BattleNotice> {
    let mut notices = salvo
        .initial_woods
        .iter()
        .chain(salvo.woods.iter())
        .flat_map(|woods| woods.notices.iter().cloned())
        .collect::<Vec<_>>();
    for group in &salvo.groups {
        super::piloting::append_feedback(
            private,
            group.impact.pilot_notices.iter().cloned(),
            notices.len(),
        );
        notices.extend(group.impact.notices.iter().cloned());
    }
    if let Some(inferno) = &salvo.inferno {
        super::piloting::append_feedback(
            private,
            inferno.pilot_notices.iter().cloned(),
            notices.len(),
        );
        notices.extend(inferno.notices.iter().cloned());
    }
    notices
}
