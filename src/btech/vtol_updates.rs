//! Aircraft movement events reuse launch, fuel, vehicle controls and contact consequences.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Append feedback through the ordinary movement publication boundary.
fn notice(report: &mut super::movement_report::MovementReport, id: ObjectId, text: &str) {
    report.notices.push(BattleNotice {
        unit: id,
        text: text.into(),
    });
}

/// Advance descent first, then launch and powered flight; new lift loss waits until the next tick.
pub(super) fn advance_all(
    world: &mut World,
    rules: BattleMovementRules,
    character: bool,
) -> Result<super::movement_report::MovementReport> {
    let mut report = super::vtol_crash::advance_all(world, rules, character)?;
    let ids: Vec<_> = world
        .btech
        .vehicles()
        .iter()
        .filter_map(|(&id, unit)| {
            (unit.vtol_flight().is_some_and(|flight| {
                matches!(
                    flight.phase,
                    BattleVtolFlightPhase::Launching { .. } | BattleVtolFlightPhase::Airborne
                )
            }) && world
                .objects
                .get(&id)
                .is_some_and(|object| !object.flags.contains(Flag::Going)))
            .then_some(id)
        })
        .collect();
    for id in ids {
        ensure!(
            character || !world.objects[&id].flags.contains(Flag::InCharacter),
            "Character flight requires the host movement transaction"
        );
        let unit = &world.btech.vehicles()[&id];
        let flight = unit
            .vtol_flight()
            .context("Aircraft flight state is unavailable")?;
        let position = unit.position().context("Aircraft is not placed")?;
        ensure!(
            world
                .objects
                .get(&position.map)
                .is_some_and(|object| !object.flags.contains(Flag::Going)),
            "Aircraft map is unavailable"
        );
        let map = world
            .btech
            .maps()
            .get(&position.map)
            .context("Aircraft map is unavailable")?;
        if matches!(flight.phase, BattleVtolFlightPhase::Launching { .. }) {
            let underground = map.has_flag(super::MapFlag::Underground);
            let result = world
                .btech
                .vehicles
                .get_mut(&id)
                .unwrap()
                .advance_vtol_takeoff(underground, rules.free_fusion_vtol_fuel)?;
            match result {
                BattleVtolTakeoff::Aborted { reason } => {
                    notice(&mut report, id, &format!("Takeoff aborted: {reason}"))
                }
                BattleVtolTakeoff::LiftedOff => {
                    notice(&mut report, id, "You lift off!");
                    report.notices.extend(super::broadcast::observer_notices(
                        world,
                        id,
                        "lifts off!",
                    ));
                }
                _ => {}
            }
            continue;
        }
        if flight.phase != BattleVtolFlightPhase::Airborne {
            continue;
        }
        // An administratively inserted, stopped aircraft waits aloft until startup finishes.
        if unit.power() != BattlePower::Running {
            continue;
        }
        if unit.is_destroyed() || unit.rotor_destroyed() {
            world.btech.vehicles.get_mut(&id).unwrap().lose_vtol_lift();
            continue;
        }
        let mut motion = unit.motion().context("Aircraft motion is unavailable")?;
        let maximum =
            super::load::movement_maximum(world, id, unit.maximum_speed(), rules.tsm_tow_bonus)?;
        let maximum = super::speed_bonus::on_map(world, Some(position), maximum)?;
        if super::load::carries_load(world, id) {
            motion.limit_load(maximum, maximum);
        }
        if !motion.active() && flight.vertical_speed == 0.0 {
            world.btech.vehicles.get_mut(&id).unwrap().motion = Some(motion);
            continue;
        }
        let speed_demon = unit
            .pilot()
            .is_some_and(|pilot| super::skills::boolean_advantage(world, pilot, "Speed_Demon"));
        let mut fall_rules = rules.fall;
        fall_rules.toughness |= unit
            .pilot()
            .is_some_and(|pilot| super::skills::boolean_advantage(world, pilot, "Toughness"));
        let movement = BattleVehicleMotionRules {
            fasa_turning: rules.fasa_turning,
            slowdown: rules.slowdown,
            speed_demon,
            movement_modifier: map.movement_modifier,
        };
        let unit = world.btech.vehicles.get_mut(&id).unwrap();
        let fuel = unit.consume_vtol_fuel(
            flight.vertical_speed,
            flight.altitude as i32,
            false,
            rules.free_fusion_vtol_fuel,
        )?;
        if let BattleVtolFuelUse::Exhausted { newly } = fuel {
            if newly {
                notice(&mut report, id, "You run out of fuel and begin to fall!");
            }
            continue;
        }
        unit.motion = Some(unit.definition().control_at_maximum(
            motion,
            super::Hex::at_level(0),
            movement,
            maximum,
        )?);
        let edge_motion = unit.motion.expect("controlled flight");
        let edge_map = unit.position().expect("placed flight").map;
        let observers = super::broadcast::observer_notices(world, id, "crashes into the ground!");
        let outcome = super::vtol_environment::advance_in_candidate(
            world,
            id,
            rules.free_fusion_vtol_fuel,
            fall_rules,
            character,
        )?;
        match outcome {
            BattleVtolEnvironment::Obstacle {
                fall,
                notices,
                pilot_notices,
                experience_messages,
                ..
            } => {
                super::piloting::append_feedback(
                    &mut report.pilot_notices,
                    pilot_notices,
                    report.notices.len(),
                );
                report.notices.extend(notices);
                report.experience_messages.extend(experience_messages);
                report.vehicle_falls.extend(fall.map(|fall| *fall));
            }
            BattleVtolEnvironment::Boundary { .. } => {
                report
                    .boundaries
                    .push(super::movement_report::BattleBoundaryCrossing::new(
                        id,
                        edge_map,
                        edge_motion,
                        "You cannot move off this map!",
                    ));
                notice(&mut report, id, "You cannot move off this map!")
            }
            BattleVtolEnvironment::Movement {
                path: BattleVtolPath::Advanced { step },
            } if step.ceiling_reached => {
                notice(
                    &mut report,
                    id,
                    "You cannot achieve orbit! Vertical movement halted!",
                );
            }
            BattleVtolEnvironment::Landed { landing, .. } => {
                let effects = super::vtol_controls::landing_consequences(
                    world, id, landing, fall_rules, character,
                )?;
                super::piloting::append_feedback(
                    &mut report.pilot_notices,
                    effects.pilot_notices.iter().cloned(),
                    report.notices.len(),
                );
                report.notices.extend(effects.notices);
                report.mines.extend(effects.mines);
            }
            BattleVtolEnvironment::Flooded { newly: true, .. } => {
                notice(&mut report, id, "You crash your vehicle into the water!");
                notice(
                    &mut report,
                    id,
                    "Water pours into the cockpit....glub glub!",
                );
                report.notices.extend(super::broadcast::observer_notices(
                    world,
                    id,
                    "splashes into the water!",
                ));
            }
            BattleVtolEnvironment::Crashed { fall, .. } => {
                notice(
                    &mut report,
                    id,
                    "CRASH! You smash your toy into the ground!",
                );
                report.notices.extend(observers);
                super::piloting::append_feedback(
                    &mut report.pilot_notices,
                    fall.feedback.pilot_notices.iter().cloned(),
                    report.notices.len(),
                );
                report.notices.extend(fall.feedback.notices.iter().cloned());
                report.vehicle_falls.push(*fall);
            }
            BattleVtolEnvironment::CrashRequired { .. }
            | BattleVtolEnvironment::ObstacleRequired { .. } => {
                unreachable!("World contacts resolve crash damage")
            }
            _ => {}
        }
        let (notice, experience) = super::building_step::entered(world, id, position)?;
        report.notices.extend(notice);
        report.experience_messages.extend(experience);
    }
    Ok(report)
}
