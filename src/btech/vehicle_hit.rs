//! Ground-vehicle hit locations and conditional critical eligibility, separate from damage.
use super::{BattleDice, BattleHitArc, BattleVehicle, BattleVehicleMovement, BattleVehicleSection};
use anyhow::{Result, ensure};
use serde::Serialize;

/// A vehicle hit and its secondary effects; a critical candidate still requires component selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[must_use = "Apply the hit and any critical effects in the enclosing damage transaction"]
pub struct BattleVehicleHit {
    pub section: BattleVehicleSection,
    pub through_armor_critical: bool,
    pub motive: Option<BattleVehicleMotiveHit>,
    /// Advanced motive roll before the movement-class adjustment.
    pub motive_roll: Option<u8>,
    pub piloting_penalty: u8,
}

/// Direct motive consequences selected by a vehicle hit table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BattleVehicleMotiveHit {
    /// Reduce maximum speed by this many movement points.
    SpeedLoss {
        movement_points: u8,
    },
    Immobilize,
}

/// Armor thresholds shared by vehicle hit tables.
#[derive(Debug, Clone, Copy)]
pub struct BattleVehicleHitRules {
    pub critical_mode: i64,
    pub critical_level: i64,
}

impl BattleVehicle {
    /// Resolve the standard ground-vehicle table from an existing 2d6 roll.
    /// Critical modes at most one disable armor-based candidates, except originally unarmored faces.
    /// The caller owns the supplied dice and commits all effects together. Advanced vehicle,
    /// dug-in and combat-safe policies must be handled by the combat adapter before choosing this table.
    pub fn standard_hit(
        &self,
        arc: BattleHitArc,
        roll: u8,
        critical_mode: i64,
        dice: &mut BattleDice,
    ) -> Result<BattleVehicleHit> {
        ensure!(
            (2..=12).contains(&roll),
            "Hit location roll must be between 2 and 12"
        );
        let section = self.hit_section(arc, roll);
        let turret_hit = section == BattleVehicleSection::Turret;
        let candidate = matches!(roll, 2 | 12) || (roll == 11 && turret_hit);
        let critical = candidate
            && self.critical_candidate(
                section,
                critical_mode,
                if turret_hit { 50 } else { 40 },
                dice,
            )?;
        Ok(BattleVehicleHit {
            section,
            through_armor_critical: critical,
            motive: None,
            motive_roll: None,
            piloting_penalty: 0,
        })
    }

    /// Standard location selection after validating the primary roll.
    fn hit_section(&self, arc: BattleHitArc, roll: u8) -> BattleVehicleSection {
        use BattleVehicleSection as S;
        let hull = arc.vehicle_section();
        let turret = self
            .sections()
            .get(&S::Turret)
            .is_some_and(|state| state.internal > 0);
        let turret_hit = turret
            && (matches!(roll, 10 | 11)
                || (roll == 12 && matches!(arc, BattleHitArc::Front | BattleHitArc::Rear)));
        if turret_hit { S::Turret } else { hull }
    }

    /// Critical-proof routing sends roll twelve to the hull even when a turret survives.
    pub fn critical_proof_hit(&self, arc: BattleHitArc, roll: u8) -> Result<BattleVehicleHit> {
        ensure!(
            (2..=12).contains(&roll),
            "Hit location roll must be between 2 and 12"
        );
        let section = self.hit_section(arc, if roll == 12 { 7 } else { roll });
        Ok(BattleVehicleHit {
            section,
            through_armor_critical: false,
            motive: None,
            motive_roll: None,
            piloting_penalty: 0,
        })
    }

    /// Advanced ground-vehicle table and its armor-gated motive check, without changing material state.
    pub fn advanced_hit(
        &self,
        arc: BattleHitArc,
        roll: u8,
        critical_mode: i64,
        critical_level: i64,
        dice: &mut BattleDice,
    ) -> Result<BattleVehicleHit> {
        ensure!(
            (2..=12).contains(&roll),
            "Hit location roll must be between 2 and 12"
        );
        use BattleVehicleSection as S;
        let side = matches!(arc, BattleHitArc::Left | BattleHitArc::Right);
        let hull = self.hit_section(arc, 7);
        let turretless = match arc {
            BattleHitArc::Front => S::Left,
            BattleHitArc::Rear => S::Right,
            _ => hull,
        };
        let turret = self
            .sections()
            .get(&S::Turret)
            .is_some_and(|section| section.internal > 0);
        let section = match roll {
            5 => match arc {
                BattleHitArc::Front => S::Right,
                BattleHitArc::Rear => S::Left,
                _ => S::Front,
            },
            9 => {
                if side {
                    S::Rear
                } else {
                    turretless
                }
            }
            10..=12 => {
                if turret {
                    S::Turret
                } else {
                    turretless
                }
            }
            _ => hull,
        };
        let mut hit = BattleVehicleHit {
            section,
            through_armor_critical: matches!(roll, 2 | 12) || (side && roll == 8),
            motive: None,
            motive_roll: None,
            piloting_penalty: 0,
        };
        if roll != 3 || !self.critical_candidate(section, critical_mode, critical_level, dice)? {
            return Ok(hit);
        }
        let motive_roll = dice.generic_roll();
        hit.motive_roll = Some(motive_roll);
        (hit.motive, hit.piloting_penalty) =
            super::vehicle_motive_effects::outcome(self.definition().movement, motive_roll);
        Ok(hit)
    }

    /// Armor thresholds gate secondary dice; exemptions and ineligible armor consume none.
    pub(super) fn critical_candidate(
        &self,
        section: BattleVehicleSection,
        mode: i64,
        threshold: i64,
        dice: &mut BattleDice,
    ) -> Result<bool> {
        if self.definition().movement == BattleVehicleMovement::Stationary
            || self.definition().has_special("CritProof_Tech")
        {
            return Ok(false);
        }
        let original = self.definition().sections[&section].armor;
        if original == 0 {
            return Ok(true);
        }
        if mode <= 1 {
            return Ok(false);
        }
        let percent = u32::from(self.sections()[&section].armor) * 100 / u32::from(original);
        if i64::from(percent) < threshold && dice.die(12)? == 6 {
            return Ok(true);
        }
        if percent == 100 {
            return Ok(dice.die(71)? == 23);
        }
        Ok(false)
    }
}
