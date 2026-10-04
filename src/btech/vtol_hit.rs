//! Rotorcraft hit selection reuses vehicle hull identities and keeps rotor effects explicit.
use super::{HitArc, VehicleHit, VehicleSection, VehicleTemplate};
use anyhow::{Result, ensure};
use serde::Serialize;

/// Rotor consequences selected before the owning impact applies damage and flight effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RotorHit {
    Damage,
    TailRotor,
    Destroy,
}

/// Located aircraft hit with the common armor request and aircraft-specific secondary effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[must_use = "Apply rotor and weapon consequences with the enclosing vehicle impact"]
pub struct VtolHit {
    pub hit: VehicleHit,
    pub rotor: Option<RotorHit>,
}

/// VTOL hit-location selection from a supplied roll.
pub trait VtolHitLocation {
    /// Select a VTOL location from a supplied 2d6 roll without drawing dice or mutating material.
    /// Critical-proof equipment uses standard locations and suppresses all secondary effects.
    fn vtol_hit(&self, arc: HitArc, roll: u8) -> Result<VtolHit>;
}

impl VtolHitLocation for VehicleTemplate {
    fn vtol_hit(&self, arc: HitArc, roll: u8) -> Result<VtolHit> {
        ensure!(
            self.is_vtol(),
            "VTOL hit selection requires a VTOL template"
        );
        ensure!(
            (2..=12).contains(&roll),
            "Hit location roll must be between 2 and 12"
        );
        let proof = self.has_special("CritProof_Tech");
        let rotor_hit = matches!(roll, 2..=4 | 10..=12);
        let section = if rotor_hit {
            VehicleSection::Rotor
        } else {
            arc.vehicle_section()
        };
        let rotor = if proof || !rotor_hit {
            None
        } else if roll == 2 {
            Some(RotorHit::Destroy)
        } else {
            Some(RotorHit::Damage)
        };
        Ok(VtolHit {
            hit: VehicleHit {
                section,
                through_armor_critical: !proof && matches!(roll, 2 | 12),
                motive: None,
                motive_roll: None,
                piloting_penalty: 0,
            },
            rotor,
        })
    }
}

impl RotorHit {
    /// Advanced rotor critical table; rolls below six have no effect.
    pub fn from_critical_roll(roll: u8) -> Result<Option<Self>> {
        ensure!(
            (2..=12).contains(&roll),
            "Critical roll must be between 2 and 12"
        );
        Ok(match roll {
            6..=8 => Some(Self::Damage),
            9 | 10 => Some(Self::TailRotor),
            11 | 12 => Some(Self::Destroy),
            _ => None,
        })
    }
}

impl super::Vehicle {
    /// Advanced aircraft routing uses surviving turret state and the shared armor critical gate.
    /// The gate runs even for noncritical table entries to preserve the saved random stream.
    pub fn advanced_vtol_hit(
        &self,
        arc: HitArc,
        roll: u8,
        critical_mode: i64,
        critical_level: i64,
        dice: &mut super::Dice,
    ) -> Result<VtolHit> {
        ensure!(
            self.definition().is_vtol(),
            "Advanced aircraft hit selection requires a VTOL"
        );
        ensure!(
            (2..=12).contains(&roll),
            "Hit location roll must be between 2 and 12"
        );
        use VehicleSection as S;
        let side = matches!(arc, HitArc::Left | HitArc::Right);
        let section = match roll {
            4 => {
                if self
                    .sections()
                    .get(&S::Turret)
                    .is_some_and(|section| section.internal > 0)
                {
                    S::Turret
                } else {
                    S::Rotor
                }
            }
            5 => match arc {
                HitArc::Front => S::Right,
                HitArc::Rear => S::Left,
                _ => S::Front,
            },
            9 => match arc {
                HitArc::Front => S::Left,
                HitArc::Rear => S::Right,
                _ => S::Rear,
            },
            10..=12 => S::Rotor,
            _ => arc.vehicle_section(),
        };
        let eligible = self.critical_candidate(section, critical_mode, critical_level, dice)?;
        Ok(VtolHit {
            hit: VehicleHit {
                section,
                through_armor_critical: eligible && (matches!(roll, 2 | 12) || (side && roll == 8)),
                motive: None,
                motive_roll: None,
                piloting_penalty: 0,
            },
            rotor: None,
        })
    }
}
