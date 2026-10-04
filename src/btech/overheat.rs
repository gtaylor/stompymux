//! Saved thermal-check cadence and atomic heat injuries, ammunition hazards and shutdowns.
use super::{Mech, Notice, Power};
use crate::{Flag, Kind, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Unit-local committed-second timing; elapsed saturates until heat reaches a checkable level.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverheatClock {
    pub elapsed: u8,
    pub phase: u8,
    pub injury_due: bool,
}

impl OverheatClock {
    /// Validate saved counters and the once-per-turn injury marker.
    pub(super) fn validate(self) -> Result<()> {
        ensure!(
            self.elapsed <= 30 && self.phase < 30 && (!self.injury_due || self.phase == 0),
            "Invalid overheat clock"
        );
        Ok(())
    }

    /// Called exactly once by thermal accounting at a committed simulation-second boundary.
    pub(super) fn tick(&mut self) {
        self.elapsed = (self.elapsed + 1).min(30);
        self.phase = (self.phase + 1) % 30;
        self.injury_due = self.phase == 0;
    }
}

impl Mech {
    /// Inspect timing without scheduling or consuming a check.
    pub fn overheat_clock(&self) -> OverheatClock {
        self.overheat_clock
    }

    /// Stable hot units still need checks even when another thermal sample changes no heat.
    pub fn overheat_active(&self) -> bool {
        !self.is_destroyed() && (self.power() == Power::Running || self.heat().excess >= 10.0)
    }
}

/// Rules required by immediate thermal damage and shutdown falls.
#[derive(Debug, Clone, Copy)]
pub struct OverheatRules {
    /// Vehicle damage policy for mines reached by this consequence chain.
    pub vehicle_impact: super::VehicleImpactRules,
    pub stacking: super::StackingRules,
    pub hit: super::HitRules,
    pub extended_piloting: bool,
    pub stagger: super::StaggerMode,
}

/// An avoidance roll, or an automatic result without RNG; Computer rolls may use unskilled dice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct HeatCheck {
    pub target: i16,
    pub roll: Option<u8>,
    pub success: bool,
    pub computer: bool,
}

/// A due thermal event; all effects have been applied and notices await the enclosing commit.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[must_use = "Publish thermal notices after the enclosing world transaction succeeds"]
pub struct OverheatReport {
    pub unit: ObjectId,
    pub injury: Option<super::TacticalPilotInjury>,
    pub character_injury: Option<super::CharacterPilotInjury>,
    pub stacking_impacts: Vec<super::TacticalImpact>,
    pub stacking_falls: Vec<super::MechFallReport>,
    pub ammunition_check: Option<HeatCheck>,
    pub explosion: Option<super::TacticalImpact>,
    pub shutdown_check: Option<HeatCheck>,
    /// Normal Computer skill award after a successful in-character shutdown override.
    pub computer_experience: Option<super::ExperienceAward>,
    pub shutdown: bool,
    pub balance: Option<super::PilotingCheck>,
    /// Accepted Computer and shutdown balance XP captured before publication.
    pub experience_messages: Vec<super::DiagnosticMessage>,
    pub fall: Option<super::MechFallReport>,
    /// Private control rolls indexed within the unformatted notice stream.
    pub pilot_notices: Vec<super::PilotNotice>,
    pub notices: Vec<Notice>,
}

/// Shared marker keeps the formatted Computer roll adjacent to its override notice.
const COMPUTER_OVERRIDE_NOTICE: &str = "You frantically attempt to override the shutdown process!";

impl OverheatReport {
    /// Render ordered cockpit messages, including the target and result of a Computer override.
    pub fn messages(&self) -> Vec<(ObjectId, String)> {
        self.messages_with_feedback(&mut Vec::new())
    }

    /// Account for inserted Computer diagnostics when carrying private notice positions.
    pub(crate) fn messages_with_feedback(
        &self,
        private: &mut Vec<super::PilotNotice>,
    ) -> Vec<(ObjectId, String)> {
        let mut messages = Vec::new();
        let mut offsets = Vec::with_capacity(self.notices.len() + 1);
        for notice in &self.notices {
            offsets.push(messages.len());
            messages.push((notice.unit, notice.text.to_owned()));
            if notice.text == COMPUTER_OVERRIDE_NOTICE
                && let Some(check) = self.shutdown_check.filter(|check| check.computer)
                && let Some(roll) = check.roll
            {
                messages.push((
                    self.unit,
                    format!(
                        "You make a computer skill roll! Modified skill BTH: {} Roll: {roll}",
                        check.target
                    ),
                ));
            }
        }
        offsets.push(messages.len());
        private.extend(self.pilot_notices.iter().cloned().map(|mut notice| {
            notice.before_notice = offsets[notice.before_notice];
            notice
        }));
        messages
    }
}

/// Consume due markers after advance_heat, preserving timing, RNG and all damage on any failure.
pub fn advance_overheat(world: &mut World, rules: OverheatRules) -> Result<Vec<OverheatReport>> {
    advance_overheat_inner(world, rules, false)
}

/// Resolve due thermal checks inside a host action that publishes character consequences.
pub(super) fn advance_overheat_in_action(
    world: &mut World,
    rules: OverheatRules,
) -> Result<Vec<OverheatReport>> {
    advance_overheat_inner(world, rules, true)
}

/// Shared heat injury, ammunition and shutdown sequence for either publication mode.
fn advance_overheat_inner(
    world: &mut World,
    rules: OverheatRules,
    character: bool,
) -> Result<Vec<OverheatReport>> {
    world.attempt(|world| {
        let ids: Vec<_> = world
            .btech
            .constructed_units()
            .iter()
            .filter(|(id, unit)| {
                !unit.is_destroyed()
                    && (unit.overheat_clock.injury_due
                        || (unit.overheat_clock.elapsed == 30 && unit.heat().excess >= 10.0))
                    && world.objects.get(id).is_some_and(|object| {
                        !object.flags.contains(Flag::Going)
                            && (character || !object.flags.contains(Flag::InCharacter))
                    })
            })
            .map(|(&id, _)| id)
            .collect();
        let mut reports = Vec::new();
        for id in ids {
            let character_unit = character && world.objects[&id].flags.contains(Flag::InCharacter);
            world.btech.constructed_units()[&id].validate()?;
            let toughness = advantage(world, id, "Toughness");
            let fall_rules = super::FallRules {
                vehicle_impact: rules.vehicle_impact,
                stacking: rules.stacking,
                hit: rules.hit,
                extended_piloting: rules.extended_piloting,
                stagger: rules.stagger,
                toughness,
            };
            let mut report = OverheatReport {
                unit: id,
                injury: None,
                character_injury: None,
                stacking_impacts: Vec::new(),
                stacking_falls: Vec::new(),
                ammunition_check: None,
                explosion: None,
                shutdown_check: None,
                shutdown: false,
                balance: None,
                experience_messages: Vec::new(),
                computer_experience: None,
                fall: None,
                notices: Vec::new(),
                pilot_notices: Vec::new(),
            };
            let unit = unit_mut(world, id);
            let heat = unit.heat().excess;
            let injury_due = std::mem::take(&mut unit.overheat_clock.injury_due);
            if injury_due {
                let failed_support = unit.system_hits(super::System::LifeSupport) > 0;
                let exposed = failed_support || (heat > 30.0 && unit.dice.die(2)? == 1);
                let hits = if exposed && heat > 25.0 {
                    if failed_support { 2 } else { 1 }
                } else if exposed && heat >= 15.0 {
                    1
                } else {
                    0
                };
                if hits > 0 && unit.pilot().is_some() {
                    report.notices.push(Notice {
                        unit: id,
                        text: "You take personal injury from heat!".to_owned(),
                    });
                    if character_unit {
                        report.character_injury =
                            Some(super::injure_character_pilot(world, id, hits, toughness)?);
                    } else {
                        let injury = super::injure_tactical_pilot(world, id, hits, toughness)?;
                        if let Some(notice) = injury.notice(id) {
                            report.notices.push(notice);
                        }
                        report.injury = Some(injury);
                    }
                }
            }
            let unit = &world.btech.constructed_units()[&id];
            if !unit.is_destroyed() && heat >= 10.0 && unit.overheat_clock.elapsed == 30 {
                unit_mut(world, id).overheat_clock.elapsed = 0;
                let inferno = if rules.hit.inferno_penalty {
                    world.btech.constructed_units()[&id].inferno_ammunition_hazard()?
                } else {
                    None
                };
                if let Some(target) = ammunition_target(heat, inferno.is_some()) {
                    let roll = unit_mut(world, id).dice.generic_roll();
                    let check = HeatCheck {
                        target,
                        roll: Some(roll),
                        success: i16::from(roll) >= target,
                        computer: false,
                    };
                    report.ammunition_check = Some(check);
                    if !check.success {
                        let hazard = match inferno {
                            Some(hazard) => Some(hazard),
                            None => {
                                world.btech.constructed_units()[&id].ammunition_hazard_maximum()?
                            }
                        };
                        if let Some(hazard) = hazard {
                            let explode = if character {
                                super::impact::explode_ammunition_in_action
                            } else {
                                super::explode_ammunition
                            };
                            let explosion = explode(world, id, hazard.index, fall_rules)?;
                            super::piloting::append_feedback(
                                &mut report.pilot_notices,
                                explosion.pilot_notices.clone(),
                                report.notices.len(),
                            );
                            report.notices.extend(explosion.notices.iter().cloned());
                            report.explosion = Some(explosion);
                        } else {
                            report.notices.push(Notice {
                                unit: id,
                                text: "You have no ammunition, lucky you!".to_owned(),
                            });
                        }
                    }
                }
                if !world.btech.constructed_units()[&id].is_destroyed() {
                    let check = shutdown_check(world, id, heat)?;
                    if check.computer {
                        report.notices.push(Notice {
                            unit: id,
                            text: COMPUTER_OVERRIDE_NOTICE.to_owned(),
                        });
                        if check.success && character_unit {
                            let (award, message) = award_computer_override(world, id)?;
                            report.computer_experience = Some(award);
                            report.experience_messages.extend(message);
                        }
                    }
                    report.shutdown_check = check
                        .computer
                        .then_some(check)
                        .or_else(|| (heat >= 14.0 || !check.success).then_some(check));
                    if !check.success
                        && world.btech.constructed_units()[&id].power() == Power::Running
                    {
                        report.notices.push(Notice {
                            unit: id,
                            text: "Reactor shutting down...".to_owned(),
                        });
                        let airborne = world.btech.constructed_units()[&id].airborne();
                        if airborne {
                            report.notices.push(Notice {
                                unit: id,
                                text: "Reactor shutdown cuts your jump short!".to_owned(),
                            });
                        }
                        report.notices.extend(super::broadcast::observer_notices(
                            world,
                            id,
                            if airborne {
                                "falls from the sky!"
                            } else {
                                "stops in mid-motion!"
                            },
                        ));
                        if airborne {
                            let input = super::stacking::physical_input(
                                world,
                                id,
                                super::StackingEntry::Fall,
                            )?;
                            let resolve = if character_unit {
                                super::fall::resolve_character_signed_fall
                            } else {
                                super::fall::resolve_signed_fall
                            };
                            let fall = resolve(
                                world,
                                id,
                                i16::try_from(input.jump_movement_points)
                                    .context("Jump fall multiplier exceeds supported range")?,
                                fall_rules,
                            )?;
                            fall.append_notices(id, &mut report.notices, &mut report.pilot_notices);
                            report.fall = Some(fall);
                            let input = super::stacking::physical_input(
                                world,
                                id,
                                super::StackingEntry::Fall,
                            )?;
                            if character {
                                let mut effects = super::stacking::StackingEffects::default();
                                report.notices.extend(super::stacking::resolve_in_action(
                                    world,
                                    id,
                                    input,
                                    rules.stacking,
                                    fall_rules,
                                    &mut effects,
                                    (&mut report.pilot_notices, report.notices.len()),
                                )?);
                                report
                                    .experience_messages
                                    .extend(effects.experience_messages);
                                report.stacking_impacts = effects.impacts;
                                report.stacking_falls = effects.falls;
                            } else {
                                report.notices.extend(super::resolve_stacking(
                                    world,
                                    id,
                                    input,
                                    rules.stacking,
                                    fall_rules,
                                )?);
                            }
                        }
                        let unit = &world.btech.constructed_units()[&id];
                        if unit.posture() != super::Posture::Prone
                            && unit
                                .motion()
                                .is_some_and(|motion| motion.speed.abs() > 10.75)
                        {
                            let mut balance =
                                super::roll_piloting(world, id, 3, rules.extended_piloting)?;
                            super::piloting::capture_feedback(
                                id,
                                world.btech.constructed_units()[&id].pilot(),
                                &balance,
                                &mut report.notices,
                                &mut report.pilot_notices,
                            );
                            if character {
                                report.experience_messages.extend(
                                    super::piloting::award_control_check(
                                        world,
                                        id,
                                        &mut balance,
                                        rules.extended_piloting,
                                    )?,
                                );
                            }
                            if !balance.success {
                                report.notices.extend(super::broadcast::observer_notices(
                                    world,
                                    id,
                                    "falls down!",
                                ));
                                let fall = if character_unit {
                                    super::fall::resolve_character_fall(world, id, 0, fall_rules)?
                                } else {
                                    super::fall::resolve_zero_fall(world, id, fall_rules)?
                                };
                                report.notices.push(Notice {
                                    unit: id,
                                    text: "You lose your balance and fall down!".to_owned(),
                                });
                                fall.append_notices(
                                    id,
                                    &mut report.notices,
                                    &mut report.pilot_notices,
                                );
                                report.fall = Some(fall);
                            }
                            report.balance = Some(balance);
                        }
                        let unit = unit_mut(world, id);
                        let dropped = unit.carried_club.take().is_some();
                        unit.power = Power::Off;
                        unit.hide_elapsed = None;
                        unit.masc.shutdown();
                        unit.supercharger.shutdown();
                        unit.reconcile_electronics();
                        unit.charge.target = None;
                        unit.jump_stabilization = 0;
                        unit.pilot = None;
                        unit.target_lock = None;
                        unit.stand_timer = None;
                        if let Some(motion) = &mut unit.motion {
                            motion.speed = 0.0;
                            motion.desired_speed = 0.0;
                            motion.desired_heading = motion.heading;
                        }
                        if dropped {
                            report
                                .notices
                                .extend(super::club::dropped_notices(world, id));
                        }
                        report.shutdown = true;
                    }
                }
            }
            world.btech.constructed_units()[&id].validate()?;
            if report.character_injury.is_some()
                || report.injury.is_some()
                || report.ammunition_check.is_some()
                || report.shutdown_check.is_some()
            {
                reports.push(report);
            }
        }
        Ok(reports)
    })
}

/// Read a canonical boolean advantage from the currently assigned pilot.
fn advantage(world: &World, id: ObjectId, name: &str) -> bool {
    world.btech.constructed_units()[&id]
        .pilot()
        .and_then(|pilot| world.btech.character_values().get(&pilot))
        .is_some_and(|values| super::advantages::enabled(values, name))
}

/// Borrow one unit in the enclosing private candidate.
fn unit_mut(world: &mut World, id: ObjectId) -> &mut Mech {
    world.btech.constructed.get_mut(&id).unwrap()
}

/// Conventional ammunition avoidance bands; inferno ammunition is not supported by construction.
fn ammunition_target(heat: f64, inferno: bool) -> Option<i16> {
    if inferno {
        return if heat >= 28.0 {
            Some(12)
        } else if heat >= 23.0 {
            Some(10)
        } else if heat >= 19.0 {
            Some(8)
        } else if heat >= 14.0 {
            Some(6)
        } else if heat >= 10.0 {
            Some(4)
        } else {
            None
        };
    }
    if heat >= 28.0 {
        Some(8)
    } else if heat >= 23.0 {
        Some(6)
    } else if heat >= 19.0 {
        Some(4)
    } else {
        None
    }
}

/// Successful in-character overrides use the ordinary skill interval, threshold and channel.
/// Shutdown admission has already established a player pilot with a successful Computer roll.
fn award_computer_override(
    world: &mut World,
    id: ObjectId,
) -> Result<(super::ExperienceAward, Option<super::DiagnosticMessage>)> {
    let pilot = world.btech.constructed_units()[&id]
        .pilot()
        .context("Computer override has no pilot")?;
    let award = super::award_skill_experience(
        world,
        pilot,
        "Computer",
        1,
        crate::clock::wall_time(),
        false,
    )?;
    let message = award.accepted.then(|| {
        super::DiagnosticMessage::new(
            super::DiagnosticChannel::Experience,
            format!(
                "{} gained 1 computer XP (mech #{})",
                world.objects[&pilot].name, id.0
            ),
        )
    });
    Ok((award, message))
}

/// Player Computer overrides and unpiloted reactor thresholds use separate reference rules.
fn shutdown_check(world: &mut World, id: ObjectId, heat: f64) -> Result<HeatCheck> {
    let pilot = world.btech.constructed_units()[&id]
        .pilot()
        .filter(|pilot| {
            world
                .objects
                .get(pilot)
                .is_some_and(|object| object.kind == Kind::Player)
        });
    if let Some(pilot) = pilot {
        if heat < 14.0 {
            return Ok(HeatCheck {
                target: 0,
                roll: None,
                success: true,
                computer: false,
            });
        }
        let modifier = if heat >= 30.0 {
            8
        } else if heat >= 26.0 {
            6
        } else if heat >= 22.0 {
            4
        } else if heat >= 18.0 {
            2
        } else {
            0
        };
        let target = super::skills::character_skill_target(
            world,
            pilot,
            "Computer",
            super::SkillCategory::Mental,
        )? + modifier;
        let skilled = world
            .btech
            .character_values()
            .get(&pilot)
            .and_then(|values| values.get("Computer"))
            .is_some_and(|value| value.effective_skill() > 0);
        let dice = &mut unit_mut(world, id).dice;
        let roll = if skilled {
            dice.two_d6()
        } else {
            let rolls = [dice.d6(), dice.d6(), dice.d6()];
            rolls.iter().sum::<u8>() - rolls.iter().max().unwrap()
        };
        return Ok(HeatCheck {
            target,
            roll: Some(roll),
            success: i16::from(roll) >= target,
            computer: true,
        });
    }
    let target = if !(14.0..30.0).contains(&heat) {
        13
    } else if heat >= 26.0 {
        10
    } else if heat >= 22.0 {
        8
    } else if heat >= 18.0 {
        6
    } else {
        4
    };
    let roll = (target != 13).then(|| unit_mut(world, id).dice.generic_roll());
    Ok(HeatCheck {
        target,
        roll,
        success: roll.is_some_and(|roll| i16::from(roll) >= target),
        computer: false,
    })
}
