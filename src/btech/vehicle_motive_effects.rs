//! The Motive System Damage Table (Total Warfare p. 193): thresholds, attack-direction and
//! vehicle-type modifiers, once-per-tier driving penalties, and the hover-sinking consequence.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result};
use serde::Serialize;

/// Vehicle-type modifier to the motive roll: tracked +0, wheeled +2, hovercraft +3.
pub(super) fn modifier(movement: VehicleMovement) -> u8 {
    match movement {
        VehicleMovement::Wheeled => 2,
        VehicleMovement::Hover => 3,
        VehicleMovement::Tracked | VehicleMovement::Vtol | VehicleMovement::Stationary => 0,
    }
}

/// Attack-direction modifier to the motive roll: rear +1, sides +2.
pub(super) fn direction_modifier(arc: HitArc) -> u8 {
    match arc {
        HitArc::Rear => 1,
        HitArc::Left | HitArc::Right => 2,
        HitArc::Front => 0,
    }
}

/// One Motive System Damage Table roll selected by a hit-location result, before application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MotiveSystemDamage {
    /// The 2d6 roll before modifiers.
    pub roll: u8,
    /// Attack-direction plus vehicle-type modifier.
    pub modifier: u8,
    pub hit: Option<VehicleMotiveHit>,
    /// Driving Skill Roll modifier of the tier reached; applied only the first time per tier.
    pub driving_penalty: u8,
}

/// Interpret a roll that already includes any attack-direction modifier; the vehicle-type
/// modifier is added here. The returned penalty is the tier's driving modifier (1, 2 or 3).
pub(super) fn outcome(movement: VehicleMovement, roll: u8) -> (Option<VehicleMotiveHit>, u8) {
    match roll.saturating_add(modifier(movement)) {
        6 | 7 => (None, 1),
        8 | 9 => (Some(VehicleMotiveHit::SpeedLoss { movement_points: 1 }), 2),
        10 | 11 => (Some(VehicleMotiveHit::HalveSpeed), 3),
        12.. => (Some(VehicleMotiveHit::Immobilize), 0),
        _ => (None, 0),
    }
}

/// Resolve a hit-location motive roll for a vehicle struck from `arc` without changing state.
pub(super) fn located(vehicle: &Vehicle, arc: HitArc, roll: u8) -> MotiveSystemDamage {
    let movement = vehicle.definition().movement;
    let direction = direction_modifier(arc);
    let (hit, tier) = outcome(movement, roll.saturating_add(direction));
    MotiveSystemDamage {
        roll,
        modifier: direction + modifier(movement),
        hit,
        driving_penalty: if tier_applied(vehicle, tier) { 0 } else { tier },
    }
}

/// Each driving modifier of the table applies once, for a maximum of +6 (TW p. 193).
fn tier_applied(vehicle: &Vehicle, tier: u8) -> bool {
    tier == 0 || vehicle.motive_penalties & (1 << tier) != 0
}

/// Commit motive damage and return cockpit notices plus raw visibility-dependent broadcasts.
/// `penalty` names the table tier reached (its driving modifier); it is applied once per tier.
pub(super) fn apply(
    vehicle: &mut Vehicle,
    id: ObjectId,
    motive: Option<VehicleMotiveHit>,
    penalty: u8,
    roll: Option<u8>,
) -> (Vec<Notice>, Vec<Notice>) {
    let mut notices = Vec::new();
    let mut broadcasts = Vec::new();
    let penalty = if tier_applied(vehicle, penalty) {
        0
    } else {
        vehicle.motive_penalties |= 1 << penalty;
        penalty
    };
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
            text: motive_notice(vehicle.definition().movement, motive, roll, was_immobilized),
        });
    }
    if roll.is_some() && speed != 0.0 {
        let text = match (motive, penalty) {
            (None, 1) => Some("wobbles slightly."),
            (Some(VehicleMotiveHit::SpeedLoss { .. }), _) => Some("wobbles violently."),
            (Some(_), _) if speed > 0.0 => Some("shakes violently then begins to slow down."),
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

/// Apply a hit-location motive result inside the damage candidate, then sink an immobilized
/// hovercraft over Depth 1 or deeper water (TW p. 193).
pub(super) fn apply_located(
    world: &mut World,
    id: ObjectId,
    damage: MotiveSystemDamage,
) -> Result<(Vec<Notice>, Vec<Notice>)> {
    let vehicle = world
        .btech
        .vehicles
        .get_mut(&id)
        .context("Vehicle is unavailable")?;
    let (mut notices, mut broadcasts) = apply(
        vehicle,
        id,
        damage.hit,
        damage.driving_penalty,
        Some(damage.roll),
    );
    let (sunk, observed) = sink_immobilized_hovercraft(world, id)?;
    notices.extend(sunk);
    broadcasts.extend(observed);
    Ok((notices, broadcasts))
}

/// An immobilized hovercraft over water of Depth 1 or more sinks and is destroyed.
pub(super) fn sink_immobilized_hovercraft(
    world: &mut World,
    id: ObjectId,
) -> Result<(Vec<Notice>, Vec<Notice>)> {
    let vehicle = &world.btech.vehicles()[&id];
    if vehicle.definition().movement != VehicleMovement::Hover
        || !vehicle.immobilized()
        || vehicle.is_destroyed()
    {
        return Ok((Vec::new(), Vec::new()));
    }
    let Some(position) = vehicle.position() else {
        return Ok((Vec::new(), Vec::new()));
    };
    let tile = world
        .btech
        .maps()
        .get(&position.map)
        .context("Vehicle map is unavailable")?
        .base_hex(i64::from(position.x), i64::from(position.y))?;
    if !tile.is_open_water() || tile.water_depth() == 0 || tile.has_bridge() {
        return Ok((Vec::new(), Vec::new()));
    }
    world
        .btech
        .vehicles
        .get_mut(&id)
        .unwrap()
        .destroy_by_flooding();
    Ok((
        vec![Notice {
            unit: id,
            text: "[fg=red bold]Without its lift fans your hovercraft settles into the water and sinks![reset]"
                .into(),
        }],
        vec![Notice {
            unit: id,
            text: "settles into the water and sinks!".into(),
        }],
    ))
}

/// Describe the table's direct motive consequence using the vehicle's movement system.
fn motive_notice(
    movement: VehicleMovement,
    motive: Option<VehicleMotiveHit>,
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
        return match motive {
            None => format!(
                "[fg=red bold]Your motive system takes a minor hit, making it harder to control your {kind}![reset]"
            ),
            Some(VehicleMotiveHit::SpeedLoss { .. }) => format!(
                "[fg=red bold]Your motive system takes a moderate hit, slowing you down and making it harder to control your {kind}![reset]"
            ),
            Some(VehicleMotiveHit::HalveSpeed) => format!(
                "[fg=red bold]Your motive system takes a heavy hit, halving your speed and making your {kind} much harder to control![reset]"
            ),
            Some(VehicleMotiveHit::Immobilize) => format!(
                "[fg=red bold]Your motive system is destroyed! Your {kind} can no longer move![reset]"
            ),
        };
    }
    let part = match movement {
        VehicleMovement::Vtol => "Your rotor",
        VehicleMovement::Tracked => "One of your tracks",
        VehicleMovement::Wheeled => "One of your wheels",
        VehicleMovement::Hover => "Your air skirt",
        VehicleMovement::Stationary => "Your motive system",
    };
    match motive {
        Some(VehicleMotiveHit::Immobilize) if movement == VehicleMovement::Hover => {
            "Your lift fan is destroyed, immobilizing your vehicle!".into()
        }
        Some(VehicleMotiveHit::Immobilize) => {
            format!("{part} is destroyed, immobilizing your vehicle!")
        }
        Some(VehicleMotiveHit::HalveSpeed) => format!("{part} is seriously damaged!"),
        _ => format!("{part} is damaged!"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_thresholds_follow_total_warfare() {
        use VehicleMovement as M;
        assert_eq!(outcome(M::Tracked, 5), (None, 0));
        assert_eq!(outcome(M::Tracked, 6), (None, 1));
        assert_eq!(outcome(M::Tracked, 7), (None, 1));
        assert_eq!(
            outcome(M::Tracked, 8),
            (Some(VehicleMotiveHit::SpeedLoss { movement_points: 1 }), 2)
        );
        assert_eq!(
            outcome(M::Tracked, 10),
            (Some(VehicleMotiveHit::HalveSpeed), 3)
        );
        assert_eq!(
            outcome(M::Tracked, 12),
            (Some(VehicleMotiveHit::Immobilize), 0)
        );
        // Wheeled +2 and hover +3 shift the same roll up the table.
        assert_eq!(
            outcome(M::Wheeled, 6),
            (Some(VehicleMotiveHit::SpeedLoss { movement_points: 1 }), 2)
        );
        assert_eq!(
            outcome(M::Hover, 9),
            (Some(VehicleMotiveHit::Immobilize), 0)
        );
    }

    #[test]
    fn attack_direction_modifiers_follow_total_warfare() {
        assert_eq!(direction_modifier(HitArc::Front), 0);
        assert_eq!(direction_modifier(HitArc::Rear), 1);
        assert_eq!(direction_modifier(HitArc::Left), 2);
        assert_eq!(direction_modifier(HitArc::Right), 2);
    }
}
