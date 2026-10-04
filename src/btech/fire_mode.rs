//! Live firing modes on constructed units and the cockpit controls that change them.
use super::{FireMode, FireModeFeedback, Mech, Power};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

impl Mech {
    /// Current firing mode, distinct from the template's initial equipment flags.
    pub fn fire_mode(&self, index: usize) -> Result<FireMode> {
        ensure!(
            index < self.loadout()?.weapons.len(),
            "Weapon index out of bounds"
        );
        Ok(self.fire_modes.get(&index).copied().unwrap_or_default())
    }
}

/// Toggle an intact, recycled flamer heat mode or coolant self-application.
pub fn toggle_flamer_heat(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<FireMode> {
    let readiness = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        readiness.weapon.supports_heat_mode(),
        "That weapon cannot be set HEAT!"
    );
    Ok(super::weapon_controls::toggle_fire_mode(
        world,
        id,
        index,
        FireMode::Heat,
    ))
}

/// Parse one comma-delimited selection clause, validating its entire range before use.
/// The reference numbers at most 96 weapons, and reversed ranges run in ascending order.
fn selection_clause(clause: &str, weapons: usize) -> Result<std::ops::RangeInclusive<usize>> {
    let clause = clause.trim_start();
    let (first, last) = if let Some((first, last)) = clause.split_once('-') {
        let invalid = || format!("Invalid value: {first}");
        let start: i32 = first.trim().parse().with_context(invalid)?;
        let end: i32 = last.trim().parse().with_context(invalid)?;
        ensure!(
            (0..96).contains(&start),
            "Invalid first number in range ({start})"
        );
        ensure!(
            (0..96).contains(&end),
            "Invalid second number in range ({end})"
        );
        (start.min(end) as usize, start.max(end) as usize)
    } else {
        let number: i32 = clause
            .trim()
            .parse()
            .with_context(|| format!("Invalid value: {clause}"))?;
        ensure!((0..96).contains(&number), "Invalid weapon number: {number}");
        (number as usize, number as usize)
    };
    ensure!(
        last < weapons,
        "Error: the mech doesn't HAVE {} weapons!",
        last + 1
    );
    Ok(first..=last)
}

/// Native selections preserve earlier clauses on input errors; transaction failures undo all changes.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    selected_command(ctx, input, |world, id, pilot, index| {
        toggle_flamer_heat(world, id, pilot, index).map(|mode| mode.message(index))
    })
}

/// Share selection ordering and transaction boundaries across weapon-mode controls.
pub(super) fn selected_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
    mut toggle: impl FnMut(&mut World, ObjectId, ObjectId, usize) -> Result<String>,
) -> Result<crate::CommandAction> {
    let result = ctx.scripts.atomic(|_| -> Result<()> {
        let id = {
            let world = ctx.scripts.world.borrow();
            world
                .objects
                .get(&ctx.player)
                .and_then(|player| player.location)
                .context("Enter a unit first")?
        };
        let weapons = {
            let world = ctx.scripts.world.borrow();
            crate::btech::with_unit!(world.btech.unit(id).unwrap(), |unit| {
                super::power::controlled(&world, id, ctx.player)?;
                ensure!(unit.power() == Power::Running, "Start the unit first");
                unit.loadout()?.weapons.len()
            })
        };
        let notify_error = |message: String| {
            crate::notification::direct(
                &ctx.scripts.outbox,
                &crate::lua::configuration(&ctx.scripts.lua),
                ctx.player,
                message.into(),
            )
        };
        // The cockpit parser consumes one space/tab-delimited argument.
        let selection = input
            .args
            .split([' ', '\t'])
            .find(|part| !part.is_empty())
            .context("Please specify a weapon number.")?;
        for clause in selection.split(',') {
            let range = match selection_clause(clause, weapons) {
                Ok(range) => range,
                Err(error) => {
                    notify_error(error.to_string())?;
                    break;
                }
            };
            for index in range {
                let result = toggle(&mut ctx.scripts.world.borrow_mut(), id, ctx.player, index);
                match result {
                    Ok(message) => super::notify_unit_text(ctx.scripts, id, &message)?,
                    Err(error) => notify_error(error.to_string())?,
                }
            }
        }
        Ok(())
    });
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

/// Toggle a supported indirect-fire launcher's hotloading independently of its ammunition selection.
pub fn toggle_hotload(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<FireMode> {
    let ready = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        ready.weapon.supports_hotload(),
        "That weapon can not be hotloaded!"
    );
    Ok(super::weapon_controls::toggle_fire_mode(
        world,
        id,
        index,
        FireMode::Hotload,
    ))
}

/// Native hotload controls use the shared bounded weapon selection parser.
pub(crate) fn hotload_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    selected_command(ctx, input, |world, id, pilot, index| {
        toggle_hotload(world, id, pilot, index).map(|mode| mode.hotload_message(index))
    })
}

impl Mech {
    /// Select the firing behavior a supply check permits; the firing transaction persists fallback.
    pub(super) fn effective_fire_mode(&self, index: usize) -> Result<FireMode> {
        let loadout = self.loadout()?;
        self.effective_fire_mode_with_loadout(&loadout, index)
    }

    /// Preserve live supply fallback while sharing this immutable equipment projection.
    pub(crate) fn effective_fire_mode_with_loadout(
        &self,
        loadout: &super::MechLoadout,
        index: usize,
    ) -> Result<FireMode> {
        ensure!(index < loadout.weapons.len(), "Weapon index out of bounds");
        effective_mode(
            self.fire_modes.get(&index).copied().unwrap_or_default(),
            |rounds| self.ammunition_feed_with_loadout(loadout, index, rounds),
        )
    }
}

/// Resolve burst fallback through the caller's unit-specific inventory query.
pub(super) fn effective_mode(
    mode: FireMode,
    feed: impl FnOnce(u16) -> Result<Vec<super::AmmunitionDraw>>,
) -> Result<FireMode> {
    let requested = mode.rounds_per_cycle();
    if requested == 1 {
        return Ok(mode);
    }
    let available: u16 = feed(requested)?.iter().map(|draw| draw.rounds).sum();
    Ok(if available < requested {
        FireMode::Normal
    } else {
        mode
    })
}

#[cfg(test)]
mod tests {
    use super::selection_clause;

    #[test]
    fn selection_ranges_are_bounded_and_reversed_ranges_are_ascending() {
        assert_eq!(
            selection_clause("3-1", 4).unwrap().collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert_eq!(selection_clause("+0", 1).unwrap(), 0..=0);
        assert_eq!(selection_clause("95", 96).unwrap(), 95..=95);
        for clause in [
            "",
            "-1",
            "96",
            "0-96",
            "96-0",
            "0-4",
            "1-2-3",
            "x",
            "2147483648",
        ] {
            assert!(selection_clause(clause, 4).is_err(), "{clause}");
        }
    }
}
