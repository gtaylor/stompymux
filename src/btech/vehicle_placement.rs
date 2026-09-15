//! Administrative vehicle placement coordinates containment, identity and shared battlefield slots.
use crate::{Flag, Kind, ObjectId, World};
use anyhow::{Result, ensure};

/// Validate live container ownership before publishing any placement mutation.
fn targets(world: &World, id: ObjectId, destination: ObjectId) -> Result<()> {
    ensure!(world.objects.get(&id).is_some_and(|object| object.kind == Kind::Thing && !object.flags.contains(Flag::Going)), "Vehicle target must be a live thing");
    ensure!(
        world
            .objects
            .get(&destination)
            .is_some_and(|object| matches!(object.kind, Kind::Room | Kind::Thing)
                && !object.flags.contains(Flag::Going)),
        "Destination must be a live room or thing"
    );
    ensure!(
        world.btech.vehicles()[&id].power() == super::BattlePower::Off,
        "Shut down the unit before administrative placement"
    );
    world.validate_move(id, destination)
}

/// Leave the battlefield for an ordinary container, releasing the shared membership slot.
pub(super) fn remove_vehicle(world: &mut World, id: ObjectId, destination: ObjectId) -> Result<()> {
    targets(world, id, destination)?;
    ensure!(
        !world.btech.maps().contains_key(&destination),
        "Use unit-place to enter a battlefield"
    );
    super::placement::detach_membership(world, id)?;
    world.objects.get_mut(&id).unwrap().location = Some(destination);
    Ok(())
}
