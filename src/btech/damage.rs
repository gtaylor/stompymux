//! Conventional armor and structure damage phases; combat resolves criticals between phases.
use super::{Mech, MechSection, SectionState};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// A material-damage phase, deliberately separate from hit selection and critical execution.
#[derive(Debug, Clone, Copy)]
pub enum DamagePhase {
    Armor { rear: bool },
    Internal,
}

/// Result of one phase. The caller decides whether and where remaining damage continues.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DamageResult<L = MechSection> {
    pub section: L,
    pub absorbed: u16,
    pub remaining: u16,
    /// For Mechs, includes the attached arm when its side torso is destroyed.
    pub destroyed_sections: Vec<L>,
    pub unit_destroyed: bool,
}

impl Mech {
    /// Core structure, cockpit loss or three engine hits destroy the unit, not its MUX object.
    pub fn is_destroyed(&self) -> bool {
        self.transport_destroyed
            || self.section_disabled(MechSection::Head)
            || self.character_pilot.is_some_and(|status| status.killed)
            || self.pilot_killed
            || (!self.systems_intact()
                && (self.system_hits(super::System::Engine) >= 3
                    || self.system_hits(super::System::Cockpit) > 0))
            || [MechSection::Head, MechSection::CenterTorso]
                .iter()
                .any(|section| {
                    self.sections
                        .get(section)
                        .is_none_or(|state| state.internal == 0)
                })
    }

    /// Resolve a material-damage phase after the calling combat handler's prerequisites.
    /// Armor leaves overflow for a critical check and then internal damage. Internal
    /// overflow returns to the next section's armor phase using `damage_transfer`.
    /// These primitives do not roll criticals, injure pilots, cause falls or notify players.
    pub fn damage_phase(
        &mut self,
        section: MechSection,
        amount: u16,
        phase: DamagePhase,
    ) -> DamageResult {
        let mut result = DamageResult {
            section,
            absorbed: 0,
            remaining: amount,
            destroyed_sections: Vec::new(),
            unit_destroyed: self.is_destroyed(),
        };
        // A destroyed location cannot absorb another hit, even if a corrupt fixture retains armor.
        if amount == 0 || self.sections[&section].internal == 0 {
            return result;
        }
        let state = self
            .sections
            .get_mut(&section)
            .expect("validated biped section");
        let protection = match phase {
            DamagePhase::Armor { rear: true }
                if matches!(
                    section,
                    MechSection::LeftTorso | MechSection::RightTorso | MechSection::CenterTorso
                ) =>
            {
                &mut state.rear
            }
            DamagePhase::Armor { .. } => &mut state.armor,
            DamagePhase::Internal => &mut state.internal,
        };
        result.absorbed = (*protection).min(amount);
        *protection -= result.absorbed;
        if result.absorbed > 0 {
            self.live_mass.invalidate();
        }
        result.remaining -= result.absorbed;
        if matches!(phase, DamagePhase::Internal) && state.internal == 0 {
            self.clear_destroyed_section(section, &mut result.destroyed_sections);
        }
        result.unit_destroyed = self.is_destroyed();
        self.reconcile_damage();
        result
    }

    /// Clear protection and ammunition in a lost location and cascade side-torso arm loss.
    fn clear_destroyed_section(&mut self, section: MechSection, destroyed: &mut Vec<MechSection>) {
        self.recalculate_section_loss(section);
        *self
            .sections
            .get_mut(&section)
            .expect("validated biped section") = SectionState {
            armor: 0,
            internal: 0,
            rear: 0,
        };
        self.beacons.remove(&section);
        self.flooded_sections.remove(&section);
        self.breached_sections.remove(&section);
        destroyed.push(section);
        if self.chassis() == super::MechChassis::Quad && self.chassis().is_leg(section) {
            self.lateral.active = super::LateralMode::None;
            if self.lateral.pending == Some(super::LateralMode::None) {
                self.lateral.pending = None;
                self.lateral.remaining = 0;
            }
        }
        let slots: Vec<_> = self.definition().sections[&section]
            .criticals
            .keys()
            .copied()
            .collect();
        self.lost_criticals.extend(
            slots
                .into_iter()
                .map(|slot| super::CriticalLocation { section, slot }),
        );

        let loadout = self.loadout().expect("validated biped equipment");
        if loadout.weapons.iter().any(|mount| {
            mount.weapon.is_ams()
                && mount
                    .criticals
                    .iter()
                    .any(|location| location.section == section)
        }) {
            self.ams_enabled = false;
        }
        for (index, bin) in loadout.ammunition.iter().enumerate() {
            if bin.location.section == section {
                self.ammunition[index] = 0;
            }
        }
        let arm = match section {
            MechSection::LeftTorso => Some(MechSection::LeftArm),
            MechSection::RightTorso => Some(MechSection::RightArm),
            _ => None,
        };
        if let Some(arm) = arm.filter(|arm| self.sections[arm].internal != 0) {
            self.clear_destroyed_section(arm, destroyed);
        }
    }
}

/// Apply one phase inside the caller's world checkpoint; all phases and criticals share its commit.
/// This internal combat-building API is not a player command or a complete weapon-damage action.
pub fn apply_damage_phase(
    world: &mut World,
    id: ObjectId,
    section: MechSection,
    amount: u16,
    phase: DamagePhase,
) -> Result<DamageResult> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Unit is unavailable"
    );
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit construction state is unavailable")?;
    unit.validate()?;
    let unit = world.btech.constructed.get_mut(&id).unwrap();
    Ok(unit.damage_phase(section, amount, phase))
}
