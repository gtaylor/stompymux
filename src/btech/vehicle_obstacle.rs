//! Shared ground-vehicle tree and rock avoidance, with chassis-specific eligibility only.
use super::*;
use crate::{ObjectId, World};
use anyhow::Result;

/// An obstacle's control modifier and speed-derived fall severity.
struct Obstacle {
    rocks: bool,
    modifier: i16,
    levels: u8,
}

/// Select the reference terrain checks without duplicating the control or fall logic.
/// Trees stand in woods; rocks lie in bare rough ground.
fn profile(
    movement: BattleVehicleMovement,
    hex: Hex,
    speed: f64,
    new_terrain: bool,
) -> Option<Obstacle> {
    let heavy_woods = hex.woods() == Some(Woods::Heavy);
    let rocks = hex.is_bare() && hex.ground() == Ground::Rough;
    let speed = speed.abs();
    if speed <= 10.75 {
        return None;
    }
    let applies = match movement {
        BattleVehicleMovement::Tracked => new_terrain && heavy_woods,
        BattleVehicleMovement::Wheeled => new_terrain && (hex.is_woods() || rocks),
        BattleVehicleMovement::Hover => hex.is_woods(),
        BattleVehicleMovement::Stationary | BattleVehicleMovement::Vtol => false,
    };
    if !applies {
        return None;
    }
    let heavy = heavy_woods && movement != BattleVehicleMovement::Tracked;
    Some(Obstacle {
        rocks,
        modifier: (speed / 10.75 / 6.0) as i16 + if heavy { 3 } else { 0 },
        levels: (speed / 10.75 / 2.0).sqrt().max(1.0) as u8,
    })
}

/// Resolve obstacle effects before slope speed loss and ordinary entry mines/fire.
pub(super) fn resolve(
    world: &mut World,
    id: ObjectId,
    hex: Hex,
    rules: BattleMovementRules,
    character: bool,
) -> Result<super::movement_report::MovementReport> {
    let unit = &world.btech.vehicles()[&id];
    let Some(obstacle) = profile(
        unit.definition().movement,
        hex,
        unit.motion().unwrap().speed,
        rules.new_terrain,
    ) else {
        return Ok(Default::default());
    };
    let control = super::terrain_control::check(
        world,
        id,
        obstacle.modifier,
        rules.fall.extended_piloting,
        character,
    )?;
    let mut notices = vec![BattleNotice {
        unit: id,
        text: if obstacle.rocks {
            "You try to avoid the rocks.."
        } else {
            "You try to dodge the larger trees.."
        }
        .into(),
    }];
    let mut pilot_notices = Vec::new();
    control.capture_feedback(id, &mut notices, &mut pilot_notices);
    if control.success {
        notices.push(BattleNotice {
            unit: id,
            text: "You manage to dodge 'em!".into(),
        });
        return Ok(super::movement_report::MovementReport {
            notices,
            pilot_notices,
            experience_messages: control.experience_messages,
            ..Default::default()
        });
    }
    notices.push(BattleNotice {
        unit: id,
        text: "You swerve, but not enough! This'll hurt!".into(),
    });
    notices.extend(super::broadcast::observer_notices(
        world,
        id,
        if obstacle.rocks {
            "cruises headlong at a rock!"
        } else {
            "cruises headlong at a tree!"
        },
    ));
    let fall = super::vehicle_fall::resolve_in_candidate(
        world,
        id,
        obstacle.levels,
        rules.fall,
        character,
    )?;
    super::piloting::append_feedback(
        &mut pilot_notices,
        fall.pilot_notices.iter().cloned(),
        notices.len(),
    );
    notices.extend(fall.notices.iter().cloned());
    Ok(super::movement_report::MovementReport {
        notices,
        pilot_notices,
        experience_messages: control.experience_messages,
        vehicle_falls: vec![fall],
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chassis_configuration_and_speed_choose_the_same_obstacle_rules() {
        for movement in [
            BattleVehicleMovement::Tracked,
            BattleVehicleMovement::Wheeled,
            BattleVehicleMovement::Hover,
        ] {
            assert!(profile(movement, Hex::new(Terrain::HeavyForest, 0), 10.75, true).is_none());
            let hit = profile(movement, Hex::new(Terrain::HeavyForest, 0), -86.0, true).unwrap();
            assert_eq!(
                hit.modifier,
                if movement == BattleVehicleMovement::Tracked {
                    1
                } else {
                    4
                }
            );
            assert_eq!(hit.levels, 2);
            assert!(!hit.rocks);
            assert_eq!(
                profile(movement, Hex::new(Terrain::HeavyForest, 0), 86.0, false).is_some(),
                movement == BattleVehicleMovement::Hover
            );
            assert_eq!(
                profile(movement, Hex::new(Terrain::LightForest, 0), 86.0, true).is_some(),
                movement != BattleVehicleMovement::Tracked
            );
            assert_eq!(
                profile(movement, Hex::new(Terrain::Rough, 0), 86.0, true).is_some(),
                movement == BattleVehicleMovement::Wheeled
            );
        }
    }
}
