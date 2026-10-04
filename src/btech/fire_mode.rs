//! Persistent firing modes and cockpit changes, independent of command or Lua dispatch.
use super::{BattlePower, BattleUnit, BattleWeapon};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Supported live weapon behavior; normal is implicit when no override is stored.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleFireMode {
    #[default]
    Normal,
    Heat,
    Hotload,
    Ultra,
    Rapid,
    Rotary2,
    Rotary3,
    Rotary4,
    Rotary5,
    Rotary6,
    Gatling,
}

impl BattleFireMode {
    /// Heat for one completed launch, shared by all carriers and their supply fallback modes.
    pub(super) fn launch_heat(
        self,
        weapon: BattleWeapon,
        gatling_damage: Option<u8>,
        launched: bool,
    ) -> u8 {
        if !launched {
            return 0;
        }
        gatling_damage.unwrap_or(weapon.profile().heat * self.rounds_per_cycle() as u8)
    }

    /// Coolant heat mode redirects the shot to its carrier before target selection.
    pub(super) fn self_cooling(self, weapon: BattleWeapon) -> bool {
        self == Self::Heat && weapon == BattleWeapon::CoolantGun
    }

    /// Whether this firing behavior is valid for the installed weapon family.
    pub(crate) fn supports(self, weapon: BattleWeapon) -> bool {
        match self {
            Self::Normal => true,
            Self::Heat => weapon.supports_heat_mode(),
            Self::Hotload => weapon.supports_hotload(),
            Self::Ultra => weapon.is_ultra(),
            Self::Rapid => weapon.supports_rapid_fire(),
            Self::Gatling => weapon.supports_gatling(),
            Self::Rotary2 | Self::Rotary3 | Self::Rotary4 | Self::Rotary5 | Self::Rotary6 => {
                weapon.is_rotary()
            }
        }
    }

    /// Hotload command feedback for both control surfaces.
    pub(crate) fn hotload_message(self, index: usize) -> String {
        format!(
            "Hotloading for weapon {index} has been toggled {}.",
            if self == Self::Hotload { "on" } else { "off" }
        )
    }

    /// Shared native/Lua cockpit feedback for a numbered weapon.
    pub(crate) fn message(self, index: usize) -> String {
        format!(
            "Weapon {index} has been set to {} mode",
            if self == Self::Heat { "HEAT" } else { "normal" }
        )
    }
}

impl BattleUnit {
    /// Current firing mode, distinct from the template's initial equipment flags.
    pub fn fire_mode(&self, index: usize) -> Result<BattleFireMode> {
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
) -> Result<BattleFireMode> {
    let readiness = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        readiness.weapon.supports_heat_mode(),
        "That weapon cannot be set HEAT!"
    );
    Ok(super::weapon_controls::toggle_fire_mode(
        world,
        id,
        index,
        BattleFireMode::Heat,
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
                ensure!(unit.power() == BattlePower::Running, "Start the unit first");
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
) -> Result<BattleFireMode> {
    let ready = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        ready.weapon.supports_hotload(),
        "That weapon can not be hotloaded!"
    );
    Ok(super::weapon_controls::toggle_fire_mode(
        world,
        id,
        index,
        BattleFireMode::Hotload,
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

impl BattleUnit {
    /// Select the firing behavior a supply check permits; the firing transaction persists fallback.
    pub(super) fn effective_fire_mode(&self, index: usize) -> Result<BattleFireMode> {
        let loadout = self.loadout()?;
        self.effective_fire_mode_with_loadout(&loadout, index)
    }

    /// Preserve live supply fallback while sharing this immutable equipment projection.
    pub(crate) fn effective_fire_mode_with_loadout(
        &self,
        loadout: &super::BattleLoadout,
        index: usize,
    ) -> Result<BattleFireMode> {
        ensure!(index < loadout.weapons.len(), "Weapon index out of bounds");
        effective_mode(
            self.fire_modes.get(&index).copied().unwrap_or_default(),
            |rounds| self.ammunition_feed_with_loadout(loadout, index, rounds),
        )
    }
}

/// Resolve burst fallback through the caller's unit-specific inventory query.
pub(super) fn effective_mode(
    mode: BattleFireMode,
    feed: impl FnOnce(u16) -> Result<Vec<super::BattleAmmunitionDraw>>,
) -> Result<BattleFireMode> {
    let requested = mode.rounds_per_cycle();
    if requested == 1 {
        return Ok(mode);
    }
    let available: u16 = feed(requested)?.iter().map(|draw| draw.rounds).sum();
    Ok(if available < requested {
        BattleFireMode::Normal
    } else {
        mode
    })
}

impl BattleWeapon {
    /// Two-shell hit table; glancing shifts the cluster roll instead of halving shell damage.
    pub(super) fn double_shot_damage_groups(self, roll: u8, glancing: bool) -> Result<Vec<u16>> {
        ensure!(
            self.is_ultra() || self.supports_rapid_fire(),
            "Weapon does not support two-round firing"
        );
        ensure!((2..=12).contains(&roll), "Invalid two-shell cluster roll");
        let hits = if roll.saturating_sub(if glancing { 4 } else { 0 }) >= 8 {
            2
        } else {
            1
        };
        Ok(vec![u16::from(self.profile().damage); hits])
    }
}

impl BattleFireMode {
    /// Requested rounds before ammunition shortage can reset the firing mode.
    pub(super) fn rounds_per_cycle(self) -> u16 {
        match self {
            Self::Ultra | Self::Rapid | Self::Rotary2 => 2,
            Self::Rotary3 => 3,
            Self::Rotary4 => 4,
            Self::Rotary5 => 5,
            Self::Rotary6 => 6,
            _ => 1,
        }
    }

    /// Rotary bursts jam at increasing thresholds and never destroy the loader directly: a
    /// to-hit roll at or below 2 jams two- and three-round bursts, 3 jams four and five, and 4
    /// jams six, as in MegaMek.
    pub(super) fn rotary_jam_threshold(self) -> u8 {
        match self {
            Self::Rotary2 | Self::Rotary3 => 2,
            Self::Rotary4 | Self::Rotary5 => 3,
            Self::Rotary6 => 4,
            _ => 0,
        }
    }

    /// Feed failures retain the mount and ammunition, requiring an explicit clearing attempt.
    pub(super) fn jams_on(self, roll: u8) -> bool {
        (self == Self::Hotload && roll <= 3)
            || (self == Self::Rapid && (3..=4).contains(&roll))
            || roll <= self.rotary_jam_threshold()
    }

    /// Ultra and conventional rapid fire have catastrophic loader failure on a two.
    pub(super) fn is_double_shot(self) -> bool {
        matches!(self, Self::Ultra | Self::Rapid)
    }
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

#[cfg(test)]
mod double_shot_tests {
    use crate::BattleWeapon;

    #[test]
    fn two_shell_tables_and_glancing_preserve_individual_damage() {
        for weapon in [
            BattleWeapon::UltraAc2,
            BattleWeapon::UltraAc5,
            BattleWeapon::UltraAc10,
            BattleWeapon::UltraAc20,
            BattleWeapon::Ac2,
            BattleWeapon::Ac5,
            BattleWeapon::Ac10,
            BattleWeapon::Ac20,
            BattleWeapon::LightAc2,
            BattleWeapon::LightAc5,
        ] {
            for roll in 2..=12 {
                for glancing in [false, true] {
                    let groups = weapon.double_shot_damage_groups(roll, glancing).unwrap();
                    assert_eq!(
                        groups.len(),
                        if roll >= if glancing { 12 } else { 8 } {
                            2
                        } else {
                            1
                        }
                    );
                    assert!(
                        groups
                            .iter()
                            .all(|&damage| damage == u16::from(weapon.profile().damage))
                    );
                }
            }
            for roll in [0, 1, 13, 255] {
                assert!(weapon.double_shot_damage_groups(roll, false).is_err());
            }
        }
        assert!(
            BattleWeapon::MediumLaser
                .double_shot_damage_groups(8, false)
                .is_err()
        );
    }
}
