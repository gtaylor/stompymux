//! Bounded, observation-only motion assessment and interception candidate selection.
use super::interception::PursuitEvidence;
use crate::{BattlePosition, Point};

const WINDOWS: [i64; 2] = [16, 64];
const OFFSETS: [i64; 3] = [6, 12, 24];
/// Models fitted using only the sighting history available at this timestamp.
type HistoricalFit = (i64, [Option<Fit>; 2]);

/// Regression position at its cutoff, together with the fitted velocity.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Fit {
    position: Point,
    velocity: (f64, f64),
}

fn center(p: BattlePosition) -> Point {
    crate::HexCoordinate {
        x: i32::from(p.x),
        y: i32::from(p.y),
    }
    .center()
}
fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

/// A retained lead becomes stale when the observed target has passed it.
fn ahead(aim: BattlePosition, observed: BattlePosition, velocity: (f64, f64)) -> bool {
    let a = center(aim);
    let b = center(observed);
    (a.x - b.x) * velocity.0 + (a.y - b.y) * velocity.1 >= -1e-12
}

/// A fitted hypothesis, including out-of-sample error in hex units.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Model {
    position: Point,
    velocity: (f64, f64),
    error: f64,
    window: i64,
}

/// Fit only observations at or before the requested cutoff.
fn fit(samples: &[(i64, BattlePosition)], cutoff: i64, window: i64) -> Option<Fit> {
    let _timing = super::diagnostics::pursuit("fit");
    let points: Vec<_> = samples
        .iter()
        .filter(|s| s.0 <= cutoff && s.0 >= cutoff - window)
        .collect();
    if points.windows(2).filter(|p| p[0].1 != p[1].1).count() < 3 {
        return None;
    }
    let origin = points.first()?.0;
    let mean = points.iter().map(|p| (p.0 - origin) as f64).sum::<f64>() / points.len() as f64;
    let mut sum = (0.0, 0.0, 0.0);
    let mut position = Point { x: 0.0, y: 0.0 };
    let n = points.len() as f64;
    for p in points {
        let t = (p.0 - origin) as f64 - mean;
        let xy = center(p.1);
        position.x += xy.x / n;
        position.y += xy.y / n;
        sum.0 += t * xy.x;
        sum.1 += t * xy.y;
        sum.2 += t * t;
    }
    if sum.2 <= 0.0 {
        return None;
    }
    let velocity = (sum.0 / sum.2, sum.1 / sum.2);
    let dt = (cutoff - origin) as f64 - mean;
    position.x += velocity.0 * dt;
    position.y += velocity.1 * dt;
    Some(Fit { position, velocity })
}

fn assess(
    samples: &[(i64, BattlePosition)],
    forecasts: &[HistoricalFit],
    now: i64,
) -> Option<Model> {
    let mut best: Option<Model> = None;
    for (index, window) in WINDOWS.into_iter().enumerate() {
        let Some(current) = forecasts
            .last()
            .filter(|s| s.0 == now)
            .and_then(|s| s.1[index])
        else {
            continue;
        };
        let mut errors = (0.0, 0.0, 0);
        let mut valid_offsets = [false; 3];
        // Average recent out-of-sample outcomes rather than the current hex phase.
        // Two models × sixteen outcomes × three offsets bounds this at 96 comparisons.
        for &(observed_at, observed) in samples.iter().filter(|s| now - s.0 < 16) {
            let actual = center(observed);
            for (offset_index, offset) in OFFSETS.into_iter().enumerate() {
                let cutoff = observed_at - offset;
                let Some(previous) = samples.iter().rev().find(|s| s.0 <= cutoff) else {
                    continue;
                };
                let Some(fitted) = forecasts
                    .iter()
                    .find(|s| s.0 == previous.0)
                    .and_then(|s| s.1[index])
                else {
                    continue;
                };
                let p = center(previous.1);
                let dt = (observed_at - previous.0) as f64;
                let predicted = Point {
                    x: fitted.position.x + fitted.velocity.0 * dt,
                    y: fitted.position.y + fitted.velocity.1 * dt,
                };
                let (Ok(predicted), Ok(actual), Ok(previous)) = (
                    predicted.containing_hex(),
                    actual.containing_hex(),
                    p.containing_hex(),
                ) else {
                    continue;
                };
                errors.0 += predicted.distance(actual) as f64;
                errors.1 += previous.distance(actual) as f64;
                errors.2 += 1;
                valid_offsets[offset_index] = true;
            }
        }
        if !valid_offsets[2]
            || valid_offsets.into_iter().filter(|v| *v).count() < 2
            || errors.1 <= 0.0
            || errors.0 > errors.1 * 0.8
        {
            continue;
        }
        let model = Model {
            position: current.position,
            velocity: current.velocity,
            error: errors.0 / f64::from(errors.2),
            window,
        };
        if best.is_none_or(|b| model.error < b.error) {
            best = Some(model);
        }
    }
    best
}

/// Transient confidence episode and selected goal, always rolled back with its controller.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct AdaptivePursuit {
    samples: Vec<(i64, BattlePosition)>,
    forecasts: Vec<HistoricalFit>,
    vector: Option<(f64, f64)>,
    last_motion: Option<i64>,
    cadence: Option<i64>,
    transitions: u8,
    reconsider_at: i64,
    aim: Option<BattlePosition>,
    rejected: Option<BattlePosition>,
    /// Useful actual geometry suspends scoring until the normal reconsideration boundary.
    engaged: bool,
    /// This tick's occupied region, only when observed positional geometry is ineffective.
    ineffective: Option<(BattlePosition, u16, u16)>,
    pub evidence: PursuitEvidence,
}
impl AdaptivePursuit {
    /// Retain one visible hex per second; a reversal begins a new confidence episode.
    pub fn sample(&mut self, now: i64, position: BattlePosition) {
        if self
            .samples
            .last()
            .is_some_and(|s| s.0 == now && s.1.map == position.map)
        {
            return;
        }
        self.evidence.confidence_reset = None;
        if self
            .samples
            .last()
            .is_some_and(|s| s.0 > now || s.1.map != position.map)
        {
            self.reset_episode("invalidated");
        }
        if let Some(previous) = self.samples.last().copied() {
            let a = center(previous.1);
            let b = center(position);
            let v = (b.x - a.x, b.y - a.y);
            if v.0 != 0.0 || v.1 != 0.0 {
                if self.vector.is_some_and(|p| {
                    p.0 * v.0 + p.1 * v.1 < 0.5 * p.0.hypot(p.1) * v.0.hypot(v.1) - 1e-12
                }) {
                    self.reset_episode("reversal");
                }
                if let Some(last) = self.last_motion {
                    self.cadence = Some(self.cadence.unwrap_or(0).max(now - last));
                }
                self.last_motion = Some(now);
                self.vector = Some(v);
                self.transitions = self.transitions.saturating_add(1).min(3);
            } else if let (Some(last), Some(cadence)) = (self.last_motion, self.cadence)
                && self.transitions >= 3
                && now - last > cadence + 1
            {
                self.reset_episode("motion_paused");
            }
        }
        self.samples.retain(|s| now - s.0 <= 64);
        self.samples.push((now, position));
        if self.samples.len() > 65 {
            self.samples.remove(0);
        }
        self.forecasts.retain(|s| now - s.0 <= 64);
        self.forecasts
            .push((now, WINDOWS.map(|window| fit(&self.samples, now, window))));
        if self.forecasts.len() > 65 {
            self.forecasts.remove(0);
        }
        if self.rejected.is_some_and(|p| p != position) {
            self.rejected = None;
        }
    }
    fn reset_episode(&mut self, reason: &'static str) {
        *self = Self::default();
        self.evidence.reason = reason;
        self.evidence.confidence_reset = Some(reason);
    }
    pub fn suspend(&mut self) {
        self.aim = None;
        if !self.engaged {
            self.reconsider_at = self.samples.last().map_or(0, |s| s.0 + 3);
        }
        self.engaged = true;
        self.evidence.reason = "settled";
        self.evidence.aim = None;
    }
    /// Supply only observed geometry; no readiness or hidden target state enters the policy.
    pub fn geometry(
        &mut self,
        now: i64,
        own: BattlePosition,
        minimum: u16,
        maximum: u16,
        positional: bool,
        actual: bool,
    ) {
        self.ineffective = (!positional).then_some((own, minimum, maximum));
        if actual {
            if !self.engaged {
                super::diagnostics::pursuit_count("engagement_enter");
            }
            self.suspend();
            if now >= self.reconsider_at {
                self.reconsider_at = now + 3;
            }
        } else if self.engaged && now >= self.reconsider_at {
            super::diagnostics::pursuit_count("engagement_exit");
            self.engaged = false;
            self.evidence.reason = "engagement_lost";
        }
    }
    pub fn reject(&mut self, target: BattlePosition) {
        self.rejected = Some(target);
        self.aim = None;
    }

    /// Assess retained fits, then score at most three navigation candidates.
    pub fn choose(
        &mut self,
        now: i64,
        own: BattlePosition,
        speed: f64,
        radius: u16,
        width: i64,
        height: i64,
        leash: Option<BattlePosition>,
        mut score: impl FnMut(BattlePosition, (f64, f64), f64) -> Option<(f64, f64)>,
    ) -> Option<BattlePosition> {
        let _timing = super::diagnostics::pursuit("choose");
        let &(time, last) = self.samples.last()?;
        let legal = |p: BattlePosition| {
            p.map == own.map
                && i64::from(p.x) < width
                && i64::from(p.y) < height
                && super::navigation::GridHex::new(last.x, last.y)
                    .distance(super::navigation::GridHex::new(p.x, p.y))
                    <= 8
                && leash.is_none_or(|o| {
                    o.map == p.map
                        && super::navigation::GridHex::new(o.x, o.y)
                            .distance(super::navigation::GridHex::new(p.x, p.y))
                            <= 6
                })
                && (p == last
                    || self.ineffective.is_none_or(|(own, minimum, maximum)| {
                        let distance = super::navigation::GridHex::new(own.x, own.y)
                            .distance(super::navigation::GridHex::new(p.x, p.y));
                        distance < u32::from(minimum) || distance > u32::from(maximum)
                    }))
        };
        if self.engaged {
            return None;
        }
        if time != now || !legal(last) || speed <= 0.0 || self.rejected == Some(last) {
            self.aim = None;
            self.evidence.aim = None;
            self.evidence.reason = "invalid_prediction";
            return None;
        }
        if now < self.reconsider_at {
            let previous = self.aim;
            self.aim = self
                .aim
                .filter(|p| legal(*p) && ahead(*p, last, self.evidence.velocity));
            if previous.is_some() && self.aim.is_none() {
                self.evidence.reason = "ineligible_lead";
                super::diagnostics::pursuit_count("lead_rejected");
            }
            self.evidence.aim = self.aim;
            return self.aim;
        }
        self.reconsider_at = now + 3;
        self.evidence.samples = self.samples.len();
        self.evidence.span = now - self.samples[0].0;
        self.evidence.observed = Some(last);
        self.evidence.horizon = 0;
        self.evidence.model = "stationary";
        self.evidence.model_error = 0.0;
        self.evidence.candidates = 0;
        self.evidence.scores.clear();
        self.evidence.scenario_scores.clear();
        let assessment = {
            let _timing = super::diagnostics::pursuit("assessment");
            assess(&self.samples, &self.forecasts, now)
        };
        let Some(model) = assessment else {
            self.aim = None;
            self.evidence.reason = "unreliable_model";
            self.evidence.aim = None;
            return None;
        };
        self.evidence.model = if model.window == 16 {
            "velocity_16"
        } else {
            "velocity_64"
        };
        self.evidence.model_error = model.error;
        self.evidence.velocity = model.velocity;
        let v = model.velocity;
        self.aim = self.aim.filter(|p| legal(*p) && ahead(*p, last, v));
        let origin = model.position;
        let here = center(own);
        let mut lead = last;
        for horizon in 1..=120 {
            let scale = (horizon as f64).min(8.0 / v.0.hypot(v.1).max(1e-9));
            let p = Point {
                x: origin.x + v.0 * scale,
                y: origin.y + v.1 * scale,
            };
            let Ok(hex) = p.containing_hex() else {
                break;
            };
            let (Ok(x), Ok(y)) = (u16::try_from(hex.x), u16::try_from(hex.y)) else {
                break;
            };
            let aim = BattlePosition {
                map: last.map,
                x,
                y,
            };
            if !legal(aim) {
                break;
            }
            lead = aim;
            self.evidence.horizon = horizon;
            if (distance(here, p) - f64::from(radius)).max(0.0) / speed <= horizon as f64 {
                break;
            }
        }
        let end = center(lead);
        let half = Point {
            x: (origin.x + end.x) * 0.5,
            y: (origin.y + end.y) * 0.5,
        }
        .containing_hex()
        .ok()
        .and_then(|h| {
            Some(BattlePosition {
                map: last.map,
                x: u16::try_from(h.x).ok()?,
                y: u16::try_from(h.y).ok()?,
            })
        });
        let mut candidates = vec![last];
        for p in half.into_iter().chain([lead]) {
            if legal(p) && ahead(p, last, v) && !candidates.contains(&p) {
                candidates.push(p);
            }
        }
        // Score the incumbent instead of the intermediate lead when hysteresis needs it.
        if let Some(aim) = self.aim.filter(|p| legal(*p))
            && !candidates.contains(&aim)
        {
            if candidates.len() == 3 {
                candidates[1] = aim;
            } else {
                candidates.push(aim);
            }
        }
        let mut scenarios = Vec::new();
        for p in candidates {
            if let Some((moving, stopped)) = score(p, v, if p == last { 0.0 } else { model.error })
                && moving.is_finite()
                && stopped.is_finite()
            {
                scenarios.push((p, moving, stopped));
            }
        }
        let best_moving = scenarios.iter().map(|s| s.1).fold(f64::INFINITY, f64::min);
        let best_stopped = scenarios.iter().map(|s| s.2).fold(f64::INFINITY, f64::min);
        // Minimize worst-case regret against both permitted motion hypotheses.
        let ranked: Vec<_> = scenarios
            .iter()
            .map(|s| (s.0, (s.1 - best_moving).max(s.2 - best_stopped)))
            .collect();
        self.evidence.scenario_scores = scenarios.clone();
        self.evidence.candidates = ranked.len();
        self.evidence.scores = ranked.clone();
        let Some(direct) = ranked.iter().find(|p| p.0 == last).copied() else {
            self.aim = None;
            self.evidence.aim = None;
            self.evidence.reason = "unscorable";
            return None;
        };
        let mut best = direct;
        for p in &ranked {
            if p.1 < best.1 {
                best = *p;
            }
        }
        let incumbent = self
            .aim
            .and_then(|aim| ranked.iter().find(|p| p.0 == aim).copied())
            .unwrap_or(direct);
        let selected = if best.0 != incumbent.0 && incumbent.1 - best.1 < 3.0 {
            incumbent
        } else {
            best
        };
        self.evidence.estimated_seconds = scenarios.iter().find(|s| s.0 == selected.0).unwrap().1;
        self.aim = (selected.0 != last).then_some(selected.0);
        self.evidence.aim = self.aim;
        self.evidence.reason = if self.aim.is_some() {
            "predicted"
        } else {
            "direct_score"
        };
        self.aim
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn engagement_recovers_on_cadence_and_rolls_back() {
        let mut p = AdaptivePursuit::default();
        p.sample(10, pos(20));
        p.geometry(10, pos(18), 1, 3, true, true);
        let checkpoint = p.clone();
        assert_eq!(p.reconsider_at, 13);
        p.geometry(11, pos(18), 1, 3, true, false);
        assert!(p.engaged);
        assert_eq!(
            p.choose(11, pos(18), 0.2, 3, 48, 48, None, |_, _, _| panic!(
                "Engaged policy must skip scoring"
            )),
            None
        );
        p.geometry(13, pos(18), 1, 3, true, false);
        assert!(!p.engaged);
        p = checkpoint.clone();
        assert_eq!(p, checkpoint);
        p.reset_episode("reversal");
        assert!(!p.engaged);
        assert!(p.samples.is_empty());
    }

    #[test]
    fn cached_satisfied_lead_falls_back_without_a_blacklist() {
        let mut p = AdaptivePursuit::default();
        p.sample(10, pos(20));
        p.aim = Some(pos(24));
        p.evidence.velocity = (1.0, 0.0);
        p.reconsider_at = 13;
        p.geometry(10, pos(23), 1, 3, false, false);
        assert_eq!(
            p.choose(10, pos(23), 0.2, 3, 48, 48, None, |_, _, _| panic!(
                "Cached decision must skip scoring"
            )),
            None
        );
        assert_eq!(p.rejected, None);
        assert_eq!(p.reconsider_at, 13);
    }
    fn pos(x: u16) -> BattlePosition {
        BattlePosition {
            map: crate::ObjectId(1),
            x,
            y: 10,
        }
    }
    #[test]
    fn fits_do_not_read_future_samples() {
        let mut s: Vec<_> = (0..=64).map(|t| (t, pos(10 + (t / 4) as u16))).collect();
        let before = fit(&s, 32, 64);
        for p in &mut s {
            if p.0 > 32 {
                p.1 = pos(100);
            }
        }
        assert_eq!(before, fit(&s, 32, 64));
    }
    #[test]
    fn regression_intercept_reduces_quantized_position_error() {
        let samples: Vec<_> = (0..=64).map(|t| (t, pos(10 + (t / 18) as u16))).collect();
        let fitted = fit(&samples, 64, 64).unwrap();
        let anchored = center(samples.last().unwrap().1);
        let error = |origin: Point| {
            samples
                .iter()
                .map(|&(t, p)| {
                    let predicted = Point {
                        x: origin.x - fitted.velocity.0 * (64 - t) as f64,
                        y: origin.y - fitted.velocity.1 * (64 - t) as f64,
                    };
                    distance(predicted, center(p)).powi(2)
                })
                .sum::<f64>()
        };
        assert!(error(fitted.position) + 1e-6 < error(anchored));
    }
    #[test]
    fn stationary_is_not_a_moving_model() {
        let s: Vec<_> = (0..=64).map(|t| (t, pos(10))).collect();
        assert!(assess(&s, &[], 64).is_none());
    }

    #[test]
    fn passed_interception_goal_is_discarded_before_reconsideration() {
        let mut pursuit = moving();
        let &(now, observed) = pursuit.samples.last().unwrap();
        pursuit.aim = Some(pos(observed.x - 2));
        pursuit.evidence.velocity = (1.0, 0.0);
        pursuit.reconsider_at = now + 3;
        let aim = pursuit.choose(now, pos(1), 1.0, 3, 100, 100, None, |_, _, _| {
            panic!("cached decision must not rescore")
        });
        assert_eq!(aim, None);
        assert_eq!(pursuit.evidence.aim, None);
    }
    #[test]
    fn startup_quantization_does_not_establish_a_pause_cadence() {
        let mut p = AdaptivePursuit::default();
        for t in 0..=45 {
            let steps = [1, 2, 20, 21, 40].into_iter().filter(|s| *s <= t).count();
            p.sample(t, pos(10 + steps as u16));
        }
        assert_eq!(p.samples.len(), 46);
        assert_eq!(p.transitions, 3);
    }
    #[test]
    fn consistent_motion_can_recover_after_pause() {
        let mut p = AdaptivePursuit::default();
        for t in 0..=40 {
            p.sample(t, pos(10 + (t / 4) as u16));
        }
        assert!(assess(&p.samples, &p.forecasts, 40).is_some());
        for t in 41..=55 {
            p.sample(t, pos(20));
        }
        assert!(assess(&p.samples, &p.forecasts, 55).is_none());
        for t in 56..=120 {
            p.sample(t, pos(20 + ((t - 56) / 4) as u16));
        }
        assert!(assess(&p.samples, &p.forecasts, 120).is_some());
        assert!(p.samples.len() <= 65);
    }

    fn moving() -> AdaptivePursuit {
        let mut p = AdaptivePursuit::default();
        for t in 0..=64 {
            p.sample(t, pos(10 + (t / 4) as u16));
        }
        p
    }
    #[test]
    fn candidates_and_lead_are_bounded_and_ties_choose_direct() {
        let mut p = moving();
        let mut calls = 0;
        let choice = p.choose(64, pos(2), 0.4, 3, 48, 48, None, |aim, _, _| {
            calls += 1;
            assert!(
                super::super::navigation::GridHex::new(26, 10)
                    .distance(super::super::navigation::GridHex::new(aim.x, aim.y))
                    <= 8
            );
            Some((10.0, 10.0))
        });
        assert!(choice.is_none());
        assert!((1..=3).contains(&calls));
    }
    #[test]
    fn three_second_benefit_and_reconsideration_are_required() {
        let mut p = moving();
        assert!(
            p.choose(64, pos(2), 0.4, 3, 48, 48, None, |aim, _, _| Some(
                if aim.x == 26 {
                    (10.0, 10.0)
                } else {
                    (8.0, 8.0)
                }
            ))
            .is_none()
        );
        p.sample(65, pos(26));
        assert!(
            p.choose(65, pos(2), 0.4, 3, 48, 48, None, |_, _, _| panic!(
                "cached decision rescored"
            ))
            .is_none()
        );
    }
    #[test]
    fn balanced_regret_rejects_a_large_stopping_detour() {
        let mut p = moving();
        let aim = p
            .choose(64, pos(2), 0.4, 3, 48, 48, None, |aim, _, _| {
                Some(if aim.x == 26 {
                    (40.0, 10.0)
                } else if aim.x <= 30 {
                    (25.0, 15.0)
                } else {
                    (10.0, 50.0)
                })
            })
            .unwrap();
        assert!(aim.x > 26 && aim.x <= 30);
        assert!(p.evidence.candidates <= 3);
    }
    #[test]
    fn invalid_contacts_leashes_and_zero_speed_never_score() {
        for (now, speed, leash) in [(65, 0.4, None), (64, 0.0, None), (64, 0.4, Some(pos(1)))] {
            assert!(
                moving()
                    .choose(now, pos(2), speed, 3, 48, 48, leash, |_, _, _| panic!(
                        "invalid candidate scored"
                    ))
                    .is_none()
            );
        }
    }
    #[test]
    fn suspension_does_not_destroy_motion_confidence() {
        let mut p = moving();
        let samples = p.samples.clone();
        p.suspend();
        assert_eq!(samples, p.samples);
        assert!(assess(&p.samples, &p.forecasts, 64).is_some());
        p.sample(65, pos(25));
        assert!(p.samples.len() < 3);
        assert!(p.aim.is_none());
    }
    #[test]
    fn checkpoints_restore_confidence_and_epoch_reset_discards_it() {
        let mut p = moving();
        let checkpoint = p.clone();
        p.sample(65, pos(25));
        assert_ne!(p, checkpoint);
        p = checkpoint.clone();
        assert_eq!(p, checkpoint);
        p.sample(1, pos(10));
        assert_eq!(p.samples.len(), 1);
    }
    #[test]
    fn historical_forecasts_survive_rolling_history_without_future_refits() {
        let mut p = moving();
        let fits = p.forecasts.clone();
        p.sample(65, pos(30));
        for old in fits.iter().filter(|s| s.0 > 0) {
            assert_eq!(p.forecasts.iter().find(|s| s.0 == old.0), Some(old));
        }
        assert_eq!(p.forecasts.len(), 65);
        p.sample(66, pos(10));
        assert_eq!(p.forecasts.len(), 1);
    }
}
