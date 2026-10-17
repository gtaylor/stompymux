//! Heartbeat stability checks share piloting, fall, internal-damage and transaction services.
//! 'Mechs roll to keep their footing; a vehicle neither falls nor rolls to keep its footing
//! (Total Warfare p. 68), so a shut-down vehicle with an unconscious crew simply sits where it
//! is. Both chassis roll at the end of a turn they outran their standard-gravity speed on a
//! light world (Tactical Operations: Advanced Rules p. 53).
use super::{MechFallReport, MechSection, Notice, PilotingCheck, Power, TacticalImpact};
use crate::{Config, Flag, ObjectId, Scripts, World};
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// What a Mech did since the last turn-clock boundary, for the checks Total Warfare makes
/// once a turn: a PSR for running with a damaged hip or gyro (p. 53) and the gravity
/// strain check (Tactical Operations p. 53).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TurnMovement {
    /// The unit travelled faster than its walking ceiling at some point this turn.
    #[serde(default)]
    pub ran: bool,
    /// The fastest speed over the unit's normal running ceiling this turn, in km/h.
    #[serde(default)]
    pub gravity_excess: f64,
}

impl TurnMovement {
    /// Note one second of travel at `speed` against the walking ceiling and the normal
    /// (1 G, unboosted) running ceiling.
    pub(super) fn observe(&mut self, speed: f64, walking: f64, normal_running: f64) {
        if speed > walking + 0.1 {
            self.ran = true;
        }
        if speed > normal_running {
            self.gravity_excess = self.gravity_excess.max(speed - normal_running);
        }
    }

    /// Saved values must be finite and non-negative.
    pub(crate) fn validate(self) -> Result<()> {
        anyhow::ensure!(
            self.gravity_excess.is_finite() && self.gravity_excess >= 0.0,
            "Invalid turn movement"
        );
        Ok(())
    }
}

impl super::Mech {
    /// What the unit has done since the last turn-clock boundary.
    pub fn turn_movement(&self) -> TurnMovement {
        self.turn_movement
    }
}

impl super::Vehicle {
    /// What the vehicle has done since the last turn-clock boundary.
    pub fn turn_movement(&self) -> TurnMovement {
        self.turn_movement
    }
}

/// Completed heartbeat check and all immediate consequences, suitable for deterministic replay.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PeriodicPiloting {
    pub unit: ObjectId,
    pub check: PilotingCheck,
    /// Assigned recipient captured before fall damage or evacuation; absence means cockpit fallback.
    pub pilot: Option<ObjectId>,
    pub gravity_damage: u16,
    pub fall: Option<MechFallReport>,
    pub impacts: Vec<TacticalImpact>,
    /// The front internal damage a vehicle's failed gravity strain roll did.
    pub vehicle_damage: Option<super::VehicleInternalDamage>,
    /// Private feedback from nested falls and gravity impacts.
    pub pilot_notices: Vec<super::PilotNotice>,
    pub notices: Vec<Notice>,
    pub experience_messages: Vec<super::DiagnosticMessage>,
}

/// Select the reference's prioritized check without consuming dice or changing unit state.
fn required(world: &World, id: ObjectId, config: &Config) -> Result<Option<(i16, u16)>> {
    let unit = &world.btech.constructed_units()[&id];
    if unit.is_destroyed()
        || unit.position().is_none()
        || (unit.power() != Power::Running && !super::crew::unit_unconscious(world, id))
    {
        return Ok(None);
    }
    let _ = config;
    let mut modifier = None;
    let mut gravity_damage = 0;
    // Being shut down, or having an unconscious MechWarrior, forces no roll of its own: the
    // +3 for a shutdown is made only in the phase the reactor shuts down, and a roll the
    // 'Mech must make then fails automatically (Total Warfare pp. 59-60).
    if !world.btech.turn_clock.starts_turn()
        || unit.posture() == super::Posture::Prone
        || unit.airborne()
    {
        return Ok(modifier.map(|modifier| (modifier, gravity_damage)));
    }
    let movement = unit.turn_movement;
    // Moving faster than its normal running MP under light gravity strains a 'Mech's legs:
    // one PSR at the turn's end, and on a failure one point of internal damage to each leg
    // for every MP by which it exceeded that figure (Tactical Operations p. 53).
    let map = &world.btech.maps()[&unit.position().unwrap().map];
    if map.gravity != 100 && movement.gravity_excess > 0.0 {
        modifier = Some(0);
        gravity_damage = (movement.gravity_excess / super::MP_KPH)
            .ceil()
            .clamp(1.0, f64::from(u16::MAX)) as u16;
    }
    // A 'Mech that runs in a turn with a damaged hip or gyro makes one PSR at the end of
    // that turn (Total Warfare pp. 53 and 59).
    let hips = unit.loadout()?.systems.iter().any(|part| {
        part.system == super::System::ShoulderOrHip
            && unit.chassis().legs().contains(&part.location.section)
            && unit.critical_unavailable(part.location)
    });
    if movement.ran && (unit.gyro_damage() > 0 || hips) {
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
        .copied()
        .filter(|id| {
            world
                .objects
                .get(id)
                .is_some_and(|o| !o.flags.contains(Flag::Going))
        })
        .collect();
    let mut reports = Vec::new();
    let boundary = world.btech.turn_clock.starts_turn();
    for id in ids {
        let required = required(world, id, config)?;
        if boundary && let Some(unit) = world.btech.constructed.get_mut(&id) {
            unit.turn_movement = TurnMovement::default();
        }
        let Some((modifier, gravity_damage)) = required else {
            continue;
        };
        let mut rules = super::FallRules::configured(config);
        rules.toughness = world.btech.constructed_units()[&id]
            .pilot()
            .is_some_and(|pilot| super::skills::boolean_advantage(world, pilot, "Toughness"));
        let mut check = super::roll_piloting(world, id, modifier, rules.extended_piloting)?;
        let pilot = world.btech.constructed_units()[&id].pilot();
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
            impacts: Vec::new(),
            vehicle_damage: None,
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
                let fall = if world.objects[&id].flags.contains(Flag::InCharacter) {
                    super::fall::resolve_character_fall(world, id, 1, rules)?
                } else {
                    super::resolve_fall(world, id, 1, rules)?
                };
                fall.append_notices(id, &mut report.notices, &mut report.pilot_notices);
                report.fall = Some(fall);
            }
        }
        reports.push(report);
    }
    reports.extend(advance_vehicle_strain(world, config, boundary)?);
    Ok(reports)
}

/// MP by which a vehicle outran its standard-gravity flank speed this turn on a map of
/// non-standard gravity, rounded up, if it did; checked once a turn.
fn vehicle_strain(world: &World, id: ObjectId) -> Option<u16> {
    let vehicle = &world.btech.vehicles()[&id];
    if vehicle.is_destroyed() || vehicle.power() != Power::Running {
        return None;
    }
    let map = world.btech.maps().get(&vehicle.position()?.map)?;
    let excess = vehicle.turn_movement.gravity_excess;
    if map.gravity == 100 || excess <= 0.0 {
        return None;
    }
    Some(
        (excess / super::MP_KPH)
            .ceil()
            .clamp(1.0, f64::from(u16::MAX)) as u16,
    )
}

/// Moving faster than its normal flanking speed under light gravity strains a vehicle, a VTOL
/// included: one driving roll at the turn's end, and on a failure one point of front internal
/// structure damage for every MP over that speed, with the critical roll that damage takes,
/// and a single roll on the Motive System Damage Table (Tactical Operations: Advanced Rules
/// p. 53).
fn advance_vehicle_strain(
    world: &mut World,
    config: &Config,
    boundary: bool,
) -> Result<Vec<PeriodicPiloting>> {
    let ids: Vec<_> = world
        .btech
        .vehicles()
        .keys()
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
        let strain = if boundary {
            vehicle_strain(world, id)
        } else {
            None
        };
        if boundary {
            world.btech.vehicles.get_mut(&id).unwrap().turn_movement = TurnMovement::default();
        }
        let Some(over) = strain else {
            continue;
        };
        let rules = super::FallRules::configured(config);
        let check = super::roll_piloting(world, id, 0, rules.extended_piloting)?;
        let pilot = world.btech.vehicles()[&id].pilot();
        let mut report = PeriodicPiloting {
            unit: id,
            check,
            pilot,
            gravity_damage: over,
            fall: None,
            impacts: Vec::new(),
            vehicle_damage: None,
            pilot_notices: Vec::new(),
            notices: Vec::new(),
            experience_messages: Vec::new(),
        };
        if !check.success {
            let vtol = world.btech.vehicles()[&id].definition().is_vtol();
            report.notices.push(Notice {
                unit: id,
                text: if vtol {
                    "Your rotor strains under the light gravity!".into()
                } else {
                    "Your drive train strains under the light gravity!".into()
                },
            });
            let front = world.btech.vehicles()[&id]
                .sections()
                .get(&super::VehicleSection::Front)
                .is_some_and(|state| state.internal > 0);
            if front {
                let damage = super::vehicle_internal_damage::resolve_in_candidate(
                    world,
                    id,
                    super::VehicleSection::Front,
                    u32::from(over),
                    rules.vehicle_impact.criticals,
                    Default::default(),
                )?;
                report.notices.extend(damage.notices.iter().cloned());
                for broadcast in &damage.broadcasts {
                    report.notices.extend(super::broadcast::observer_notices(
                        world,
                        id,
                        &broadcast.text,
                    ));
                }
                super::piloting::append_feedback(
                    &mut report.pilot_notices,
                    damage.pilot_notices.iter().cloned(),
                    report.notices.len(),
                );
                report.vehicle_damage = Some(damage);
            }
            report.notices.extend(super::ram::motive_roll(world, id)?);
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
            for impact in &report.impacts {
                super::evacuation::publish_impact_consequences(scripts, config, impact)?;
            }
        }
        super::evacuation::publish_new_casualties(scripts, config, before)?;
        scripts.world().validate_action(config)?;
        Ok(reports)
    })
}
