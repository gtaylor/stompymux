//! Durable stand attempts and retry delays, including water and ice fall consequences.
use super::{
    BattleFallReport, BattleFallRules, BattleNotice, BattlePilotingCheck, BattlePosture,
    BattlePower,
};
use crate::{Flag, ObjectId, World};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// Successful rises are upright but movement-locked; failed attempts recover while prone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum BattleStandTimer {
    Rising { remaining: u8 },
    Recovering { remaining: u8 },
}
impl BattleStandTimer {
    /// Committed seconds until movement or another attempt is allowed.
    pub fn remaining(self) -> u8 {
        match self {
            Self::Rising { remaining } | Self::Recovering { remaining } => remaining,
        }
    }
}

/// Normal refuses impossible base targets; anyway overrides that refusal; careful uses -2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleStandMode {
    Normal,
    Anyway,
    Careful,
}

#[derive(Clone, Copy)]
enum StandActor {
    Player(ObjectId),
    Autopilot,
}

impl StandActor {
    fn pilot(self) -> Option<ObjectId> {
        match self {
            Self::Player(pilot) => Some(pilot),
            Self::Autopilot => None,
        }
    }
}

/// Fully resolved attempt; the caller stages its fall/injury notices with the world commit.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[must_use = "Stage attempt and fall notices before committing"]
pub struct BattleStandAttempt {
    pub check: BattlePilotingCheck,
    pub fall: Option<BattleFallReport>,
    pub timer: Option<BattleStandTimer>,
    /// Ordered observer, cockpit and damage feedback captured by the attempt transaction.
    pub notices: Vec<BattleNotice>,
    /// Captured private roll feedback interleaved before stand/fall consequences.
    pub pilot_notices: Vec<super::BattlePilotNotice>,
}

impl super::BattleUnit {
    /// Current rise/retry countdown, if any.
    pub fn stand_timer(&self) -> Option<BattleStandTimer> {
        self.stand_timer
    }
}

/// Validate local pilot and physical support, returning the unmodified stand target without dice.
pub fn stand_target(world: &World, id: ObjectId, pilot: ObjectId, extended: bool) -> Result<i32> {
    stand_target_by_actor(world, id, StandActor::Player(pilot), extended)
}

fn stand_target_by_actor(
    world: &World,
    id: ObjectId,
    actor: StandActor,
    extended: bool,
) -> Result<i32> {
    match actor {
        StandActor::Player(pilot) => super::power::controlled_unit(world, id, pilot)?,
        StandActor::Autopilot => {
            super::power::autopilot_controlled_unit(world, id)?;
        }
    }
    let unit = &world.btech.constructed_units()[&id];
    ensure!(unit.power() == BattlePower::Running, "Start the unit first");
    unit.hull_down.require_mobile()?;
    ensure!(!unit.airborne(), "Land before trying to stand");
    ensure!(
        unit.posture() == BattlePosture::Prone,
        "Unit is already standing"
    );
    ensure!(
        unit.stand_timer.is_none(),
        "Still recovering from the last stand attempt"
    );
    unit.stand_requires_roll()?;
    Ok(
        i32::from(super::skills::control_target(world, id, extended)?)
            + i32::from(unit.mobility().piloting_modifier)
            + i32::from(unit.cockpit_piloting_modifier()),
    )
}

/// Begin a ground or water attempt atomically, including fall damage if the no-XP skill check fails.
pub fn begin_stand(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    mode: BattleStandMode,
    careful_enabled: bool,
    rules: BattleFallRules,
) -> Result<BattleStandAttempt> {
    begin_stand_inner(
        world,
        id,
        StandActor::Player(pilot),
        mode,
        careful_enabled,
        rules,
        false,
    )
}

/// Begin a normal standing attempt for an attached ground autopilot.  The
/// controller supplies only cockpit authority; power, mobility, blindness,
/// damage, and the ordinary no-pilot skill fallback remain in force.
pub(crate) fn begin_stand_autopilot(
    world: &mut World,
    id: ObjectId,
    mode: BattleStandMode,
    careful_enabled: bool,
    rules: BattleFallRules,
) -> Result<BattleStandAttempt> {
    begin_stand_inner(
        world,
        id,
        StandActor::Autopilot,
        mode,
        careful_enabled,
        rules,
        false,
    )
}

/// Resolve a stand attempt inside a host action that publishes character consequences.
pub(super) fn begin_stand_in_action(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    mode: BattleStandMode,
    careful_enabled: bool,
    rules: BattleFallRules,
) -> Result<BattleStandAttempt> {
    begin_stand_inner(
        world,
        id,
        StandActor::Player(pilot),
        mode,
        careful_enabled,
        rules,
        true,
    )
}

/// Shared stand eligibility, control roll, fall and recovery scheduling.
fn begin_stand_inner(
    world: &mut World,
    id: ObjectId,
    actor: StandActor,
    mode: BattleStandMode,
    careful_enabled: bool,
    rules: BattleFallRules,
    character: bool,
) -> Result<BattleStandAttempt> {
    let target = stand_target_by_actor(world, id, actor, rules.extended_piloting)?;
    ensure!(
        mode != BattleStandMode::Careful || careful_enabled,
        "Careful standing is disabled"
    );
    ensure!(
        mode == BattleStandMode::Anyway || target <= 12,
        "You would fail; use stand anyway"
    );
    ensure!(
        character || !world.objects[&id].flags.contains(Flag::InCharacter),
        "Standing requires tactical casualty rules"
    );
    world.attempt(|world| {
        let mut notices = super::broadcast::observer_notices(world, id, "attempts to stand up.");
        let mut pilot_notices = Vec::new();
        world.btech.constructed.get_mut(&id).unwrap().posture = BattlePosture::Standing;
        let check = super::piloting::roll_standing(
            world,
            id,
            if mode == BattleStandMode::Careful {
                -2
            } else {
                0
            },
            rules.extended_piloting,
        )?;
        super::piloting::capture_feedback(
            id,
            actor.pilot(),
            &check,
            &mut notices,
            &mut pilot_notices,
        );
        notices.push(BattleNotice {
            unit: id,
            text: if check.success {
                "You begin to stand up."
            } else {
                "You fail your attempt to stand and fall back on the ground."
            }
            .to_owned(),
        });
        let fall = if check.success {
            None
        } else {
            notices.extend(super::broadcast::observer_notices(world, id, "falls down!"));
            let fall = if character && world.objects[&id].flags.contains(Flag::InCharacter) {
                super::fall::resolve_character_fall(world, id, 1, rules)?
            } else {
                super::resolve_fall(world, id, 1, rules)?
            };
            fall.append_notices(id, &mut notices, &mut pilot_notices);
            Some(fall)
        };
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        let timer = if unit.is_destroyed() {
            None
        } else {
            let base =
                (30.0 / (unit.movement_maximum_speed() / 21.5).clamp(1.0, 30.0)).floor() as u8;
            let remaining = if mode == BattleStandMode::Careful {
                if check.success {
                    base * 2
                } else {
                    (base * 2).max(30)
                }
            } else {
                base
            };
            Some(if check.success {
                BattleStandTimer::Rising { remaining }
            } else {
                BattleStandTimer::Recovering { remaining }
            })
        };
        unit.stand_timer = timer;
        Ok(BattleStandAttempt {
            check,
            fall,
            timer,
            notices,
            pilot_notices,
        })
    })
}

/// Finish stand/retry events on committed seconds, including while shut down.
pub fn advance_standing(world: &mut World) -> Vec<BattleNotice> {
    let mut notices = Vec::new();
    let pending: Vec<_> = world
        .btech
        .constructed_units()
        .iter()
        .filter_map(|(&id, unit)| unit.stand_timer.map(|timer| (id, timer)))
        .collect();
    for (id, timer) in pending {
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        let remaining = timer.remaining() - 1;
        if remaining > 0 {
            unit.stand_timer = Some(match timer {
                BattleStandTimer::Rising { .. } => BattleStandTimer::Rising { remaining },
                BattleStandTimer::Recovering { .. } => BattleStandTimer::Recovering { remaining },
            });
            continue;
        }
        unit.stand_timer = None;
        if matches!(timer, BattleStandTimer::Rising { .. }) {
            notices.extend(
                super::observer_messages(world, id, "stands up!")
                    .into_iter()
                    .map(|(unit, text)| BattleNotice { unit, text }),
            );
        }

        notices.push(BattleNotice {
            unit: id,
            text: (match timer {
                BattleStandTimer::Rising { .. } => "You have finally finished standing up.",
                BattleStandTimer::Recovering { .. } => {
                    "You have finally recovered from your attempt to stand."
                }
            })
            .to_owned(),
        });
    }
    notices
}

/// Apply the same configured stand and casualty rules for native and Lua callers.
pub(crate) fn configured_stand(
    scripts: &crate::Scripts,
    config: &crate::Config,
    id: ObjectId,
    pilot: ObjectId,
    mode: BattleStandMode,
) -> Result<BattleStandAttempt> {
    let settings = &config.battletech;
    let toughness = scripts
        .world
        .borrow()
        .btech
        .character_values()
        .get(&pilot)
        .is_some_and(|values| super::advantages::enabled(values, "Toughness"));
    super::evacuation::stand_action(
        scripts,
        config,
        id,
        pilot,
        mode,
        settings.standcareful != 0,
        BattleFallRules {
            vehicle_impact: crate::BattleVehicleImpactRules::configured(settings, false),
            stacking: crate::BattleStackingRules {
                mode: settings.stacking,
                damage_percent: settings.stackdamage,
                hit_arcs: settings.hit_arcs,
            },
            stagger: super::BattleStaggerMode::from_setting(settings.newstagger),
            hit: super::BattleHitRules {
                inferno_penalty: settings.inferno_penalty != 0,
                exile_stun_mode: settings.exile_stun_code.clamp(0, 2) as u8,
            },
            extended_piloting: settings.extended_piloting != 0,
            toughness,
        },
    )
}
