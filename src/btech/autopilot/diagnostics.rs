//! Optional, synchronous CPU attribution for autopilot benchmark phases.
use serde::Serialize;
use std::cell::RefCell;
use std::time::Instant;

/// Inclusive timings: nested categories must not be added together.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AutopilotDiagnostics {
    pub calls: [u64; 7],
    pub nanoseconds: [u128; 7],
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
                for i in 0..7 {
                    total.calls[i] += current.calls[i];
                    total.nanoseconds[i] += current.nanoseconds[i];
                }
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
