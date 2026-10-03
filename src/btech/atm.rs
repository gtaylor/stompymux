//! Advanced Tactical Missiles: Extended Range and High Explosive ammunition profiles, and
//! ammunition controls that reuse indirect-launcher eligibility, feed selection and
//! transaction ordering.
use super::{BattleAmmunitionMode, BattleWeapon, WeaponProfile};
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

/// Cluster-roll bonus from the ATM's integral guidance, which ECM suppresses.
pub(super) const ATM_CLUSTER_BONUS: i16 = 2;

impl BattleWeapon {
    /// Clan Advanced Tactical Missile launchers.
    pub fn is_atm(self) -> bool {
        matches!(
            self,
            Self::ClanAtm3 | Self::ClanAtm6 | Self::ClanAtm9 | Self::ClanAtm12
        )
    }

    /// Extended Range missiles trade damage for reach and High Explosive missiles trade reach
    /// for damage; standard ammunition keeps the catalogue profile.
    pub(super) fn atm_profile(
        mut profile: WeaponProfile,
        ammunition: BattleAmmunitionMode,
    ) -> WeaponProfile {
        let (damage, minimum, short, medium, long) = match ammunition {
            BattleAmmunitionMode::ExtendedRange => (1, 4, 9, 18, 27),
            BattleAmmunitionMode::HighExplosive => (3, 0, 3, 6, 9),
            _ => return profile,
        };
        profile.damage = damage;
        profile.minimum_range = minimum;
        profile.short_range = short;
        profile.medium_range = medium;
        profile.long_range = long;
        profile
    }
}

impl BattleAmmunitionMode {
    /// Cockpit wording for the selected ATM ammunition.
    pub(crate) fn atm_message(self, index: usize) -> String {
        let label = match self {
            Self::ExtendedRange => "Extended Range",
            Self::HighExplosive => "High Explosive",
            _ => "normal",
        };
        format!("Weapon {index} has been set to fire {label} missiles.")
    }
}

/// Toggle either reference ATM ammunition marker using the shared supported indirect-launcher gate.
pub fn toggle_atm_ammunition(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
    mode: BattleAmmunitionMode,
) -> Result<BattleAmmunitionMode> {
    ensure!(
        matches!(
            mode,
            BattleAmmunitionMode::ExtendedRange | BattleAmmunitionMode::HighExplosive
        ),
        "Select Extended Range or High Explosive ammunition"
    );
    let ready = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        mode.supports(ready.weapon),
        "That weapon cannot fire this ammunition!"
    );
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world, id, index, mode,
    ))
}

/// Both native mode names share weapon selection and publication rollback.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let mode = if input.name == "atmrange" {
        BattleAmmunitionMode::ExtendedRange
    } else {
        BattleAmmunitionMode::HighExplosive
    };
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        toggle_atm_ammunition(world, id, pilot, index, mode).map(|mode| mode.atm_message(index))
    })
}
