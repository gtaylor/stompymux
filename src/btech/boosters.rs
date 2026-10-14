//! Booster installation facts derived from live critical slots, without cached technology flags.
use super::{Mech, System};
use anyhow::Result;

impl Mech {
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

    /// Installed MASC can remain physically present after it stops meeting its working threshold.
    pub fn masc_installed(&self) -> Result<bool> {
        Ok(self
            .loadout()?
            .systems
            .iter()
            .filter(|part| part.system == System::Masc)
            .count()
            >= self.masc_required_slots())
    }

    /// Working MASC requires the full threshold of functional, unflooded critical slots. A
    /// failed activation damages the legs but leaves the MASC itself working (Total Warfare
    /// p. 137).
    pub fn masc_operational(&self) -> Result<bool> {
        Ok(!self.is_destroyed()
            && self
                .loadout()?
                .systems
                .iter()
                .filter(|part| {
                    part.system == System::Masc && !self.critical_unavailable(part.location)
                })
                .count()
                >= self.masc_required_slots())
    }
}

impl Mech {
    /// The template technology flag controls supercharger installation independently of its critical slot.
    pub fn supercharger_installed(&self) -> bool {
        self.definition().has_special("SuperCharger_Tech")
    }
    /// A failed compressor stays unavailable until rebuilt; ordinary critical loss retains the technology flag.
    pub fn supercharger_operational(&self) -> bool {
        self.supercharger_installed() && !self.supercharger.failed && !self.is_destroyed()
    }
    /// Saved compressor activation and overload/recovery history.
    pub fn supercharger(&self) -> super::BoosterState {
        self.supercharger
    }
    /// Whether the compressor currently contributes to powered movement.
    pub fn supercharger_active(&self) -> bool {
        self.supercharger.enabled
            && self.power() == super::Power::Running
            && self.supercharger_operational()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A failed MASC activation leaves the system installed and working; only the
    /// supercharger's failure flag takes it out of service.
    #[test]
    fn masc_survives_its_failures_and_a_failed_supercharger_does_not() {
        let mut unit = Mech::from_template(
            super::super::MechTemplate::parse(
                "JR7-D",
                include_str!("../../tests/fixtures/btech/units/JR7-D.toml"),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(!unit.masc_installed().unwrap());
        unit.masc.failed = true;
        assert!(!unit.masc_operational().unwrap());
        unit.supercharger.failed = true;
        assert!(!unit.supercharger_operational());
        let restored: Mech = serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
        assert!(restored.supercharger().failed);
    }
}
