//! BattleMech attack directions and hit-location rules, separate from damage application.
use super::{Dice, Mech, MechChassis, MechSection};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// Direction from which an attack reaches the target, independent of weapon firing arcs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HitArc {
    Front,
    Rear,
    Left,
    Right,
}

impl HitArc {
    /// Hull face seen from an incoming attack, shared by ground vehicles and rotorcraft.
    pub fn vehicle_section(self) -> super::VehicleSection {
        match self {
            Self::Front => super::VehicleSection::Front,
            Self::Rear => super::VehicleSection::Rear,
            Self::Left => super::VehicleSection::Left,
            Self::Right => super::VehicleSection::Right,
        }
    }

    /// Classify a target-relative bearing with the 'Mech Attack Direction Diagram (Total
    /// Warfare p. 119): a 180-degree front, 60-degree sides and a 60-degree rear.
    pub fn from_bearing(bearing: f64, heading: f64) -> Result<Self> {
        Self::classify_bearing(bearing, heading, false)
    }

    /// Classify a target-relative bearing with the Vehicle Attack Direction Diagram (Total
    /// Warfare p. 192): one hexside each for the front and rear, two for each side.
    pub fn from_vehicle_bearing(bearing: f64, heading: f64) -> Result<Self> {
        Self::classify_bearing(bearing, heading, true)
    }

    /// Boundary inclusion and angle normalization are common to both diagrams.
    fn classify_bearing(bearing: f64, heading: f64, vehicle: bool) -> Result<Self> {
        ensure!(
            bearing.is_finite() && heading.is_finite(),
            "Invalid attack direction"
        );
        let angle = (bearing.rem_euclid(360.0) - heading.rem_euclid(360.0)).rem_euclid(360.0);
        let (front, rear) = if vehicle { (30.0, 30.0) } else { (90.0, 30.0) };
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
pub enum HitTable {
    Weapon,
    Punch,
    Kick,
}

impl HitTable {
    /// Resolve a supplied 2d6 weapon roll or d6 physical roll without consuming random state.
    pub fn location(self, chassis: MechChassis, arc: HitArc, roll: u8) -> Result<MechSection> {
        use HitArc::*;
        use MechSection::*;
        let (low, high) = if self == Self::Weapon {
            (2, 12)
        } else {
            (1, 6)
        };
        ensure!((low..=high).contains(&roll), "Invalid hit-location roll");
        if chassis == MechChassis::Quad && self != Self::Weapon {
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
        let row: &[MechSection] = match (self, arc) {
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
pub struct Hit {
    pub section: MechSection,
    /// The hit came from the rear arc. A torso takes it on its rear armor, and damage that
    /// passes from an arm or leg to its side torso strikes that torso's rear armor too (Total
    /// Warfare p. 123); the limbs and the head have only front armor.
    pub rear_armor: bool,
    pub through_armor_critical: bool,
}

/// Resolve an already successful weapon hit on the 'Mech Hit Location Table. The caller
/// supplies the 2d6 location roll; critical-proof targets draw their own. All returned
/// effects and dice commit together.
pub fn resolve_hit(target: &Mech, arc: HitArc, roll: u8, dice: &mut Dice) -> Result<Hit> {
    use MechSection::*;
    ensure!((2..=12).contains(&roll), "Invalid hit-location roll");
    let critical_proof = target.definition().has_special("CritProof_Tech");
    let mut roll = if critical_proof {
        dice.generic_roll()
    } else {
        roll
    };
    let mut section = HitTable::Weapon.location(target.chassis(), arc, roll)?;
    // A torso-mounted cockpit's Mech re-rolls hits on its destroyed head (Tactical Operations:
    // Advanced Units & Equipment p. 113).
    while section == Head && target.rerolls_head_hits() {
        roll = dice.generic_roll();
        section = HitTable::Weapon.location(target.chassis(), arc, roll)?;
    }
    if target.combat_safe {
        return Ok(Hit {
            section: LeftArm,
            rear_armor: false,
            through_armor_critical: false,
        });
    }
    // Total Warfare: a location roll of 2 is always a through-armor critical chance.
    let critical = roll == 2 && !critical_proof;
    Ok(Hit {
        section,
        rear_armor: arc == HitArc::Rear,
        through_armor_critical: critical,
    })
}

/// A d6 hit location on a punch or kick `table`, re-rolling hits on a torso-mounted cockpit
/// Mech's destroyed head (Tactical Operations: Advanced Units & Equipment p. 113).
pub(super) fn d6_location(
    target: &Mech,
    table: HitTable,
    arc: HitArc,
    dice: &mut Dice,
) -> Result<MechSection> {
    loop {
        let section = table.location(target.chassis(), arc, dice.d6())?;
        if section != MechSection::Head || !target.rerolls_head_hits() {
            return Ok(section);
        }
    }
}
