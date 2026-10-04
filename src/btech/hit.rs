//! BattleMech attack directions and hit-location rules, separate from damage application.
use super::{BattleDice, BattleMechChassis, BattleSection, BattleUnit};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// Direction from which an attack reaches the target, independent of weapon firing arcs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BattleHitArc {
    Front,
    Rear,
    Left,
    Right,
}

impl BattleHitArc {
    /// Hull face seen from an incoming attack, shared by ground vehicles and rotorcraft.
    pub fn vehicle_section(self) -> super::BattleVehicleSection {
        match self {
            Self::Front => super::BattleVehicleSection::Front,
            Self::Rear => super::BattleVehicleSection::Rear,
            Self::Left => super::BattleVehicleSection::Left,
            Self::Right => super::BattleVehicleSection::Right,
        }
    }

    /// Classify target-relative bearing using configured biped hit arcs.
    /// Modes 0 and 2 use 180-degree front and 60-degree rear arcs; mode 1 uses quadrants.
    pub fn from_bearing(bearing: f64, heading: f64, mode: i64) -> Result<Self> {
        Self::classify_bearing(bearing, heading, mode, false)
    }

    /// Ground vehicles and VTOLs use a 60-degree front arc in mode zero.
    /// Modes one and two share the Mech arc widths.
    pub fn from_vehicle_bearing(bearing: f64, heading: f64, mode: i64) -> Result<Self> {
        Self::classify_bearing(bearing, heading, mode, true)
    }

    /// Boundary inclusion and angle normalization are common to every supported chassis.
    fn classify_bearing(bearing: f64, heading: f64, mode: i64, vehicle: bool) -> Result<Self> {
        ensure!(
            bearing.is_finite() && heading.is_finite(),
            "Invalid attack direction"
        );
        ensure!((0..=2).contains(&mode), "Unsupported hit arc mode");
        let angle = (bearing.rem_euclid(360.0) - heading.rem_euclid(360.0)).rem_euclid(360.0);
        let (front, rear) = if mode == 1 {
            (45.0, 45.0)
        } else if mode == 0 && vehicle {
            (30.0, 30.0)
        } else {
            (90.0, 30.0)
        };
        if angle <= front || angle >= 360.0 - front {
            return Ok(Self::Front);
        }
        if angle >= 180.0 - rear && angle <= 180.0 + rear {
            return Ok(Self::Rear);
        }
        Ok(if angle > 180.0 {
            Self::Left
        } else {
            Self::Right
        })
    }
}

/// Hit-location distribution selected by a weapon hit, punch, or kick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum BattleHitTable {
    Weapon,
    Punch,
    Kick,
}

impl BattleHitTable {
    /// Resolve a supplied 2d6 weapon roll or d6 physical roll without consuming random state.
    pub fn location(
        self,
        chassis: BattleMechChassis,
        arc: BattleHitArc,
        roll: u8,
    ) -> Result<BattleSection> {
        use BattleHitArc::*;
        use BattleSection::*;
        let (low, high) = if self == Self::Weapon {
            (2, 12)
        } else {
            (1, 6)
        };
        ensure!((low..=high).contains(&roll), "Invalid hit-location roll");
        if chassis == BattleMechChassis::Quad && self != Self::Weapon {
            let row = match (self, arc) {
                (Self::Punch, Front) => {
                    [LeftArm, LeftTorso, CenterTorso, RightTorso, RightArm, Head]
                }
                (Self::Punch, Rear) => {
                    [LeftLeg, LeftTorso, CenterTorso, RightTorso, RightLeg, Head]
                }
                (Self::Punch, Left) => [LeftTorso, LeftTorso, CenterTorso, LeftArm, LeftLeg, Head],
                (Self::Punch, Right) => [
                    RightTorso,
                    RightTorso,
                    CenterTorso,
                    RightArm,
                    RightLeg,
                    Head,
                ],
                (Self::Kick, Front) => [RightArm, RightArm, RightArm, LeftArm, LeftArm, LeftArm],
                (Self::Kick, Rear) => [RightLeg, RightLeg, RightLeg, LeftLeg, LeftLeg, LeftLeg],
                (Self::Kick, Left) => [LeftArm, LeftArm, LeftArm, LeftLeg, LeftLeg, LeftLeg],
                (Self::Kick, Right) => [RightArm, RightArm, RightArm, RightLeg, RightLeg, RightLeg],
                (Self::Weapon, _) => unreachable!("weapon table is shared"),
            };
            return Ok(row[usize::from(roll - low)]);
        }
        let row: &[BattleSection] = match (self, arc) {
            (Self::Weapon, Front | Rear) => &[
                CenterTorso,
                RightArm,
                RightArm,
                RightLeg,
                RightTorso,
                CenterTorso,
                LeftTorso,
                LeftLeg,
                LeftArm,
                LeftArm,
                Head,
            ],
            (Self::Weapon, Left) => &[
                LeftTorso,
                LeftLeg,
                LeftArm,
                LeftArm,
                LeftLeg,
                LeftTorso,
                CenterTorso,
                RightTorso,
                RightArm,
                RightLeg,
                Head,
            ],
            (Self::Weapon, Right) => &[
                RightTorso,
                RightLeg,
                RightArm,
                RightArm,
                RightLeg,
                RightTorso,
                CenterTorso,
                LeftTorso,
                LeftArm,
                LeftLeg,
                Head,
            ],
            (Self::Punch, Front | Rear) => {
                &[LeftArm, LeftTorso, CenterTorso, RightTorso, RightArm, Head]
            }
            (Self::Punch, Left) => &[LeftTorso, LeftTorso, CenterTorso, LeftArm, LeftArm, Head],
            (Self::Punch, Right) => &[
                RightTorso,
                RightTorso,
                CenterTorso,
                RightArm,
                RightArm,
                Head,
            ],
            (Self::Kick, Front | Rear) => {
                &[RightLeg, RightLeg, RightLeg, LeftLeg, LeftLeg, LeftLeg]
            }
            (Self::Kick, Left) => &[LeftLeg; 6],
            (Self::Kick, Right) => &[RightLeg; 6],
        };
        Ok(row[usize::from(roll - low)])
    }
}

/// Effects a damage handler must apply together; a critical candidate is not a destroyed component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleHit {
    pub section: BattleSection,
    pub rear_armor: bool,
    pub through_armor_critical: bool,
    pub crew_stun: bool,
}

/// Conventional biped hit variants selected by game configuration.
#[derive(Debug, Clone, Copy)]
pub struct BattleHitRules {
    /// Add thirty stored heat when an inferno ammunition bin explodes.
    pub inferno_penalty: bool,
    /// Zero preserves head hits; one rerolls and stuns on a graze; larger values only reroll.
    pub exile_stun_mode: u8,
}

impl BattleHitRules {
    /// Resolve an already successful weapon hit, including conditional secondary rolls.
    /// The caller supplies the routing-entry roll; critical-proof tables draw
    /// their own location roll. All returned effects and dice commit together.
    pub fn resolve(
        self,
        target: &BattleUnit,
        arc: BattleHitArc,
        roll: u8,
        dice: &mut BattleDice,
    ) -> Result<BattleHit> {
        use BattleSection::*;
        ensure!((2..=12).contains(&roll), "Invalid hit-location roll");
        let critical_proof = target.definition().has_special("CritProof_Tech");
        let roll = if critical_proof {
            dice.generic_roll()
        } else {
            roll
        };
        let mut section = BattleHitTable::Weapon.location(target.chassis(), arc, roll)?;
        if target.combat_safe {
            return Ok(BattleHit {
                section: LeftArm,
                rear_armor: false,
                through_armor_critical: false,
                crew_stun: false,
            });
        }
        let mut critical = false;
        if roll == 2 && !critical_proof {
            let original = target.definition().sections[&section].armor;
            let armor = target.sections()[&section].armor;
            critical = if original == 0 {
                true
            } else {
                let remaining = u32::from(armor) * 100 / u32::from(original);
                match remaining {
                    0..60 => dice.die(12)? == 6,
                    100 => dice.die(71)? == 23,
                    _ => false,
                }
            };
        }
        let mut crew_stun = false;
        if roll == 12 && self.exile_stun_mode != 0 {
            section = BattleHitTable::Punch.location(target.chassis(), arc, dice.d6())?;
            crew_stun = self.exile_stun_mode == 1 && section != Head;
        }
        Ok(BattleHit {
            section,
            rear_armor: arc == BattleHitArc::Rear
                && matches!(section, LeftTorso | RightTorso | CenterTorso),
            through_armor_critical: critical,
            crew_stun,
        })
    }
}
