//! Bounded motion estimates from visible hex-center samples only; no world or dice access.
use super::navigation::Hex;
use crate::{BattleHexCoordinate, BattlePoint, BattlePosition, ObjectId};

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
    #[default]
    G,
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
            Self::G => (64, 65, 120, 8),
        }
    }
}

/// Observation-derived evidence, emitted only by the committed diagnostic harness.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct PursuitEvidence {
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
    /// An observed pause makes constant-velocity interception unreliable for this contact.
    motion_uncertain: bool,
    /// Pause detection needs a previously established cadence, not startup quantization.
    cadence_gap: Option<i64>,
    /// Saturating count allows confidence to mature without increasing retained samples.
    motion_transitions: u8,
    last_motion_at: Option<i64>,
}

fn center(p: BattlePosition) -> BattlePoint {
    BattleHexCoordinate {
        x: i32::from(p.x),
        y: i32::from(p.y),
    }
    .center()
}

impl Pursuit {
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
                    self.cadence_gap = None;
                    self.motion_transitions = 0;
                    self.aim = None;
                    self.reconsider_at = now;
                    self.evidence.reason = "reversal";
                }
                self.vector = Some(next);
                self.last_motion_at = Some(now);
                if self.policy == PursuitPolicy::G && self.motion_transitions == 4 {
                    // New confidence must not reuse an earlier pending decision.
                    self.reconsider_at = now;
                }
                self.motion_transitions = self.motion_transitions.saturating_add(1).min(5);
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
        self.rejected_at = Some(observed);
        self.aim = None;
        self.evidence.reason = "unreachable";
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
        if self.policy == PursuitPolicy::G
            && let (Some(period), Some(last_motion)) = (self.cadence_gap, self.last_motion_at)
            && now - last_motion > period + 1
        {
            self.motion_uncertain = true;
        }
        if self.engaged {
            self.aim = None;
            self.evidence.reason = "engaged";
            return None;
        }
        if self.motion_uncertain {
            self.aim = None;
            self.evidence.reason = "motion_paused";
            return None;
        }
        let legal = |p: BattlePosition| {
            p.map == own.map
                && i64::from(p.x) < width
                && i64::from(p.y) < height
                && leash.is_none_or(|o| {
                    o.map == p.map && Hex::new(o.x, o.y).distance(Hex::new(p.x, p.y)) <= 6
                })
        };
        if !legal(last) {
            self.aim = None;
            return None;
        }
        if now < self.reconsider_at {
            self.aim = self.aim.filter(|p| {
                legal(*p) && Hex::new(last.x, last.y).distance(Hex::new(p.x, p.y)) <= lead_limit
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
        let (first_transition, last_transition, transition_count, longest_gap) = self
            .samples
            .windows(2)
            .filter(|pair| pair[0].1 != pair[1].1)
            .fold(
                (None, None, 0usize, 0i64),
                |(first, last, count, gap), pair| {
                    (
                        first.or(Some(pair[1])),
                        Some(pair[1]),
                        count + 1,
                        gap.max(last.map_or(0, |last: (i64, BattlePosition)| pair[1].0 - last.0)),
                    )
                },
            );
        if transition_count >= 3 {
            self.cadence_gap = Some(self.cadence_gap.unwrap_or(0).max(longest_gap));
        }
        // One offset-hex transition cannot distinguish steady travel from a diagonal
        // quantization step. Require five transitions overall and three in the current window.
        if self.policy == PursuitPolicy::G && (self.motion_transitions < 5 || transition_count < 3)
        {
            self.evidence.reason = "insufficient_motion";
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
        // Slow contact motion offers little interception benefit, while quantized
        // goal changes can cost more turning time than the small lead saves.
        if self.policy == PursuitPolicy::G {
            if let (Some(first), Some(last)) = (first_transition, last_transition) {
                let span = (last.0 - first.0) as f64;
                if span > 0.0 {
                    let pace = center(first.1).range(center(last.1)).ok()? / span;
                    if pace > speed * 0.7 {
                        self.evidence.reason = "low_closing_margin";
                        return None;
                    }
                    if pace.min(v.0.hypot(v.1)) < speed * 0.3 {
                        self.evidence.reason = "direct_closure";
                        return None;
                    }
                }
            }
        }
        let own = center(own);
        let mut bounded = None;
        for horizon in 1..=horizon_limit {
            let scale = (horizon as f64).min(f64::from(lead_limit) / v.0.hypot(v.1));
            let mut point = BattlePoint {
                x: b.x + v.0 * scale,
                y: b.y + v.1 * scale,
            };
            let hex = point.containing_hex().ok()?;
            let (Ok(x), Ok(y)) = (u16::try_from(hex.x), u16::try_from(hex.y)) else {
                return None;
            };
            let mut aim = BattlePosition {
                map: last.map,
                x,
                y,
            };
            if !legal(aim) {
                self.evidence.reason = "boundary";
                return None;
            }
            if Hex::new(last.x, last.y).distance(Hex::new(x, y)) > lead_limit {
                if self.policy == PursuitPolicy::G {
                    let Some((previous_point, previous_aim)) = bounded else {
                        return None;
                    };
                    point = previous_point;
                    aim = previous_aim;
                } else {
                    self.evidence.reason = "boundary";
                    return None;
                }
            } else {
                bounded = Some((point, aim));
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
        self.engaged = true;
        self.aim = None;
        self.reconsider_at = 0;
        self.evidence.reason = "settled";
    }

    pub fn evidence(&self) -> PursuitEvidence {
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
            PursuitPolicy::G,
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
                        Hex::new(observed.x, observed.y).distance(Hex::new(aim.x, aim.y))
                            <= policy.limits().3
                    );
                }
            }
            a.sample_policy(1, ObjectId(2), pos(10, 15), 100, policy);
            assert_eq!(prediction(&mut a, 100), None);
        }
    }

    #[test]
    fn capped_lead_keeps_a_legal_intercept_instead_of_oscillating_to_direct() {
        let mut pursuit = Pursuit::default();
        for now in 0..=64 {
            pursuit.sample_policy(
                1,
                ObjectId(2),
                pos(22 + (now / 10) as u16, 10),
                now,
                PursuitPolicy::G,
            );
        }
        let aim = pursuit
            .predict(64, pos(12, 20), 0.183, 3, 48, 48, None)
            .expect("bounded intercept");
        assert!(Hex::new(28, 10).distance(Hex::new(aim.x, aim.y)) <= 8);
    }

    #[test]
    fn missed_observed_transition_suppresses_stale_motion_without_rejecting_normal_gaps() {
        let mut pursuit = Pursuit::default();
        for now in 0..=69 {
            pursuit.sample_policy(
                1,
                ObjectId(2),
                pos(10 + (now.min(60) / 10) as u16, 10),
                now,
                PursuitPolicy::G,
            );
        }
        assert!(
            pursuit
                .predict(69, pos(2, 20), 0.2, 3, 48, 48, None)
                .is_some()
        );
        for now in 70..=79 {
            pursuit.sample_policy(1, ObjectId(2), pos(16, 10), now, PursuitPolicy::G);
        }
        assert!(
            pursuit
                .predict(79, pos(2, 20), 0.2, 3, 48, 48, None)
                .is_none()
        );
        assert_eq!(pursuit.evidence.reason, "motion_paused");
    }

    #[test]
    fn pause_is_detected_when_rolling_window_loses_motion_confidence() {
        let mut pursuit = Pursuit::default();
        for now in 0..=111 {
            pursuit.sample_policy(
                1,
                ObjectId(2),
                pos(10 + (now.min(80) / 20) as u16, 10),
                now,
                PursuitPolicy::G,
            );
            if now == 80 {
                pursuit.predict(now, pos(2, 20), 0.1, 3, 48, 48, None);
                assert!(pursuit.cadence_gap.is_some());
            }
        }
        assert!(
            pursuit
                .predict(111, pos(2, 20), 0.1, 3, 48, 48, None)
                .is_none()
        );
        assert_eq!(pursuit.evidence.reason, "motion_paused");
        for now in 112..=180 {
            pursuit.sample_policy(
                1,
                ObjectId(2),
                pos(14 + ((now - 112) / 20) as u16, 10),
                now,
                PursuitPolicy::G,
            );
        }
        assert!(
            pursuit
                .predict(180, pos(2, 20), 0.1, 3, 48, 48, None)
                .is_none()
        );
        assert!(pursuit.motion_uncertain);
    }

    #[test]
    fn uneven_hex_intervals_do_not_look_like_a_pause() {
        let mut pursuit = Pursuit::default();
        for now in 0..=45 {
            let steps = [1, 2, 20, 21, 40].into_iter().filter(|t| *t <= now).count();
            pursuit.sample_policy(
                1,
                ObjectId(2),
                pos(10 + steps as u16, 10),
                now,
                PursuitPolicy::G,
            );
            pursuit.predict(now, pos(2, 20), 0.2, 3, 48, 48, None);
            assert!(!pursuit.motion_uncertain);
        }
        assert_eq!(pursuit.motion_transitions, 5);
        assert!(pursuit.cadence_gap.unwrap() >= 18);
        let checkpoint = pursuit.clone();
        pursuit.sample_policy(2, ObjectId(2), pos(15, 10), 46, PursuitPolicy::G);
        assert_eq!(pursuit.cadence_gap, None);
        assert_eq!(pursuit.motion_transitions, 0);
        assert_ne!(pursuit, checkpoint);
    }

    #[test]
    fn engagement_latch_is_transient_and_scoped_to_contact_identity() {
        let mut pursuit = Pursuit::default();
        for now in 0..=64 {
            pursuit.sample_policy(
                1,
                ObjectId(2),
                pos(10 + (now / 10) as u16, 10),
                now,
                PursuitPolicy::G,
            );
        }
        let checkpoint = pursuit.clone();
        pursuit.settle();
        pursuit.sample_policy(1, ObjectId(2), pos(17, 10), 65, PursuitPolicy::G);
        assert!(
            pursuit
                .predict(65, pos(2, 20), 0.2, 3, 48, 48, None)
                .is_none()
        );
        assert!(pursuit.engaged);
        assert!(!checkpoint.engaged);
        pursuit.sample_policy(1, ObjectId(3), pos(17, 10), 66, PursuitPolicy::G);
        assert!(!pursuit.engaged);
        pursuit = checkpoint;
        assert!(
            pursuit
                .predict(64, pos(2, 20), 0.2, 3, 48, 48, None)
                .is_some()
        );
    }

    #[test]
    fn slow_relative_motion_uses_direct_closure_and_rolls_back() {
        let mut pursuit = Pursuit::default();
        for now in 0..=115 {
            pursuit.sample_policy(
                1,
                ObjectId(2),
                pos(10 + (now / 20) as u16, 10),
                now,
                PursuitPolicy::G,
            );
        }
        let checkpoint = pursuit.clone();
        assert!(
            pursuit
                .predict(115, pos(2, 20), 0.2, 3, 48, 48, None)
                .is_none()
        );
        assert_eq!(pursuit.evidence.reason, "direct_closure");
        pursuit = checkpoint.clone();
        assert!(
            pursuit
                .predict(115, pos(2, 20), 0.1, 3, 48, 48, None)
                .is_some()
        );
        pursuit.settle();
        assert!(pursuit.aim.is_none());
        assert_eq!(pursuit.samples, checkpoint.samples);
    }

    #[test]
    fn lead_requires_additional_observed_transitions() {
        let mut pursuit = Pursuit::default();
        for now in 0..=64 {
            pursuit.sample_policy(
                1,
                ObjectId(2),
                pos(10 + (now / 20) as u16, 10),
                now,
                PursuitPolicy::G,
            );
        }
        assert!(
            pursuit
                .predict(64, pos(2, 20), 0.1, 3, 48, 48, None)
                .is_none()
        );
        for now in 65..=104 {
            pursuit.sample_policy(
                1,
                ObjectId(2),
                pos(10 + (now / 20) as u16, 10),
                now,
                PursuitPolicy::G,
            );
        }
        let aim = pursuit
            .predict(104, pos(2, 20), 0.1, 3, 48, 48, None)
            .unwrap();
        assert!(Hex::new(15, 10).distance(Hex::new(aim.x, aim.y)) > 3);
        assert!(Hex::new(15, 10).distance(Hex::new(aim.x, aim.y)) <= 8);
    }

    #[test]
    fn observed_pause_invalidates_a_cached_lead_immediately() {
        let mut pursuit = Pursuit::default();
        for now in 0..=75 {
            pursuit.sample_policy(
                1,
                ObjectId(2),
                pos(10 + (now.min(60) / 10) as u16, 10),
                now,
                PursuitPolicy::G,
            );
        }
        assert!(
            pursuit
                .predict(75, pos(2, 20), 0.2, 3, 48, 48, None)
                .is_some()
        );
        pursuit.sample_policy(1, ObjectId(2), pos(16, 10), 76, PursuitPolicy::G);
        assert!(
            pursuit
                .predict(76, pos(2, 20), 0.2, 3, 48, 48, None)
                .is_none()
        );
        assert_eq!(pursuit.evidence.reason, "motion_paused");
    }

    #[test]
    fn additional_motion_evidence_invalidates_pending_confidence() {
        let mut pursuit = Pursuit::default();
        for now in 0..=84 {
            pursuit.sample_policy(
                1,
                ObjectId(2),
                pos(10 + (now / 20) as u16, 10),
                now,
                PursuitPolicy::G,
            );
        }
        assert!(
            pursuit
                .predict(84, pos(2, 20), 0.1, 3, 48, 48, None)
                .is_none()
        );
        pursuit.sample_policy(1, ObjectId(2), pos(15, 10), 85, PursuitPolicy::G);
        let long = pursuit
            .predict(85, pos(2, 20), 0.1, 3, 48, 48, None)
            .unwrap();
        assert!(Hex::new(15, 10).distance(Hex::new(long.x, long.y)) > 2);
    }

    #[test]
    fn one_quantized_transition_does_not_establish_a_long_lead() {
        let mut pursuit = Pursuit::default();
        for now in 0..=40 {
            pursuit.sample_policy(
                1,
                ObjectId(2),
                pos(if now < 25 { 22 } else { 23 }, 10),
                now,
                PursuitPolicy::G,
            );
        }
        assert!(prediction(&mut pursuit, 40).is_none());
    }

    #[test]
    fn slow_hex_center_motion_keeps_confidence_between_crossings() {
        let mut pursuit = Pursuit::default();
        for now in 0..=115 {
            pursuit.sample_policy(
                1,
                ObjectId(2),
                pos(10 + (now / 20) as u16, 10),
                now,
                PursuitPolicy::G,
            );
        }
        assert!(
            pursuit
                .predict(115, pos(2, 20), 0.1, 3, 48, 48, None)
                .is_some()
        );
        assert_eq!(pursuit.samples.len(), 65);
        assert!(pursuit.evidence.velocity.0 > 0.03);
        assert!(pursuit.evidence.velocity.1.abs() < 0.01);
        // A genuine observed reversal discards the long history immediately.
        pursuit.sample_policy(1, ObjectId(2), pos(12, 10), 116, PursuitPolicy::G);
        assert!(prediction(&mut pursuit, 116).is_none());
        assert_eq!(pursuit.samples.len(), 1);
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
        assert!(Hex::new(10, 12).distance(Hex::new(lead.x, lead.y)) <= 3);
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
    use std::sync::Arc;

    /// Transient evidence participates in checkpoint equality and is cleared at lifecycle boundaries.
    #[tokio::test]
    async fn settling_retains_consecutive_visible_samples() {
        let root = autopilot::benchmark::copy_game_root().unwrap();
        let config = crate::Config::load(&root).unwrap();
        let base = crate::persistence::load(&config.database()).await.unwrap();
        let (mut world, id, _, _) = autopilot::encounters::fixture(
            &config,
            base,
            include_str!("../../../game/mechs/JR7-D"),
            "behind",
            1,
        )
        .unwrap();
        for now in 1..=3 {
            runtime::advance(&mut world, &config, now).unwrap();
        }
        assert_eq!(world.btech.autopilot_plans[&id].pursuit.samples.len(), 3);
        assert!(world.btech.autopilot_plans[&id].pursuit.aim.is_none());
    }

    #[tokio::test]
    async fn tracking_checkpoint_takeover_replacement_and_restart() {
        let root = autopilot::benchmark::copy_game_root().unwrap();
        let config = crate::Config::load(&root).unwrap();
        let base = crate::persistence::load(&config.database()).await.unwrap();
        let (mut world, id, target, map) = autopilot::encounters::fixture(
            &config,
            base,
            include_str!("../../../game/mechs/JR7-D"),
            "long_approach",
            1,
        )
        .unwrap();
        runtime::advance(&mut world, &config, 1).unwrap();
        let p = Arc::make_mut(&mut world.btech.autopilot_plans)
            .get_mut(&id)
            .unwrap();
        let before = p.clone();
        p.pursuit
            .sample(1, target, BattlePosition { map, x: 6, y: 1 }, 2);
        assert_ne!(*p, before);
        let tracked = world.clone();
        assert_eq!(world.btech.autopilot_plans, tracked.btech.autopilot_plans);
        autopilot::manual_takeover(&mut world, id).unwrap();
        assert!(!world.btech.autopilot_plans.contains_key(&id));
        world = tracked.clone();
        Arc::make_mut(&mut world.btech.controllers)
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
