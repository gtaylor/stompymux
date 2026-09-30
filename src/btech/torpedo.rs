//! Torpedo rounds: LRM and SRM launchers fire them only underwater, at targets in the water.
use super::{BattleAmmunitionMode, BattleWaterRanges, BattleWeapon, Terrain};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

impl BattleWeapon {
    /// Standard LRM and SRM launchers, which may load long- and short-range torpedoes.
    pub fn supports_torpedo(self) -> bool {
        matches!(
            self,
            Self::Lrm5
                | Self::Lrm10
                | Self::Lrm15
                | Self::Lrm20
                | Self::ClanLrm5
                | Self::ClanLrm10
                | Self::ClanLrm15
                | Self::ClanLrm20
                | Self::Srm2
                | Self::Srm4
                | Self::Srm6
                | Self::ClanSrm2
                | Self::ClanSrm4
                | Self::ClanSrm6
        )
    }

    /// Underwater range bands for the loaded ammunition. Torpedoes keep their launcher's
    /// ordinary ranges; every other round uses the weapon's own water profile, if any.
    pub fn water_ranges_for(self, ammunition: BattleAmmunitionMode) -> Option<BattleWaterRanges> {
        if ammunition.munition() != BattleAmmunitionMode::Torpedo {
            return self.water_ranges();
        }
        let profile = self.profile();
        Some(BattleWaterRanges {
            minimum_range: profile.minimum_range,
            short_range: profile.short_range,
            medium_range: profile.medium_range,
            long_range: Some(profile.long_range),
        })
    }
}

impl BattleAmmunitionMode {
    /// Cockpit feedback shared by native commands and Lua.
    pub(crate) fn torpedo_message(self, index: usize) -> String {
        if self.munition() == Self::Torpedo {
            return format!("Weapon {index} has been set to fire torpedoes.");
        }
        format!("Weapon {index} has been set to fire normal missiles")
    }
}

/// Toggle a controlled, intact and recycled LRM or SRM launcher between missiles and torpedoes.
pub fn toggle_torpedo(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<BattleAmmunitionMode> {
    super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        super::weapon_controls::selectable_munition(
            world,
            id,
            index,
            BattleAmmunitionMode::Torpedo
        ),
        "That weapon cannot fire torpedoes!"
    );
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world,
        id,
        index,
        BattleAmmunitionMode::Torpedo,
    ))
}

/// Use the ordinary bounded multi-weapon selection and world/effect checkpoint.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        toggle_torpedo(world, id, pilot, index).map(|mode| mode.torpedo_message(index))
    })
}

/// Whether a unit sits in a water hex at or below the surface, where a torpedo can reach it.
pub(super) fn target_in_water(world: &World, target: ObjectId) -> Result<bool> {
    let position = super::scanner::scanner_unit(world, target)
        .context("Target is unavailable")?
        .position
        .context("Target is not placed")?;
    let tile = world
        .btech
        .maps()
        .get(&position.map)
        .context("Map not found")?
        .base_hex(i64::from(position.x), i64::from(position.y))?;
    if tile.terrain != Terrain::Water {
        return Ok(false);
    }
    let elevation = if let Some(vehicle) = world.btech.vehicles().get(&target) {
        if vehicle.definition().movement == super::BattleVehicleMovement::Hover {
            return Ok(false);
        }
        vehicle.elevation_level(tile)
    } else {
        world
            .btech
            .constructed_units()
            .get(&target)
            .context("Target is not constructed")?
            .elevation_level(tile)
    };
    Ok(elevation <= 0)
}

/// Torpedoes need a target in the water; other rounds are unaffected.
pub(super) fn check_target(
    world: &World,
    ammunition: BattleAmmunitionMode,
    target: ObjectId,
) -> Result<()> {
    if ammunition.munition() != BattleAmmunitionMode::Torpedo {
        return Ok(());
    }
    ensure!(
        target_in_water(world, target)?,
        "Torpedoes can only strike targets in the water!"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Torpedoes load into standard LRM and SRM launchers and keep their ordinary reach.
    #[test]
    fn torpedoes_use_standard_launchers_and_their_ranges() {
        for weapon in [
            BattleWeapon::Lrm15,
            BattleWeapon::Srm2,
            BattleWeapon::ClanSrm6,
        ] {
            assert!(BattleAmmunitionMode::Torpedo.supports(weapon));
            assert!(weapon.water_ranges().is_none());
            let ranges = weapon
                .water_ranges_for(BattleAmmunitionMode::Torpedo)
                .unwrap();
            assert_eq!(ranges.long_range, Some(weapon.profile().long_range));
        }
        for weapon in [
            BattleWeapon::StreakSrm6,
            BattleWeapon::Mml5,
            BattleWeapon::Nlrm20,
            BattleWeapon::MediumLaser,
        ] {
            assert!(!BattleAmmunitionMode::Torpedo.supports(weapon));
        }
        assert_eq!(
            BattleWeapon::Lrm10
                .water_range_modifier_for(BattleAmmunitionMode::Torpedo, 13.0, false)
                .unwrap()
                .unwrap()
                .modifier,
            2
        );
        assert!(BattleAmmunitionMode::Torpedo.bypasses_ams());
    }
}
