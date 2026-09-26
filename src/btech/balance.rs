//! Immediate ground and airborne balance consequences of BattleMech damage.
use super::{
    BattleFallReport, BattleFallRules, BattlePilotingCheck, BattlePosture, BattleSection,
    BattleSystem, CriticalLocation,
};
use crate::{ObjectId, World};
use anyhow::Result;
use serde::Serialize;

/// Damage event evaluated with the unit's condition at that exact point in the cascade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum BattleBalanceCause {
    Critical {
        location: CriticalLocation,
        system: BattleSystem,
    },
    SectionLost(BattleSection),
}

/// A balance attempt or forced fall, including its separately rolled pilot protection check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleBalanceReport {
    /// Assigned pilot captured before a fall can change crew state.
    pub pilot: Option<ObjectId>,
    pub cause: BattleBalanceCause,
    /// Forced falls do not roll to remain upright.
    pub check: Option<BattlePilotingCheck>,
    /// Accepted balance-check XP captured before any subsequent fall.
    pub experience_messages: Vec<super::BattleChannelMessage>,
    pub fall: Option<BattleFallReport>,
    /// Observer feedback captured before the fall changes posture, visibility or power.
    pub observer_notices: Vec<super::BattleNotice>,
    /// Impacts caused by landing among other units after this balance failure.
    pub collision_impacts: Vec<super::BattleTacticalImpact>,
    /// Additional avoidance falls caused by the landing collision.
    pub collision_falls: Vec<BattleFallReport>,
}

/// Apply a newly recorded loss exactly once; the enclosing damage action owns rollback.
pub(super) fn resolve_balance(
    world: &mut World,
    id: ObjectId,
    cause: BattleBalanceCause,
    rules: BattleFallRules,
) -> Result<Option<BattleBalanceReport>> {
    let unit = &world.btech.constructed_units()[&id];
    if unit.is_destroyed() {
        return Ok(None);
    }
    let pilot = unit.pilot();
    let chassis = unit.chassis();
    let leg = |section| chassis.is_leg(section);
    let airborne = unit.airborne();
    let gyro = matches!(
        cause,
        BattleBalanceCause::Critical {
            system: BattleSystem::Gyro,
            ..
        }
    );
    if gyro && unit.protected_gyro_hit() {
        return Ok(None);
    }
    let thrust_loss = airborne
        && matches!(
            cause,
            BattleBalanceCause::Critical {
                system: BattleSystem::JumpJet,
                ..
            } | BattleBalanceCause::SectionLost(_)
        )
        && unit.jump_capacity(100)?.speed < 10.75;
    if unit.posture() == BattlePosture::Prone && !thrust_loss {
        return Ok(None);
    }
    if airborne && !gyro && !thrust_loss {
        if unit.airborne_support_lost() {
            // Structural collapse changes posture while the jets continue the same trajectory.
            let unit = world.btech.constructed.get_mut(&id).unwrap();
            unit.hull_down = Default::default();
            unit.posture = BattlePosture::Prone;
            unit.facing = Default::default();
            if rules.stagger != super::BattleStaggerMode::Traditional {
                unit.stagger.clear_damage();
            }
        }
        return Ok(None);
    }
    let levels = if airborne && gyro {
        let map = &world.btech.maps()[&unit.position().expect("validated airborne position").map];
        u8::try_from(unit.jump_capacity(map.gravity)?.movement_points)?
    } else {
        1
    };
    let forced = if thrust_loss {
        true
    } else {
        match cause {
            BattleBalanceCause::SectionLost(section) if leg(section) => true,
            BattleBalanceCause::Critical {
                system: BattleSystem::Gyro,
                ..
            } => match unit.gyro_damage() {
                1 => false,
                2 => true,
                _ => return Ok(None),
            },
            BattleBalanceCause::Critical { location, system } if leg(location.section) => {
                match system {
                    BattleSystem::ShoulderOrHip => false,
                    BattleSystem::UpperActuator
                    | BattleSystem::LowerActuator
                    | BattleSystem::HandOrFootActuator => {
                        let hip_lost = unit.loadout()?.systems.iter().any(|part| {
                            part.location.section == location.section
                                && part.system == BattleSystem::ShoulderOrHip
                                && unit.critical_unavailable(part.location)
                        });
                        if hip_lost {
                            return Ok(None);
                        }
                        false
                    }
                    _ => return Ok(None),
                }
            }
            _ => return Ok(None),
        }
    };
    let mut check = if forced {
        None
    } else {
        Some(super::roll_piloting(world, id, 0, rules.extended_piloting)?)
    };
    let experience_messages = if let Some(check) = &mut check {
        super::piloting::award_control_check(world, id, check, rules.extended_piloting)?
            .into_iter()
            .collect()
    } else {
        Vec::new()
    };
    let mut observer_notices = Vec::new();
    let fall = if check.is_none_or(|check| !check.success) {
        let text = if airborne {
            "falls from the sky!"
        } else if matches!(cause, BattleBalanceCause::SectionLost(_)) {
            "crashes to the ground!"
        } else if gyro && forced {
            "is knocked over!"
        } else if gyro {
            "stumbles and falls down."
        } else {
            "stumbles and falls down!"
        };
        observer_notices = super::broadcast::observer_notices(world, id, text);
        Some(
            if world.objects[&id].flags.contains(crate::Flag::InCharacter) {
                super::fall::resolve_character_fall(world, id, levels, rules)?
            } else if levels == 0 {
                super::fall::resolve_zero_fall(world, id, rules)?
            } else {
                super::resolve_fall(world, id, levels, rules)?
            },
        )
    } else {
        None
    };
    Ok(Some(BattleBalanceReport {
        experience_messages,
        pilot,
        cause,
        check,
        fall,
        observer_notices,
        collision_impacts: Vec::new(),
        collision_falls: Vec::new(),
    }))
}
