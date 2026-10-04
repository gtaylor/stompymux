//! Conventional jump capacity and continuous flight geometry, independent of command authorization.
use super::{BattleSystem, BattleUnit, Point};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// Current jump performance at one map gravity, before launch and landing restrictions.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BattleJumpCapacity {
    pub speed: f64,
    /// Whole movement points bound both horizontal range and endpoint elevation difference.
    pub movement_points: u16,
}

impl BattleJumpCapacity {
    /// Validate a capacity before it can be used by a saved flight cursor.
    pub(crate) fn validate(self) -> Result<()> {
        ensure!(
            self.speed.is_finite()
                && self.speed >= 0.0
                && self.speed / 10.75 < f64::from(u16::MAX) + 1.0
                && (self.speed / 10.75).floor() as u16 == self.movement_points,
            "Invalid jump capacity"
        );
        Ok(())
    }

    /// Derive whole movement points only within the flight model's representable range.
    pub(super) fn from_speed(speed: f64) -> Result<Self> {
        let capacity = Self {
            speed,
            movement_points: (speed / 10.75).floor() as u16,
        };
        capacity.validate()?;
        Ok(capacity)
    }
}

impl BattleUnit {
    /// Template thrust minus effective jet losses; gravity is independent of the special-rules flag.
    /// This is capacity, not permission to jump while shut down, prone or otherwise restricted.
    pub fn jump_capacity(&self, gravity: i64) -> Result<BattleJumpCapacity> {
        ensure!((0..=255).contains(&gravity), "Invalid jump gravity");
        let speed = if self.is_destroyed() {
            0.0
        } else {
            self.propulsion.jump(
                self.definition().jump_speed,
                usize::from(self.system_hits(BattleSystem::JumpJet)),
            ) * 100.0
                / gravity.max(50) as f64
        };
        BattleJumpCapacity::from_speed(speed)
    }
}

/// Route geometry in normalized hex-height coordinates and terrain elevation levels.
/// This value neither moves a unit nor promises an unobstructed route or a successful landing.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "JumpPathDefinition", into = "JumpPathDefinition")]
pub struct BattleJumpPath {
    start: Point,
    end: Point,
    start_elevation: f64,
    end_elevation: f64,
    distance: f64,
    apex: u16,
    movement_points: u16,
    projection: Option<JumpProjection>,
    target_range: Option<f64>,
    /// The route begins at an already committed airborne sample.
    continuation: bool,
}

/// Bearing/range admission precedes destination-center snapping in player launches.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JumpProjection {
    bearing: i32,
    range: f64,
}

/// Persist route inputs; derived distance and apex are reconstructed and validated.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JumpPathDefinition {
    start: Point,
    end: Point,
    start_elevation: f64,
    end_elevation: i16,
    movement_points: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    projection: Option<JumpProjection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target_range: Option<f64>,
    #[serde(default)]
    continuation: bool,
}

impl From<BattleJumpPath> for JumpPathDefinition {
    fn from(path: BattleJumpPath) -> Self {
        Self {
            start: path.start,
            end: path.end,
            start_elevation: path.start_elevation,
            end_elevation: path.end_elevation as i16,
            movement_points: path.movement_points,
            projection: path.projection,
            target_range: path.target_range,
            continuation: path.continuation,
        }
    }
}

impl TryFrom<JumpPathDefinition> for BattleJumpPath {
    type Error = anyhow::Error;

    fn try_from(value: JumpPathDefinition) -> Result<Self> {
        ensure!(
            value.projection.is_none() || value.target_range.is_none(),
            "Jump cannot have two range admission modes"
        );
        if value.continuation {
            ensure!(
                value.projection.is_none() && value.target_range.is_none(),
                "A jump continuation cannot repeat launch admission"
            );
            return Self::continuation(
                BattleJumpSample {
                    point: value.start,
                    elevation: value.start_elevation,
                },
                value.end,
                value.end_elevation,
                value.movement_points,
            );
        }
        ensure!(
            value.start_elevation.is_finite()
                && value.start_elevation.fract() == 0.0
                && (f64::from(i16::MIN)..=f64::from(i16::MAX)).contains(&value.start_elevation),
            "Invalid takeoff elevation"
        );
        if let Some(range) = value.target_range {
            return Self::targeted(
                value.start,
                value.end,
                range,
                value.start_elevation as i16,
                value.end_elevation,
                value.movement_points,
            );
        }
        if let Some(projection) = value.projection {
            let path = Self::projected(
                value.start,
                projection.bearing,
                projection.range,
                value.start_elevation as i16,
                value.end_elevation,
                value.movement_points,
            )?;
            ensure!(
                path.end == value.end,
                "Saved jump destination differs from its projection"
            );
            return Ok(path);
        }
        Self::new(
            value.start,
            value.end,
            value.start_elevation as i16,
            value.end_elevation,
            value.movement_points,
        )
    }
}

/// An airborne sample; elevation is deliberately separate from the ground hex's surface.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BattleJumpSample {
    pub point: Point,
    pub elevation: f64,
}

impl BattleJumpPath {
    /// Plan a nonzero center-directed jump within current horizontal and vertical capacity.
    /// Map bounds, same-hex rejection, terrain, crew state and flight events belong to the caller.
    pub fn new(
        start: Point,
        end: Point,
        start_elevation: i16,
        end_elevation: i16,
        movement_points: u16,
    ) -> Result<Self> {
        Self::build(
            BattleJumpSample {
                point: start,
                elevation: f64::from(start_elevation),
            },
            end,
            end_elevation,
            movement_points,
            None,
            None,
            false,
        )
    }

    /// Admit the requested range before snapping to its destination hex center.
    /// The actual flight may be slightly longer; the apex uses the original requested range.
    pub fn projected(
        start: Point,
        bearing: i32,
        range: f64,
        start_elevation: i16,
        end_elevation: i16,
        movement_points: u16,
    ) -> Result<Self> {
        ensure!(
            range.is_finite() && range > 0.0 && range <= f64::from(movement_points),
            "Jump destination is out of range"
        );
        let end = start
            .project(f64::from(bearing), range)?
            .containing_hex()?
            .center();
        Self::build(
            BattleJumpSample {
                point: start,
                elevation: f64::from(start_elevation),
            },
            end,
            end_elevation,
            movement_points,
            Some(JumpProjection { bearing, range }),
            None,
            false,
        )
    }

    /// Fix a target hex center while using launch-time target range for admission and apex.
    pub fn targeted(
        start: Point,
        end: Point,
        range: f64,
        start_elevation: i16,
        end_elevation: i16,
        movement_points: u16,
    ) -> Result<Self> {
        ensure!(
            end.containing_hex()?.center() == end,
            "Targeted jump destination must be a hex center"
        );
        Self::build(
            BattleJumpSample {
                point: start,
                elevation: f64::from(start_elevation),
            },
            end,
            end_elevation,
            movement_points,
            None,
            Some(range),
            false,
        )
    }

    /// Plan the remaining route from an exact airborne point and fractional altitude.
    /// Descent may exceed launch elevation capacity because the unit is already aloft;
    /// horizontal range and further climbing remain bounded by the supplied capacity.
    /// This only constructs geometry; the caller retains flight time, intent and collisions.
    pub fn continuation(
        start: BattleJumpSample,
        end: Point,
        end_elevation: i16,
        movement_points: u16,
    ) -> Result<Self> {
        Self::build(start, end, end_elevation, movement_points, None, None, true)
    }

    /// Build cached geometry from validated exact-destination or projected launch inputs.
    fn build(
        start: BattleJumpSample,
        end: Point,
        end_elevation: i16,
        movement_points: u16,
        projection: Option<JumpProjection>,
        target_range: Option<f64>,
        continuation: bool,
    ) -> Result<Self> {
        ensure!(
            start.elevation.is_finite()
                && (f64::from(i16::MIN)..=f64::from(i16::MAX)).contains(&start.elevation),
            "Invalid jump start altitude"
        );
        let distance = start.point.range(end)?;
        let range = target_range
            .unwrap_or_else(|| projection.map_or(distance, |projection| projection.range));
        ensure!(
            range.is_finite() && range > 0.0,
            "Invalid jump admission range"
        );
        ensure!(
            distance > 0.0 && movement_points > 0,
            "Jump needs a destination and functioning jets"
        );
        ensure!(
            range <= f64::from(movement_points),
            "Jump destination is out of range"
        );
        ensure!(
            if continuation {
                f64::from(end_elevation) - start.elevation <= f64::from(movement_points)
            } else {
                (start.elevation - f64::from(end_elevation)).abs() <= f64::from(movement_points)
            },
            "Jump elevation difference exceeds capacity"
        );
        let apex = (f64::from(movement_points) + 1.0 - range / 3.0)
            .min(2.0 * range + 2.0)
            .floor() as u16;
        Ok(Self {
            start: start.point,
            end,
            start_elevation: start.elevation,
            end_elevation: f64::from(end_elevation),
            distance,
            apex,
            movement_points,
            projection,
            target_range,
            continuation,
        })
    }

    /// Whether this segment begins in flight rather than at a new takeoff.
    pub(super) fn is_continuation(self) -> bool {
        self.continuation
    }

    /// Whole-degree direction of the committed route between its endpoints.
    pub fn heading(self) -> Result<u16> {
        Ok(self
            .start
            .bearing(self.end)?
            .unwrap_or(180.0)
            .round()
            .rem_euclid(360.0) as u16)
    }

    /// Straight horizontal distance from the segment origin to its destination.
    pub fn distance(self) -> f64 {
        self.distance
    }

    /// Additional midpoint elevation above the straight line between endpoint heights.
    pub fn apex(self) -> u16 {
        self.apex
    }

    /// Launch-time whole movement points; subsequent thrust loss does not rewrite the path.
    pub fn movement_points(self) -> u16 {
        self.movement_points
    }

    /// Sample the curved flight path, including its exact takeoff and landing endpoints.
    /// Lost thrust can switch the curve from quartic to quadratic during a flight.
    pub fn sample(self, progress: f64, current_movement_points: u16) -> Result<BattleJumpSample> {
        ensure!(
            progress.is_finite() && (0.0..=1.0).contains(&progress),
            "Invalid jump progress"
        );
        if progress == 0.0 {
            return Ok(BattleJumpSample {
                point: self.start,
                elevation: self.start_elevation,
            });
        }
        if progress == 1.0 {
            return Ok(BattleJumpSample {
                point: self.end,
                elevation: self.end_elevation,
            });
        }
        let midpoint = (progress - 0.5) * 2.0;
        let curve = if u32::from(self.apex) > u32::from(current_movement_points) {
            midpoint.powi(2)
        } else {
            midpoint.powi(4)
        };
        Ok(BattleJumpSample {
            point: Point {
                x: self.start.x + (self.end.x - self.start.x) * progress,
                y: self.start.y + (self.end.y - self.start.y) * progress,
            },
            elevation: self.start_elevation
                + (self.end_elevation - self.start_elevation) * progress
                + (1.0 - curve) * f64::from(self.apex),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Redirection geometry must preserve the committed point and fractional altitude exactly.
    #[test]
    fn airborne_continuation_preserves_samples_through_restart() {
        let start = BattleJumpSample {
            point: Point { x: 1.25, y: 2.5 },
            elevation: 7.125,
        };
        let end = start.point.project(90.0, 2.0).unwrap();
        let path = BattleJumpPath::continuation(start, end, 0, 3).unwrap();
        assert_eq!(path.sample(0.0, 3).unwrap(), start);
        assert_eq!(
            path.sample(1.0, 3).unwrap(),
            BattleJumpSample {
                point: end,
                elevation: 0.0
            }
        );
        let saved = serde_json::to_value(path).unwrap();
        assert_eq!(saved["start_elevation"], 7.125);
        let restored: BattleJumpPath = serde_json::from_value(saved).unwrap();
        assert_eq!(restored, path);
        for thrust in [0, 1, 3] {
            for progress in [0.0, 0.125, 0.5, 0.875, 1.0] {
                assert_eq!(
                    restored.sample(progress, thrust).unwrap(),
                    path.sample(progress, thrust).unwrap()
                );
            }
        }
        let mut flight = super::super::BattleJumpFlight::new(path);
        assert_eq!(flight.sample(), start);
        let step = flight
            .advance(BattleJumpCapacity::from_speed(32.25).unwrap(), 100)
            .unwrap();
        assert_eq!(step.from, start);
        assert!(flight.travelled() > 0.0);
        let replay: super::super::BattleJumpFlight =
            serde_json::from_value(serde_json::to_value(flight).unwrap()).unwrap();
        assert_eq!(replay, flight);
    }

    /// Airborne descent does not waive range, climb or coordinate representability limits.
    #[test]
    fn airborne_continuation_retains_admission_bounds() {
        let start = BattleJumpSample {
            point: Point { x: 0.0, y: 0.0 },
            elevation: 8.25,
        };
        let end = start.point.project(90.0, 2.0).unwrap();
        assert!(BattleJumpPath::continuation(start, end, 0, 3).is_ok());
        assert!(BattleJumpPath::new(start.point, end, 8, 0, 3).is_err());
        assert!(BattleJumpPath::continuation(start, end, 12, 3).is_err());
        assert!(BattleJumpPath::continuation(start, end, 0, 1).is_err());
        assert!(BattleJumpPath::continuation(start, start.point, 0, 3).is_err());
        for elevation in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            32768.0,
            -32769.0,
        ] {
            assert!(
                BattleJumpPath::continuation(BattleJumpSample { elevation, ..start }, end, 0, 3)
                    .is_err()
            );
        }
    }

    /// Saved normal launches cannot opt into fractional takeoff or conflicting admission modes.
    #[test]
    fn saved_jump_origin_modes_are_validated() {
        let start = Point { x: 1.0, y: 2.0 };
        let path = BattleJumpPath::projected(start, 90, 2.0, 0, 0, 3).unwrap();
        let original = serde_json::to_value(path).unwrap();
        for elevation in [0.25, 32768.0, -32769.0] {
            let mut saved = original.clone();
            saved["start_elevation"] = elevation.into();
            assert!(serde_json::from_value::<BattleJumpPath>(saved).is_err());
        }
        let mut saved = original;
        saved["continuation"] = true.into();
        assert!(serde_json::from_value::<BattleJumpPath>(saved).is_err());
    }
}
