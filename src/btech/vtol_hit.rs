//! Rotorcraft hit selection reuses vehicle hull identities and keeps rotor effects explicit.
use super::{BattleHitArc, BattleVehicleHit, BattleVehicleSection, BattleVehicleTemplate};
use anyhow::{Result, ensure};
use serde::Serialize;

/// Rotor consequences selected before the owning impact applies damage and flight effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleRotorHit {
    Damage,
    TailRotor,
    Destroy,
}

/// Located aircraft hit with the common armor request and aircraft-specific secondary effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[must_use = "Apply rotor and weapon consequences with the enclosing vehicle impact"]
pub struct BattleVtolHit {
    pub hit: BattleVehicleHit,
    pub rotor: Option<BattleRotorHit>,
}

/// VTOL hit-location selection from a supplied roll.
pub trait BattleVtolHitLocation {
    /// Select a VTOL location from a supplied 2d6 roll without drawing dice or mutating material.
    /// Critical-proof equipment uses standard locations and suppresses all secondary effects.
    fn vtol_hit(&self, arc: BattleHitArc, roll: u8) -> Result<BattleVtolHit>;
}

impl BattleVtolHitLocation for BattleVehicleTemplate {
    fn vtol_hit(&self, arc: BattleHitArc, roll: u8) -> Result<BattleVtolHit> {
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
            BattleVehicleSection::Rotor
        } else {
            arc.vehicle_section()
        };
        let rotor = if proof || !rotor_hit {
            None
        } else if roll == 2 {
            Some(BattleRotorHit::Destroy)
        } else {
            Some(BattleRotorHit::Damage)
        };
        Ok(BattleVtolHit {
            hit: BattleVehicleHit {
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

impl BattleRotorHit {
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

impl super::BattleVehicle {
    /// Advanced aircraft routing uses surviving turret state and the shared armor critical gate.
    /// The gate runs even for noncritical table entries to preserve the saved random stream.
    pub fn advanced_vtol_hit(
        &self,
        arc: BattleHitArc,
        roll: u8,
        critical_mode: i64,
        critical_level: i64,
        dice: &mut super::BattleDice,
    ) -> Result<BattleVtolHit> {
        ensure!(
            self.definition().is_vtol(),
            "Advanced aircraft hit selection requires a VTOL"
        );
        ensure!(
            (2..=12).contains(&roll),
            "Hit location roll must be between 2 and 12"
        );
        use BattleVehicleSection as S;
        let side = matches!(arc, BattleHitArc::Left | BattleHitArc::Right);
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
                BattleHitArc::Front => S::Right,
                BattleHitArc::Rear => S::Left,
                _ => S::Front,
            },
            9 => match arc {
                BattleHitArc::Front => S::Left,
                BattleHitArc::Rear => S::Right,
                _ => S::Rear,
            },
            10..=12 => S::Rotor,
            _ => arc.vehicle_section(),
        };
        let eligible = self.critical_candidate(section, critical_mode, critical_level, dice)?;
        Ok(BattleVtolHit {
            hit: BattleVehicleHit {
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
