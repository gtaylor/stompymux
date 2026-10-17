//! Immediate ground and airborne balance consequences of BattleMech damage.
use super::{
    CriticalLocation, FallRules, MechChassis, MechFallReport, MechSection, PilotingCheck, Posture,
    System,
};
use crate::{ObjectId, World};
use anyhow::Result;
use serde::Serialize;

/// Damage event evaluated with the unit's condition at that exact point in the cascade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum BalanceCause {
    Critical {
        location: CriticalLocation,
        system: System,
    },
    SectionLost(MechSection),
}

/// A balance attempt or forced fall, including its separately rolled pilot protection check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BalanceReport {
    /// Assigned pilot captured before a fall can change crew state.
    pub pilot: Option<ObjectId>,
    pub cause: BalanceCause,
    /// Forced falls do not roll to remain upright.
    pub check: Option<PilotingCheck>,
    /// Accepted balance-check XP captured before any subsequent fall.
    pub experience_messages: Vec<super::DiagnosticMessage>,
    pub fall: Option<MechFallReport>,
    /// Observer feedback captured before the fall changes posture, visibility or power.
    pub observer_notices: Vec<super::Notice>,
    /// Impacts caused by landing among other units after this balance failure.
    pub collision_impacts: Vec<super::TacticalImpact>,
    /// Additional avoidance falls caused by the landing collision.
    pub collision_falls: Vec<MechFallReport>,
}

/// Apply a newly recorded loss exactly once; the enclosing damage action owns rollback.
pub(super) fn resolve_balance(
    world: &mut World,
    id: ObjectId,
    cause: BalanceCause,
    rules: FallRules,
) -> Result<Option<BalanceReport>> {
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
        BalanceCause::Critical {
            system: System::Gyro,
            ..
        }
    );
    if gyro && unit.protected_gyro_hit() {
        return Ok(None);
    }
    let thrust_loss = airborne
        && matches!(
            cause,
            BalanceCause::Critical {
                system: System::JumpJet,
                ..
            } | BalanceCause::SectionLost(_)
        )
        && unit.jump_capacity(100)?.speed < 10.75;
    if unit.posture() == Posture::Prone && !thrust_loss {
        return Ok(None);
    }
    if airborne && !gyro && !thrust_loss {
        if unit.airborne_support_lost() {
            // Structural collapse changes posture while the jets continue the same trajectory.
            let unit = world.btech.constructed.get_mut(&id).unwrap();
            unit.hull_down = Default::default();
            unit.posture = Posture::Prone;
            unit.facing = Default::default();
        }
        return Ok(None);
    }
    // A jumping 'Mech that falls on a failed Piloting Skill Roll is considered to have fallen
    // zero levels (Total Warfare p. 68), so an airborne gyro hit falls like a ground fall: the
    // fall multiplier is one either way.
    let forced = if thrust_loss {
        true
    } else {
        match cause {
            BalanceCause::SectionLost(section) if leg(section) => true,
            BalanceCause::Critical {
                system: System::Gyro,
                ..
            } => match unit.gyro_damage() {
                1 => false,
                2 => true,
                _ => return Ok(None),
            },
            BalanceCause::Critical { location, system } if leg(location.section) => match system {
                System::ShoulderOrHip => false,
                System::UpperActuator | System::LowerActuator | System::HandOrFootActuator => {
                    // A biped's frozen hip makes later hits in that leg moot (Total Warfare
                    // p. 127); a quad's leg damage is cumulative, so each still takes its roll
                    // (TW p. 125).
                    let hip_lost = unit.loadout()?.systems.iter().any(|part| {
                        part.location.section == location.section
                            && part.system == System::ShoulderOrHip
                            && unit.critical_unavailable(part.location)
                    });
                    if hip_lost && chassis == MechChassis::Biped {
                        return Ok(None);
                    }
                    false
                }
                _ => return Ok(None),
            },
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
        } else if matches!(cause, BalanceCause::SectionLost(_)) {
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
                super::fall::resolve_character_fall(world, id, 1, rules)?
            } else {
                super::resolve_fall(world, id, 1, rules)?
            },
        )
    } else {
        None
    };
    Ok(Some(BalanceReport {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A running unit of `source` with its rear-left hip destroyed, on a small clear map.
    fn hipless(source: &str) -> (World, ObjectId) {
        let config = crate::Config::load("tests/fixtures/game").unwrap();
        let mut world = World::default();
        let id = world.create(&config, "Mech".into(), crate::Kind::Thing);
        let map = world.create(&config, "Map".into(), crate::Kind::Room);
        super::super::create_unit(
            &mut world,
            id,
            crate::MechTemplate::parse("test", source).unwrap(),
        )
        .unwrap();
        super::super::create_map(
            &mut world,
            map,
            "test",
            super::super::MapAsset::from_cells("2 2\n.0.0\n.0.0\n").unwrap(),
        )
        .unwrap();
        super::super::place_unit(&mut world, id, map, 0, 0).unwrap();
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        unit.power = super::super::Power::Running;
        unit.dice = super::super::Dice::seeded([0; 32]);
        unit.lost_criticals.insert(CriticalLocation {
            section: MechSection::LeftLeg,
            slot: 0,
        });
        (world, id)
    }

    /// A biped's frozen hip makes a later actuator hit in the same leg moot, but a quad's leg
    /// damage is cumulative, so the hit still takes its roll (Total Warfare pp. 125, 127).
    #[test]
    fn quad_leg_actuator_hits_roll_after_a_lost_hip() {
        for (source, rolls) in [
            (include_str!("../../game/units/SCP-1N.toml"), true),
            (
                include_str!("../../tests/fixtures/btech/units/JR7-D.toml"),
                false,
            ),
        ] {
            let (mut world, id) = hipless(source);
            let location = CriticalLocation {
                section: MechSection::LeftLeg,
                slot: 1,
            };
            world
                .btech
                .constructed
                .get_mut(&id)
                .unwrap()
                .lost_criticals
                .insert(location);
            let cause = BalanceCause::Critical {
                location,
                system: System::UpperActuator,
            };
            let report = resolve_balance(&mut world, id, cause, FallRules::STANDARD).unwrap();
            assert_eq!(report.is_some(), rolls);
            if let Some(report) = report {
                assert!(report.check.is_some());
            }
        }
    }
}
