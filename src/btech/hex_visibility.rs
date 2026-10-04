//! Sight and sensor visibility of terrain coordinates without acquiring or identifying their occupants.
use super::{DetectionChannel, HexCoordinate, PerceptionProfile};
use crate::{ObjectId, World};
use anyhow::Result;
use std::cell::OnceCell;

/// Inspect an empty terrain target on the observer's map without contacts, randomness or mutation.
pub fn hex_visible(world: &World, observer: ObjectId, target: HexCoordinate) -> Result<bool> {
    Ok(hex_detection(world, observer, target)?.is_some())
}

/// Report which channel reaches a terrain coordinate: the sensor band or sight.
pub fn hex_detection(
    world: &World,
    observer: ObjectId,
    target: HexCoordinate,
) -> Result<Option<DetectionChannel>> {
    super::hex_perception(world, observer, target)
}

/// Artillery selects an observer by sight before applying its separate power-state check.
pub(super) fn observation_visible(
    world: &World,
    observer: ObjectId,
    target: HexCoordinate,
) -> Result<bool> {
    let profile = super::perception_profile(world, observer)?;
    Ok(
        super::perception::hex_perception_prepared(world, observer, &profile, target, false)?
            .is_some(),
    )
}

/// One observer's reach, computed once and reused while drawing many hexes of one map view.
pub(super) struct HexViewer<'w> {
    world: &'w World,
    observer: ObjectId,
    profile: OnceCell<PerceptionProfile>,
}

impl<'w> HexViewer<'w> {
    /// Defer the profile until a hex actually needs it.
    pub(super) fn new(world: &'w World, observer: ObjectId) -> Self {
        Self {
            world,
            observer,
            profile: OnceCell::new(),
        }
    }

    /// Whether the observer's sensor band or sight currently reaches this hex.
    pub(super) fn visible(&self, target: HexCoordinate) -> Result<bool> {
        let profile = match self.profile.get() {
            Some(profile) => profile,
            None => {
                let profile = super::perception_profile(self.world, self.observer)?;
                self.profile.get_or_init(|| profile)
            }
        };
        Ok(super::perception::hex_perception_prepared(
            self.world,
            self.observer,
            profile,
            target,
            true,
        )?
        .is_some())
    }
}
