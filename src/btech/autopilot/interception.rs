//! Bounded motion estimates from visible hex-center samples only; no world or dice access.
use super::navigation::GridHex;
use crate::{BattlePosition, HexCoordinate, ObjectId, Point};

/// Fixed, bounded policies for isolated pursuit comparisons; not a game setting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub enum PursuitPolicy {
    Control,
    A,
    B,
    C,
    D,
    E,
    F,
    /// Production pursuit policy with recoverable engagement and adaptive interception.
    #[default]
    Adaptive,
}

impl PursuitPolicy {
    fn limits(self) -> (i64, usize, i64, u32) {
        match self {
            Self::Control => (4, 6, 5, 3),
            Self::A => (16, 17, 30, 3),
            Self::B => (32, 33, 60, 6),
            Self::C => (32, 33, 120, 8),
            Self::D => (16, 17, 120, 3),
            Self::E => (16, 17, 120, 6),
            Self::F => (64, 65, 120, 6),
            Self::Adaptive => (64, 65, 120, 8),
        }
    }
}

/// Observation-derived evidence, emitted only by the committed diagnostic harness.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct PursuitEvidence {
    /// Confidence reset observed on this committed simulation second, if any.
    pub confidence_reset: Option<&'static str>,
    pub model: &'static str,
    pub model_error: f64,
    pub candidates: usize,
    pub estimated_seconds: f64,
    pub scores: Vec<(BattlePosition, f64)>,
    /// Moving-target and stopped-target estimates for each candidate, in seconds.
    pub scenario_scores: Vec<(BattlePosition, f64, f64)>,
    pub samples: usize,
    pub span: i64,
    pub velocity: (f64, f64),
    pub horizon: i64,
    pub reason: &'static str,
    pub observed: Option<BattlePosition>,
    pub aim: Option<BattlePosition>,
}

/// One synchronous controller's observation history, excluded from persistence.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Pursuit {
    adaptive: super::adaptive_pursuit::AdaptivePursuit,
    policy: PursuitPolicy,
    evidence: PursuitEvidence,
    key: Option<(u64, ObjectId, ObjectId)>,
    samples: Vec<(i64, BattlePosition)>,
    vector: Option<(f64, f64)>,
    aim: Option<BattlePosition>,
    reconsider_at: i64,
    /// A failed predicted search falls back until the next observed target hex.
    rejected_at: Option<BattlePosition>,
    /// Once in firing range, direct pursuit owns this uninterrupted contact episode.
    engaged: bool,
}

fn center(p: BattlePosition) -> Point {
    HexCoordinate {
        x: i32::from(p.x),
        y: i32::from(p.y),
    }
    .center()
}

impl Pursuit {
    /// Latest reliable observed velocity, available only to experimental steering.
    pub fn velocity(&self) -> Option<(f64, f64)> {
        (self.policy == PursuitPolicy::Adaptive
            && matches!(self.adaptive.evidence.model, "velocity_16" | "velocity_64"))
        .then_some(self.adaptive.evidence.velocity)
    }
    /// Update adaptive suspension and ineffective lead eligibility from permitted geometry.
    pub fn geometry(
        &mut self,
        now: i64,
        own: BattlePosition,
        band: super::AutopilotRangeBand,
        positional: bool,
        actual: bool,
    ) {
        if self.policy == PursuitPolicy::Adaptive {
            self.adaptive
                .geometry(now, own, band.minimum, band.maximum, positional, actual);
        }
    }

    /// Add at most one sample per simulation second. Reversals immediately drop confidence.
    #[cfg(test)]
    pub fn sample(&mut self, order: u64, target: ObjectId, position: BattlePosition, now: i64) {
        self.sample_policy(order, target, position, now, PursuitPolicy::Control);
    }

    pub fn sample_policy(
        &mut self,
        order: u64,
        target: ObjectId,
        position: BattlePosition,
        now: i64,
        policy: PursuitPolicy,
    ) {
        let key = (order, target, position.map);
        if self.key != Some(key)
            || self.policy != policy
            || self.samples.last().is_some_and(|s| s.0 > now)
        {
            *self = Self {
                key: Some(key),
                policy,
                ..Self::default()
            };
        }
        if self.samples.last().is_some_and(|s| s.0 == now) {
            return;
        }
        if policy == PursuitPolicy::Adaptive {
            self.adaptive.sample(now, position);
            return;
        }
        if let Some((_, previous)) = self.samples.last() {
            let a = center(*previous);
            let b = center(position);
            let next = (b.x - a.x, b.y - a.y);
            if next.0 != 0.0 || next.1 != 0.0 {
                if self.vector.is_some_and(|v| {
                    v.0 * next.0 + v.1 * next.1
                        < 0.5 * v.0.hypot(v.1) * next.0.hypot(next.1) - 1e-12
                }) {
                    self.samples.clear();
                    self.aim = None;
                    self.reconsider_at = now;
                    self.evidence.reason = "reversal";
                }
                self.vector = Some(next);
            }
        }
        let (window, limit, _, _) = self.policy.limits();
        self.samples.retain(|s| now - s.0 <= window);
        self.samples.push((now, position));
        if self.samples.len() > limit {
            self.samples.remove(0);
        }
        if self.rejected_at.is_some_and(|p| p != position) {
            self.rejected_at = None;
        }
    }

    /// Reject one predicted region, allowing direct pursuit through the normal scheduler.
    pub fn reject(&mut self, observed: BattlePosition) {
        self.adaptive.reject(observed);
        self.rejected_at = Some(observed);
        self.aim = None;
        self.evidence.reason = "unreachable";
    }

    /// Score the bounded adaptive candidates, retaining isolated comparison policies.
    pub fn choose(
        &mut self,
        now: i64,
        own: BattlePosition,
        speed: f64,
        radius: u16,
        width: i64,
        height: i64,
        leash: Option<BattlePosition>,
        score: impl FnMut(BattlePosition, (f64, f64), f64) -> Option<(f64, f64)>,
    ) -> Option<BattlePosition> {
        if self.policy == PursuitPolicy::Adaptive {
            return self
                .adaptive
                .choose(now, own, speed, radius, width, height, leash, score);
        }
        self.predict(now, own, speed, radius, width, height, leash)
    }

    /// Estimate reachable lead using allowed own speed and public map bounds.
    /// `speed` is in map units per simulation second, already adjusted for map rate.
    pub fn predict(
        &mut self,
        now: i64,
        own: BattlePosition,
        speed: f64,
        radius: u16,
        width: i64,
        height: i64,
        leash: Option<BattlePosition>,
    ) -> Option<BattlePosition> {
        let (_, _, horizon_limit, lead_limit) = self.policy.limits();
        let &(last_time, last) = self.samples.last()?;
        if last_time != now || self.rejected_at == Some(last) {
            self.aim = None;
            return None;
        }
        if self.engaged {
            self.aim = None;
            self.evidence.reason = "engaged";
            return None;
        }
        let legal = |p: BattlePosition| {
            p.map == own.map
                && i64::from(p.x) < width
                && i64::from(p.y) < height
                && leash.is_none_or(|o| {
                    o.map == p.map && GridHex::new(o.x, o.y).distance(GridHex::new(p.x, p.y)) <= 6
                })
        };
        if !legal(last) {
            self.aim = None;
            return None;
        }
        if now < self.reconsider_at {
            self.aim = self.aim.filter(|p| {
                legal(*p)
                    && GridHex::new(last.x, last.y).distance(GridHex::new(p.x, p.y)) <= lead_limit
            });
            return self.aim;
        }
        self.reconsider_at = now + 3;
        self.aim = None;
        let &(first_time, first) = self.samples.first()?;
        self.evidence.samples = self.samples.len();
        self.evidence.span = last_time - first_time;
        self.evidence.observed = Some(last);
        if self.evidence.reason != "reversal" {
            self.evidence.reason = "insufficient_samples";
        }
        self.evidence.horizon = 0;
        if self.samples.len() < 3 || last_time - first_time < 2 || speed <= 0.0 {
            return None;
        }
        let a = center(first);
        let b = center(last);
        let dt = (last_time - first_time) as f64;
        let v = if self.policy == PursuitPolicy::Control {
            ((b.x - a.x) / dt, (b.y - a.y) / dt)
        } else {
            // Center time before summation: simulation uptime must not affect conditioning.
            let n = self.samples.len() as f64;
            let mean = self
                .samples
                .iter()
                .map(|s| (s.0 - first_time) as f64)
                .sum::<f64>()
                / n;
            let mut xy = (0.0, 0.0);
            let mut variance = 0.0;
            for &(time, position) in &self.samples {
                let t = (time - first_time) as f64 - mean;
                let p = center(position);
                variance += t * t;
                xy.0 += t * (p.x - a.x);
                xy.1 += t * (p.y - a.y);
            }
            (xy.0 / variance, xy.1 / variance)
        };
        self.evidence.velocity = v;
        if v.0.hypot(v.1) < 1e-9 {
            self.evidence.reason = "stationary";
            return None;
        }
        let own = center(own);
        for horizon in 1..=horizon_limit {
            let scale = (horizon as f64).min(f64::from(lead_limit) / v.0.hypot(v.1));
            let point = Point {
                x: b.x + v.0 * scale,
                y: b.y + v.1 * scale,
            };
            let hex = point.containing_hex().ok()?;
            let (Ok(x), Ok(y)) = (u16::try_from(hex.x), u16::try_from(hex.y)) else {
                return None;
            };
            let aim = BattlePosition {
                map: last.map,
                x,
                y,
            };
            if !legal(aim) {
                self.evidence.reason = "boundary";
                return None;
            }
            if GridHex::new(last.x, last.y).distance(GridHex::new(x, y)) > lead_limit {
                self.evidence.reason = "boundary";
                return None;
            }
            let travel = (own.range(point).ok()? - f64::from(radius)).max(0.0) / speed;
            if travel <= horizon as f64 || horizon == horizon_limit {
                self.aim = (aim != last).then_some(aim);
                self.evidence.horizon = horizon;
                self.evidence.reason = if self.aim.is_some() {
                    "predicted"
                } else {
                    "same_hex"
                };
                return self.aim;
            }
        }
        None
    }

    /// Settling uses actual target geometry and cancels the current lead immediately.
    pub fn settle(&mut self) {
        if self.policy == PursuitPolicy::Adaptive {
            self.adaptive.suspend();
            return;
        }
        self.engaged = true;
        self.aim = None;
        self.reconsider_at = 0;
        self.evidence.reason = "settled";
    }

    pub fn evidence(&self) -> PursuitEvidence {
        if self.policy == PursuitPolicy::Adaptive {
            return self.adaptive.evidence.clone();
        }
        PursuitEvidence {
            aim: self.aim,
            ..self.evidence.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pos(x: u16, y: u16) -> BattlePosition {
        BattlePosition {
            map: ObjectId(1),
            x,
            y,
        }
    }
    fn prediction(p: &mut Pursuit, t: i64) -> Option<BattlePosition> {
        p.predict(t, pos(2, 20), 0.2, 3, 48, 48, None)
    }
    #[test]
    fn experimental_policies_bound_storage_lead_and_time_origin() {
        for policy in [
            PursuitPolicy::Control,
            PursuitPolicy::A,
            PursuitPolicy::B,
            PursuitPolicy::C,
            PursuitPolicy::D,
            PursuitPolicy::E,
            PursuitPolicy::F,
        ] {
            let mut a = Pursuit::default();
            let mut b = Pursuit::default();
            for t in 0..100 {
                let observed = pos(10, 10 + (t / 16) as u16);
                a.sample_policy(1, ObjectId(2), observed, t, policy);
                b.sample_policy(1, ObjectId(2), observed, t + 1_000_000, policy);
                let aim = prediction(&mut a, t);
                assert_eq!(aim, prediction(&mut b, t + 1_000_000));
                assert!(a.samples.len() <= policy.limits().1);
                if let Some(aim) = aim {
                    assert!(
                        GridHex::new(observed.x, observed.y).distance(GridHex::new(aim.x, aim.y))
                            <= policy.limits().3
                    );
                }
            }
            a.sample_policy(1, ObjectId(2), pos(10, 15), 100, policy);
            assert_eq!(prediction(&mut a, 100), None);
        }
    }

    #[test]
    fn stationary_and_insufficient_samples_do_not_predict() {
        let mut p = Pursuit::default();
        for t in 0..20 {
            p.sample(1, ObjectId(2), pos(10, 10), t);
            assert_eq!(prediction(&mut p, t), None);
            assert!(p.samples.len() <= 6);
        }
    }
    #[test]
    fn lead_is_bounded_and_reversal_discards_it() {
        let mut p = Pursuit::default();
        for t in 0..3 {
            p.sample(1, ObjectId(2), pos(10, 10 + t as u16), t);
        }
        let lead = prediction(&mut p, 2).unwrap();
        assert!(GridHex::new(10, 12).distance(GridHex::new(lead.x, lead.y)) <= 3);
        p.sample(1, ObjectId(2), pos(10, 11), 3);
        assert_eq!(prediction(&mut p, 3), None);
        assert_eq!(p.samples.len(), 1);
    }
    #[test]
    fn quantized_sixty_degree_boundary_keeps_confidence() {
        let mut p = Pursuit::default();
        for t in 0..3 {
            p.sample(1, ObjectId(2), pos(10 + t as u16, 10), t);
        }
        assert_eq!(p.samples.len(), 3);
    }

    #[test]
    fn boundaries_leash_and_stale_time_reject_predictions() {
        for (width, leash) in [(11, None), (48, Some(pos(10, 5)))] {
            let mut p = Pursuit::default();
            for t in 0..3 {
                p.sample(1, ObjectId(2), pos(10 + t as u16, 10), t);
            }
            assert_eq!(p.predict(2, pos(2, 20), 0.2, 3, width, 48, leash), None);
        }
        let mut p = Pursuit::default();
        for t in 0..3 {
            p.sample(1, ObjectId(2), pos(10, 10 + t as u16), t);
        }
        assert_eq!(prediction(&mut p, 3), None);
    }
    #[test]
    fn rejection_and_identity_reset_are_scoped() {
        let mut p = Pursuit::default();
        for t in 0..3 {
            p.sample(1, ObjectId(2), pos(10, 10 + t as u16), t);
        }
        p.reject(pos(10, 12));
        assert_eq!(prediction(&mut p, 2), None);
        p.sample(2, ObjectId(2), pos(10, 13), 3);
        assert_eq!(p.samples.len(), 1);
        p.sample(2, ObjectId(3), pos(10, 14), 4);
        assert_eq!(p.samples.len(), 1);
        let clone = p.clone();
        p.sample(2, ObjectId(3), pos(10, 15), 5);
        assert_ne!(p, clone);
    }
    #[test]
    fn duplicate_seconds_and_unobservable_world_changes_have_no_input() {
        let mut a = Pursuit::default();
        let mut b = Pursuit::default();
        for t in 0..8 {
            a.sample(1, ObjectId(2), pos(10, 10 + t as u16 / 2), t);
            b.sample(1, ObjectId(2), pos(10, 10 + t as u16 / 2), t);
            a.sample(1, ObjectId(2), pos(40, 40), t);
            assert_eq!(prediction(&mut a, t), prediction(&mut b, t));
            assert_eq!(a, b);
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::btech::autopilot::{self, AutopilotOrder, AutopilotSubmissionMode, runtime};

    #[tokio::test]
    async fn tracking_checkpoint_takeover_replacement_and_restart() {
        let root = autopilot::benchmark::copy_game_root().unwrap();
        let config = crate::Config::load(&root).unwrap();
        let base = crate::persistence::load(&config.database()).await.unwrap();
        let (mut world, id, target, map) = autopilot::encounters::fixture(
            &config,
            base,
            include_str!("../../../game/mechs/JR7-D.toml"),
            "long_approach",
            1,
        )
        .unwrap();
        runtime::advance(&mut world, &config, 1).unwrap();
        let p = world.btech.autopilot_plans.get_mut(&id).unwrap();
        let before = p.clone();
        p.pursuit
            .sample(1, target, BattlePosition { map, x: 6, y: 1 }, 2);
        assert_ne!(*p, before);
        let tracked = world.clone();
        assert_eq!(world.btech.autopilot_plans, tracked.btech.autopilot_plans);
        autopilot::manual_takeover(&mut world, id).unwrap();
        assert!(!world.btech.autopilot_plans.contains_key(&id));
        world = tracked.clone();
        world
            .btech
            .controllers
            .get_mut(&id)
            .unwrap()
            .submit(
                vec![AutopilotOrder::Hold],
                AutopilotSubmissionMode::Replace,
                None,
            )
            .unwrap();
        runtime::advance(&mut world, &config, 3).unwrap();
        assert!(
            world
                .btech
                .autopilot_plans
                .get(&id)
                .is_none_or(|p| p.pursuit == Pursuit::default())
        );
        let encoded = serde_json::to_value(&tracked.btech).unwrap();
        assert!(!encoded.to_string().contains("reconsider_at"));
        crate::persistence::save(&config.database(), &tracked)
            .await
            .unwrap();
        let loaded = crate::persistence::load(&config.database()).await.unwrap();
        assert!(loaded.btech.autopilot_plans.is_empty());
    }
}
