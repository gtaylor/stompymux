//! Optional, synchronous CPU attribution for autopilot benchmark phases.
use serde::Serialize;
use std::cell::RefCell;
use std::time::Instant;

/// Inclusive timings: nested categories must not be added together.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AutopilotDiagnostics {
    pub calls: [u64; 7],
    pub nanoseconds: [u128; 7],
    /// Named, inclusive combat measurements and counters; absent when disabled.
    pub combat: std::collections::BTreeMap<String, CombatSample>,
    /// Named pursuit costs; independent of the established combat categories.
    pub pursuit: std::collections::BTreeMap<String, CombatSample>,
}

/// One named measurement; counters have zero nanoseconds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CombatSample {
    pub calls: u64,
    pub nanoseconds: u128,
}

impl AutopilotDiagnostics {
    /// Accumulate one synchronous phase or benchmark sample.
    pub(crate) fn merge(&mut self, other: &Self) {
        for i in 0..7 {
            self.calls[i] += other.calls[i];
            self.nanoseconds[i] += other.nanoseconds[i];
        }
        for (name, sample) in &other.pursuit {
            let total = self.pursuit.entry(name.clone()).or_default();
            total.calls += sample.calls;
            total.nanoseconds += sample.nanoseconds;
        }
        for (name, sample) in &other.combat {
            let total = self.combat.entry(name.clone()).or_default();
            total.calls += sample.calls;
            total.nanoseconds += sample.nanoseconds;
        }
    }
}

/// Named combat attribution. Disabled execution reads no clock and allocates nothing.
pub(crate) fn combat(name: &'static str) -> CombatMeasurement {
    CombatMeasurement {
        name,
        pursuit: false,
        started: ACTIVE.with(|slot| slot.borrow().as_ref().map(|_| Instant::now())),
    }
}

/// Named navigation attribution; disabled execution has no timer or allocation.
pub(crate) fn pursuit(name: &'static str) -> CombatMeasurement {
    let mut measurement = combat(name);
    measurement.pursuit = true;
    measurement
}

pub(crate) struct CombatMeasurement {
    name: &'static str,
    pursuit: bool,
    started: Option<Instant>,
}

impl Drop for CombatMeasurement {
    fn drop(&mut self) {
        if let Some(started) = self.started {
            let elapsed = started.elapsed().as_nanos();
            if self.pursuit {
                ACTIVE.with(|slot| {
                    if let Some(total) = slot.borrow_mut().as_mut() {
                        let sample = total.pursuit.entry(self.name.to_owned()).or_default();
                        sample.calls += 1;
                        sample.nanoseconds += elapsed;
                    }
                });
            } else {
                record(self.name, elapsed);
            }
        }
    }
}

fn record(name: &'static str, nanoseconds: u128) {
    ACTIVE.with(|slot| {
        if let Some(total) = slot.borrow_mut().as_mut() {
            let sample = total.combat.entry(name.to_owned()).or_default();
            sample.calls += 1;
            sample.nanoseconds += nanoseconds;
        }
    });
}

/// Count attempted work without timing it.
pub(crate) fn count(name: &'static str) {
    record(name, 0);
}

/// Count bounded pursuit work with no clock reads; publication belongs to the enclosing tick.
pub(crate) fn pursuit_count(name: &'static str) {
    ACTIVE.with(|slot| {
        if let Some(total) = slot.borrow_mut().as_mut() {
            total.pursuit.entry(name.to_owned()).or_default().calls += 1;
        }
    });
}

/// Count rejection on every early return, including errors after expenditure.
pub(crate) struct Attempt(bool);
impl Attempt {
    pub(crate) fn begin() -> Self {
        count("attempted_shots");
        Self(false)
    }
    pub(crate) fn succeed(&mut self) {
        self.0 = true;
    }
}
impl Drop for Attempt {
    fn drop(&mut self) {
        count(if self.0 {
            "successful_shots"
        } else {
            "rejected_shots"
        });
    }
}

/// Attribute cloning separately from copy-on-write detachments during effects.
pub(crate) fn candidate(world: &crate::World, nested: bool) -> crate::World {
    let _measurement = combat("candidate_creation");
    count(if nested {
        "nested_transactions"
    } else {
        "shot_transactions"
    });
    world.clone()
}

/// Time only copy-on-write operations that actually detach shared storage.
pub(crate) fn make_mut<T: Clone>(value: &mut std::sync::Arc<T>) -> &mut T {
    let _measurement = (std::sync::Arc::strong_count(value) > 1
        || std::sync::Arc::weak_count(value) > 0)
        .then(|| combat("copy_on_write"));
    std::sync::Arc::make_mut(value)
}

/// Inclusive state validation plus its time outside local unit/map checks.
pub(crate) struct ValidationMeasurement {
    started: Option<Instant>,
    local_before: u128,
}
fn local_time(total: &AutopilotDiagnostics) -> u128 {
    ["validation_unit", "validation_map"]
        .iter()
        .filter_map(|name| total.combat.get(*name))
        .map(|sample| sample.nanoseconds)
        .sum()
}
pub(crate) fn validation() -> ValidationMeasurement {
    ACTIVE.with(|slot| {
        let active = slot.borrow();
        ValidationMeasurement {
            started: active.as_ref().map(|_| Instant::now()),
            local_before: active.as_ref().map_or(0, local_time),
        }
    })
}
impl Drop for ValidationMeasurement {
    fn drop(&mut self) {
        if let Some(started) = self.started {
            let elapsed = started.elapsed().as_nanos();
            let local = ACTIVE.with(|slot| slot.borrow().as_ref().map_or(0, local_time));
            record("validation", elapsed);
            record(
                "validation_cross_object",
                elapsed.saturating_sub(local.saturating_sub(self.local_before)),
            );
        }
    }
}

/// Stable diagnostic categories, in CSV/JSON array order.
#[derive(Clone, Copy)]
pub(crate) enum Category {
    Contacts,
    Geometry,
    Sensors,
    Illumination,
    Readiness,
    Selection,
    Shots,
}

thread_local! {
    static ACTIVE: RefCell<Option<AutopilotDiagnostics>> = const { RefCell::new(None) };
}

/// Scope only synchronous decision work; never hold across an await.
pub(crate) struct Scope(Option<AutopilotDiagnostics>);
impl Scope {
    pub(crate) fn begin(enabled: bool) -> Self {
        Self(ACTIVE.with(|slot| slot.replace(enabled.then(AutopilotDiagnostics::default))))
    }
    pub(crate) fn finish(self, total: &mut AutopilotDiagnostics) {
        ACTIVE.with(|slot| {
            if let Some(current) = slot.borrow().as_ref() {
                total.merge(current);
            }
        });
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        ACTIVE.with(|slot| {
            slot.replace(self.0.take());
        });
    }
}

/// No clock read or allocation when diagnostics are disabled.
pub(crate) fn measure(category: Category) -> Measurement {
    Measurement {
        category,
        started: ACTIVE.with(|slot| slot.borrow().as_ref().map(|_| Instant::now())),
    }
}
pub(crate) struct Measurement {
    category: Category,
    started: Option<Instant>,
}
impl Drop for Measurement {
    fn drop(&mut self) {
        if let Some(started) = self.started {
            ACTIVE.with(|slot| {
                if let Some(total) = slot.borrow_mut().as_mut() {
                    total.calls[self.category as usize] += 1;
                    total.nanoseconds[self.category as usize] += started.elapsed().as_nanos();
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn combat_counts_early_returns_and_merges_only_enabled_scopes() {
        let mut total = AutopilotDiagnostics::default();
        let scope = Scope::begin(true);
        {
            let _attempt = Attempt::begin();
        }
        {
            let mut attempt = Attempt::begin();
            attempt.succeed();
        }
        {
            let _measurement = combat("admission_aim");
            let _pursuit = pursuit("estimation");
        }
        {
            let disabled = Scope::begin(false);
            assert!(combat("disabled").started.is_none());
            assert!(pursuit("disabled").started.is_none());
            let attempt = Attempt::begin();
            drop(attempt);
            disabled.finish(&mut total);
        }
        scope.finish(&mut total);
        assert_eq!(total.combat["attempted_shots"].calls, 2);
        assert_eq!(total.combat["rejected_shots"].calls, 1);
        assert_eq!(total.combat["successful_shots"].calls, 1);
        assert_eq!(total.combat["admission_aim"].calls, 1);
        assert!(!total.combat.contains_key("disabled"));
        assert_eq!(total.pursuit["estimation"].calls, 1);
        assert!(!total.pursuit.contains_key("disabled"));
        assert!(combat("outside").started.is_none());
    }

    #[test]
    fn attribution_is_scoped_and_disabled_by_default() {
        let mut total = AutopilotDiagnostics::default();
        {
            let scope = Scope::begin(true);
            {
                let _sample = measure(Category::Contacts);
            }
            {
                let disabled = Scope::begin(false);
                {
                    let _sample = measure(Category::Shots);
                }
                disabled.finish(&mut AutopilotDiagnostics::default());
            }
            scope.finish(&mut total);
        }
        assert_eq!(total.calls, [1, 0, 0, 0, 0, 0, 0]);
        assert!(ACTIVE.with(|slot| slot.borrow().is_none()));
        {
            let _sample = measure(Category::Shots);
        }
        assert!(ACTIVE.with(|slot| slot.borrow().is_none()));
    }
}
