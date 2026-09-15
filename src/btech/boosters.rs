//! Booster installation facts derived from live critical slots, without cached technology flags.
use super::{BattleSystem, BattleUnit};
use anyhow::Result;

impl BattleUnit {
    /// MASC requires at least one slot, then one per twenty tons (twenty-five for Clan designs).
    /// The reference uses integer division, so surplus installed slots can tolerate a loss.
    pub fn masc_required_slots(&self) -> usize {
        usize::from(
            (self.definition().tons
                / if self.definition().has_special("Clan") {
                    25
                } else {
                    20
                })
            .max(1),
        )
    }

    /// Completed MASC seizure; fall resolution precedes hip damage in the failure action.
    pub(super) fn masc_seized(&self) -> bool {
        self.masc.failed
            && self.chassis().legs().iter().all(|&section| {
                self.critical_destroyed(super::CriticalLocation { section, slot: 0 })
            })
    }

    /// A failed MASC seizes every chassis hip, including quad front legs.
    pub(super) fn damage_masc_hips(&mut self) -> Result<()> {
        for &section in self.chassis().legs() {
            self.destroy_critical(super::CriticalLocation { section, slot: 0 })?;
        }
        Ok(())
    }

    /// Installed MASC can remain physically present after it stops meeting its working threshold.
    pub fn masc_installed(&self) -> Result<bool> {
        Ok(self
            .loadout()?
            .systems
            .iter()
            .filter(|part| part.system == BattleSystem::Masc)
            .count()
            >= self.masc_required_slots())
    }

    /// Working MASC requires the full threshold of functional, unflooded critical slots.
    pub fn masc_operational(&self) -> Result<bool> {
        Ok(!self.masc.failed
            && !self.is_destroyed()
            && self
                .loadout()?
                .systems
                .iter()
                .filter(|part| {
                    part.system == BattleSystem::Masc && !self.critical_unavailable(part.location)
                })
                .count()
                >= self.masc_required_slots())
    }
}

impl BattleUnit {
    /// The template technology flag controls supercharger installation independently of its critical slot.
    pub fn supercharger_installed(&self) -> bool {
        self.definition().has_special("SuperCharger_Tech")
    }
    /// A failed compressor stays unavailable until rebuilt; ordinary critical loss retains the technology flag.
    pub fn supercharger_operational(&self) -> bool {
        self.supercharger_installed() && !self.supercharger.failed && !self.is_destroyed()
    }
    /// Saved compressor activation and overload/recovery history.
    pub fn supercharger(&self) -> super::BattleBoosterState {
        self.supercharger
    }
    /// Whether the compressor currently contributes to powered movement.
    pub fn supercharger_active(&self) -> bool {
        self.supercharger.enabled
            && self.power() == super::BattlePower::Running
            && self.supercharger_operational()
    }
    /// Each active device adds one third of the unboosted running speed.
    pub(super) fn booster_multiplier(&self) -> f64 {
        1.0 + f64::from(u8::from(self.masc_active()) + u8::from(self.supercharger_active())) / 3.0
    }
    /// Successive toggles can temporarily request more than the combined effective ceiling.
    pub(super) fn booster_speed_envelope(&self) -> f64 {
        let count = u8::from(self.masc_installed().unwrap_or(false))
            + u8::from(self.supercharger_installed());
        (4.0_f64 / 3.0).powi(i32::from(count))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ordinary quad hip loss retains slow movement; MASC seizure immobilizes and prevents upright landing.
    #[test]
    fn masc_seizes_all_chassis_hips_and_survives_serialization() {
        let biped = BattleUnit::from_template(
            super::super::BattleTemplate::parse(include_str!(
                "../../tests/fixtures/btech/mechs/JR7-D"
            ))
            .unwrap(),
        )
        .unwrap();
        for chassis in ["Biped", "Quad"] {
            let mut encoded = serde_json::to_value(&biped).unwrap();
            if chassis == "Quad" {
                encoded["definition"] = serde_json::to_value(
                    super::super::BattleTemplate::parse(include_str!("../../game/mechs/SCP-1N"))
                        .unwrap(),
                )
                .unwrap();
            }
            let mut unit: BattleUnit = serde_json::from_value(encoded).unwrap();
            // The enclosing fall still uses pre-seizure support and movement.
            unit.masc.failed = true;
            assert!(unit.mobility().maximum_speed > 0.0);
            assert!(!unit.airborne_support_lost());
            unit.masc.failed = false;
            unit.damage_masc_hips().unwrap();
            for &section in unit.chassis().legs() {
                assert!(
                    unit.critical_destroyed(super::super::CriticalLocation { section, slot: 0 })
                );
            }
            if chassis == "Quad" {
                assert!(unit.mobility().maximum_speed > 0.0);
                assert!(!unit.airborne_support_lost());
            }
            unit.masc.failed = true;
            assert_eq!(unit.mobility().maximum_speed, 0.0);
            assert!(unit.airborne_support_lost());
            unit.masc.shutdown();
            let restored: BattleUnit =
                serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
            assert_eq!(restored.mobility(), unit.mobility());
            assert!(restored.airborne_support_lost());
            assert!(restored.masc().failed);
        }
    }
}
