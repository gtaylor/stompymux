//! Contact acquisition: perceived targets become contacts at once, except hidden hostile units.
//!
//! A hidden hostile unit cannot be found beyond [`HIDDEN_DETECTION_RANGE`] unless an active
//! probe reaches it. Closer than [`AUTOMATIC_DETECTION_RANGE`] it is found automatically, and
//! in between the observer rolls against its facing and the pilot's perception skill.
use super::Perception;
use crate::btech::{Facing, VehicleSection};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Hidden hostile units cannot be found beyond this range without a probe.
pub const HIDDEN_DETECTION_RANGE: f64 = 5.0;

/// Inside this range even hidden units are found without a roll.
pub const AUTOMATIC_DETECTION_RANGE: f64 = 3.0;

/// Observer-relative direction used to weight searches and lock settling; independent of hit arcs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensorArc {
    Front,
    Side,
    Rear,
}

impl SensorArc {
    /// Determine direction from continuous compass bearing and torso-adjusted heading.
    pub fn from_bearing(bearing: f64, heading: f64, facing: Facing) -> Result<Self> {
        ensure!(
            bearing.is_finite() && heading.is_finite(),
            "Invalid sensor direction"
        );
        let twist = facing.torso.offset();
        let relative =
            (bearing.rem_euclid(360.0) - heading.rem_euclid(360.0) - twist).rem_euclid(360.0);
        if relative <= 60.0 || relative >= 300.0 {
            return Ok(Self::Front);
        }
        if relative > 120.0 && relative < 240.0 {
            return Ok(Self::Rear);
        }
        Ok(Self::Side)
    }
}

/// Target facts that decide whether acquiring a newly perceived unit needs a roll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcquisitionRules {
    pub hostile: bool,
    pub hidden: bool,
    /// The observing pilot's perception skill target.
    pub perception: i16,
}

/// One acquisition decision. Only hidden-unit searches consume dice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Detection {
    pub detected: bool,
    /// Strict upper bound for a successful 1–10,000 roll; zero when no search was needed.
    pub threshold: u16,
    /// Present only when a hidden-unit search was rolled.
    pub roll: Option<u16>,
}

impl Detection {
    /// A decision made without a search.
    const fn settled(detected: bool) -> Self {
        Self {
            detected,
            threshold: 0,
            roll: None,
        }
    }
}

/// Hostile-target perception factor derived from the padded 2d6 success table.
pub fn perception_factor(perception: i16) -> u16 {
    let successes = match perception {
        i16::MIN..=2 => 36,
        3 => 33,
        4 => 30,
        5 => 26,
        6 => 21,
        7 => 15,
        8 => 10,
        9 => 6,
        10 => 3,
        11..=i16::MAX => 1,
    };
    64 + successes
}

/// Decide whether a newly perceived target becomes a contact, rolling only for hidden units.
/// Observer dice are committed only when a search is actually rolled.
pub(crate) fn acquire(
    world: &mut World,
    observer: ObjectId,
    perception: &Perception,
    rules: AcquisitionRules,
) -> Result<Detection> {
    if !(rules.hidden && rules.hostile) || perception.probed {
        return Ok(Detection::settled(true));
    }
    let distance = perception.range.spatial;
    if distance > HIDDEN_DETECTION_RANGE {
        return Ok(Detection::settled(false));
    }
    let threshold = hidden_threshold(world, observer, perception, rules.perception)?;
    if distance < AUTOMATIC_DETECTION_RANGE {
        return Ok(Detection {
            detected: true,
            threshold,
            roll: None,
        });
    }
    let roll = crate::btech::dice::unit_dice_mut(world, observer)?.die(10_000)?;
    Ok(Detection {
        detected: roll < threshold,
        threshold,
        roll: Some(roll),
    })
}

/// Arc weight, quartered perception factor and a close-range bonus, in integer order.
fn hidden_threshold(
    world: &World,
    observer: ObjectId,
    perception: &Perception,
    skill: i16,
) -> Result<u16> {
    let bearing = perception.range.bearing.unwrap_or(180.0);
    let base = observer_arc_base(world, observer, bearing)?;
    let base = base * perception_factor(skill) / 100 / 4;
    let proximity = (100.0 - perception.range.spatial / 3.0) as u16;
    Ok(base * proximity)
}

/// Mech weights follow torso facing; vehicles use hull weights with a live turret bonus.
fn observer_arc_base(world: &World, observer: ObjectId, bearing: f64) -> Result<u16> {
    let Some(vehicle) = world.btech.vehicles().get(&observer) else {
        let unit = world
            .btech
            .constructed_units()
            .get(&observer)
            .context("Unit construction state is unavailable")?;
        let heading = unit.motion().context("Unit is not placed")?.heading;
        return Ok(
            match SensorArc::from_bearing(bearing, heading, unit.facing())? {
                SensorArc::Front => 100,
                SensorArc::Side => 70,
                SensorArc::Rear => 40,
            },
        );
    };
    let heading = vehicle.motion().context("Vehicle is not placed")?.heading;
    let base = match SensorArc::from_bearing(bearing.round(), heading.trunc(), Facing::default())? {
        SensorArc::Front => 100,
        SensorArc::Side => 80,
        SensorArc::Rear => 50,
    };
    let turret = vehicle
        .definition()
        .sections
        .get(&VehicleSection::Turret)
        .is_some_and(|section| section.internal > 0)
        && {
            let facing = heading + vehicle.turret_offset;
            let angle = (bearing.rem_euclid(360.0).round() - facing.rem_euclid(360.0).trunc())
                .rem_euclid(360.0);
            angle <= 30.0 || angle >= 330.0
        };
    Ok(base + if turret { 15 } else { 0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The perception table keeps the reference's integer factors.
    #[test]
    fn perception_factor_follows_the_success_table() {
        for (perception, factor) in [
            (0, 100),
            (2, 100),
            (3, 97),
            (4, 94),
            (5, 90),
            (6, 85),
            (7, 79),
            (8, 74),
            (9, 70),
            (10, 67),
            (11, 65),
            (12, 65),
        ] {
            assert_eq!(perception_factor(perception), factor);
        }
    }

    /// Bearings split into a 120-degree front, two sides and a rear, following torso twist.
    #[test]
    fn arcs_follow_heading_and_torso_twist() {
        let facing = Facing::default();
        for (bearing, arc) in [
            (0.0, SensorArc::Front),
            (60.0, SensorArc::Front),
            (90.0, SensorArc::Side),
            (120.0, SensorArc::Side),
            (180.0, SensorArc::Rear),
            (300.0, SensorArc::Front),
        ] {
            assert_eq!(SensorArc::from_bearing(bearing, 0.0, facing).unwrap(), arc);
        }
        assert!(SensorArc::from_bearing(f64::NAN, 0.0, facing).is_err());
    }
}
