//! Shared command-network range selection without changing physical distance or firing permission.
use super::network_unit::unit as network_unit;
use super::{
    BattleAimModifiers, BattleHexCoordinate, BattlePower, BattleRangeBracket, BattleWeapon,
    BattleWeaponRange,
};
use crate::{BattleCommandNetwork, ObjectId, World};
use anyhow::Result;
use serde::Serialize;

/// Network-derived aiming distance; physical range remains in the enclosing aim report.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BattleNetworkRange {
    /// Network family selected for this calculation; classic C3 takes priority over C3i.
    pub kind: BattleCommandNetwork,
    /// Closest usable sighting, or the shooter's physical distance if no peer improves it.
    pub distance: f64,
    /// Peer supplying a strictly shorter sighting; None means the shooter remains closest.
    pub source: Option<ObjectId>,
}

/// Target identity or coordinates, kept separate so empty terrain needs no fictitious unit.
#[derive(Clone, Copy)]
pub(super) enum NetworkTarget {
    Unit(ObjectId),
    Hex(BattleHexCoordinate),
}

/// Apply command-network assistance only after physical reach and raw minimum-range checks have admitted network use.
pub(super) fn apply(
    world: &World,
    shooter: ObjectId,
    target: NetworkTarget,
    weapon: BattleWeapon,
    ammunition: super::BattleAmmunitionMode,
    submerged: bool,
    aim: &mut BattleAimModifiers,
) -> Result<()> {
    let unit = network_unit(world, shooter)?;
    let minimum = weapon.profile().minimum_range;
    if (unit.c3i_network.is_none() && unit.c3_network.is_none())
        || aim.range.is_none()
        || (minimum > 0 && aim.distance <= f64::from(minimum))
        || unit.power() != BattlePower::Running
        || super::electronic_field(world, shooter)?.blocks_outgoing_guidance()
    {
        return Ok(());
    }
    let kind = if !super::command_network::c3_members(world, shooter)?.is_empty() {
        BattleCommandNetwork::C3
    } else if !super::command_network::members(world, shooter)?.is_empty() {
        BattleCommandNetwork::C3i
    } else {
        return Ok(());
    };
    let members = super::command_network::active_members(world, shooter, kind, false)?;
    let selected = select(world, shooter, target, aim.distance, &members, kind)?;
    let water = submerged.then(|| weapon.water_ranges_for(ammunition));
    aim.range = bracket(weapon, aim.distance, selected.distance, water);
    aim.network_range = Some(selected);
    Ok(())
}

/// C3 brackets use shared distance, ignore a peer's minimum range, and disable extended range.
/// `water` holds the loaded round's underwater bands when the launcher is submerged.
fn bracket(
    weapon: BattleWeapon,
    physical: f64,
    shared: f64,
    water: Option<Option<super::BattleWaterRanges>>,
) -> Option<BattleWeaponRange> {
    let (short, medium, maximum) = if let Some(water) = water {
        let profile = water?;
        (
            profile.short_range,
            profile.medium_range,
            profile.effective_range(false),
        )
    } else {
        let profile = weapon.profile();
        (
            profile.short_range,
            profile.medium_range,
            u16::from(profile.long_range),
        )
    };
    if (physical + 0.95).floor() > f64::from(maximum) {
        return None;
    }
    let rounded = (shared + 0.95).floor();
    let (bracket, modifier) = if rounded > f64::from(medium) {
        (BattleRangeBracket::Long, 4)
    } else if rounded > f64::from(short) {
        (BattleRangeBracket::Medium, 2)
    } else {
        (BattleRangeBracket::Short, 0)
    };
    Some(BattleWeaponRange { bracket, modifier })
}

/// Closest usable network sighting, shared by weapon aim and network contact displays.
pub(super) fn select(
    world: &World,
    shooter: ObjectId,
    target: NetworkTarget,
    physical: f64,
    members: &[ObjectId],
    kind: BattleCommandNetwork,
) -> Result<BattleNetworkRange> {
    let mut selected = BattleNetworkRange {
        kind,
        distance: physical,
        source: None,
    };
    for &peer in members {
        if peer == shooter || matches!(target, NetworkTarget::Unit(id) if id == peer) {
            continue;
        }
        let unit = network_unit(world, peer)?;
        if unit.power() != BattlePower::Running
            || super::electronic_field(world, peer)?.blocks_outgoing_guidance()
        {
            continue;
        }
        // C3 range sharing checks power and interference, but not the peer pilot's consciousness.
        let distance = match target {
            NetworkTarget::Unit(target) => {
                super::visible_contact(world, peer, target)?.map(|contact| contact.range.spatial)
            }
            NetworkTarget::Hex(hex) => {
                // The network's coordinate targeting path requires positive map coordinates.
                if hex.x <= 0 || hex.y <= 0 {
                    continue;
                }
                let (_, distance) = super::los::unit_hex_los(world, peer, hex)?;
                super::visibility::hex_unblocked(world, peer, hex)?.then_some(distance)
            }
        };
        if let Some(distance) = distance.filter(|distance| *distance < selected.distance) {
            selected = BattleNetworkRange {
                kind,
                distance,
                source: Some(peer),
            };
        }
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Water C3 never extends physical reach, even with an adjacent observer.
    #[test]
    fn water_network_uses_water_bands_and_physical_limit() {
        for (shared, expected) in [(0.0, 0), (4.04, 0), (4.06, 2), (7.04, 2), (7.06, 4)] {
            assert_eq!(
                bracket(
                    BattleWeapon::Ppc,
                    10.04,
                    shared,
                    Some(BattleWeapon::Ppc.water_ranges())
                )
                .unwrap()
                .modifier,
                expected
            );
        }
        assert!(
            bracket(
                BattleWeapon::Ppc,
                10.06,
                0.0,
                Some(BattleWeapon::Ppc.water_ranges())
            )
            .is_none()
        );
        assert!(
            bracket(
                BattleWeapon::SmallLaser,
                2.04,
                0.0,
                Some(BattleWeapon::SmallLaser.water_ranges())
            )
            .is_some()
        );
        assert!(
            bracket(
                BattleWeapon::SmallLaser,
                2.06,
                0.0,
                Some(BattleWeapon::SmallLaser.water_ranges())
            )
            .is_none()
        );
        assert!(
            bracket(
                BattleWeapon::Lrm20,
                1.0,
                0.0,
                Some(BattleWeapon::Lrm20.water_ranges())
            )
            .is_none()
        );
    }

    #[test]
    fn brackets_preserve_rounding_and_ignore_peer_minimum_range() {
        for (distance, expected) in [(7.049, 0), (7.051, 2), (14.049, 2), (14.051, 4)] {
            assert_eq!(
                bracket(BattleWeapon::Lrm20, 20.0, distance, None)
                    .unwrap()
                    .modifier,
                expected
            );
        }
        assert_eq!(
            bracket(BattleWeapon::Lrm20, 20.0, 0.0, None)
                .unwrap()
                .modifier,
            0
        );
        assert!(bracket(BattleWeapon::Lrm20, 21.051, 1.0, None).is_none());
        assert!(bracket(BattleWeapon::Lrm20, 21.049, 1.0, None).is_some());
    }
}
