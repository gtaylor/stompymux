//! Shared motive consequences and feedback for located hits and environmental fire.
use super::*;
use crate::ObjectId;

/// Movement vulnerability used by both advanced motive and fire checks.
pub(super) fn modifier(movement: VehicleMovement) -> u8 {
    match movement {
        VehicleMovement::Wheeled => 2,
        VehicleMovement::Hover => 4,
        _ => 0,
    }
}

/// Resolve an advanced motive roll without changing state or drawing additional dice.
pub(super) fn outcome(movement: VehicleMovement, roll: u8) -> (Option<VehicleMotiveHit>, u8) {
    match roll + modifier(movement) {
        8 | 9 => (None, 1),
        10 | 11 => (Some(VehicleMotiveHit::SpeedLoss { movement_points: 1 }), 2),
        12.. => (Some(VehicleMotiveHit::Immobilize), 0),
        _ => (None, 0),
    }
}

/// Commit motive damage and return cockpit notices plus raw visibility-dependent broadcasts.
pub(super) fn apply(
    vehicle: &mut Vehicle,
    id: ObjectId,
    motive: Option<VehicleMotiveHit>,
    penalty: u8,
    roll: Option<u8>,
) -> (Vec<Notice>, Vec<Notice>) {
    let mut notices = Vec::new();
    let mut broadcasts = Vec::new();
    vehicle.piloting_damage = vehicle.piloting_damage.saturating_add(penalty).min(127);
    let was_immobilized = vehicle.immobilized();
    let speed = vehicle.motion().map_or(0.0, |motion| motion.speed);
    if let Some(motive) = motive {
        vehicle.apply_motive_hit(motive);
    }
    if motive.is_some() || penalty > 0 {
        notices.push(Notice {
            unit: id,
            text: "[fg=yellow bold]CRITICAL HIT![reset]".into(),
        });
        notices.push(Notice {
            unit: id,
            text: motive_notice(
                vehicle.definition().movement,
                motive,
                penalty,
                roll,
                was_immobilized,
            ),
        });
    }
    if roll.is_some() && speed != 0.0 {
        let text = match penalty {
            1 => Some("wobbles slightly."),
            2 => Some("wobbles violently."),
            _ if motive.is_some() && speed > 0.0 => {
                Some("shakes violently then begins to slow down.")
            }
            _ => None,
        };
        if let Some(text) = text {
            broadcasts.push(Notice {
                unit: id,
                text: text.into(),
            });
        }
    }
    (notices, broadcasts)
}

/// Describe the table's direct motive consequence using the vehicle's movement system.
fn motive_notice(
    movement: VehicleMovement,
    motive: Option<VehicleMotiveHit>,
    penalty: u8,
    roll: Option<u8>,
    repeated: bool,
) -> String {
    if roll.is_some() {
        if repeated {
            return "[fg=red bold]Your destroyed motive system takes another hit![reset]".into();
        }
        let kind = match movement {
            VehicleMovement::Vtol => "rotorcraft",
            VehicleMovement::Tracked => "tank",
            VehicleMovement::Wheeled => "vehicle",
            VehicleMovement::Hover => "hovercraft",
            VehicleMovement::Stationary => "weird unidentifiable toy (warn a wizard!)",
        };
        return match penalty {
            1 => format!(
                "[fg=red bold]Your motive system takes a minor hit, making it harder to control your {kind}![reset]"
            ),
            2 => format!(
                "[fg=red bold]Your motive system takes a moderate hit, slowing you down and making it harder to control your {kind}![reset]"
            ),
            _ => format!(
                "[fg=red bold]Your motive system is destroyed! Your {kind} can no longer move![reset]"
            ),
        };
    }
    let severe = matches!(
        motive,
        Some(VehicleMotiveHit::SpeedLoss { movement_points: 2 })
    );
    let part = match movement {
        VehicleMovement::Vtol => "Your rotor",
        VehicleMovement::Tracked => "One of your tracks",
        VehicleMovement::Wheeled => "One of your wheels",
        VehicleMovement::Hover => "Your air skirt",
        VehicleMovement::Stationary => "Your motive system",
    };
    if motive == Some(VehicleMotiveHit::Immobilize) {
        if movement == VehicleMovement::Hover {
            return "Your lift fan is destroyed, immobilizing your vehicle!".into();
        }
        return format!("{part} is destroyed, immobilizing your vehicle!");
    }
    format!(
        "{part} is {}damaged!",
        if severe { "seriously " } else { "" }
    )
}
