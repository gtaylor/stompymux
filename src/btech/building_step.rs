//! Shared construction-factor feedback when a unit enters a building's surface hex.
use super::{BattleChannelMessage, BattleNotice, BattlePosition};
use crate::{Flag, ObjectId, World};
use anyhow::Result;

/// Resolve a changed hex after movement consequences, inside the caller's transaction.
/// Concealed structures use the same perception roll and experience service as scans.
pub(super) fn entered(
    world: &mut World,
    id: ObjectId,
    previous: BattlePosition,
) -> Result<(Option<BattleNotice>, Option<BattleChannelMessage>)> {
    let unit = super::network_unit::unit(world, id)?;
    let Some(position) = unit.position() else {
        return Ok((None, None));
    };
    if position == previous
        || world
            .btech
            .constructed_units()
            .get(&id)
            .is_some_and(|u| u.hex_sync_pending())
    {
        return Ok((None, None));
    }
    let pilot = unit.pilot();
    let map = &world.btech.maps()[&position.map];
    let tile = map.base_hex(i64::from(position.x), i64::from(position.y))?;
    if super::unit_elevation(world, id)? != Some(i32::from(tile.surface_height())) {
        return Ok((None, None));
    }
    let Some(entrance) = map.building_at(super::BattleHexCoordinate {
        x: i32::from(position.x),
        y: i32::from(position.y),
    })?
    else {
        return Ok((None, None));
    };
    let interior = entrance.interior;
    let Some(building) = world.btech.maps().get(&interior).map(|m| m.building) else {
        return Ok((None, None));
    };
    if building.is_dropship()
        || world
            .objects
            .get(&interior)
            .is_none_or(|o| o.flags.contains(Flag::Going))
    {
        return Ok((None, None));
    }
    let mut experience = None;
    if building.is_hidden() {
        let Some(pilot) = pilot else {
            return Ok((None, None));
        };
        let (success, message) =
            super::perception_check::attempt(world, id, pilot, 0, crate::clock::wall_time())?;
        if !success {
            return Ok((None, None));
        }
        experience = message;
    }
    let name = super::building_entrance::structure_name(world, interior)?.to_ascii_uppercase();
    Ok((
        Some(BattleNotice {
            unit: id,
            text: format!("{name} has CF of {}.", building.integrity),
        }),
        experience,
    ))
}
