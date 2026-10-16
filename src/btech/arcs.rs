//! BattleMech firing arcs and durable torso/arm facing controls (Total Warfare p. 105).
//!
//! A torso twist turns the upper body one hexside (60 degrees) and persists until changed;
//! reversed arms fire only into the rear arc, and neither pose may be combined with the other.
use super::{Mech, MechChassis, MechSection, Notice, Power, WeaponMount};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Relative torso orientation; a twist turns the upper body one hexside (60 degrees).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Torso {
    Left,
    #[default]
    Center,
    Right,
    /// Both flags can result from the mutual-charge torso merge; geometry favors right.
    Both,
}

impl Torso {
    /// Effective geometry offset in degrees; a merged pose retains rightward geometry.
    pub fn offset(self) -> f64 {
        match self {
            Self::Left => -60.0,
            Self::Center => 0.0,
            Self::Right | Self::Both => 60.0,
        }
    }

    /// Preserve both observable torso flags when a charge merges poses.
    pub(crate) fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Center, value) | (value, Self::Center) => value,
            (left, right) if left == right => left,
            _ => Self::Both,
        }
    }

    /// Real-forward arc probing restores left first when both torso flags were set.
    pub(crate) fn restored_after_forward_arc(self) -> Self {
        if self == Self::Both { Self::Left } else { self }
    }
}

/// Upper-body facing, independent of the movement heading.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Facing {
    pub torso: Torso,
    pub arms_flipped: bool,
}

/// General biped contact direction, separate from individual mount firing eligibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContactArc {
    Front,
    Right,
    Rear,
    Left,
}

impl ContactArc {
    /// Human-readable arc name shared by scan and contact reports.
    pub(crate) fn description(self, vehicle: bool) -> &'static str {
        match self {
            Self::Front => "Forward",
            Self::Rear => "Rear",
            Self::Left if vehicle => "Left Side",
            Self::Right if vehicle => "Right Side",
            Self::Left => "Left Arm",
            Self::Right => "Right Arm",
        }
    }

    /// Compact native display symbol.
    pub fn symbol(self) -> char {
        match self {
            Self::Front => '*',
            Self::Right => 'r',
            Self::Rear => 'v',
            Self::Left => 'l',
        }
    }
}

impl Facing {
    /// Classify a contact using whole heading degrees, nearest bearing and torso offset.
    /// Arm flips do not change this general direction; mounts retain their own firing checks.
    pub fn contact_arc(self, heading: f64, bearing: f64) -> Result<ContactArc> {
        ensure!(
            heading.is_finite() && bearing.is_finite(),
            "Invalid contact direction"
        );
        let angle = (bearing.rem_euclid(360.0).round()
            - heading.rem_euclid(360.0).trunc()
            - self.torso.offset())
        .rem_euclid(360.0);
        Ok(if angle <= 60.0 || angle >= 300.0 {
            ContactArc::Front
        } else if angle <= 120.0 {
            ContactArc::Right
        } else if angle < 240.0 {
            ContactArc::Rear
        } else {
            ContactArc::Left
        })
    }
}

/// Firing-arc geometry for Mech weapon mounts.
pub trait MountArcs {
    /// Test a compass bearing against mounting arcs (Total Warfare p. 105).
    ///
    /// Torso, head and leg mounts fire into the forward arc; arm mounts add their own side
    /// arc, or fire only into the rear arc while the arms are reversed. Rear mounts fire only
    /// into the rear arc. Leg mounts, including a quad's front legs, ignore torso rotation
    /// and carry no side arc.
    fn bears_on(
        &self,
        chassis: MechChassis,
        heading: f64,
        bearing: f64,
        facing: Facing,
    ) -> Result<bool>;
}

impl MountArcs for WeaponMount {
    fn bears_on(
        &self,
        chassis: MechChassis,
        heading: f64,
        bearing: f64,
        facing: Facing,
    ) -> Result<bool> {
        ensure!(
            heading.is_finite() && bearing.is_finite(),
            "Invalid firing direction"
        );
        let section = self
            .criticals
            .first()
            .context("Weapon mount has no criticals")?
            .section;
        let leg = chassis.is_leg(section);
        let twist = if leg { 0.0 } else { facing.torso.offset() };
        let angle =
            (bearing.rem_euclid(360.0) - heading.rem_euclid(360.0) - twist).rem_euclid(360.0);
        let front = angle <= 60.0 || angle >= 300.0;
        let rear = angle > 120.0 && angle < 240.0;
        if self.rear_mount {
            return Ok(rear);
        }
        if leg {
            return Ok(front);
        }
        let side = match section {
            MechSection::LeftArm => (240.0..300.0).contains(&angle),
            MechSection::RightArm => angle > 60.0 && angle <= 120.0,
            _ => return Ok(front),
        };
        if facing.arms_flipped {
            return Ok(rear);
        }
        Ok(side || front)
    }
}

impl Mech {
    /// Persisted upper-body pose used by weapon arc queries.
    pub fn facing(&self) -> Facing {
        self.facing
    }
}

/// Rotate one step toward left/right, or center directly; guards run before mutation.
pub fn rotate_torso(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    direction: Torso,
) -> Result<Notice> {
    let notice = rotate_torso_by_actor(
        world,
        id,
        super::combat_operator::ControlActor::Player(pilot),
        direction,
    )?;
    let _ = super::autopilot::manual_takeover(world, id);
    Ok(notice)
}

/// Autonomous torso movement retains every mechanical and crew-health guard.
pub(crate) fn rotate_torso_autopilot(
    world: &mut World,
    id: ObjectId,
    direction: Torso,
) -> Result<Notice> {
    rotate_torso_by_actor(
        world,
        id,
        super::combat_operator::ControlActor::Autopilot,
        direction,
    )
}

fn rotate_torso_by_actor(
    world: &mut World,
    id: ObjectId,
    actor: super::combat_operator::ControlActor,
    direction: Torso,
) -> Result<Notice> {
    ensure!(direction != Torso::Both, "Choose left, right or center");
    super::power::controlled_unit_by_actor(world, id, actor)?;
    let unit = &world.btech.constructed_units()[&id];
    ensure!(unit.power() == Power::Running, "Start the unit first");
    ensure!(
        unit.posture() != super::Posture::Prone,
        "Stand the unit up first"
    );
    ensure!(
        unit.chassis() != MechChassis::Quad,
        "Quads can't rotate their torsos."
    );
    // Reversed arms take the place of a torso twist; centering is harmless either way.
    ensure!(
        direction == Torso::Center || !unit.facing.arms_flipped,
        "You cannot rotate your torso while your arms are flipped backward!"
    );
    let torso = match (unit.facing.torso, direction) {
        (Torso::Left | Torso::Both, Torso::Left) | (Torso::Right | Torso::Both, Torso::Right) => {
            anyhow::bail!("You cannot rotate torso beyond 60 degrees!")
        }
        (Torso::Left, Torso::Right) | (Torso::Right, Torso::Left) => Torso::Center,
        (_, direction) => direction,
    };
    world.btech.constructed.get_mut(&id).unwrap().facing.torso = torso;
    Ok(Notice {
        unit: id,
        text: (match direction {
            Torso::Left => "You rotate your torso left.",
            Torso::Right => "You rotate your torso right.",
            Torso::Center => "You center your torso.",
            Torso::Both => unreachable!("direction checked above"),
        })
        .to_owned(),
    })
}

/// Toggle arms only on chassis explicitly supporting arm flipping.
///
/// Reversing the arms takes the place of a torso twist, so a twisted torso must be centered
/// first; flipping back forward is always allowed (Total Warfare p. 105).
pub fn flip_arms(world: &mut World, id: ObjectId, pilot: ObjectId) -> Result<Notice> {
    super::power::controlled_unit(world, id, pilot)?;
    let unit = &world.btech.constructed_units()[&id];
    ensure!(unit.power() == Power::Running, "Start the unit first");
    ensure!(
        unit.posture() != super::Posture::Prone,
        "Stand the unit up first"
    );
    ensure!(
        unit.definition().has_special("FlipArms"),
        "You cannot flip the arms in this mech"
    );
    ensure!(
        unit.facing.arms_flipped || unit.facing.torso == Torso::Center,
        "You cannot flip your arms while your torso is rotated!"
    );
    let unit = world.btech.constructed.get_mut(&id).unwrap();
    unit.facing.arms_flipped = !unit.facing.arms_flipped;
    let flipped = unit.facing.arms_flipped;
    let _ = super::autopilot::manual_takeover(world, id);
    Ok(Notice {
        unit: id,
        text: (if flipped {
            "Arms have been flipped to BACKWARD position"
        } else {
            "Arms have been flipped to FORWARD position"
        })
        .to_owned(),
    })
}

/// Query current mount geometry on a shared battlefield; this does not imply readiness or LOS.
pub fn weapon_bears_on(
    world: &World,
    shooter: ObjectId,
    target: ObjectId,
    index: usize,
) -> Result<bool> {
    let range = super::unit_range(world, shooter, target)?;
    let unit = &world.btech.constructed_units()[&shooter];
    let loadout = unit.loadout()?;
    let mount = loadout
        .weapons
        .get(index)
        .context("Weapon index out of bounds")?;
    mount.bears_on(
        unit.chassis(),
        unit.motion().context("Unit is not placed")?.heading,
        range.bearing.unwrap_or(180.0),
        unit.facing(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::{AmmunitionMode, CriticalLocation, FireMode, Weapon};

    fn mount(section: MechSection, rear_mount: bool) -> WeaponMount {
        WeaponMount {
            weapon: Weapon::MediumLaser,
            criticals: vec![CriticalLocation { section, slot: 0 }],
            rear_mount,
            one_shot: false,
            initially_spent: false,
            initial_ammunition_mode: AmmunitionMode::Normal,
            initial_fire_mode: FireMode::Normal,
        }
    }

    fn bears(section: MechSection, chassis: MechChassis, bearing: f64, facing: Facing) -> bool {
        mount(section, false)
            .bears_on(chassis, 0.0, bearing, facing)
            .unwrap()
    }

    /// A torso twist is one hexside, so a twisted forward arc still spans three hexsides.
    #[test]
    fn torso_twist_is_one_hexside() {
        assert_eq!(Torso::Left.offset(), -60.0);
        assert_eq!(Torso::Right.offset(), 60.0);
        assert_eq!(Torso::Both.offset(), 60.0);
        let right = Facing {
            torso: Torso::Right,
            arms_flipped: false,
        };
        let left = Facing {
            torso: Torso::Left,
            arms_flipped: false,
        };
        for bearing in [0.0, 60.0, 120.0] {
            assert!(bears(
                MechSection::CenterTorso,
                MechChassis::Biped,
                bearing,
                right
            ));
        }
        for bearing in [359.999, 120.001, 180.0] {
            assert!(!bears(
                MechSection::CenterTorso,
                MechChassis::Biped,
                bearing,
                right
            ));
        }
        for bearing in [240.0, 300.0, 0.0] {
            assert!(bears(
                MechSection::CenterTorso,
                MechChassis::Biped,
                bearing,
                left
            ));
        }
        for bearing in [0.001, 239.999, 180.0] {
            assert!(!bears(
                MechSection::CenterTorso,
                MechChassis::Biped,
                bearing,
                left
            ));
        }
        // Leg mounts keep the body's forward arc through any twist.
        assert!(bears(
            MechSection::LeftLeg,
            MechChassis::Biped,
            300.0,
            right
        ));
        assert!(!bears(
            MechSection::LeftLeg,
            MechChassis::Biped,
            120.0,
            right
        ));
    }

    /// Reversed arms fire into the rear arc instead of the forward and side arcs.
    #[test]
    fn flipped_arms_fire_only_into_the_rear_arc() {
        let flipped = Facing {
            torso: Torso::Center,
            arms_flipped: true,
        };
        for section in [MechSection::LeftArm, MechSection::RightArm] {
            for bearing in [120.001, 180.0, 239.999] {
                assert!(
                    bears(section, MechChassis::Biped, bearing, flipped),
                    "{bearing}"
                );
            }
            for bearing in [0.0, 60.0, 90.0, 120.0, 240.0, 270.0, 300.0] {
                assert!(
                    !bears(section, MechChassis::Biped, bearing, flipped),
                    "{section:?} {bearing}"
                );
            }
        }
        // Torso mounts are unaffected by the arms.
        assert!(bears(
            MechSection::CenterTorso,
            MechChassis::Biped,
            0.0,
            flipped
        ));
        assert!(!bears(
            MechSection::CenterTorso,
            MechChassis::Biped,
            180.0,
            flipped
        ));
    }

    /// A quad's front legs are legs: forward arc only, no side arc, no twist.
    #[test]
    fn quad_front_legs_use_only_the_forward_arc() {
        let twisted = Facing {
            torso: Torso::Right,
            arms_flipped: true,
        };
        for section in [MechSection::LeftArm, MechSection::RightArm] {
            for facing in [Facing::default(), twisted] {
                for bearing in [0.0, 60.0, 300.0] {
                    assert!(bears(section, MechChassis::Quad, bearing, facing));
                }
                for bearing in [61.0, 90.0, 180.0, 270.0, 299.0] {
                    assert!(
                        !bears(section, MechChassis::Quad, bearing, facing),
                        "{section:?} {bearing} {facing:?}"
                    );
                }
            }
            assert!(
                mount(section, true)
                    .bears_on(MechChassis::Quad, 0.0, 180.0, twisted)
                    .unwrap()
            );
        }
    }
}
