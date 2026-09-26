//! Shared removal of map-owned objects and reciprocal building return routes.
use crate::{ObjectId, World};
use anyhow::{Context, Result};

/// Remove all owned map objects, leaving terrain, units and general environmental settings intact.
pub(super) fn clear(world: &mut World, id: ObjectId) -> Result<()> {
    let entrances: Vec<_> = world
        .btech
        .maps()
        .get(&id)
        .context("Map not found")?
        .building_entrances
        .keys()
        .copied()
        .collect();
    for ordinal in entrances {
        super::set_building_entrance(world, id, ordinal, None)?;
    }
    let maps = &mut world.btech.maps;
    let map = maps.get_mut(&id).unwrap();
    map.decorations = Default::default();
    map.static_decorations = Default::default();
    map.minefields = Default::default();
    map.minefield_order = Default::default();
    map.landing_exclusions = Default::default();
    map.lookup_bits = None;
    map.landing_exclusion_order = Default::default();
    map.building_entrances = Default::default();
    map.building_entry_points = Default::default();
    map.building_exits = Default::default();
    map.linked_markers = Default::default();
    map.flags &= !1;
    Ok(())
}
