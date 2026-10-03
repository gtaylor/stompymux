//! Transactional runtime weapon values shared by firing, defenses, reports and experience.
use super::BattleWeapon;
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};

/// Largest legal recycle countdown, including timers started under an earlier runtime setting.
pub(super) const MAX_RECYCLE_SECONDS: u16 = 127;

/// Effective weapon values; changing these does not alter an already running countdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleWeaponValues {
    pub recycle_seconds: u8,
    pub battle_value: u32,
}

impl BattleWeaponValues {
    /// Initial values for a new runtime, before operator overrides.
    fn catalogue(weapon: BattleWeapon) -> Self {
        Self {
            recycle_seconds: weapon.profile().recycle_seconds,
            battle_value: u32::from(weapon.battle_value()),
        }
    }
}

/// Sparse runtime overrides participate in world transactions and reset on database reload.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleWeaponSettings(Arc<BTreeMap<BattleWeapon, BattleWeaponValues>>);

impl BattleWeaponSettings {
    /// Effective values for one catalogue weapon, independent of its mounting chassis.
    pub fn get(&self, weapon: BattleWeapon) -> BattleWeaponValues {
        self.0
            .get(&weapon)
            .copied()
            .unwrap_or_else(|| BattleWeaponValues::catalogue(weapon))
    }

    /// Recycle time reserved by a newly accepted weapon or defensive activation.
    pub fn recycle_seconds(&self, weapon: BattleWeapon) -> u8 {
        self.get(weapon).recycle_seconds
    }

    /// Weapon contribution to valuation and the Battle Value experience formula.
    pub fn battle_value(&self, weapon: BattleWeapon) -> u32 {
        self.get(weapon).battle_value
    }

    /// Reject malformed runtime snapshots before they can drive combat arithmetic.
    pub(super) fn validate(&self) -> Result<()> {
        ensure!(
            self.0.values().all(|value| (1..=MAX_RECYCLE_SECONDS)
                .contains(&u16::from(value.recycle_seconds))
                && value.battle_value <= i32::MAX as u32),
            "Invalid runtime weapon settings"
        );
        Ok(())
    }
}

/// Select one independently editable value while sharing authorization and override storage.
enum Setting {
    Recycle,
    BattleValue,
}

/// Validate all inputs before changing the runtime; catalogue defaults remove redundant entries.
fn set(
    world: &mut World,
    actor: ObjectId,
    name: &str,
    setting: Setting,
    value: i64,
) -> Result<BattleWeaponValues> {
    ensure!(
        crate::authority::is_wizard(world, actor),
        "Permission denied."
    );
    let weapon = BattleWeapon::parse_operator_name(name)?;
    apply(world, weapon, setting, value)
}

/// Apply validated typed values after the caller has established its command-specific authority.
fn apply(
    world: &mut World,
    weapon: BattleWeapon,
    setting: Setting,
    value: i64,
) -> Result<BattleWeaponValues> {
    let mut values = world.btech.weapon_settings.get(weapon);
    match setting {
        Setting::Recycle => {
            ensure!(
                (1..=i64::from(MAX_RECYCLE_SECONDS)).contains(&value),
                "Recycle time must be from 1 through 127"
            );
            values.recycle_seconds = value as u8;
        }
        Setting::BattleValue => {
            ensure!(
                (0..=i64::from(i32::MAX)).contains(&value),
                "Battle Value must be from 0 through 2147483647"
            );
            values.battle_value = value as u32;
        }
    }
    let overrides = Arc::make_mut(&mut world.btech.weapon_settings.0);
    if values == BattleWeaponValues::catalogue(weapon) {
        overrides.remove(&weapon);
    } else {
        overrides.insert(weapon, values);
    }
    Ok(values)
}

/// Set runtime recycle time for future launches without rewriting active countdowns.
pub fn set_weapon_recycle(
    world: &mut World,
    actor: ObjectId,
    name: &str,
    seconds: i64,
) -> Result<BattleWeaponValues> {
    set(world, actor, name, Setting::Recycle, seconds)
}

/// Set runtime Battle Value for subsequent valuation and experience calculations.
pub fn set_weapon_battle_value(
    world: &mut World,
    actor: ObjectId,
    name: &str,
    value: i64,
) -> Result<BattleWeaponValues> {
    set(world, actor, name, Setting::BattleValue, value)
}

/// Share native edit confirmation while retaining the single typed mutation path used by Lua.
pub(crate) fn edit_command(
    ctx: &crate::CommandContext<'_>,
    name: &str,
    value: i64,
    recycle: bool,
) -> Result<String> {
    let weapon = BattleWeapon::parse_operator_name(name)?;
    let values = super::edit_weapon_settings(ctx.scripts, ctx.player, name, value, recycle)?;
    Ok(format!(
        "{} for {} set to {}.",
        if recycle { "VRT" } else { "BV" },
        weapon.name(),
        if recycle {
            u32::from(values.recycle_seconds)
        } else {
            values.battle_value
        }
    ))
}

/// Standalone debug syntax accepts exactly a weapon name and signed 32-bit value.
fn native_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
    recycle: bool,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let args: Vec<_> = input.args.split_whitespace().collect();
        ensure!(args.len() == 2, "Expected weapon name and value");
        let value = args[1]
            .parse::<i32>()
            .context("Expected a signed 32-bit integer")?;
        edit_command(ctx, args[0], i64::from(value), recycle)
    })();
    Ok(match result {
        Ok(text) => crate::CommandAction::CommitReply(text),
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

/// Set the recycle duration for subsequent activations through the shared control.
pub(crate) fn recycle_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    native_command(ctx, input, true)
}

/// Set the valuation used by subsequent combat calculations through the shared control.
pub(crate) fn battle_value_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    native_command(ctx, input, false)
}

/// Selected DEBUG commands preserve catalogue permissions and exact reference diagnostics.
/// Public SETWBV shares storage validation with privileged APIs without changing their authority.
pub(crate) fn debug_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let args: Vec<_> = input
            .args
            .split([' ', '\t'])
            .filter(|arg| !arg.is_empty())
            .collect();
        ensure!(args.len() == 2, "Invalid arguments!");
        let value = args[1].parse::<i32>().context("Invalid value!")?;
        let recycle = input.name == "setvrt";
        if recycle {
            ensure!(value > 0, "VRT needs to be >0");
            ensure!(value <= 127, "VRT can be at max 127");
        } else {
            ensure!(value >= 0, "BV needs to be >=0");
        }
        let weapon = super::stock_selection::canonical_part(args[0])
            .and_then(BattleWeapon::from_part_id)
            .context("That is no weapon!")?;
        if recycle {
            return edit_command(ctx, weapon.name(), i64::from(value), true);
        }
        apply(
            &mut ctx.scripts.world_mut(),
            weapon,
            Setting::BattleValue,
            i64::from(value),
        )?;
        Ok(format!("BV for {} set to {value}.", weapon.name()))
    })();
    Ok(match result {
        Ok(text) => crate::CommandAction::CommitReply(text),
        Err(error) => crate::CommandAction::Report(crate::CommandReport::Reply(error.to_string())),
    })
}
