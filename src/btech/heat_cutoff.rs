//! Pilot-selected thermal regulation and its durable four-second cockpit transition.
use crate::{CommandAction, CommandContext, CommandInput, CommandReport, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Disabled cooling is measured in heat points, independently of physical heat sink damage.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleHeatCutoff {
    pub enabled: bool,
    pub disabled: u16,
    /// Committed seconds until the current setting flips; further toggles are rejected meanwhile.
    pub remaining: Option<u8>,
}

impl BattleHeatCutoff {
    /// Damage can reduce usable capacity before the next sample clamps disabled cooling.
    pub(super) fn validate(self, installed: u16) -> Result<()> {
        ensure!(
            self.disabled <= installed,
            "Invalid disabled cooling capacity"
        );
        ensure!(
            self.remaining.is_none_or(|n| (1..=4).contains(&n)),
            "Invalid heat cutoff transition"
        );
        Ok(())
    }

    /// Advance the toggle without imposing startup or map requirements on an admitted command.
    pub(super) fn tick(&mut self) {
        let Some(remaining) = self.remaining else {
            return;
        };
        if remaining > 1 {
            self.remaining = Some(remaining - 1);
            return;
        }
        self.remaining = None;
        self.enabled = !self.enabled;
    }

    /// Adjust the current environment sample, retaining its water bonus until the next sample.
    pub(super) fn regulate(&mut self, excess: f64, capacity: u16, maximum: u16) -> f64 {
        self.disabled = self.disabled.min(capacity);
        let previous = self.disabled;
        if !self.enabled || excess >= 10.0 {
            let requested = if self.enabled {
                (excess - 10.0).floor() + 1.0
            } else {
                f64::from(maximum)
            };
            self.disabled -= (requested.min(f64::from(maximum)) as u16).min(self.disabled);
        } else if excess < 9.0 {
            let requested = (9.0 - excess).floor() + 1.0;
            self.disabled +=
                (requested.min(f64::from(maximum)) as u16).min(capacity - self.disabled);
        }
        f64::from(previous) - f64::from(self.disabled)
    }
}

/// Begin a pilot-owned toggle; configuration gates commands, not already admitted transitions.
pub fn toggle_battle_heat_cutoff(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    allowed: bool,
) -> Result<super::BattleNotice> {
    ensure!(allowed, "This command has been disabled.");
    super::power::controlled_unit(world, id, pilot)?;
    let unit = world
        .btech
        .constructed
        .get_mut(&id)
        .context("Unit does not use Mech heat accounting")?;
    ensure!(
        unit.heat_cutoff.remaining.is_none(),
        "You are already toggling heat cutoff status. Please be patient."
    );
    unit.heat_cutoff.remaining = Some(4);
    Ok(super::BattleNotice {
        unit: id,
        text: if unit.heat_cutoff.enabled {
            "Disengaging heat dissipation cutoff..."
        } else {
            "Engaging heat dissipation cutoff..."
        }
        .into(),
    })
}

impl super::BattleUnit {
    /// Current regulation and transition state, including intentionally disabled cooling points.
    pub fn heat_cutoff(&self) -> BattleHeatCutoff {
        self.heat_cutoff
    }
}

/// Cockpit control uses the same toggle and notice boundary as trusted Lua callbacks.
pub(crate) fn command(ctx: &CommandContext<'_>, _input: &CommandInput) -> Result<CommandAction> {
    let result = (|| {
        let mut world = ctx.scripts.world.borrow_mut();
        let id = world
            .objects
            .get(&ctx.player)
            .and_then(|p| p.location)
            .context("Enter a unit first")?;
        toggle_battle_heat_cutoff(
            &mut world,
            id,
            ctx.player,
            ctx.config.battletech.heatcutoff > 0,
        )
    })();
    Ok(match result {
        Ok(notice) => CommandAction::CommitReply(notice.text),
        Err(error) => CommandAction::Report(CommandReport::Reply(format!("{error:#}"))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fractional boundaries, capacity bounds and both technology rates follow the same rule.
    #[test]
    fn regulation_boundaries_and_gradual_restoration() {
        for maximum in [2, 4] {
            for (excess, delta) in [
                (8.9, -1.0),
                (8.0, -2.0),
                (9.0, 0.0),
                (9.999, 0.0),
                (10.0, 1.0),
                (11.0, 2.0),
            ] {
                let mut state = BattleHeatCutoff {
                    enabled: true,
                    disabled: 5,
                    remaining: None,
                };
                assert_eq!(state.regulate(excess, 10, maximum), delta);
            }
            let mut state = BattleHeatCutoff {
                enabled: true,
                disabled: 0,
                remaining: None,
            };
            assert_eq!(state.regulate(-100.0, 10, maximum), -f64::from(maximum));
            state.disabled = 9;
            assert_eq!(state.regulate(-100.0, 10, maximum), -1.0);
            state.enabled = false;
            assert_eq!(state.regulate(0.0, 10, maximum), f64::from(maximum));
            while state.disabled != 0 {
                state.regulate(0.0, 10, maximum);
            }
            assert_eq!(state.regulate(100.0, 10, maximum), 0.0);
            state.disabled = 10;
            state.enabled = true;
            assert_eq!(state.regulate(9.5, 3, maximum), 0.0);
            assert_eq!(state.disabled, 3);
        }
    }
}
