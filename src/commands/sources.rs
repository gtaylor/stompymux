//! Deterministic, nonrecursive command-source discovery shared with catalog inspection.
use crate::{
    flags::Flag,
    world::{Kind, ObjectId, World},
};
use std::collections::BTreeSet;

/// An eligible object and its position in local command fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandSource {
    pub object: ObjectId,
    pub stage: &'static str,
}

/// Runtime eligibility, rechecked before each invocation.
pub fn eligible(world: &World, id: ObjectId) -> bool {
    world.objects.get(&id).is_some_and(|o| {
        o.kind != Kind::Garbage
            && ![Flag::Going, Flag::Halted, Flag::NoCommand]
                .into_iter()
                .any(|f| o.flags.contains(f))
    })
}

/// Capture direct sources once; callbacks cannot grow the current command's search.
pub fn sources(world: &World, player: ObjectId) -> Vec<CommandSource> {
    let Some(actor) = world.objects.get(&player) else {
        return Vec::new();
    };
    let location = actor.location;
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    let mut add = |object, stage| {
        if seen.insert(object) && eligible(world, object) {
            result.push(CommandSource { object, stage });
        }
    };
    add(player, "caller");
    if let Some(location) = location {
        for o in world
            .objects
            .values()
            .filter(|o| o.location == Some(location) && o.kind != Kind::Exit)
        {
            add(o.id, "nearby");
        }
        add(location, "location");
    }
    for o in world
        .objects
        .values()
        .filter(|o| o.location == Some(player) && o.kind != Kind::Exit)
    {
        add(o.id, "inventory");
    }
    let zone = location
        .and_then(|id| world.objects.get(&id))
        .and_then(|o| o.zone);
    if let Some(zone) = zone.and_then(|id| world.objects.get(&id)) {
        if zone.kind == Kind::Room {
            for o in world
                .objects
                .values()
                .filter(|o| o.location == Some(zone.id) && o.kind != Kind::Exit)
            {
                add(o.id, "location-zone fallback");
            }
        } else {
            add(zone.id, "location-zone fallback");
        }
    }
    if let Some(zone) = actor.zone.filter(|id| Some(*id) != zone) {
        add(zone, "player-zone fallback");
    }
    result
}
