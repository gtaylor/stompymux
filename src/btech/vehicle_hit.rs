//! The Ground Combat Vehicle Hit Location Table (Total Warfare p. 193) and the direct motive
//! consequences it selects, separate from armor damage.
use super::vehicle_motive_effects::MotiveSystemDamage;
use super::{Dice, HitArc, Vehicle, VehicleMovement, VehicleSection};
use anyhow::{Result, ensure};
use serde::Serialize;

/// A located vehicle hit and the table footnotes it triggered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[must_use = "Apply the hit and any critical effects in the enclosing damage transaction"]
pub struct VehicleHit {
    pub section: VehicleSection,
    /// A result of 2 or 12 (or 8 from the side) rolls once on the critical hits table.
    pub through_armor_critical: bool,
    /// A dagger result rolled on the Motive System Damage Table.
    pub motive: Option<MotiveSystemDamage>,
}

/// Direct motive consequences selected by the Motive System Damage Table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VehicleMotiveHit {
    /// Reduce Cruising MP by this many points and refigure flank from it (Total Warfare
    /// pp. 193, 197).
    SpeedLoss {
        movement_points: u8,
    },
    /// Halve the remaining Cruising MP, rounding up, and refigure flank from it (Total
    /// Warfare p. 193).
    HalveSpeed,
    Immobilize,
}

impl Vehicle {
    /// Resolve the Ground Combat Vehicle Hit Location Table from an existing 2d6 roll.
    /// Motive rolls draw from the supplied dice; the caller owns them and commits all effects
    /// together. Combat-safe policy is handled by the combat adapter before choosing a location.
    pub fn hit(&self, arc: HitArc, roll: u8, dice: &mut Dice) -> Result<VehicleHit> {
        let (section, critical, motive) = self.table_row(arc, roll)?;
        // Fixed emplacements have no motive system and critical-proof scenario props take no
        // criticals or motive damage.
        let proof = self.definition().movement == VehicleMovement::Stationary
            || self.definition().has_special("CritProof_Tech");
        let motive = (motive && !proof)
            .then(|| super::vehicle_motive_effects::located(self, arc, dice.generic_roll()));
        Ok(VehicleHit {
            section,
            through_armor_critical: critical && !proof,
            motive,
        })
    }

    /// The location alone, for attacks such as infernos that deal no damage but need a column.
    pub fn hit_location(&self, arc: HitArc, roll: u8) -> Result<VehicleSection> {
        Ok(self.table_row(arc, roll)?.0)
    }

    /// Location, critical flag and motive-roll flag for one row of the table.
    fn table_row(&self, arc: HitArc, roll: u8) -> Result<(VehicleSection, bool, bool)> {
        ensure!(
            (2..=12).contains(&roll),
            "Hit location roll must be between 2 and 12"
        );
        use VehicleSection as S;
        let face = arc.vehicle_section();
        let side = matches!(arc, HitArc::Left | HitArc::Right);
        // Without a turret, a turret hit strikes the armor on the side attacked.
        let turret = if self.turret_heading().is_some() {
            S::Turret
        } else {
            face
        };
        Ok(match roll {
            2 => (face, true, false),
            3 | 4 => (face, false, true),
            5 => (
                match arc {
                    HitArc::Front => S::Right,
                    HitArc::Rear => S::Left,
                    _ => S::Front,
                },
                false,
                true,
            ),
            6 | 7 => (face, false, false),
            8 => (face, side, false),
            9 => (
                match arc {
                    HitArc::Front => S::Left,
                    HitArc::Rear => S::Right,
                    _ => S::Rear,
                },
                false,
                true,
            ),
            10 | 11 => (turret, false, false),
            _ => (turret, true, false),
        })
    }
}
