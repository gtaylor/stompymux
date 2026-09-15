//! Runtime equipment conditions can be edited independently of material critical-slot losses.
use super::{BattleGyro, BattleSystem, BattleUnit};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// Administrative conditions shared by supported unit classes, with no raw status-word copy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct CriticalConditions {
    pub(super) light_probe_failure: Option<bool>,
    gyro: Option<GyroCondition>,
    /// Hardened gyro contribution after ordered critical and actuator recalculation events.
    #[serde(default)]
    gyro_piloting: Option<u8>,
}

/// Gyro state at an edit; subsequent material hits advance the same protection sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct GyroCondition {
    losses: u8,
    damage: u8,
    hardened_hit_used: bool,
}

impl CriticalConditions {
    /// Raw material replacement retains the preexisting secondary probe condition.
    pub(super) fn preserve_light_probe(&mut self, previous_failure: Option<bool>, available: bool) {
        let current_failure = !available;
        if let Some(failed) = previous_failure
            && failed != current_failure
        {
            self.light_probe_failure = Some(failed);
        }
    }

    /// A fresh probe hit overrides an administrative restoration on either chassis.
    pub(super) fn lose_light_probe(&mut self) {
        if let Some(failed) = &mut self.light_probe_failure {
            *failed = true;
        }
    }

    /// Vehicles have no gyro condition, and Mech corrections cannot predate removed losses.
    pub(super) fn validate(self, gyro_losses: Option<u8>) -> Result<()> {
        ensure!(
            self.gyro_piloting
                .is_none_or(|modifier| gyro_losses.is_some() && matches!(modifier, 0 | 2 | 3 | 5)),
            "Invalid hardened gyro piloting contribution"
        );
        if let Some(gyro) = self.gyro {
            ensure!(
                gyro_losses.is_some_and(|losses| gyro.losses <= losses) && gyro.damage <= 4,
                "Invalid gyro condition"
            );
        }
        Ok(())
    }
}

impl BattleUnit {
    /// Rebuild impairment from replacement material, retaining the independent protection flag.
    pub(super) fn reconstruct_gyro(&mut self, protection_used: bool) {
        self.critical_conditions.gyro = None;
        self.critical_conditions.gyro_piloting = None;
        self.recalculate_actuators();
        if self.gyro() == BattleGyro::Hardened {
            self.critical_conditions.gyro = Some(GyroCondition {
                losses: self.system_hits(BattleSystem::Gyro),
                damage: self.gyro_damage(),
                hardened_hit_used: protection_used,
            });
        }
    }

    /// Current gyro contribution preserves the order of hardened protection and recalculation.
    pub(super) fn gyro_piloting_modifier(&self) -> u8 {
        let derived = u8::from(self.gyro_damage() > 0) * 3;
        if self.gyro() == BattleGyro::Hardened {
            return self.critical_conditions.gyro_piloting.unwrap_or(derived);
        }
        derived
    }

    /// A first impairing hit adds three to the current contribution; protection and later hits do not.
    pub(super) fn record_gyro_critical(&mut self, previous_damage: u8, previous_modifier: u8) {
        if self.gyro() == BattleGyro::Hardened {
            self.critical_conditions.gyro_piloting = Some(
                previous_modifier
                    + if previous_damage == 0 && self.gyro_damage() > 0 {
                        3
                    } else {
                        0
                    },
            );
        }
    }

    /// Section loss or exposure recalculates every Mech location except a biped arm.
    pub(super) fn recalculate_section_loss(&mut self, section: super::BattleSection) {
        if self.chassis().is_leg(section)
            || !matches!(
                section,
                super::BattleSection::LeftArm | super::BattleSection::RightArm
            )
        {
            self.recalculate_actuators();
        }
    }

    /// Rebuild actuator speed and gyro piloting together at the shared damage reset points.
    pub(super) fn recalculate_actuators(&mut self) {
        self.propulsion.recalculate(self.template_speed());
        if self.gyro() == BattleGyro::Hardened {
            let (damage, protection_used) = self.gyro_condition();
            self.critical_conditions.gyro_piloting = Some(if !protection_used {
                0
            } else if damage > 0 {
                3
            } else {
                2
            });
        }
    }

    /// Current impairment and consumed hardened protection, including hits since an edit.
    pub(super) fn gyro_condition(&self) -> (u8, bool) {
        let losses = self.system_hits(BattleSystem::Gyro);
        if let Some(gyro) = self.critical_conditions.gyro {
            let added = losses.saturating_sub(gyro.losses);
            return (
                gyro.damage
                    .saturating_add(added.saturating_sub(u8::from(!gyro.hardened_hit_used))),
                gyro.hardened_hit_used || added > 0,
            );
        }
        let hardened = self.gyro() == BattleGyro::Hardened;
        (
            losses.saturating_sub(u8::from(hardened)),
            hardened && losses > 0,
        )
    }

    /// Identify the newly consumed protection hit without confusing it with existing impairment.
    pub(super) fn protected_gyro_hit(&self) -> bool {
        if self.gyro() != BattleGyro::Hardened {
            return false;
        }
        let losses = self.system_hits(BattleSystem::Gyro);
        self.critical_conditions.gyro.map_or(losses == 1, |gyro| {
            !gyro.hardened_hit_used && losses == gyro.losses.saturating_add(1)
        })
    }

    /// Change only the hardened first-hit condition; preserve current impairment and material.
    pub(super) fn set_hardened_hit_used(&mut self, used: bool) -> Result<()> {
        ensure!(
            !used || self.gyro() == BattleGyro::Hardened,
            "A hardened gyro is not installed"
        );
        let (damage, previous) = self.gyro_condition();
        if used != previous {
            self.critical_conditions.gyro = Some(GyroCondition {
                losses: self.system_hits(BattleSystem::Gyro),
                damage,
                hardened_hit_used: used,
            });
        }
        Ok(())
    }
}
