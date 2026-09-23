//! Bounded motion estimates from visible hex-center samples only; no world or dice access.
use super::navigation::Hex;
use crate::{BattleHexCoordinate, BattlePoint, BattlePosition, ObjectId};

/// One synchronous controller's observation history, excluded from persistence.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Pursuit {
    key: Option<(u64, ObjectId, ObjectId)>,
    samples: Vec<(i64, BattlePosition)>,
    vector: Option<(f64, f64)>,
    aim: Option<BattlePosition>,
    reconsider_at: i64,
    /// A failed predicted search falls back until the next observed target hex.
    rejected_at: Option<BattlePosition>,
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
    pub fn sample(&mut self, order: u64, target: ObjectId, position: BattlePosition, now: i64) {
        let key = (order, target, position.map);
        if self.key != Some(key) || self.samples.last().is_some_and(|s| s.0 > now) {
            *self = Self {
                key: Some(key),
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
                    self.aim = None;
                    self.reconsider_at = now;
                }
                self.vector = Some(next);
            }
        }
        self.samples.retain(|s| now - s.0 <= 4);
        self.samples.push((now, position));
        if self.samples.len() > 6 {
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
        let &(last_time, last) = self.samples.last()?;
        if last_time != now || self.rejected_at == Some(last) {
            self.aim = None;
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
                legal(*p) && Hex::new(last.x, last.y).distance(Hex::new(p.x, p.y)) <= 3
            });
            return self.aim;
        }
        self.reconsider_at = now + 3;
        self.aim = None;
        let &(first_time, first) = self.samples.first()?;
        if self.samples.len() < 3 || last_time - first_time < 2 || speed <= 0.0 {
            return None;
        }
        let a = center(first);
        let b = center(last);
        let dt = (last_time - first_time) as f64;
        let v = ((b.x - a.x) / dt, (b.y - a.y) / dt);
        if v.0.hypot(v.1) < 1e-9 {
            return None;
        }
        let own = center(own);
        for horizon in 1..=5 {
            let scale = (horizon as f64).min(3.0 / v.0.hypot(v.1));
            let point = BattlePoint {
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
            if !legal(aim) || Hex::new(last.x, last.y).distance(Hex::new(x, y)) > 3 {
                return None;
            }
            let travel = (own.range(point).ok()? - f64::from(radius)).max(0.0) / speed;
            if travel <= horizon as f64 || horizon == 5 {
                self.aim = (aim != last).then_some(aim);
                return self.aim;
            }
        }
        None
    }

    /// Settling uses actual target geometry and cancels the current lead immediately.
    pub fn settle(&mut self) {
        self.aim = None;
        self.reconsider_at = 0;
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
