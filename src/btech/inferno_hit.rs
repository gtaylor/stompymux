//! Atomic inferno missile exposure of a Mech: two heat points per missile (Total Warfare p. 141).
use super::Notice;
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// An inferno exposure has no armor damage or location rolls; it heats the target instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish inferno notices within the enclosing attack checkpoint"]
pub struct InfernoHit {
    /// Target whose heat was raised.
    pub target: ObjectId,
    /// Missiles that reached the target after interception and cluster selection.
    pub missiles: u16,
    /// Heat points added to the target's Heat Scale: two per missile, within the turn's
    /// fifteen-point cap on outside heat (Total Warfare p. 159).
    pub heat: u8,
    /// Cockpit and observer messages captured in event order.
    pub notices: Vec<Notice>,
}

/// Resolve a positive, admitted inferno exposure to a constructed biped.
/// The enclosing caller owns firing authorization, ammunition and interception.
/// Every missile that strikes a Mech raises its heat by two points for the turn in which it
/// hits; the heat then dissipates like any other (Total Warfare p. 141).
pub fn resolve_inferno_hit(
    world: &mut World,
    target: ObjectId,
    missiles: u16,
) -> Result<InfernoHit> {
    ensure!(missiles > 0, "Inferno hit requires at least one missile");
    ensure!(
        world
            .objects
            .get(&target)
            .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
        "Unit is unavailable"
    );
    let unit = world
        .btech
        .constructed
        .get_mut(&target)
        .context("Unit is not constructed")?;
    let points = missiles
        .saturating_mul(2)
        .min(u16::from(super::overheat::MAX_OUTSIDE_HEAT)) as u8;
    let heat = unit.absorb_outside_heat(points);
    let mut notices = super::broadcast::observer_notices(
        world,
        target,
        "is splashed with burning inferno jelly!",
    );
    notices.push(Notice {
        unit: target,
        text: format!(
            "[fg=red bold]You are sprayed with burning jelly! Your heat rises by {heat}.[reset]"
        ),
    });
    Ok(InfernoHit {
        target,
        missiles,
        heat,
        notices,
    })
}

/// Initial feedback for a unit that infernos set burning, before its chassis-specific effects.
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

/// Burning jelly duration for vehicles, which keep burning after inferno hits: each pair of
/// surviving missiles, rounded up, supplies three minutes.
pub(super) fn duration(missiles: u16) -> u32 {
    u32::from(missiles).div_ceil(2) * 180
}
