//! Rotary autocannon burst selection and full-damage shell grouping.
use super::{BattleFireMode, BattleWeapon};
use crate::{ObjectId, World};
use anyhow::{Result, bail, ensure};

/// Validate an explicit burst length independently of weapon selection or mutation.
fn burst_mode(rounds: u8) -> Result<BattleFireMode> {
    Ok(match rounds {
        1 => BattleFireMode::Normal,
        2 => BattleFireMode::Rotary2,
        4 => BattleFireMode::Rotary4,
        6 => BattleFireMode::Rotary6,
        _ => bail!("Rotary autocannons fire one, two, four or six rounds"),
    })
}

impl BattleWeapon {
    /// Rotary shell counts follow the corresponding SRM table; glancing shifts its roll down four.
    pub(super) fn rotary_damage_groups(
        self,
        mode: BattleFireMode,
        roll: u8,
        glancing: bool,
    ) -> Result<Vec<u16>> {
        ensure!(self.is_rotary(), "Weapon is not a rotary autocannon");
        ensure!((2..=12).contains(&roll), "Invalid rotary cluster roll");
        let table = match mode {
            BattleFireMode::Rotary2 => Self::Srm2,
            BattleFireMode::Rotary4 => Self::Srm4,
            BattleFireMode::Rotary6 => Self::Srm6,
            _ => bail!("Rotary burst mode is required"),
        };
        let adjusted = roll.saturating_sub(if glancing { 4 } else { 0 });
        let hits = if adjusted < 2 {
            1
        } else {
            table.missile_hits(adjusted)?
        };
        Ok(vec![u16::from(self.profile().damage); usize::from(hits)])
    }
}

/// Select an intact, recycled rotary weapon's burst length; repeated selection leaves it enabled.
/// Returns whether the mode changed. Supply fallback occurs during firing, not selection.
pub fn set_rotary(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
    rounds: u8,
) -> Result<bool> {
    let mode = burst_mode(rounds)?;
    let ready = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(ready.weapon.is_rotary(), "That weapon is not a RotaryAC!");
    Ok(super::weapon_controls::set_fire_mode(
        world, id, index, mode,
    ))
}

/// Exact burst-selection feedback shared by native and Lua controls after validated selection.
pub(crate) fn message(index: usize, rounds: u8, changed: bool) -> String {
    let count = match rounds {
        1 => "one shot",
        2 => "two shots",
        4 => "four shots",
        6 => "six shots",
        _ => unreachable!("validated burst length"),
    };
    format!(
        "Weapon {index} {} to fire {count} at a time.",
        if changed {
            "has been set"
        } else {
            "is already set"
        }
    )
}

/// The cockpit accepts a weapon selection and uses the first character of the optional rate.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let rounds = match input
        .args
        .split_whitespace()
        .nth(1)
        .and_then(|arg| arg.as_bytes().first())
        .copied()
    {
        Some(b'2') => 2,
        Some(b'4') => 4,
        Some(b'6') => 6,
        _ => 1,
    };
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        set_rotary(world, id, pilot, index, rounds).map(|changed| message(index, rounds, changed))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotary_shell_tables_and_glancing() {
        for weapon in [
            BattleWeapon::RotaryAc2,
            BattleWeapon::RotaryAc5,
            BattleWeapon::ClanRotaryAc2,
            BattleWeapon::ClanRotaryAc5,
            BattleWeapon::ClanRotaryAc10,
        ] {
            for (rounds, counts) in [
                (2, [1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2]),
                (4, [1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4]),
                (6, [2, 2, 3, 3, 4, 4, 4, 5, 5, 6, 6]),
            ] {
                for roll in 2..=12 {
                    for glancing in [false, true] {
                        let adjusted = roll - if glancing { 4_i16 } else { 0 };
                        let count = if adjusted < 2 {
                            1
                        } else {
                            counts[(adjusted - 2) as usize]
                        };
                        let groups = weapon
                            .rotary_damage_groups(burst_mode(rounds).unwrap(), roll as u8, glancing)
                            .unwrap();
                        assert_eq!(groups, vec![u16::from(weapon.profile().damage); count]);
                    }
                }
            }
        }
        for rounds in [0, 3, 5, 7, 255] {
            assert!(burst_mode(rounds).is_err());
        }
    }
}
