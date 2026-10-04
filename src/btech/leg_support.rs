//! Chassis-aware support for standing and landing, derived from current limb damage.
use super::{CriticalLocation, Mech, MechChassis, System};
use anyhow::{Result, ensure};

impl Mech {
    /// Destroyed or flooded load-bearing sections, including both front legs on quads.
    pub fn unavailable_legs(&self) -> usize {
        self.chassis()
            .legs()
            .iter()
            .filter(|leg| self.leg_unavailable(**leg))
            .count()
    }

    /// Charge selection permits one unavailable quad leg; bipeds need both legs.
    pub fn validate_charge_support(&self) -> Result<()> {
        let missing = self.unavailable_legs();
        if self.chassis() == MechChassis::Biped {
            ensure!(missing == 0, "With one leg? Are you kidding?");
            return Ok(());
        }
        ensure!(
            missing <= 1,
            "It'd unbalance you too much in your condition.."
        );
        Ok(())
    }

    /// Validate physical support and return whether standing needs a control roll.
    /// Pilot, power, posture and retry timers remain the stand action's responsibility.
    pub fn stand_requires_roll(&self) -> Result<bool> {
        let chassis = self.chassis();
        let missing = self.unavailable_legs();
        ensure!(missing < chassis.legs().len(), "No legs to stand on");
        ensure!(missing <= 2, "You'd be far too unstable!");
        ensure!(self.gyro_damage() < 2, "Cannot stand with a destroyed gyro");
        Ok(chassis != MechChassis::Quad || missing != 0)
    }

    /// Damage that prevents an upright landing without ending jump thrust.
    /// Quads lose support with three unavailable legs; repeated hip hits alone do not force it.
    pub fn airborne_support_lost(&self) -> bool {
        let chassis = self.chassis();
        if self.masc_seized() {
            return true;
        }
        if chassis == MechChassis::Quad {
            return self.unavailable_legs() >= 3;
        }
        let legs = chassis.legs();
        if self.unavailable_legs() == legs.len() {
            return true;
        }
        legs.iter().all(|section| {
            self.definition().sections[section]
                .criticals
                .iter()
                .any(|(&slot, part)| {
                    System::named(&part.equipment) == Some(System::ShoulderOrHip)
                        && self.critical_destroyed(CriticalLocation {
                            section: *section,
                            slot,
                        })
                })
        })
    }
}
