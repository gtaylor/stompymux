//! Pilot-chosen heat-sink cutoff: shutting off any number of heat sinks to build up heat
//! (Total Warfare p. 159, Shutting Off Heat Sinks).
use crate::{CommandAction, CommandContext, CommandInput, CommandReport, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Heat sinks the pilot has shut off, counted in sinks rather than heat points so that a
/// double heat sink is one sink.
///
/// The book lets a MechWarrior shut off as many heat sinks as desired during the End Phase;
/// they dissipate nothing in the following Heat Phase and may only be switched back on in a
/// later End Phase (Total Warfare p. 159). Here the request is recorded at once and both
/// shutting off and switching back on take effect at the unit's next thirty-second turn
/// boundary.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeatCutoff {
    /// Sinks the pilot asked to shut off, applied at the next turn boundary.
    pub requested: u16,
    /// Sinks shut off for the current turn, dissipating no heat.
    pub shut_off: u16,
}

impl HeatCutoff {
    /// Neither count may exceed the sinks the Mech carries; damage clamps both later.
    pub(super) fn validate(self, installed: u16) -> Result<()> {
        ensure!(
            self.requested <= installed && self.shut_off <= installed,
            "Invalid heat sink cutoff"
        );
        Ok(())
    }

    /// Whether a request waits for the next turn boundary.
    pub fn pending(self) -> bool {
        self.requested != self.shut_off
    }

    /// Make the request the current setting, as the turn boundary does.
    pub(super) fn apply(&mut self) {
        self.shut_off = self.requested;
    }

    /// Keep both counts within the sinks that still work after damage.
    pub(super) fn clamp(&mut self, working: u16) {
        self.requested = self.requested.min(working);
        self.shut_off = self.shut_off.min(working);
    }

    /// Heat points the shut-off sinks would otherwise dissipate, bounded by surviving cooling.
    pub(super) fn disabled_points(self, points_each: u16, capacity: u16) -> u16 {
        self.shut_off.saturating_mul(points_each).min(capacity)
    }
}

/// Ask to shut off `sinks` heat sinks (zero switches every sink back on). The change takes
/// effect at the unit's next turn boundary; asking again before then replaces the request.
pub fn set_battle_heat_cutoff(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    sinks: u16,
) -> Result<super::Notice> {
    super::power::controlled_unit(world, id, pilot)?;
    let unit = world
        .btech
        .constructed
        .get_mut(&id)
        .context("Unit does not use Mech heat accounting")?;
    let working = unit.working_heat_sinks();
    ensure!(
        sinks <= working,
        "Only {working} heat sinks are working on this 'Mech"
    );
    unit.heat_cutoff.requested = sinks;
    Ok(super::Notice {
        unit: id,
        text: describe(unit.heat_cutoff, working),
    })
}

/// Describe the current cutoff and any request still waiting for the turn boundary.
fn describe(cutoff: HeatCutoff, working: u16) -> String {
    let current = format!("{} of {working} heat sinks shut off", cutoff.shut_off);
    if !cutoff.pending() {
        return format!("{current}.");
    }
    format!(
        "{current}; {} will be shut off from the next turn.",
        cutoff.requested
    )
}

impl super::Mech {
    /// Current and requested shut-off heat sinks.
    pub fn heat_cutoff(&self) -> HeatCutoff {
        self.heat_cutoff
    }

    /// Heat points one heat sink dissipates a turn: two for double heat sinks.
    pub(super) fn heat_sink_points_each(&self) -> u16 {
        self.definition().heat_per_sink()
    }

    /// Heat sinks that still work, whether or not the pilot has shut any off.
    pub fn working_heat_sinks(&self) -> u16 {
        self.cooling_capacity() / self.heat_sink_points_each()
    }
}

/// `heatcutoff <sinks|all>` shuts off that many heat sinks from the next turn; `heatcutoff`
/// alone reports the current setting.
pub(crate) fn command(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<CommandAction> {
    let result: Result<(bool, String)> = (|| {
        let mut world = ctx.scripts.world.borrow_mut();
        let id = world
            .objects
            .get(&ctx.player)
            .and_then(|p| p.location)
            .context("Enter a unit first")?;
        let argument = input.args.trim();
        if argument.is_empty() {
            super::power::controlled_unit(&world, id, ctx.player)?;
            let unit = world
                .btech
                .constructed_units()
                .get(&id)
                .context("Unit does not use Mech heat accounting")?;
            return Ok((
                false,
                describe(unit.heat_cutoff(), unit.working_heat_sinks()),
            ));
        }
        let sinks = if argument.eq_ignore_ascii_case("all") {
            world
                .btech
                .constructed_units()
                .get(&id)
                .context("Unit does not use Mech heat accounting")?
                .working_heat_sinks()
        } else {
            argument
                .parse::<u16>()
                .context("Give the number of heat sinks to shut off, or 'all'")?
        };
        let notice = set_battle_heat_cutoff(&mut world, id, ctx.player, sinks)?;
        Ok((true, notice.text))
    })();
    Ok(match result {
        Ok((true, text)) => CommandAction::CommitReply(text),
        Ok((false, text)) => CommandAction::Report(CommandReport::Reply(text)),
        Err(error) => CommandAction::Report(CommandReport::Reply(format!("{error:#}"))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Requests wait for the boundary, damage clamps both counts and doubles count twice.
    #[test]
    fn requests_apply_at_the_boundary_and_stay_within_working_sinks() {
        let mut cutoff = HeatCutoff {
            requested: 4,
            shut_off: 0,
        };
        assert!(cutoff.pending());
        assert_eq!(cutoff.disabled_points(1, 10), 0);
        cutoff.apply();
        assert!(!cutoff.pending());
        assert_eq!(cutoff.disabled_points(1, 10), 4);
        assert_eq!(cutoff.disabled_points(2, 10), 8);
        assert_eq!(cutoff.disabled_points(2, 6), 6);
        cutoff.clamp(3);
        assert_eq!(
            cutoff,
            HeatCutoff {
                requested: 3,
                shut_off: 3
            }
        );
        assert!(cutoff.validate(3).is_ok());
        assert!(cutoff.validate(2).is_err());
        assert_eq!(describe(cutoff, 3), "3 of 3 heat sinks shut off.");
        cutoff.requested = 0;
        assert_eq!(
            describe(cutoff, 3),
            "3 of 3 heat sinks shut off; 0 will be shut off from the next turn."
        );
    }
}
