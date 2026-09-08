//! Communication policy derived from world state and configuration.

use crate::{
    config::Config,
    flags::Flag,
    world::{Kind, ObjectId, World},
};

/// Determine whether a player is currently subject to in-character communication rules.
pub fn in_character(world: &World, config: &Config, player: ObjectId) -> bool {
    let Some(object) = world.objects.get(&player) else {
        return true;
    };
    if object.flags.contains(Flag::Gagged) {
        return true;
    }
    if config.battletech.ooc_comsys != 0 {
        return false;
    }
    let mut location = object.location;
    for _ in 0..100 {
        let Some(object) = location.and_then(|id| world.objects.get(&id)) else {
            return false;
        };
        if object.kind != Kind::Player {
            return object.flags.contains(Flag::InCharacter);
        }
        if location == object.location {
            break;
        }
        location = object.location;
    }
    location
        .and_then(|id| world.objects.get(&id))
        .is_some_and(|object| object.flags.contains(Flag::InCharacter))
}
