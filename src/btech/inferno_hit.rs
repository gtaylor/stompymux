//! Atomic inferno missile exposure, shared by admitted hits and ammunition explosions.
use super::{Notice, apply_inferno_burn, extinguish_inferno_in_water};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// An inferno exposure has no armor damage, location rolls or immediate heat addition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish inferno notices within the enclosing attack checkpoint"]
pub struct InfernoHit {
    /// Target whose burn state was changed.
    pub target: ObjectId,
    /// Missiles that reached the target after interception and cluster selection.
    pub missiles: u16,
    /// Duration added before any immediate water extinguishing.
    pub burn_seconds: u32,
    /// Whether immersion canceled the resulting burn.
    pub extinguished: bool,
    /// Cockpit and observer messages captured in event order.
    pub notices: Vec<Notice>,
}

/// Resolve a positive, admitted inferno exposure to a constructed biped.
/// The enclosing caller owns firing authorization, ammunition and interception.
/// Each pair of missiles, rounded up, adds three minutes; immersion follows ignition.
pub fn resolve_inferno_hit(
    world: &mut World,
    target: ObjectId,
    missiles: u16,
) -> Result<InfernoHit> {
    ensure!(missiles > 0, "Inferno hit requires at least one missile");
    let burning = world
        .btech
        .constructed_units()
        .get(&target)
        .context("Unit is not constructed")?
        .inferno_remaining()
        > 0;
    let burn_seconds = duration(missiles);
    let mut candidate = world.clone();
    apply_inferno_burn(&mut candidate, target, burn_seconds)?;
    let mut notices = exposure_notices(world, target, burning);
    notices.extend(extinguish_inferno_in_water(&mut candidate, target)?);
    let extinguished = candidate.btech.constructed_units()[&target].inferno_remaining() == 0;
    candidate.btech.validate_action(&candidate)?;
    *world = candidate;
    Ok(InfernoHit {
        target,
        missiles,
        burn_seconds,
        extinguished,
        notices,
    })
}

/// Shared initial inferno feedback, before anatomy-specific burning or explosion effects.
pub(super) fn exposure_notices(world: &World, target: ObjectId, burning: bool) -> Vec<Notice> {
    let mut notices = super::broadcast::observer_notices(
        world,
        target,
        if burning {
            "burns a bit more brightly."
        } else {
            "suddenly bursts into flames!"
        },
    );
    notices.push(Notice {
        unit: target,
        text: if burning {
            "[fg=red bold]More burning jelly joins the flames![reset]"
        } else {
            "[fg=red bold]You are sprayed with burning jelly![reset]"
        }
        .into(),
    });
    notices
}

/// Each pair of surviving inferno missiles supplies three minutes of burning jelly.
pub(super) fn duration(missiles: u16) -> u32 {
    u32::from(missiles).div_ceil(2) * 180
}
