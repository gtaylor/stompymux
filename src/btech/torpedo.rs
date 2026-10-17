//! Torpedo launchers: LRTs and SRTs fire from and at units in the water, along a line that stays
//! in the water (Total Warfare p. 138), and the surface rules for every other weapon.
use super::{VehicleMovement, Weapon};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Whether a unit sits in a water hex at or below the surface, where a torpedo can reach it
/// and from where it can fire one. Hovercraft skim over the water and are out of reach.
pub(super) fn in_water(world: &World, target: ObjectId) -> Result<bool> {
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
        if vehicle.definition().movement == VehicleMovement::Hover {
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

/// Which targets a weapon can reach across the water's surface (Total Warfare pp. 107-108,
/// 138). A torpedo needs a target in the water and water along the whole line; a weapon fired
/// from a submerged mount cannot leave the water; and a weapon above the surface cannot reach
/// a unit entirely under it. A wading 'Mech's legs are under the surface and its upper body
/// above it, so only its leg weapons reach underwater targets.
pub(super) fn check_target(
    world: &World,
    weapon: Weapon,
    shooter: ObjectId,
    target: ObjectId,
    submerged: bool,
) -> Result<()> {
    if weapon.is_torpedo() {
        ensure!(
            in_water(world, target)?,
            "Torpedoes can only strike targets in the water!"
        );
        ensure!(
            !super::unit_terrain_los(world, shooter, target)?.dry,
            "Torpedoes need water all the way to the target!"
        );
        return Ok(());
    }
    if submerged {
        ensure!(
            in_water(world, target)?,
            "Submerged weapons can only strike targets in the water!"
        );
        return Ok(());
    }
    ensure!(
        !super::los::unit_underwater(world, target)?,
        "Only weapons under the water can reach a target beneath it!"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::WeaponSalvo;

    /// Torpedo launchers keep their ordinary reach underwater and take no special munitions
    /// beyond Artemis guidance.
    #[test]
    fn torpedoes_fire_underwater_at_their_ordinary_ranges() {
        for weapon in [
            Weapon::Lrt15,
            Weapon::Srt2,
            Weapon::ClanSrt6,
            Weapon::ClanLrt20,
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
            assert!(super::super::AmmunitionMode::Artemis.supports(weapon));
            for mode in [
                super::super::AmmunitionMode::Smoke,
                super::super::AmmunitionMode::Mine,
                super::super::AmmunitionMode::Narc,
                super::super::AmmunitionMode::Inferno,
            ] {
                assert!(!mode.supports(weapon), "{weapon:?} {mode:?}");
            }
        }
        assert!(!Weapon::Lrm20.is_torpedo());
        assert_eq!(
            Weapon::Lrt10
                .water_range_modifier(13.0)
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
            (Weapon::Lrt20, Weapon::Lrm20),
            (Weapon::ClanLrt10, Weapon::ClanLrm10),
            (Weapon::Srt4, Weapon::Srm4),
            (Weapon::ClanSrt6, Weapon::ClanSrm6),
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
