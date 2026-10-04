//! Reactor section-loss admission and bounded committed timing, shared by every damage source.
use super::{Mech, Power};
use crate::World;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Global startup grace preserves the zero-initialized reference timestamp without an unbounded clock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ReactorState {
    pub(crate) startup_remaining: u8,
    /// Host configuration participates in checkpoints but is not saved as scenario state.
    #[serde(default)]
    pub(crate) enabled: bool,
    #[serde(default)]
    pub(crate) punch: bool,
}

impl Default for ReactorState {
    fn default() -> Self {
        Self {
            startup_remaining: 31,
            enabled: true,
            punch: false,
        }
    }
}

impl ReactorState {
    /// Thirty elapsed seconds remain eligible; the thirty-first closes the window.
    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(
            self.startup_remaining <= 31,
            "Invalid reactor startup window"
        );
        Ok(())
    }
}

impl Mech {
    /// None uses the initial world startup grace; zero denotes an expired damage window.
    pub fn reactor_instability_remaining(&self) -> Option<u8> {
        self.reactor_instability_remaining
    }
}

/// Configure section-loss explosions without changing an existing damage window.
pub fn configure_battle_reactor_policy(world: &mut World, enabled: bool, punch: bool) {
    world.btech.reactor.enabled = enabled;
    world.btech.reactor.punch = punch;
}

/// Idle clocks still expire, including while their reactors are stopped.
pub fn reactor_windows_pending(world: &World) -> bool {
    world.btech.reactor.startup_remaining > 0
        || world.btech.constructed_units().values().any(|unit| {
            unit.reactor_instability_remaining
                .is_some_and(|remaining| remaining > 0)
        })
}

/// Advance bounded clocks once within the host's commit checkpoint.
pub fn advance_battle_reactor_windows(world: &mut World) {
    world.btech.reactor.startup_remaining = world.btech.reactor.startup_remaining.saturating_sub(1);
    for unit in world.btech.constructed.values_mut() {
        if let Some(remaining) = &mut unit.reactor_instability_remaining {
            *remaining = remaining.saturating_sub(1);
        }
    }
}

/// Called when section loss reaches three engine losses, with power captured before destruction.
pub(super) fn triggered(world: &mut World, id: crate::ObjectId, power: Power) -> bool {
    if !world.btech.reactor.enabled {
        return false;
    }
    let initial = world.btech.reactor.startup_remaining;
    let unit = world.btech.constructed.get_mut(&id).unwrap();
    if unit.reactor_instability_remaining.unwrap_or(initial) == 0 {
        return false;
    }
    let roll = unit.dice.generic_roll();
    roll >= 9 && matches!(power, Power::Running | Power::Starting { .. })
}

/// Section destruction and flooding share engine-loss admission and blast resolution.
pub(super) fn section_loss(
    world: &mut World,
    id: crate::ObjectId,
    power: Power,
    previous_hits: u8,
    rules: super::FallRules,
) -> Result<Option<super::ReactorExplosion>> {
    if previous_hits >= 3
        || world.btech.constructed_units()[&id].system_hits(super::System::Engine) < 3
        || !triggered(world, id, power)
    {
        return Ok(None);
    }
    let position = world.btech.constructed_units()[&id]
        .position()
        .context("Reactor must be on a map")?;
    let mut notices = super::broadcast::hex_notices(
        world,
        position.map,
        super::HexCoordinate {
            x: i32::from(position.x),
            y: i32::from(position.y),
        },
        true,
        |_| {
            "[fg=red bold]The hit destroys the last safety systems, releasing the fusion reaction![reset]".into()
        },
    )?;
    let mut blast =
        super::reactor_explosion::detonate(world, id, rules, world.btech.reactor.punch)?;
    let private = std::mem::take(&mut blast.pilot_notices);
    super::piloting::append_feedback(&mut blast.pilot_notices, private, notices.len());
    notices.append(&mut blast.notices);
    blast.notices = notices;
    Ok(Some(blast))
}
