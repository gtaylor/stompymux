//! Restartable committed-second jump progression; the enclosing action owns collisions and landing.
use super::{JumpCapacity, JumpPath, JumpSample};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// A flight cursor retaining the last sampled thrust, so damage between ticks does not move it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "JumpFlightRecord")]
pub struct JumpFlight {
    path: JumpPath,
    /// Explicit scenario position retained until the next committed flight sample.
    #[serde(default)]
    relocated: Option<JumpSample>,
    travelled: f64,
    /// Horizontal distance completed on earlier segments of this same flight.
    #[serde(default)]
    completed_distance: f64,
    /// Finish at the committed position on the next simulation step.
    #[serde(default)]
    landing_requested: bool,
    sampled_movement_points: u16,
    #[serde(default)]
    dfa_target: Option<crate::ObjectId>,
    /// This route was admitted on another map; subsequent steps use current boundary rules.
    #[serde(default)]
    reassigned: bool,
}

/// Stored cursor inputs are validated before they can produce an airborne coordinate.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JumpFlightRecord {
    path: JumpPath,
    /// Explicit scenario position retained until the next committed flight sample.
    #[serde(default)]
    relocated: Option<JumpSample>,
    travelled: f64,
    /// Horizontal distance completed on earlier segments of this same flight.
    #[serde(default)]
    completed_distance: f64,
    /// Finish at the committed position on the next simulation step.
    #[serde(default)]
    landing_requested: bool,
    sampled_movement_points: u16,
    #[serde(default)]
    dfa_target: Option<crate::ObjectId>,
    /// This route was admitted on another map; subsequent steps use current boundary rules.
    #[serde(default)]
    reassigned: bool,
}

impl TryFrom<JumpFlightRecord> for JumpFlight {
    type Error = anyhow::Error;

    fn try_from(record: JumpFlightRecord) -> Result<Self> {
        ensure!(
            record.travelled.is_finite()
                && (0.0..=record.path.distance()).contains(&record.travelled),
            "Invalid saved jump distance"
        );
        ensure!(
            record.completed_distance.is_finite()
                && record.completed_distance >= 0.0
                && (record.completed_distance + record.path.distance()).is_finite(),
            "Invalid completed jump distance"
        );
        if let Some(sample) = record.relocated {
            sample.point.containing_hex()?;
            ensure!(
                sample.elevation.is_finite()
                    && (f64::from(i16::MIN)..=f64::from(i16::MAX)).contains(&sample.elevation),
                "Invalid relocated jump altitude"
            );
        }
        Ok(Self {
            relocated: record.relocated,
            path: record.path,
            travelled: record.travelled,
            completed_distance: record.completed_distance,
            landing_requested: record.landing_requested,
            sampled_movement_points: record.sampled_movement_points,
            dfa_target: record.dfa_target,
            reassigned: record.reassigned,
        })
    }
}

/// The enclosing game transaction must handle these outcomes before committing the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JumpOutcome {
    Airborne,
    Landing,
    LostThrust,
}

/// Segment traversed during one tick, available for collision checks before committing movement.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[must_use = "Resolve collisions, landing or lost thrust in the same transaction as this step"]
pub struct JumpStep {
    pub from: JumpSample,
    pub to: JumpSample,
    pub outcome: JumpOutcome,
}

impl JumpFlight {
    /// Begin at the validated takeoff point without consuming time or altering the launch geometry.
    pub fn new(path: JumpPath) -> Self {
        Self {
            path,
            relocated: None,
            travelled: 0.0,
            completed_distance: 0.0,
            landing_requested: false,
            sampled_movement_points: path.movement_points(),
            dfa_target: None,
            reassigned: false,
        }
    }

    /// Attach the target identity to this flight so cancellation also discards the intent.
    pub(super) fn with_dfa_target(mut self, target: crate::ObjectId) -> Self {
        self.dfa_target = Some(target);
        self
    }

    /// Landing target fixed at launch; its later movement does not change the flight path.
    pub fn dfa_target(self) -> Option<crate::ObjectId> {
        self.dfa_target
    }

    /// Immutable path shared by flight progression and prospective collision checks.
    pub fn path(self) -> JumpPath {
        self.path
    }

    /// Horizontal distance completed, bounded by the path length.
    pub fn travelled(self) -> f64 {
        self.travelled
    }

    /// Total horizontal progress across all segments, including administrative redirection.
    pub fn total_travelled(self) -> f64 {
        self.completed_distance + self.travelled
    }

    /// Total completed and planned horizontal distance for the current course.
    pub fn total_distance(self) -> f64 {
        self.completed_distance + self.path.distance()
    }

    /// Replace only the remaining route, preserving the committed sample, progress and DFA intent.
    /// The caller must admit the new route on the current map before publishing this cursor.
    pub fn redirect(&mut self, path: JumpPath) -> Result<()> {
        ensure!(
            !self.arrived(),
            "Jump flight has already reached its destination"
        );
        ensure!(
            path.sample(0.0, self.sampled_movement_points)? == self.sample(),
            "Redirected jump must begin at the committed airborne sample"
        );
        let completed_distance = self.total_travelled();
        ensure!(
            (completed_distance + path.distance()).is_finite(),
            "Jump distance overflow"
        );
        self.path = path;
        self.completed_distance = completed_distance;
        self.travelled = 0.0;
        self.relocated = None;
        self.landing_requested = false;
        self.reassigned = false;
        Ok(())
    }

    /// Whether a length edit has requested landing at the current committed position.
    pub(super) fn landing_requested(self) -> bool {
        self.landing_requested
    }

    /// Schedule existing landing handling without moving the cursor or consuming a tick.
    pub(super) fn request_landing(&mut self) {
        self.landing_requested = true;
    }

    /// Last committed airborne point and height; inspecting it never advances the flight.
    pub fn sample(self) -> JumpSample {
        if let Some(sample) = self.relocated {
            return sample;
        }
        self.path
            .sample(
                self.travelled / self.path.distance(),
                self.sampled_movement_points,
            )
            .expect("validated jump cursor")
    }

    /// Administrative relocation preserves the jump path, thrust sample and progress.
    pub(super) fn relocate(&mut self, point: super::Point, elevation: f64) {
        self.relocated = Some(JumpSample { point, elevation });
    }

    /// Rebind a scenario-transferred route while preserving its exact sampled altitude and progress.
    /// Compatible routes need no override; incompatible routes settle boundaries during movement.
    pub(super) fn rebind(&mut self, map: &super::StoredMap, point: super::Point) -> Result<bool> {
        if super::jumping::validate_route(map, self.path).is_ok() {
            return Ok(false);
        }
        let elevation = self.sample().elevation;
        self.relocated = Some(JumpSample { point, elevation });
        self.reassigned = true;
        Ok(true)
    }

    /// Ordinary routes retain launch admission checks; reassigned routes resolve edges during updates.
    pub(super) fn validate_on_map(self, map: &super::StoredMap) -> Result<()> {
        if !self.reassigned {
            super::jumping::validate_route(map, self.path)?;
        }
        Ok(())
    }

    /// Whether the endpoint has been reached; landing consequences remain caller-owned.
    pub fn arrived(self) -> bool {
        self.travelled == self.path.distance()
    }

    /// Advance one second using current gravity-adjusted thrust and the map's movement percentage.
    /// Nonpositive movement percentages use the standard rate, as in ground movement.
    /// A requested landing returns the current sample for ordinary landing resolution.
    /// Invalid inputs and complete flights leave the cursor unchanged; absent thrust returns
    /// a stationary segment so the caller can apply a fall at the last airborne position.
    pub fn advance(&mut self, capacity: JumpCapacity, movement_modifier: i64) -> Result<JumpStep> {
        ensure!(
            !self.arrived(),
            "Jump flight has already reached its destination"
        );
        capacity.validate()?;
        let from = self.sample();
        if self.landing_requested {
            return Ok(JumpStep {
                from,
                to: from,
                outcome: JumpOutcome::Landing,
            });
        }
        if capacity.speed == 0.0 {
            return Ok(JumpStep {
                from,
                to: from,
                outcome: JumpOutcome::LostThrust,
            });
        }
        let rate = if movement_modifier > 0 {
            movement_modifier as f64 / 100.0
        } else {
            1.0
        };
        let distance = capacity.speed / 645.0 * rate;
        let remaining = self.path.distance() - self.travelled;
        // Avoid adding an extra tick solely for accumulated floating-point roundoff.
        let arrived = remaining <= distance + self.path.distance() * f64::EPSILON * 16.0;
        let travelled = if arrived {
            self.path.distance()
        } else {
            self.travelled + distance
        };
        let candidate = Self {
            relocated: None,
            path: self.path,
            travelled,
            completed_distance: self.completed_distance,
            landing_requested: self.landing_requested,
            sampled_movement_points: capacity.movement_points,
            dfa_target: self.dfa_target,
            reassigned: self.reassigned,
        };
        let step = JumpStep {
            from,
            to: candidate.sample(),
            outcome: if arrived {
                JumpOutcome::Landing
            } else {
                JumpOutcome::Airborne
            },
        };
        *self = candidate;
        Ok(step)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::Point;

    /// Course replacement retains the exact cursor, cumulative distance and attack intent.
    #[test]
    fn redirect_retains_progress_and_replays_the_shared_integrator() {
        let start = Point { x: 4.0, y: 4.0 };
        let path = JumpPath::new(start, start.project(0.0, 3.0).unwrap(), 0, 0, 4).unwrap();
        let capacity = JumpCapacity::from_speed(43.0).unwrap();
        let mut flight = JumpFlight::new(path).with_dfa_target(crate::ObjectId(99));
        for _ in 0..12 {
            let step = flight.advance(capacity, 100).unwrap();
            assert_ne!(step.outcome, JumpOutcome::LostThrust);
        }
        for heading in [90.0, 180.0] {
            let before = flight;
            let origin = before.sample();
            let end = origin.point.project(heading, 2.0).unwrap();
            let path = JumpPath::continuation(origin, end, 0, 4).unwrap();
            flight.redirect(path).unwrap();
            assert_eq!(flight.sample(), origin);
            assert_eq!(flight.total_travelled(), before.total_travelled());
            assert_eq!(
                flight.total_distance(),
                before.total_travelled() + path.distance()
            );
            assert_eq!(flight.dfa_target(), before.dfa_target());
            assert_eq!(
                flight.sampled_movement_points,
                before.sampled_movement_points
            );
            let mut restored: JumpFlight =
                serde_json::from_value(serde_json::to_value(flight).unwrap()).unwrap();
            let next = flight.advance(capacity, 100).unwrap();
            assert_eq!(next.from, origin);
            assert_eq!(restored.advance(capacity, 100).unwrap(), next);
            assert_eq!(restored, flight);
            assert!(flight.total_travelled() > before.total_travelled());
        }
        for _ in 0..100 {
            if flight.arrived() {
                break;
            }
            let step = flight.advance(capacity, 100).unwrap();
            assert_ne!(step.outcome, JumpOutcome::LostThrust);
        }
        assert!(flight.arrived());
        assert_eq!(flight.total_travelled(), flight.total_distance());
        assert_eq!(flight.sample().elevation, 0.0);
    }

    /// Invalid replacement and saved progress must not move the live cursor.
    #[test]
    fn redirect_rejects_discontinuous_start_and_invalid_saved_progress() {
        let start = Point { x: 4.0, y: 4.0 };
        let path = JumpPath::new(start, start.project(0.0, 3.0).unwrap(), 0, 0, 4).unwrap();
        let mut flight = JumpFlight::new(path);
        let step = flight
            .advance(JumpCapacity::from_speed(43.0).unwrap(), 100)
            .unwrap();
        assert_eq!(step.outcome, JumpOutcome::Airborne);
        let before = flight;
        assert!(flight.redirect(path).is_err());
        assert_eq!(flight, before);
        let mut saved = serde_json::to_value(flight).unwrap();
        saved["completed_distance"] = (-1).into();
        assert!(serde_json::from_value::<JumpFlight>(saved).is_err());
    }
}
