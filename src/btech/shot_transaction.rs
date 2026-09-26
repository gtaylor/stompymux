//! Shot-owned candidate worlds and test hooks for shot rollback.
//!
//! A shot resolves in a private copy of the world and commits it only on success,
//! so every effect inside the shot mutates that copy directly. Standalone effects
//! wrap themselves in [`World::attempt`] instead.
use crate::World;
use std::ops::{Deref, DerefMut};

/// An independently committed shot. Dropping it discards every staged effect.
pub(super) struct ShotCandidate(Option<World>);

impl ShotCandidate {
    pub(super) fn new(world: &World) -> Self {
        Self(Some(super::autopilot::diagnostics::candidate(world)))
    }

    pub(super) fn commit(mut self, world: &mut World) {
        *world = self.0.take().expect("uncommitted shot");
    }
}

impl Deref for ShotCandidate {
    type Target = World;
    fn deref(&self) -> &World {
        self.0.as_ref().expect("uncommitted shot")
    }
}
impl DerefMut for ShotCandidate {
    fn deref_mut(&mut self) -> &mut World {
        self.0.as_mut().expect("uncommitted shot")
    }
}

#[cfg(test)]
thread_local! {
    static REFERENCE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAILURE: std::cell::Cell<Option<FailurePoint>> = const { std::cell::Cell::new(None) };
}

/// Test oracle that turns off validation shortcuts, such as the dense contact position index.
#[cfg(test)]
pub(super) struct ReferenceScope(bool);
#[cfg(test)]
impl ReferenceScope {
    pub(super) fn begin() -> Self {
        Self(REFERENCE.with(|flag| flag.replace(true)))
    }
}
#[cfg(test)]
impl Drop for ReferenceScope {
    fn drop(&mut self) {
        REFERENCE.with(|flag| flag.set(self.0));
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FailurePoint {
    Expenditure,
    Damage,
    Validation,
}
#[cfg(test)]
pub(super) struct FailureScope(Option<FailurePoint>);
#[cfg(test)]
impl FailureScope {
    pub(super) fn begin(point: FailurePoint) -> Self {
        Self(FAILURE.with(|failure| failure.replace(Some(point))))
    }
}
#[cfg(test)]
impl Drop for FailureScope {
    fn drop(&mut self) {
        FAILURE.with(|failure| failure.set(self.0));
    }
}
#[cfg(test)]
pub(super) fn checkpoint(point: FailurePoint) -> anyhow::Result<()> {
    anyhow::ensure!(
        !FAILURE.with(|failure| failure.get() == Some(point)),
        "Injected shot failure: {point:?}"
    );
    Ok(())
}

impl Drop for ShotCandidate {
    fn drop(&mut self) {
        if self.0.is_some() {
            super::validation_context::invalidate();
            super::equipment_context::invalidate();
        }
    }
}

#[cfg(test)]
#[path = "shot_transaction_tests.rs"]
mod tests;

#[cfg(test)]
pub(super) fn reference_enabled() -> bool {
    REFERENCE.with(|reference| reference.get())
}
