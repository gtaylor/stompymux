//! The VTOL Combat Vehicle Hit Location Table (Total Warfare p. 196, with the turret row of
//! Tactical Operations: Advanced Rules p. 193) and the rotor outcomes shared with its critical table.
use super::{HitArc, VehicleHit, VehicleSection};
use anyhow::{Result, ensure};
use serde::Serialize;

/// Rotors-column consequences, selected before the owning impact applies damage and flight effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RotorHit {
    /// Rotor Damage: one less Cruising MP.
    Damage,
    /// Flight Stabilizer Hit: cruising speed only, +3 to Driving Skill Rolls, +1 to-hit.
    FlightStabilizer,
    /// Rotors Destroyed: the VTOL is destroyed; in flight it crashes first.
    Destroy,
}

/// Located aircraft hit with the common armor request and the rotor footnote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[must_use = "Apply rotor and weapon consequences with the enclosing vehicle impact"]
pub struct VtolHit {
    pub hit: VehicleHit,
    /// A Rotors result: weapon damage is divided by ten and the VTOL loses one Cruising MP.
    pub rotor: bool,
}

impl super::Vehicle {
    /// Resolve the VTOL Combat Vehicle Hit Location Table from an existing 2d6 roll without
    /// drawing dice or changing material state.
    pub fn vtol_hit(&self, arc: HitArc, roll: u8) -> Result<VtolHit> {
        let (section, critical) = self.vtol_table_row(arc, roll)?;
        let proof = self.definition().has_special("CritProof_Tech");
        Ok(VtolHit {
            hit: VehicleHit {
                section,
                through_armor_critical: critical && !proof,
                motive: None,
            },
            rotor: section == VehicleSection::Rotor && !proof,
        })
    }

    /// The location alone, for attacks such as infernos that deal no damage but need a column.
    pub fn vtol_hit_location(&self, arc: HitArc, roll: u8) -> Result<VehicleSection> {
        Ok(self.vtol_table_row(arc, roll)?.0)
    }

    /// Location and critical flag for one row of the table.
    fn vtol_table_row(&self, arc: HitArc, roll: u8) -> Result<(VehicleSection, bool)> {
        ensure!(
            self.definition().is_vtol(),
            "VTOL hit selection requires a VTOL"
        );
        ensure!(
            (2..=12).contains(&roll),
            "Hit location roll must be between 2 and 12"
        );
        use VehicleSection as S;
        let face = arc.vehicle_section();
        let side = matches!(arc, HitArc::Left | HitArc::Right);
        // Tactical Operations: a turret strike on a turretless VTOL hits the rotors.
        let turret = if self.turret_heading().is_some() {
            S::Turret
        } else {
            S::Rotor
        };
        Ok(match roll {
            2 => (face, true),
            3 | 10 | 11 => (S::Rotor, false),
            4 => (turret, false),
            5 => (
                match arc {
                    HitArc::Front => S::Right,
                    HitArc::Rear => S::Left,
                    _ => S::Front,
                },
                false,
            ),
            6 | 7 => (face, false),
            8 => (face, side),
            9 => (
                match arc {
                    HitArc::Front => S::Left,
                    HitArc::Rear => S::Right,
                    _ => S::Rear,
                },
                false,
            ),
            _ => (S::Rotor, true),
        })
    }
}
