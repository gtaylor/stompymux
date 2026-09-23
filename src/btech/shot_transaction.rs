//! Shot-owned candidates and explicit nested effect savepoints.
//!
//! An in-shot effect may mutate its owner's unpublished world because its caller
//! propagates every error. Standalone effects retain their own atomic savepoint.
use crate::World;
use std::ops::{Deref, DerefMut};

/// An independently committed shot. Dropping it discards every staged effect.
pub(super) struct ShotCandidate(Option<World>);

impl ShotCandidate {
    pub(super) fn new(world: &World) -> Self {
        Self(Some(super::autopilot::diagnostics::candidate(world, false)))
    }

    pub(super) fn mode(&self) -> EffectMode {
        #[cfg(test)]
        if REFERENCE.with(|reference| reference.get()) {
            return EffectMode::Atomic;
        }
        EffectMode::InShot
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

/// Only shot entry points choose InShot; all standalone adapters use Atomic.
#[derive(Clone, Copy)]
pub(super) enum EffectMode {
    Atomic,
    InShot,
}

/// Either a standalone savepoint or a borrow of the enclosing shot candidate.
pub(super) enum EffectCandidate<'a> {
    Atomic {
        candidate: Box<World>,
        destination: &'a mut World,
    },
    InShot(&'a mut World),
}

impl<'a> EffectCandidate<'a> {
    pub(super) fn new(world: &'a mut World, mode: EffectMode) -> Self {
        match mode {
            EffectMode::Atomic => Self::Atomic {
                candidate: Box::new(super::autopilot::diagnostics::candidate(world, true)),
                destination: world,
            },
            EffectMode::InShot => Self::InShot(world),
        }
    }

    pub(super) fn commit(self) {
        if let Self::Atomic {
            candidate,
            destination,
        } = self
        {
            *destination = *candidate;
        }
    }
}
impl Deref for EffectCandidate<'_> {
    type Target = World;
    fn deref(&self) -> &World {
        match self {
            Self::Atomic { candidate, .. } => candidate,
            Self::InShot(world) => world,
        }
    }
}
impl DerefMut for EffectCandidate<'_> {
    fn deref_mut(&mut self) -> &mut World {
        match self {
            Self::Atomic { candidate, .. } => candidate,
            Self::InShot(world) => world,
        }
    }
}

#[cfg(test)]
thread_local! {
    static REFERENCE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAILURE: std::cell::Cell<Option<FailurePoint>> = const { std::cell::Cell::new(None) };
}

/// Test oracle uses the same rules with an independent savepoint for each salvo.
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
