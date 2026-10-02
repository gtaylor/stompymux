//! Torpedo launchers: LRTs and SRTs fire only underwater, at targets in the water.
use super::{BattleVehicleMovement, BattleWeapon};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

impl BattleWeapon {
    /// Long- and short-range torpedo launchers, which fire only from a submerged mount.
    pub fn is_torpedo(self) -> bool {
        matches!(
            self,
            Self::Lrt5
                | Self::Lrt10
                | Self::Lrt15
                | Self::Lrt20
                | Self::Srt2
                | Self::Srt4
                | Self::Srt6
                | Self::ClanLrt5
                | Self::ClanLrt10
                | Self::ClanLrt15
                | Self::ClanLrt20
                | Self::ClanSrt2
                | Self::ClanSrt4
                | Self::ClanSrt6
        )
    }
}

/// Whether a unit sits in a water hex at or below the surface, where a torpedo can reach it.
/// Hovercraft skim over the water and are out of reach.
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
    if !tile.is_open_water() {
        return Ok(false);
    }
    let elevation = if let Some(vehicle) = world.btech.vehicles().get(&target) {
        if vehicle.definition().movement == BattleVehicleMovement::Hover {
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
    Ok(elevation <= i32::from(tile.water_line()))
}

/// Torpedoes need a target in the water; other weapons are unaffected.
pub(super) fn check_target(world: &World, weapon: BattleWeapon, target: ObjectId) -> Result<()> {
    if !weapon.is_torpedo() {
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

    /// Torpedo launchers keep their ordinary reach underwater and take no special munitions
    /// beyond Artemis guidance.
    #[test]
    fn torpedoes_fire_underwater_at_their_ordinary_ranges() {
        for weapon in [
            BattleWeapon::Lrt15,
            BattleWeapon::Srt2,
            BattleWeapon::ClanSrt6,
            BattleWeapon::ClanLrt20,
        ] {
            assert!(weapon.is_torpedo());
            let ranges = weapon.water_ranges().unwrap();
            let profile = weapon.profile();
            assert_eq!(
                (ranges.minimum_range, ranges.medium_range, ranges.long_range),
                (
                    profile.minimum_range,
                    profile.medium_range,
                    Some(profile.long_range)
                )
            );
            assert!(!weapon.supports_indirect_fire());
            assert!(super::super::BattleAmmunitionMode::Artemis.supports(weapon));
            for mode in [
                super::super::BattleAmmunitionMode::Smoke,
                super::super::BattleAmmunitionMode::Mine,
                super::super::BattleAmmunitionMode::Narc,
                super::super::BattleAmmunitionMode::Inferno,
            ] {
                assert!(!mode.supports(weapon), "{weapon:?} {mode:?}");
            }
        }
        assert!(!BattleWeapon::Lrm20.is_torpedo());
        assert_eq!(
            BattleWeapon::Lrt10
                .water_range_modifier(13.0, false)
                .unwrap()
                .unwrap()
                .modifier,
            2
        );
    }

    /// Torpedo salvos use their missile counterparts' cluster tables.
    #[test]
    fn torpedo_salvos_match_missile_launchers() {
        for (torpedo, launcher) in [
            (BattleWeapon::Lrt20, BattleWeapon::Lrm20),
            (BattleWeapon::ClanLrt10, BattleWeapon::ClanLrm10),
            (BattleWeapon::Srt4, BattleWeapon::Srm4),
            (BattleWeapon::ClanSrt6, BattleWeapon::ClanSrm6),
        ] {
            for roll in 2..=12 {
                assert_eq!(
                    torpedo.damage_groups(Some(roll)).unwrap(),
                    launcher.damage_groups(Some(roll)).unwrap()
                );
            }
        }
    }
}
