//! Pilot-owned turn preference actions and unit-owned persistent mode.
use crate::{ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};
use std::sync::Arc;

impl super::BattleUnit {
    /// Whether slowdown mode two applies the extra tight-turn speed reduction.
    pub fn tight_turn_mode(&self) -> bool {
        self.tight_turn_mode
    }
}

impl super::BattleVehicle {
    /// Whether slowdown mode two applies the extra tight-turn speed reduction.
    pub fn tight_turn_mode(&self) -> bool {
        self.tight_turn_mode
    }
}

/// Read the saved preference for either supported anatomy.
pub(super) fn selected(world: &World, id: ObjectId) -> Result<bool> {
    if let Some(unit) = world.btech.constructed_units().get(&id) {
        return Ok(unit.tight_turn_mode());
    }
    Ok(world
        .btech
        .vehicles()
        .get(&id)
        .context("Unit is unavailable")?
        .tight_turn_mode())
}

/// Set the authoritative preference after the enclosing action admits its actor.
pub(super) fn set(world: &mut World, id: ObjectId, enabled: bool) -> Result<()> {
    if let Some(unit) = Arc::make_mut(&mut world.btech.constructed).get_mut(&id) {
        unit.tight_turn_mode = enabled;
    } else {
        Arc::make_mut(&mut world.btech.vehicles)
            .get_mut(&id)
            .context("Unit is unavailable")?
            .tight_turn_mode = enabled;
    }
    Ok(())
}

/// Shared turning throttle adjustment after heading changes, before acceleration.
pub(super) fn throttle(target: f64, remaining: f64, slowdown: i64, tight: bool) -> f64 {
    let remaining = remaining.abs();
    let factor = match slowdown {
        1 if remaining != 0.0 => 2.0 / 3.0,
        1 => 0.75,
        2 if remaining >= 1.0 => (8.0 - ((remaining - 1.0) / 30.0).floor()) / 10.0,
        _ => 1.0,
    };
    target * factor
        - if slowdown == 2 && remaining >= 1.0 && tight {
            10.75 * 0.4
        } else {
            0.0
        }
}

/// Set tight/normal turning or report the current mode, with atomic occupant notification.
pub fn turnmode(
    scripts: &Scripts,
    unit: ObjectId,
    pilot: ObjectId,
    argument: &str,
) -> Result<super::BattleNotice> {
    let before = scripts.world.borrow().clone();
    let effects = scripts.effects.checkpoint();
    let result = (|| {
        super::radio::controlled(&scripts.world(), unit, pilot)?;
        ensure!(
            super::lateral::maneuvering_ace(&scripts.world(), pilot),
            "You're not skilled enough to do that."
        );
        let mode = match argument.trim().to_ascii_lowercase().as_str() {
            "tight" => Some(true),
            "normal" => Some(false),
            _ => None,
        };
        let text = if let Some(mode) = mode {
            set(&mut scripts.world_mut(), unit, mode)?;
            if mode {
                "You brace for tighter turns.".into()
            } else {
                "You assume a normal turn mode.".into()
            }
        } else {
            format!(
                "Your turning type is : {}",
                if selected(&scripts.world(), unit)? {
                    "TIGHT"
                } else {
                    "NORMAL"
                }
            )
        };
        super::notify_unit_text(scripts, unit, &text)?;
        Ok(super::BattleNotice { unit, text })
    })();
    if result.is_err() {
        *scripts.world.borrow_mut() = before;
        scripts.effects.restore(effects);
    }
    result
}

/// Native turning control resolves the invoking player's current cockpit.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let unit = ctx
            .scripts
            .world()
            .objects
            .get(&ctx.player)
            .and_then(|player| player.location)
            .context("Enter a unit first")?;
        turnmode(ctx.scripts, unit, ctx.player, &input.args)
    })();
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
