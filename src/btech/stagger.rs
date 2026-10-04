//! Durable damage windows and transactional piloting checks for biped staggering.
use super::{FallRules, Mech, MechFallReport, PilotingCheck, Posture, Power};
use crate::{Flag, ObjectId, World};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// Traditional per-turn checks or the configured rolling sixty-second damage window.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StaggerMode {
    Traditional,
    #[default]
    Retain,
    Consume,
}

impl StaggerMode {
    /// Configuration zero selects traditional rules, two consumes history, other values retain it.
    pub fn from_setting(value: i64) -> Self {
        match value {
            0 => Self::Traditional,
            2 => Self::Consume,
            _ => Self::Retain,
        }
    }
}

/// One incoming damage group; whole groups are marked or consumed, including threshold overshoot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaggerHit {
    pub damage: u16,
    pub remaining: u8,
    pub counted: bool,
}

/// Saved unit-local cadence and incoming damage; fall damage and internal explosions are excluded.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stagger {
    /// Restored action-time damage scalar; ordinary hits update history independently.
    #[serde(default)]
    pub action_damage: i32,
    pub hits: Vec<StaggerHit>,
    pub elapsed: u64,
    pub turn_damage: u32,
    pub phase: u8,
    pub checked_phase: Option<u8>,
}

impl Stagger {
    /// Positive twenty-point levels govern action-time checks, independently of rolling history.
    pub fn action_level(&self) -> i32 {
        (self.action_damage / 20).max(0)
    }

    /// Reject malformed windows and bound the work and serialized state of damage accumulation.
    pub(super) fn validate(&self) -> Result<()> {
        ensure!(
            self.phase < 30 && self.checked_phase.is_none_or(|phase| phase < 30),
            "Invalid stagger phase"
        );
        ensure!(
            self.hits.len() <= 4096
                && self
                    .hits
                    .iter()
                    .all(|hit| hit.damage > 0 && (1..=60).contains(&hit.remaining)),
            "Invalid stagger history"
        );
        ensure!(
            self.hits
                .windows(2)
                .all(|pair| pair[0].remaining <= pair[1].remaining),
            "Unordered stagger history"
        );
        Ok(())
    }

    /// Incoming ordinary damage counts once, before criticals or transfer change the unit.
    pub(super) fn record(&mut self, damage: u16, mode: StaggerMode) -> Result<()> {
        if damage == 0 {
            return Ok(());
        }
        if mode == StaggerMode::Traditional {
            self.hits.clear();
            self.elapsed = 0;
            self.turn_damage = self.turn_damage.saturating_add(u32::from(damage));
            return Ok(());
        }
        ensure!(self.hits.len() < 4096, "Stagger history exceeds work limit");
        self.turn_damage = 0;
        self.checked_phase = None;
        self.hits.push(StaggerHit {
            damage,
            remaining: 60,
            counted: false,
        });
        Ok(())
    }

    /// A fall removes accumulated damage without changing the unit's per-turn phase.
    pub(super) fn clear_damage(&mut self) {
        self.hits.clear();
        self.turn_damage = 0;
        self.elapsed = 0;
    }
}

impl Mech {
    /// Saved damage and cadence used by stagger checks.
    pub fn stagger(&self) -> &Stagger {
        &self.stagger
    }

    /// Traditional running units keep their phase even between hits; rolling windows age while present.
    pub fn stagger_active(&self, mode: StaggerMode) -> bool {
        !self.is_destroyed()
            && (!self.stagger.hits.is_empty()
                || self.stagger.turn_damage > 0
                || (mode == StaggerMode::Traditional && self.power() == Power::Running))
    }
}

/// Reference weight-class adjustment shared by rolling and action-time checks.
fn tonnage_modifier(tons: u16) -> i32 {
    match tons {
        0..=35 => 1,
        36..=55 => 0,
        56..=75 => -1,
        _ => -2,
    }
}

/// Action-time checks always apply tonnage, independently of the rolling-window setting.
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
    pub mode: StaggerMode,
    pub interval: u64,
    pub tonnage: bool,
    pub hit: super::HitRules,
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
                unit.stagger_active(rules.mode)
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
            let conscious_off = unit.power() != Power::Running
                && unit
                    .pilot()
                    .is_none_or(|pilot| !world.btech.unconscious(pilot));
            let prone = unit.posture() == Posture::Prone;
            let airborne = unit.airborne();
            let tons = unit.definition().tons;
            let unit = world.btech.constructed.get_mut(&id).unwrap();
            let history = &mut unit.stagger;
            for hit in &mut history.hits {
                hit.remaining -= 1;
            }
            history.hits.retain(|hit| hit.remaining > 0);
            let level = if rules.mode == StaggerMode::Traditional {
                if conscious_off {
                    continue;
                }
                history.phase = (history.phase + 1) % 30;
                if history.turn_damage >= 20
                    && history
                        .checked_phase
                        .is_none_or(|phase| phase == history.phase)
                {
                    history.turn_damage = 0;
                    history.checked_phase = Some(history.phase);
                    if prone || airborne {
                        continue;
                    }
                    1
                } else {
                    if history.checked_phase == Some(history.phase)
                        || (history.checked_phase.is_none() && history.phase == 0)
                    {
                        history.turn_damage = 0;
                        history.checked_phase = None;
                    }
                    continue;
                }
            } else {
                history.turn_damage = 0;
                history.checked_phase = None;
                history.elapsed = history.elapsed.saturating_add(1);
                if history.elapsed < rules.interval.max(1) {
                    continue;
                }
                history.elapsed = 0;
                let fresh: u32 = history
                    .hits
                    .iter()
                    .filter(|hit| !hit.counted)
                    .map(|hit| u32::from(hit.damage))
                    .sum();
                if fresh < 20 {
                    continue;
                }
                let counted: u32 = history
                    .hits
                    .iter()
                    .filter(|hit| hit.counted)
                    .map(|hit| u32::from(hit.damage))
                    .sum();
                let mut remaining = fresh / 20 * 20;
                for hit in &mut history.hits {
                    if remaining == 0 {
                        break;
                    }
                    if hit.counted {
                        continue;
                    }
                    remaining = remaining.saturating_sub(u32::from(hit.damage));
                    hit.counted = true;
                }
                if rules.mode == StaggerMode::Consume {
                    history.hits.retain(|hit| !hit.counted);
                    fresh / 20
                } else {
                    (fresh + counted) / 20
                }
            };
            let tonnage = i64::from(tonnage_modifier(tons));
            let modifier = if world.btech.constructed_units()[&id].power() != Power::Running {
                999
            } else if rules.mode == StaggerMode::Traditional {
                1
            } else {
                i64::from(level) - 1 + if rules.tonnage { tonnage } else { 0 }
            };
            let mut notices = Vec::new();
            if rules.mode != StaggerMode::Traditional {
                let (own, observed) = match level {
                    1 => (
                        "The damage causes you to stagger a little.",
                        "stumbles slightly!",
                    ),
                    2 => (
                        "The damage causes you to stagger even more!",
                        "starts to stagger from the damage!",
                    ),
                    _ => (
                        "The damage causes you to stagger violently while attempting to keep your footing!",
                        "staggers back and forth attempting to keep its footing!",
                    ),
                };
                notices.push(super::Notice {
                    unit: id,
                    text: own.to_owned(),
                });
                notices.extend(super::broadcast::observer_notices(world, id, observed));
            }
            notices.push(super::Notice {
                unit: id,
                text: "You stagger from the damage!".to_owned(),
            });
            let mut check = super::roll_piloting(
                world,
                id,
                modifier.clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16,
                rules.extended_piloting,
            )?;
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
                    if rules.mode == StaggerMode::Traditional {
                        "falls down, staggered by the damage!"
                    } else {
                        "tumbles over, staggered by the damage!"
                    },
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
                        stacking: crate::StackingRules::STANDARD,
                        hit: rules.hit,
                        extended_piloting: rules.extended_piloting,
                        stagger: rules.mode,
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
