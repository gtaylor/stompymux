//! Shared command-network range selection without changing physical distance or firing permission.
use super::network_unit::unit as network_unit;
use super::{
    AimModifiers, AmmunitionMode, HexCoordinate, Power, RangeBracket, Weapon, WeaponRange,
};
use crate::{CommandNetwork, ObjectId, World};
use anyhow::Result;
use serde::Serialize;

/// Network-derived aiming distance; physical range remains in the enclosing aim report.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct NetworkRange {
    /// Network family selected for this calculation; classic C3 takes priority over C3i.
    pub kind: CommandNetwork,
    /// Hexes from the closest usable sighting to the target, or the shooter's own weapon
    /// range if no peer improves it. Like any weapon range, it is a hex count (Total Warfare
    /// p. 109).
    pub distance: f64,
    /// Peer supplying a strictly shorter sighting; None means the shooter remains closest.
    pub source: Option<ObjectId>,
}

/// Target identity or coordinates, kept separate so empty terrain needs no fictitious unit.
#[derive(Clone, Copy)]
pub(super) enum NetworkTarget {
    Unit(ObjectId),
    Hex(HexCoordinate),
}

/// Apply command-network assistance only after physical reach and raw minimum-range checks have
/// admitted network use. Minimum range is always measured from the attacker to the target, so
/// a shot inside it keeps its own bracket, and a network gives no help to LRM indirect fire
/// (Total Warfare pp. 131-132); callers set `aim.indirect` first. Brackets follow the
/// ammunition the weapon fires, such as an MML's LRM rounds or an ATM's ER rounds.
pub(super) fn apply(
    world: &World,
    shooter: ObjectId,
    target: NetworkTarget,
    (weapon, ammunition): (Weapon, AmmunitionMode),
    submerged: bool,
    aim: &mut AimModifiers,
) -> Result<()> {
    let unit = network_unit(world, shooter)?;
    if (unit.c3i_network.is_none() && unit.c3_network.is_none())
        || aim.indirect.is_some()
        || aim
            .range
            .is_none_or(|range| range.bracket == RangeBracket::Minimum)
        || unit.power() != Power::Running
        || super::electronic_field(world, shooter)?.blocks_outgoing_guidance()
    {
        return Ok(());
    }
    let kind = if !super::command_network::c3_members(world, shooter)?.is_empty() {
        CommandNetwork::C3
    } else if !super::command_network::members(world, shooter)?.is_empty() {
        CommandNetwork::C3i
    } else {
        return Ok(());
    };
    let members = super::command_network::active_members(world, shooter, kind, false)?;
    let selected = select(world, shooter, target, aim.distance, &members, kind)?;
    aim.range = bracket(
        (weapon, ammunition),
        aim.distance,
        selected.distance,
        submerged,
    );
    aim.network_range = Some(selected);
    Ok(())
}

/// C3 brackets use shared distance, ignore a peer's minimum range, and disable extended range.
/// Out of water they follow the loaded ammunition's range profile (Total Warfare p. 131).
fn bracket(
    (weapon, ammunition): (Weapon, AmmunitionMode),
    physical: f64,
    shared: f64,
    submerged: bool,
) -> Option<WeaponRange> {
    let (short, medium, maximum) = if submerged {
        let profile = weapon.water_ranges()?;
        (
            profile.short_range,
            profile.medium_range,
            profile.effective_range(),
        )
    } else {
        let profile = weapon.profile_for_ammunition(ammunition);
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
        (RangeBracket::Long, 4)
    } else if rounded > f64::from(short) {
        (RangeBracket::Medium, 2)
    } else {
        (RangeBracket::Short, 0)
    };
    Some(WeaponRange { bracket, modifier })
}

/// Closest usable network sighting, shared by weapon aim and network contact displays.
pub(super) fn select(
    world: &World,
    shooter: ObjectId,
    target: NetworkTarget,
    physical: f64,
    members: &[ObjectId],
    kind: CommandNetwork,
) -> Result<NetworkRange> {
    let mut selected = NetworkRange {
        kind,
        distance: physical,
        source: None,
    };
    for &peer in members {
        if peer == shooter || matches!(target, NetworkTarget::Unit(id) if id == peer) {
            continue;
        }
        let unit = network_unit(world, peer)?;
        if unit.power() != Power::Running
            || super::electronic_field(world, peer)?.blocks_outgoing_guidance()
        {
            continue;
        }
        // C3 range sharing checks power and interference, but not the peer pilot's consciousness.
        // The peer must perceive the target itself with a line of fire to it (Total Warfare
        // p. 132); a row its side relays to it does not count.
        let distance = match target {
            NetworkTarget::Unit(target) => super::contacts::ContactReader::new(world, peer)?
                .facts(target)?
                .filter(|facts| facts.detection.is_some() && facts.line_of_fire)
                .map(|facts| facts.range.hexes()),
            NetworkTarget::Hex(hex) => {
                // The network's coordinate targeting path requires positive map coordinates.
                if hex.x <= 0 || hex.y <= 0 {
                    continue;
                }
                super::visibility::hex_unblocked(world, peer, hex)?
                    .then(|| super::geometry::unit_hex_range(world, peer, hex))
                    .transpose()?
            }
        };
        if let Some(distance) = distance.filter(|distance| *distance < selected.distance) {
            selected = NetworkRange {
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
                bracket((Weapon::Ppc, AmmunitionMode::Normal), 10.04, shared, true)
                    .unwrap()
                    .modifier,
                expected
            );
        }
        assert!(bracket((Weapon::Ppc, AmmunitionMode::Normal), 10.06, 0.0, true).is_none());
        assert!(
            bracket(
                (Weapon::SmallLaser, AmmunitionMode::Normal),
                2.04,
                0.0,
                true
            )
            .is_some()
        );
        assert!(
            bracket(
                (Weapon::SmallLaser, AmmunitionMode::Normal),
                2.06,
                0.0,
                true
            )
            .is_none()
        );
        assert!(bracket((Weapon::Lrm20, AmmunitionMode::Normal), 1.0, 0.0, true).is_none());
    }

    #[test]
    fn brackets_preserve_rounding_and_ignore_peer_minimum_range() {
        for (distance, expected) in [(7.049, 0), (7.051, 2), (14.049, 2), (14.051, 4)] {
            assert_eq!(
                bracket(
                    (Weapon::Lrm20, AmmunitionMode::Normal),
                    20.0,
                    distance,
                    false
                )
                .unwrap()
                .modifier,
                expected
            );
        }
        assert_eq!(
            bracket((Weapon::Lrm20, AmmunitionMode::Normal), 20.0, 0.0, false)
                .unwrap()
                .modifier,
            0
        );
        assert!(bracket((Weapon::Lrm20, AmmunitionMode::Normal), 21.051, 1.0, false).is_none());
        assert!(bracket((Weapon::Lrm20, AmmunitionMode::Normal), 21.049, 1.0, false).is_some());
    }

    /// Shared brackets follow the ammunition being fired (Total Warfare p. 131; TechManual
    /// p. 229): an MML on LRM rounds reaches 21 hexes on 7/14/21, its SRM rounds stop at 9, and
    /// an ATM on ER rounds reaches 27 hexes on 9/18/27 instead of the standard 15.
    #[test]
    fn brackets_follow_the_loaded_ammunition() {
        let mml_lrm = (Weapon::Mml9, AmmunitionMode::MmlLrm);
        for (shared, expected) in [(7.049, 0), (7.051, 2), (14.049, 2), (14.051, 4)] {
            assert_eq!(
                bracket(mml_lrm, 20.0, shared, false).unwrap().modifier,
                expected
            );
        }
        assert!(bracket(mml_lrm, 21.049, 1.0, false).is_some());
        assert!(bracket(mml_lrm, 21.051, 1.0, false).is_none());
        let mml_srm = (Weapon::Mml9, AmmunitionMode::Normal);
        assert!(bracket(mml_srm, 9.049, 1.0, false).is_some());
        assert!(bracket(mml_srm, 9.051, 1.0, false).is_none());
        let atm_er = (Weapon::ClanAtm6, AmmunitionMode::ExtendedRange);
        for (shared, expected) in [(9.049, 0), (9.051, 2), (18.049, 2), (18.051, 4)] {
            assert_eq!(
                bracket(atm_er, 26.0, shared, false).unwrap().modifier,
                expected
            );
        }
        assert!(bracket(atm_er, 27.049, 1.0, false).is_some());
        assert!(bracket(atm_er, 27.051, 1.0, false).is_none());
        let atm_he = (Weapon::ClanAtm6, AmmunitionMode::HighExplosive);
        assert!(bracket(atm_he, 9.051, 1.0, false).is_none());
    }
}
