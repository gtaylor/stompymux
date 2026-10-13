//! Runtime equipment conditions can be edited independently of material critical-slot losses.
use super::{Gyro, Mech, System};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// Administrative conditions shared by supported unit classes, with no raw status-word copy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct CriticalConditions {
    pub(super) light_probe_failure: Option<bool>,
    gyro: Option<GyroCondition>,
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
        if let Some(gyro) = self.gyro {
            ensure!(
                gyro_losses.is_some_and(|losses| gyro.losses <= losses) && gyro.damage <= 4,
                "Invalid gyro condition"
            );
        }
        Ok(())
    }
}

impl Mech {
    /// Rebuild impairment from replacement material, retaining the independent protection flag.
    pub(super) fn reconstruct_gyro(&mut self, protection_used: bool) {
        self.critical_conditions.gyro = None;
        self.recalculate_actuators();
        if self.gyro() == Gyro::Hardened {
            self.critical_conditions.gyro = Some(GyroCondition {
                losses: self.system_hits(System::Gyro),
                damage: self.gyro_damage(),
                hardened_hit_used: protection_used,
            });
        }
    }

    /// Piloting Skill Roll modifier from gyro damage (Total Warfare p. 60 and 126): a hit gyro
    /// adds +3, and a destroyed gyro adds +6 to the check against MechWarrior damage in the
    /// automatic fall. A heavy-duty gyro's first critical hit only adds +1 (p. 126).
    pub(super) fn gyro_piloting_modifier(&self) -> u8 {
        let (damage, protection_used) = self.gyro_condition();
        match damage {
            0 => u8::from(protection_used),
            1 => 3,
            _ => 6,
        }
    }

    /// Section loss or exposure recalculates every Mech location except a biped arm.
    pub(super) fn recalculate_section_loss(&mut self, section: super::MechSection) {
        if self.chassis().is_leg(section)
            || !matches!(
                section,
                super::MechSection::LeftArm | super::MechSection::RightArm
            )
        {
            self.recalculate_actuators();
        }
    }

    /// Rebuild actuator speed at the shared damage reset points.
    pub(super) fn recalculate_actuators(&mut self) {
        self.propulsion.recalculate(self.template_speed());
    }

    /// Current impairment and consumed hardened protection, including hits since an edit.
    pub(super) fn gyro_condition(&self) -> (u8, bool) {
        let losses = self.system_hits(System::Gyro);
        if let Some(gyro) = self.critical_conditions.gyro {
            let added = losses.saturating_sub(gyro.losses);
            return (
                gyro.damage
                    .saturating_add(added.saturating_sub(u8::from(!gyro.hardened_hit_used))),
                gyro.hardened_hit_used || added > 0,
            );
        }
        let hardened = self.gyro() == Gyro::Hardened;
        (
            losses.saturating_sub(u8::from(hardened)),
            hardened && losses > 0,
        )
    }

    /// Identify the newly consumed protection hit without confusing it with existing impairment.
    pub(super) fn protected_gyro_hit(&self) -> bool {
        if self.gyro() != Gyro::Hardened {
            return false;
        }
        let losses = self.system_hits(System::Gyro);
        self.critical_conditions.gyro.map_or(losses == 1, |gyro| {
            !gyro.hardened_hit_used && losses == gyro.losses.saturating_add(1)
        })
    }

    /// Change only the hardened first-hit condition; preserve current impairment and material.
    pub(super) fn set_hardened_hit_used(&mut self, used: bool) -> Result<()> {
        ensure!(
            !used || self.gyro() == Gyro::Hardened,
            "A hardened gyro is not installed"
        );
        let (damage, previous) = self.gyro_condition();
        if used != previous {
            self.critical_conditions.gyro = Some(GyroCondition {
                losses: self.system_hits(System::Gyro),
                damage,
                hardened_hit_used: used,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::{CriticalLocation, Mech, MechTemplate, System};

    /// The test Jenner, optionally with a heavy-duty gyro.
    fn jenner(hardened: bool) -> Mech {
        let mut template = MechTemplate::parse(
            "JR7-D",
            include_str!("../../tests/fixtures/btech/units/JR7-D.toml"),
        )
        .unwrap();
        if hardened {
            template
                .attributes
                .entry("specials".into())
                .or_default()
                .push_str(" HDGYRO");
        }
        Mech::from_template(template).unwrap()
    }

    /// Standard gyro: +3 after one hit, +6 once destroyed. Heavy-duty gyro: +1, then +3, then +6.
    #[test]
    fn gyro_piloting_modifiers_follow_total_warfare() {
        for (hardened, expected) in [(false, [3, 6, 6]), (true, [1, 3, 6])] {
            let mut unit = jenner(hardened);
            let gyro: Vec<CriticalLocation> = unit
                .loadout()
                .unwrap()
                .systems
                .iter()
                .filter(|part| part.system == System::Gyro)
                .map(|part| part.location)
                .collect();
            assert_eq!(unit.gyro_piloting_modifier(), 0);
            for (slot, modifier) in gyro.iter().zip(expected) {
                unit.destroy_critical(*slot).unwrap();
                assert_eq!(unit.gyro_piloting_modifier(), modifier, "{hardened}");
            }
        }
    }
}
