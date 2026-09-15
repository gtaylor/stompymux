//! BattleMech firing arcs and durable torso/arm facing controls.
use super::{BattleMechChassis, BattleNotice, BattlePower, BattleSection, BattleUnit, WeaponMount};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Relative torso orientation; firing geometry uses the game's 59-degree offsets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleTorso {
    Left,
    #[default]
    Center,
    Right,
    /// Both flags can result from the mutual-charge torso merge; geometry favors right.
    Both,
}

impl BattleTorso {
    /// Effective geometry offset; a merged pose retains rightward geometry.
    pub fn offset(self) -> f64 {
        match self {
            Self::Left => -59.0,
            Self::Center => 0.0,
            Self::Right | Self::Both => 59.0,
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
pub struct BattleFacing {
    pub torso: BattleTorso,
    pub arms_flipped: bool,
}

/// General biped contact direction, separate from individual mount firing eligibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleContactArc {
    Front,
    Right,
    Rear,
    Left,
}

impl BattleContactArc {
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

impl BattleFacing {
    /// Classify a contact using whole heading degrees, nearest bearing and torso offset.
    /// Arm flips do not change this general direction; mounts retain their own firing checks.
    pub fn contact_arc(self, heading: f64, bearing: f64) -> Result<BattleContactArc> {
        ensure!(
            heading.is_finite() && bearing.is_finite(),
            "Invalid contact direction"
        );
        let angle = (bearing.rem_euclid(360.0).round()
            - heading.rem_euclid(360.0).trunc()
            - self.torso.offset())
        .rem_euclid(360.0);
        Ok(if angle <= 60.0 || angle >= 300.0 {
            BattleContactArc::Front
        } else if angle <= 120.0 {
            BattleContactArc::Right
        } else if angle < 240.0 {
            BattleContactArc::Rear
        } else {
            BattleContactArc::Left
        })
    }
}

impl WeaponMount {
    /// Test a compass bearing against mounting arcs; leg mounts ignore torso rotation.
    pub fn bears_on(
        &self,
        chassis: BattleMechChassis,
        heading: f64,
        bearing: f64,
        facing: BattleFacing,
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
        let twist = if chassis.is_leg(section) {
            0.0
        } else {
            facing.torso.offset()
        };
        let angle =
            (bearing.rem_euclid(360.0) - heading.rem_euclid(360.0) - twist).rem_euclid(360.0);
        let front = angle <= 60.0 || angle >= 300.0;
        let rear = angle > 120.0 && angle < 240.0;
        if self.rear_mount {
            return Ok(rear);
        }
        let side = match section {
            BattleSection::LeftArm => (240.0..300.0).contains(&angle),
            BattleSection::RightArm => angle > 60.0 && angle <= 120.0,
            _ => return Ok(front),
        };
        Ok(side || if facing.arms_flipped { rear } else { front })
    }
}

impl BattleUnit {
    /// Persisted upper-body pose used by weapon arc queries.
    pub fn facing(&self) -> BattleFacing {
        self.facing
    }
}

/// Rotate one step toward left/right, or center directly; guards run before mutation.
pub fn rotate_torso(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    direction: BattleTorso,
) -> Result<BattleNotice> {
    ensure!(
        direction != BattleTorso::Both,
        "Choose left, right or center"
    );
    super::power::controlled_unit(world, id, pilot)?;
    let unit = &world.btech.constructed_units()[&id];
    ensure!(unit.power() == BattlePower::Running, "Start the unit first");
    ensure!(
        unit.posture() != super::BattlePosture::Prone,
        "Stand the unit up first"
    );
    ensure!(
        unit.chassis() != BattleMechChassis::Quad,
        "Quads can't rotate their torsos."
    );
    let torso = match (unit.facing.torso, direction) {
        (BattleTorso::Left | BattleTorso::Both, BattleTorso::Left)
        | (BattleTorso::Right | BattleTorso::Both, BattleTorso::Right) => {
            anyhow::bail!("You cannot rotate torso beyond 60 degrees!")
        }
        (BattleTorso::Left, BattleTorso::Right) | (BattleTorso::Right, BattleTorso::Left) => {
            BattleTorso::Center
        }
        (_, direction) => direction,
    };
    Arc::make_mut(&mut world.btech.constructed)
        .get_mut(&id)
        .unwrap()
        .facing
        .torso = torso;
    Ok(BattleNotice {
        unit: id,
        text: (match direction {
            BattleTorso::Left => "You rotate your torso left.",
            BattleTorso::Right => "You rotate your torso right.",
            BattleTorso::Center => "You center your torso.",
            BattleTorso::Both => unreachable!("direction checked above"),
        })
        .to_owned(),
    })
}

/// Toggle arms only on chassis explicitly supporting arm flipping.
pub fn flip_arms(world: &mut World, id: ObjectId, pilot: ObjectId) -> Result<BattleNotice> {
    super::power::controlled_unit(world, id, pilot)?;
    let unit = &world.btech.constructed_units()[&id];
    ensure!(unit.power() == BattlePower::Running, "Start the unit first");
    ensure!(
        unit.posture() != super::BattlePosture::Prone,
        "Stand the unit up first"
    );
    ensure!(
        unit.definition().has_special("FlipArms"),
        "You cannot flip the arms in this mech"
    );
    let unit = Arc::make_mut(&mut world.btech.constructed)
        .get_mut(&id)
        .unwrap();
    unit.facing.arms_flipped = !unit.facing.arms_flipped;
    Ok(BattleNotice {
        unit: id,
        text: (if unit.facing.arms_flipped {
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
