//! Shared cloud-boundary policy for supported units and persisted operator controls.
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Change a map's cloud boundary; zero disables it and negative levels remain meaningful underground.
pub fn set_map_cloud_base(
    world: &mut World,
    actor: ObjectId,
    map: ObjectId,
    altitude: i16,
) -> Result<()> {
    ensure!(
        crate::authority::is_wizard(world, actor),
        "Permission denied."
    );
    ensure!(
        world
            .objects
            .get(&map)
            .is_some_and(|object| object.kind != crate::Kind::Garbage
                && !object.flags.contains(crate::Flag::Going)),
        "Map is unavailable"
    );
    let map = world.btech.maps.get_mut(&map).context("Map not found")?;
    map.cloud_base = altitude;
    Ok(())
}

/// Whether a sightline between two elevation levels crosses an enabled cloud boundary.
/// A level equal to the base belongs to its upper side.
pub(super) fn crosses(base: i16, first: i32, second: i32) -> bool {
    let base = i32::from(base);
    base != 0 && ((first < base) != (second < base))
}

#[cfg(test)]
mod tests {
    use super::crosses;

    /// Zero disables the boundary, and equality counts as above it.
    #[test]
    fn crossing_uses_the_upper_side_for_equality() {
        assert!(!crosses(0, 0, 50));
        assert!(crosses(10, 9, 10));
        assert!(!crosses(10, 10, 20));
        assert!(!crosses(10, 0, 9));
        assert!(crosses(-2, -3, 0));
    }
}
