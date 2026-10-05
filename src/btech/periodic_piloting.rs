//! Heartbeat stability checks share piloting, fall, internal-damage and transaction services.
use super::{MechFallReport, MechSection, Notice, PilotingCheck, Power, TacticalImpact};
use crate::{Config, Flag, ObjectId, Scripts, World};
use anyhow::Result;
use serde::Serialize;

/// Completed heartbeat check and all immediate consequences, suitable for deterministic replay.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PeriodicPiloting {
    pub unit: ObjectId,
    pub check: PilotingCheck,
    /// Assigned recipient captured before fall damage or evacuation; absence means cockpit fallback.
    pub pilot: Option<ObjectId>,
    pub gravity_damage: u16,
    pub fall: Option<MechFallReport>,
    pub vehicle_fall: Option<super::VehicleFallReport>,
    pub impacts: Vec<TacticalImpact>,
    /// Private feedback from nested falls and gravity impacts.
    pub pilot_notices: Vec<super::PilotNotice>,
    pub notices: Vec<Notice>,
    pub experience_messages: Vec<super::DiagnosticMessage>,
}

/// Select the reference's prioritized check without consuming dice or changing unit state.
fn required(world: &World, id: ObjectId, config: &Config) -> Result<Option<(i16, u16)>> {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        let fallen = unit
            .vtol_flight()
            .is_some_and(|f| f.phase == super::VtolFlightPhase::Landed)
            && unit
                .sections()
                .get(&super::VehicleSection::Rotor)
                .is_some_and(|s| s.internal == 0);
        return Ok((world.btech.turn_clock.due()
            && unit.power() != Power::Running
            && super::crew::unit_unconscious(world, id)
            && !unit.is_destroyed()
            && unit.position().is_some()
            && !fallen
            && unit.orbital_drop().is_none()
            && unit.free_fall().is_none())
        .then_some((3, 0)));
    }
    let unit = &world.btech.constructed_units()[&id];
    if unit.is_destroyed()
        || unit.position().is_none()
        || (unit.power() != Power::Running && !super::crew::unit_unconscious(world, id))
    {
        return Ok(None);
    }
    let speed = unit.motion().map_or(0.0, |m| m.speed);
    let unloaded = unit.mobility().maximum_speed;
    let mut maximum =
        super::load::movement_maximum(world, id, unloaded, config.battletech.tsm_tow_bonus != 0)?;
    let mut modifier = None;
    let mut gravity_damage = 0;
    if world.btech.turn_clock.due() && unit.posture() != super::Posture::Prone && !unit.airborne() {
        if unit.power() != Power::Running {
            modifier = Some(3);
        }
        if unit.triple_myomer_active() {
            maximum = (((maximum / 1.5 / 10.75).round_ties_even() + 1.0) * 1.5).ceil() * 10.75;
        }
        let map = &world.btech.maps()[&unit.position().unwrap().map];
        if map.uses_special_rules() && map.gravity != 100 && speed > unloaded {
            modifier = Some(0);
            gravity_damage =
                (((speed - unloaded) / 10.75).trunc() + 1.0).min(f64::from(u16::MAX)) as u16;
        }
    }
    let hips = unit.loadout()?.systems.iter().any(|part| {
        part.system == super::System::ShoulderOrHip
            && unit.chassis().legs().contains(&part.location.section)
            && unit.critical_unavailable(part.location)
    });
    if speed > 2.0 * maximum / 3.0 + 0.1 && (unit.gyro_damage() > 0 || hips) {
        modifier = Some(0);
    }
    Ok(modifier.map(|modifier| (modifier, gravity_damage)))
}

/// Check every admitted Mech at the current global phase; the host owns clock advancement and publication.
pub fn advance_periodic_piloting(
    world: &mut World,
    config: &Config,
) -> Result<Vec<PeriodicPiloting>> {
    let ids: Vec<_> = world
        .btech
        .constructed_units()
        .keys()
        .chain(world.btech.vehicles().keys())
        .copied()
        .filter(|id| {
            world
                .objects
                .get(id)
                .is_some_and(|o| !o.flags.contains(Flag::Going))
        })
        .collect();
    let mut reports = Vec::new();
    for id in ids {
        let Some((modifier, gravity_damage)) = required(world, id, config)? else {
            continue;
        };
        let mut rules = super::FallRules::configured(config);
        rules.toughness = world
            .btech
            .constructed_units()
            .get(&id)
            .map_or_else(|| world.btech.vehicles()[&id].pilot(), |u| u.pilot())
            .is_some_and(|pilot| super::skills::boolean_advantage(world, pilot, "Toughness"));
        let mut check = super::roll_piloting(world, id, modifier, rules.extended_piloting)?;
        let pilot = world
            .btech
            .constructed_units()
            .get(&id)
            .map_or_else(|| world.btech.vehicles()[&id].pilot(), |unit| unit.pilot());
        let experience_messages =
            super::piloting::award_control_check(world, id, &mut check, rules.extended_piloting)?
                .into_iter()
                .collect();
        let mut report = PeriodicPiloting {
            unit: id,
            check,
            pilot,
            gravity_damage,
            fall: None,
            vehicle_fall: None,
            impacts: Vec::new(),
            pilot_notices: Vec::new(),
            notices: Vec::new(),
            experience_messages,
        };
        if !check.success {
            if gravity_damage > 0 {
                report.notices.push(Notice {
                    unit: id,
                    text: "Your legs take some damage!".into(),
                });
                let legs: Vec<_> = world.btech.constructed_units()[&id]
                    .chassis()
                    .legs()
                    .to_vec();
                for section in [
                    MechSection::LeftArm,
                    MechSection::RightArm,
                    MechSection::LeftLeg,
                    MechSection::RightLeg,
                ] {
                    if !legs.contains(&section)
                        || world.btech.constructed_units()[&id].sections()[&section].internal == 0
                    {
                        continue;
                    }
                    let impact = super::impact::resolve_internal_stress(
                        world,
                        id,
                        section,
                        gravity_damage,
                        rules,
                    )?;
                    super::piloting::append_feedback(
                        &mut report.pilot_notices,
                        impact.pilot_notices.iter().cloned(),
                        report.notices.len(),
                    );
                    report.notices.extend(impact.notices.clone());
                    report.impacts.push(impact);
                }
            } else {
                report.notices.push(Notice {
                    unit: id,
                    text: "Your damaged mech falls as you try to run!".into(),
                });
                report
                    .notices
                    .extend(super::broadcast::observer_notices(world, id, "falls down."));
                if world.btech.vehicles().contains_key(&id) {
                    let fall = super::vehicle_fall::resolve_in_candidate(
                        world,
                        id,
                        1,
                        rules,
                        world.objects[&id].flags.contains(Flag::InCharacter),
                    )?;
                    super::piloting::append_feedback(
                        &mut report.pilot_notices,
                        fall.feedback.pilot_notices.iter().cloned(),
                        report.notices.len(),
                    );
                    report.notices.extend(fall.feedback.notices.clone());
                    report.vehicle_fall = Some(fall);
                } else {
                    let fall = if world.objects[&id].flags.contains(Flag::InCharacter) {
                        super::fall::resolve_character_fall(world, id, 1, rules)?
                    } else {
                        super::resolve_fall(world, id, 1, rules)?
                    };
                    fall.append_notices(id, &mut report.notices, &mut report.pilot_notices);
                    report.fall = Some(fall);
                }
            }
        }
        reports.push(report);
    }
    Ok(reports)
}

/// Publish all check consequences inside one rollback boundary; the enclosing server commit includes the clock.
pub fn advance_periodic_piloting_action(
    scripts: &Scripts,
    config: &Config,
) -> Result<Vec<PeriodicPiloting>> {
    scripts.atomic(|before| {
        let reports = advance_periodic_piloting(&mut scripts.world_mut(), config)?;
        for report in &reports {
            if let Some(diagnostic) = report.check.diagnostic(true) {
                super::diagnostics::publish(scripts, std::slice::from_ref(&diagnostic));
            }
            if let Some(messages) = report.check.messages() {
                let recipient = report
                    .pilot
                    .map(super::MessageTarget::Player)
                    .unwrap_or(super::MessageTarget::Unit(report.unit));
                for text in messages {
                    super::notify_message(scripts, recipient, &text)?;
                }
            }
            super::piloting::publish_ordered_notices(
                scripts,
                &report.notices,
                &report.pilot_notices,
            )?;
            super::diagnostics::publish(scripts, &report.experience_messages);
            if let Some(fall) = &report.fall {
                super::evacuation::publish_fall_consequences(scripts, config, fall)?;
            }
            if let Some(fall) = &report.vehicle_fall {
                super::evacuation::publish_vehicle_fall_consequences(scripts, config, fall)?;
            }
            for impact in &report.impacts {
                super::evacuation::publish_impact_consequences(scripts, config, impact)?;
            }
        }
        super::evacuation::publish_new_casualties(scripts, config, before)?;
        scripts.world().validate_action(config)?;
        Ok(reports)
    })
}
