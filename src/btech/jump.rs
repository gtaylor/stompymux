//! Conventional jump capacity and continuous flight geometry, independent of command authorization.
use super::{Mech, Point, System};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// Current jump performance at one map gravity, before launch and landing restrictions.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct JumpCapacity {
    /// Gravity-scaled horizontal flight speed in km/h.
    pub speed: f64,
    /// Whole Jumping MP, which bound both horizontal range and the climb to the destination.
    pub movement_points: u16,
}

/// Fastest flight speed the cursor can represent; thrust beyond it saturates.
const MAXIMUM_JUMP_SPEED: f64 = 65535.0 * 10.75;

/// Whole Jumping MP for a flight speed: the speed's MP rounded to the nearest whole number,
/// halves down (Tactical Operations p. 53).
fn whole_movement_points(speed: f64) -> u16 {
    (speed / 10.75 - 0.5).ceil().max(0.0) as u16
}

impl JumpCapacity {
    /// Validate a capacity before it can be used by a saved flight cursor.
    pub(crate) fn validate(self) -> Result<()> {
        ensure!(
            self.speed.is_finite()
                && (0.0..=MAXIMUM_JUMP_SPEED).contains(&self.speed)
                && whole_movement_points(self.speed) == self.movement_points,
            "Invalid jump capacity"
        );
        Ok(())
    }

    /// Derive whole movement points only within the flight model's representable range.
    pub(super) fn from_speed(speed: f64) -> Result<Self> {
        let capacity = Self {
            speed,
            movement_points: whole_movement_points(speed),
        };
        capacity.validate()?;
        Ok(capacity)
    }

    /// Scale standard-gravity thrust by the map's gravity (Tactical Operations p. 53): Jumping
    /// MP are divided by the G-rating and rounded to the nearest whole number, halves down,
    /// with no lower bound on the gravity. The thrust itself must fit the flight model; a
    /// gravity of zero is treated as one hundredth of a G so the division stays defined, and
    /// the scaled speed saturates at the model's range.
    fn at_gravity(speed: f64, gravity: i64) -> Result<Self> {
        ensure!((0..=255).contains(&gravity), "Invalid jump gravity");
        Self::from_speed(speed)?;
        Self::from_speed((speed * 100.0 / gravity.max(1) as f64).min(MAXIMUM_JUMP_SPEED))
    }
}

/// Heat from one jump (Total Warfare p. 53 and p. 158): one point for every hex jumped, with
/// a minimum of three. Improved jump jets halve it to one point per two hexes or part
/// thereof, still with the minimum of three. An XXL engine doubles the heat per hex with a
/// minimum of six, and with improved jets the halving and the doubling cancel out (Tactical
/// Operations: Advanced Units & Equipment p. 121). Gravity never changes it.
pub fn jump_heat(hexes: u64, improved: bool, xxl: bool) -> u64 {
    match (improved, xxl) {
        (false, false) => hexes.max(3),
        (true, false) => hexes.div_ceil(2).max(3),
        (false, true) => (hexes * 2).max(6),
        (true, true) => hexes.max(6),
    }
}

/// Gravity modifier to the jump-strain roll (Tactical Operations p. 53): one for every full
/// half G the map differs from standard gravity in either direction.
pub fn jump_strain_modifier(gravity: i64) -> i16 {
    (gravity.abs_diff(100) / 50) as i16
}

impl Mech {
    /// Template thrust minus effective jet losses at `gravity` percent of standard.
    /// This is capacity, not permission to jump while shut down, prone or otherwise restricted.
    pub fn jump_capacity(&self, gravity: i64) -> Result<JumpCapacity> {
        ensure!((0..=255).contains(&gravity), "Invalid jump gravity");
        if self.is_destroyed() {
            return JumpCapacity::from_speed(0.0);
        }
        let speed = self.propulsion.jump(
            self.definition().jump_speed,
            usize::from(self.system_hits(System::JumpJet)),
        );
        JumpCapacity::at_gravity(speed, gravity)
    }

    /// Thrust from the jets a 'Mech standing in Depth 1 water may fire: only torso-mounted
    /// jets, each providing one Jumping MP (Total Warfare p. 53). Leg jets are submerged,
    /// and a quad's front legs count as legs. Administrative thrust overrides do not apply,
    /// since they carry no jet locations.
    pub fn torso_jump_capacity(&self, gravity: i64) -> Result<JumpCapacity> {
        ensure!((0..=255).contains(&gravity), "Invalid jump gravity");
        if self.is_destroyed() {
            return JumpCapacity::from_speed(0.0);
        }
        let legs = self.chassis().legs();
        let improved = self.definition().has_special("ImprovedJJ_Tech");
        let jets = self
            .loadout()?
            .jump_jet_groups(improved)?
            .into_iter()
            .filter(|group| {
                group.iter().all(|location| {
                    !legs.contains(&location.section) && !self.critical_unavailable(*location)
                })
            })
            .count();
        JumpCapacity::at_gravity(jets as f64 * 10.75, gravity)
    }
}

/// Route geometry in normalized hex-height coordinates and terrain elevation levels.
/// This value neither moves a unit nor promises an unobstructed route or a successful landing.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "JumpPathDefinition", into = "JumpPathDefinition")]
pub struct JumpPath {
    start: Point,
    end: Point,
    start_elevation: f64,
    end_elevation: f64,
    distance: f64,
    apex: u16,
    /// Lowest apex that keeps the displayed curve above every crossed hex.
    clearance: u16,
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
    #[serde(default)]
    clearance: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    projection: Option<JumpProjection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target_range: Option<f64>,
    #[serde(default)]
    continuation: bool,
}

impl From<JumpPath> for JumpPathDefinition {
    fn from(path: JumpPath) -> Self {
        Self {
            start: path.start,
            end: path.end,
            start_elevation: path.start_elevation,
            end_elevation: path.end_elevation as i16,
            movement_points: path.movement_points,
            clearance: path.clearance,
            projection: path.projection,
            target_range: path.target_range,
            continuation: path.continuation,
        }
    }
}

impl TryFrom<JumpPathDefinition> for JumpPath {
    type Error = anyhow::Error;

    fn try_from(value: JumpPathDefinition) -> Result<Self> {
        ensure!(
            value.projection.is_none() || value.target_range.is_none(),
            "Jump cannot have two range admission modes"
        );
        let path = if value.continuation {
            ensure!(
                value.projection.is_none() && value.target_range.is_none(),
                "A jump continuation cannot repeat launch admission"
            );
            Self::continuation(
                JumpSample {
                    point: value.start,
                    elevation: value.start_elevation,
                },
                value.end,
                value.end_elevation,
                value.movement_points,
            )?
        } else {
            ensure!(
                value.start_elevation.is_finite()
                    && value.start_elevation.fract() == 0.0
                    && (f64::from(i16::MIN)..=f64::from(i16::MAX)).contains(&value.start_elevation),
                "Invalid takeoff elevation"
            );
            if let Some(range) = value.target_range {
                Self::targeted(
                    value.start,
                    value.end,
                    range,
                    value.start_elevation as i16,
                    value.end_elevation,
                    value.movement_points,
                )?
            } else if let Some(projection) = value.projection {
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
                path
            } else {
                Self::new(
                    value.start,
                    value.end,
                    value.start_elevation as i16,
                    value.end_elevation,
                    value.movement_points,
                )?
            }
        };
        Ok(path.with_clearance(value.clearance))
    }
}

/// An airborne sample; elevation is deliberately separate from the ground hex's surface.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct JumpSample {
    pub point: Point,
    pub elevation: f64,
}

impl JumpPath {
    /// Plan a center-directed jump within current horizontal range and climb capacity.
    /// A jump may end in the hex it starts in (Total Warfare p. 52). Map bounds, terrain,
    /// crew state and flight events belong to the caller.
    pub fn new(
        start: Point,
        end: Point,
        start_elevation: i16,
        end_elevation: i16,
        movement_points: u16,
    ) -> Result<Self> {
        Self::build(
            JumpSample {
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
            range.is_finite() && range >= 0.0 && range <= f64::from(movement_points),
            "Jump destination is out of range"
        );
        let end = start
            .project(f64::from(bearing), range)?
            .containing_hex()?
            .center();
        Self::build(
            JumpSample {
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
        ensure!(
            range > 0.0 || start.containing_hex()? == end.containing_hex()?,
            "Jump destination is out of range"
        );
        Self::build(
            JumpSample {
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
    /// Horizontal range and further climbing remain bounded by the supplied capacity.
    /// This only constructs geometry; the caller retains flight time, intent and landing.
    pub fn continuation(
        start: JumpSample,
        end: Point,
        end_elevation: i16,
        movement_points: u16,
    ) -> Result<Self> {
        Self::build(start, end, end_elevation, movement_points, None, None, true)
    }

    /// Build cached geometry from validated exact-destination or projected launch inputs.
    /// The climb to the destination is bounded by Jumping MP; descent is unlimited (Total
    /// Warfare p. 53). A same-hex jump has no horizontal distance and lands where it began.
    fn build(
        start: JumpSample,
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
            range.is_finite() && range >= 0.0,
            "Invalid jump admission range"
        );
        ensure!(movement_points > 0, "Jump needs functioning jets");
        ensure!(
            !continuation || distance > 0.0,
            "Jump continuation needs a destination"
        );
        ensure!(
            range <= f64::from(movement_points),
            "Jump destination is out of range"
        );
        ensure!(
            f64::from(end_elevation) - start.elevation <= f64::from(movement_points),
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
            clearance: 0,
            movement_points,
            projection,
            target_range,
            continuation,
        })
    }

    /// Raise the apex to at least `clearance`, so the displayed curve clears crossed terrain.
    /// The caller derives the value from the map; a legal route never collides with terrain
    /// (Total Warfare p. 52), so this only shapes the flight's altitude.
    pub fn with_clearance(mut self, clearance: u16) -> Self {
        self.clearance = clearance;
        self
    }

    /// Whole-degree direction of the committed route between its endpoints; a same-hex jump
    /// reports south.
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

    /// Additional midpoint elevation above the straight line between endpoint heights: the
    /// natural apex of the launch, raised to the clearance the crossed terrain needs.
    pub fn apex(self) -> u16 {
        self.apex.max(self.clearance)
    }

    /// Launch-time whole movement points; subsequent thrust loss does not rewrite the path.
    pub fn movement_points(self) -> u16 {
        self.movement_points
    }

    /// Sample the curved flight path, including its exact takeoff and landing endpoints.
    /// Lost thrust can switch the curve from quartic to quadratic during a flight.
    /// A same-hex jump samples its single point at every progress.
    pub fn sample(self, progress: f64, current_movement_points: u16) -> Result<JumpSample> {
        ensure!(
            progress.is_finite() && (0.0..=1.0).contains(&progress),
            "Invalid jump progress"
        );
        if progress == 0.0 || self.distance == 0.0 {
            return Ok(JumpSample {
                point: self.start,
                elevation: self.start_elevation,
            });
        }
        if progress == 1.0 {
            return Ok(JumpSample {
                point: self.end,
                elevation: self.end_elevation,
            });
        }
        // Thrust lost since launch flattens the natural curve; the raised clearance does not.
        let midpoint = (progress - 0.5) * 2.0;
        let curve = if u32::from(self.apex) > u32::from(current_movement_points) {
            midpoint.powi(2)
        } else {
            midpoint.powi(4)
        };
        Ok(JumpSample {
            point: Point {
                x: self.start.x + (self.end.x - self.start.x) * progress,
                y: self.start.y + (self.end.y - self.start.y) * progress,
            },
            elevation: self.start_elevation
                + (self.end_elevation - self.start_elevation) * progress
                + (1.0 - curve) * f64::from(self.apex()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One heat per hex jumped with a minimum of three; improved jets halve the count, not
    /// the minimum (Total Warfare p. 53). An XXL engine doubles the count with a minimum of
    /// six, and improved jets cancel the doubling (Tactical Operations: AUE p. 121).
    #[test]
    fn jump_heat_charges_hexes_with_a_minimum_of_three() {
        for (hexes, standard, improved, xxl, xxl_improved) in [
            (0, 3, 3, 6, 6),
            (1, 3, 3, 6, 6),
            (3, 3, 3, 6, 6),
            (4, 4, 3, 8, 6),
            (5, 5, 3, 10, 6),
            (6, 6, 3, 12, 6),
            (7, 7, 4, 14, 7),
            (8, 8, 4, 16, 8),
            (9, 9, 5, 18, 9),
        ] {
            assert_eq!(jump_heat(hexes, false, false), standard, "{hexes}");
            assert_eq!(jump_heat(hexes, true, false), improved, "{hexes}");
            assert_eq!(jump_heat(hexes, false, true), xxl, "{hexes}");
            assert_eq!(jump_heat(hexes, true, true), xxl_improved, "{hexes}");
        }
    }

    /// One per full half G off standard in either direction (Tactical Operations p. 53).
    #[test]
    fn jump_strain_modifier_counts_full_half_gravities() {
        for (gravity, modifier) in [
            (100, 0),
            (60, 0),
            (50, 1),
            (20, 1),
            (0, 2),
            (150, 1),
            (260, 3),
        ] {
            assert_eq!(jump_strain_modifier(gravity), modifier, "{gravity}");
        }
    }

    /// Jumping MP divided by the G-rating rounds to the nearest whole number, halves down,
    /// with no lower bound on the gravity (Tactical Operations p. 53).
    #[test]
    fn gravity_rounds_jump_points_to_nearest_with_halves_down() {
        for (points, gravity, expected) in [
            (4, 75, 5),
            (4, 125, 3),
            (5, 75, 7),
            (5, 200, 2),
            (3, 200, 1),
            (1, 200, 0),
            (5, 20, 25),
            (5, 0, 500),
            (5, 100, 5),
        ] {
            let capacity = JumpCapacity::at_gravity(f64::from(points) * 10.75, gravity).unwrap();
            assert_eq!(
                capacity.movement_points, expected,
                "{points} MP at {gravity}"
            );
            assert_eq!(
                capacity.speed,
                f64::from(points) * 10.75 * 100.0 / gravity.max(1) as f64
            );
        }
        assert!(JumpCapacity::at_gravity(10.75, 256).is_err());
        assert!(JumpCapacity::at_gravity(MAXIMUM_JUMP_SPEED + 1.0, 100).is_err());
        let saturated = JumpCapacity::at_gravity(MAXIMUM_JUMP_SPEED, 1).unwrap();
        assert_eq!(saturated.movement_points, u16::MAX);
        assert_eq!(saturated.speed, MAXIMUM_JUMP_SPEED);
    }

    /// Redirection geometry must preserve the committed point and fractional altitude exactly.
    #[test]
    fn airborne_continuation_preserves_samples_through_restart() {
        let start = JumpSample {
            point: Point { x: 1.25, y: 2.5 },
            elevation: 7.125,
        };
        let end = start.point.project(90.0, 2.0).unwrap();
        let path = JumpPath::continuation(start, end, 0, 3).unwrap();
        assert_eq!(path.sample(0.0, 3).unwrap(), start);
        assert_eq!(
            path.sample(1.0, 3).unwrap(),
            JumpSample {
                point: end,
                elevation: 0.0
            }
        );
        let saved = serde_json::to_value(path).unwrap();
        assert_eq!(saved["start_elevation"], 7.125);
        let restored: JumpPath = serde_json::from_value(saved).unwrap();
        assert_eq!(restored, path);
        for thrust in [0, 1, 3] {
            for progress in [0.0, 0.125, 0.5, 0.875, 1.0] {
                assert_eq!(
                    restored.sample(progress, thrust).unwrap(),
                    path.sample(progress, thrust).unwrap()
                );
            }
        }
        let mut flight = super::super::JumpFlight::new(path);
        assert_eq!(flight.sample(), start);
        let step = flight
            .advance(JumpCapacity::from_speed(32.25).unwrap())
            .unwrap();
        assert_eq!(step.from, start);
        assert!(flight.travelled() > 0.0);
        let replay: super::super::JumpFlight =
            serde_json::from_value(serde_json::to_value(flight).unwrap()).unwrap();
        assert_eq!(replay, flight);
    }

    /// Neither takeoff nor airborne descent is bounded, but range, climb and coordinate
    /// representability limits still apply.
    #[test]
    fn airborne_continuation_retains_admission_bounds() {
        let start = JumpSample {
            point: Point { x: 0.0, y: 0.0 },
            elevation: 8.25,
        };
        let end = start.point.project(90.0, 2.0).unwrap();
        assert!(JumpPath::continuation(start, end, 0, 3).is_ok());
        assert!(JumpPath::new(start.point, end, 8, 0, 3).is_ok());
        assert!(JumpPath::continuation(start, end, 12, 3).is_err());
        assert!(JumpPath::continuation(start, end, 0, 1).is_err());
        assert!(JumpPath::continuation(start, start.point, 0, 3).is_err());
        for elevation in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            32768.0,
            -32769.0,
        ] {
            assert!(JumpPath::continuation(JumpSample { elevation, ..start }, end, 0, 3).is_err());
        }
    }

    /// Saved normal launches cannot opt into fractional takeoff or conflicting admission modes.
    #[test]
    fn saved_jump_origin_modes_are_validated() {
        let start = Point { x: 1.0, y: 2.0 };
        let path = JumpPath::projected(start, 90, 2.0, 0, 0, 3).unwrap();
        let original = serde_json::to_value(path).unwrap();
        for elevation in [0.25, 32768.0, -32769.0] {
            let mut saved = original.clone();
            saved["start_elevation"] = elevation.into();
            assert!(serde_json::from_value::<JumpPath>(saved).is_err());
        }
        let mut saved = original;
        saved["continuation"] = true.into();
        assert!(serde_json::from_value::<JumpPath>(saved).is_err());
    }
}
