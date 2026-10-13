//! Durable damage windows and transactional piloting checks for biped staggering.
use super::{FallRules, Mech, MechFallReport, PilotingCheck, Posture, Power};
use crate::{Flag, ObjectId, World};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// Saved unit-local turn cadence and the damage taken during the current turn.
///
/// Total Warfare p. 60 counts every point of damage a Mech takes in one phase toward the
/// twenty that force a Piloting Skill Roll (+1), so internal ammunition explosions count like
/// any other hit. Fall damage is the one exclusion: a Mech that has just fallen is prone and
/// cannot fall again, so a completed fall ends the turn's accounting instead.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stagger {
    /// Restored action-time damage scalar; ordinary hits update the turn total independently.
    #[serde(default)]
    pub action_damage: i32,
    /// Damage taken since the current turn's check.
    pub turn_damage: u32,
    /// Second of the unit's thirty-second turn, 0 through 29.
    pub phase: u8,
    /// Phase at which the previous check was made, so damage after a check waits a full turn.
    pub checked_phase: Option<u8>,
}

impl Stagger {
    /// Positive twenty-point levels govern action-time checks, independently of the turn total.
    pub fn action_level(&self) -> i32 {
        (self.action_damage / 20).max(0)
    }

    /// Reject malformed turn phases.
    pub(super) fn validate(&self) -> Result<()> {
        ensure!(
            self.phase < 30 && self.checked_phase.is_none_or(|phase| phase < 30),
            "Invalid stagger phase"
        );
        Ok(())
    }

    /// Incoming ordinary damage counts once, before criticals or transfer change the unit.
    pub(super) fn record(&mut self, damage: u16) {
        self.turn_damage = self.turn_damage.saturating_add(u32::from(damage));
    }

    /// A completed fall ends the turn's damage accounting without changing the unit's phase.
    pub(super) fn clear_damage(&mut self) {
        self.turn_damage = 0;
    }
}

impl Mech {
    /// Saved damage and cadence used by stagger checks.
    pub fn stagger(&self) -> &Stagger {
        &self.stagger
    }

    /// Running units keep their turn phase even between hits; damaged units wait for their check.
    pub fn stagger_active(&self) -> bool {
        !self.is_destroyed() && (self.stagger.turn_damage > 0 || self.power() == Power::Running)
    }
}

/// Reference weight-class adjustment for action-time checks.
fn tonnage_modifier(tons: u16) -> i32 {
    match tons {
        0..=35 => 1,
        36..=55 => 0,
        56..=75 => -1,
        _ => -2,
    }
}

/// Action-time checks always apply the tonnage adjustment.
pub(super) fn action_modifier(unit: &Mech) -> i16 {
    if unit.power() != Power::Running {
        return 999;
    }
    (unit.stagger().action_level() + tonnage_modifier(unit.definition().tons))
        .clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

/// Rule choices shared by the server heartbeat and isolated simulation callers.
#[derive(Debug, Clone, Copy)]
pub struct StaggerRules {
    /// Vehicle damage policy for mines reached by this consequence chain.
    pub vehicle_impact: super::VehicleImpactRules,
    pub extended_piloting: bool,
}

/// One completed stagger check and its optional fall; notices are staged by the caller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish stagger and fall notices with the enclosing world commit"]
pub struct StaggerReport {
    pub unit: ObjectId,
    pub level: u32,
    pub check: PilotingCheck,
    /// Notice boundary immediately before the initial control feedback, including cockpit fallback.
    pub check_notice_index: usize,
    /// Accepted control XP captured before a possible protection check.
    pub experience_messages: Vec<super::DiagnosticMessage>,
    pub fall: Option<MechFallReport>,
    /// Ordered severity, observer, cockpit and damage feedback captured before the fall.
    pub notices: Vec<super::Notice>,
    /// Private roll feedback captured before fall consequences change crew state.
    pub pilot_notices: Vec<super::PilotNotice>,
}

/// Advance one committed second atomically, including all history, checks and resulting falls.
pub fn advance_stagger(world: &mut World, rules: StaggerRules) -> Result<Vec<StaggerReport>> {
    advance_stagger_inner(world, rules, false)
}

/// Advance stagger within a host action that publishes character injuries and casualties.
pub(super) fn advance_stagger_in_action(
    world: &mut World,
    rules: StaggerRules,
) -> Result<Vec<StaggerReport>> {
    advance_stagger_inner(world, rules, true)
}

/// Shared history expiry, control checks and falls for either publication mode.
///
/// A shut-down Mech that must make the roll fails it automatically and falls (Total Warfare
/// p. 60, Piloting Skill Roll Table), as does one whose MechWarrior is unconscious; both are
/// blocked rolls in `roll_piloting`.
fn advance_stagger_inner(
    world: &mut World,
    rules: StaggerRules,
    character: bool,
) -> Result<Vec<StaggerReport>> {
    world.attempt(|world| {
        let ids: Vec<_> = world
            .btech
            .constructed_units()
            .iter()
            .filter(|(id, unit)| {
                unit.stagger_active()
                    && world.objects.get(id).is_some_and(|object| {
                        !object.flags.contains(Flag::Going)
                            && (character || !object.flags.contains(Flag::InCharacter))
                    })
            })
            .map(|(&id, _)| id)
            .collect();
        let mut reports = Vec::new();
        for id in ids {
            let unit = &world.btech.constructed_units()[&id];
            unit.validate()?;
            let prone = unit.posture() == Posture::Prone;
            let airborne = unit.airborne();
            let unit = world.btech.constructed.get_mut(&id).unwrap();
            let history = &mut unit.stagger;
            history.phase = (history.phase + 1) % 30;
            if history.turn_damage < 20
                || history
                    .checked_phase
                    .is_some_and(|phase| phase != history.phase)
            {
                if history.checked_phase == Some(history.phase)
                    || (history.checked_phase.is_none() && history.phase == 0)
                {
                    history.turn_damage = 0;
                    history.checked_phase = None;
                }
                continue;
            }
            history.turn_damage = 0;
            history.checked_phase = Some(history.phase);
            if prone || airborne {
                continue;
            }
            let level = 1;
            let mut notices = Vec::new();
            notices.push(super::Notice {
                unit: id,
                text: "You stagger from the damage!".to_owned(),
            });
            let mut check = super::roll_piloting(world, id, 1, rules.extended_piloting)?;
            let experience_messages = if character {
                super::piloting::award_control_check(
                    world,
                    id,
                    &mut check,
                    rules.extended_piloting,
                )?
                .into_iter()
                .collect()
            } else {
                Vec::new()
            };
            let mut pilot_notices = Vec::new();
            let check_notice_index = notices.len();
            super::piloting::capture_feedback(
                id,
                world.btech.constructed_units()[&id].pilot(),
                &check,
                &mut notices,
                &mut pilot_notices,
            );
            let toughness = world.btech.constructed_units()[&id]
                .pilot()
                .and_then(|pilot| world.btech.character_values().get(&pilot))
                .is_some_and(|values| super::advantages::enabled(values, "Toughness"));
            let fall = if check.success {
                None
            } else {
                notices.push(super::Notice {
                    unit: id,
                    text: "You fall over from all the damage!".to_owned(),
                });
                notices.extend(super::broadcast::observer_notices(
                    world,
                    id,
                    "falls down, staggered by the damage!",
                ));
                let resolve = if character && world.objects[&id].flags.contains(Flag::InCharacter) {
                    super::fall::resolve_character_fall
                } else {
                    super::resolve_fall
                };
                let fall = resolve(
                    world,
                    id,
                    1,
                    FallRules {
                        vehicle_impact: rules.vehicle_impact,
                        extended_piloting: rules.extended_piloting,
                        toughness,
                    },
                )?;
                fall.append_notices(id, &mut notices, &mut pilot_notices);
                Some(fall)
            };
            reports.push(StaggerReport {
                experience_messages,
                check_notice_index,
                unit: id,
                level,
                check,
                fall,
                notices,
                pilot_notices,
            });
        }
        Ok(reports)
    })
}
