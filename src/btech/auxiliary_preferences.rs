//! Shared auxiliary runtime preferences, including startup-reset weapon safeties.
use crate::{ObjectId, World};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Safety concerns unsupported MechWarrior targets; debug and standing flags have no combat consumers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct AuxiliaryPreferences {
    #[serde(default)]
    pub player_killer: bool,
    pub bth_debug: bool,
    pub stand_anyway: bool,
}

/// Public field bits retained by these preferences.
pub(super) const MASK: u32 = 1 | 32 | 512;

/// Read configuration without borrowing mutable unit state or consuming random numbers.
fn read(world: &World, id: ObjectId) -> Result<AuxiliaryPreferences> {
    let unit = world.btech.unit(id).context("Unit is unavailable")?;
    Ok(unit.auxiliary_preferences())
}

/// Select the existing unit-owned preference storage after caller admission.
fn storage(world: &mut World, id: ObjectId) -> Result<&mut AuxiliaryPreferences> {
    super::with_unit_mut!(
        world.btech.unit_mut(id).context("Unit is unavailable")?,
        |unit| { Ok(&mut unit.auxiliary_preferences) }
    )
}

/// Project the field mask from the saved booleans.
pub(super) fn bits(world: &World, id: ObjectId) -> Result<u32> {
    let flags = read(world, id)?;
    Ok(if flags.bth_debug { 512 } else { 0 }
        | if flags.stand_anyway { 32 } else { 0 }
        | if flags.player_killer { 1 } else { 0 })
}

/// Apply already validated administrative bits to their sole stored representation.
pub(super) fn set_bits(world: &mut World, id: ObjectId, bits: u32) -> Result<()> {
    *storage(world, id)? = AuxiliaryPreferences {
        player_killer: bits & 1 != 0,
        bth_debug: bits & 512 != 0,
        stand_anyway: bits & 32 != 0,
    };
    Ok(())
}

/// Toggle the pilot's retained BTHDebug preference through the common cockpit boundary.
pub fn set_bth_debug(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    enabled: bool,
) -> Result<()> {
    super::radio::controlled(world, id, pilot)?;
    storage(world, id)?.bth_debug = enabled;
    Ok(())
}

impl super::BattleUnit {
    /// Current BTHDebug configuration; attack reports do not consume this flag.
    pub fn bth_debug(&self) -> bool {
        self.auxiliary_preferences.bth_debug
    }
}

impl super::BattleVehicle {
    /// Current BTHDebug configuration; attack reports do not consume this flag.
    pub fn bth_debug(&self) -> bool {
        self.auxiliary_preferences.bth_debug
    }
}

/// Read the positive safety setting without exposing the inverse persisted bit to controls.
pub(super) fn mw_safety(world: &World, id: ObjectId) -> Result<bool> {
    Ok(!read(world, id)?.player_killer)
}

/// Change the pilot's MechWarrior safety independently of teammate protection.
pub fn set_mw_safety(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    enabled: bool,
) -> Result<()> {
    super::radio::controlled(world, id, pilot)?;
    storage(world, id)?.player_killer = !enabled;
    Ok(())
}

impl super::BattleUnit {
    /// Whether the MechWarrior weapon-safety preference is enabled.
    pub fn mw_safety(&self) -> bool {
        !self.auxiliary_preferences.player_killer
    }
}

impl super::BattleVehicle {
    /// Whether the MechWarrior weapon-safety preference is enabled.
    pub fn mw_safety(&self) -> bool {
        !self.auxiliary_preferences.player_killer
    }
}
