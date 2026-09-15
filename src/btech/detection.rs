//! Optical contact acquisition rolls with explicit scenario inputs and transactional unit dice.
use super::{BattleDice, BattleSensorMode, BattleSensorReport};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::sync::Arc;

/// Observer-relative sensor direction; this is independent of target hit-location arcs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleSensorArc {
    Front,
    Side,
    Rear,
}

/// Scenario inputs not yet supplied by persistent team, skill and sensor-selection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleDetectionRules {
    pub arc: BattleSensorArc,
    pub perception: i16,
    pub hostile: bool,
    pub hidden: bool,
    pub secondary: bool,
}

/// A resolved attempt. Acquiring contact state and publishing notifications belong to the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleDetection {
    pub detected: bool,
    /// Strict upper bound for a successful 1–10,000 roll; not an inclusive percentage.
    pub threshold: u16,
    /// Absent for ineligible targets and automatic close-range detection.
    pub roll: Option<u16>,
}

/// Sensor query and acquisition inputs for a supported Mech or vehicle target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleSensorAttempt {
    pub sensor: BattleSensorMode,
    pub target_lit: bool,
    pub disabled: bool,
    pub rules: BattleDetectionRules,
}

impl BattleDetectionRules {
    /// Hostile-target perception factor derived from the padded 2d6 success table.
    pub fn perception_factor(self) -> u16 {
        let successes = match self.perception {
            i16::MIN..=2 => 36,
            3 => 33,
            4 => 30,
            5 => 26,
            6 => 21,
            7 => 15,
            8 => 10,
            9 => 6,
            10 => 3,
            11 => 1,
            12..=i16::MAX => 1,
        };
        64 + successes
    }
}

impl BattleSensorReport {
    /// Roll with Mech direction weights after validating all inputs.
    /// World acquisition actions supply vehicle weights when appropriate.
    /// Rejected and automatic attempts preserve the stream.
    pub fn roll_detection(
        self,
        dice: &mut BattleDice,
        distance: f64,
        rules: BattleDetectionRules,
    ) -> Result<BattleDetection> {
        self.roll_detection_with_base(dice, distance, rules, mech_arc_base(rules.arc))
    }

    /// Resolve the shared probability formula with the observer class's directional factor.
    fn roll_detection_with_base(
        self,
        dice: &mut BattleDice,
        distance: f64,
        rules: BattleDetectionRules,
        arc: u16,
    ) -> Result<BattleDetection> {
        ensure!(
            distance.is_finite() && distance >= 0.0,
            "Invalid sensor distance"
        );
        ensure!(
            self.acquisition_factor <= 101,
            "Invalid optical acquisition factor"
        );
        let mut result = BattleDetection {
            detected: false,
            threshold: 0,
            roll: None,
        };
        if !self.eligible
            || self.acquisition_factor == 0
            || (rules.hidden && rules.hostile && distance > 5.0 && self.acquisition_factor <= 100)
        {
            return Ok(result);
        }
        let mut base =
            arc * if rules.hostile {
                rules.perception_factor()
            } else {
                100
            } / 100;
        if rules.hidden && rules.hostile && self.acquisition_factor <= 100 {
            base /= 4;
        }
        result.threshold =
            base * u16::from(self.acquisition_factor) / if rules.secondary { 2 } else { 1 };
        if distance < 3.0 {
            result.detected = true;
            return Ok(result);
        }
        let roll = dice.die(10_000)?;
        result.roll = Some(roll);
        result.detected = roll < result.threshold;
        Ok(result)
    }
}

/// Evaluate saved map conditions before consuming the observer's durable dice stream.
/// This is a domain action, not a player reroll API; its caller commits the returned detection.
pub fn roll_optical_detection(
    world: &mut World,
    observer: ObjectId,
    target: ObjectId,
    attempt: BattleSensorAttempt,
) -> Result<BattleDetection> {
    let report = super::map_optical_contact(
        world,
        observer,
        target,
        attempt.sensor,
        attempt.target_lit,
        attempt.disabled,
    )?;
    let range = super::unit_range(world, observer, target)?;
    let base = observer_arc_base(
        world,
        observer,
        attempt.rules.arc,
        range.bearing.unwrap_or(180.0),
    )?;
    let mut dice = observer_dice(world, observer)?.clone();
    let result = report.roll_detection_with_base(&mut dice, range.spatial, attempt.rules, base)?;
    *observer_dice_mut(world, observer)? = dice;
    Ok(result)
}

/// Target facts needed by a conventional optical scan until team/light/hidden state is owned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleScanTarget {
    pub lit: bool,
    pub hostile: bool,
    pub hidden: bool,
}

/// Paired optical modes and availability supplied by the scanner's caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleSensorScan {
    pub primary: BattleSensorMode,
    pub secondary: BattleSensorMode,
    pub visual_disabled: bool,
    pub amplification_disabled: bool,
    pub perception: i16,
    pub target: BattleScanTarget,
}

/// Ordered attempts; an absent secondary means it was unnecessary or duplicated the primary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleSensorScanReport {
    pub primary: BattleDetection,
    pub secondary: Option<BattleDetection>,
    pub detected_by: Option<BattleSensorMode>,
}

impl BattleSensorArc {
    /// Determine scanner direction from continuous compass bearing and torso-adjusted heading.
    pub fn from_bearing(bearing: f64, heading: f64, facing: super::BattleFacing) -> Result<Self> {
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

/// Scan one target in primary/secondary order, committing observer dice only after the entire operation succeeds.
/// Facing and map conditions come from the world; contact storage and scan cadence remain caller responsibilities.
pub fn scan_optical_target(
    world: &mut World,
    observer: ObjectId,
    target: ObjectId,
    scan: BattleSensorScan,
) -> Result<BattleSensorScanReport> {
    let range = super::unit_range(world, observer, target)?;
    let bearing = range.bearing.unwrap_or(180.0);
    let (heading, facing) = if let Some(vehicle) = world.btech.vehicles().get(&observer) {
        (
            vehicle.motion().context("Vehicle is not placed")?.heading,
            super::BattleFacing::default(),
        )
    } else {
        let unit = world
            .btech
            .constructed_units()
            .get(&observer)
            .context("Unit construction state is unavailable")?;
        (
            unit.motion().context("Unit is not placed")?.heading,
            unit.facing(),
        )
    };
    let arc = if world.btech.vehicles().contains_key(&observer) {
        BattleSensorArc::from_bearing(bearing.round(), heading.trunc(), facing)?
    } else {
        BattleSensorArc::from_bearing(bearing, heading, facing)?
    };
    let base = observer_arc_base(world, observer, arc, bearing)?;
    let rules = BattleDetectionRules {
        arc,
        perception: scan.perception,
        hostile: scan.target.hostile,
        hidden: scan.target.hidden,
        secondary: false,
    };
    let query = |sensor| {
        let disabled = match sensor {
            BattleSensorMode::Infrared
            | BattleSensorMode::Seismic
            | BattleSensorMode::Electromagnetic
            | BattleSensorMode::Radar
            | BattleSensorMode::BeagleProbe
            | BattleSensorMode::LightProbe
            | BattleSensorMode::BloodhoundProbe => false,
            BattleSensorMode::Visual => scan.visual_disabled,
            BattleSensorMode::LightAmplification => scan.amplification_disabled,
        };
        super::map_optical_contact(world, observer, target, sensor, scan.target.lit, disabled)
    };
    let first = query(scan.primary)?;
    let second = if scan.primary == scan.secondary {
        None
    } else {
        Some(query(scan.secondary)?)
    };
    let mut dice = observer_dice(world, observer)?.clone();
    let primary = first.roll_detection_with_base(&mut dice, range.spatial, rules, base)?;
    let mut report = BattleSensorScanReport {
        primary,
        secondary: None,
        detected_by: primary.detected.then_some(scan.primary),
    };
    if !primary.detected
        && let Some(second) = second
    {
        let secondary = second.roll_detection_with_base(
            &mut dice,
            range.spatial,
            BattleDetectionRules {
                secondary: true,
                ..rules
            },
            base,
        )?;
        report.secondary = Some(secondary);
        report.detected_by = secondary.detected.then_some(scan.secondary);
    }
    *observer_dice_mut(world, observer)? = dice;
    Ok(report)
}

/// Mech acquisition weights after torso-relative direction classification.
fn mech_arc_base(arc: BattleSensorArc) -> u16 {
    match arc {
        BattleSensorArc::Front => 100,
        BattleSensorArc::Side => 70,
        BattleSensorArc::Rear => 40,
    }
}

/// Vehicles use hull-relative weights; original turret construction supplies the arc bonus.
fn observer_arc_base(
    world: &World,
    observer: ObjectId,
    arc: BattleSensorArc,
    bearing: f64,
) -> Result<u16> {
    let Some(vehicle) = world.btech.vehicles().get(&observer) else {
        return Ok(mech_arc_base(arc));
    };
    let base = match arc {
        BattleSensorArc::Front => 100,
        BattleSensorArc::Side => 80,
        BattleSensorArc::Rear => 50,
    };
    ensure!(bearing.is_finite(), "Invalid sensor direction");
    let turret = vehicle
        .definition()
        .sections
        .get(&super::BattleVehicleSection::Turret)
        .is_some_and(|section| section.internal > 0)
        && {
            let heading =
                vehicle.motion().context("Vehicle is not placed")?.heading + vehicle.turret_offset;
            let angle = (bearing.rem_euclid(360.0).round() - heading.rem_euclid(360.0).trunc())
                .rem_euclid(360.0);
            angle <= 30.0 || angle >= 330.0
        };
    Ok(base + if turret { 15 } else { 0 })
}

/// Borrow only the observer's stream, regardless of its construction class.
fn observer_dice(world: &World, observer: ObjectId) -> Result<&BattleDice> {
    if let Some(vehicle) = world.btech.vehicles().get(&observer) {
        return Ok(&vehicle.dice);
    }
    Ok(&world
        .btech
        .constructed_units()
        .get(&observer)
        .context("Unit construction state is unavailable")?
        .dice)
}

/// Commit a validated acquisition without replacing unrelated unit state.
fn observer_dice_mut(world: &mut World, observer: ObjectId) -> Result<&mut BattleDice> {
    if world.btech.vehicles().contains_key(&observer) {
        return Ok(&mut Arc::make_mut(&mut world.btech.vehicles)
            .get_mut(&observer)
            .expect("checked vehicle")
            .dice);
    }
    Ok(&mut Arc::make_mut(&mut world.btech.constructed)
        .get_mut(&observer)
        .context("Unit construction state is unavailable")?
        .dice)
}

#[cfg(test)]
mod tests {
    use super::*;
    const RULES: BattleDetectionRules = BattleDetectionRules {
        arc: BattleSensorArc::Front,
        perception: 7,
        hostile: true,
        hidden: false,
        secondary: false,
    };
    const REPORT: BattleSensorReport = BattleSensorReport {
        eligible: true,
        acquisition_factor: 90,
        aim_modifier: 0,
    };

    #[test]
    fn perception_direction_hidden_and_secondary_modifiers_keep_integer_order() {
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
            assert_eq!(
                BattleDetectionRules {
                    perception,
                    ..RULES
                }
                .perception_factor(),
                factor
            );
        }
        let mut dice = BattleDice::seeded([51; 32]);
        let rules = BattleDetectionRules {
            arc: BattleSensorArc::Side,
            hidden: true,
            secondary: true,
            ..RULES
        };
        // floor(70*79/100)=55, floor(55/4)=13, floor(13*90/2)=585.
        assert_eq!(
            REPORT
                .roll_detection(&mut dice, 5.0, rules)
                .unwrap()
                .threshold,
            585
        );
        let friendly = BattleDetectionRules {
            hostile: false,
            ..rules
        };
        assert_eq!(
            REPORT
                .roll_detection(&mut dice, 5.0, friendly)
                .unwrap()
                .threshold,
            3150
        );
    }

    #[test]
    fn automatic_rejected_and_invalid_attempts_preserve_dice() {
        let mut dice = BattleDice::seeded([17; 32]);
        let before = dice.clone();
        assert!(
            REPORT
                .roll_detection(&mut dice, 2.999, RULES)
                .unwrap()
                .detected
        );
        assert!(
            !REPORT
                .roll_detection(
                    &mut dice,
                    5.001,
                    BattleDetectionRules {
                        hidden: true,
                        ..RULES
                    }
                )
                .unwrap()
                .detected
        );
        assert!(
            !BattleSensorReport {
                eligible: false,
                ..REPORT
            }
            .roll_detection(&mut dice, 1.0, RULES)
            .unwrap()
            .detected
        );
        assert!(REPORT.roll_detection(&mut dice, f64::NAN, RULES).is_err());
        assert_eq!(dice, before);
        let result = REPORT.roll_detection(&mut dice, 3.0, RULES).unwrap();
        let mut expected = before;
        let roll = expected.die(10_000).unwrap();
        assert_eq!(result.roll, Some(roll));
        assert_eq!(result.detected, roll < result.threshold);
        assert_eq!(dice, expected);
    }
    #[test]
    fn a_roll_equal_to_the_threshold_fails() {
        let mut stream = BattleDice::seeded([91; 32]);
        for _ in 0..10_000 {
            let mut candidate = stream.clone();
            let roll = stream.die(10_000).unwrap();
            if !roll.is_multiple_of(100) {
                continue;
            }
            let report = BattleSensorReport {
                acquisition_factor: (roll / 100) as u8,
                ..REPORT
            };
            let result = report
                .roll_detection(
                    &mut candidate,
                    3.0,
                    BattleDetectionRules {
                        hostile: false,
                        ..RULES
                    },
                )
                .unwrap();
            assert_eq!(result.threshold, roll);
            assert_eq!(result.roll, Some(roll));
            assert!(!result.detected);
            return;
        }
        panic!("Seed did not supply an exact threshold fixture");
    }
    #[test]
    fn sensor_direction_uses_torso_offsets_and_inclusive_front_edges() {
        use super::super::{BattleFacing, BattleTorso};
        let facing = BattleFacing::default();
        for bearing in [0.0, 60.0, 300.0, 360.0] {
            assert_eq!(
                BattleSensorArc::from_bearing(bearing, 0.0, facing).unwrap(),
                BattleSensorArc::Front
            );
        }
        for bearing in [60.001, 120.0, 240.0, 299.999] {
            assert_eq!(
                BattleSensorArc::from_bearing(bearing, 0.0, facing).unwrap(),
                BattleSensorArc::Side
            );
        }
        assert_eq!(
            BattleSensorArc::from_bearing(180.0, 0.0, facing).unwrap(),
            BattleSensorArc::Rear
        );
        let facing = BattleFacing {
            torso: BattleTorso::Right,
            arms_flipped: true,
        };
        assert_eq!(
            BattleSensorArc::from_bearing(119.0, 0.0, facing).unwrap(),
            BattleSensorArc::Front
        );
        assert_eq!(
            BattleSensorArc::from_bearing(119.001, 0.0, facing).unwrap(),
            BattleSensorArc::Side
        );
        assert!(BattleSensorArc::from_bearing(f64::NAN, 0.0, facing).is_err());
    }
}
