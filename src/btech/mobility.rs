//! Derive BattleMech mobility from persistent section and actuator damage.
use super::{BattleMechChassis, BattleSystem, BattleUnit, CriticalLocation};
use serde::Serialize;

/// Damage-only movement limits; heat, cargo, terrain and pilot advantages are applied separately.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BattleMobility {
    pub maximum_speed: f64,
    pub piloting_modifier: u8,
}

impl BattleUnit {
    /// Recompute mobility from facts instead of accumulating mutable penalties after each hit.
    pub fn mobility(&self) -> BattleMobility {
        // Hardened armor costs one running MP before damage adjusts the ceiling.
        let baseline = (self.propulsion.baseline(self.definition().max_speed)
            - self.hardened_speed_penalty())
        .max(0.0);
        let chassis = self.chassis();
        let legs = chassis.legs();
        let missing = self.unavailable_legs();
        let gyro = self.gyro_damage();
        let (mut maximum_speed, leg_penalty) = match (chassis, missing) {
            (_, 0) => (baseline, 0),
            (BattleMechChassis::Quad, 1) => (baseline - 10.75, 2),
            (BattleMechChassis::Quad, 2) => (10.75, 7),
            (BattleMechChassis::Quad, _) => (0.0, 2),
            (BattleMechChassis::Biped, 1) => (10.75, 5),
            (BattleMechChassis::Biped, _) => (0.0, 10),
        };
        let mut piloting_modifier = leg_penalty + self.gyro_piloting_modifier();
        let mut hips = 0;
        for &section in legs {
            if self.leg_unavailable(section) {
                continue;
            }
            let damaged: Vec<_> = self.definition().sections[&section]
                .criticals
                .iter()
                .filter(|(slot, _)| {
                    self.critical_destroyed(CriticalLocation {
                        section,
                        slot: **slot,
                    })
                })
                .filter_map(|(_, part)| BattleSystem::named(&part.equipment))
                .collect();
            if damaged.contains(&BattleSystem::ShoulderOrHip) {
                hips += 1;
                maximum_speed /= 2.0;
                piloting_modifier += 2;
                continue;
            }
            let actuators = damaged
                .iter()
                .filter(|system| {
                    matches!(
                        system,
                        BattleSystem::UpperActuator
                            | BattleSystem::LowerActuator
                            | BattleSystem::HandOrFootActuator
                    )
                })
                .count() as u8;
            maximum_speed -= f64::from(actuators) * 10.75;
            piloting_modifier += actuators;
        }
        maximum_speed = self.propulsion.maximum(maximum_speed);
        if (chassis == BattleMechChassis::Biped && hips == 2)
            || missing
                >= if chassis == BattleMechChassis::Quad {
                    3
                } else {
                    2
                }
            || gyro >= 2
            || self.is_destroyed()
            || self.masc_seized()
        {
            maximum_speed = 0.0;
        }
        BattleMobility {
            maximum_speed: maximum_speed.max(0.0),
            piloting_modifier,
        }
    }
}
